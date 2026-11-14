//! Application views and physical mounts, without a second layout model.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    rc::Rc,
    time::Duration,
};

use anyhow::{Result, ensure};
use dockspace::{
    geometry::{LogicalPoint, LogicalRect, LogicalSize},
    model::{DockPlacement, DockspaceView, ItemId, SurfaceId},
    runtime::{
        DockspaceHostFrame, DockspaceSession, HostFrameReport, HostInputOutcome,
        NativeEffectRequest, SurfacePresentationArtifact, SurfaceUnavailableReason,
    },
};
use open_gpui::{
    AnyView, AnyWindowHandle, App, Bounds, FocusHandle, GlobalElementId, IntoElement, Pixels,
    SharedString, Subscription, Window, WindowId,
};
use web_time::Instant;

use super::{GpuiDockspaceConfig, continuation, element, host_native, input, presentation};

pub(super) type SharedHost = Rc<RefCell<HostState>>;

/// A distinct View instance. Multiple Views may share an application document.
#[derive(Clone)]
pub struct DockPane {
    pub(super) id: ItemId,
    pub(super) title: SharedString,
    pub(super) view: AnyView,
    pub(super) minimum_size: LogicalSize,
}

impl DockPane {
    /// Retains the original entity while it moves between tabs, surfaces and windows.
    pub fn new(id: ItemId, title: impl Into<SharedString>, view: impl Into<AnyView>) -> Self {
        Self {
            id,
            title: title.into(),
            view: view.into(),
            minimum_size: LogicalSize::new(120.0, 90.0).expect("valid default pane size"),
        }
    }

    /// Sets the content minimum used by the session's layout projection.
    pub fn with_minimum_size(mut self, minimum_size: LogicalSize) -> Self {
        self.minimum_size = minimum_size;
        self
    }

    /// Returns the docking identity of this View instance.
    pub fn id(&self) -> ItemId {
        self.id
    }
}

/// A logical surface inside one physical GPUI element.
#[derive(Clone, Copy, Debug)]
pub struct DockRegion {
    pub(super) surface: SurfaceId,
    pub(super) bounds: Option<LogicalRect>,
}

impl DockRegion {
    /// Fills the bounds allocated by the parent.
    pub fn fill(surface: SurfaceId) -> Self {
        Self {
            surface,
            bounds: None,
        }
    }

    /// Uses element-local logical pixels. Regions must be disjoint and contained.
    pub fn new(surface: SurfaceId, bounds: LogicalRect) -> Self {
        Self {
            surface,
            bounds: Some(bounds),
        }
    }
}

/// One workspace and its retained application Views.
///
/// Use one element per physical window. [`Self::regions`] combines multiple
/// logical surfaces into that element and consequently one exact GPUI ticket.
#[derive(Clone)]
pub struct DockSurface {
    pub(super) host: SharedHost,
}

impl DockSurface {
    /// Takes sole semantic ownership of the supplied session.
    ///
    /// A single GPUI entity cannot be registered under two pane identities. To
    /// duplicate a document View, create another entity sharing that document.
    pub fn new(
        session: DockspaceSession,
        panes: impl IntoIterator<Item = DockPane>,
        config: GpuiDockspaceConfig,
        cx: &mut App,
    ) -> Result<Self> {
        Self::create(session, panes, config, false, cx)
    }

    /// Creates one managed native workspace using the joined native input producer.
    ///
    /// Register each application root with `bind_native_root` or `open_native_root`.
    /// A session previously enrolled with the local pointer producer is rejected.
    pub fn new_managed(
        session: DockspaceSession,
        panes: impl IntoIterator<Item = DockPane>,
        config: GpuiDockspaceConfig,
        cx: &mut App,
    ) -> Result<Self> {
        Self::create(session, panes, config, true, cx)
    }

