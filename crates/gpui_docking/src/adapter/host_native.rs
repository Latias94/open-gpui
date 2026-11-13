//! Native execution for the same retained host and the same docking session.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    rc::{Rc, Weak},
    sync::Arc,
};

use anyhow::{Result, anyhow};
use dockspace::{
    geometry::{LogicalSize, PhysicalPoint, PhysicalRect, ScaleFactor},
    model::SurfaceId,
    runtime::{
        DockspaceHostFrame, DockspaceSession, HostFrameReport, HostWindowToken,
        NativeHostCapability, NativeReceiverAnswer, NativeReceiverPurpose, NativeReceiverQuery,
        NativeSurfaceBinding, NativeSurfaceCloseAction, NativeSurfaceCloseRequest,
        NativeSurfacePlacement, NativeWindowBinding, NativeWindowHit,
    },
};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, IntoElement, Pixels, Render,
    Subscription, Window, WindowId, WindowOptions, div, point, prelude::*, px,
};

use super::{
    DockSurface, HostState, SharedHost,
    host::WindowMount,
    native::{
        NativeBridge, NativeBridgeError, NativeCallbacks, NativeCloseDecision, NativeGeometry,
        NativeOpenFailure, NativeReport, NativeTransport, NativeWindowRequest,
    },
    presentation,
};

#[cfg(test)]
#[path = "host_native_tests.rs"]
mod tests;

type WindowFactory = dyn Fn(
    &DockSurface,
    NativeWindowRequest,
    &NativeCallbacks,
    &mut App,
) -> std::result::Result<AnyWindowHandle, NativeOpenFailure>;

pub(super) struct ManagedHost {
    bridge: Option<NativeBridge>,
    pub(super) transport: NativeTransport,
    pub(super) callbacks: BTreeMap<WindowId, NativeCallbacks>,
    roots: BTreeSet<WindowId>,
    pending_reports: VecDeque<HostFrameReport>,
    reports: VecDeque<NativeReport>,
    retired: BTreeSet<NativeSurfaceBinding>,
    close_policy: NativeCloseDecision,
    factory: Rc<WindowFactory>,
    _quit: Subscription,
    pump_scheduled: bool,
    wake: Option<NativeWake>,
    registrations: BTreeMap<WindowId, RootRegistration>,
    retirements: BTreeMap<WindowId, open_gpui::WindowPresentationShutdownTicket>,
    _terminal: Subscription,
}

struct RootRegistration {
    surface: SurfaceId,
    window: AnyWindowHandle,
    anchor: bool,
}

struct NativeWake {
    request: dockspace::runtime::NativePointerContinuation,
    _task: open_gpui::Task<()>,
}

pub(super) fn accept_owned_report(native: &mut ManagedHost, mut report: HostFrameReport) {
    if let Some(bridge) = &mut native.bridge {
        bridge.accept_report(&mut report);
    } else {
        native.pending_reports.push_back(report);
    }
}

pub(super) fn enroll(shared: &SharedHost, cx: &mut App) -> Result<()> {
    install_router(cx);
    let weak = Rc::downgrade(shared);
    let transport = NativeTransport::new(move |cx| {
        if let Some(shared) = weak.upgrade() {
            pump(&shared, cx);
        }
    });
    let mut capabilities =
        NativeBridge::gpui_capabilities(cx).with(NativeHostCapability::PointerHitTestObservation);
    let mutations = cx.window_capabilities().mutations;
    if mutations.physical_placement.is_live() {
        capabilities = capabilities.with(NativeHostCapability::GlobalWindowPlacement);
    }
    if mutations.pointer_input.is_live() {
        capabilities = capabilities.with(NativeHostCapability::PointerHitTestControl);
    }
    let bridge = NativeBridge::enable(
        &mut shared.borrow_mut().session,
        transport.clone(),
        capabilities,
        Some(Rc::new(NativeGeometry::gpui())),
    )?;
    let weak = Rc::downgrade(shared);
    let shutdown = transport.clone();
    let quit = cx.on_app_quit(move |cx| {
        shutdown.freeze_admission();
        let weak = weak.clone();
        cx.defer_shutdown_critical_before_window_registry_clear_or_run_now(move |cx| {
            if let Some(shared) = weak.upgrade() {
                pump(&shared, cx);
            }
        });
        async {}
    });
    let wake = transport.clone();
    let terminal = cx.on_window_native_terminal(move |cx, _| wake.wake(cx));
    shared.borrow_mut().native = Some(ManagedHost {
        bridge: Some(bridge),
        transport,
        callbacks: BTreeMap::new(),
        roots: BTreeSet::new(),
        pending_reports: VecDeque::new(),
        reports: VecDeque::new(),
        retired: BTreeSet::new(),
        close_policy: NativeCloseDecision::Allow(NativeSurfaceCloseAction::RecoverPresentation),
        factory: Rc::new(default_factory),
        _quit: quit,
        pump_scheduled: false,
        wake: None,
        registrations: BTreeMap::new(),
        retirements: BTreeMap::new(),
        _terminal: terminal,
    });
    Ok(())
}

struct NativeRouter {
    _subscription: Subscription,
}
impl open_gpui::Global for NativeRouter {}

struct NativeDragPayload {
    host: Weak<RefCell<HostState>>,
}

