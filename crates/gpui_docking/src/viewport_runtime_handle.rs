#[cfg(test)]
use crate::DockViewportActivationTransaction;
use crate::surface::{
    DockSurfaceActivationOutcome, DockSurfaceChangeCategory, DockSurfaceOwner,
    DockSurfaceTransactionId, with_detached_root_transaction,
};
use crate::{
    DockActionApplyError, DockController, DockDropDelivery, DockHost, DockItemId, DockNodeId,
    DockSpaceId, DockViewportCloseOutcome, DockViewportClosePolicy, DockViewportCloseStatus,
    DockViewportDropRouteOutcome, DockViewportDropRouteRequest, DockViewportOpenOutcome,
    DockViewportOpenStatus, DockViewportPlacementLayout, DockViewportPlacementValidationError,
    DockViewportPlatformFocusRestoreGate, DockViewportPlatformFocusRestorePolicy,
    DockViewportPlatformSyncDispatch, DockViewportPlatformSyncRejectedReason,
    DockViewportPlatformSyncRequest, DockViewportResolvedDropRoute,
    DockViewportResolvedDropRouteOutcome, DockViewportRestoreReadiness,
    DockViewportRoutedDropPreview, DockViewportRuntime, DockViewportRuntimeStatus,
    DockViewportRuntimeUpdate, DockViewportShouldCloseOutcome, DockViewportTearOffCancelReason,
    DockViewportTearOffOpenOutcome, DockViewportTearOffPending, DockViewportTearOffRequest,
    DockViewportWindowEffects, DockViewportWindowFacts, DockViewportWindowOpenAttemptKey,
    DockVisualAffordanceDebugSummary, apply_viewport_window_effects,
    apply_viewport_window_effects_excluding, close_window_quietly,
    drag::{DockDragPayload, DockDragTearOffGeometry},
    drop_runtime::DockHostDropSceneFact,
    interaction::DockRuntimeDragSession,
    refresh_runtime_update, refresh_runtime_update_excluding,
    refresh_viewport_window_effects_excluding, refresh_windows,
    viewport_activation::{
        DockViewportActivationApplyOutcome, apply_viewport_activation_transaction,
    },
    viewport_coordinates::{
        DockViewportFrameObservation, DockViewportFrameSample, DockViewportFrameSampleRequest,
    },
    viewport_drop_scene::{
        DockViewportHostSceneDraft, DockViewportHostSceneFrame, DockViewportHostSceneRegistration,
    },
    viewport_platform_sync::{
        DockViewportPlatformSyncDispatchResult, resolve_render_passthrough_pointer_input_request,
        sync_pointer_input_window, sync_reused_viewport_window_with_request_gate,
        unavailable_reused_viewport_window_sync,
    },
    viewport_registry::DockViewportRegistrationKey,
    viewport_runtime::{
        DockViewportClaimedTearOffTarget, DockViewportPreparedTearOffBegin,
        DockViewportPreparedTearOffDrop,
    },
    viewport_window_lifecycle::DockViewportReusableWindow,
};
#[cfg(test)]
use crate::{
    DockViewportDropPayload, DockViewportDropRoute, DockViewportPlatformSignals,
    interaction::DockPayloadDropReleaseOrigin,
    viewport_registry::DockViewportRouteUnavailableReason,
};
#[cfg(test)]
use open_gpui::WindowBounds;
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, Pixels, Point, Result,
    Subscription, WeakEntity, Window, WindowId, WindowMutationDomain, WindowMutationRequest,
    WindowOptions, WindowPlacementRequest, WindowPlatformFacts,
};
#[cfg(test)]
use std::cell::{Ref, RefMut};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

mod close_ops;
mod route_ops;
mod scene_ops;

/// Cloneable application handle for the shared viewport runtime.
///
/// GPUI application-level callbacks such as [`App::on_window_closed`] require `'static` closures.
/// This handle hides the required interior mutability while keeping the runtime itself testable as
/// a normal Rust value.
#[derive(Clone, Debug)]
pub struct DockViewportRuntimeHandle {
    runtime: Rc<RefCell<DockViewportRuntime>>,
    window_closed_observer_installed: Rc<Cell<bool>>,
    platform_mutation_observation_subscriptions:
        Rc<RefCell<HashMap<DockViewportPlatformMutationSubscriptionKey, Subscription>>>,
    pending_platform_mutations:
        Rc<RefCell<HashMap<DockViewportPlatformMutationKey, DockViewportPendingPlatformMutation>>>,
    terminal_platform_mutations:
        Rc<RefCell<HashMap<DockViewportPlatformMutationKey, DockViewportTerminalPlatformMutation>>>,
    open_reservations: DockViewportOpenReservations,
    surface_commit_sink: DockViewportRuntimeCommitSink,
    active_surface_transaction: Rc<Cell<Option<DockSurfaceTransactionId>>>,
    surface_owner: Rc<RefCell<Option<WeakEntity<DockSurfaceOwner>>>>,
    #[cfg(test)]
    window_close_apply_test_hook: DockViewportWindowCloseApplyTestHook,
}

#[cfg(test)]
type DockViewportWindowCloseApplyCallback = Box<dyn FnOnce(&mut App)>;

#[cfg(test)]
#[derive(Clone, Default)]
struct DockViewportWindowCloseApplyTestHook(
    Rc<RefCell<Option<DockViewportWindowCloseApplyCallback>>>,
);

#[cfg(test)]
impl std::fmt::Debug for DockViewportWindowCloseApplyTestHook {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DockViewportWindowCloseApplyTestHook")
            .field("installed", &self.0.borrow().is_some())
            .finish()
    }
}

type DockViewportPlatformMutationKey = (WindowId, WindowMutationDomain);
type DockViewportPlatformMutationSubscriptionKey = (WindowId, WindowMutationDomain, u64);

#[derive(Clone, Debug, PartialEq)]
struct DockViewportPendingPlatformMutation {
    generation: u64,
    request: WindowMutationRequest,
    registration: Option<DockViewportRegistrationKey>,
}

#[derive(Clone, Debug, PartialEq)]
struct DockViewportTerminalPlatformMutation {
    request: WindowMutationRequest,
    facts: WindowPlatformFacts,
    registration: Option<DockViewportRegistrationKey>,
}

#[derive(Clone, Debug, Default)]
struct DockViewportOpenReservations {
    state: Rc<RefCell<DockViewportOpenReservationState>>,
}

#[derive(Debug, Default)]
struct DockViewportOpenReservationState {
    next_generation: u64,
    active: HashMap<DockSpaceId, u64>,
}

#[derive(Debug)]
struct DockViewportOpenReservation {
    state: Rc<RefCell<DockViewportOpenReservationState>>,
    space: DockSpaceId,
    generation: u64,
}

impl DockViewportOpenReservations {
    fn try_reserve(
        &self,
        space: DockSpaceId,
    ) -> std::result::Result<DockViewportOpenReservation, ()> {
        let mut state = self.state.borrow_mut();
        if state.active.contains_key(&space) {
            return Err(());
        }
        let generation = state.next_generation;
        state.next_generation = state
            .next_generation
            .checked_add(1)
            .expect("dock viewport open reservation generation space exhausted");
        state.active.insert(space.clone(), generation);
        Ok(DockViewportOpenReservation {
            state: self.state.clone(),
            space,
            generation,
        })
    }
}

impl Drop for DockViewportOpenReservation {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        if state.active.get(&self.space) == Some(&self.generation) {
            state.active.remove(&self.space);
        }
    }
}

fn immediate_terminal_window_mutation(
    dispatch: &DockViewportPlatformSyncDispatch,
) -> Option<WindowMutationRequest> {
    let request = match dispatch {
        DockViewportPlatformSyncDispatch::Unsupported(unsupported) => &unsupported.request,
        DockViewportPlatformSyncDispatch::Rejected(rejected)
            if rejected.reason == DockViewportPlatformSyncRejectedReason::RejectedByWindowApi =>
        {
            &rejected.request
        }
        DockViewportPlatformSyncDispatch::Rejected(_) => return None,
        DockViewportPlatformSyncDispatch::WindowClosed { request } => request,
        DockViewportPlatformSyncDispatch::Immediate { .. }
        | DockViewportPlatformSyncDispatch::Queued { .. }
        | DockViewportPlatformSyncDispatch::Unchanged { .. } => return None,
    };
    match request {
        DockViewportPlatformSyncRequest::PointerInput { requested } => {
            Some(WindowMutationRequest::PointerInput(*requested))
        }
        DockViewportPlatformSyncRequest::Placement { requested } => {
            Some(WindowMutationRequest::Placement(
                WindowPlacementRequest::from_window_bounds(*requested),
            ))
        }
        DockViewportPlatformSyncRequest::BackgroundAppearance { requested } => {
            Some(WindowMutationRequest::Alpha(*requested))
        }
        _ => None,
    }
}

