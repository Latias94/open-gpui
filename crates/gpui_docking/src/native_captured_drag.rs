use crate::{
    DockCapturedNativeDropRoute, DockCapturedNativeHostTarget, DockHost, DockHostWindowBinding,
    DockSpaceId, DockViewportDropRouteRequest, DockViewportHostGeometry,
    DockViewportLockedDropRoute, DockViewportRoutedPreviewOwner, DockViewportRuntimeHandle,
    DockViewportRuntimeIdentity, DockViewportRuntimeWorkContext,
    drag::DockDragPayload,
    interaction::DockRuntimeDragSession,
    presentation_scene::DockPresentationScene,
    surface::live_undock::{
        DockLiveUndockCancelReason, DockLiveUndockDragGeneration, DockLiveUndockFact,
        DockLiveUndockHostTarget, DockLiveUndockIdentity, DockLiveUndockPhysicalBounds,
        DockLiveUndockPhysicalPoint, DockLiveUndockPlacementGeneration, DockLiveUndockReleaseLock,
        DockLiveUndockRouteFeedback, DockLiveUndockSourceFocusSnapshot,
        DockLiveUndockSourceSnapshot, DockLiveUndockTrigger,
    },
    surface::live_undock_runtime::{
        DockLiveUndockExecutionSeed, DockLiveUndockHostReleaseAuthority,
        DockLiveUndockReleaseAdoption, DockPayloadDragFinalizer,
        settle_payload_drag_finalizer_claim,
    },
    viewport_drop_scene::{DockViewportHostSceneDraft, DockViewportHostSceneFrame},
};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, DragStartGeometry, Global, NativeCapturedDragEvent,
    NativeCapturedDragGeneration, NativeCapturedDragPhase, NativeCapturedDragReleaseBarrier,
    NativeCapturedDragReleaseTerminal, NativeIngressSequence, Pixels,
    PlatformNativeDragStartSnapshot, PlatformWindowHit, PlatformWindowHitStack, Point,
    PointerCancelReason, PreparedNativeCapturedDragConsumer, Subscription, WeakEntity, Window,
    WindowId,
};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
    sync::Arc,
};

struct DockNativeCapturedDragRouter {
    state: Rc<RefCell<DockNativeCapturedDragState>>,
    _drag_subscription: Subscription,
    _window_closed_subscription: Subscription,
    _window_native_terminal_subscription: Subscription,
}

impl Global for DockNativeCapturedDragRouter {}

#[derive(Default)]
struct DockNativeCapturedDragState {
    next_epoch: u64,
    active: Option<DockNativeCapturedDragRoute>,
    locked_releases: HashMap<u64, Arc<DockNativeCapturedReleaseReservation>>,
    retired_pending:
        HashMap<DockNativeCapturedDragRetiredKey, DockNativeCapturedDragRetiredPending>,
    // A failed barrier remains unsafe until its surface claims the failure or the exact source
    // window reaches native terminal state.
    failed_releases: HashSet<DockNativeCapturedDragRetiredKey>,
    scenes: HashMap<WindowId, Vec<DockNativeCapturedHostScene>>,
    #[cfg(test)]
    panic_next: Option<DockNativeCapturedDragTestPanic>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct DockNativeCapturedDragRetiredKey {
    runtime_identity: DockViewportRuntimeIdentity,
    surface_lease: crate::surface::window_session::DockSurfaceWindowSessionLease,
    source_window: WindowId,
    drag_generation: NativeCapturedDragGeneration,
}

// Logical route retirement can precede native capture release. Retain the route until the exact
// release barrier settles so a later surface shutdown can claim its cleanup continuation.
struct DockNativeCapturedDragRetiredPending {
    barrier: NativeCapturedDragReleaseBarrier,
    cleanup: Option<DockNativeCapturedRouteCleanup>,
}

struct DockNativeCapturedRouteCleanup {
    route: DockNativeCapturedDragRoute,
    first_panic: Option<Box<dyn Any + Send>>,
    locked_drop: Option<Result<DockViewportLockedDropRoute, crate::DockActionApplyError>>,
    live_undock_release_adopted: bool,
}

impl DockNativeCapturedDragRetiredKey {
    fn for_route(route: &DockNativeCapturedDragRoute) -> Option<Self> {
        let crate::DockViewportRuntimeLineage::Surface(surface_lease) =
            route.work_context.lineage()
        else {
            return None;
        };
        Some(Self {
            runtime_identity: route.runtime_identity,
            surface_lease,
            source_window: route.source_window,
            drag_generation: route.generation,
        })
    }

