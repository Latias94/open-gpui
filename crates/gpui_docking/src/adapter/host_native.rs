//! Native execution for the same retained host and the same docking session.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    rc::Rc,
};

use anyhow::{Result, anyhow};
use dockspace::{
    geometry::{LogicalSize, PhysicalPoint, PhysicalRect, ScaleFactor},
    model::SurfaceId,
    runtime::{
        DockspaceHostFrame, DockspaceSession, HostFrameReport, HostWindowToken,
        NativeHostCapability, NativeSurfaceBinding, NativeSurfaceCloseRequest,
        NativeSurfacePlacement, NativeSurfaceRole, NativeWindowBinding,
    },
};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Context, HitboxTag, IntoElement, Pixels,
    PlatformFrameDeliveryMode, Render, Subscription, Window, WindowId, WindowOptions, div,
    prelude::*,
};

#[cfg(test)]
use super::presentation;
use super::{
    DockSurface, HostState, SharedHost,
    host::WindowMount,
    native::{
        NativeBridge, NativeBridgeError, NativeCallbacks, NativeCloseDecision, NativeGeometry,
        NativeOpenFailure, NativeReport, NativeTransport, NativeWindowRequest,
    },
};

#[cfg(test)]
#[path = "host_native_tests.rs"]
pub(super) mod tests;

#[cfg(test)]
#[path = "host_native_factory_tests.rs"]
mod factory_tests;

#[cfg(test)]
#[path = "receiver_tests.rs"]
mod receiver_tests;

mod capture;
mod driver;
pub(super) mod provisional;
mod receiver;
mod reports;
mod termination;

#[cfg(test)]
use capture::local_sample;
pub(super) use capture::{queue_mouse_terminal, queue_pointer, routes_captured_drag, wrap};
#[cfg(test)]
use driver::{DeferredHostWork, collect_host, process_reports};
pub(super) use driver::{prepare_window, pump, schedule};
use receiver::resolve;
#[cfg(test)]
use receiver::resolve_in_window;

type WindowFactory = dyn Fn(
    &DockSurface,
    NativeWindowRequest,
    &NativeCallbacks,
    &mut App,
) -> std::result::Result<AnyWindowHandle, NativeOpenFailure>;

type QuiescenceCallback = Box<dyn FnOnce(&mut App)>;

pub(super) struct ManagedHost {
    bridge: Option<NativeBridge>,
    pub(super) wrapper_hitbox_tag: HitboxTag,
    pub(super) transport: NativeTransport,
    pub(super) callbacks: BTreeMap<WindowId, NativeCallbacks>,
    roots: BTreeSet<WindowId>,
    pending_reports: VecDeque<HostFrameReport>,
    reports: reports::Reports,
    retired: BTreeSet<NativeSurfaceBinding>,
    close_policy: Option<NativeCloseDecision>,
    factory: Option<Rc<WindowFactory>>,
    factory_opening: Option<NativeCallbacks>,
    provisional: provisional::Provisional,
    _quit: Option<Subscription>,
    pump_scheduled: bool,
    wake: Option<NativeWake>,
    blocked_continuation: Option<dockspace::runtime::NativePointerContinuation>,
    quiescence_callbacks: Vec<QuiescenceCallback>,
    termination_callbacks: Vec<termination::TerminationCallback>,
    failed_shutdown: Option<termination::FailedShutdown>,
    registrations: BTreeMap<WindowId, RootRegistration>,
    retirements: BTreeMap<WindowId, open_gpui::WindowPresentationShutdownTicket>,
    _terminal: Option<Subscription>,
    _displays: Option<Subscription>,
}

impl ManagedHost {
    pub(super) fn queue_docking_controls(
        &mut self,
        binding: NativeWindowBinding,
        enabled: bool,
        mode: dockspace::model::TabGroupDockMode,
        cx: &mut App,
    ) {
        self.transport.docking_controls(binding, enabled, mode, cx);
        self.blocked_continuation = None;
    }

    pub(super) fn has_preview_window(&self) -> bool {
        self.provisional.has_preview_window()
    }

    pub(super) fn is_failed(&self) -> bool {
        self.failed_shutdown.is_some()
    }

    pub(super) fn can_dispatch_close(&self) -> bool {
        self.bridge.is_some() && !self.transport.is_frozen()
    }
}

struct RootRegistration {
    surface: SurfaceId,
    window: AnyWindowHandle,
    anchor: bool,
    retirement: termination::UnboundWindowRetirement,
}