fn relevant_window_mutation_facts_match(
    request: WindowMutationRequest,
    previous: &WindowPlatformFacts,
    current: &WindowPlatformFacts,
) -> bool {
    match request {
        WindowMutationRequest::PointerInput(_) => {
            previous.accepts_pointer_input == current.accepts_pointer_input
        }
        WindowMutationRequest::Placement(_) => {
            previous.bounds == current.bounds
                && previous.coordinate_space == current.coordinate_space
                && previous.window_bounds == current.window_bounds
                && previous.inner_window_bounds == current.inner_window_bounds
                && previous.content_size == current.content_size
                && previous.scale_factor == current.scale_factor
                && previous.display_id == current.display_id
                && previous.is_minimized == current.is_minimized
                && previous.is_maximized == current.is_maximized
                && previous.is_fullscreen == current.is_fullscreen
        }
        WindowMutationRequest::FocusOnAppearing(_) => {
            previous.focus_on_appearing == current.focus_on_appearing
        }
        WindowMutationRequest::FocusOnClick(_) => previous.focus_on_click == current.focus_on_click,
        WindowMutationRequest::Alpha(_) => {
            previous.background_appearance == current.background_appearance
        }
        WindowMutationRequest::Topmost(_) => previous.topmost == current.topmost,
        WindowMutationRequest::TaskbarVisibility(_) => {
            previous.taskbar_visible == current.taskbar_visible
        }
    }
}

type DockViewportRuntimeCommitCallback =
    dyn Fn(Option<DockSurfaceTransactionId>, &[DockSurfaceChangeCategory], &mut App);

#[derive(Clone, Default)]
struct DockViewportRuntimeCommitSink {
    callback: Rc<RefCell<Option<Rc<DockViewportRuntimeCommitCallback>>>>,
}

impl std::fmt::Debug for DockViewportRuntimeCommitSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DockViewportRuntimeCommitSink")
            .field("installed", &self.callback.borrow().is_some())
            .finish()
    }
}

impl DockViewportRuntimeCommitSink {
    fn install(
        &self,
        callback: impl Fn(Option<DockSurfaceTransactionId>, &[DockSurfaceChangeCategory], &mut App)
        + 'static,
    ) {
        let mut slot = self.callback.borrow_mut();
        assert!(
            slot.is_none(),
            "dock viewport runtime surface commit sink is already installed"
        );
        *slot = Some(Rc::new(callback));
    }

    fn publish(
        &self,
        surface_transaction: Option<DockSurfaceTransactionId>,
        categories: &[DockSurfaceChangeCategory],
        cx: &mut App,
    ) {
        if categories.is_empty() {
            return;
        }
        let callback = self.callback.borrow().clone();
        if let Some(callback) = callback {
            callback(surface_transaction, categories, cx);
        }
    }
}

#[derive(Debug)]
struct DockViewportRuntimeTransactionScope {
    active: Rc<Cell<Option<DockSurfaceTransactionId>>>,
    previous: Option<DockSurfaceTransactionId>,
}

impl Drop for DockViewportRuntimeTransactionScope {
    fn drop(&mut self) {
        self.active.set(self.previous);
    }
}

fn clear_dockhost_drop_preview_for_window(window: AnyWindowHandle, cx: &mut App) -> bool {
    window
        .update(cx, |view, _window, cx| {
            let Ok(host) = view.downcast::<DockHost>() else {
                return false;
            };
            host.update(cx, |host, _cx| host.clear_drop_preview_interaction())
        })
        .unwrap_or(false)
}

fn clear_dockhost_drop_previews(
    windows: impl IntoIterator<Item = AnyWindowHandle>,
    cx: &mut App,
) -> bool {
    let mut changed = false;
    let mut cleared_window_ids = Vec::new();
    for window in windows {
        if cleared_window_ids
            .iter()
            .any(|window_id| *window_id == window.window_id())
        {
            continue;
        }
        cleared_window_ids.push(window.window_id());
        changed |= clear_dockhost_drop_preview_for_window(window, cx);
    }
    changed
}

#[cfg(test)]
fn refresh_runtime_update_with_commit(
    runtime: &DockViewportRuntimeHandle,
    update: DockViewportRuntimeUpdate,
    cx: &mut App,
) -> bool {
    runtime.publish_surface_commit(&update, cx);
    refresh_runtime_update(update, cx)
}

#[cfg(test)]
fn apply_runtime_update(
    runtime: &DockViewportRuntimeHandle,
    update: DockViewportRuntimeUpdate,
    cx: &mut App,
) -> bool {
    let reconciled = runtime.reconcile_viewport_frame(cx);
    let changed = refresh_runtime_update_with_commit(runtime, update, cx);
    changed || reconciled
}

fn apply_runtime_update_from_window(
    runtime: &DockViewportRuntimeHandle,
    update: DockViewportRuntimeUpdate,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let current_window = window.window_handle().window_id();
    let reconciled = runtime.reconcile_viewport_frame_except_window(current_window, cx);
    runtime.publish_surface_commit(&update, cx);
    let changed = refresh_runtime_update_excluding(update, Some(current_window), cx);
    changed || reconciled
}

fn apply_viewport_window_effects_from_window_context(
    runtime: &Rc<RefCell<DockViewportRuntime>>,
    effects: DockViewportWindowEffects,
    current_window: Option<&mut Window>,
    cx: &mut App,
) {
    let current_window_id = current_window
        .as_ref()
        .map(|window| window.window_handle().window_id());
    let refresh_current = current_window_id.is_some_and(|window_id| {
        effects
            .refresh()
            .iter()
            .any(|window| window.window_id() == window_id)
    });
    apply_viewport_window_effects_excluding(runtime, effects, current_window_id, cx);
    if refresh_current && let Some(window) = current_window {
        window.refresh();
    }
}

#[derive(Debug)]
pub(crate) struct DockViewportRenderedHostScenePreparation {
    changed: bool,
    draft: DockViewportHostSceneDraft,
    expected_registration: Option<DockViewportRegistrationKey>,
    window: AnyWindowHandle,
    window_facts: DockViewportWindowFacts,
}

impl DockViewportRenderedHostScenePreparation {
    pub(crate) fn changed(&self) -> bool {
        self.changed
    }
}

#[derive(Debug)]
pub(crate) struct DockViewportRenderedHostSceneCommit {
    changed: bool,
    pub(crate) frame: Option<DockViewportHostSceneFrame>,
    registration_update: DockViewportRuntimeUpdate,
    route_preview_update: DockViewportRuntimeUpdate,
}

fn apply_close_recovery_activation_for_runtime(
    runtime: &Rc<RefCell<DockViewportRuntime>>,
    outcome: &DockViewportCloseOutcome,
    cx: &mut App,
) -> DockViewportActivationApplyOutcome {
    let prepared = runtime.borrow().prepare_close_recovery_window(outcome);
    let applied = prepared.map(|prepared| prepared.sample(cx));
    let recovery = runtime
        .borrow_mut()
        .finalize_close_recovery_activation(outcome, applied);
    let recovery_effects = recovery.window_effects();
    let _ = clear_dockhost_drop_previews(recovery_effects.refresh().iter().cloned(), cx);
    apply_viewport_window_effects(runtime, recovery_effects.clone(), cx);
    apply_viewport_activation_transaction(recovery.activation, cx)
}

fn viewport_close_removed_runtime_mapping(outcome: &DockViewportCloseOutcome) -> bool {
    matches!(
        outcome.status(),
        DockViewportCloseStatus::Closed
            | DockViewportCloseStatus::MergedBack
            | DockViewportCloseStatus::MergeBackFailed
    )
}

fn install_should_close_hook(
    runtime: DockViewportRuntimeHandle,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    window.update(cx, move |_, window, cx| {
        install_should_close_hook_from_window(runtime, window, cx);
    })
}

fn install_should_close_hook_from_window(
    runtime: DockViewportRuntimeHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let window_id = window.window_handle().window_id();
    window.on_window_should_close(cx, move |_, cx| {
        runtime
            .handle_window_should_close_with_app(window_id, cx)
            .allows_close()
    });
}

impl DockViewportRuntimeHandle {
    fn retire_window_open_attempt_for_close(
        &self,
        open_attempt: DockViewportWindowOpenAttemptKey,
        window: AnyWindowHandle,
        cx: &mut App,
    ) -> bool {
        let close = self
            .runtime
            .borrow_mut()
            .retire_window_open_attempt_for_close(open_attempt, window);
        let Some(close) = close else {
            return false;
        };
        apply_viewport_window_effects(
            &self.runtime,
            DockViewportWindowEffects::close_now_only(close),
            cx,
        );
        true
    }

    fn retire_claimed_window_open_attempt_for_close(
        &self,
        open_attempt: DockViewportWindowOpenAttemptKey,
        window: AnyWindowHandle,
        cx: &mut App,
    ) -> bool {
        let close = self
            .runtime
            .borrow_mut()
            .retire_claimed_window_open_attempt_for_close(open_attempt, window);
        let Some(close) = close else {
            return false;
        };
        apply_viewport_window_effects(
            &self.runtime,
            DockViewportWindowEffects::close_now_only(close),
            cx,
        );
        true
    }

    fn rollback_claimed_tear_off_target(
        &self,
        claimed: DockViewportClaimedTearOffTarget,
        excluded_window: Option<WindowId>,
        cx: &mut App,
    ) {
        let rolled_back = self
            .runtime
            .borrow_mut()
            .rollback_tear_off_target_claim(claimed);
        let window = rolled_back.window();
        let open_attempt = rolled_back.open_attempt();
        apply_viewport_window_effects_excluding(
            &self.runtime,
            rolled_back.into_window_effects(),
            excluded_window,
            cx,
        );
        let _ = self.retire_claimed_window_open_attempt_for_close(open_attempt, window, cx);
    }