    fn belongs_to_surface(
        self,
        runtime_identity: DockViewportRuntimeIdentity,
        surface_lease: crate::surface::window_session::DockSurfaceWindowSessionLease,
    ) -> bool {
        self.runtime_identity == runtime_identity && self.surface_lease == surface_lease
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockNativeCapturedDragTestPanic {
    BeginRouteAfterInstall,
    ResolveTarget,
}

#[derive(Clone, Debug)]
pub(crate) struct DockNativeCapturedDragRouteReceipt {
    epoch: u64,
    generation: NativeCapturedDragGeneration,
    runtime_identity: DockViewportRuntimeIdentity,
    source_binding: DockHostWindowBinding,
    session: DockRuntimeDragSession,
    transport: DockNativeCapturedDragTransportLease,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockNativeCapturedDragTransportKey {
    epoch: u64,
    generation: NativeCapturedDragGeneration,
    runtime_identity: DockViewportRuntimeIdentity,
    source_binding: DockHostWindowBinding,
}

#[derive(Clone, Debug)]
pub(crate) struct DockNativeCapturedDragTransportLease {
    key: DockNativeCapturedDragTransportKey,
    active: Rc<Cell<bool>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DockNativeCapturedDragTransportRetirementReceipt {
    key: DockNativeCapturedDragTransportKey,
}

impl DockNativeCapturedDragTransportRetirementReceipt {
    pub(crate) const fn key(self) -> DockNativeCapturedDragTransportKey {
        self.key
    }
}

impl DockNativeCapturedDragTransportLease {
    fn new(key: DockNativeCapturedDragTransportKey) -> Self {
        Self {
            key,
            active: Rc::new(Cell::new(true)),
        }
    }

    pub(crate) const fn key(&self) -> DockNativeCapturedDragTransportKey {
        self.key
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.get()
    }

    pub(crate) fn retire(&self) -> DockNativeCapturedDragTransportRetirementReceipt {
        self.active.set(false);
        DockNativeCapturedDragTransportRetirementReceipt { key: self.key }
    }
}

impl DockNativeCapturedDragTransportKey {
    pub(crate) const fn runtime_identity(self) -> DockViewportRuntimeIdentity {
        self.runtime_identity
    }

    pub(crate) const fn source_binding(self) -> DockHostWindowBinding {
        self.source_binding
    }

    pub(crate) const fn source_window(self) -> WindowId {
        self.source_binding.window_id()
    }
}

impl DockNativeCapturedDragRouteReceipt {
    pub(crate) const fn transport_key(&self) -> DockNativeCapturedDragTransportKey {
        DockNativeCapturedDragTransportKey {
            epoch: self.epoch,
            generation: self.generation,
            runtime_identity: self.runtime_identity,
            source_binding: self.source_binding,
        }
    }

    pub(crate) fn transport_lease(&self) -> DockNativeCapturedDragTransportLease {
        self.transport.clone()
    }
}

#[derive(Clone)]
struct DockNativeCapturedDragRoute {
    epoch: u64,
    generation: NativeCapturedDragGeneration,
    runtime_identity: DockViewportRuntimeIdentity,
    runtime: DockViewportRuntimeHandle,
    work_context: DockViewportRuntimeWorkContext,
    session: DockRuntimeDragSession,
    payload: DockDragPayload,
    source_window: WindowId,
    source_window_handle: AnyWindowHandle,
    source_feedback_window_position: Point<Pixels>,
    source_host: WeakEntity<DockHost>,
    source_binding: DockHostWindowBinding,
    source_focus: Option<DockLiveUndockSourceFocusSnapshot>,
    transport: DockNativeCapturedDragTransportLease,
    native_drag_start_snapshot: Option<PlatformNativeDragStartSnapshot>,
    live_undock_source_scene: Option<DockNativeCapturedHostScene>,
    live_undock_identity: Rc<Cell<Option<DockLiveUndockIdentity>>>,
    payload_finalizer: DockPayloadDragFinalizer,
    start_consumer: PreparedNativeCapturedDragConsumer,
    foreign_previews: Rc<RefCell<DockNativeCapturedForeignPreviewState>>,
    latest_sequence: Rc<Cell<Option<NativeIngressSequence>>>,
    latest_event: Rc<RefCell<Option<NativeCapturedDragEvent>>>,
    preview_refresh_scheduled: Rc<Cell<bool>>,
    published_scene_frames: Rc<RefCell<Vec<DockNativeCapturedScenePublication>>>,
}

#[derive(Clone)]
struct DockNativeCapturedScenePublication {
    scene: DockNativeCapturedHostScene,
}

impl DockNativeCapturedScenePublication {
    fn has_same_owner(&self, scene: &DockNativeCapturedHostScene) -> bool {
        self.scene.window_id == scene.window_id
            && self.scene.host == scene.host
            && self.scene.host_binding == scene.host_binding
    }

    fn has_same_routing_scene(&self, scene: &DockNativeCapturedHostScene) -> bool {
        self.has_same_owner(scene)
            && self.scene.runtime_identity == scene.runtime_identity
            && self.scene.work_context == scene.work_context
            && self.scene.space == scene.space
            && self
                .scene
                .routing_scene
                .has_same_native_routing_content(&scene.routing_scene)
            && self.scene.presentation_scene == scene.presentation_scene
    }
}

#[derive(Clone)]
pub(crate) struct DockNativeCapturedHostScene {
    window_id: WindowId,
    host: WeakEntity<DockHost>,
    host_binding: DockHostWindowBinding,
    runtime_identity: DockViewportRuntimeIdentity,
    runtime: DockViewportRuntimeHandle,
    work_context: DockViewportRuntimeWorkContext,
    space: DockSpaceId,
    frame: DockViewportHostSceneFrame,
    geometry: DockViewportHostGeometry,
    routing_scene: DockViewportHostSceneDraft,
    presentation_scene: DockPresentationScene,
}

#[derive(Clone)]
struct DockNativeCapturedHostTarget {
    scene: DockNativeCapturedHostScene,
    window_position: Point<Pixels>,
    host_position: Point<Pixels>,
}

#[derive(Clone)]
struct DockNativeCapturedForeignPreview {
    runtime: DockViewportRuntimeHandle,
    owner: DockViewportRoutedPreviewOwner,
    window_id: WindowId,
    host: WeakEntity<DockHost>,
    host_binding: DockHostWindowBinding,
    frame: DockViewportHostSceneFrame,
}

#[derive(Default)]
struct DockNativeCapturedForeignPreviewState {
    current: Option<DockNativeCapturedForeignPreview>,
    pending_cleanup: Vec<DockNativeCapturedForeignPreview>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockNativeCapturedSurfaceReleaseOutcome {
    Released,
    Failed,
}

impl DockNativeCapturedSurfaceReleaseOutcome {
    fn from_native_terminal(terminal: NativeCapturedDragReleaseTerminal) -> Self {
        match terminal {
            NativeCapturedDragReleaseTerminal::Released
            | NativeCapturedDragReleaseTerminal::NativeWindowTerminal
            | NativeCapturedDragReleaseTerminal::NotRequired => Self::Released,
            NativeCapturedDragReleaseTerminal::Failed => Self::Failed,
        }
    }
}

impl DockNativeCapturedForeignPreview {
    fn same_projection(&self, other: &Self) -> bool {
        self.runtime.identity() == other.runtime.identity()
            && self.owner == other.owner
            && self.window_id == other.window_id
            && self.host == other.host
            && self.host_binding == other.host_binding
            && self.frame == other.frame
    }

    fn same_owner_runtime(&self, other: &Self) -> bool {
        self.runtime.identity() == other.runtime.identity() && self.owner == other.owner
    }
}

#[derive(Clone)]
enum DockNativeCapturedTarget {
    Host(DockNativeCapturedHostTarget),
    ForeignSurfaceTarget(DockNativeCapturedHostTarget),
    Desktop(DockNativeCapturedDesktopRoute),
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DockNativeCapturedDesktopRoute {
    OpenSpace,
    OpaqueBarrier,
}

struct DockNativeCapturedReleaseReservation {
    route_epoch: u64,
    generation: NativeCapturedDragGeneration,
    sequence: NativeIngressSequence,
    source_window: WindowId,
    runtime_identity: DockViewportRuntimeIdentity,
    work_context: DockViewportRuntimeWorkContext,
    session: DockRuntimeDragSession,
    payload: DockDragPayload,
    route: RefCell<Option<DockNativeCapturedDragRoute>>,
    target: RefCell<DockNativeCapturedTarget>,
    locked_drop: RefCell<Option<Result<DockViewportLockedDropRoute, crate::DockActionApplyError>>>,
    live_undock_release_adopted: Cell<bool>,
    resolution_panic: RefCell<Option<Box<dyn Any + Send>>>,
}

impl DockNativeCapturedReleaseReservation {
    fn matches_route(
        &self,
        route: &DockNativeCapturedDragRoute,
        event: &NativeCapturedDragEvent,
    ) -> bool {
        self.route_epoch == route.epoch
            && self.generation == route.generation
            && self.generation == event.generation()
            && self.sequence == event.sequence()
            && self.source_window == route.source_window
            && self.source_window == event.source_window()
            && self.runtime_identity == route.runtime_identity
            && self.work_context == route.work_context
            && self.session == route.session
            && self.payload == route.payload
            && event.payload::<DockDragPayload>() == Some(&self.payload)
    }

    fn target(&self) -> DockNativeCapturedTarget {
        self.target.borrow().clone()
    }

    fn take_resolution_panic(&self) -> Option<Box<dyn Any + Send>> {
        self.resolution_panic.borrow_mut().take()
    }
}

fn claim_locked_release_route(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    reservation: &Arc<DockNativeCapturedReleaseReservation>,
) -> Option<DockNativeCapturedRouteCleanup> {
    let route = reservation.route.borrow_mut().take()?;
    let mut state = state.borrow_mut();
    if state
        .locked_releases
        .get(&reservation.route_epoch)
        .is_some_and(|current| Arc::ptr_eq(current, reservation))
    {
        state.locked_releases.remove(&reservation.route_epoch);
    }
    drop(state);
    Some(DockNativeCapturedRouteCleanup {
        route,
        first_panic: reservation.take_resolution_panic(),
        locked_drop: reservation.locked_drop.borrow_mut().take(),
        live_undock_release_adopted: reservation.live_undock_release_adopted.get(),
    })
}

fn claim_locked_release_routes(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    mut matches: impl FnMut(&DockNativeCapturedDragRoute) -> bool,
) -> Vec<DockNativeCapturedRouteCleanup> {
    let reservations = state
        .borrow()
        .locked_releases
        .values()
        .cloned()
        .collect::<Vec<_>>();
    reservations
        .into_iter()
        .filter_map(|reservation| {
            let is_match = reservation
                .route
                .borrow()
                .as_ref()
                .is_some_and(&mut matches);
            is_match
                .then(|| claim_locked_release_route(state, &reservation))
                .flatten()
        })
        .collect()
}

pub(crate) fn ensure_native_captured_drag_router(cx: &mut App) {
    if cx.has_global::<DockNativeCapturedDragRouter>() {
        return;
    }
    let state = Rc::new(RefCell::new(DockNativeCapturedDragState::default()));
    let lock_state = Rc::downgrade(&state);
    let callback_state = Rc::downgrade(&state);
    let drag_subscription = cx.consume_native_captured_drag(
        move |event, source_window, cx| {
            lock_native_captured_release(&lock_state, event, source_window, cx)
        },
        move |event, cx| {
            let Some(state) = callback_state.upgrade() else {
                return;
            };
            consume_native_captured_drag_event(&state, event, cx);
        },
    );

    let callback_state = Rc::downgrade(&state);
    let window_closed_subscription = cx.on_window_closed(move |cx, window_id| {
        let Some(state) = callback_state.upgrade() else {
            return;
        };
        handle_native_captured_window_closed(&state, window_id, cx);
    });
    let callback_state = Rc::downgrade(&state);
    let window_native_terminal_subscription = cx.on_window_native_terminal(move |_, window_id| {
        let Some(state) = callback_state.upgrade() else {
            return;
        };
        clear_failed_native_captured_releases_for_source_window(&state, window_id);
    });
    cx.set_global(DockNativeCapturedDragRouter {
        state,
        _drag_subscription: drag_subscription,
        _window_closed_subscription: window_closed_subscription,
        _window_native_terminal_subscription: window_native_terminal_subscription,
    });
}

fn handle_native_captured_window_closed(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    window_id: WindowId,
    cx: &mut App,
) {
    let (source_route, target_preview, routed_previews) = {
        let mut state = state.borrow_mut();
        state.scenes.remove(&window_id);
        match state.active.as_ref() {
            None => (None, None, Vec::new()),
            Some(active) => {
                let routed_previews = active
                    .published_scene_frames
                    .borrow()
                    .iter()
                    .filter_map(|publication| {
                        (publication.scene.window_id == window_id
                            && publication.scene.runtime_identity == active.runtime_identity
                            && active.latest_event.borrow().as_ref().is_some_and(|event| {
                                native_event_may_target_scene(active, event, &publication.scene)
                            }))
                        .then(|| (active.runtime.clone(), publication.scene.frame.clone()))
                    })
                    .collect::<Vec<_>>();
                active
                    .published_scene_frames
                    .borrow_mut()
                    .retain(|publication| publication.scene.window_id != window_id);

                if active.source_window == window_id {
                    (state.active.take(), None, Vec::new())
                } else {
                    let target_preview = active
                        .foreign_previews
                        .borrow()
                        .current
                        .as_ref()
                        .filter(|preview| preview.window_id == window_id)
                        .cloned()
                        .map(|preview| (active.clone(), preview));
                    (None, target_preview, routed_previews)
                }
            }
        }
    };

    let locked_source_routes =
        claim_locked_release_routes(state, |route| route.source_window == window_id);
    if let Some(source_route) = source_route {
        schedule_route_retirement_with_reason(
            state,
            source_route,
            PointerCancelReason::WindowClosed,
            cx,
        );
    }
    for cleanup in locked_source_routes {
        schedule_route_cleanup_with_reason(state, cleanup, PointerCancelReason::WindowClosed, cx);
    }
    for (runtime, frame) in routed_previews {
        cx.defer(move |cx| {
            runtime.clear_routed_drop_preview_for_target_scene_frame(&frame, cx);
        });
    }
    if let Some((target_route, target_preview)) = target_preview {
        cx.defer(move |cx| clear_foreign_preview_if_matches(&target_route, &target_preview, cx));
    }
}

fn clear_failed_native_captured_releases_for_source_window(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    window_id: WindowId,
) {
    state
        .borrow_mut()
        .failed_releases
        .retain(|key| key.source_window != window_id);
}

fn router_state(cx: &App) -> Option<Rc<RefCell<DockNativeCapturedDragState>>> {
    cx.try_global::<DockNativeCapturedDragRouter>()
        .map(|router| router.state.clone())
}

#[cfg(test)]
pub(crate) fn panic_next_native_captured_drag_for_test(
    panic: DockNativeCapturedDragTestPanic,
    cx: &mut App,
) {
    ensure_native_captured_drag_router(cx);
    let state = router_state(cx).expect("installed Dock native captured-drag router must exist");
    state.borrow_mut().panic_next = Some(panic);
}

#[cfg(test)]
pub(crate) fn has_active_native_captured_drag_route_for_test(cx: &App) -> bool {
    router_state(cx).is_some_and(|state| state.borrow().active.is_some())
}

#[cfg(test)]
pub(crate) fn active_live_undock_route_facts_for_test(
    cx: &App,
) -> Option<(bool, bool, bool, bool, bool)> {
    let state = router_state(cx)?;
    let state = state.borrow();
    let route = state.active.as_ref()?;
    Some((
        route.native_drag_start_snapshot.is_some(),
        route.live_undock_source_scene.is_some(),
        route
            .runtime
            .active_payload_drag_tear_off_geometry(Some(&route.session))
            .is_some(),
        route
            .latest_event
            .borrow()
            .as_ref()
            .and_then(NativeCapturedDragEvent::physical_frame)
            .is_some(),
        route.live_undock_identity.get().is_some(),
    ))
}

#[cfg(test)]
pub(crate) fn has_failed_native_captured_release_for_surface_for_test(
    runtime_identity: DockViewportRuntimeIdentity,
    lease: crate::surface::window_session::DockSurfaceWindowSessionLease,
    cx: &App,
) -> bool {
    router_state(cx).is_some_and(|state| {
        state
            .borrow()
            .failed_releases
            .iter()
            .any(|key| key.belongs_to_surface(runtime_identity, lease))
    })
}

pub(crate) fn owns_native_captured_drag_source(
    runtime_identity: DockViewportRuntimeIdentity,
    session: Option<&DockRuntimeDragSession>,
    payload: &DockDragPayload,
    source_window: WindowId,
    source_host: &WeakEntity<DockHost>,
    source_binding: Option<DockHostWindowBinding>,
    cx: &App,
) -> bool {
    let Some(session) = session else {
        return false;
    };
    router_state(cx).is_some_and(|state| {
        let state = state.borrow();
        let owns_source = |route: &DockNativeCapturedDragRoute| {
            route.start_consumer.is_active()
                && route.runtime_identity == runtime_identity
                && &route.session == session
                && &route.payload == payload
                && route.source_window == source_window
                && route.source_host == *source_host
                && source_binding.is_some_and(|binding| route.source_binding == binding)
        };
        state.active.as_ref().is_some_and(&owns_source)
            || state.locked_releases.values().any(|reservation| {
                reservation
                    .route
                    .borrow()
                    .as_ref()
                    .is_some_and(&owns_source)
            })
    })
}

#[cfg(test)]
fn panic_native_captured_drag_if_requested(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    requested: DockNativeCapturedDragTestPanic,
) {
    let should_panic = {
        let mut state = state.borrow_mut();
        (state.panic_next == Some(requested)).then(|| state.panic_next.take())
    }
    .is_some();
    if should_panic {
        panic!("injected Dock native captured-drag failure at {requested:?}");
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn begin_native_captured_drag_route(
    runtime: DockViewportRuntimeHandle,
    work_context: DockViewportRuntimeWorkContext,
    session: DockRuntimeDragSession,
    payload: DockDragPayload,
    source_window_handle: AnyWindowHandle,
    source_host: WeakEntity<DockHost>,
    source_binding: DockHostWindowBinding,
    source_focus: Option<DockLiveUndockSourceFocusSnapshot>,
    drag_start: &DragStartGeometry,
    cx: &mut App,
) -> DockNativeCapturedDragRouteReceipt {
    let source_window = source_window_handle.window_id();
    assert!(
        session.accepts_payload(&payload),
        "native captured-drag route payload must match its runtime session"
    );
    assert_eq!(
        session.lineage(),
        work_context.lineage(),
        "native captured-drag route must use the runtime session lineage"
    );
    ensure_native_captured_drag_router(cx);
    let state = router_state(cx).expect("installed Dock native captured-drag router must exist");
    let runtime_identity = runtime.identity();
    let start_consumer = drag_start.prepare_native_captured_drag_consumer();
    let generation = start_consumer.generation();
    assert_eq!(
        drag_start.pointer_capture_handle().window_id(),
        source_window,
        "native captured-drag route must retain its source window's pointer owner"
    );
    debug_assert_eq!(generation, drag_start.native_captured_drag_generation());
    let epoch = {
        let mut state = state.borrow_mut();
        state.next_epoch = state
            .next_epoch
            .checked_add(1)
            .expect("Dock native captured-drag route epoch space exhausted");
        state.next_epoch
    };
    let published_scene_frames = state
        .borrow()
        .scenes
        .values()
        .flatten()
        .cloned()
        .map(|scene| DockNativeCapturedScenePublication { scene })
        .collect::<Vec<_>>();
    let live_undock_source_scene = published_scene_frames
        .iter()
        .find(|publication| {
            let scene = &publication.scene;
            scene.window_id == source_window
                && scene.host == source_host
                && scene.host_binding == source_binding
                && scene.runtime_identity == runtime_identity
                && scene.work_context == work_context
                && scene.space == payload.source_space
                && scene
                    .frame
                    .matches_viewport(&payload.source_space, source_window)
        })
        .map(|publication| publication.scene.clone());
    let transport = DockNativeCapturedDragTransportLease::new(DockNativeCapturedDragTransportKey {
        epoch,
        generation,
        runtime_identity,
        source_binding,
    });
    let route = DockNativeCapturedDragRoute {
        epoch,
        generation,
        runtime_identity,
        runtime,
        work_context,
        session: session.clone(),
        payload,
        source_window,
        source_window_handle,
        source_feedback_window_position: drag_start.window_position(),
        source_host,
        source_binding,
        source_focus,
        transport: transport.clone(),
        native_drag_start_snapshot: drag_start.native_drag_start_snapshot(),
        live_undock_source_scene,
        live_undock_identity: Rc::new(Cell::new(None)),
        payload_finalizer: DockPayloadDragFinalizer::new(),
        start_consumer,
        foreign_previews: Rc::new(RefCell::new(
            DockNativeCapturedForeignPreviewState::default(),
        )),
        latest_sequence: Rc::new(Cell::new(None)),
        latest_event: Rc::new(RefCell::new(None)),
        preview_refresh_scheduled: Rc::new(Cell::new(false)),
        published_scene_frames: Rc::new(RefCell::new(published_scene_frames)),
    };
    let displaced = state.borrow_mut().active.replace(route);
    let install = catch_unwind(AssertUnwindSafe(|| {
        #[cfg(test)]
        panic_native_captured_drag_if_requested(
            &state,
            DockNativeCapturedDragTestPanic::BeginRouteAfterInstall,
        );
        let cleanup_state = Rc::downgrade(&state);
        cx.defer(move |cx| {
            let Some(state) = cleanup_state.upgrade() else {
                return;
            };
            let revoked = {
                let mut state = state.borrow_mut();
                let is_revoked = state.active.as_ref().is_some_and(|active| {
                    active.epoch == epoch && active.start_consumer.is_revoked()
                });
                is_revoked.then(|| state.active.take()).flatten()
            };
            if let Some(revoked) = revoked {
                schedule_route_retirement(&state, revoked, cx);
            }
        });
    }));
    if install.is_err() {
        detach_native_captured_drag_route_start(&state, epoch, generation);
    }
    if let Some(displaced) = displaced {
        schedule_route_retirement(&state, displaced, cx);
    }
    if let Err(payload) = install {
        resume_unwind(payload);
    }
    DockNativeCapturedDragRouteReceipt {
        epoch,
        generation,
        runtime_identity,
        source_binding,
        session,
        transport,
    }
}

pub(crate) fn rollback_native_captured_drag_route_start(
    receipt: &DockNativeCapturedDragRouteReceipt,
    cx: &App,
) -> bool {
    let Some(state) = router_state(cx) else {
        return false;
    };
    let matches = {
        state.borrow().active.as_ref().is_some_and(|active| {
            active.epoch == receipt.epoch
                && active.generation == receipt.generation
                && active.runtime_identity == receipt.runtime_identity
                && active.session == receipt.session
        })
    };
    matches
        && detach_native_captured_drag_route_start(&state, receipt.epoch, receipt.generation)
            .is_some()
}

fn detach_native_captured_drag_route_start(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    epoch: u64,
    generation: NativeCapturedDragGeneration,
) -> Option<DockNativeCapturedDragRoute> {
    let mut state = state.borrow_mut();
    let matches = state
        .active
        .as_ref()
        .is_some_and(|active| active.epoch == epoch && active.generation == generation);
    matches.then(|| state.active.take()).flatten()
}

pub(crate) fn publish_native_captured_host_scene(scene: DockNativeCapturedHostScene, cx: &mut App) {
    ensure_native_captured_drag_router(cx);
    let Some(state) = router_state(cx) else {
        return;
    };
    let refresh_epoch = {
        let mut state = state.borrow_mut();
        let scenes = state.scenes.entry(scene.window_id).or_default();
        scenes.retain(|current| {
            current.host_binding != scene.host_binding || current.host != scene.host
        });
        scenes.push(scene.clone());
        state.active.as_mut().and_then(|active| {
            let scene_changed = {
                let mut published = active.published_scene_frames.borrow_mut();
                if let Some(current) = published
                    .iter_mut()
                    .find(|current| current.has_same_owner(&scene))
                {
                    let changed = !current.has_same_routing_scene(&scene);
                    current.scene = scene.clone();
                    changed
                } else {
                    published.push(DockNativeCapturedScenePublication {
                        scene: scene.clone(),
                    });
                    true
                }
            };
            if let Some(preview) = active.foreign_previews.borrow_mut().current.as_mut()
                && preview.window_id == scene.window_id
                && preview.host == scene.host
                && preview.host_binding == scene.host_binding
            {
                preview.frame = scene.frame.clone();
            }
            let event = active.latest_event.borrow();
            let targets_scene = event
                .as_ref()
                .is_some_and(|event| native_event_may_target_scene(active, event, &scene));
            let supports_source_route = scene_supports_source_route(&scene, active);
            if supports_source_route && active.live_undock_identity.get().is_none() {
                active.live_undock_source_scene = Some(scene.clone());
            }
            let should_refresh = scene_changed
                && (targets_scene || supports_source_route)
                && active.start_consumer.is_active()
                && !active.preview_refresh_scheduled.replace(true);
            if should_refresh {
                Some(active.epoch)
            } else {
                None
            }
        })
    };
    if let Some(epoch) = refresh_epoch {
        let state = Rc::downgrade(&state);
        cx.defer(move |cx| {
            let Some(state) = state.upgrade() else {
                return;
            };
            refresh_native_captured_preview_after_scene_commit(&state, epoch, cx);
        });
    }
}

pub(crate) fn clear_native_captured_host_scene(
    window_id: WindowId,
    host: &WeakEntity<DockHost>,
    host_binding: DockHostWindowBinding,
    expected_frame: Option<&DockViewportHostSceneFrame>,
    cx: &mut App,
) {
    let Some(state) = router_state(cx) else {
        return;
    };
    let (source_route, target_preview, routed_previews) = {
        let mut state = state.borrow_mut();
        let active = state.active.clone();
        let mut removed_source_scene = false;
        let mut removed_scenes = Vec::new();
        let remove_window_scenes = state.scenes.get_mut(&window_id).is_some_and(|scenes| {
            scenes.retain(|scene| {
                let exact_host = scene.host == *host && scene.host_binding == host_binding;
                let exact_frame = expected_frame.is_none_or(|frame| &scene.frame == frame);
                let remove = exact_host && exact_frame;
                if remove {
                    removed_source_scene |= active
                        .as_ref()
                        .is_some_and(|route| scene_supports_source_route(scene, route));
                    removed_scenes.push(scene.clone());
                }
                !remove
            });
            scenes.is_empty()
        });
        if remove_window_scenes {
            state.scenes.remove(&window_id);
        }
        let replacement_supports_source_route = active.as_ref().is_some_and(|route| {
            state
                .scenes
                .get(&route.source_window)
                .is_some_and(|scenes| {
                    scenes
                        .iter()
                        .any(|scene| scene_supports_source_route(scene, route))
                })
        });
        let retire_source_route = removed_source_scene
            && !replacement_supports_source_route
            && active
                .as_ref()
                .is_some_and(|route| route.live_undock_identity.get().is_none());
        if let Some(active) = active.as_ref() {
            active
                .published_scene_frames
                .borrow_mut()
                .retain(|publication| {
                    !removed_scenes.iter().any(|removed| {
                        publication.has_same_owner(removed)
                            && publication.scene.frame == removed.frame
                    })
                });
        }
        let target_preview = state.active.as_ref().and_then(|active| {
            active
                .foreign_previews
                .borrow()
                .current
                .as_ref()
                .filter(|preview| {
                    preview.window_id == window_id
                        && preview.host == *host
                        && preview.host_binding == host_binding
                        && expected_frame.is_none_or(|frame| &preview.frame == frame)
                })
                .map(|preview| (active.clone(), preview.clone()))
        });
        let routed_previews = state.active.as_ref().map_or_else(Vec::new, |active| {
            let latest_event = active.latest_event.borrow();
            removed_scenes
                .iter()
                .filter_map(|scene| {
                    (scene.runtime_identity == active.runtime_identity
                        && latest_event.as_ref().is_some_and(|event| {
                            native_event_may_target_scene(active, event, scene)
                        }))
                    .then(|| (active.runtime.clone(), scene.frame.clone()))
                })
                .collect()
        });
        (
            retire_source_route.then(|| state.active.take()).flatten(),
            target_preview,
            routed_previews,
        )
    };
    if let Some(source_route) = source_route {
        schedule_route_retirement(&state, source_route, cx);
        return;
    }
    for (runtime, frame) in routed_previews {
        cx.defer(move |cx| {
            runtime.clear_routed_drop_preview_for_target_scene_frame(&frame, cx);
        });
    }
    if let Some((target_route, target_preview)) = target_preview {
        cx.defer(move |cx| clear_foreign_preview_if_matches(&target_route, &target_preview, cx));
    }
}

fn scene_supports_source_route(
    scene: &DockNativeCapturedHostScene,
    route: &DockNativeCapturedDragRoute,
) -> bool {
    scene.window_id == route.source_window
        && scene.host == route.source_host
        && scene.host_binding == route.source_binding
        && scene.runtime_identity == route.runtime_identity
        && scene.work_context == route.work_context
        && scene.space == route.payload.source_space
        && scene
            .frame
            .matches_viewport(&route.payload.source_space, route.source_window)
}

pub(crate) fn cancel_native_captured_drag_route(
    runtime_identity: DockViewportRuntimeIdentity,
    session: Option<&DockRuntimeDragSession>,
    payload: Option<&DockDragPayload>,
    source_host: &WeakEntity<DockHost>,
    source_binding: Option<DockHostWindowBinding>,
    reason: PointerCancelReason,
    cx: &mut App,
) {
    let Some(state) = router_state(cx) else {
        return;
    };
    let route = {
        let mut state = state.borrow_mut();
        let matches = state.active.as_ref().is_some_and(|active| {
            active.runtime_identity == runtime_identity
                && active.source_host == *source_host
                && source_binding.is_none_or(|binding| active.source_binding == binding)
                && session.is_none_or(|session| &active.session == session)
                && payload.is_none_or(|payload| &active.payload == payload)
        });
        matches.then(|| state.active.take()).flatten()
    };
    if let Some(route) = route {
        schedule_route_retirement_with_reason(&state, route, reason, cx);
    }
}

pub(crate) fn cancel_native_captured_drag_route_for_surface(
    runtime_identity: DockViewportRuntimeIdentity,
    lease: crate::surface::window_session::DockSurfaceWindowSessionLease,
    on_native_capture_terminal: impl FnOnce(
        DockNativeCapturedSurfaceReleaseOutcome,
        &mut Option<Box<dyn Any + Send + 'static>>,
        &mut App,
    ) + 'static,
    cx: &mut App,
) {
    let Some(state) = router_state(cx) else {
        defer_native_captured_surface_terminal(
            DockNativeCapturedSurfaceReleaseOutcome::Released,
            Box::new(on_native_capture_terminal),
            cx,
        );
        return;
    };
    let (active, retired_pending, release_failed) = {
        let mut state = state.borrow_mut();
        let matches = state.active.as_ref().is_some_and(|active| {
            active.runtime_identity == runtime_identity
                && active.work_context.lineage()
                    == crate::DockViewportRuntimeLineage::Surface(lease)
        });
        let active = matches.then(|| state.active.take()).flatten();
        let retired_pending = state
            .retired_pending
            .iter_mut()
            .filter_map(|(key, pending)| {
                key.belongs_to_surface(runtime_identity, lease).then_some((
                    *key,
                    pending.barrier,
                    pending.cleanup.take(),
                ))
            })
            .collect::<Vec<_>>();
        let failed_release_keys = state
            .failed_releases
            .iter()
            .filter(|key| key.belongs_to_surface(runtime_identity, lease))
            .copied()
            .collect::<Vec<_>>();
        let release_failed = !failed_release_keys.is_empty();
        for key in failed_release_keys {
            state.failed_releases.remove(&key);
        }
        (active, retired_pending, release_failed)
    };
    let locked = claim_locked_release_routes(&state, |route| {
        route.runtime_identity == runtime_identity
            && route.work_context.lineage() == crate::DockViewportRuntimeLineage::Surface(lease)
    });
    let pending_count = usize::from(active.is_some()) + locked.len() + retired_pending.len();
    if pending_count == 0 {
        defer_native_captured_surface_terminal(
            if release_failed {
                DockNativeCapturedSurfaceReleaseOutcome::Failed
            } else {
                DockNativeCapturedSurfaceReleaseOutcome::Released
            },
            Box::new(on_native_capture_terminal),
            cx,
        );
        return;
    }

    let completion = Rc::new(RefCell::new(DockNativeCapturedSurfaceCancellation {
        remaining: pending_count,
        on_native_capture_terminal: Some(Box::new(on_native_capture_terminal)),
        release_failed,
        first_panic: None,
    }));
    if let Some(route) = active {
        attach_active_surface_route_release(
            &state,
            DockNativeCapturedRouteCleanup {
                route,
                first_panic: None,
                locked_drop: None,
                live_undock_release_adopted: false,
            },
            completion.clone(),
            cx,
        );
    }
    for cleanup in locked {
        attach_active_surface_route_release(&state, cleanup, completion.clone(), cx);
    }
    for (key, barrier, cleanup) in retired_pending {
        attach_retired_surface_route_release(&state, key, barrier, cleanup, completion.clone(), cx);
    }
}

type DockNativeCapturedSurfaceTerminal = Box<
    dyn FnOnce(
        DockNativeCapturedSurfaceReleaseOutcome,
        &mut Option<Box<dyn Any + Send + 'static>>,
        &mut App,
    ),
>;

struct DockNativeCapturedSurfaceCancellation {
    remaining: usize,
    on_native_capture_terminal: Option<DockNativeCapturedSurfaceTerminal>,
    release_failed: bool,
    first_panic: Option<Box<dyn Any + Send + 'static>>,
}

fn defer_native_captured_surface_terminal(
    release_outcome: DockNativeCapturedSurfaceReleaseOutcome,
    on_native_capture_terminal: DockNativeCapturedSurfaceTerminal,
    cx: &mut App,
) {
    cx.defer_shutdown_critical_before_window_registry_clear(move |cx| {
        invoke_native_captured_surface_terminal(
            on_native_capture_terminal,
            release_outcome,
            None,
            cx,
        );
    });
}

fn invoke_native_captured_surface_terminal(
    on_native_capture_terminal: DockNativeCapturedSurfaceTerminal,
    release_outcome: DockNativeCapturedSurfaceReleaseOutcome,
    mut first_panic: Option<Box<dyn Any + Send + 'static>>,
    cx: &mut App,
) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
        on_native_capture_terminal(release_outcome, &mut first_panic, cx)
    })) {
        if first_panic.is_none() {
            first_panic = Some(payload);
        } else {
            log::error!(
                "suppressed a DockSurface capture-terminal panic after an earlier shutdown panic"
            );
        }
    }
    if let Some(payload) = first_panic {
        resume_unwind(payload);
    }
}

fn attach_active_surface_route_release(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    cleanup: DockNativeCapturedRouteCleanup,
    completion: Rc<RefCell<DockNativeCapturedSurfaceCancellation>>,
    cx: &mut App,
) {
    retire_route_transport_proxy(&cleanup.route);
    cancel_live_undock_route(&cleanup.route, PointerCancelReason::WindowClosed, cx);
    let key = DockNativeCapturedDragRetiredKey::for_route(&cleanup.route)
        .expect("a surface shutdown route must carry its exact surface lease");
    let pending_cleanup = Rc::new(RefCell::new(Some(cleanup)));
    let terminal_cleanup = pending_cleanup.clone();
    let terminal_state = Rc::downgrade(state);
    let terminal_completion = completion.clone();
    let barrier = cx.cancel_native_captured_drag_with_release_barrier(
        key.source_window,
        key.drag_generation,
        PointerCancelReason::WindowClosed,
        move |barrier, terminal, cx| {
            if let Some(state) = terminal_state.upgrade() {
                let _ = clear_retired_pending_if_matches(&state, key, barrier);
            }
            let cleanup = terminal_cleanup
                .borrow_mut()
                .take()
                .expect("one captured route release must settle exactly once");
            finish_surface_route_cancellation(
                Some(cleanup),
                DockNativeCapturedSurfaceReleaseOutcome::from_native_terminal(terminal),
                terminal_completion,
                cx,
            );
        },
    );
    if let Some(barrier) = barrier {
        insert_retired_pending(state, key, barrier, None);
        return;
    }

    let cleanup = pending_cleanup
        .borrow_mut()
        .take()
        .expect("an unreserved captured route must remain available for cleanup");
    cx.defer_shutdown_critical_before_window_registry_clear(move |cx| {
        finish_surface_route_cancellation(
            Some(cleanup),
            DockNativeCapturedSurfaceReleaseOutcome::Released,
            completion,
            cx,
        )
    });
}

fn attach_retired_surface_route_release(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    key: DockNativeCapturedDragRetiredKey,
    expected_barrier: NativeCapturedDragReleaseBarrier,
    cleanup: Option<DockNativeCapturedRouteCleanup>,
    completion: Rc<RefCell<DockNativeCapturedSurfaceCancellation>>,
    cx: &mut App,
) {
    let pending_cleanup = Rc::new(RefCell::new(Some(cleanup)));
    let terminal_cleanup = pending_cleanup.clone();
    let terminal_state = Rc::downgrade(state);
    let terminal_completion = completion.clone();
    let barrier = cx.cancel_native_captured_drag_with_release_barrier(
        key.source_window,
        key.drag_generation,
        PointerCancelReason::WindowClosed,
        move |barrier, terminal, cx| {
            if let Some(state) = terminal_state.upgrade() {
                let _ = clear_retired_pending_if_matches(&state, key, barrier);
                state.borrow_mut().failed_releases.remove(&key);
            }
            let cleanup = terminal_cleanup
                .borrow_mut()
                .take()
                .expect("one retired capture release must settle exactly once");
            finish_surface_route_cancellation(
                cleanup,
                DockNativeCapturedSurfaceReleaseOutcome::from_native_terminal(terminal),
                terminal_completion,
                cx,
            );
        },
    );
    if let Some(barrier) = barrier {
        debug_assert_eq!(
            barrier, expected_barrier,
            "one retired Dock route generation must attach to its original release barrier"
        );
        return;
    }

    let _ = clear_retired_pending_if_matches(state, key, expected_barrier);
    let cleanup = pending_cleanup
        .borrow_mut()
        .take()
        .expect("an already-terminal retired capture must remain available for cleanup");
    cx.defer_shutdown_critical_before_window_registry_clear(move |cx| {
        finish_surface_route_cancellation(
            cleanup,
            DockNativeCapturedSurfaceReleaseOutcome::Failed,
            completion,
            cx,
        )
    });
}

fn finish_surface_route_cancellation(
    cleanup: Option<DockNativeCapturedRouteCleanup>,
    release_outcome: DockNativeCapturedSurfaceReleaseOutcome,
    completion: Rc<RefCell<DockNativeCapturedSurfaceCancellation>>,
    cx: &mut App,
) {
    let route_panic = cleanup.and_then(|cleanup| finish_route_cleanup(cleanup, cx));
    let terminal = {
        let mut completion = completion.borrow_mut();
        if completion.first_panic.is_none() {
            completion.first_panic = route_panic;
        } else if route_panic.is_some() {
            log::error!(
                "suppressed a Dock route-retirement panic while awaiting surface capture terminals"
            );
        }
        completion.release_failed |=
            release_outcome == DockNativeCapturedSurfaceReleaseOutcome::Failed;
        completion.remaining = completion
            .remaining
            .checked_sub(1)
            .expect("a surface capture-release candidate must settle exactly once");
        (completion.remaining == 0).then(|| {
            (
                completion
                    .on_native_capture_terminal
                    .take()
                    .expect("surface capture-terminal continuation must run exactly once"),
                if completion.release_failed {
                    DockNativeCapturedSurfaceReleaseOutcome::Failed
                } else {
                    DockNativeCapturedSurfaceReleaseOutcome::Released
                },
                completion.first_panic.take(),
            )
        })
    };
    let Some((on_native_capture_terminal, release_outcome, first_panic)) = terminal else {
        return;
    };
    invoke_native_captured_surface_terminal(
        on_native_capture_terminal,
        release_outcome,
        first_panic,
        cx,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn native_captured_host_scene(
    window_id: WindowId,
    host: WeakEntity<DockHost>,
    host_binding: DockHostWindowBinding,
    runtime: &DockViewportRuntimeHandle,
    work_context: DockViewportRuntimeWorkContext,
    space: DockSpaceId,
    frame: DockViewportHostSceneFrame,
    routing_scene: DockViewportHostSceneDraft,
    presentation_scene: DockPresentationScene,
) -> DockNativeCapturedHostScene {
    let geometry = routing_scene.host_geometry.clone();
    DockNativeCapturedHostScene {
        window_id,
        host,
        host_binding,
        runtime_identity: runtime.identity(),
        runtime: runtime.clone(),
        work_context,
        space,
        frame,
        geometry,
        routing_scene,
        presentation_scene,
    }
}

fn lock_native_captured_release(
    state: &std::rc::Weak<RefCell<DockNativeCapturedDragState>>,
    event: &NativeCapturedDragEvent,
    source_window: &mut Window,
    cx: &mut App,
) -> Arc<dyn Any> {
    if event.phase() != NativeCapturedDragPhase::Released {
        return Arc::new(());
    }
    let Some(state) = state.upgrade() else {
        return Arc::new(());
    };
    let (route, reservation) = {
        let mut state = state.borrow_mut();
        let matches = state.active.as_ref().is_some_and(|active| {
            active.start_consumer.is_active()
                && active.generation == event.generation()
                && active.source_window == event.source_window()
                && active.session.accepts_payload(&active.payload)
                && event.payload::<DockDragPayload>() == Some(&active.payload)
                && active
                    .latest_sequence
                    .get()
                    .is_none_or(|sequence| sequence < event.sequence())
        });
        if !matches {
            return Arc::new(());
        }
        let route = state
            .active
            .take()
            .expect("validated release route must remain active");
        route.latest_sequence.set(Some(event.sequence()));
        let route_snapshot = route.clone();
        let reservation = Arc::new(DockNativeCapturedReleaseReservation {
            route_epoch: route.epoch,
            generation: route.generation,
            sequence: event.sequence(),
            source_window: route.source_window,
            runtime_identity: route.runtime_identity,
            work_context: route.work_context,
            session: route.session.clone(),
            payload: route.payload.clone(),
            route: RefCell::new(Some(route)),
            target: RefCell::new(DockNativeCapturedTarget::Unavailable),
            locked_drop: RefCell::new(None),
            live_undock_release_adopted: Cell::new(false),
            resolution_panic: RefCell::new(None),
        });
        let replaced = state
            .locked_releases
            .insert(route_snapshot.epoch, reservation.clone());
        debug_assert!(
            replaced.is_none(),
            "one Dock route epoch must own at most one locked release"
        );
        (route_snapshot, reservation)
    };
    let resolution = catch_unwind(AssertUnwindSafe(|| {
        let target = if route.runtime.admits_work_context(route.work_context)
            && route.runtime.active_payload_drag_session(&route.payload)
                == Some(route.session.clone())
        {
            resolve_native_captured_target_with_source_window(
                &state,
                &route,
                event,
                Some(source_window),
                cx,
            )
        } else {
            DockNativeCapturedTarget::Unavailable
        };
        let mut locked_drop =
            lock_native_captured_drop(&route, event, &target, source_window.window_handle(), cx);
        let live_undock_release_adopted =
            lock_live_undock_release(&route, event, &target, &mut locked_drop, cx);
        (target, locked_drop, live_undock_release_adopted)
    }));
    match resolution {
        Ok((target, locked_drop, live_undock_release_adopted)) => {
            reservation.target.replace(target);
            reservation.locked_drop.replace(locked_drop);
            reservation
                .live_undock_release_adopted
                .set(live_undock_release_adopted);
        }
        Err(payload) => {
            reservation.resolution_panic.replace(Some(payload));
            reservation
                .target
                .replace(DockNativeCapturedTarget::Unavailable);
        }
    };
    reservation
}

fn lock_native_captured_drop(
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    target: &DockNativeCapturedTarget,
    source_window: AnyWindowHandle,
    cx: &mut App,
) -> Option<Result<DockViewportLockedDropRoute, crate::DockActionApplyError>> {
    let captured_route = match target {
        DockNativeCapturedTarget::Host(target) => captured_host_target(route, event, target)
            .map(DockCapturedNativeDropRoute::Host)
            .unwrap_or(DockCapturedNativeDropRoute::Unavailable),
        DockNativeCapturedTarget::Desktop(_) => DockCapturedNativeDropRoute::Desktop,
        DockNativeCapturedTarget::ForeignSurfaceTarget(_)
        | DockNativeCapturedTarget::Unavailable => return None,
    };
    let request = native_route_request(route, event, captured_route, cx);
    Some(
        route
            .runtime
            .lock_payload_drop_from_screen(&request, source_window, cx),
    )
}

fn consume_native_captured_drag_event(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    event: NativeCapturedDragEvent,
    cx: &mut App,
) {
    let terminal = !matches!(event.phase(), NativeCapturedDragPhase::Moved);
    let (route, mut terminal_cleanup) =
        if matches!(event.phase(), NativeCapturedDragPhase::Released) {
            let reservation = {
                let locked = event.route_lock::<DockNativeCapturedReleaseReservation>();
                let Some(locked) = locked else {
                    return;
                };
                if locked.generation != event.generation()
                    || locked.sequence != event.sequence()
                    || locked.source_window != event.source_window()
                    || event.payload::<DockDragPayload>() != Some(&locked.payload)
                {
                    return;
                }
                state
                    .borrow()
                    .locked_releases
                    .get(&locked.route_epoch)
                    .filter(|reservation| std::ptr::eq(reservation.as_ref(), locked))
                    .cloned()
            };
            let Some(reservation) = reservation else {
                return;
            };
            let Some(cleanup) = claim_locked_release_route(state, &reservation) else {
                return;
            };
            if !reservation.matches_route(&cleanup.route, &event) {
                schedule_route_cleanup(cleanup, cx);
                return;
            }
            (cleanup.route.clone(), Some(cleanup))
        } else {
            let mut state = state.borrow_mut();
            let Some(active) = state.active.as_ref() else {
                return;
            };
            if !active.start_consumer.is_active() {
                return;
            }
            if active.generation != event.generation()
                || active.source_window != event.source_window()
                || !active.session.accepts_payload(&active.payload)
                || event.payload::<DockDragPayload>() != Some(&active.payload)
                || active
                    .latest_sequence
                    .get()
                    .is_some_and(|sequence| sequence >= event.sequence())
            {
                return;
            }
            if terminal {
                let active = state
                    .active
                    .take()
                    .expect("validated terminal Dock route must remain active");
                active.latest_sequence.set(Some(event.sequence()));
                let route = active.clone();
                (
                    route,
                    Some(DockNativeCapturedRouteCleanup {
                        route: active,
                        first_panic: None,
                        locked_drop: None,
                        live_undock_release_adopted: false,
                    }),
                )
            } else {
                let active = state
                    .active
                    .as_mut()
                    .expect("validated moving Dock route must remain active");
                active.latest_sequence.set(Some(event.sequence()));
                active.latest_event.replace(Some(event.clone()));
                (active.clone(), None)
            }
        };

    let mut resolution_panic = terminal_cleanup
        .as_mut()
        .and_then(|cleanup| cleanup.first_panic.take());
    let mut locked_drop = terminal_cleanup
        .as_mut()
        .and_then(|cleanup| cleanup.locked_drop.take());
    let live_undock_release_adopted = terminal_cleanup
        .as_ref()
        .is_some_and(|cleanup| cleanup.live_undock_release_adopted);

    let result = catch_unwind(AssertUnwindSafe(|| {
        if let Some(payload) = resolution_panic.take() {
            resume_unwind(payload);
        }
        let work_context_admitted = route.runtime.admits_work_context(route.work_context);
        let session_current = route.runtime.active_payload_drag_session(&route.payload)
            == Some(route.session.clone());
        if !work_context_admitted || !session_current {
            return false;
        }

        if !terminal {
            let target = resolve_native_captured_target(state, &route, &event, cx);
            update_native_captured_preview(state, &route, &event, target.clone(), cx);
            update_live_undock_move(&route, &event, &target, cx);
            return true;
        }

        match event.phase() {
            NativeCapturedDragPhase::Released => {
                if !live_undock_release_adopted {
                    let target = locked_native_captured_release_target(state, &route, &event, cx);
                    commit_native_captured_release(&route, &event, target, locked_drop.take(), cx);
                }
            }
            NativeCapturedDragPhase::Cancelled(reason) => {
                cancel_live_undock_route(&route, reason, cx);
            }
            NativeCapturedDragPhase::Moved => unreachable!("moving route was handled above"),
        }
        true
    }));

    if terminal {
        let mut cleanup = terminal_cleanup
            .take()
            .expect("one terminal Dock route must retain its cleanup authority");
        let cleanup_panic = match result {
            Ok(true) => finish_route_cleanup(cleanup, cx),
            Ok(false) => finish_route_cleanup_with_cancel(
                cleanup,
                terminal_live_undock_cancel_reason(event.phase()),
                cx,
            ),
            Err(payload) => {
                cleanup.first_panic = Some(payload);
                finish_route_cleanup_with_cancel(
                    cleanup,
                    terminal_live_undock_cancel_reason(event.phase()),
                    cx,
                )
            }
        };
        if let Some(payload) = cleanup_panic {
            resume_unwind(payload);
        }
        return;
    }

    match result {
        Ok(true) => {}
        Ok(false) => finish_matching_route(state, route.epoch, cx),
        Err(payload) => {
            let _cleanup_panic = finish_matching_route_cleanup(state, route.epoch, cx);
            resume_unwind(payload);
        }
    }
}

fn locked_native_captured_release_target(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    cx: &mut App,
) -> DockNativeCapturedTarget {
    let Some(reservation) = event.route_lock::<DockNativeCapturedReleaseReservation>() else {
        return DockNativeCapturedTarget::Unavailable;
    };
    if !reservation.matches_route(route, event) {
        return DockNativeCapturedTarget::Unavailable;
    }
    match reservation.target() {
        DockNativeCapturedTarget::Host(target) => {
            current_locked_native_captured_host_target(state, route, event, &target, cx)
                .map(DockNativeCapturedTarget::Host)
                .unwrap_or(DockNativeCapturedTarget::Unavailable)
        }
        DockNativeCapturedTarget::ForeignSurfaceTarget(target) => {
            current_locked_native_captured_host_target(state, route, event, &target, cx)
                .map(DockNativeCapturedTarget::ForeignSurfaceTarget)
                .unwrap_or(DockNativeCapturedTarget::Unavailable)
        }
        DockNativeCapturedTarget::Desktop(desktop) => DockNativeCapturedTarget::Desktop(desktop),
        DockNativeCapturedTarget::Unavailable => DockNativeCapturedTarget::Unavailable,
    }
}

fn current_locked_native_captured_host_target(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    target: &DockNativeCapturedHostTarget,
    cx: &mut App,
) -> Option<DockNativeCapturedHostTarget> {
    let scenes = state
        .borrow()
        .scenes
        .get(&target.scene.window_id)
        .cloned()?;
    let target_window = native_captured_registered_window(route, event)?;
    if target_window.window_id() != target.scene.window_id {
        return None;
    }
    let current = cx
        .update_window(target_window, |_, window, cx| {
            select_frontmost_host_scene(scenes, target.window_position, window, cx)
        })
        .ok()
        .flatten()?;
    if !same_locked_native_captured_host_candidate(&current.scene, &target.scene) {
        return None;
    }
    if !current
        .scene
        .runtime
        .is_current_viewport_host_scene_frame(&current.scene.frame)
    {
        return None;
    }
    let host = current.scene.host.upgrade()?;
    let accepted = host.update(cx, |host, host_cx| {
        host.accepts_viewport_scene_candidate(
            current.scene.host_binding,
            Some(current.scene.frame.registration_key()),
            current.scene.work_context,
            current.scene.window_id,
            host_cx,
        ) && host.viewport_runtime().identity() == current.scene.runtime_identity
    });
    if !accepted {
        return None;
    }
    Some(current)
}

fn same_locked_native_captured_host_candidate(
    current: &DockNativeCapturedHostScene,
    locked: &DockNativeCapturedHostScene,
) -> bool {
    current.host == locked.host
        && current.host_binding == locked.host_binding
        && current.runtime_identity == locked.runtime_identity
        && current.work_context == locked.work_context
        && current.space == locked.space
        && current.frame.registration_key() == locked.frame.registration_key()
        && current
            .routing_scene
            .has_same_native_routing_content(&locked.routing_scene)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DockNativeCapturedWindowHit {
    OpenDesktop,
    OpaqueBarrier,
    RegisteredApplication {
        window: AnyWindowHandle,
        window_position: Point<Pixels>,
    },
    Unavailable,
}

fn classify_native_captured_window_hit(
    stack: &PlatformWindowHitStack,
    global_position: Point<open_gpui::DevicePixels>,
    mut admits_provisional: impl FnMut(AnyWindowHandle, u64) -> bool,
) -> DockNativeCapturedWindowHit {
    let observation = match stack {
        PlatformWindowHitStack::Available(observation)
            if observation.sampled_point() == global_position =>
        {
            observation
        }
        PlatformWindowHitStack::Available(_) | PlatformWindowHitStack::Unavailable => {
            return DockNativeCapturedWindowHit::Unavailable;
        }
    };
    let Some((terminal, prefix)) = observation.hits().split_last() else {
        return DockNativeCapturedWindowHit::OpenDesktop;
    };
    if !prefix.iter().all(|hit| {
        matches!(
            *hit,
            PlatformWindowHit::ProvisionalPassThrough {
                window,
                session_generation,
                coverage,
                ..
            } if coverage.contains(global_position)
                && admits_provisional(window, session_generation)
        )
    }) {
        return DockNativeCapturedWindowHit::Unavailable;
    }
    match *terminal {
        PlatformWindowHit::OpaqueBarrier { coverage } => {
            if coverage.contains(global_position) {
                DockNativeCapturedWindowHit::OpaqueBarrier
            } else {
                DockNativeCapturedWindowHit::Unavailable
            }
        }
        PlatformWindowHit::RegisteredApplication {
            window,
            coverage,
            geometry,
        } => {
            if !coverage.contains(global_position) {
                return DockNativeCapturedWindowHit::Unavailable;
            }
            if !geometry.contains_global(global_position) {
                return DockNativeCapturedWindowHit::OpaqueBarrier;
            }
            geometry.global_to_local(global_position).map_or(
                DockNativeCapturedWindowHit::Unavailable,
                |window_position| DockNativeCapturedWindowHit::RegisteredApplication {
                    window,
                    window_position,
                },
            )
        }
        PlatformWindowHit::ProvisionalPassThrough { .. } => {
            DockNativeCapturedWindowHit::Unavailable
        }
    }
}

fn route_admits_provisional_pass_through(
    route: &DockNativeCapturedDragRoute,
    window: AnyWindowHandle,
    session_generation: u64,
) -> bool {
    let crate::DockViewportRuntimeLineage::Surface(lease) = route.work_context.lineage() else {
        return false;
    };
    route.runtime.admits_work_context(route.work_context)
        && route
            .runtime
            .windows_for_surface(lease)
            .into_iter()
            .any(|(role, owned_window)| {
                owned_window == window
                    && matches!(
                        role,
                        crate::DockViewportWindowRole::ProvisionalViewport(opening)
                            if opening.lease() == lease
                                && opening.generation() == session_generation
                    )
            })
}

fn native_captured_window_hit(
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
) -> DockNativeCapturedWindowHit {
    let Some(physical_frame) = event.physical_frame() else {
        return DockNativeCapturedWindowHit::Unavailable;
    };
    classify_native_captured_window_hit(
        event.window_hit_stack(),
        physical_frame.global_position(),
        |window, session_generation| {
            route_admits_provisional_pass_through(route, window, session_generation)
        },
    )
}

fn native_captured_registered_window(
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
) -> Option<AnyWindowHandle> {
    match native_captured_window_hit(route, event) {
        DockNativeCapturedWindowHit::RegisteredApplication { window, .. } => Some(window),
        DockNativeCapturedWindowHit::OpenDesktop
        | DockNativeCapturedWindowHit::OpaqueBarrier
        | DockNativeCapturedWindowHit::Unavailable => None,
    }
}

fn native_event_may_target_scene(
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    scene: &DockNativeCapturedHostScene,
) -> bool {
    matches!(
        native_captured_window_hit(route, event),
        DockNativeCapturedWindowHit::RegisteredApplication { window, .. }
            if window.window_id() == scene.window_id
    )
}

fn refresh_native_captured_preview_after_scene_commit(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    epoch: u64,
    cx: &mut App,
) {
    let route_and_event = {
        let state = state.borrow();
        let Some(active) = state.active.as_ref().filter(|active| active.epoch == epoch) else {
            return;
        };
        active.preview_refresh_scheduled.set(false);
        if !active.start_consumer.is_active() {
            return;
        }
        let event = active.latest_event.borrow().clone();
        event.map(|event| (active.clone(), event))
    };
    let Some((route, event)) = route_and_event else {
        return;
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        if !route.runtime.admits_work_context(route.work_context)
            || route.runtime.active_payload_drag_session(&route.payload)
                != Some(route.session.clone())
        {
            return false;
        }
        let target = resolve_native_captured_target(state, &route, &event, cx);
        update_native_captured_preview(state, &route, &event, target.clone(), cx);
        update_live_undock_move(&route, &event, &target, cx);
        true
    }));
    match result {
        Ok(true) => {}
        Ok(false) => finish_matching_route(state, epoch, cx),
        Err(payload) => {
            let _cleanup_panic = finish_matching_route_cleanup(state, epoch, cx);
            resume_unwind(payload);
        }
    }
}

fn resolve_native_captured_target(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    cx: &mut App,
) -> DockNativeCapturedTarget {
    resolve_native_captured_target_with_source_window(state, route, event, None, cx)
}

fn resolve_native_captured_target_with_source_window(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    source_window: Option<&mut Window>,
    cx: &mut App,
) -> DockNativeCapturedTarget {
    #[cfg(test)]
    panic_native_captured_drag_if_requested(state, DockNativeCapturedDragTestPanic::ResolveTarget);
    let (target_window, target_window_position) = match native_captured_window_hit(route, event) {
        DockNativeCapturedWindowHit::OpenDesktop => {
            return DockNativeCapturedTarget::Desktop(DockNativeCapturedDesktopRoute::OpenSpace);
        }
        DockNativeCapturedWindowHit::OpaqueBarrier => {
            return DockNativeCapturedTarget::Desktop(
                DockNativeCapturedDesktopRoute::OpaqueBarrier,
            );
        }
        DockNativeCapturedWindowHit::RegisteredApplication {
            window,
            window_position,
        } => (window, window_position),
        DockNativeCapturedWindowHit::Unavailable => {
            return DockNativeCapturedTarget::Unavailable;
        }
    };

    let scenes = state
        .borrow()
        .scenes
        .get(&target_window.window_id())
        .cloned()
        .unwrap_or_default();
    if scenes.is_empty() {
        return DockNativeCapturedTarget::Desktop(DockNativeCapturedDesktopRoute::OpaqueBarrier);
    }
    let selection = if source_window
        .as_ref()
        .is_some_and(|window| window.window_handle().window_id() == target_window.window_id())
    {
        source_window
            .map(|window| select_frontmost_host_scene(scenes, target_window_position, window, cx))
    } else {
        cx.update_window(target_window, |_, window, cx| {
            select_frontmost_host_scene(scenes, target_window_position, window, cx)
        })
        .ok()
    };
    let target = match selection {
        Some(Some(target)) => target,
        Some(None) => {
            return DockNativeCapturedTarget::Desktop(
                DockNativeCapturedDesktopRoute::OpaqueBarrier,
            );
        }
        None => return DockNativeCapturedTarget::Unavailable,
    };
    classify_host_target(route, target)
}

fn classify_host_target(
    route: &DockNativeCapturedDragRoute,
    target: DockNativeCapturedHostTarget,
) -> DockNativeCapturedTarget {
    if target.scene.runtime_identity != route.runtime_identity
        || target.scene.work_context.lineage() != route.work_context.lineage()
    {
        DockNativeCapturedTarget::ForeignSurfaceTarget(target)
    } else {
        DockNativeCapturedTarget::Host(target)
    }
}

fn select_frontmost_host_scene(
    scenes: Vec<DockNativeCapturedHostScene>,
    window_position: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> Option<DockNativeCapturedHostTarget> {
    scenes
        .into_iter()
        .filter_map(|scene| {
            if !scene.frame.matches_viewport(&scene.space, scene.window_id) {
                return None;
            }
            let Some(hitbox) = scene.geometry.committed_hitbox() else {
                return None;
            };
            let rank = hitbox.window_point_target_rank(window_position, window);
            let host_position = scene.geometry.window_to_host(window_position);
            let host_position = host_position?;
            let host = scene.host.upgrade()?;
            let rank = rank?;
            let current = host.update(cx, |host, host_cx| {
                host.accepts_viewport_scene_candidate(
                    scene.host_binding,
                    Some(scene.frame.registration_key()),
                    scene.work_context,
                    scene.window_id,
                    host_cx,
                ) && host.viewport_runtime().identity() == scene.runtime_identity
            });
            current.then_some((
                rank,
                DockNativeCapturedHostTarget {
                    scene,
                    window_position,
                    host_position,
                },
            ))
        })
        .min_by_key(|(rank, _)| *rank)
        .map(|(_, target)| target)
}

fn native_route_request(
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    target: DockCapturedNativeDropRoute,
    cx: &App,
) -> DockViewportDropRouteRequest {
    let source_local_position = event
        .source_local_position()
        .expect("moved and released native captured-drag facts carry a source-local position");
    let tear_off_geometry = route
        .runtime
        .active_payload_drag_tear_off_geometry(Some(&route.session));
    let suggested_window_bounds = suggested_live_undock_window_bounds(route, event);
    DockViewportDropRouteRequest::from_captured_native_route(
        &route.payload,
        route.session.clone(),
        route.source_window_handle,
        tear_off_geometry,
        suggested_window_bounds,
        source_local_position,
        target,
        event.generation(),
        event.sequence(),
        cx,
    )
}

fn suggested_live_undock_window_bounds(
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
) -> Option<open_gpui::WindowBounds> {
    route
        .runtime
        .active_payload_drag_tear_off_geometry(Some(&route.session))
        .and_then(|geometry| {
            event.physical_frame().and_then(|physical_frame| {
                crate::viewport_runtime::suggested_tear_off_window_bounds_from_native_frame(
                    physical_frame,
                    geometry,
                )
            })
        })
}

fn desired_live_undock_physical_bounds(
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
) -> Option<DockLiveUndockPhysicalBounds> {
    let physical_frame = event.physical_frame()?;
    let logical = suggested_live_undock_window_bounds(route, event)?.get_bounds();
    let physical = logical.to_device_pixels(physical_frame.source_geometry().scale_factor());
    DockLiveUndockPhysicalBounds::new(
        DockLiveUndockPhysicalPoint::new(physical.origin.x.0, physical.origin.y.0),
        u32::try_from(physical.size.width.0).ok()?,
        u32::try_from(physical.size.height.0).ok()?,
    )
}

fn live_undock_route_feedback(target: &DockNativeCapturedTarget) -> DockLiveUndockRouteFeedback {
    match target {
        DockNativeCapturedTarget::Host(target) => DockLiveUndockRouteFeedback::Host(
            DockLiveUndockHostTarget::new(target.scene.window_id, target.scene.frame.generation()),
        ),
        DockNativeCapturedTarget::ForeignSurfaceTarget(target) => {
            DockLiveUndockRouteFeedback::ForeignSurface {
                window_id: target.scene.window_id,
            }
        }
        DockNativeCapturedTarget::Desktop(DockNativeCapturedDesktopRoute::OpenSpace) => {
            DockLiveUndockRouteFeedback::Desktop
        }
        DockNativeCapturedTarget::Desktop(DockNativeCapturedDesktopRoute::OpaqueBarrier) => {
            DockLiveUndockRouteFeedback::OpaqueBarrier
        }
        DockNativeCapturedTarget::Unavailable => DockLiveUndockRouteFeedback::Unavailable,
    }
}

fn live_undock_trigger_for_move(
    drag_start: Option<PlatformNativeDragStartSnapshot>,
    current: Option<Point<open_gpui::DevicePixels>>,
    drag_generation: DockLiveUndockDragGeneration,
    source: DockLiveUndockSourceSnapshot,
    route: DockLiveUndockRouteFeedback,
) -> Option<DockLiveUndockTrigger> {
    let drag_start = drag_start?;
    let current = current?;
    if !drag_start
        .hysteresis()
        .is_exceeded(drag_start.pointer_frame().global_position(), current)
    {
        return None;
    }
    DockLiveUndockTrigger::new(drag_generation, source, route)
}

fn live_undock_drag_generation(
    generation: NativeCapturedDragGeneration,
) -> DockLiveUndockDragGeneration {
    DockLiveUndockDragGeneration::new(generation.ordinal())
        .expect("GPUI native captured-drag generations are non-zero")
}

fn submit_live_undock_fact(
    route: &DockNativeCapturedDragRoute,
    fact: DockLiveUndockFact,
    event: Option<&NativeCapturedDragEvent>,
    cx: &mut App,
) -> bool {
    let crate::DockViewportRuntimeLineage::Surface(route_lease) = route.work_context.lineage()
    else {
        return false;
    };
    let Some(owner) = route.runtime.surface_owner_entity() else {
        return false;
    };
    let runtime = cx.read_entity(&owner, |owner, _| owner.live_undock_runtime());
    match fact {
        DockLiveUndockFact::Trigger { lease, trigger } => {
            if lease != route_lease {
                return false;
            }
            let Some(scene) = route.live_undock_source_scene.as_ref() else {
                return false;
            };
            if scene.window_id != route.source_window
                || scene.host != route.source_host
                || scene.host_binding != route.source_binding
                || scene.runtime_identity != route.runtime_identity
                || scene.work_context != route.work_context
                || scene.space != route.payload.source_space
                || scene.frame.generation() != trigger.source().scene_generation()
            {
                return false;
            }
            let seed = DockLiveUndockExecutionSeed::new(
                route.runtime.clone(),
                route.work_context,
                route.session.clone(),
                route.payload.clone(),
                route.source_window_handle,
                route.source_host.clone(),
                route.source_binding,
                route.transport.clone(),
                route.source_focus.clone(),
                scene.frame.clone(),
                scene.presentation_scene.clone(),
                event.and_then(|event| suggested_live_undock_window_bounds(route, event)),
                route.live_undock_identity.clone(),
                route.payload_finalizer.clone(),
            );
            runtime.start(lease, trigger, seed, cx)
        }
        fact => runtime.submit(fact, cx),
    }
}

fn update_live_undock_move(
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    target: &DockNativeCapturedTarget,
    cx: &mut App,
) {
    debug_assert_eq!(event.phase(), NativeCapturedDragPhase::Moved);
    let feedback = live_undock_route_feedback(target);
    if let crate::DockViewportRuntimeLineage::Surface(lease) = route.work_context.lineage()
        && let Some(source_scene) = route.live_undock_source_scene.as_ref()
        && let source =
            DockLiveUndockSourceSnapshot::new(route.source_window, source_scene.frame.generation())
        && suggested_live_undock_window_bounds(route, event).is_some()
        && let Some(trigger) = live_undock_trigger_for_move(
            route.native_drag_start_snapshot,
            event.physical_frame().map(|frame| frame.global_position()),
            live_undock_drag_generation(route.generation),
            source,
            feedback,
        )
    {
        let _ = submit_live_undock_fact(
            route,
            DockLiveUndockFact::Trigger { lease, trigger },
            Some(event),
            cx,
        );
    }
    if let Some(identity) = route.live_undock_identity.get() {
        let _ = submit_live_undock_fact(
            route,
            DockLiveUndockFact::RouteObserved {
                identity,
                route: feedback,
            },
            None,
            cx,
        );
    }
}

fn lock_live_undock_release(
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    target: &DockNativeCapturedTarget,
    locked_drop: &mut Option<Result<DockViewportLockedDropRoute, crate::DockActionApplyError>>,
    cx: &mut App,
) -> bool {
    let Some(identity) = route.live_undock_identity.get() else {
        return false;
    };
    let Some(point) = event.physical_frame().map(|frame| frame.global_position()) else {
        cancel_live_undock_identity(route, identity, DockLiveUndockCancelReason::CaptureLost, cx);
        return false;
    };
    let Some(placement_generation) =
        DockLiveUndockPlacementGeneration::new(event.sequence().ordinal())
    else {
        cancel_live_undock_identity(route, identity, DockLiveUndockCancelReason::CaptureLost, cx);
        return false;
    };
    let Some(desired_bounds) = desired_live_undock_physical_bounds(route, event) else {
        cancel_live_undock_identity(route, identity, DockLiveUndockCancelReason::CaptureLost, cx);
        return false;
    };
    let feedback = live_undock_route_feedback(target);
    let _ = submit_live_undock_fact(
        route,
        DockLiveUndockFact::RouteObserved {
            identity,
            route: feedback,
        },
        None,
        cx,
    );
    let release = DockLiveUndockReleaseLock::new(
        DockLiveUndockPhysicalPoint::new(point.x.0, point.y.0),
        feedback,
        desired_bounds,
        placement_generation,
    );
    let Some(owner) = route.runtime.surface_owner_entity() else {
        return false;
    };
    let host_release = match target {
        DockNativeCapturedTarget::Host(target) => {
            let locked = match locked_drop.take() {
                Some(Ok(locked)) => locked,
                other => {
                    *locked_drop = other;
                    return false;
                }
            };
            match DockLiveUndockHostReleaseAuthority::try_new(
                locked,
                open_gpui::WindowHandle::<DockHost>::new(target.scene.window_id),
                target.scene.host.clone(),
                target.scene.host_binding,
                target.scene.space.clone(),
                target.scene.frame.clone(),
            ) {
                Ok(authority) => Some(authority),
                Err(locked) => {
                    *locked_drop = Some(Ok(locked));
                    return false;
                }
            }
        }
        DockNativeCapturedTarget::ForeignSurfaceTarget(_)
        | DockNativeCapturedTarget::Desktop(_)
        | DockNativeCapturedTarget::Unavailable => None,
    };
    let runtime = cx.read_entity(&owner, |owner, _| owner.live_undock_runtime());
    match runtime.adopt_release(
        identity,
        release,
        host_release,
        &route.runtime,
        route.work_context,
        &route.session,
        &route.payload,
        &route.payload_finalizer,
        cx,
    ) {
        DockLiveUndockReleaseAdoption::Adopted => true,
        DockLiveUndockReleaseAdoption::Rejected(host_release) => {
            if let Some(host_release) = host_release {
                *locked_drop = Some(Ok(host_release.into_locked_drop()));
            }
            false
        }
    }
}

fn cancel_live_undock_route(
    route: &DockNativeCapturedDragRoute,
    reason: PointerCancelReason,
    cx: &mut App,
) {
    let Some(identity) = route.live_undock_identity.get() else {
        return;
    };
    cancel_live_undock_identity(route, identity, live_undock_cancel_reason(reason), cx);
}

fn live_undock_cancel_reason(reason: PointerCancelReason) -> DockLiveUndockCancelReason {
    match reason {
        PointerCancelReason::PlatformCaptureLost | PointerCancelReason::CaptureRevoked => {
            DockLiveUndockCancelReason::CaptureLost
        }
        PointerCancelReason::WindowDeactivated => DockLiveUndockCancelReason::SourceDeactivated,
        PointerCancelReason::WindowClosed => DockLiveUndockCancelReason::SourceClosed,
    }
}

fn terminal_live_undock_cancel_reason(phase: NativeCapturedDragPhase) -> PointerCancelReason {
    match phase {
        NativeCapturedDragPhase::Released => PointerCancelReason::CaptureRevoked,
        NativeCapturedDragPhase::Cancelled(reason) => reason,
        NativeCapturedDragPhase::Moved => {
            unreachable!("moving routes do not enter terminal cleanup")
        }
    }
}

fn cancel_live_undock_identity(
    route: &DockNativeCapturedDragRoute,
    identity: DockLiveUndockIdentity,
    reason: DockLiveUndockCancelReason,
    cx: &mut App,
) {
    let _ = submit_live_undock_fact(
        route,
        DockLiveUndockFact::Cancel { identity, reason },
        None,
        cx,
    );
}

fn update_native_captured_preview(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    target: DockNativeCapturedTarget,
    cx: &mut App,
) {
    clear_native_source_local_preview(route, cx);
    match target {
        DockNativeCapturedTarget::Host(target) => {
            clear_foreign_preview(route, cx);
            let Some(captured_target) = captured_host_target(route, event, &target) else {
                route.runtime.clear_routed_drop_preview(cx);
                return;
            };
            let request = native_route_request(
                route,
                event,
                DockCapturedNativeDropRoute::Host(captured_target),
                cx,
            );
            let source_feedback = current_source_feedback_target(state, route, cx);
            let (changed, projected_source_feedback) = match source_feedback {
                Some((_, source_position)) => (
                    route.runtime.resolve_and_update_host_routed_drop_preview(
                        &request,
                        &route.payload,
                        route.payload.source_space.clone(),
                        route.source_window,
                        source_position,
                        cx,
                    ),
                    true,
                ),
                None => (
                    route
                        .runtime
                        .resolve_and_update_routed_drop_preview(&request, &route.payload, cx)
                        .1,
                    false,
                ),
            };
            if changed && projected_source_feedback {
                refresh_native_source_feedback(route.source_window, cx);
            }
        }
        DockNativeCapturedTarget::ForeignSurfaceTarget(target) => {
            route.runtime.clear_routed_drop_preview(cx);
            update_foreign_surface_preview(state, route, event, target, cx);
        }
        DockNativeCapturedTarget::Desktop(_) => {
            clear_foreign_preview(route, cx);
            let request =
                native_route_request(route, event, DockCapturedNativeDropRoute::Desktop, cx);
            let source_feedback = current_source_feedback_target(state, route, cx);
            let (changed, projected_source_feedback) = match source_feedback {
                Some((_, source_position)) => (
                    route.runtime.resolve_and_update_host_routed_drop_preview(
                        &request,
                        &route.payload,
                        route.payload.source_space.clone(),
                        route.source_window,
                        source_position,
                        cx,
                    ),
                    true,
                ),
                None => (
                    route
                        .runtime
                        .resolve_and_update_routed_drop_preview(&request, &route.payload, cx)
                        .1,
                    false,
                ),
            };
            if changed && projected_source_feedback {
                refresh_native_source_feedback(route.source_window, cx);
            }
        }
        DockNativeCapturedTarget::Unavailable => {
            clear_foreign_preview(route, cx);
            route.runtime.clear_routed_drop_preview(cx);
            let request =
                native_route_request(route, event, DockCapturedNativeDropRoute::Unavailable, cx);
            route
                .runtime
                .resolve_and_update_routed_drop_preview(&request, &route.payload, cx);
        }
    }
}

fn clear_native_source_local_preview(route: &DockNativeCapturedDragRoute, cx: &mut App) {
    let Some(source_host) = route.source_host.upgrade() else {
        return;
    };
    source_host.update(cx, |host, host_cx| {
        if host.accepts_bound_window(Some(route.source_binding))
            && host.clear_drop_preview_interaction()
        {
            host_cx.notify();
        }
    });
}

fn commit_native_captured_release(
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    target: DockNativeCapturedTarget,
    locked_drop: Option<Result<DockViewportLockedDropRoute, crate::DockActionApplyError>>,
    cx: &mut App,
) {
    clear_foreign_preview(route, cx);
    match target {
        DockNativeCapturedTarget::Host(_) | DockNativeCapturedTarget::Desktop(_) => {
            commit_locked_native_captured_drop(route, locked_drop, cx);
        }
        DockNativeCapturedTarget::ForeignSurfaceTarget(target) => {
            let Some(target) = captured_host_target(route, event, &target) else {
                return route.runtime.record_locked_payload_drop_failure(
                    &route.session,
                    crate::DockActionApplyError::DropTargetUnavailable,
                    cx,
                );
            };
            let request = native_route_request(
                route,
                event,
                DockCapturedNativeDropRoute::ForbiddenTarget(target),
                cx,
            );
            let owner = DockViewportRoutedPreviewOwner::captured_native(
                route.runtime_identity,
                event.generation(),
                event.sequence(),
                route.session.clone(),
                route.latest_sequence.clone(),
            );
            if route
                .runtime
                .record_captured_native_foreign_surface_terminal(&request, &owner, &route.payload)
            {
                return;
            }
            route.runtime.record_locked_payload_drop_failure(
                &route.session,
                crate::DockActionApplyError::DropTargetUnavailable,
                cx,
            );
        }
        DockNativeCapturedTarget::Unavailable => {
            let request =
                native_route_request(route, event, DockCapturedNativeDropRoute::Unavailable, cx);
            let owner = DockViewportRoutedPreviewOwner::captured_native(
                route.runtime_identity,
                event.generation(),
                event.sequence(),
                route.session.clone(),
                route.latest_sequence.clone(),
            );
            if !route.runtime.record_captured_native_unavailable_terminal(
                &request,
                &owner,
                &route.payload,
            ) {
                route.runtime.record_locked_payload_drop_failure(
                    &route.session,
                    crate::DockActionApplyError::DropTargetUnavailable,
                    cx,
                );
            }
        }
    }
}

fn commit_locked_native_captured_drop(
    route: &DockNativeCapturedDragRoute,
    locked_drop: Option<Result<DockViewportLockedDropRoute, crate::DockActionApplyError>>,
    cx: &mut App,
) {
    let Some(locked_drop) = locked_drop else {
        return route.runtime.record_locked_payload_drop_failure(
            &route.session,
            crate::DockActionApplyError::DropTargetUnavailable,
            cx,
        );
    };
    let locked = match locked_drop {
        Ok(locked) if locked.drag_session() == &route.session => locked,
        Ok(_) => {
            return route.runtime.record_locked_payload_drop_failure(
                &route.session,
                crate::DockActionApplyError::DropDragSessionStale {
                    session: route.session.id(),
                },
                cx,
            );
        }
        Err(error) => {
            return route
                .runtime
                .record_locked_payload_drop_failure(&route.session, error, cx);
        }
    };
    if let Ok(outcome) = route
        .runtime
        .commit_locked_payload_drop_from_screen(locked, cx)
    {
        let _ = crate::viewport_activation::apply_viewport_activation_transaction(
            outcome.activation_transaction(),
            cx,
        );
    }
}

fn finish_matching_route(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    epoch: u64,
    cx: &mut App,
) {
    if let Some(payload) = finish_matching_route_cleanup(state, epoch, cx) {
        resume_unwind(payload);
    }
}

fn finish_matching_route_cleanup(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    epoch: u64,
    cx: &mut App,
) -> Option<Box<dyn Any + Send>> {
    let route = {
        let mut state = state.borrow_mut();
        if state
            .active
            .as_ref()
            .is_none_or(|active| active.epoch != epoch)
        {
            return None;
        }
        state.active.take()
    };
    let mut cleanup = DockNativeCapturedRouteCleanup {
        route: route?,
        first_panic: None,
        locked_drop: None,
        live_undock_release_adopted: false,
    };
    record_route_cleanup_cancel(&mut cleanup, PointerCancelReason::CaptureRevoked, cx);
    let cleanup =
        retain_retired_route_release(state, cleanup, PointerCancelReason::CaptureRevoked, cx)?;
    finish_route_cleanup(cleanup, cx)
}

fn schedule_route_retirement(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    route: DockNativeCapturedDragRoute,
    cx: &mut App,
) {
    schedule_route_cleanup_with_reason(
        state,
        DockNativeCapturedRouteCleanup {
            route,
            first_panic: None,
            locked_drop: None,
            live_undock_release_adopted: false,
        },
        PointerCancelReason::CaptureRevoked,
        cx,
    );
}

fn schedule_route_retirement_with_reason(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    route: DockNativeCapturedDragRoute,
    reason: PointerCancelReason,
    cx: &mut App,
) {
    schedule_route_cleanup_with_reason(
        state,
        DockNativeCapturedRouteCleanup {
            route,
            first_panic: None,
            locked_drop: None,
            live_undock_release_adopted: false,
        },
        reason,
        cx,
    );
}

fn schedule_route_cleanup(cleanup: DockNativeCapturedRouteCleanup, cx: &mut App) {
    retire_route_transport_proxy(&cleanup.route);
    cx.defer(move |cx| {
        if let Some(payload) = finish_route_cleanup(cleanup, cx) {
            resume_unwind(payload);
        }
    });
}

fn schedule_route_cleanup_with_reason(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    mut cleanup: DockNativeCapturedRouteCleanup,
    reason: PointerCancelReason,
    cx: &mut App,
) {
    record_route_cleanup_cancel(&mut cleanup, reason, cx);
    if let Some(cleanup) = retain_retired_route_release(state, cleanup, reason, cx) {
        schedule_route_cleanup(cleanup, cx);
    }
}

fn retain_retired_route_release(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    cleanup: DockNativeCapturedRouteCleanup,
    reason: PointerCancelReason,
    cx: &mut App,
) -> Option<DockNativeCapturedRouteCleanup> {
    let Some(key) = DockNativeCapturedDragRetiredKey::for_route(&cleanup.route) else {
        return Some(cleanup);
    };
    let terminal_state = Rc::downgrade(state);
    let Some(barrier) = cx.cancel_native_captured_drag_with_release_barrier(
        key.source_window,
        key.drag_generation,
        reason,
        move |barrier, terminal, cx| {
            let Some(state) = terminal_state.upgrade() else {
                return;
            };
            let Some(pending) = clear_retired_pending_if_matches(&state, key, barrier) else {
                return;
            };
            if terminal == NativeCapturedDragReleaseTerminal::Failed {
                state.borrow_mut().failed_releases.insert(key);
            }
            if let Some(cleanup) = pending.cleanup
                && let Some(payload) = finish_route_cleanup(cleanup, cx)
            {
                resume_unwind(payload);
            }
        },
    ) else {
        return Some(cleanup);
    };

    insert_retired_pending(state, key, barrier, Some(cleanup));
    None
}

fn insert_retired_pending(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    key: DockNativeCapturedDragRetiredKey,
    barrier: NativeCapturedDragReleaseBarrier,
    cleanup: Option<DockNativeCapturedRouteCleanup>,
) {
    let mut state = state.borrow_mut();
    match state.retired_pending.entry(key) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(DockNativeCapturedDragRetiredPending { barrier, cleanup });
        }
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            debug_assert_eq!(
                entry.get().barrier,
                barrier,
                "one retired Dock route generation must retain one exact native release barrier"
            );
            if entry.get().barrier == barrier && entry.get().cleanup.is_none() {
                entry.get_mut().cleanup = cleanup;
            }
        }
    }
}

fn clear_retired_pending_if_matches(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    key: DockNativeCapturedDragRetiredKey,
    barrier: NativeCapturedDragReleaseBarrier,
) -> Option<DockNativeCapturedDragRetiredPending> {
    let mut state = state.borrow_mut();
    let matches = state
        .retired_pending
        .get(&key)
        .is_some_and(|pending| pending.barrier == barrier);
    if matches {
        state.retired_pending.remove(&key)
    } else {
        None
    }
}

fn retire_route_cleanup(
    route: DockNativeCapturedDragRoute,
    cx: &mut App,
) -> Option<Box<dyn Any + Send>> {
    let mut first_panic = None;
    run_idempotent_cleanup_stage(&mut first_panic, || {
        retire_route_transport_proxy(&route);
    });
    run_idempotent_cleanup_stage(&mut first_panic, || clear_foreign_preview(&route, cx));

    let mut route_was_active = false;
    run_idempotent_cleanup_stage(&mut first_panic, || {
        route_was_active = route.runtime.active_payload_drag_session(&route.payload)
            == Some(route.session.clone());
    });
    run_idempotent_cleanup_stage(&mut first_panic, || {
        if let Some(host) = route.source_host.upgrade() {
            host.update(cx, |host, cx| {
                if !host.accepts_bound_window(Some(route.source_binding)) {
                    return;
                }
                let changed = host
                    .interaction_mut()
                    .clear_payload_drag_anchor_for_session(&route.session)
                    | (route_was_active && host.clear_drop_preview_interaction());
                if changed {
                    cx.notify();
                }
            });
        }
    });
    run_idempotent_cleanup_stage(&mut first_panic, || {
        settle_payload_drag_finalizer_claim(
            route.payload_finalizer.claim_route(),
            &route.runtime,
            route.work_context,
            &route.session,
            cx,
        );
    });
    first_panic
}

fn finish_route_cleanup(
    mut cleanup: DockNativeCapturedRouteCleanup,
    cx: &mut App,
) -> Option<Box<dyn Any + Send>> {
    let cleanup_panic = retire_route_cleanup(cleanup.route, cx);
    if cleanup.first_panic.is_none() {
        return cleanup_panic;
    }
    if cleanup_panic.is_some() {
        log::error!(
            "suppressed a Dock route-retirement panic after an earlier locked-release panic"
        );
    }
    cleanup.first_panic.take()
}

fn finish_route_cleanup_with_cancel(
    mut cleanup: DockNativeCapturedRouteCleanup,
    reason: PointerCancelReason,
    cx: &mut App,
) -> Option<Box<dyn Any + Send>> {
    record_route_cleanup_cancel(&mut cleanup, reason, cx);
    finish_route_cleanup(cleanup, cx)
}

fn record_route_cleanup_cancel(
    cleanup: &mut DockNativeCapturedRouteCleanup,
    reason: PointerCancelReason,
    cx: &mut App,
) {
    let route = cleanup.route.clone();
    run_idempotent_cleanup_stage(&mut cleanup.first_panic, || {
        retire_route_transport_proxy(&route)
    });
    run_idempotent_cleanup_stage(&mut cleanup.first_panic, || {
        cancel_live_undock_route(&route, reason, cx)
    });
}

fn retire_route_transport_proxy(route: &DockNativeCapturedDragRoute) {
    route.transport.retire();
}

fn run_idempotent_cleanup_stage(
    first_panic: &mut Option<Box<dyn Any + Send>>,
    mut cleanup: impl FnMut(),
) -> bool {
    for _ in 0..2 {
        match catch_unwind(AssertUnwindSafe(&mut cleanup)) {
            Ok(()) => return true,
            Err(payload) => {
                if first_panic.is_none() {
                    *first_panic = Some(payload);
                }
            }
        }
    }
    false
}

fn captured_host_target(
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    target: &DockNativeCapturedHostTarget,
) -> Option<DockCapturedNativeHostTarget> {
    let target_window = native_captured_registered_window(route, event)?;
    let target_window =
        (target_window.window_id() == target.scene.window_id).then_some(target_window)?;
    Some(DockCapturedNativeHostTarget::new(
        target_window,
        target.scene.space.clone(),
        target.host_position,
        target.scene.frame.clone(),
    ))
}

fn update_foreign_surface_preview(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    target: DockNativeCapturedHostTarget,
    cx: &mut App,
) {
    let Some(captured_target) = captured_host_target(route, event, &target) else {
        clear_foreign_preview(route, cx);
        return;
    };
    let request = native_route_request(
        route,
        event,
        DockCapturedNativeDropRoute::ForbiddenTarget(captured_target),
        cx,
    );
    let owner = DockViewportRoutedPreviewOwner::captured_native(
        route.runtime_identity,
        event.generation(),
        event.sequence(),
        route.session.clone(),
        route.latest_sequence.clone(),
    );
    let next = DockNativeCapturedForeignPreview {
        runtime: target.scene.runtime.clone(),
        owner,
        window_id: target.scene.window_id,
        host: target.scene.host.clone(),
        host_binding: target.scene.host_binding,
        frame: target.scene.frame.clone(),
    };
    {
        let mut previews = route.foreign_previews.borrow_mut();
        let displaced = previews.current.replace(next.clone());
        if let Some(displaced) = displaced.filter(|current| !current.same_owner_runtime(&next)) {
            queue_foreign_preview_cleanup(&mut previews, displaced);
        }
    }
    flush_foreign_preview_cleanups(route, cx);
    let current = next
        .runtime
        .resolve_and_project_captured_native_foreign_surface_preview(&request, &next.owner, cx);
    if !current {
        clear_foreign_preview_if_matches(route, &next, cx);
        return;
    }
    if !route
        .runtime
        .record_captured_native_source_foreign_surface_feedback(
            &request,
            &next.owner,
            &route.payload,
        )
    {
        clear_foreign_preview_if_matches(route, &next, cx);
        return;
    }
    if let Some((source_frame, source_position)) = current_source_feedback_target(state, route, cx)
    {
        let _ = route
            .runtime
            .project_captured_native_source_foreign_surface_preview(
                &request,
                &next.owner,
                &route.payload,
                route.source_window,
                &source_frame,
                source_position,
                cx,
            );
    }
}

fn current_source_feedback_target(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    route: &DockNativeCapturedDragRoute,
    cx: &mut App,
) -> Option<(DockViewportHostSceneFrame, Point<Pixels>)> {
    let scene = state
        .borrow()
        .scenes
        .get(&route.source_window)?
        .iter()
        .find(|scene| {
            scene.runtime_identity == route.runtime_identity
                && scene.work_context == route.work_context
                && scene.host == route.source_host
                && scene.host_binding == route.source_binding
        })?
        .clone();
    let host_position = scene
        .geometry
        .window_to_host(route.source_feedback_window_position)?;
    let host = scene.host.upgrade()?;
    let current = host.update(cx, |host, host_cx| {
        host.accepts_viewport_scene_candidate(
            scene.host_binding,
            Some(scene.frame.registration_key()),
            scene.work_context,
            scene.window_id,
            host_cx,
        ) && host.viewport_runtime().identity() == route.runtime_identity
            && host.active_payload_drag_session(&route.payload) == Some(route.session.clone())
    });
    current.then_some((scene.frame, host_position))
}

fn refresh_native_source_feedback(source_window: WindowId, cx: &mut App) {
    let Some(source_window) = cx
        .windows()
        .into_iter()
        .find(|window| window.window_id() == source_window)
    else {
        return;
    };
    let _ = cx.update_window(source_window, |_, window, _| window.refresh());
}

fn clear_foreign_preview(route: &DockNativeCapturedDragRoute, cx: &mut App) {
    {
        let mut previews = route.foreign_previews.borrow_mut();
        if let Some(current) = previews.current.take() {
            queue_foreign_preview_cleanup(&mut previews, current);
        }
    }
    flush_foreign_preview_cleanups(route, cx);
}

fn clear_foreign_preview_if_matches(
    route: &DockNativeCapturedDragRoute,
    expected: &DockNativeCapturedForeignPreview,
    cx: &mut App,
) {
    {
        let mut previews = route.foreign_previews.borrow_mut();
        if previews
            .current
            .as_ref()
            .is_some_and(|current| current.same_projection(expected))
        {
            let current = previews
                .current
                .take()
                .expect("matched foreign preview must remain current");
            queue_foreign_preview_cleanup(&mut previews, current);
        }
    }
    flush_foreign_preview_cleanups(route, cx);
}

fn queue_foreign_preview_cleanup(
    previews: &mut DockNativeCapturedForeignPreviewState,
    cleanup: DockNativeCapturedForeignPreview,
) {
    if previews
        .pending_cleanup
        .iter()
        .any(|pending| pending.same_projection(&cleanup))
    {
        return;
    }
    previews.pending_cleanup.push(cleanup);
}

fn flush_foreign_preview_cleanups(route: &DockNativeCapturedDragRoute, cx: &mut App) {
    loop {
        let Some(cleanup) = route
            .foreign_previews
            .borrow()
            .pending_cleanup
            .first()
            .cloned()
        else {
            return;
        };
        let mut first_panic = None;
        let target_cleared = run_idempotent_cleanup_stage(&mut first_panic, || {
            cleanup
                .runtime
                .clear_routed_drop_preview_for_owner(&cleanup.owner, cx);
        });
        let source_cleared = run_idempotent_cleanup_stage(&mut first_panic, || {
            route
                .runtime
                .clear_routed_drop_preview_for_owner(&cleanup.owner, cx);
        });
        if target_cleared && source_cleared {
            let mut previews = route.foreign_previews.borrow_mut();
            if let Some(index) = previews
                .pending_cleanup
                .iter()
                .position(|pending| pending.same_projection(&cleanup))
            {
                previews.pending_cleanup.remove(index);
            }
        }
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::{
        live_undock::{DockLiveUndockEffect, DockLiveUndockSession},
        window_session::{DockSurfaceWindowSession, DockSurfaceWindowSessionLease},
    };
    use open_gpui::{
        Bounds, DevicePixels, Empty, EntityId, PlatformNativeDragHysteresis,
        PlatformNativePointerPhysicalFrame, PlatformWindowPhysicalCoverage,
        PlatformWindowPhysicalGeometry, WindowHandle, point, size,
    };

    fn test_window(raw: u64) -> AnyWindowHandle {
        WindowHandle::<Empty>::new(WindowId::from(raw)).into()
    }

    fn test_point() -> Point<DevicePixels> {
        point(DevicePixels(50), DevicePixels(50))
    }

    fn test_coverage() -> PlatformWindowPhysicalCoverage {
        PlatformWindowPhysicalCoverage::try_new(Bounds::new(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(100), DevicePixels(100)),
        ))
        .expect("test coverage should be representable")
    }

    fn test_geometry() -> PlatformWindowPhysicalGeometry {
        PlatformWindowPhysicalGeometry::try_new(
            Bounds::new(
                point(DevicePixels(0), DevicePixels(0)),
                size(DevicePixels(100), DevicePixels(100)),
            ),
            1.0,
        )
        .expect("test geometry should be representable")
    }

    fn test_stack(hits: Vec<PlatformWindowHit>) -> PlatformWindowHitStack {
        PlatformWindowHitStack::try_available(test_point(), hits)
            .expect("test hit stack should have one legal exact shape")
    }

    fn provisional_hit(window: AnyWindowHandle, generation: u64) -> PlatformWindowHit {
        PlatformWindowHit::ProvisionalPassThrough {
            window,
            session_generation: generation,
            coverage: test_coverage(),
            geometry: test_geometry(),
        }
    }

    fn registered_hit(window: AnyWindowHandle) -> PlatformWindowHit {
        PlatformWindowHit::RegisteredApplication {
            window,
            coverage: test_coverage(),
            geometry: test_geometry(),
        }
    }

    fn active_surface_lease() -> DockSurfaceWindowSessionLease {
        let mut session = DockSurfaceWindowSession::new(EntityId::from(71));
        let opening = session
            .reserve_opening()
            .expect("test lease should reserve");
        session
            .commit_opening(opening, WindowId::from(72))
            .expect("test lease should activate")
    }

    fn drag_start_snapshot() -> PlatformNativeDragStartSnapshot {
        PlatformNativeDragStartSnapshot::new(
            PlatformNativePointerPhysicalFrame::new(
                point(DevicePixels(-10), DevicePixels(-20)),
                test_geometry(),
            ),
            PlatformNativeDragHysteresis::try_new(DevicePixels(4), DevicePixels(6))
                .expect("positive test hysteresis should be valid"),
        )
    }

    fn drag_generation(value: u64) -> DockLiveUndockDragGeneration {
        DockLiveUndockDragGeneration::new(value)
            .expect("test live-undock generation should be non-zero")
    }

    fn source_snapshot(value: u64) -> DockLiveUndockSourceSnapshot {
        DockLiveUndockSourceSnapshot::new(WindowId::from(73), value)
    }

    #[test]
    fn exact_current_provisional_prefix_reaches_registered_terminal() {
        let provisional = test_window(11);
        let terminal = test_window(12);
        let stack = test_stack(vec![
            provisional_hit(provisional, 41),
            registered_hit(terminal),
        ]);

        let resolved =
            classify_native_captured_window_hit(&stack, test_point(), |window, generation| {
                window == provisional && generation == 41
            });

        assert!(matches!(
            resolved,
            DockNativeCapturedWindowHit::RegisteredApplication { window, .. }
                if window == terminal
        ));
    }

    #[test]
    fn stale_or_foreign_provisional_prefix_fails_closed() {
        let current = test_window(21);
        let foreign = test_window(22);
        let terminal = test_window(23);
        for stack in [
            test_stack(vec![provisional_hit(current, 40), registered_hit(terminal)]),
            test_stack(vec![provisional_hit(foreign, 41), registered_hit(terminal)]),
        ] {
            assert_eq!(
                classify_native_captured_window_hit(&stack, test_point(), |window, generation| {
                    window == current && generation == 41
                }),
                DockNativeCapturedWindowHit::Unavailable
            );
        }
    }

    #[test]
    fn empty_exact_hit_stack_is_open_desktop() {
        let stack = test_stack(Vec::new());

        assert_eq!(
            classify_native_captured_window_hit(&stack, test_point(), |_, _| {
                panic!("empty desktop must not request provisional authority")
            }),
            DockNativeCapturedWindowHit::OpenDesktop
        );
    }

    #[test]
    fn opaque_terminal_remains_an_opaque_barrier() {
        let stack = test_stack(vec![PlatformWindowHit::OpaqueBarrier {
            coverage: test_coverage(),
        }]);

        assert_eq!(
            classify_native_captured_window_hit(&stack, test_point(), |_, _| {
                panic!("a terminal-only stack must not request provisional authority")
            }),
            DockNativeCapturedWindowHit::OpaqueBarrier
        );
    }

    #[test]
    fn registered_terminal_outside_its_physical_geometry_is_an_opaque_barrier() {
        let stack = PlatformWindowHitStack::try_available(
            test_point(),
            vec![PlatformWindowHit::RegisteredApplication {
                window: test_window(31),
                coverage: test_coverage(),
                geometry: PlatformWindowPhysicalGeometry::try_new(
                    Bounds::new(
                        point(DevicePixels(0), DevicePixels(0)),
                        size(DevicePixels(25), DevicePixels(25)),
                    ),
                    1.0,
                )
                .expect("test geometry should be representable"),
            }],
        )
        .expect("the registered terminal should form an exact stack");

        assert_eq!(
            classify_native_captured_window_hit(&stack, test_point(), |_, _| false),
            DockNativeCapturedWindowHit::OpaqueBarrier
        );
    }

    #[test]
    fn live_undock_threshold_is_inclusive_and_accepts_either_physical_axis() {
        let snapshot = drag_start_snapshot();
        let source = source_snapshot(1);
        assert!(
            live_undock_trigger_for_move(
                Some(snapshot),
                Some(point(DevicePixels(-7), DevicePixels(-20))),
                drag_generation(1),
                source,
                DockLiveUndockRouteFeedback::Desktop,
            )
            .is_none(),
            "three physical pixels remain inside the four-pixel horizontal hysteresis"
        );
        assert!(
            live_undock_trigger_for_move(
                Some(snapshot),
                Some(point(DevicePixels(-6), DevicePixels(-20))),
                drag_generation(1),
                source,
                DockLiveUndockRouteFeedback::Desktop,
            )
            .is_some(),
            "the horizontal boundary is eligible"
        );
        assert!(
            live_undock_trigger_for_move(
                Some(snapshot),
                Some(point(DevicePixels(-10), DevicePixels(-26))),
                drag_generation(1),
                source,
                DockLiveUndockRouteFeedback::OpaqueBarrier,
            )
            .is_some(),
            "the vertical boundary is eligible in the negative desktop quadrant"
        );
    }

    #[test]
    fn ineligible_routes_remain_armed_until_an_eligible_desktop_route() {
        let snapshot = drag_start_snapshot();
        let current = Some(point(DevicePixels(100), DevicePixels(100)));
        let source = source_snapshot(2);
        for feedback in [
            DockLiveUndockRouteFeedback::Host(DockLiveUndockHostTarget::new(WindowId::from(74), 1)),
            DockLiveUndockRouteFeedback::ForeignSurface {
                window_id: WindowId::from(75),
            },
            DockLiveUndockRouteFeedback::Unavailable,
        ] {
            assert!(
                live_undock_trigger_for_move(
                    Some(snapshot),
                    current,
                    drag_generation(2),
                    source,
                    feedback,
                )
                .is_none()
            );
        }
        let trigger = live_undock_trigger_for_move(
            Some(snapshot),
            current,
            drag_generation(2),
            source,
            DockLiveUndockRouteFeedback::Desktop,
        )
        .expect("a later open-space observation must still trigger");
        let lease = active_surface_lease();
        let mut reducer = DockLiveUndockSession::new();
        let effects = reducer.apply(DockLiveUndockFact::Trigger { lease, trigger });
        assert!(
            effects
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::OpenProvisional { .. })),
            "the single reducer accepts the still-armed drag generation"
        );
    }

    #[test]
    fn open_space_and_opaque_barrier_share_trigger_eligibility_but_not_route_identity() {
        let snapshot = drag_start_snapshot();
        let current = Some(point(DevicePixels(100), DevicePixels(100)));
        let source = source_snapshot(3);
        assert_eq!(
            live_undock_route_feedback(&DockNativeCapturedTarget::Desktop(
                DockNativeCapturedDesktopRoute::OpenSpace,
            )),
            DockLiveUndockRouteFeedback::Desktop
        );
        assert_eq!(
            live_undock_route_feedback(&DockNativeCapturedTarget::Desktop(
                DockNativeCapturedDesktopRoute::OpaqueBarrier,
            )),
            DockLiveUndockRouteFeedback::OpaqueBarrier
        );
        let open = live_undock_trigger_for_move(
            Some(snapshot),
            current,
            drag_generation(3),
            source,
            DockLiveUndockRouteFeedback::Desktop,
        )
        .expect("open desktop should trigger");
        let barrier = live_undock_trigger_for_move(
            Some(snapshot),
            current,
            drag_generation(3),
            source,
            DockLiveUndockRouteFeedback::OpaqueBarrier,
        )
        .expect("opaque desktop barrier should trigger");

        assert_eq!(open.initial_route(), DockLiveUndockRouteFeedback::Desktop);
        assert_eq!(
            barrier.initial_route(),
            DockLiveUndockRouteFeedback::OpaqueBarrier
        );
    }

    #[test]
    fn reducer_fences_duplicate_and_stale_live_undock_move_generations() {
        let lease = active_surface_lease();
        let source = source_snapshot(5);
        let trigger = DockLiveUndockTrigger::new(
            drag_generation(5),
            source,
            DockLiveUndockRouteFeedback::Desktop,
        )
        .expect("desktop should be eligible");
        let mut reducer = DockLiveUndockSession::new();
        let first = reducer.apply(DockLiveUndockFact::Trigger { lease, trigger });
        assert!(
            first
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::OpenProvisional { .. }))
        );
        assert!(
            reducer
                .apply(DockLiveUndockFact::Trigger { lease, trigger })
                .is_empty(),
            "a duplicate move cannot open a second provisional"
        );
        let stale = DockLiveUndockTrigger::new(
            drag_generation(4),
            source,
            DockLiveUndockRouteFeedback::OpaqueBarrier,
        )
        .expect("opaque barrier should be eligible");
        assert!(
            reducer
                .apply(DockLiveUndockFact::Trigger {
                    lease,
                    trigger: stale,
                })
                .is_empty(),
            "a stale drag generation cannot replace the reducer authority"
        );
    }