fn install_router(cx: &mut App) {
    if cx.has_global::<NativeRouter>() {
        return;
    }
    let subscription = cx.consume_native_captured_drag(
        |_, _, _| Arc::new(()),
        |event, cx| {
            let Some(shared) = event
                .payload::<NativeDragPayload>()
                .and_then(|payload| payload.host.upgrade())
            else {
                return;
            };
            {
                let host = shared.borrow();
                let metadata = dockspace::runtime::PointerEventMetadata::new(
                    dockspace::runtime::PointerDeviceKind::Mouse,
                    host.observed_at(),
                );
                if let Some(native) = &host.native
                    && let Some(callbacks) = native.callbacks.get(&event.source_window())
                {
                    native.transport.captured_drag(
                        callbacks.binding(),
                        event,
                        dockspace::runtime::NativePointerId::new(1),
                        metadata,
                        cx,
                    );
                }
            }
            pump(&shared, cx);
        },
    );
    cx.set_global(NativeRouter {
        _subscription: subscription,
    });
}

pub(super) fn wrap(shared: &SharedHost, element: open_gpui::AnyElement) -> open_gpui::AnyElement {
    if shared.borrow().native.is_none() {
        return element;
    }
    div()
        .id("dockspace-native-capture")
        .size_full()
        .on_drag(
            NativeDragPayload {
                host: Rc::downgrade(shared),
            },
            |payload, geometry, window, cx| {
                if let Some(shared) = payload.host.upgrade() {
                    start_drag(&shared, geometry, window, cx);
                }
                cx.new(|_| open_gpui::Empty)
            },
        )
        .child(element)
        .into_any_element()
}

fn start_drag(
    shared: &SharedHost,
    geometry: &open_gpui::DragStartGeometry,
    window: &Window,
    cx: &mut App,
) {
    let consumer = geometry.prepare_native_captured_drag_consumer();
    let host = shared.borrow();
    let Some(native) = &host.native else {
        return;
    };
    let Some(callbacks) = native.callbacks.get(&window.window_handle().window_id()) else {
        return;
    };
    let binding = callbacks.binding();
    let Some(sample) = local_sample(&host.session, binding, geometry.window_position(), window)
    else {
        return;
    };
    let metadata = dockspace::runtime::PointerEventMetadata::new(
        dockspace::runtime::PointerDeviceKind::Mouse,
        host.observed_at(),
    );
    let transport = native.transport.clone();
    let weak = Rc::downgrade(shared);
    drop(host);
    cx.defer(move |cx| {
        if !consumer.is_active() {
            return;
        }
        use dockspace::runtime::{
            NativeLocalPointerLocation, NativePointerEvent, NativePointerId, NativePointerInput,
            NativePointerOwner,
        };
        transport.pointer(
            NativePointerInput::new_local(
                NativePointerId::new(1),
                NativePointerEvent::Moved,
                NativeLocalPointerLocation::Exact(sample),
                NativePointerOwner::Native(binding),
                NativePointerOwner::Unknown,
            )
            .with_metadata(metadata),
            cx,
        );
        if let Some(shared) = weak.upgrade() {
            pump(&shared, cx);
        }
    });
}

impl DockSurface {
    /// Returns the native FIFO for an owning platform's original input callbacks.
    pub fn native_transport(&self) -> Option<NativeTransport> {
        self.host
            .borrow()
            .native
            .as_ref()
            .map(|native| native.transport.clone())
    }

    /// Reads a finite native request without supplying pointer or capture authority.
    pub fn native_pointer_continuation(
        &self,
    ) -> Option<dockspace::runtime::NativePointerContinuation> {
        self.host.borrow().session.native_pointer_continuation()
    }

    /// Returns the original callback route for a physical window in this workspace.
    pub fn native_callbacks_for_window(&self, window: WindowId) -> Option<NativeCallbacks> {
        let host = self.host.borrow();
        let native = host.native.as_ref()?;
        native
            .callbacks
            .get(&window)
            .cloned()
            .or_else(|| native.bridge.as_ref()?.callbacks_for_window(window))
    }

    /// Returns the current core-minted lifetime for a physical window.
    pub fn native_binding_for_window(&self, window: WindowId) -> Option<NativeWindowBinding> {
        self.native_callbacks_for_window(window)
            .map(|callbacks| callbacks.binding())
    }