    /// Runs runtime work inside the current surface transaction, or opens one when this handle
    /// belongs to a facade-owned surface.
    ///
    /// Runtime callbacks can synchronously re-enter GPUI while a platform window is opening or
    /// closing. The explicit scope keeps those nested observations attached to the same owner
    /// transaction without holding an entity borrow across the re-entry.
    pub(crate) fn with_surface_transaction<R>(
        &self,
        cx: &mut App,
        update: impl FnOnce(Option<DockSurfaceTransactionId>, &mut App) -> R,
    ) -> R {
        if let Some(transaction) = self.active_surface_transaction.get() {
            return update(Some(transaction), cx);
        }
        let Some(owner) = self.surface_owner() else {
            return update(None, cx);
        };
        with_detached_root_transaction(&owner, cx, |transaction, cx| {
            let _scope = self.enter_surface_transaction(transaction);
            update(Some(transaction), cx)
        })
    }

    fn settle_backend_focus_cancellation(&self, cx: &mut App) {
        self.settle_backend_focus_cancellations(cx);
    }

    /// Settles every surface activation displaced by backend-focus bookkeeping performed since
    /// the previous runtime boundary.
    ///
    /// Runtime state can be sampled more than once during one route resolution. The queue is
    /// drained only after the runtime borrow ends, and each binding schedules delivery from the
    /// owner context so callbacks cannot re-enter an active owner update.
    pub(crate) fn settle_backend_focus_cancellations<C: open_gpui::AppContext>(&self, cx: &mut C) {
        let cancellations = self.runtime.borrow_mut().take_backend_focus_cancellations();
        if cancellations.is_empty() {
            return;
        }
        for activation in cancellations {
            if let Some(binding) = activation.surface_activation_binding() {
                binding.settle(DockSurfaceActivationOutcome::Superseded, cx);
            }
        }
    }

    /// Creates a handle around a runtime with the default close policy.
    pub fn new(controller: Entity<DockController>) -> Self {
        DockViewportRuntime::new(controller).into_handle()
    }

    /// Creates a handle around a runtime with an explicit close policy.
    pub fn with_close_policy(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
    ) -> Self {
        DockViewportRuntime::with_close_policy(controller, close_policy).into_handle()
    }

    /// Creates a handle whose hosts resolve visual style in their active render context.
    pub fn with_visual_style_resolver(
        controller: Entity<DockController>,
        visual_style_resolver: crate::DockVisualStyleResolver,
    ) -> Self {
        Self::with_close_policy_and_visual_style_resolver(
            controller,
            DockViewportClosePolicy::default(),
            visual_style_resolver,
        )
    }

    /// Creates a handle with explicit close policy and host visual-style resolution.
    pub fn with_close_policy_and_visual_style_resolver(
        controller: Entity<DockController>,
        close_policy: DockViewportClosePolicy,
        visual_style_resolver: crate::DockVisualStyleResolver,
    ) -> Self {
        DockViewportRuntime::with_close_policy_and_visual_style_resolver(
            controller,
            close_policy,
            Some(visual_style_resolver),
        )
        .into_handle()
    }

    /// Creates a handle from a prepared runtime.
    pub(crate) fn from_runtime(runtime: DockViewportRuntime) -> Self {
        Self {
            runtime: Rc::new(RefCell::new(runtime)),
            window_closed_observer_installed: Rc::new(Cell::new(false)),
            platform_mutation_observation_subscriptions: Rc::new(RefCell::new(HashMap::new())),
            pending_platform_mutations: Rc::new(RefCell::new(HashMap::new())),
            terminal_platform_mutations: Rc::new(RefCell::new(HashMap::new())),
            open_reservations: DockViewportOpenReservations::default(),
            surface_commit_sink: DockViewportRuntimeCommitSink::default(),
            active_surface_transaction: Rc::new(Cell::new(None)),
            surface_owner: Rc::new(RefCell::new(None)),
            #[cfg(test)]
            window_close_apply_test_hook: DockViewportWindowCloseApplyTestHook::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn install_window_close_apply_hook_for_test(
        &self,
        hook: impl FnOnce(&mut App) + 'static,
    ) {
        let mut installed = self.window_close_apply_test_hook.0.borrow_mut();
        assert!(
            installed.is_none(),
            "dock viewport window-close apply test hook is already installed"
        );
        *installed = Some(Box::new(hook));
    }

    #[cfg(test)]
    pub(super) fn run_window_close_apply_hook_for_test(&self, cx: &mut App) {
        let hook = self.window_close_apply_test_hook.0.borrow_mut().take();
        if let Some(hook) = hook {
            hook(cx);
        }
    }

    #[cfg(test)]
    pub(crate) fn downgrade_runtime_for_test(&self) -> std::rc::Weak<RefCell<DockViewportRuntime>> {
        Rc::downgrade(&self.runtime)
    }

    pub(crate) fn install_surface_owner(&self, owner: WeakEntity<DockSurfaceOwner>) {
        let mut installed = self.surface_owner.borrow_mut();
        assert!(
            installed.is_none(),
            "dock viewport runtime surface owner is already installed"
        );
        *installed = Some(owner);
    }

    fn surface_owner(&self) -> Option<Entity<DockSurfaceOwner>> {
        self.surface_owner
            .borrow()
            .as_ref()
            .and_then(WeakEntity::upgrade)
    }

    pub(crate) fn install_surface_commit_sink(
        &self,
        callback: impl Fn(Option<DockSurfaceTransactionId>, &[DockSurfaceChangeCategory], &mut App)
        + 'static,
    ) {
        self.surface_commit_sink.install(callback);
    }

    fn publish_surface_commit(&self, update: &DockViewportRuntimeUpdate, cx: &mut App) {
        let active_transaction = self.active_surface_transaction.get();
        if let (Some(update_transaction), Some(active_transaction)) =
            (update.surface_transaction(), active_transaction)
        {
            assert_eq!(
                update_transaction, active_transaction,
                "viewport runtime commit belongs to a different active surface transaction"
            );
        }
        self.surface_commit_sink.publish(
            update.surface_transaction().or(active_transaction),
            update.change_categories(),
            cx,
        );
    }

    fn enter_surface_transaction(
        &self,
        transaction: DockSurfaceTransactionId,
    ) -> DockViewportRuntimeTransactionScope {
        let previous = self.active_surface_transaction.get();
        assert!(
            previous.is_none() || previous == Some(transaction),
            "cannot nest viewport runtime work from different surface transactions"
        );
        self.active_surface_transaction.set(Some(transaction));
        DockViewportRuntimeTransactionScope {
            active: self.active_surface_transaction.clone(),
            previous,
        }
    }

    pub(crate) fn visual_style_resolver(&self) -> Option<crate::DockVisualStyleResolver> {
        self.runtime.borrow().visual_style_resolver()
    }

    #[cfg(test)]
    pub(crate) fn borrow(&self) -> Ref<'_, DockViewportRuntime> {
        self.runtime.borrow()
    }

    #[cfg(test)]
    pub(crate) fn borrow_mut(&self) -> RefMut<'_, DockViewportRuntime> {
        self.runtime.borrow_mut()
    }

    /// Returns the shared close policy used by runtime-opened viewport windows.
    pub fn close_policy(&self) -> DockViewportClosePolicy {
        self.runtime.borrow().close_policy()
    }

    /// Returns the latest read-only runtime diagnostic snapshot.
    pub fn runtime_status(&self) -> DockViewportRuntimeStatus {
        self.runtime.borrow().runtime_status()
    }

    /// Returns runtime diagnostics enriched with the active backend and each viewport's actual
    /// window-kind mutation profile.
    pub fn runtime_status_for_app(&self, cx: &App) -> DockViewportRuntimeStatus {
        let status = self
            .runtime_status()
            .with_platform_capabilities(cx.viewport_capabilities());
        let viewport_windows = {
            let runtime = self.runtime.borrow();
            status
                .viewport_lifecycle
                .iter()
                .filter_map(|record| {
                    let window = runtime.adapter().window_for_space(&record.space)?;
                    Some((record.space.clone(), record.window_id, window))
                })
                .collect::<Vec<_>>()
        };
        let capabilities: Vec<_> = viewport_windows
            .into_iter()
            .filter_map(|(space, window_id, window)| {
                let profile = cx.window_mutation_profile(window)?;
                Some(
                    crate::DockViewportWindowMutationCapabilityRecord::from_profile(
                        space, window_id, profile,
                    ),
                )
            })
            .collect();
        status.with_window_mutation_capabilities(capabilities)
    }