    fn create(
        mut session: DockspaceSession,
        panes: impl IntoIterator<Item = DockPane>,
        config: GpuiDockspaceConfig,
        managed: bool,
        cx: &mut App,
    ) -> Result<Self> {
        ensure!(
            f32::from(config.text_size).is_finite() && config.text_size > open_gpui::px(0.0),
            "dockspace text size must be positive and finite"
        );
        let mut panels = BTreeMap::new();
        let mut entities = BTreeSet::new();
        for pane in panes {
            ensure!(
                entities.insert(pane.view.entity_id()),
                "each docking View needs a distinct GPUI entity"
            );
            ensure!(
                panels.insert(pane.id, pane).is_none(),
                "duplicate docking pane identity"
            );
        }
        for surface in session.view().surfaces() {
            let mut nodes = Vec::new();
            if let Some(root) = surface.main_root() {
                nodes.extend(root.content());
            }
            for contained in surface.contained() {
                nodes.extend(contained.root().and_then(|root| root.content()));
            }
            while let Some(node) = nodes.pop() {
                if let Some(tabs) = node.tabs() {
                    for item in tabs.items() {
                        ensure!(
                            panels.contains_key(item),
                            "open item {item:?} has no GPUI View"
                        );
                    }
                } else if let Some(split) = node.split() {
                    nodes.extend(split.children());
                }
            }
        }
        if !managed {
            session.enable_host_pointer()?;
        }
        let clock = cx.background_executor().clone();
        let clock_origin = clock.now();
        let surface = Self {
            host: Rc::new(RefCell::new(HostState {
                session,
                panels,
                config,
                mounts: BTreeMap::new(),
                owners: BTreeMap::new(),
                inputs: Vec::new(),
                native_effects: VecDeque::new(),
                presentation_pending: false,
                error: None,
                close_handler: None,
                close_requests: VecDeque::new(),
                close_pump_scheduled: false,
                close_dispatching: false,
                clock,
                clock_origin,
                native: None,
            })),
        };
        if managed {
            host_native::enroll(&surface.host, cx)?;
        }
        Ok(surface)
    }

    /// Renders one logical surface in its physical window.
    pub fn element(&self, surface: SurfaceId) -> impl IntoElement + use<> {
        self.regions([DockRegion::fill(surface)])
    }

    /// Renders disjoint logical regions sharing a physical window and frame ticket.
    pub fn regions<I: IntoIterator<Item = DockRegion>>(
        &self,
        regions: I,
    ) -> impl IntoElement + use<I> {
        let element =
            element::DockspaceElement::new(self.host.clone(), regions.into_iter().collect());
        host_native::wrap(&self.host, element.into_any_element())
    }

    /// Reads the session's published model without exposing a second mutable owner.
    pub fn with_view<R>(&self, inspect: impl FnOnce(DockspaceView<'_>) -> R) -> R {
        inspect(self.host.borrow().session.view())
    }

    /// Resolves an exact persisted View key, including a closed historical occurrence.
    ///
    /// This does not allocate or rebind an identity. Different Views of the same
    /// application document require distinct external keys and GPUI entities.
    pub fn item_id_for_external_key(&self, external_key: &str) -> Option<ItemId> {
        self.host
            .borrow()
            .session
            .item_id_for_external_key(external_key)
    }

    /// Atomically registers a previously unregistered View and opens its pane.
    ///
    /// A persistent workspace requires an existing session-owned pane identity;
    /// [`Self::item_id_for_external_key`] also resolves closed historical keys.
    /// Neither an existing pane registration nor another pane's entity can be
    /// replaced. Reopen an already registered pane through [`Self::update`].
    /// The registration publishes only after the sole semantic frame commits.
    /// A busy lease, rejected placement or failed commit leaves it unregistered.
    pub fn open_pane(&self, pane: DockPane, placement: DockPlacement, cx: &mut App) -> Result<()> {
        let mut host = self
            .host
            .try_borrow_mut()
            .map_err(|_| anyhow::anyhow!("dockspace host is already borrowed"))?;
        ensure!(
            !host.panels.contains_key(&pane.id),
            "docking pane identity is already registered"
        );
        ensure!(
            host.panels
                .values()
                .all(|registered| registered.view.entity_id() != pane.view.entity_id()),
            "each docking View needs a distinct GPUI entity"
        );
        ensure!(
            host.session.document_id().is_none()
                || host.session.external_key_for_item(pane.id).is_some(),
            "pane identity has no session-owned external View key"
        );
        let HostState {
            session,
            mounts,
            native,
            ..
        } = &mut *host;
        let mut frame = host_native::begin_frame(session, mounts, native.as_ref(), None, cx)?;
        ensure!(
            frame.view().item(pane.id).is_none(),
            "docking pane is already open"
        );
        frame.open_item_current(pane.id, placement)?;
        frame.complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)?;
        ensure!(
            frame.view().item(pane.id).is_some(),
            "docking core rejected opening pane {:?}",
            pane.id
        );
        let report = frame.commit()?;
        host.panels.insert(pane.id, pane);
        host.apply_report(report);
        drop(host);
        cx.refresh_windows();
        host_native::pump(&self.host, cx);
        Ok(())
    }