    /// Registers an application root during its original GPUI construction/update.
    ///
    /// Register once per physical window. Additional logical regions in that
    /// window are declared atomically by the element's complete placement roster.
    pub fn bind_native_root(
        &self,
        surface: SurfaceId,
        anchor: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> std::result::Result<(), NativeBridgeError> {
        let owner = window.window_handle().window_id();
        let mut host = self.host.borrow_mut();
        let native = host
            .native
            .as_ref()
            .ok_or(NativeBridgeError::UnknownBinding)?;
        let bridge = native
            .bridge
            .as_ref()
            .ok_or(NativeBridgeError::NotQuiescent)?;
        bridge.preflight_root_binding(window, anchor, cx)?;
        if native.registrations.contains_key(&owner)
            || host.session.view().surface(surface).is_none()
            || host.session.native_window_bindings().any(|binding| {
                host.session
                    .native_surface_bindings_for_window(binding)
                    .any(|binding| binding.surface() == surface)
            })
        {
            return Err(NativeBridgeError::UnknownBinding);
        }
        // A failed prelude must precede the irreversible recorder append. In
        // particular another Window's suspended candidate owns this lease.
        {
            let HostState {
                session,
                mounts,
                native,
                ..
            } = &mut *host;
            let preflight = session.begin_native_host_frame(|query| {
                resolve(query, mounts, native.as_ref().unwrap(), Some(window), cx)
            })?;
            drop(preflight);
        }
        host.session
            .register_native_root(surface, HostWindowToken::new(owner.as_u64()))?;
        // After append, retain custody even if preparation, commit or callback
        // binding fails. The normal pump retries the same recorder prefix.
        host.native.as_mut().unwrap().registrations.insert(
            owner,
            RootRegistration {
                surface,
                window: window.window_handle(),
                anchor,
            },
        );
        let HostState {
            session,
            mounts,
            native,
            ..
        } = &mut *host;
        let frame = session.begin_native_host_frame(|query| {
            resolve(query, mounts, native.as_ref().unwrap(), Some(window), cx)
        })?;
        let report = frame.commit()?;
        let binding = report
            .native_bindings()
            .iter()
            .find(|binding| binding.surface() == surface)
            .copied();
        host.apply_report(report);
        let Some(binding) = binding else {
            host.native.as_mut().unwrap().registrations.remove(&owner);
            return Err(NativeBridgeError::UnknownBinding);
        };
        let native = host.native.as_mut().unwrap();
        let bridge = native
            .bridge
            .as_mut()
            .ok_or(NativeBridgeError::NotQuiescent)?;
        let result = bridge.bind_window_current(binding.window(), window, anchor, cx);
        if let Some(callbacks) = bridge.callbacks_for_window(owner) {
            native.callbacks.insert(owner, callbacks);
            native.roots.insert(owner);
            native.registrations.remove(&owner);
        }
        result.map(|_| ())
    }

    /// Opens a normal native root containing the original retained workspace Views.
    pub fn open_native_root(
        &self,
        surface: SurfaceId,
        anchor: bool,
        options: WindowOptions,
        cx: &mut App,
    ) -> std::result::Result<AnyWindowHandle, NativeOpenFailure> {
        let dock = self.clone();
        let failure = Rc::new(std::cell::RefCell::new(None));
        let root_failure = failure.clone();
        let window = cx.open_window_detailed(options, move |window, cx| {
            if let Err(error) = dock.bind_native_root(surface, anchor, window, cx) {
                *root_failure.borrow_mut() = Some(error);
                window.remove_window(cx);
            }
            cx.new(|_| NativeRoot { dock, surface })
        });
        let root_binding = failure.borrow_mut().take();
        match window {
            Ok(window) => Ok(window.into()),
            Err(source) => {
                self.retain_native_root_open_failure(&source, cx);
                let mut error = NativeOpenFailure::from(source);
                error.root_binding = root_binding;
                Err(error)
            }
        }
    }

    /// Retains the original rollback authority after a custom root builder fails.
    ///
    /// Call this with the error returned by `App::open_window_detailed` when
    /// mounting a custom root through `bind_native_root`. The owning GPUI
    /// transaction, rather than the binding error, supplies its native ticket.
    pub fn retain_native_root_open_failure(
        &self,
        error: &open_gpui::WindowOpenError,
        cx: &mut App,
    ) {
        let Some(ticket) = error.retirement_ticket().cloned() else {
            return;
        };
        let owner = ticket.snapshot().window_id();
        let mut host = self.host.borrow_mut();
        let Some(native) = &mut host.native else {
            return;
        };
        if let Some(callbacks) = native.callbacks.get(&owner) {
            callbacks.retain_open_retirement(ticket.clone(), cx);
        }
        native.retirements.insert(owner, ticket);
        if let Some(bridge) = &native.bridge {
            if let Err(error) =
                bridge.retain_root_retirement_dependencies(native.retirements.keys().copied(), cx)
            {
                host.fail(error);
            }
        }
        drop(host);
        schedule(&self.host, cx);
    }

    /// Selects ordinary native close policy. Anchor shutdown always drains the group.
    pub fn set_native_close_policy(&self, policy: NativeCloseDecision) -> Result<()> {
        let mut host = self.host.borrow_mut();
        host.native
            .as_mut()
            .ok_or_else(|| anyhow!("managed native hosting is not enabled"))?
            .close_policy = policy;
        Ok(())
    }

    /// Resolves a previously deferred exact native close edge.
    pub fn decide_native_close(
        &self,
        close: NativeSurfaceCloseRequest,
        decision: NativeCloseDecision,
        cx: &mut App,
    ) -> Result<()> {
        let mut host = self.host.borrow_mut();
        let HostState {
            native, session, ..
        } = &mut *host;
        native
            .as_mut()
            .and_then(|native| native.bridge.as_mut())
            .ok_or_else(|| anyhow!("native dispatch is already running or disabled"))?
            .decide_close(session, close, decision)?;
        drop(host);
        pump(&self.host, cx);
        Ok(())
    }

    /// Installs an application root factory while preserving the original native callbacks.
    pub fn set_native_window_factory(
        &self,
        factory: impl Fn(
            &DockSurface,
            NativeWindowRequest,
            &NativeCallbacks,
            &mut App,
        ) -> std::result::Result<AnyWindowHandle, NativeOpenFailure>
        + 'static,
    ) -> Result<()> {
        self.host
            .borrow_mut()
            .native
            .as_mut()
            .ok_or_else(|| anyhow!("managed native hosting is not enabled"))?
            .factory = Rc::new(factory);
        Ok(())
    }

    /// Drains native facts and deferred close edges intended for application policy/UI.
    pub fn take_native_reports(&self) -> Vec<NativeReport> {
        self.host
            .borrow_mut()
            .native
            .as_mut()
            .map_or_else(Vec::new, |native| native.reports.drain(..).collect())
    }

    /// Retries native progress after an application close decision or external wakeup.
    pub fn pump_native(&self, cx: &mut App) {
        pump(&self.host, cx);
    }

    /// Freezes this workspace and drains dependent windows before its anchor.
    ///
    /// This forced path bypasses ordinary close veto/defer policy. It never
    /// requests application exit or retires an isolated sibling workspace.
    pub fn begin_native_shutdown(&self, cx: &mut App) -> Result<()> {
        {
            let mut host = self.host.borrow_mut();
            let native = host
                .native
                .as_mut()
                .ok_or_else(|| anyhow!("managed native hosting is not enabled"))?;
            if !native.transport.is_frozen() {
                native.transport.freeze_admission();
                native
                    .reports
                    .push_back(NativeReport::GroupShutdownRequested);
            }
            native.wake = None;
        }
        pump(&self.host, cx);
        Ok(())
    }

    /// Returns whether this workspace has retired all physical, output and callback owners.
    ///
    /// Native terminal, original capture release, GPUI retirement and the core's
    /// permanent binding-quiescence boundary must converge independently.
    pub fn native_is_quiescent(&self) -> bool {
        let host = self.host.borrow();
        host.native.as_ref().is_some_and(|native| {
            native
                .bridge
                .as_ref()
                .is_some_and(NativeBridge::is_quiescent)
                && native.pending_reports.is_empty()
                && native.retired.is_empty()
                && native.registrations.is_empty()
                && native.retirements.is_empty()
                && native.callbacks.is_empty()
                && native.wake.is_none()
                && host.mounts.is_empty()
                && !host.session.has_pending_native_input()
        })
    }
}

struct NativeRoot {
    dock: DockSurface,
    surface: SurfaceId,
}

impl Render for NativeRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.dock.element(self.surface))
    }
}