    fn record_platform_dispatch_result(
        &self,
        result: DockViewportPlatformSyncDispatchResult,
        facts: &WindowPlatformFacts,
        expected_registration: Option<&DockViewportRegistrationKey>,
    ) -> bool {
        let (record, tickets) = result.into_parts();
        let window_id = record.window_id;
        if expected_registration.is_some_and(|registration| {
            !self
                .runtime
                .borrow()
                .adapter()
                .is_current_registration(registration)
        }) {
            return false;
        }
        let immediate_terminals = record
            .dispatches
            .iter()
            .filter_map(immediate_terminal_window_mutation)
            .collect::<Vec<_>>();
        self.runtime.borrow_mut().record_platform_dispatch(record);
        for request in immediate_terminals {
            self.terminal_platform_mutations.borrow_mut().insert(
                (window_id, request.domain()),
                DockViewportTerminalPlatformMutation {
                    request,
                    facts: facts.clone(),
                    registration: expected_registration.cloned(),
                },
            );
        }
        for ticket in tickets {
            self.observe_platform_mutation_ticket(
                window_id,
                ticket,
                expected_registration.cloned(),
            );
        }
        true
    }

    fn observe_platform_mutation_ticket(
        &self,
        window_id: WindowId,
        ticket: open_gpui::WindowMutationTicket,
        registration: Option<DockViewportRegistrationKey>,
    ) {
        let domain = ticket.domain();
        let generation = ticket.generation();
        let request = ticket.request();
        let mutation_key = (window_id, domain);
        let subscription_key = (window_id, domain, generation);
        self.terminal_platform_mutations
            .borrow_mut()
            .remove(&mutation_key);
        self.pending_platform_mutations.borrow_mut().insert(
            mutation_key,
            DockViewportPendingPlatformMutation {
                generation,
                request,
                registration: registration.clone(),
            },
        );

        let runtime = Rc::downgrade(&self.runtime);
        let pending_platform_mutations = self.pending_platform_mutations.clone();
        let terminal_platform_mutations = self.terminal_platform_mutations.clone();
        let platform_mutation_observation_subscriptions =
            self.platform_mutation_observation_subscriptions.clone();
        let subscription = ticket.subscribe(move |observation| {
            let registration_is_current = registration.as_ref().is_none_or(|registration| {
                runtime.upgrade().is_some_and(|runtime| {
                    runtime
                        .borrow()
                        .adapter()
                        .is_current_registration(registration)
                })
            });
            let remove_pending = pending_platform_mutations
                .borrow()
                .get(&mutation_key)
                .is_some_and(|pending| {
                    pending.generation == generation && pending.registration == registration
                });
            if remove_pending {
                pending_platform_mutations
                    .borrow_mut()
                    .remove(&mutation_key);
                if !registration_is_current
                    || matches!(
                        observation.outcome,
                        open_gpui::WindowMutationOutcome::Exact
                            | open_gpui::WindowMutationOutcome::Superseded
                    )
                {
                    terminal_platform_mutations
                        .borrow_mut()
                        .remove(&mutation_key);
                } else {
                    terminal_platform_mutations.borrow_mut().insert(
                        mutation_key,
                        DockViewportTerminalPlatformMutation {
                            request: observation.request,
                            facts: observation.facts.clone(),
                            registration: registration.clone(),
                        },
                    );
                }
            }
            platform_mutation_observation_subscriptions
                .borrow_mut()
                .remove(&subscription_key);
            let Some(runtime) = runtime.upgrade() else {
                return;
            };
            if !registration_is_current {
                return;
            }
            runtime
                .borrow_mut()
                .record_platform_observation(window_id, observation.into());
        });
        if ticket.observation().is_none() {
            self.platform_mutation_observation_subscriptions
                .borrow_mut()
                .insert(subscription_key, subscription);
        }
    }

    fn pending_platform_mutation_request(
        &self,
        window_id: WindowId,
        domain: WindowMutationDomain,
        expected_registration: Option<&DockViewportRegistrationKey>,
    ) -> Option<WindowMutationRequest> {
        self.pending_platform_mutations
            .borrow()
            .get(&(window_id, domain))
            .filter(|pending| pending.registration.as_ref() == expected_registration)
            .map(|pending| pending.request)
    }

    fn platform_mutation_retry_is_blocked(
        &self,
        window_id: WindowId,
        request: WindowMutationRequest,
        facts: &WindowPlatformFacts,
        expected_registration: Option<&DockViewportRegistrationKey>,
    ) -> bool {
        let key = (window_id, request.domain());
        let blocked = self
            .terminal_platform_mutations
            .borrow()
            .get(&key)
            .is_some_and(|terminal| {
                terminal.registration.as_ref() == expected_registration
                    && terminal.request == request
                    && relevant_window_mutation_facts_match(request, &terminal.facts, facts)
            });
        if !blocked {
            self.terminal_platform_mutations.borrow_mut().remove(&key);
        }
        blocked
    }

    fn clear_platform_mutation_observation_subscriptions(&self, window_id: WindowId) {
        self.platform_mutation_observation_subscriptions
            .borrow_mut()
            .retain(|(observed_window_id, _, _), _| *observed_window_id != window_id);
        self.pending_platform_mutations
            .borrow_mut()
            .retain(|(pending_window_id, _), _| *pending_window_id != window_id);
        self.terminal_platform_mutations
            .borrow_mut()
            .retain(|(terminal_window_id, _), _| *terminal_window_id != window_id);
    }

    pub(crate) fn record_visual_affordance_status(
        &self,
        space: DockSpaceId,
        window_id: WindowId,
        summary: DockVisualAffordanceDebugSummary,
    ) {
        self.runtime
            .borrow_mut()
            .record_visual_affordance_status(space, window_id, summary);
    }

    pub(crate) fn clear_visual_affordance_status(&self, space: &DockSpaceId, window_id: WindowId) {
        self.runtime
            .borrow_mut()
            .clear_visual_affordance_status(space, window_id);
    }

    #[cfg(test)]
    pub(crate) fn focus_command_for_confirmed_backend_window_focus(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
        mouse_down: bool,
        cx: &mut App,
    ) -> Option<crate::DockViewportFocusCommand> {
        self.confirmed_backend_window_focus_outcome(
            space,
            window_id,
            DockViewportPlatformFocusRestoreGate::from_mouse_down(mouse_down),
            cx,
        )
        .into_focus_command()
    }

    pub(crate) fn confirmed_backend_window_focus_outcome(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
        platform_focus_restore_gate: DockViewportPlatformFocusRestoreGate,
        cx: &mut App,
    ) -> crate::DockViewportConfirmedBackendFocusOutcome {
        let backend_focus = cx.focused_window();
        let controller = self.runtime.borrow().controller_entity();
        let platform_focus_restore_policy =
            DockViewportPlatformFocusRestorePolicy::from_platform_focus_sets_dock_focus(
                controller
                    .read(cx)
                    .policy()
                    .platform_focus_sets_dock_focus(),
            );
        let outcome = self
            .runtime
            .borrow_mut()
            .confirmed_backend_window_focus_outcome(
                space,
                window_id,
                platform_focus_restore_gate,
                backend_focus,
                platform_focus_restore_policy,
            );
        self.settle_backend_focus_cancellation(cx);
        outcome
    }

    pub(crate) fn reconcile_backend_window_focus(&self, cx: &mut App) -> bool {
        let backend_focus = cx.focused_window();
        let changed = self
            .runtime
            .borrow_mut()
            .record_confirmed_backend_focus_signal(backend_focus);
        self.settle_backend_focus_cancellation(cx);
        changed
    }

    pub(crate) fn apply_activation_backend_focus(
        &self,
        activation: &crate::DockViewportActivationTransaction,
        backend_focus: crate::DockViewportActivationBackendFocusObservation,
    ) -> crate::DockViewportActivationBackendFocusApply {
        self.runtime
            .borrow_mut()
            .apply_activation_backend_focus(activation, backend_focus)
    }

    pub(crate) fn settle_backend_focus_cancellation_in_context(&self, cx: &mut Context<DockHost>) {
        self.settle_backend_focus_cancellations(cx);
    }

    #[cfg(test)]
    pub(crate) fn record_confirmed_backend_focus_for_window(&self, window_id: WindowId) -> bool {
        self.runtime
            .borrow_mut()
            .record_confirmed_backend_focus_for_window(window_id)
    }

    #[cfg(test)]
    pub(crate) fn pending_activation(&self) -> Option<DockViewportActivationTransaction> {
        self.runtime.borrow().pending_activation().cloned()
    }

    #[cfg(test)]
    pub(crate) fn record_pending_activation(
        &self,
        activation: crate::DockViewportActivationTransaction,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .record_pending_activation(activation)
    }

    pub(crate) fn record_panel_focus(&self, space: DockSpaceId, item: DockItemId) {
        self.runtime.borrow_mut().record_panel_focus(space, item);
    }

    pub(crate) fn record_no_panel_focus(&self, space: &DockSpaceId) {
        self.runtime.borrow_mut().record_no_panel_focus(space);
    }