struct NativeWake {
    request: dockspace::runtime::NativePointerContinuation,
    capture: Option<super::native::NativeCapture>,
    _task: open_gpui::Task<()>,
}

pub(super) fn accept_owned_report(native: &mut ManagedHost, report: HostFrameReport) {
    if let Some(bridge) = &mut native.bridge {
        bridge.accept_report(report);
    } else {
        native.pending_reports.push_back(report);
    }
}

pub(super) fn inspection(host: &HostState) -> Option<super::DockNativeInspection> {
    let native = host.native.as_ref()?;
    let (provisional_opening, provisional_promotion_count, provisional_retirement_count) =
        native.provisional.inspection();
    let tickets: Vec<_> = native
        .retirements
        .values()
        .map(|ticket| ticket.snapshot())
        .collect();
    Some(super::DockNativeInspection {
        admission_frozen: native.transport.is_frozen(),
        bridge_available: native.bridge.is_some(),
        quiescent: is_quiescent(host, native),
        failed_generation: native.is_failed(),
        termination: termination::disposition(host, native),
        transport_empty: native.transport.is_empty(),
        transport: native.transport.inspection(),
        retained_report_count: native.reports.len(),
        discarded_report_count: native.reports.discarded(),
        pending_host_report_count: native.pending_reports.len(),
        callback_window_count: native.callbacks.len(),
        root_window_count: native.roots.len(),
        retiring_binding_count: native.retired.len(),
        pending_root_registration_count: native.registrations.len(),
        root_retirement_count: tickets.len(),
        pending_root_terminal_count: tickets
            .iter()
            .filter(|ticket| !ticket.native_terminal())
            .count(),
        pending_root_quiescence_count: tickets.iter().filter(|ticket| !ticket.quiesced()).count(),
        pump_scheduled: native.pump_scheduled,
        continuation_pending: native.wake.is_some(),
        continuation_blocked: native.blocked_continuation.is_some(),
        provisional_opening,
        provisional_promotion_count,
        provisional_retirement_count,
    })
}

fn is_quiescent(host: &HostState, native: &ManagedHost) -> bool {
    native.failed_shutdown.is_none()
        && native
            .bridge
            .as_ref()
            .is_some_and(NativeBridge::is_quiescent)
        && native.pending_reports.is_empty()
        && native.retired.is_empty()
        && native.registrations.is_empty()
        && native.retirements.is_empty()
        && native.callbacks.is_empty()
        && native.wake.is_none()
        && native.provisional.is_quiescent()
        && host.mounts.is_empty()
        && host.unavailable_surfaces.is_empty()
        && !host.session.has_pending_native_input()
}

#[cfg(test)]
mod inspection_tests {
    use dockspace::{model::DockspaceLayout, policy::DockPolicy, runtime::NativeReceiverAnswer};
    use open_gpui::TestAppContext;

    use super::*;
    use crate::adapter::GpuiDockspaceConfig;

    #[open_gpui::test]
    fn inspection_keeps_native_reports_and_pending_commits_with_their_original_owners(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            let dock = DockSurface::new_managed(
                DockspaceSession::from_layout(DockspaceLayout::empty(), DockPolicy::default())
                    .unwrap(),
                [],
                GpuiDockspaceConfig::default(),
                cx,
            )
            .unwrap();
            let bridge = {
                let mut host = dock.host.borrow_mut();
                let report = host
                    .session
                    .begin_native_host_frame(|_| NativeReceiverAnswer::Unknown)
                    .unwrap()
                    .commit()
                    .unwrap();
                host.fail(&std::io::Error::other("retained original error"));
                let native = host.native.as_mut().unwrap();
                let bridge = native.bridge.take().unwrap();
                accept_owned_report(native, report);
                native.transport.freeze_admission();
                native
                    .reports
                    .push_back(NativeReport::GroupShutdownRequested);
                native.reports.push_back(NativeReport::Unsupported {
                    operation: "test-operation",
                });
                bridge
            };
            {
                let host = dock.host.borrow_mut();
                let recorded = host.session.native_input_recorded_count();
                let version = host.session.version();
                assert_eq!(
                    dock.inspection(),
                    Err(super::super::DockInspectionError::Busy)
                );
                assert_eq!(host.session.native_input_recorded_count(), recorded);
                assert_eq!(host.session.version(), version);
                assert_eq!(host.native.as_ref().unwrap().reports.len(), 2);
                assert_eq!(host.native.as_ref().unwrap().pending_reports.len(), 1);
            }
            let before = dock.inspection().unwrap();
            assert_eq!(before, dock.inspection().unwrap());
            assert_eq!(
                before.last_error.as_deref(),
                Some("retained original error")
            );
            let native = before.native.unwrap();
            assert!(native.admission_frozen);
            assert!(!native.bridge_available);
            assert!(!native.quiescent);
            assert_eq!(native.retained_report_count, 2);
            assert_eq!(native.pending_host_report_count, 1);
            assert!(!dock.native_is_quiescent());
            assert_eq!(
                dock.host
                    .borrow()
                    .native
                    .as_ref()
                    .unwrap()
                    .pending_reports
                    .len(),
                1
            );
            let reports = dock.take_native_reports();
            assert!(matches!(
                reports.as_slice(),
                [
                    NativeReport::GroupShutdownRequested,
                    NativeReport::Unsupported {
                        operation: "test-operation"
                    }
                ]
            ));
            dock.host.borrow_mut().native.as_mut().unwrap().bridge = Some(bridge);
        });
    }
}