fn default_factory(
    dock: &DockSurface,
    request: NativeWindowRequest,
    callbacks: &NativeCallbacks,
    cx: &mut App,
) -> std::result::Result<AnyWindowHandle, NativeOpenFailure> {
    let dock = dock.clone();
    let callbacks_for_root = callbacks.clone();
    callbacks.open(
        WindowOptions::default(),
        move |window, cx| {
            if let Some(native) = dock.host.borrow_mut().native.as_mut() {
                native
                    .callbacks
                    .insert(window.window_handle().window_id(), callbacks_for_root);
            }
            cx.new(|_| NativeRoot {
                dock,
                surface: request.binding.surface(),
            })
        },
        cx,
    )
}

pub(super) fn begin_frame<'session>(
    session: &'session mut DockspaceSession,
    mounts: &BTreeMap<WindowId, WindowMount>,
    native: Option<&ManagedHost>,
    current: Option<&Window>,
    cx: &mut App,
) -> Result<DockspaceHostFrame<'session>> {
    Ok(if let Some(native) = native {
        session.begin_native_host_frame(|query| resolve(query, mounts, native, current, cx))?
    } else {
        session.begin_host_frame()?
    })
}

pub(super) fn set_placements(
    frame: &mut DockspaceHostFrame<'_>,
    native: Option<&ManagedHost>,
    owner: WindowId,
    regions: &BTreeMap<SurfaceId, Bounds<Pixels>>,
    window: &Window,
) -> Result<()> {
    let Some(native) = native.filter(|native| native.roots.contains(&owner)) else {
        return Ok(());
    };
    let binding = native
        .callbacks
        .get(&owner)
        .ok_or_else(|| anyhow!("native root has no callback route"))?
        .binding();
    let scale = ScaleFactor::new(f64::from(window.scale_factor()))?;
    let placements: Result<Vec<_>> = regions
        .iter()
        .filter(|(surface, _)| frame.view().surface(**surface).is_some())
        .map(|(surface, bounds)| {
            let x = f64::from(f32::from(bounds.origin.x)) * scale.get();
            let y = f64::from(f32::from(bounds.origin.y)) * scale.get();
            let width = f64::from(f32::from(bounds.size.width));
            let height = f64::from(f32::from(bounds.size.height));
            Ok(NativeSurfacePlacement::new(
                *surface,
                LogicalSize::new(width, height)?,
                PhysicalPoint::new(x, y)?,
                scale,
                PhysicalRect::new(x, y, width * scale.get(), height * scale.get())?,
            ))
        })
        .collect();
    let placements = placements?;
    // Core already detaches a window's final placement when its ordinary root
    // disappears. An empty roster cannot redeclare that retired binding.
    if !placements.is_empty() {
        frame.set_native_surface_placements(binding, &placements)?;
    }
    Ok(())
}