    pub(crate) fn recorded_panel_focus_matches(
        &self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> bool {
        self.runtime
            .borrow()
            .recorded_panel_focus_matches(space, item)
    }

    pub(crate) fn registration_key_for_space_window(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockViewportRegistrationKey> {
        self.runtime
            .borrow()
            .registration_key_for_space_window(space, window_id)
    }

    fn is_current_registration(&self, registration: &DockViewportRegistrationKey) -> bool {
        self.runtime
            .borrow()
            .adapter()
            .is_current_registration(registration)
    }

    pub(crate) fn release_host_binding_from_window(
        &self,
        registration: &DockViewportRegistrationKey,
        window: &Window,
        cx: &mut App,
    ) -> bool {
        let update = self.runtime.borrow_mut().release_host_binding(registration);
        refresh_runtime_update_excluding(update, Some(window.window_handle().window_id()), cx)
    }

    pub(crate) fn apply_close_recovery_activation(
        &self,
        outcome: &DockViewportCloseOutcome,
        cx: &mut App,
    ) -> DockViewportActivationApplyOutcome {
        apply_close_recovery_activation_for_runtime(&self.runtime, outcome, cx)
    }

    #[cfg(test)]
    pub(crate) fn recorded_had_panel_focus_for_test(&self, space: &DockSpaceId) -> Option<bool> {
        self.runtime
            .borrow()
            .recorded_had_panel_focus_for_test(space)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn mark_viewport_window_snapshot_stale(
        &self,
        window_id: WindowId,
        cx: &mut App,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .mark_viewport_window_snapshot_stale(window_id);
        refresh_runtime_update(update, cx)
    }

    #[cfg(test)]
    pub(crate) fn apply_platform_window_facts(
        &self,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        cx: &mut App,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .apply_platform_window_facts(window_id, window_facts);
        refresh_runtime_update_with_commit(self, update, cx)
    }

    pub(crate) fn apply_platform_window_facts_from_window(
        &self,
        window_facts: DockViewportWindowFacts,
        window: &Window,
        cx: &mut App,
    ) -> bool {
        let current_window = window.window_handle().window_id();
        let update = self
            .runtime
            .borrow_mut()
            .apply_platform_window_facts(current_window, window_facts);
        self.publish_surface_commit(&update, cx);
        refresh_runtime_update_excluding(update, Some(current_window), cx)
    }

    #[cfg(test)]
    pub(crate) fn begin_payload_drag(&self, payload: &DockDragPayload) -> DockRuntimeDragSession {
        self.runtime.borrow_mut().begin_payload_drag(payload)
    }

    #[cfg(test)]
    pub(crate) fn begin_payload_drag_with_app(
        &self,
        payload: &DockDragPayload,
        cx: &mut App,
    ) -> DockRuntimeDragSession {
        let prepared = { self.runtime.borrow().prepare_drag_focus_item(payload) };
        let focus_item = prepared.and_then(|prepared| prepared.sample(cx));
        let session = self
            .runtime
            .borrow_mut()
            .begin_payload_drag_with_focus(payload, focus_item);
        self.reconcile_viewport_frame(cx);
        session
    }

    pub(crate) fn begin_payload_drag_from_window_with_drag_visual_style(
        &self,
        payload: &DockDragPayload,
        drag_visual_style: crate::DockDragVisualStyle,
        window: &mut Window,
        cx: &mut App,
    ) -> DockRuntimeDragSession {
        let prepared = { self.runtime.borrow().prepare_drag_focus_item(payload) };
        let focus_item = prepared.and_then(|prepared| prepared.sample(cx));
        let session = self
            .runtime
            .borrow_mut()
            .begin_payload_drag_with_focus_and_drag_visual_style(
                payload,
                focus_item,
                drag_visual_style,
            );
        self.reconcile_viewport_frame_except_window(window.window_handle().window_id(), cx);
        session
    }

    pub(crate) fn update_payload_drag_tear_off_geometry(
        &self,
        session: &DockRuntimeDragSession,
        geometry: DockDragTearOffGeometry,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .update_payload_drag_tear_off_geometry(session, geometry)
    }

    pub(crate) fn active_payload_drag_session(
        &self,
        payload: &DockDragPayload,
    ) -> Option<DockRuntimeDragSession> {
        self.runtime.borrow().active_payload_drag_session(payload)
    }

    pub(crate) fn active_payload_drag_visual_style(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<crate::DockDragVisualStyle> {
        self.runtime
            .borrow()
            .active_payload_drag_visual_style(session)
    }

    pub(crate) fn active_payload_drag_source_window_id(
        &self,
        payload: &DockDragPayload,
    ) -> Option<WindowId> {
        self.runtime
            .borrow()
            .active_payload_drag_source_window_id(payload)
    }

    pub(crate) fn is_foreign_payload_drag_for_window(
        &self,
        payload: &DockDragPayload,
        receiver_window_id: WindowId,
    ) -> bool {
        self.active_payload_drag_source_window_id(payload)
            .is_some_and(|source_window_id| source_window_id != receiver_window_id)
    }

    pub(crate) fn is_payload_drag_source_window(
        &self,
        payload: &DockDragPayload,
        receiver_window_id: WindowId,
    ) -> bool {
        self.active_payload_drag_source_window_id(payload) == Some(receiver_window_id)
    }

    pub(crate) fn record_payload_drag_hovered_viewport(
        &self,
        session: &DockRuntimeDragSession,
        space: DockSpaceId,
        window_id: WindowId,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .record_payload_drag_hovered_viewport(session, space, window_id)
    }

    pub(crate) fn active_payload_drag_tear_off_geometry(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockDragTearOffGeometry> {
        self.runtime
            .borrow()
            .active_payload_drag_tear_off_geometry(session)
    }

    #[cfg(test)]
    pub(crate) fn finish_payload_drag_with_app(
        &self,
        session: &DockRuntimeDragSession,
        cx: &mut App,
    ) -> bool {
        let update = self.runtime.borrow_mut().finish_payload_drag(session);
        apply_runtime_update(self, update, cx)
    }

    pub(crate) fn finish_payload_drag_from_window(
        &self,
        session: &DockRuntimeDragSession,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let update = self.runtime.borrow_mut().finish_payload_drag(session);
        apply_runtime_update_from_window(self, update, window, cx)
    }

    pub(crate) fn finish_payload_drag_for_source_space_from_window(
        &self,
        space: &DockSpaceId,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .finish_payload_drag_for_source_space(space);
        apply_runtime_update_from_window(self, update, window, cx)
    }

    /// Returns registered dock spaces in stable lexical order.
    pub fn registered_viewport_spaces(&self) -> Vec<DockSpaceId> {
        self.runtime.borrow().adapter().spaces()
    }

    /// Returns true when a logical dock space currently has a runtime window mapping.
    pub fn is_viewport_open(&self, space: &DockSpaceId) -> bool {
        self.runtime
            .borrow()
            .adapter()
            .window_for_space(space)
            .is_some()
    }

    #[cfg(test)]
    pub(crate) fn viewport_route_ready(&self, space: &DockSpaceId) -> bool {
        self.runtime.borrow().viewport_route_ready(space)
    }

    #[cfg(test)]
    pub(crate) fn viewport_route_unavailable_reason(
        &self,
        space: &DockSpaceId,
    ) -> Option<DockViewportRouteUnavailableReason> {
        self.runtime
            .borrow()
            .viewport_route_unavailable_reason(space)
    }

    /// Replaces the shared close policy used by runtime-opened viewport windows.
    pub fn set_close_policy(&self, close_policy: DockViewportClosePolicy) {
        self.runtime.borrow_mut().set_close_policy(close_policy);
    }

    fn ensure_platform_viewport_windows_supported(&self, cx: &App) -> Result<()> {
        if cx.viewport_capabilities().platform_viewport_windows {
            return Ok(());
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "platform viewport windows are not supported by this backend",
        )
        .into())
    }

    fn ensure_platform_viewports_allowed(&self, cx: &App) -> Result<()> {
        let controller = self.runtime.borrow().controller_entity();
        cx.read_entity(&controller, |controller, _| {
            controller.policy().validate_platform_viewports()
        })?;
        Ok(())
    }

    /// Opens or reuses a controller-backed viewport window for a logical dock space.
    ///
    /// This validates the controller policy before touching backend windows. Crate-internal
    /// runtime tests that need to exercise low-level window mechanics without policy setup use the
    /// crate-private `open_viewport_unchecked_policy` helper explicitly.
    pub fn open_viewport(
        &self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportOpenOutcome> {
        self.ensure_platform_viewports_allowed(cx)?;
        self.open_viewport_unchecked_policy(space, options, cx)
    }

    /// Opens or reuses a viewport while the caller owns a live GPUI window update.
    ///
    /// Callers in render or event-listener contexts must use this entry point so reusing that same
    /// viewport does not try to borrow the current window again through [`AnyWindowHandle`].
    pub fn open_viewport_from_window(
        &self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        current_window: &mut Window,
        cx: &mut App,
    ) -> Result<DockViewportOpenOutcome> {
        self.ensure_platform_viewports_allowed(cx)?;
        self.open_viewport_unchecked_policy_for_surface_transaction(
            space,
            options,
            None,
            Some(current_window),
            cx,
        )
    }

    /// Opens or reuses a controller-backed viewport window after the caller has handled policy.
    ///
    /// The handle installs a should-close hook that consults the shared runtime at close time, so
    /// later close-policy changes are observed by already-open windows.
    pub(crate) fn open_viewport_unchecked_policy(
        &self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportOpenOutcome> {
        self.open_viewport_unchecked_policy_for_surface_transaction(space, options, None, None, cx)
    }

    pub(crate) fn open_viewport_unchecked_policy_in_transaction(
        &self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        surface_transaction: DockSurfaceTransactionId,
        cx: &mut App,
    ) -> Result<DockViewportOpenOutcome> {
        let _surface_transaction_scope = self.enter_surface_transaction(surface_transaction);
        self.open_viewport_unchecked_policy_for_surface_transaction(
            space,
            options,
            Some(surface_transaction),
            None,
            cx,
        )
    }

    fn open_viewport_unchecked_policy_for_surface_transaction(
        &self,
        space: impl Into<DockSpaceId>,
        options: WindowOptions,
        surface_transaction: Option<DockSurfaceTransactionId>,
        mut current_window: Option<&mut Window>,
        cx: &mut App,
    ) -> Result<DockViewportOpenOutcome> {
        self.ensure_platform_viewport_windows_supported(cx)?;
        self.ensure_window_closed_observer(cx);

        let space = space.into();
        let _open_reservation =
            self.open_reservations
                .try_reserve(space.clone())
                .map_err(|()| {
                    std::io::Error::new(
                        std::io::ErrorKind::WouldBlock,
                        format!(
                            "viewport open is already in progress for dock space `{}`",
                            space.as_str()
                        ),
                    )
                })?;
        let live_window = current_window.as_ref().map(|window| window.window_handle());
        let reusable_probe = self
            .runtime
            .borrow()
            .prepare_reusable_window_for_space(&space, live_window);
        let reusable_observation = reusable_probe.sample(cx);
        let reusable_outcome = self
            .runtime
            .borrow_mut()
            .finalize_reusable_window(reusable_observation);
        let reusable_topology_changed = reusable_outcome.topology_changed();
        let (reusable, reusable_effects) = reusable_outcome.into_parts();
        let status = match reusable {
            DockViewportReusableWindow::Reused {
                registration,
                window,
            } => {
                let existing_kind = match cx.window_mutation_profile(window) {
                    Some(profile) => profile.kind.clone(),
                    None => {
                        if self.is_current_registration(&registration) {
                            self.runtime.borrow_mut().record_platform_dispatch(
                                unavailable_reused_viewport_window_sync(window.window_id()),
                            );
                        }
                        return Err(std::io::Error::other(
                            "reused viewport window has no registered mutation profile",
                        )
                        .into());
                    }
                };
                let reuses_current_window = current_window.as_ref().is_some_and(|current_window| {
                    current_window.window_handle().window_id() == window.window_id()
                });
                if reuses_current_window {
                    install_should_close_hook_from_window(
                        self.clone(),
                        current_window
                            .as_deref_mut()
                            .expect("current viewport window was checked above"),
                        cx,
                    );
                } else if let Err(error) = install_should_close_hook(self.clone(), window, cx) {
                    if self.is_current_registration(&registration) {
                        self.runtime.borrow_mut().record_platform_dispatch(
                            unavailable_reused_viewport_window_sync(window.window_id()),
                        );
                    }
                    return Err(error);
                }
                if !self.is_current_registration(&registration) {
                    return Err(std::io::Error::other(
                        "reused viewport registration changed while installing close handling",
                    )
                    .into());
                }
                let platform_requests = self.runtime.borrow().platform_requests_for_space(&space);
                let runtime = self.clone();
                let window_id = window.window_id();
                let sync_window = |window: &mut Window| {
                    let sync_result = sync_reused_viewport_window_with_request_gate(
                        window,
                        &existing_kind,
                        options,
                        platform_requests,
                        |request, facts| {
                            runtime.is_current_registration(&registration)
                                && runtime.pending_platform_mutation_request(
                                    window_id,
                                    request.domain(),
                                    Some(&registration),
                                ) != Some(request)
                                && !runtime.platform_mutation_retry_is_blocked(
                                    window_id,
                                    request,
                                    facts,
                                    Some(&registration),
                                )
                        },
                    );
                    (sync_result, window.platform_facts().clone())
                };
                let (sync_result, platform_facts) = if reuses_current_window {
                    sync_window(
                        current_window
                            .as_deref_mut()
                            .expect("current viewport window was checked above"),
                    )
                } else {
                    match window.update(cx, |_, window, _| sync_window(window)) {
                        Ok(sync_result) => sync_result,
                        Err(error) => {
                            if self.is_current_registration(&registration) {
                                self.runtime.borrow_mut().record_platform_dispatch(
                                    unavailable_reused_viewport_window_sync(window.window_id()),
                                );
                            }
                            return Err(error);
                        }
                    }
                };
                self.record_platform_dispatch_result(
                    sync_result,
                    &platform_facts,
                    Some(&registration),
                )
                .then_some(())
                .ok_or_else(|| {
                    std::io::Error::other(
                        "reused viewport registration changed during platform synchronization",
                    )
                })?;
                if let Some(current_window_id) = current_window
                    .as_ref()
                    .map(|current_window| current_window.window_handle().window_id())
                {
                    self.reconcile_viewport_frame_except_window(current_window_id, cx);
                } else {
                    self.reconcile_viewport_frame(cx);
                }
                if !self.is_current_registration(&registration) {
                    return Err(std::io::Error::other(
                        "reused viewport registration changed during frame reconciliation",
                    )
                    .into());
                }
                if reuses_current_window {
                    current_window
                        .as_deref_mut()
                        .expect("current viewport window was checked above")
                        .refresh();
                } else {
                    refresh_windows(vec![window], cx);
                }
                if !self.is_current_registration(&registration) {
                    return Err(std::io::Error::other(
                        "reused viewport registration changed during refresh",
                    )
                    .into());
                }
                return Ok(DockViewportOpenOutcome::new(
                    space,
                    window,
                    DockViewportOpenStatus::Reused,
                ));
            }
            DockViewportReusableWindow::Stale => DockViewportOpenStatus::Replaced,
            DockViewportReusableWindow::Missing => DockViewportOpenStatus::Opened,
        };
        apply_viewport_window_effects_from_window_context(
            &self.runtime,
            reusable_effects,
            current_window.as_deref_mut(),
            cx,
        );
        let mut stale_cleanup_update = DockViewportRuntimeUpdate::default();
        stale_cleanup_update.mark_viewport_topology(reusable_topology_changed, surface_transaction);
        self.publish_surface_commit(&stale_cleanup_update, cx);

        let controller = self.runtime.borrow().controller_entity();
        let host_space = space.clone();
        let host_runtime = self.clone();
        let open_attempt_runtime = host_runtime.clone();
        let open_attempt_slot = Rc::new(Cell::new(None));
        let open_attempt_slot_for_builder = open_attempt_slot.clone();
        let surface_owner = self.surface_owner();
        let open_result = cx.open_window(options, move |window, cx| {
            let open_attempt = open_attempt_runtime
                .runtime
                .borrow_mut()
                .begin_window_open_attempt(window.window_handle().window_id());
            open_attempt_slot_for_builder.set(open_attempt);
            cx.new(move |cx| match surface_owner {
                Some(surface_owner) => DockHost::from_surface_owner(
                    controller,
                    host_space,
                    host_runtime,
                    &surface_owner,
                    cx,
                ),
                None => DockHost::from_controller(controller, host_space, host_runtime, cx),
            })
        });
        let window = match open_result {
            Ok(window) => window.into(),
            Err(error) => {
                if let Some(open_attempt) = open_attempt_slot.take() {
                    let _ = self
                        .runtime
                        .borrow_mut()
                        .abort_window_open_attempt(open_attempt);
                }
                return Err(error);
            }
        };
        let Some(open_attempt) = open_attempt_slot.take() else {
            close_window_quietly(window, cx);
            return Err(std::io::Error::other(
                "opened viewport window id is already owned by another runtime generation",
            )
            .into());
        };

        if let Err(error) = install_should_close_hook(self.clone(), window, cx) {
            let _ = self.retire_window_open_attempt_for_close(open_attempt, window, cx);
            return Err(error);
        }

        let registration = match surface_transaction {
            Some(surface_transaction) => self
                .runtime
                .borrow_mut()
                .register_opened_viewport_from_attempt_with_cleanup_in_transaction(
                    space.clone(),
                    window,
                    open_attempt,
                    surface_transaction,
                ),
            None => self
                .runtime
                .borrow_mut()
                .register_opened_viewport_from_attempt_with_cleanup(
                    space.clone(),
                    window,
                    open_attempt,
                ),
        };
        let registration = match registration {
            Ok(Some(registration)) => registration,
            Ok(None) => {
                let _ = self.retire_window_open_attempt_for_close(open_attempt, window, cx);
                return Err(std::io::Error::other(
                    "viewport open attempt was superseded before registration",
                )
                .into());
            }
            Err(error) => {
                let _ = self.retire_window_open_attempt_for_close(open_attempt, window, cx);
                return Err(error.into());
            }
        };
        let registration_key = registration.outcome.registration_key().clone();
        self.publish_surface_commit(registration.runtime_update(), cx);
        let registration_current_after_publication =
            self.is_current_registration(&registration_key);
        apply_viewport_window_effects_from_window_context(
            &self.runtime,
            registration.window_effects(),
            current_window.as_deref_mut(),
            cx,
        );
        if !registration_current_after_publication {
            let _ = self.retire_claimed_window_open_attempt_for_close(open_attempt, window, cx);
            return Err(std::io::Error::other(
                "opened viewport registration changed during surface publication",
            )
            .into());
        }
        if !self.is_current_registration(&registration_key) {
            let _ = self.retire_claimed_window_open_attempt_for_close(open_attempt, window, cx);
            return Err(std::io::Error::other(
                "opened viewport registration changed while applying window effects",
            )
            .into());
        }
        refresh_windows(vec![window], cx);
        if !self.is_current_registration(&registration_key) {
            let _ = self.retire_claimed_window_open_attempt_for_close(open_attempt, window, cx);
            return Err(std::io::Error::other(
                "opened viewport registration changed during refresh",
            )
            .into());
        }

        Ok(DockViewportOpenOutcome::new(space, window, status))
    }

    fn open_unregistered_viewport_window(
        &self,
        space: DockSpaceId,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<(AnyWindowHandle, DockViewportWindowOpenAttemptKey)> {
        self.ensure_platform_viewports_allowed(cx)?;
        self.ensure_platform_viewport_windows_supported(cx)?;
        self.ensure_window_closed_observer(cx);

        let controller = self.runtime.borrow().controller_entity();
        let host_runtime = self.clone();
        let open_attempt_runtime = host_runtime.clone();
        let open_attempt_slot = Rc::new(Cell::new(None));
        let open_attempt_slot_for_builder = open_attempt_slot.clone();
        let surface_owner = self.surface_owner();
        let open_result = cx.open_window(options, move |window, cx| {
            let open_attempt = open_attempt_runtime
                .runtime
                .borrow_mut()
                .begin_window_open_attempt(window.window_handle().window_id());
            open_attempt_slot_for_builder.set(open_attempt);
            cx.new(move |cx| match surface_owner {
                Some(surface_owner) => DockHost::from_surface_owner(
                    controller,
                    space,
                    host_runtime,
                    &surface_owner,
                    cx,
                ),
                None => DockHost::from_controller(controller, space, host_runtime, cx),
            })
        });
        let window = match open_result {
            Ok(window) => window.into(),
            Err(error) => {
                if let Some(open_attempt) = open_attempt_slot.take() {
                    let _ = self
                        .runtime
                        .borrow_mut()
                        .abort_window_open_attempt(open_attempt);
                }
                return Err(error);
            }
        };
        let Some(open_attempt) = open_attempt_slot.take() else {
            close_window_quietly(window, cx);
            return Err(std::io::Error::other(
                "opened tear-off window id is already owned by another runtime generation",
            )
            .into());
        };

        if let Err(error) = install_should_close_hook(self.clone(), window, cx) {
            self.retire_window_open_attempt_for_close(open_attempt, window, cx);
            return Err(error);
        }

        Ok((window, open_attempt))
    }

    /// Opens a controller-backed viewport window and completes a tear-off transaction.
    #[cfg(test)]
    pub(crate) fn open_tear_off_viewport(
        &self,
        request: DockViewportTearOffRequest,
        target_space: impl Into<DockSpaceId>,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        let target_space = target_space.into();
        let probe = self.runtime.borrow().prepare_tear_off_drop_route_for_test(
            request,
            target_space,
            options,
        );
        let prepared = probe.sample(cx)?;
        self.open_prepared_tear_off_viewport(prepared, None, cx)
    }

    fn open_prepared_tear_off_viewport(
        &self,
        prepared: DockViewportPreparedTearOffDrop,
        excluded_window: Option<WindowId>,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        if self.is_viewport_open(prepared.target_space()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "tear-off target space {} is already open",
                    prepared.target_space()
                ),
            )
            .into());
        }
        let begin = self
            .runtime
            .borrow_mut()
            .begin_prepared_tear_off_drop(prepared);
        match begin {
            DockViewportPreparedTearOffBegin::Pending(prepared_window) => self
                .complete_opened_tear_off_viewport(
                    prepared_window.pending,
                    prepared_window.options,
                    excluded_window,
                    cx,
                ),
            DockViewportPreparedTearOffBegin::Duplicate(pending) => {
                let outcome = DockViewportTearOffOpenOutcome::Duplicate(pending);
                self.runtime.borrow_mut().record_tear_off_outcome(&outcome);
                Ok(outcome)
            }
            DockViewportPreparedTearOffBegin::Unavailable(pending) => Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "tear-off target space {} is already registered or reserved",
                    pending.target_space()
                ),
            )
            .into()),
        }
    }

    fn complete_opened_tear_off_viewport(
        &self,
        pending: DockViewportTearOffPending,
        options: WindowOptions,
        excluded_window: Option<WindowId>,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        let (window, open_attempt) = match self.open_unregistered_viewport_window(
            pending.target_space().clone(),
            options,
            cx,
        ) {
            Ok(window) => window,
            Err(error) => {
                self.runtime
                    .borrow_mut()
                    .cancel_tear_off_pending(&pending, DockViewportTearOffCancelReason::Cancelled);
                return Err(error);
            }
        };
        if !self
            .runtime
            .borrow_mut()
            .bind_tear_off_target_window(&pending, window)
        {
            self.retire_window_open_attempt_for_close(open_attempt, window, cx);
            let _ = self
                .runtime
                .borrow_mut()
                .cancel_tear_off_pending(&pending, DockViewportTearOffCancelReason::Cancelled);
            return Err(std::io::Error::other(
                "tear-off target reservation changed while opening its viewport",
            )
            .into());
        }

        self.finish_opened_tear_off_viewport(pending, window, open_attempt, excluded_window, cx)
    }

    fn finish_opened_tear_off_viewport(
        &self,
        pending: DockViewportTearOffPending,
        window: AnyWindowHandle,
        open_attempt: DockViewportWindowOpenAttemptKey,
        excluded_window: Option<WindowId>,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        let source_check = self
            .runtime
            .borrow()
            .prepare_tear_off_source_check(&pending);
        let source_observation = source_check.sample(cx);
        let cancelled = self
            .runtime
            .borrow_mut()
            .finalize_tear_off_source_check(source_observation);
        if let Some(cancelled) = cancelled {
            self.retire_window_open_attempt_for_close(open_attempt, window, cx);
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "tear-off source no longer available before completion for target {}: {:?}",
                    cancelled.pending().target_space(),
                    cancelled.reason(),
                ),
            )
            .into());
        }

        let prepared_claim = match {
            self.runtime
                .borrow()
                .prepare_tear_off_target_claim(&pending, window, open_attempt)
        } {
            Ok(prepared) => prepared,
            Err(error) => {
                let _ = self
                    .runtime
                    .borrow_mut()
                    .cancel_tear_off_pending(&pending, DockViewportTearOffCancelReason::Cancelled);
                self.retire_window_open_attempt_for_close(open_attempt, window, cx);
                return Err(error.into());
            }
        };
        let applied_claim = prepared_claim.sample(cx);
        let claim_result = self
            .runtime
            .borrow_mut()
            .finalize_tear_off_target_claim(applied_claim);
        let claimed = match claim_result {
            Ok(claimed) => claimed,
            Err(error) => {
                let _ = self
                    .runtime
                    .borrow_mut()
                    .cancel_tear_off_pending(&pending, DockViewportTearOffCancelReason::Cancelled);
                self.retire_window_open_attempt_for_close(open_attempt, window, cx);
                return Err(error.into());
            }
        };

        let prepared_move = {
            self.runtime
                .borrow_mut()
                .prepare_tear_off_move_apply(&pending)
        };
        let applied_move = prepared_move.map(|prepared| prepared.apply(cx));
        let commit_result = applied_move.and_then(|applied| {
            self.runtime
                .borrow_mut()
                .finalize_tear_off_move_apply(applied)
        });
        let (committed, source_is_empty) = match commit_result {
            Ok(committed) => committed,
            Err(error) => {
                self.rollback_claimed_tear_off_target(claimed, excluded_window, cx);
                return Err(error.into());
            }
        };
        let completion_result = self.runtime.borrow_mut().complete_tear_off_registration(
            committed,
            claimed,
            source_is_empty,
        );
        let (completed, runtime_update) = match completion_result {
            Ok(finalized) => finalized,
            Err((error, claimed)) => {
                self.rollback_claimed_tear_off_target(claimed, excluded_window, cx);
                return Err(error.into());
            }
        };
        let registration_key = completed.registration().registration_key().clone();
        self.publish_surface_commit(&runtime_update, cx);
        let mut graph_update = DockViewportRuntimeUpdate::default();
        graph_update.mark_graph_commit(
            completed.action().changed(),
            self.active_surface_transaction.get(),
        );
        self.publish_surface_commit(&graph_update, cx);
        let registration_current_after_publication =
            self.is_current_registration(&registration_key);
        apply_viewport_window_effects_excluding(
            &self.runtime,
            completed.window_effects(),
            excluded_window,
            cx,
        );
        if !registration_current_after_publication {
            self.retire_claimed_window_open_attempt_for_close(open_attempt, window, cx);
            return Err(std::io::Error::other(
                "tear-off viewport registration changed during surface publication",
            )
            .into());
        }
        if !self.is_current_registration(&registration_key) {
            self.retire_claimed_window_open_attempt_for_close(open_attempt, window, cx);
            return Err(std::io::Error::other(
                "tear-off viewport registration changed while applying window effects",
            )
            .into());
        }
        let outcome = DockViewportTearOffOpenOutcome::Completed(completed);
        self.runtime.borrow_mut().record_tear_off_outcome(&outcome);
        Ok(outcome)
    }

    #[cfg(test)]
    pub(crate) fn complete_opened_tear_off_viewport_for_test(
        &self,
        pending: DockViewportTearOffPending,
        window: AnyWindowHandle,
        cx: &mut App,
    ) -> Result<DockViewportTearOffOpenOutcome> {
        let Some(open_attempt) = self
            .runtime
            .borrow_mut()
            .begin_window_open_attempt(window.window_id())
        else {
            return Err(std::io::Error::other(
                "test tear-off window id is already owned by another runtime generation",
            )
            .into());
        };
        if !self
            .runtime
            .borrow_mut()
            .bind_tear_off_target_window(&pending, window)
        {
            self.retire_window_open_attempt_for_close(open_attempt, window, cx);
            return Err(std::io::Error::other(
                "test tear-off viewport did not own the target reservation",
            )
            .into());
        }
        self.finish_opened_tear_off_viewport(pending, window, open_attempt, None, cx)
    }

    /// Exports serializable placement snapshots from the shared runtime.
    pub fn export_placement(&self) -> DockViewportPlacementLayout {
        self.runtime.borrow().export_placement()
    }

    /// Checks saved placement snapshots against windows currently registered in the runtime.
    ///
    /// This does not open, move, or resize platform windows. Use
    /// [`DockViewportPlacementLayout::window_options_for_space`] when opening a viewport from
    /// saved placement.
    pub fn check_placement_restore(
        &self,
        placement: &DockViewportPlacementLayout,
    ) -> Result<DockViewportRestoreReadiness, DockViewportPlacementValidationError> {
        self.runtime.borrow_mut().check_placement_restore(placement)
    }
}