    #[test]
    fn released_route_failure_cancels_the_exact_live_undock_generation() {
        let lease = active_surface_lease();
        let trigger = DockLiveUndockTrigger::new(
            drag_generation(6),
            source_snapshot(6),
            DockLiveUndockRouteFeedback::Desktop,
        )
        .expect("desktop should be eligible");
        let mut reducer = DockLiveUndockSession::new();
        let identity = reducer
            .apply(DockLiveUndockFact::Trigger { lease, trigger })
            .into_iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::OpenProvisional { identity, .. } => Some(identity),
                _ => None,
            })
            .expect("the exact generation should begin one live-undock session");

        let reason = live_undock_cancel_reason(terminal_live_undock_cancel_reason(
            NativeCapturedDragPhase::Released,
        ));
        assert_eq!(reason, DockLiveUndockCancelReason::CaptureLost);
        let effects = reducer.apply(DockLiveUndockFact::Cancel { identity, reason });

        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                identity: current,
                result: crate::surface::live_undock::DockLiveUndockTerminalResult::Restored(
                    crate::surface::live_undock::DockLiveUndockRestoreReason::Cancelled(
                        DockLiveUndockCancelReason::CaptureLost
                    )
                ),
            } if *current == identity
        )));
        assert_eq!(
            reducer.phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Retiring
        );
        reducer.apply(DockLiveUndockFact::OpeningFailed { identity });
        assert_eq!(
            reducer.phase(),
            crate::surface::live_undock::DockLiveUndockPhase::Idle
        );
    }

    #[test]
    fn recorded_resolution_panic_does_not_skip_the_cancel_cleanup_stage() {
        let mut first_panic: Option<Box<dyn Any + Send>> = Some(Box::new("resolution panic"));
        let cancel_attempts = Cell::new(0);

        assert!(!run_idempotent_cleanup_stage(&mut first_panic, || {
            cancel_attempts.set(cancel_attempts.get() + 1);
            panic!("cancel cleanup panic");
        }));

        assert_eq!(cancel_attempts.get(), 2);
        assert_eq!(
            first_panic
                .as_ref()
                .and_then(|payload| payload.downcast_ref::<&'static str>())
                .copied(),
            Some("resolution panic"),
            "the locked-release failure remains the panic reported after cleanup"
        );
    }

    #[test]
    fn failed_native_release_does_not_authorize_surface_dependent_effects() {
        assert_eq!(
            DockNativeCapturedSurfaceReleaseOutcome::from_native_terminal(
                NativeCapturedDragReleaseTerminal::Failed,
            ),
            DockNativeCapturedSurfaceReleaseOutcome::Failed,
        );
        for terminal in [
            NativeCapturedDragReleaseTerminal::Released,
            NativeCapturedDragReleaseTerminal::NativeWindowTerminal,
            NativeCapturedDragReleaseTerminal::NotRequired,
        ] {
            assert_eq!(
                DockNativeCapturedSurfaceReleaseOutcome::from_native_terminal(terminal),
                DockNativeCapturedSurfaceReleaseOutcome::Released,
            );
        }
    }
}