fn resolve(
    query: NativeReceiverQuery,
    mounts: &BTreeMap<WindowId, WindowMount>,
    native: &ManagedHost,
    current: Option<&Window>,
    cx: &mut App,
) -> NativeReceiverAnswer {
    let owner = match query {
        NativeReceiverQuery::WindowHit(query) => {
            native.callbacks.iter().find_map(|(owner, callbacks)| {
                (callbacks.binding() == query.window()).then_some(*owner)
            })
        }
        NativeReceiverQuery::Surface(query) => mounts.iter().find_map(|(owner, mount)| {
            mount
                .regions
                .get(&query.surface())
                .and_then(|region| region.presented)
                .filter(|(_, artifact)| query.matches_presentation_artifact(*artifact))
                .and_then(|_| native.callbacks.get(owner))
                .map(|_| *owner)
        }),
    };
    let Some(owner) = owner else {
        return NativeReceiverAnswer::Unknown;
    };
    let Some(mount) = mounts.get(&owner) else {
        return NativeReceiverAnswer::Unknown;
    };
    // The immutable committed graph also works while another Window owns its
    // update borrow. Never enter a sibling's render/update path from the reducer.
    resolve_in_window(query, mount, |hitbox, point| {
        if let Some(window) = current.filter(|window| window.window_handle().window_id() == owner) {
            hitbox.window_point_target_rank(point, window)
        } else {
            cx.window_point_target_rank(mount.window, hitbox, point)
        }
    })
}

fn resolve_in_window(
    query: NativeReceiverQuery,
    mount: &WindowMount,
    target_rank: impl Fn(open_gpui::HitboxId, open_gpui::Point<Pixels>) -> Option<usize>,
) -> NativeReceiverAnswer {
    match query {
        NativeReceiverQuery::WindowHit(query) => {
            for frame in &mount.input_frames {
                let Some((serial, artifact)) = mount
                    .regions
                    .get(&frame.surface)
                    .and_then(|region| region.presented)
                else {
                    continue;
                };
                if serial != frame.serial {
                    continue;
                }
                let point = point(
                    px((query.point().x() / frame.scale) as f32),
                    px((query.point().y() / frame.scale) as f32),
                );
                if !frame.bounds.contains(&point) {
                    continue;
                }
                let exposed = frame
                    .hitboxes
                    .iter()
                    .any(|hitbox| target_rank(*hitbox, point) == Some(0));
                return NativeReceiverAnswer::WindowHit(if exposed {
                    NativeWindowHit::Surface(artifact)
                } else {
                    NativeWindowHit::Blocked
                });
            }
            NativeReceiverAnswer::WindowHit(NativeWindowHit::Unknown)
        }
        NativeReceiverQuery::Surface(query) => {
            let Some(frame) = mount.input_frames.iter().find(|frame| {
                frame.surface == query.surface()
                    && mount
                        .regions
                        .get(&frame.surface)
                        .and_then(|region| region.presented)
                        .is_some_and(|(serial, artifact)| {
                            serial == frame.serial && query.matches_presentation_artifact(artifact)
                        })
            }) else {
                return NativeReceiverAnswer::Unknown;
            };
            let Some(client) = query.client_position() else {
                return NativeReceiverAnswer::Unknown;
            };
            let point = point(
                px((client.x() / frame.scale) as f32),
                px((client.y() / frame.scale) as f32),
            );
            if let Some((descriptor, _)) = frame
                .receivers
                .iter()
                .find(|(_, hitbox)| target_rank(*hitbox, point) == Some(0))
            {
                if matches!(query.purpose(), NativeReceiverPurpose::HoverHit) {
                    query.clear_hover_lane(descriptor).map_or(
                        NativeReceiverAnswer::Unknown,
                        NativeReceiverAnswer::ClearHoverLane,
                    )
                } else {
                    query.bind_receiver(descriptor).map_or(
                        NativeReceiverAnswer::NoReceiver(query.presented_surface()),
                        NativeReceiverAnswer::Dock,
                    )
                }
            } else if frame
                .hitboxes
                .iter()
                .any(|hitbox| target_rank(*hitbox, point) == Some(0))
            {
                NativeReceiverAnswer::NoReceiver(query.presented_surface())
            } else {
                NativeReceiverAnswer::Blocked(query.presented_surface())
            }
        }
    }
}

pub(super) fn prepare_window(shared: &SharedHost, window: &mut Window, cx: &mut App) {
    let mut host = shared.borrow_mut();
    let Some(native) = host.native.as_mut() else {
        return;
    };
    if let Some(callbacks) = native.callbacks.get(&window.window_handle().window_id()) {
        callbacks.observe(window, cx);
    }
    let Some(mut bridge) = native.bridge.take() else {
        return;
    };
    let more = match collect_host(&mut host, &mut bridge, Some(window), cx) {
        Ok(more) => more,
        Err(error) => {
            host.fail(error);
            false
        }
    };
    host.native.as_mut().unwrap().bridge = Some(bridge);
    schedule_continuation(shared, &mut host, cx);
    drop(host);
    if more {
        schedule(shared, cx);
    }
}

pub(super) fn watch_submission(
    host: &HostState,
    ticket: &open_gpui::WindowFrameSubmissionTicket,
    window: &mut Window,
) -> Result<()> {
    if let Some(native) = &host.native {
        native
            .callbacks
            .get(&window.window_handle().window_id())
            .ok_or_else(|| anyhow!("a managed docking mount needs its native root binding"))?
            .watch_submission(ticket, window)?;
    }
    Ok(())
}

pub(super) fn output_binding(
    host: &HostState,
    owner: WindowId,
    output: &dockspace::runtime::PaintedSurfaceOutput,
) -> Option<NativeSurfaceBinding> {
    let window = host.native.as_ref()?.callbacks.get(&owner)?.binding();
    host.session
        .retained_native_surface_bindings_for_window(window)
        .find(|binding| output.matches_native_binding(*binding))
}