#[cfg(test)]
mod current_window_open_tests {
    use super::{DockViewportRuntimeHandle, apply_viewport_window_effects_from_window_context};
    use crate::{
        DockController, DockGraph, DockNode, DockSpaceId, DockViewportOpenStatus,
        DockViewportWindowEffects, DockWorkspace,
        host_test_support::{item, test_view, viewport_window_options},
    };
    use open_gpui::{
        AppContext as _, Context, IntoElement, Render, TestAppContext, Window, div, px, size,
    };
    use std::{cell::Cell, rc::Rc};

    struct RenderCounter {
        renders: Rc<Cell<usize>>,
    }

    impl Render for RenderCounter {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            div()
        }
    }

    #[open_gpui::test]
    fn current_window_reuse_does_not_require_generic_handle_update(cx: &mut TestAppContext) {
        let space = DockSpaceId::from("secondary");
        let mut graph = DockGraph::new();
        let root = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(space.clone(), root);

        let mut workspace = DockWorkspace::new(space.clone(), graph);
        workspace.policy_mut().set_allow_platform_viewports(true);
        workspace.register_panel_view(item("b"), "Panel B", test_view(cx, "B"));
        let controller = cx.new(|_| DockController::new(workspace));
        let runtime = DockViewportRuntimeHandle::new(controller);

        let opened = cx
            .update(|app| {
                runtime.open_viewport(space.clone(), viewport_window_options(360.0, 220.0), app)
            })
            .expect("viewport should open before exercising current-window reuse");
        let window = opened.window();
        let registration = runtime
            .registration_key_for_space_window(&space, window.window_id())
            .expect("opened viewport should own a registration");

        let reused = window
            .update(cx, |_, current_window, app| {
                assert!(
                    window.update(app, |_, _, _| ()).is_err(),
                    "generic handle update must be unavailable while the current window is borrowed"
                );
                runtime.open_viewport_from_window(
                    space.clone(),
                    viewport_window_options(480.0, 260.0),
                    current_window,
                    app,
                )
            })
            .expect("outer current-window update should remain live")
            .expect("explicit current-window reuse should succeed");

        assert_eq!(reused.status(), DockViewportOpenStatus::Reused);
        assert_eq!(reused.window(), window);
        assert_eq!(
            runtime.registration_key_for_space_window(&space, window.window_id()),
            Some(registration),
            "current-window reuse must preserve the existing registration generation"
        );
        assert_eq!(
            runtime
                .runtime_status()
                .last_platform_dispatch
                .as_ref()
                .map(|record| record.window_id),
            Some(window.window_id()),
            "current-window reuse should still publish platform synchronization diagnostics"
        );
    }

    #[open_gpui::test]
    fn current_window_refresh_effect_uses_live_window_borrow(cx: &mut TestAppContext) {
        let controller = cx.new(|_| {
            DockController::new(DockWorkspace::new(
                DockSpaceId::from("main"),
                DockGraph::new(),
            ))
        });
        let runtime = DockViewportRuntimeHandle::new(controller);
        let renders = Rc::new(Cell::new(0));
        let window = cx.open_window(size(px(320.0), px(240.0)), |_, _| RenderCounter {
            renders: renders.clone(),
        });
        cx.run_until_parked();
        let renders_before_refresh = renders.get();
        let any_window: open_gpui::AnyWindowHandle = window.into();

        window
            .update(cx, |_, current_window, app| {
                assert!(
                    any_window.update(app, |_, _, _| ()).is_err(),
                    "the generic handle must be unavailable during a live window update"
                );
                apply_viewport_window_effects_from_window_context(
                    &runtime.runtime,
                    DockViewportWindowEffects::refresh_only([any_window]),
                    Some(current_window),
                    app,
                );
            })
            .expect("current-window refresh effect must not reborrow the window");
        cx.run_until_parked();

        assert!(
            renders.get() > renders_before_refresh,
            "a current-window refresh effect must schedule a real follow-up render"
        );
    }
}

