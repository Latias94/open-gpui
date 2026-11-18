//! Application views and physical mounts, without a second layout model.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    rc::Rc,
    time::Duration,
};

use anyhow::{Result, ensure};
use dockspace::{
    geometry::{LogicalPoint, LogicalRect, LogicalSize, PaneSizing},
    model::{DockPlacement, DockspaceView, ItemId, SurfaceId},
    runtime::{
        DockspaceHostFrame, DockspaceSession, HostFrameReport, HostInputOutcome,
        NativeEffectRequest, SurfacePresentationArtifact, SurfaceUnavailableReason,
    },
};
use open_gpui::{
    AnyView, AnyWindowHandle, App, Bounds, FocusHandle, GlobalElementId, HitboxTag, IntoElement,
    Pixels, SharedString, Subscription, Window, WindowId,
};
use web_time::Instant;

use super::{
    DockHostFailure, GpuiDockspaceConfig, continuation, element, host_native, input, presentation,
};

pub(super) type SharedHost = Rc<RefCell<HostState>>;

/// Application intent was rejected after an original protocol or internal failure.
///
/// This diagnostic does not reset the session or revoke the original owners'
/// native, input and renderer cleanup paths.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("docking host rejected application intent after a primary failure: {}", .failure.message)]
pub struct DockHostAdmissionError {
    /// The retained primary failure, copied without consuming its original owner.
    pub failure: DockHostFailure,
}

/// Admission failures for pane metadata and registration lifecycle operations.
///
/// Pane methods return `anyhow::Result`; these failures can be downcast separately
/// from core runtime errors and native shutdown's `NativeBridgeError::Draining`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DockPaneError {
    /// An original update, input, presentation or retained packet still owns the content.
    #[error("docking pane lifecycle is busy")]
    Busy,
    /// No application View is registered for this identity, including closed history.
    #[error("docking pane {0:?} is not registered")]
    UnknownPane(ItemId),
    /// An open pane must complete its semantic close before releasing its View.
    #[error("docking pane {0:?} is still open")]
    PaneOpen(ItemId),
}

/// A distinct View instance. Multiple Views may share an application document.
#[derive(Clone)]
pub struct DockPane {
    pub(super) id: ItemId,
    pub(super) title: SharedString,
    pub(super) view: AnyView,
    pub(super) sizing: PaneSizing,
    pub(super) content_focus: Option<FocusHandle>,
    pub(super) content_receivers: Vec<HitboxTag>,
}

impl DockPane {
    /// Retains the original entity while it moves between tabs, surfaces and windows.
    pub fn new(id: ItemId, title: impl Into<SharedString>, view: impl Into<AnyView>) -> Self {
        Self {
            id,
            title: title.into(),
            view: view.into(),
            sizing: LogicalSize::new(120.0, 90.0)
                .expect("valid default pane size")
                .into(),
            content_focus: None,
            content_receivers: Vec::new(),
        }
    }

    /// Sets complete content sizing in logical points, before docking chrome.
    ///
    /// Maximum and fixed extents constrain the content frame. Preferred extents
    /// apply only to an explicit preferred-size splitter adjustment; ordinary
    /// layout retains user weights. Minimum-only declarations can pass a
    /// [`LogicalSize`] directly. The validated value is retained across moves.
    pub fn with_sizing(mut self, sizing: impl Into<PaneSizing>) -> Self {
        self.sizing = sizing.into();
        self
    }

    /// Replaces sizing with content minimums and unbounded maximums.
    pub fn with_minimum_size(self, minimum_size: LogicalSize) -> Self {
        self.with_sizing(minimum_size)
    }

    /// Returns the application-owned content sizing declaration.
    pub fn sizing(&self) -> PaneSizing {
        self.sizing
    }

    /// Binds the content's existing focus target without replacing its View.
    pub fn with_focus_handle(mut self, focus: FocusHandle) -> Self {
        self.content_focus = Some(focus);
        self
    }