pub(super) fn schedule(shared: &SharedHost, cx: &mut App) {
    {
        let mut host = shared.borrow_mut();
        let Some(native) = &mut host.native else {
            return;
        };
        if std::mem::replace(&mut native.pump_scheduled, true) {
            return;
        }
    }
    let weak = Rc::downgrade(shared);
    cx.defer_shutdown_critical_before_window_registry_clear_or_run_now(move |cx| {
        if let Some(shared) = weak.upgrade() {
            if let Some(native) = &mut shared.borrow_mut().native {
                native.pump_scheduled = false;
            }
            pump(&shared, cx);
        }
    });
}

pub(super) fn queue_pointer(
    host: &mut HostState,
    surface: SurfaceId,
    serial: u64,
    event: dockspace::runtime::SurfacePointerEvent,
    position: open_gpui::Point<Pixels>,
    bounds: Bounds<Pixels>,
    hitboxes: &[open_gpui::HitboxId],
    captured: bool,
    modifiers: open_gpui::Modifiers,
    window: &Window,
) -> bool {
    use dockspace::runtime::{
        NativeLocalPointerLocation, NativePointerButton, NativePointerCancelReason,
        NativePointerEvent, NativePointerId, NativePointerInput, NativePointerOwner,
        PointerDeviceKind, PointerEventMetadata, SurfacePointerEvent,
    };
    let owner = window.window_handle().window_id();
    if !super::input::is_presented(host, owner, surface, serial) {
        return false;
    }
    let Some(native) = &host.native else {
        return false;
    };
    if native.transport.is_frozen() {
        return false;
    }
    let Some(callbacks) = native.callbacks.get(&owner) else {
        return false;
    };
    let binding = callbacks.binding();
    let event = match event {
        SurfacePointerEvent::ButtonPressed(_) => {
            NativePointerEvent::ButtonPressed(NativePointerButton::Primary)
        }
        SurfacePointerEvent::ButtonReleased(_) => {
            NativePointerEvent::ButtonReleased(NativePointerButton::Primary)
        }
        SurfacePointerEvent::Moved => NativePointerEvent::Moved,
        SurfacePointerEvent::StreamCancelled(_) => NativePointerEvent::StreamCancelled(
            NativePointerCancelReason::ExplicitPlatformCancellation,
        ),
        SurfacePointerEvent::DockingEnabledChanged { enabled } => {
            return host
                .session
                .report_native_docking_enabled(binding, enabled)
                .is_ok();
        }
        _ => return false,
    };
    let clear = hitboxes
        .iter()
        .any(|hitbox| hitbox.window_point_target_rank(position, window) == Some(0));
    if !captured && !clear {
        return false;
    }
    let location = local_sample(&host.session, binding, position, window).map_or(
        NativeLocalPointerLocation::Unknown,
        NativeLocalPointerLocation::Exact,
    );
    native.transport.pointer_in_window(
        NativePointerInput::new_local(
            NativePointerId::new(1),
            event,
            location,
            NativePointerOwner::Native(binding),
            NativePointerOwner::Unknown,
        )
        .with_metadata(
            PointerEventMetadata::new(PointerDeviceKind::Mouse, host.observed_at())
                .with_docking_enabled(host.config.input.docking_enabled(modifiers)),
        ),
    );
    if let Some(region) = host
        .mounts
        .get_mut(&owner)
        .and_then(|mount| mount.regions.get_mut(&surface))
    {
        region.pointer = super::input::logical_point(position, bounds);
        region.dirty = true;
    }
    true
}

fn local_sample(
    session: &DockspaceSession,
    binding: NativeWindowBinding,
    position: open_gpui::Point<Pixels>,
    window: &Window,
) -> Option<dockspace::runtime::NativeLocalPointerSample> {
    use dockspace::{
        geometry::PhysicalSize,
        runtime::{NativeLocalGeometry, NativeLocalPointerFact},
    };
    let facts = window.platform_facts();
    let scale = ScaleFactor::new(f64::from(facts.scale_factor)).ok()?;
    let size = if let Some(geometry) = facts.physical_geometry {
        PhysicalSize::new(
            f64::from(geometry.client_bounds().size.width.0),
            f64::from(geometry.client_bounds().size.height.0),
        )
        .ok()?
    } else {
        PhysicalSize::new(
            f64::from(f32::from(facts.content_size.width)) * scale.get(),
            f64::from(f32::from(facts.content_size.height)) * scale.get(),
        )
        .ok()?
    };
    session
        .native_local_coordinates(binding)?
        .capture(
            binding,
            NativeLocalPointerFact::new(
                PhysicalPoint::new(
                    f64::from(f32::from(position.x)) * scale.get(),
                    f64::from(f32::from(position.y)) * scale.get(),
                )
                .ok()?,
                size,
                scale,
            ),
            NativeLocalGeometry::new(size, scale),
        )
        .ok()
}