#[cfg(test)]
mod open_reservation_tests {
    use super::DockViewportOpenReservations;
    use crate::DockSpaceId;

    #[test]
    fn open_reservations_are_exclusive_per_space_only() {
        let reservations = DockViewportOpenReservations::default();
        let main = DockSpaceId::from("main");
        let secondary = DockSpaceId::from("secondary");

        let main_reservation = reservations
            .try_reserve(main.clone())
            .expect("first main open should reserve the space");
        assert!(
            reservations.try_reserve(main.clone()).is_err(),
            "a nested open for the same space must not become a second creator"
        );
        let secondary_reservation = reservations
            .try_reserve(secondary)
            .expect("an unrelated space should retain independent open progress");

        drop(main_reservation);
        let replacement = reservations
            .try_reserve(main)
            .expect("dropping the current reservation should release its space");
        drop(replacement);
        drop(secondary_reservation);
    }

    #[test]
    fn stale_open_reservation_drop_does_not_clear_replacement_generation() {
        let reservations = DockViewportOpenReservations::default();
        let main = DockSpaceId::from("main");
        let stale = reservations
            .try_reserve(main.clone())
            .expect("initial open should reserve the space");

        let replacement_generation = {
            let mut state = reservations.state.borrow_mut();
            let generation = stale.generation + 1;
            state.active.insert(main.clone(), generation);
            generation
        };
        drop(stale);

        assert_eq!(
            reservations.state.borrow().active.get(&main),
            Some(&replacement_generation),
            "an old guard must not release a replacement reservation"
        );
    }
}