    /// Applies application intent atomically and requests fresh physical frames.
    ///
    /// The callback can use current-frame docking and close-decision APIs. It
    /// cannot manufacture a painted output or a submitted presentation.
    /// A still-prepared candidate owns the sole semantic update; callers may
    /// wait on [`Self::is_update_pending`] before issuing a new synchronous intent.
    pub fn update<R>(
        &self,
        update: impl FnOnce(&mut DockspaceHostFrame<'_>) -> Result<R>,
        cx: &mut App,
    ) -> Result<R> {
        let mut host = self.host.borrow_mut();
        let HostState {
            session,
            mounts,
            native,
            ..
        } = &mut *host;
        let mut frame = host_native::begin_frame(session, mounts, native.as_ref(), None, cx)?;
        let value = update(&mut frame)?;
        let report = frame.commit()?;
        host.apply_report(report);
        drop(host);
        cx.refresh_windows();
        host_native::pump(&self.host, cx);
        Ok(value)
    }

    /// Returns the last transport error, if any.
    pub fn error(&self) -> Option<String> {
        self.host.borrow().error.clone()
    }

    /// Reports whether an unaccepted GPUI candidate owns the semantic update.
    ///
    /// Accepted outputs waiting for renderer submission do not hold this lease.
    /// This is scheduling readiness, not proof of output submission or retirement.
    pub fn is_update_pending(&self) -> bool {
        self.host.borrow().has_unaccepted_update()
    }

    /// Returns whether the session owns a pointer gesture.
    pub fn has_active_gesture(&self) -> bool {
        self.host.borrow().session.has_active_gesture()
    }
}

pub(super) struct HostState {
    pub(super) session: DockspaceSession,
    pub(super) panels: BTreeMap<ItemId, DockPane>,
    pub(super) config: GpuiDockspaceConfig,
    pub(super) mounts: BTreeMap<WindowId, WindowMount>,
    pub(super) owners: BTreeMap<SurfaceId, WindowId>,
    pub(super) inputs: Vec<input::QueuedPointer>,
    pub(super) native_effects: VecDeque<NativeEffectRequest>,
    pub(super) presentation_pending: bool,
    pub(super) error: Option<String>,
    pub(super) close_handler: Option<Rc<super::close::CloseHandler>>,
    pub(super) close_requests: VecDeque<dockspace::runtime::DockspaceClosePlan>,
    pub(super) close_pump_scheduled: bool,
    pub(super) close_dispatching: bool,
    pub(super) clock: open_gpui::BackgroundExecutor,
    pub(super) clock_origin: Instant,
    pub(super) native: Option<host_native::ManagedHost>,
}

pub(super) struct WindowMount {
    pub(super) window: AnyWindowHandle,
    pub(super) _close_observers: Option<Subscription>,
    pub(super) element: Option<GlobalElementId>,
    pub(super) regions: BTreeMap<SurfaceId, RegionState>,
    pub(super) pending: Option<presentation::PendingSubmission>,
    pub(super) serial: u64,
    pub(super) publication: open_gpui::PrepaintPublicationId,
    pub(super) capture: open_gpui::PointerCaptureHandle,
    pub(super) focus: FocusHandle,
    pub(super) input_frames: Vec<continuation::InputFrame>,
    pub(super) wake: Option<continuation::Wake>,
    pub(super) attached: bool,
    pub(super) waiting_for_update: bool,
}

pub(super) struct RegionState {
    pub(super) bounds: Option<Bounds<Pixels>>,
    pub(super) pointer: Option<LogicalPoint>,
    pub(super) packet: Option<Rc<element::Packet>>,
    pub(super) presented: Option<(u64, SurfacePresentationArtifact)>,
    pub(super) dirty: bool,
}

impl HostState {
    pub(super) fn has_unaccepted_update(&self) -> bool {
        self.mounts.values().any(|mount| {
            mount
                .pending
                .as_ref()
                .is_some_and(|pending| pending.update.is_some())
        })
    }