fn collect_host(
    host: &mut HostState,
    bridge: &mut NativeBridge,
    current: Option<&Window>,
    cx: &mut App,
) -> Result<bool> {
    // A prepared GPUI candidate still owns its rollbackable semantic update.
    // Preserve ingress ordering until that exact candidate is accepted or
    // withdrawn; collecting a native prefix here would require another update.
    if host.has_unaccepted_update() {
        return Ok(false);
    }
    recover_root_registrations(host, bridge, current, cx)?;
    for _ in 0..4 {
        let collected = bridge.collect(&mut host.session);
        if let Err(error) = &collected {
            if !host.session.has_pending_native_input() {
                return Err(anyhow!(error.to_string()));
            }
        }
        process_reports(host, bridge, cx)?;
        let pending_input = host.session.has_pending_native_input();
        let may_create = !host.native.as_ref().unwrap().transport.is_frozen()
            && host.session.has_pending_native_create();
        if !pending_input && !may_create {
            return Ok(false);
        }
        let HostState {
            session,
            mounts,
            native,
            ..
        } = host;
        let mut frame = begin_frame(session, mounts, native.as_ref(), current, cx)?;
        let advanced = may_create && frame.advance_next_native_surface_create()?;
        if !pending_input && !advanced {
            return Ok(false);
        }
        let mut report = frame.commit()?;
        let progressed = advanced || !report.inputs().is_empty();
        host.invalidate_report(&report);
        bridge.accept_report(&mut report);
        recover_root_registrations(host, bridge, current, cx)?;
        process_reports(host, bridge, cx)?;
        if !progressed {
            return Ok(false);
        }
    }
    Ok(host.session.has_pending_native_input() || !bridge.transport().is_empty())
}

fn recover_root_registrations(
    host: &mut HostState,
    bridge: &mut NativeBridge,
    current: Option<&Window>,
    cx: &mut App,
) -> Result<()> {
    let owners: Vec<_> = host
        .native
        .as_ref()
        .unwrap()
        .registrations
        .keys()
        .copied()
        .collect();
    for owner in owners {
        let surface = host.native.as_ref().unwrap().registrations[&owner].surface;
        let Some(binding) = host
            .session
            .native_window_bindings()
            .find(|binding| binding.window_token() == HostWindowToken::new(owner.as_u64()))
        else {
            // A committed semantic rejection leaves no binding to retire.
            if !host.session.has_pending_native_input() {
                host.native.as_mut().unwrap().registrations.remove(&owner);
            }
            continue;
        };
        if !host
            .session
            .native_surface_bindings_for_window(binding)
            .any(|binding| binding.surface() == surface)
        {
            continue;
        }
        let native = host.native.as_mut().unwrap();
        let registration = &native.registrations[&owner];
        let callbacks = if let Some(ticket) = native.retirements.get(&owner) {
            bridge.retain_root_rollback(binding, ticket.clone(), registration.anchor, cx)?
        } else if let Some(callbacks) = bridge.callbacks_for_window(owner) {
            callbacks
        } else {
            // The original construction Window cannot be checked out again.
            // Successful synchronous binding handles that case directly.
            if current.is_some_and(|window| window.window_handle().window_id() == owner) {
                continue;
            }
            registration.window.update(cx, |_, window, cx| {
                bridge.bind_window_current(binding, window, registration.anchor, cx)
            })??
        };
        native.callbacks.insert(owner, callbacks);
        native.roots.insert(owner);
        native.registrations.remove(&owner);
    }
    Ok(())
}

fn process_reports(host: &mut HostState, bridge: &mut NativeBridge, cx: &mut App) -> Result<()> {
    for report in bridge.take_reports() {
        match &report {
            NativeReport::CloseRequested(close) => {
                let policy = host.native.as_ref().unwrap().close_policy;
                if !host.native.as_ref().unwrap().transport.is_frozen() {
                    bridge.decide_close(&mut host.session, *close, policy)?;
                }
            }
            NativeReport::LogicalClosed(binding) => {
                if let Some(owner) =
                    host.native
                        .as_ref()
                        .unwrap()
                        .callbacks
                        .iter()
                        .find_map(|(owner, callbacks)| {
                            (callbacks.binding() == *binding).then_some(*owner)
                        })
                {
                    presentation::close(host, owner, cx);
                }
            }
            NativeReport::NativeTerminal(binding) => {
                let retired: Vec<_> = host
                    .session
                    .retained_native_surface_bindings_for_window(*binding)
                    .collect();
                host.native.as_mut().unwrap().retired.extend(retired);
            }
            NativeReport::RetirePlacement(binding) => {
                host.native.as_mut().unwrap().retired.insert(*binding);
            }
            NativeReport::OpenFailed { message, .. } => {
                log::warn!("native docking create failed: {message}")
            }
            _ => {}
        }
        host.native.as_mut().unwrap().reports.push_back(report);
    }
    Ok(())
}