    /// Allows docking hover over ordinary content targets carrying this explicit label.
    ///
    /// Label each editor, canvas or scroll target with `InteractiveElement::hitbox_tag`, or use
    /// `Window::insert_hitbox_with_tag` in a custom element. Labels apply to individual targets;
    /// descendants and overlapping unknown controls remain blockers. Repeat for additional tags.
    /// Only registrations actually prepainted inside this pane's clipped content are accepted.
    /// Popup, modal and foreign-overlay targets must not be labeled as ordinary content.
    ///
    /// This declaration preserves the original View and its input handling. Docking can observe
    /// pane activation and hover without capturing content clicks, drags or focus.
    pub fn with_content_receiver(mut self, tag: HitboxTag) -> Self {
        if !self.content_receivers.contains(&tag) {
            self.content_receivers.push(tag);
        }
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
    /// Every owned pane, including hidden and inactive descendants, needs a View.
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
            let roots = surface
                .main_root()
                .into_iter()
                .chain(surface.contained().filter_map(|contained| contained.root()));
            for item in roots.flat_map(|root| root.items()) {
                ensure!(
                    panels.contains_key(&item),
                    "open item {item:?} has no GPUI View"
                );
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
                unavailable_surfaces: BTreeSet::new(),
                failure: None,
                close_handler: None,
                close_requests: VecDeque::new(),
                close_pump_scheduled: false,
                close_dispatching: false,
                commit_handler: None,
                commit_notifications: VecDeque::new(),
                commit_pump_scheduled: false,
                commit_dispatching: false,
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
    ///
    /// `with_view(|view| view.to_layout())` copies the committed logical layout
    /// for reconstruction in a fresh session. The copy excludes live gestures,
    /// queued input, native bindings, GPUI entities and presentation authority.
    /// Use [`Self::capture_document`] when external View keys and document
    /// generations must also survive; a logical checkpoint does not replace it.
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
    /// Native shutdown and a primary protocol failure reject registration before
    /// the session or application registry changes.
    pub fn open_pane(&self, pane: DockPane, placement: DockPlacement, cx: &mut App) -> Result<()> {
        let mut host = self
            .host
            .try_borrow_mut()
            .map_err(|_| anyhow::anyhow!("dockspace host is already borrowed"))?;
        host.ensure_application_admission()?;
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

    /// Atomically replaces a registered pane's title and complete content sizing.
    ///
    /// Pass validated [`PaneSizing`] for maximum, fixed or preferred extents,
    /// or [`LogicalSize`] for minimum-only sizing. Core adds docking chrome.
    ///
    /// The original View, identity and content focus binding remain retained.
    /// Closed registrations can be updated before reopening. Unknown identities
    /// return [`DockPaneError::UnknownPane`]. An unaccepted candidate, queued
    /// input, gesture or presentation transition returns [`DockPaneError::Busy`].
    /// Accepted renderer outputs may finish with their original metadata; the
    /// next encoding uses fresh measurements. Native shutdown and a primary
    /// protocol failure reject changes before publishing metadata.
    /// A failed operation leaves both metadata and measurement authority intact.
    pub fn update_pane_metadata(
        &self,
        item: ItemId,
        title: impl Into<SharedString>,
        sizing: impl Into<PaneSizing>,
        cx: &mut App,
    ) -> Result<()> {
        let title = title.into();
        let sizing = sizing.into();
        let mut host = self
            .host
            .try_borrow_mut()
            .map_err(|_| DockPaneError::Busy)?;
        host.ensure_application_admission()?;
        ensure!(
            host.panels.contains_key(&item),
            DockPaneError::UnknownPane(item)
        );
        ensure!(!host.has_pane_lifecycle_work(), DockPaneError::Busy);
        let HostState {
            session,
            mounts,
            native,
            panels,
            ..
        } = &mut *host;
        let pane = panels
            .get_mut(&item)
            .ok_or(DockPaneError::UnknownPane(item))?;
        // Even an unchanged value must respect the session's original update lease.
        let mut frame = host_native::begin_frame(session, mounts, native.as_ref(), None, cx)?;
        if pane.title == title && pane.sizing == sizing {
            return Ok(());
        }
        let surface = frame.view().item(item).map(|item| item.surface());
        if let Some(surface) = surface {
            // Dirty GPUI packets alone do not revoke cached core measurements.
            // A deferred contribution requires the next pass to measure again.
            frame.defer_surface(surface, SurfaceUnavailableReason::Deferred)?;
        }
        let report = frame.commit()?;
        pane.title = title;
        pane.sizing = sizing;
        host.apply_report(report);
        if let Some(surface) = surface {
            if let Some(owner) = host.owners.get(&surface).copied() {
                if let Some(region) = host
                    .mounts
                    .get_mut(&owner)
                    .and_then(|mount| mount.regions.get_mut(&surface))
                {
                    region.dirty = true;
                }
            }
        }
        drop(host);
        cx.refresh_windows();
        host_native::pump(&self.host, cx);
        Ok(())
    }

    /// Takes a closed pane's registration and its original application View.
    ///
    /// Drop the returned pane to release the registration's content ownership,
    /// or pass it to [`Self::open_pane`] to register that same View again. The
    /// session's external-key history and document generation are preserved.
    ///
    /// Returns [`DockPaneError::Busy`] until input and presentation work drains
    /// and every retained mount has replaced its pre-close packet and input
    /// tree, or retired. Dirty or detached mounts and deferred renderer outputs
    /// cannot prove this boundary. A pruned surface with a retained diagnostic
    /// must retire its mount. Retry after real frames or mount retirement;
    /// this method never discards their owners.
    /// Open and unknown panes return their respective [`DockPaneError`] values.
    /// Native shutdown and a primary protocol failure reject registration changes
    /// before removing any content.
    pub fn take_closed_pane(&self, item: ItemId) -> Result<DockPane> {
        let mut host = self
            .host
            .try_borrow_mut()
            .map_err(|_| DockPaneError::Busy)?;
        host.ensure_application_admission()?;
        ensure!(
            host.panels.contains_key(&item),
            DockPaneError::UnknownPane(item)
        );
        ensure!(
            host.session.view().item(item).is_none(),
            DockPaneError::PaneOpen(item)
        );
        ensure!(host.can_release_closed_pane(), DockPaneError::Busy);
        host.panels
            .remove(&item)
            .ok_or_else(|| DockPaneError::UnknownPane(item).into())
    }

    /// Applies application intent atomically and requests fresh physical frames.
    ///
    /// The callback can use current-frame docking and close-decision APIs. It
    /// cannot manufacture a painted output or a submitted presentation.
    /// A still-prepared candidate owns the sole semantic update; callers may
    /// wait on [`Self::is_update_pending`] before issuing a new synchronous intent.
    /// Native shutdown and a primary protocol failure reject new intent before
    /// invoking the callback. The latter returns [`DockHostAdmissionError`]
    /// with the retained original diagnostic. Renderer and native cleanup
    /// continue through their separate owners.
    pub fn update<R>(
        &self,
        update: impl FnOnce(&mut DockspaceHostFrame<'_>) -> Result<R>,
        cx: &mut App,
    ) -> Result<R> {
        self.update_with_report(update, |_, value| value, cx)
    }

    /// Commits application intent and maps the exact submitted action's outcome.
    ///
    /// Return the token produced by the desired action from the callback. A
    /// successful frame can return a product rejection or `StaleRejected`;
    /// these remain ordinary [`HostInputOutcome`] values. The mapper receives
    /// `None` when this report has no outcome for the token, including one from
    /// another session, frame or rolled-back attempt, or an action superseded
    /// by a layout/document replacement in the same frame.
    ///
    /// A `ProductPresentationActionRequested` is pending, not a terminal success.
    /// Correlate its transition identity with [`Self::set_commit_handler`].
    ///
    /// The mapper runs synchronously after commit, while the host is borrowed,
    /// before the report is consumed. It must not reenter this surface. Copy
    /// only the application-owned information needed in the returned value.
    /// The original report and all native and renderer capabilities retain
    /// their existing host owners, including when the token does not match.
    /// If the mapper panics, the host consumes the committed report and
    /// schedules its work before resuming the panic.
    pub fn update_action<T>(
        &self,
        update: impl FnOnce(
            &mut DockspaceHostFrame<'_>,
        ) -> Result<dockspace::runtime::DockspaceSubmittedAction>,
        map: impl FnOnce(Option<&HostInputOutcome>) -> T,
        cx: &mut App,
    ) -> Result<T> {
        self.update_with_report(
            update,
            |report, submitted| map(report.submitted_action_outcome(submitted)),
            cx,
        )
    }

    fn update_with_report<R, T>(
        &self,
        update: impl FnOnce(&mut DockspaceHostFrame<'_>) -> Result<R>,
        inspect: impl FnOnce(&HostFrameReport, R) -> T,
        cx: &mut App,
    ) -> Result<T> {
        let mut host = self.host.borrow_mut();
        host.ensure_application_admission()?;
        let HostState {
            session,
            mounts,
            native,
            ..
        } = &mut *host;
        let mut frame = host_native::begin_frame(session, mounts, native.as_ref(), None, cx)?;
        let value = update(&mut frame)?;
        let report = frame.commit()?;
        let value =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inspect(&report, value)));
        host.apply_report(report);
        drop(host);
        cx.refresh_windows();
        host_native::pump(&self.host, cx);
        match value {
            Ok(value) => Ok(value),
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    /// Submits the core-owned dock-back command for an exact root in tests.
    ///
    /// This keeps recovery qualification on the shared session's presentation command path. It
    /// is intentionally available only with `test-support`; applications should expose their
    /// own recovery command or presentation affordance instead of using this seam.
    #[cfg(feature = "test-support")]
    pub fn submit_native_dock_back_for_test(
        &self,
        root: dockspace::model::RootId,
        cx: &mut App,
    ) -> Result<()> {
        let action = {
            let host = self.host.borrow();
            host.session
                .presentation_commands(root)
                .dock_back()?
                .into_prepared()
                .map_err(|error| anyhow::anyhow!("native dock-back is unavailable: {error}"))?
        };
        self.update(
            |frame| {
                frame.submit_prepared_action(action)?;
                Ok(())
            },
            cx,
        )
    }

    /// Returns the primary background diagnostic, if any.
    ///
    /// The first classified protocol, internal or foreign-update failure is
    /// retained across later cleanup errors. Other failures retain the latest
    /// diagnostic. Use [`Self::failure`] for the original core categories.
    pub fn error(&self) -> Option<String> {
        self.failure().map(|failure| failure.message)
    }

    /// Copies the primary background failure without consuming any failed owner.
    ///
    /// This is diagnostic only. A protocol failure can retain a rejected FIFO
    /// head and prevent quiescence; neither inspecting the failure nor closing
    /// its windows authorizes clearing that input or reusing the old session.
    /// For an irreparable managed generation, explicitly choose
    /// [`Self::begin_failed_native_shutdown`] and wait for
    /// [`Self::native_termination`] before disposing of the complete old host.
    pub fn failure(&self) -> Option<DockHostFailure> {
        self.host.borrow().failure.clone()
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
    pub(super) inputs: Vec<input::QueuedInput>,
    pub(super) native_effects: VecDeque<NativeEffectRequest>,
    pub(super) presentation_pending: bool,
    // Real mount removal awaiting a committed unavailable contribution.
    pub(super) unavailable_surfaces: BTreeSet<SurfaceId>,
    pub(super) failure: Option<DockHostFailure>,
    pub(super) close_handler: Option<Rc<super::close::CloseHandler>>,
    pub(super) close_requests: VecDeque<dockspace::runtime::DockspaceClosePlan>,
    pub(super) close_pump_scheduled: bool,
    pub(super) close_dispatching: bool,
    pub(super) commit_handler: Option<Rc<super::notifications::CommitHandler>>,
    pub(super) commit_notifications: VecDeque<super::notifications::DockCommitNotification>,
    pub(super) commit_pump_scheduled: bool,
    pub(super) commit_dispatching: bool,
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
    pub(super) input_frames: Vec<input::InputFrame>,
    pub(super) wake: Option<continuation::Wake>,
    pub(super) attached: bool,
    pub(super) waiting_for_update: bool,
    // Retain dirt while an output budget waits for a committed terminal prefix.
    pub(super) capacity_blocked: bool,
}

pub(super) struct RegionState {
    pub(super) bounds: Option<Bounds<Pixels>>,
    pub(super) pointer: Option<LogicalPoint>,
    pub(super) packet: Option<Rc<element::Packet>>,
    pub(super) presented: Option<(u64, SurfacePresentationArtifact)>,
    pub(super) presented_panes: BTreeMap<ItemId, open_gpui::HitTestSnapshot>,
    pub(super) dirty: bool,
}

impl HostState {
    pub(super) fn ensure_application_admission(&self) -> Result<()> {
        if let Some(failure) = self
            .failure
            .as_ref()
            .filter(|failure| failure.is_primary_protocol_failure())
        {
            return Err(DockHostAdmissionError {
                failure: failure.clone(),
            }
            .into());
        }
        if self
            .native
            .as_ref()
            .is_some_and(|native| native.transport.is_frozen())
        {
            return Err(super::native::NativeBridgeError::Draining.into());
        }
        Ok(())
    }

    pub(super) fn has_unaccepted_update(&self) -> bool {
        self.mounts.values().any(|mount| {
            mount
                .pending
                .as_ref()
                .is_some_and(|pending| pending.update.is_some())
        })
    }

    fn has_pane_lifecycle_work(&self) -> bool {
        self.has_unaccepted_update()
            || !self.inputs.is_empty()
            || self.session.has_active_gesture()
            || self.session.has_pending_presentation()
            || self.session.has_pending_native_input()
            || self
                .native
                .as_ref()
                .is_some_and(|native| !native.can_dispatch_close() || !native.transport.is_empty())
    }

    fn can_release_closed_pane(&self) -> bool {
        if self.has_pane_lifecycle_work()
            || self.presentation_pending
            || !self.unavailable_surfaces.is_empty()
            || !self.native_effects.is_empty()
            || self.session.has_pending_native_create()
            || !self.close_requests.is_empty()
            || self.close_dispatching
        {
            return false;
        }
        self.mounts.iter().all(|(owner, mount)| {
            mount.pending.is_none()
                && mount.wake.is_none()
                && mount.regions.iter().all(|(surface, region)| {
                    if region.packet.is_none() {
                        return mount
                            .input_frames
                            .iter()
                            .all(|frame| frame.surface != *surface);
                    }
                    // Every topology commit dirties all regions. Only accepting a
                    // replacement packet can clear that dirt with no pending ticket.
                    // Require its corresponding input tree and real presentation too.
                    // A pruned surface instead encodes an empty physical slot.
                    // Without an artifact to check, an earlier failed candidate
                    // makes that proof unavailable until the mount retires.
                    mount.attached
                        && !region.dirty
                        && mount
                            .input_frames
                            .iter()
                            .any(|frame| frame.surface == *surface && frame.serial == mount.serial)
                        && (input::is_presented(self, *owner, *surface, mount.serial)
                            || (self.failure.is_none()
                                && self.session.view().surface(*surface).is_none()))
                })
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
                        presented_panes: BTreeMap::new(),
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
                capacity_blocked: false,
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
        super::notifications::observe(self, report);
        super::close::observe(self, report);
        if !report.surface_presentation_commits().is_empty() {
            for mount in self.mounts.values_mut() {
                mount.capacity_blocked = false;
            }
        }
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

    pub(super) fn fail(&mut self, error: &(dyn std::error::Error + 'static)) {
        let failure = DockHostFailure::from_error(error);
        log::error!("GPUI docking: {}", failure.message);
        if !self
            .failure
            .as_ref()
            .is_some_and(DockHostFailure::is_primary_protocol_failure)
        {
            self.failure = Some(failure);
        }
    }
}

#[cfg(test)]
#[path = "host_facade_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "host_pane_lifecycle_tests.rs"]
mod pane_lifecycle_tests;

#[cfg(test)]
#[path = "composable_ownership_tests.rs"]
mod composable_ownership_tests;