#[cfg(test)]
mod window_mutation_retry_tests {
    use super::relevant_window_mutation_facts_match;
    use open_gpui::{
        Bounds, WindowBackgroundAppearance, WindowBounds, WindowCoordinateSpace,
        WindowMutationRequest, WindowPlacementRequest, WindowPlatformFacts, point, px, size,
    };

    fn facts() -> WindowPlatformFacts {
        let bounds = Bounds::new(point(px(10.0), px(20.0)), size(px(300.0), px(200.0)));
        WindowPlatformFacts {
            bounds,
            coordinate_space: WindowCoordinateSpace::GlobalScreen,
            window_bounds: WindowBounds::Windowed(bounds),
            inner_window_bounds: WindowBounds::Windowed(bounds),
            content_size: bounds.size,
            scale_factor: 1.0,
            display_id: None,
            is_minimized: false,
            is_maximized: false,
            is_fullscreen: false,
            accepts_pointer_input: true,
            focus_on_appearing: true,
            focus_on_click: true,
            background_appearance: WindowBackgroundAppearance::Opaque,
            topmost: false,
            taskbar_visible: true,
            is_active: true,
        }
    }

    #[test]
    fn placement_retry_fingerprint_ignores_unrelated_active_and_pointer_facts() {
        let request =
            WindowMutationRequest::Placement(WindowPlacementRequest::windowed(facts().bounds));
        let previous = facts();
        let mut current = previous.clone();
        current.is_active = false;
        current.accepts_pointer_input = false;

        assert!(relevant_window_mutation_facts_match(
            request, &previous, &current
        ));

        current.scale_factor = 1.5;
        assert!(!relevant_window_mutation_facts_match(
            request, &previous, &current
        ));
    }
}