pub(super) fn pump(shared: &SharedHost, cx: &mut App) {
    let Ok(mut host) = shared.try_borrow_mut() else {
        return;
    };
    let Some(native) = host.native.as_mut() else {
        return;
    };
    let Some(mut bridge) = native.bridge.take() else {
        return;
    };
    native.retirements.retain(|owner, ticket| {
        let snapshot = ticket.snapshot();
        native.registrations.contains_key(owner)
            || !(snapshot.quiesced() && snapshot.native_terminal())
    });
    let factory = native.factory.clone();
    let mut more = match collect_host(&mut host, &mut bridge, None, cx) {
        Ok(more) => more,
        Err(error) => {
            host.fail(error);
            false
        }
    };
    while let Some(mut report) = host.native.as_mut().unwrap().pending_reports.pop_front() {
        bridge.accept_report(&mut report);
    }
    drop(host);
    let dock = DockSurface {
        host: shared.clone(),
    };
    if let Err(error) = bridge.dispatch_effects(
        &mut |request, callbacks, cx| factory(&dock, request, callbacks, cx),
        cx,
    ) {
        shared.borrow_mut().fail(error);
    }
    if let Err(error) = bridge.drive_shutdown(cx) {
        shared.borrow_mut().fail(error);
    }
    let mut host = shared.borrow_mut();
    while let Some(mut report) = host.native.as_mut().unwrap().pending_reports.pop_front() {
        bridge.accept_report(&mut report);
        more = true;
    }
    if let Err(error) = process_reports(&mut host, &mut bridge, cx) {
        host.fail(error);
    }
    if !host.session.has_pending_native_input() {
        let retired: Vec<_> = host
            .native
            .as_ref()
            .unwrap()
            .retired
            .iter()
            .copied()
            .collect();
        for binding in retired {
            let retained = host.mounts.values().any(|mount| {
                mount
                    .input_frames
                    .iter()
                    .any(|frame| frame.native_binding == Some(binding))
                    || mount
                        .pending
                        .as_ref()
                        .is_some_and(|pending| pending.outputs_for_binding(binding))
            });
            if retained {
                continue;
            }
            if let Some(callbacks) = bridge.callbacks(binding.window()) {
                // A placement may retire while the physical root continues.
                // Its live successor keeps the callback installation unsealed.
                let _ = callbacks.seal();
            }
            if bridge.quiesce_binding(&mut host.session, binding).is_ok() {
                host.native.as_mut().unwrap().retired.remove(&binding);
                if bridge.callbacks(binding.window()).is_none() {
                    host.native
                        .as_mut()
                        .unwrap()
                        .callbacks
                        .retain(|_, callbacks| callbacks.binding() != binding.window());
                }
                more = true;
            }
        }
    }
    host.native.as_mut().unwrap().bridge = Some(bridge);
    schedule_continuation(shared, &mut host, cx);
    let windows: Vec<_> = host
        .mounts
        .values()
        // An emitted output waiting on its renderer is a continuation, not a
        // new paint request. Its actual terminal callback wakes retained dirt.
        .filter(|mount| {
            mount.pending.is_none() && mount.regions.values().any(|region| region.dirty)
        })
        .map(|mount| mount.window)
        .collect();
    drop(host);
    for window in windows {
        let _ = window.update(cx, |_, window, _| window.refresh());
    }
    if more {
        schedule(shared, cx);
    }
}

fn schedule_continuation(shared: &SharedHost, host: &mut HostState, cx: &App) {
    let native = host.native.as_mut().unwrap();
    let request = (!native.transport.is_frozen() && native.transport.is_empty())
        .then(|| host.session.native_pointer_continuation())
        .flatten();
    let Some(request) = request else {
        native.wake = None;
        return;
    };
    if native
        .wake
        .as_ref()
        .is_some_and(|wake| wake.request == request)
    {
        return;
    }
    let timer = host
        .clock
        .timer(request.deadline().saturating_sub(host.observed_at()));
    let weak = Rc::downgrade(shared);
    let task = cx.spawn(async move |cx| {
        timer.await;
        let Some(shared) = weak.upgrade() else {
            return;
        };
        let _ = cx.update(|cx| continuation_without_native_sample(&shared, request, cx));
    });
    host.native.as_mut().unwrap().wake = Some(NativeWake {
        request,
        _task: task,
    });
}

fn continuation_without_native_sample(
    shared: &SharedHost,
    request: dockspace::runtime::NativePointerContinuation,
    cx: &mut App,
) {
    use dockspace::runtime::{
        NativeDesktopPointerLocation, NativeDesktopPosition, NativeLocalPointerLocation,
        NativePointerHover, NativePointerInput, NativePointerOwner, PointerDeviceKind,
        PointerEventMetadata,
    };
    let mut host = shared.borrow_mut();
    if !host.native.as_ref().is_some_and(|native| {
        native
            .wake
            .as_ref()
            .is_some_and(|wake| wake.request == request)
            && native.transport.is_empty()
    }) || host.session.native_pointer_continuation() != Some(request)
        || host.observed_at() < request.deadline()
    {
        return;
    }
    // Cached mouse_position and a logical capture handle cannot execute a
    // stationary native action. Unknown facts revoke this finite action through
    // the original journal; later real motion can rearm it.
    let input = if request.local_coordinates().is_some() {
        NativePointerInput::new_local(
            request.pointer(),
            request.event(),
            NativeLocalPointerLocation::Unknown,
            NativePointerOwner::Unknown,
            NativePointerOwner::Unknown,
        )
    } else {
        NativePointerInput::new(
            request.pointer(),
            request.event(),
            NativeDesktopPointerLocation::new(
                NativeDesktopPosition::Unknown,
                NativePointerHover::Unknown,
                None,
            ),
            NativePointerOwner::Unknown,
            NativePointerOwner::Unknown,
        )
    }
    .with_metadata(PointerEventMetadata::new(
        PointerDeviceKind::Mouse,
        host.observed_at(),
    ));
    let native = host.native.as_mut().unwrap();
    native.wake = None;
    native.reports.push_back(NativeReport::Unsupported {
        operation: "fresh stationary native pointer, hover and capture sampling",
    });
    native.transport.pointer(input, cx);
}