pub(super) fn enroll(shared: &SharedHost, cx: &mut App) -> Result<()> {
    capture::install_router(cx);
    let weak = Rc::downgrade(shared);
    let transport = NativeTransport::for_managed_host(move |cx| {
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
    let desktop = transport.clone();
    let displays =
        cx.observe_display_publication(move |snapshot, cx| desktop.desktop(snapshot, cx));
    let desktop = transport.clone();
    shared.borrow_mut().native = Some(ManagedHost {
        bridge: Some(bridge),
        wrapper_hitbox_tag: HitboxTag::new(),
        transport,
        callbacks: BTreeMap::new(),
        roots: BTreeSet::new(),
        pending_reports: VecDeque::new(),
        reports: reports::Reports::default(),
        retired: BTreeSet::new(),
        close_policy: None,
        factory: None,
        factory_opening: None,
        provisional: provisional::Provisional::default(),
        _quit: Some(quit),
        pump_scheduled: false,
        wake: None,
        blocked_continuation: None,
        quiescence_callbacks: Vec::new(),
        termination_callbacks: Vec::new(),
        failed_shutdown: None,
        registrations: BTreeMap::new(),
        retirements: BTreeMap::new(),
        _terminal: Some(terminal),
        _displays: Some(displays),
    });
    desktop.desktop(&cx.display_snapshot(), cx);
    Ok(())
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

    /// Reads the current live-route owner's original outer intent and placement ticket.
    ///
    /// This borrows the existing owner; it retains no separate evidence history.
    /// After promotion, inspect the adopted callbacks' original final receipt instead.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn native_provisional_placement_for_test(
        &self,
        window: WindowId,
    ) -> Option<(
        dockspace::geometry::PhysicalRect,
        open_gpui::WindowProvisionalPlacementSnapshot,
    )> {
        self.host
            .borrow()
            .native
            .as_ref()?
            .provisional
            .placement_for_test(window)
    }

    /// Registers an application root during its original GPUI construction/update.
    ///
    /// Register once per physical window. Additional logical regions in that
    /// window are declared atomically by the element's complete placement roster.
    /// On deferred renderers, a docking element in the construction frame
    /// paints its background and joins the workspace on a subsequent frame.
    pub fn bind_native_root(
        &self,
        surface: SurfaceId,
        anchor: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> std::result::Result<(), NativeBridgeError> {
        let owner = window.window_handle().window_id();
        let mut host = self.host.borrow_mut();
        reject_primary_failure(&host)?;
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
        let native = host.native.as_mut().unwrap();
        native.registrations.insert(
            owner,
            RootRegistration {
                surface,
                window: window.window_handle(),
                anchor,
                retirement: termination::UnboundWindowRetirement::observe(
                    owner,
                    &native.transport,
                    cx,
                ),
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
            if result.is_ok() && should_stage_root_construction(window, cx) {
                window.defer(cx, |window, _| window.refresh());
            }
        }
        result.map(|_| ())
    }

    /// Opens a normal native root containing the original retained workspace Views.
    /// On deferred renderers the construction draw paints the background and
    /// leaves the semantic update available for other roots opened in the same
    /// App update. Dockspace content mounts in a subsequent frame.
    pub fn open_native_root(
        &self,
        surface: SurfaceId,
        anchor: bool,
        options: WindowOptions,
        cx: &mut App,
    ) -> std::result::Result<AnyWindowHandle, NativeOpenFailure> {
        if let Err(error) = reject_primary_failure(&self.host.borrow()) {
            return Err(NativeOpenFailure {
                message: error.to_string(),
                root_binding: Some(error),
                stage: None,
                retirement_ticket: None,
                source: None,
            });
        }
        let dock = self.clone();
        let failure = Rc::new(std::cell::RefCell::new(None));
        let root_failure = failure.clone();
        let window = cx.open_window_detailed(options, move |window, cx| {
            if let Err(error) = dock.bind_native_root(surface, anchor, window, cx) {
                *root_failure.borrow_mut() = Some(error);
                window.remove_window(cx);
            }
            cx.new(|_| NativeRoot {
                dock,
                surface,
                provisional: None,
            })
        });
        let root_binding = failure.borrow_mut().take();
        match window {
            Ok(window) => Ok(window.into()),
            Err(source) => {
                self.retain_native_root_open_failure(&source, cx);
                let mut error = NativeOpenFailure::from(source);
                if let Some(binding_error) = &root_binding {
                    error.message = format!("{}: {binding_error}", error.message);
                }
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
                host.fail(&error);
            }
        }
        drop(host);
        schedule(&self.host, cx);
    }

    /// Overrides ordinary native close policy. Anchor shutdown always drains the group.
    ///
    /// By default an application root retains its logical layout; a native child
    /// recovers through its original presentation obligation. Content decisions
    /// are supplied separately through [`Self::set_close_handler`].
    pub fn set_native_close_policy(&self, policy: NativeCloseDecision) -> Result<()> {
        let mut host = self.host.borrow_mut();
        host.native
            .as_mut()
            .ok_or_else(|| anyhow!("managed native hosting is not enabled"))?
            .close_policy = Some(policy);
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
        if !matches!(decision, NativeCloseDecision::Defer) {
            native.as_mut().unwrap().reports.resolve_close(close);
        }
        drop(host);
        pump(&self.host, cx);
        Ok(())
    }

    /// Installs an application root factory while preserving the original native callbacks.
    ///
    /// Custom factories own window creation options and opt out of the default factory's
    /// pre-release native window. The core's source-hosted placement cue remains
    /// available without creating a competing HWND. Any outstanding default
    /// preview window retires through GPUI.
    /// Open through [`NativeCallbacks::open`] or install the supplied callbacks during
    /// root construction. The managed host routes that exact lifetime before staged
    /// surface painting; the factory must not register a second application root.
    /// Mount the requested surface with [`Self::element`] inside the custom window.
    /// Its displayed bounds and clip define the docking region, so application
    /// chrome can occupy the rest of the client area.
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
            .factory = Some(Rc::new(factory));
        Ok(())
    }

    /// Drains native observations and original deferred close decisions.
    ///
    /// Observational history retains the latest 256 entries. Unresolved deferred
    /// close decisions are retained separately from that budget until consumed,
    /// resolved or made obsolete by the original native terminal. Inspection's
    /// `discarded_report_count` records historical observations discarded so far.
    /// The ordered ingress and internal lifecycle receipts are never truncated.
    pub fn take_native_reports(&self) -> Vec<NativeReport> {
        self.host
            .borrow_mut()
            .native
            .as_mut()
            .map_or_else(Vec::new, |native| native.reports.take())
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
        host.native
            .as_ref()
            .is_some_and(|native| is_quiescent(&host, native))
    }

    /// Runs once, outside host borrows, after all original physical owners retire.
    ///
    /// Register after opening roots or starting failure cleanup. An already
    /// quiescent host defers the callback through App. This observes the same
    /// full predicate as [`Self::native_is_quiescent`]; it does not clear errors,
    /// retire owners or authorize reuse of an undrained session. Capture weak
    /// application owners rather than a strong clone of this surface.
    pub fn when_native_quiescent(
        &self,
        callback: impl FnOnce(&mut App) + 'static,
        cx: &mut App,
    ) -> Result<()> {
        let mut host = self
            .host
            .try_borrow_mut()
            .map_err(|_| anyhow!("docking host is busy"))?;
        let native = host
            .native
            .as_ref()
            .ok_or_else(|| anyhow!("managed native hosting is not enabled"))?;
        if is_quiescent(&host, native) {
            drop(host);
            cx.defer(callback);
        } else {
            host.native
                .as_mut()
                .unwrap()
                .quiescence_callbacks
                .push(Box::new(callback));
            drop(host);
            schedule(&self.host, cx);
        }
        Ok(())
    }
}

fn reject_primary_failure(host: &HostState) -> std::result::Result<(), NativeBridgeError> {
    if let Some(failure) = host
        .failure
        .as_ref()
        .filter(|failure| failure.is_primary_protocol_failure())
    {
        return Err(super::DockHostAdmissionError {
            failure: failure.clone(),
        }
        .into());
    }
    Ok(())
}

struct NativeRoot {
    dock: DockSurface,
    surface: SurfaceId,
    provisional: Option<Rc<RefCell<provisional::ProvisionalWindow>>>,
}

impl Render for NativeRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(preview) = self
            .provisional
            .as_ref()
            .and_then(|state| provisional::prepare(&self.dock, state, window, cx))
        {
            return preview;
        }
        div()
            .size_full()
            .child(self.dock.element(self.surface))
            .into_any_element()
    }
}