    pub(super) fn ensure_mount(
        &mut self,
        id: Option<&GlobalElementId>,
        regions: &[DockRegion],
        window: &mut Window,
        cx: &App,
    ) -> Result<bool> {
        let handle = window.window_handle();
        let owner = handle.window_id();
        ensure!(
            !regions.is_empty(),
            "a docking mount needs at least one logical surface"
        );
        let unique: BTreeSet<_> = regions.iter().map(|region| region.surface).collect();
        ensure!(
            unique.len() == regions.len(),
            "duplicate logical surface mount"
        );
        if let Some(mount) = self.mounts.get_mut(&owner) {
            ensure!(
                mount.element.as_ref() == id,
                "combine this window's surfaces in one docking element"
            );
            ensure!(
                mount.regions.keys().copied().collect::<BTreeSet<_>>() == unique,
                "a live docking mount cannot silently replace its logical surfaces"
            );
            // Moving the last ordinary root prunes its logical surface. The
            // physical region remains a vacant slot until its mount retires.
            mount.attached = true;
            return Ok(false);
        }
        for region in regions {
            let native_staging = self
                .native
                .as_ref()
                .and_then(|native| native.callbacks.get(&owner))
                .is_some_and(|callbacks| {
                    self.session
                        .native_surface_bindings_for_window(callbacks.binding())
                        .any(|binding| binding.surface() == region.surface)
                });
            ensure!(
                self.session.view().surface(region.surface).is_some() || native_staging,
                "unknown logical surface"
            );
            ensure!(
                self.owners
                    .get(&region.surface)
                    .is_none_or(|previous| *previous == owner),
                "a logical surface is already mounted in another window"
            );
        }
        for region in regions {
            self.owners.insert(region.surface, owner);
        }
        let regions = regions
            .iter()
            .map(|region| {
                (
                    region.surface,
                    RegionState {
                        bounds: None,
                        pointer: None,
                        packet: None,
                        presented: None,
                        dirty: true,
                    },
                )
            })
            .collect();
        self.mounts.insert(
            owner,
            WindowMount {
                window: handle,
                _close_observers: None,
                element: id.cloned(),
                regions,
                pending: None,
                serial: 0,
                publication: open_gpui::PrepaintPublicationId::new(),
                capture: window.new_pointer_capture_handle(),
                focus: cx.focus_handle(),
                input_frames: Vec::new(),
                wake: None,
                attached: true,
                waiting_for_update: false,
            },
        );
        Ok(true)
    }

    pub(super) fn apply_report(&mut self, mut report: HostFrameReport) {
        self.invalidate_report(&report);
        if let Some(native) = &mut self.native {
            host_native::accept_owned_report(native, report);
        } else {
            self.native_effects.extend(report.take_native_effects());
        }
    }

    pub(super) fn invalidate_report(&mut self, report: &HostFrameReport) {
        super::close::observe(self, report);
        // Affected surfaces also include provenance-only presentation updates.
        // Only the repaint roster requests new encoding for those observations.
        for surface in report.repaint_surfaces() {
            if let Some(owner) = self.owners.get(surface) {
                if let Some(region) = self
                    .mounts
                    .get_mut(owner)
                    .and_then(|mount| mount.regions.get_mut(surface))
                {
                    region.dirty = true;
                }
            }
        }
        if report.workspace_changed() {
            for mount in self.mounts.values_mut() {
                for region in mount.regions.values_mut() {
                    region.dirty = true;
                }
            }
        }
        if self.native.is_some()
            && report.inputs().iter().any(|input| {
                matches!(
                    input,
                    HostInputOutcome::NativePlatformSnapshotApplied { .. }
                )
            })
        {
            // A Bootstrap scene can keep the same stamp when native readiness
            // changes. Its staging packet never executed a program, so retry it
            // on this committed observation even without a scene repaint delta.
            for mount in self.mounts.values_mut() {
                for region in mount.regions.values_mut() {
                    if region
                        .packet
                        .as_ref()
                        .is_some_and(|packet| packet.is_deferred())
                    {
                        region.dirty = true;
                    }
                }
            }
        }
    }

    pub(super) fn observed_at(&self) -> Duration {
        self.clock
            .now()
            .saturating_duration_since(self.clock_origin)
    }

    pub(super) fn fail(&mut self, error: impl std::fmt::Display) {
        let message = format!("{error:#}");
        log::error!("GPUI docking: {message}");
        self.error = Some(message);
    }
}

#[cfg(test)]
#[path = "host_facade_tests.rs"]
mod tests;