fn default_factory(
    dock: &DockSurface,
    request: NativeWindowRequest,
    callbacks: &NativeCallbacks,
    cx: &mut App,
) -> std::result::Result<AnyWindowHandle, NativeOpenFailure> {
    if let Some(window) = provisional::adopt(dock, request, callbacks, cx) {
        return window;
    }
    let dock = dock.clone();
    callbacks.open(
        WindowOptions::default(),
        move |_, cx| {
            cx.new(|_| NativeRoot {
                dock,
                surface: request.binding.surface(),
                provisional: None,
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
    anyhow::ensure!(
        native.is_none_or(|native| native.failed_shutdown.is_none()),
        "a failed generation cannot begin another semantic frame"
    );
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
    regions: &BTreeMap<SurfaceId, super::input::RegionGeometry>,
) -> Result<()> {
    let Some(native) = native else {
        return Ok(());
    };
    let is_root = native.roots.contains(&owner);
    let Some(callbacks) = native.callbacks.get(&owner) else {
        anyhow::ensure!(!is_root, "native root has no callback route");
        return Ok(());
    };
    let binding = callbacks.binding();
    if !is_root {
        if let Some(request) = callbacks
            .create_request()
            .filter(|request| request.role == NativeSurfaceRole::Child)
            && let Some(current) = frame.native_surface_placement(request.binding)
            && let Some(geometry) = regions.get(&current.surface())
        {
            let placement = surface_placement(current.surface(), geometry)?;
            if current != placement {
                frame.set_native_surface_placements(binding, &[placement])?;
            }
        }
        return Ok(());
    }
    let placements: Result<Vec<_>> = regions
        .iter()
        .filter(|(surface, _)| frame.view().surface(**surface).is_some())
        .map(|(&surface, geometry)| surface_placement(surface, geometry))
        .collect();
    let placements = placements?;
    // Core already detaches a window's final placement when its ordinary root
    // disappears. An empty roster cannot redeclare that retired binding.
    if !placements.is_empty() {
        frame.set_native_surface_placements(binding, &placements)?;
    }
    Ok(())
}

fn surface_placement(
    surface: SurfaceId,
    geometry: &super::input::RegionGeometry,
) -> Result<NativeSurfacePlacement> {
    let layout = geometry.element.layout_bounds();
    let displayed = geometry.element.displayed_bounds();
    let scale = geometry.device_scale;
    let physical = |pixels: Pixels| f64::from(f32::from(pixels)) * scale;
    Ok(NativeSurfacePlacement::new(
        surface,
        LogicalSize::new(
            f64::from(f32::from(layout.size.width)),
            f64::from(f32::from(layout.size.height)),
        )?,
        PhysicalPoint::new(physical(displayed.origin.x), physical(displayed.origin.y))?,
        ScaleFactor::new(scale * f64::from(geometry.element.window_scale().width))?,
        PhysicalRect::new(
            physical(geometry.clip.origin.x),
            physical(geometry.clip.origin.y),
            physical(geometry.clip.size.width),
            physical(geometry.clip.size.height),
        )?,
    ))
}

fn should_stage_root_construction(window: &Window, cx: &App) -> bool {
    window.frame_delivery_mode() == PlatformFrameDeliveryMode::Deferred
        // GPUI publishes the window profile only after its reservation commits.
        && cx.window_profile(window.window_handle()).is_none()
}

pub(super) fn stage_root_construction(host: &HostState, window: &Window, cx: &App) -> bool {
    should_stage_root_construction(window, cx)
        && host
            .native
            .as_ref()
            .is_some_and(|native| native.roots.contains(&window.window_handle().window_id()))
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
