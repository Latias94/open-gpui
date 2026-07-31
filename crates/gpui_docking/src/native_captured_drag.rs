use crate::{
    DockCapturedNativeDropRoute, DockCapturedNativeHostTarget, DockHost, DockHostWindowBinding,
    DockSpaceId, DockViewportDropRouteRequest, DockViewportHostGeometry,
    DockViewportRoutedPreviewOwner, DockViewportRuntimeHandle, DockViewportRuntimeIdentity,
    DockViewportRuntimeWorkContext,
    drag::DockDragPayload,
    interaction::DockRuntimeDragSession,
    viewport_drop_scene::{DockViewportHostSceneDraft, DockViewportHostSceneFrame},
};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, DragStartGeometry, Global, NativeCapturedDragEvent,
    NativeCapturedDragGeneration, NativeCapturedDragPhase, NativeCapturedDragReleaseBarrier,
    NativeIngressSequence, Pixels, PlatformWindowHit, PlatformWindowHitStack, Point,
    PointerCancelReason, PreparedNativeCapturedDragConsumer, Subscription, WeakEntity, Window,
    WindowId,
};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
    sync::Arc,
};

struct DockNativeCapturedDragRouter {
    state: Rc<RefCell<DockNativeCapturedDragState>>,
    _drag_subscription: Subscription,
    _window_closed_subscription: Subscription,
}

impl Global for DockNativeCapturedDragRouter {}

#[derive(Default)]
struct DockNativeCapturedDragState {
    next_epoch: u64,
    active: Option<DockNativeCapturedDragRoute>,
    retired_pending:
        HashMap<DockNativeCapturedDragRetiredKey, DockNativeCapturedDragRetiredPending>,
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
    route: Option<DockNativeCapturedDragRoute>,
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
    session: DockRuntimeDragSession,
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
    source_feedback_window_position: Point<Pixels>,
    source_host: WeakEntity<DockHost>,
    source_binding: DockHostWindowBinding,
    start_consumer: PreparedNativeCapturedDragConsumer,
    foreign_previews: Rc<RefCell<DockNativeCapturedForeignPreviewState>>,
    latest_sequence: Rc<Cell<Option<NativeIngressSequence>>>,
    latest_event: Rc<RefCell<Option<NativeCapturedDragEvent>>>,
    preview_refresh_scheduled: Rc<Cell<bool>>,
    published_scene_frames: Rc<RefCell<Vec<DockNativeCapturedScenePublication>>>,
    terminal: bool,
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
}

#[derive(Clone)]
struct DockNativeCapturedHostTarget {
    scene: DockNativeCapturedHostScene,
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
    Desktop,
    Unavailable,
}

struct DockNativeCapturedReleaseReservation {
    route_epoch: u64,
    generation: NativeCapturedDragGeneration,
    target: DockNativeCapturedTarget,
    resolution_panic: RefCell<Option<Box<dyn Any + Send>>>,
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
    cx.set_global(DockNativeCapturedDragRouter {
        state,
        _drag_subscription: drag_subscription,
        _window_closed_subscription: window_closed_subscription,
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

        let Some(active) = state.active.as_ref() else {
            return;
        };
        let routed_previews = active
            .published_scene_frames
            .borrow()
            .iter()
            .filter_map(|publication| {
                (publication.scene.window_id == window_id
                    && publication.scene.runtime_identity == active.runtime_identity
                    && active.latest_event.borrow().as_ref().is_some_and(|event| {
                        native_event_may_target_scene(event, &publication.scene)
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
    };

    if let Some(source_route) = source_route {
        schedule_route_retirement(state, source_route, cx);
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
        state.borrow().active.as_ref().is_some_and(|active| {
            active.start_consumer.is_active()
                && !active.terminal
                && active.runtime_identity == runtime_identity
                && &active.session == session
                && &active.payload == payload
                && active.source_window == source_window
                && active.source_host == *source_host
                && source_binding.is_some_and(|binding| active.source_binding == binding)
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
    source_window: WindowId,
    source_host: WeakEntity<DockHost>,
    source_binding: DockHostWindowBinding,
    drag_start: &DragStartGeometry,
    cx: &mut App,
) -> DockNativeCapturedDragRouteReceipt {
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
        .collect();
    let route = DockNativeCapturedDragRoute {
        epoch,
        generation,
        runtime_identity,
        runtime,
        work_context,
        session: session.clone(),
        payload,
        source_window,
        source_feedback_window_position: drag_start.window_position(),
        source_host,
        source_binding,
        start_consumer,
        foreign_previews: Rc::new(RefCell::new(
            DockNativeCapturedForeignPreviewState::default(),
        )),
        latest_sequence: Rc::new(Cell::new(None)),
        latest_event: Rc::new(RefCell::new(None)),
        preview_refresh_scheduled: Rc::new(Cell::new(false)),
        published_scene_frames: Rc::new(RefCell::new(published_scene_frames)),
        terminal: false,
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
        session,
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
        state.active.as_ref().and_then(|active| {
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
                .is_some_and(|event| native_event_may_target_scene(event, &scene));
            let supports_source_route = scene_supports_source_route(&scene, active);
            let should_refresh = scene_changed
                && (targets_scene || supports_source_route)
                && active.start_consumer.is_active()
                && !active.terminal
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
        let retire_source_route = removed_source_scene && !replacement_supports_source_route;
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
                        && latest_event
                            .as_ref()
                            .is_some_and(|event| native_event_may_target_scene(event, scene)))
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
    on_native_capture_terminal: impl FnOnce(&mut Option<Box<dyn Any + Send + 'static>>, &mut App)
    + 'static,
    cx: &mut App,
) {
    let Some(state) = router_state(cx) else {
        cx.defer(move |cx| on_native_capture_terminal(&mut None, cx));
        return;
    };
    let (active, retired_pending) = {
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
                    pending.route.take(),
                ))
            })
            .collect::<Vec<_>>();
        (active, retired_pending)
    };
    let pending_count = usize::from(active.is_some()) + retired_pending.len();
    if pending_count == 0 {
        cx.defer(move |cx| on_native_capture_terminal(&mut None, cx));
        return;
    }

    let completion = Rc::new(RefCell::new(DockNativeCapturedSurfaceCancellation {
        remaining: pending_count,
        on_native_capture_terminal: Some(Box::new(on_native_capture_terminal)),
        first_panic: None,
    }));
    if let Some(route) = active {
        attach_active_surface_route_release(&state, route, completion.clone(), cx);
    }
    for (key, barrier, route) in retired_pending {
        attach_retired_surface_route_release(&state, key, barrier, route, completion.clone(), cx);
    }
}

type DockNativeCapturedSurfaceTerminal =
    Box<dyn FnOnce(&mut Option<Box<dyn Any + Send + 'static>>, &mut App)>;

struct DockNativeCapturedSurfaceCancellation {
    remaining: usize,
    on_native_capture_terminal: Option<DockNativeCapturedSurfaceTerminal>,
    first_panic: Option<Box<dyn Any + Send + 'static>>,
}

fn attach_active_surface_route_release(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    route: DockNativeCapturedDragRoute,
    completion: Rc<RefCell<DockNativeCapturedSurfaceCancellation>>,
    cx: &mut App,
) {
    let key = DockNativeCapturedDragRetiredKey::for_route(&route)
        .expect("a surface shutdown route must carry its exact surface lease");
    let pending_route = Rc::new(RefCell::new(Some(route)));
    let terminal_route = pending_route.clone();
    let terminal_state = Rc::downgrade(state);
    let terminal_completion = completion.clone();
    let barrier = cx.cancel_native_captured_drag_with_release_barrier(
        key.source_window,
        key.drag_generation,
        PointerCancelReason::WindowClosed,
        move |barrier, _, cx| {
            if let Some(state) = terminal_state.upgrade() {
                let _ = clear_retired_pending_if_matches(&state, key, barrier);
            }
            let route = terminal_route
                .borrow_mut()
                .take()
                .expect("one captured route release must settle exactly once");
            finish_surface_route_cancellation(Some(route), terminal_completion, cx);
        },
    );
    if let Some(barrier) = barrier {
        insert_retired_pending(state, key, barrier, None);
        return;
    }

    let route = pending_route
        .borrow_mut()
        .take()
        .expect("an unreserved captured route must remain available for cleanup");
    cx.defer(move |cx| finish_surface_route_cancellation(Some(route), completion, cx));
}

fn attach_retired_surface_route_release(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    key: DockNativeCapturedDragRetiredKey,
    expected_barrier: NativeCapturedDragReleaseBarrier,
    route: Option<DockNativeCapturedDragRoute>,
    completion: Rc<RefCell<DockNativeCapturedSurfaceCancellation>>,
    cx: &mut App,
) {
    let pending_route = Rc::new(RefCell::new(Some(route)));
    let terminal_route = pending_route.clone();
    let terminal_state = Rc::downgrade(state);
    let terminal_completion = completion.clone();
    let barrier = cx.cancel_native_captured_drag_with_release_barrier(
        key.source_window,
        key.drag_generation,
        PointerCancelReason::WindowClosed,
        move |barrier, _, cx| {
            if let Some(state) = terminal_state.upgrade() {
                let _ = clear_retired_pending_if_matches(&state, key, barrier);
            }
            let route = terminal_route
                .borrow_mut()
                .take()
                .expect("one retired capture release must settle exactly once");
            finish_surface_route_cancellation(route, terminal_completion, cx);
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
    let route = pending_route
        .borrow_mut()
        .take()
        .expect("an already-terminal retired capture must remain available for cleanup");
    cx.defer(move |cx| finish_surface_route_cancellation(route, completion, cx));
}

fn finish_surface_route_cancellation(
    route: Option<DockNativeCapturedDragRoute>,
    completion: Rc<RefCell<DockNativeCapturedSurfaceCancellation>>,
    cx: &mut App,
) {
    let route_panic = route.and_then(|route| retire_route_cleanup(route, cx));
    let terminal = {
        let mut completion = completion.borrow_mut();
        if completion.first_panic.is_none() {
            completion.first_panic = route_panic;
        } else if route_panic.is_some() {
            log::error!(
                "suppressed a Dock route-retirement panic while awaiting surface capture terminals"
            );
        }
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
                completion.first_panic.take(),
            )
        })
    };
    let Some((on_native_capture_terminal, mut first_panic)) = terminal else {
        return;
    };
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
        on_native_capture_terminal(&mut first_panic, cx)
    })) {
        if first_panic.is_none() {
            first_panic = Some(payload);
        } else {
            log::error!(
                "suppressed a DockSurface capture-terminal panic after route retirement panicked"
            );
        }
    }
    if let Some(payload) = first_panic {
        resume_unwind(payload);
    }
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
    let route = {
        let state = state.borrow();
        state
            .active
            .as_ref()
            .filter(|active| {
                active.start_consumer.is_active()
                    && !active.terminal
                    && active.generation == event.generation()
                    && active.source_window == event.source_window()
                    && active.session.accepts_payload(&active.payload)
                    && event.payload::<DockDragPayload>() == Some(&active.payload)
                    && active
                        .latest_sequence
                        .get()
                        .is_none_or(|sequence| sequence < event.sequence())
            })
            .cloned()
    };
    let Some(route) = route else {
        return Arc::new(());
    };
    let resolution = catch_unwind(AssertUnwindSafe(|| {
        if route.runtime.admits_work_context(route.work_context)
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
        }
    }));
    let (target, resolution_panic) = match resolution {
        Ok(target) => (target, None),
        Err(payload) => (DockNativeCapturedTarget::Unavailable, Some(payload)),
    };
    Arc::new(DockNativeCapturedReleaseReservation {
        route_epoch: route.epoch,
        generation: route.generation,
        target,
        resolution_panic: RefCell::new(resolution_panic),
    })
}

fn consume_native_captured_drag_event(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    event: NativeCapturedDragEvent,
    cx: &mut App,
) {
    let terminal = !matches!(event.phase(), NativeCapturedDragPhase::Moved);
    let route = {
        let mut state = state.borrow_mut();
        let Some(active) = state.active.as_ref() else {
            return;
        };
        if !active.start_consumer.is_active() {
            return;
        }
        if active.terminal
            || active.generation != event.generation()
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
            let mut active = state
                .active
                .take()
                .expect("validated terminal Dock route must remain active");
            active.latest_sequence.set(Some(event.sequence()));
            active.terminal = true;
            active
        } else {
            let active = state
                .active
                .as_mut()
                .expect("validated moving Dock route must remain active");
            active.latest_sequence.set(Some(event.sequence()));
            active.latest_event.replace(Some(event.clone()));
            active.clone()
        }
    };

    let result = catch_unwind(AssertUnwindSafe(|| {
        if matches!(event.phase(), NativeCapturedDragPhase::Released) {
            resume_locked_native_captured_release_panic(&route, &event);
        }
        let work_context_admitted = route.runtime.admits_work_context(route.work_context);
        let session_current = route.runtime.active_payload_drag_session(&route.payload)
            == Some(route.session.clone());
        if !work_context_admitted || !session_current {
            return false;
        }

        if !terminal {
            let target = resolve_native_captured_target(state, &route, &event, cx);
            update_native_captured_preview(state, &route, &event, target, cx);
            return true;
        }

        match event.phase() {
            NativeCapturedDragPhase::Released => {
                let target = locked_native_captured_release_target(state, &route, &event, cx);
                commit_native_captured_release(&route, &event, target, cx);
            }
            NativeCapturedDragPhase::Cancelled(_) => {}
            NativeCapturedDragPhase::Moved => unreachable!("moving route was handled above"),
        }
        true
    }));

    if terminal {
        let cleanup_panic = retire_route_cleanup(route, cx);
        if let Err(payload) = result {
            resume_unwind(payload);
        }
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
    if reservation.route_epoch != route.epoch || reservation.generation != route.generation {
        return DockNativeCapturedTarget::Unavailable;
    }
    match &reservation.target {
        DockNativeCapturedTarget::Host(target) => {
            current_locked_native_captured_host_target(state, event, target, cx)
                .map(DockNativeCapturedTarget::Host)
                .unwrap_or(DockNativeCapturedTarget::Unavailable)
        }
        DockNativeCapturedTarget::ForeignSurfaceTarget(target) => {
            current_locked_native_captured_host_target(state, event, target, cx)
                .map(DockNativeCapturedTarget::ForeignSurfaceTarget)
                .unwrap_or(DockNativeCapturedTarget::Unavailable)
        }
        DockNativeCapturedTarget::Desktop => DockNativeCapturedTarget::Desktop,
        DockNativeCapturedTarget::Unavailable => DockNativeCapturedTarget::Unavailable,
    }
}

fn resume_locked_native_captured_release_panic(
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
) {
    let Some(reservation) = event.route_lock::<DockNativeCapturedReleaseReservation>() else {
        return;
    };
    if reservation.route_epoch != route.epoch || reservation.generation != route.generation {
        return;
    }
    if let Some(payload) = reservation.resolution_panic.borrow_mut().take() {
        resume_unwind(payload);
    }
}

fn current_locked_native_captured_host_target(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    event: &NativeCapturedDragEvent,
    target: &DockNativeCapturedHostTarget,
    cx: &mut App,
) -> Option<DockNativeCapturedHostTarget> {
    let scene = state
        .borrow()
        .scenes
        .get(&target.scene.window_id)
        .and_then(|scenes| {
            scenes
                .iter()
                .find(|scene| {
                    scene.host == target.scene.host
                        && scene.host_binding == target.scene.host_binding
                        && scene.runtime_identity == target.scene.runtime_identity
                        && scene.work_context == target.scene.work_context
                        && scene.space == target.scene.space
                        && scene.frame.registration_key() == target.scene.frame.registration_key()
                        && scene
                            .routing_scene
                            .has_same_native_routing_content(&target.scene.routing_scene)
                })
                .cloned()
        })?;
    let target_window = event.window_hit_stack().first_registered_window()?;
    if target_window.window_id() != target.scene.window_id {
        return None;
    }
    if cx.update_window(target_window, |_, _, _| ()).is_err() {
        return None;
    }
    if !scene
        .runtime
        .is_current_viewport_host_scene_frame(&scene.frame)
    {
        return None;
    }
    let host = scene.host.upgrade()?;
    let accepted = host.update(cx, |host, host_cx| {
        host.accepts_viewport_scene_candidate(
            scene.host_binding,
            Some(scene.frame.registration_key()),
            scene.work_context,
            scene.window_id,
            host_cx,
        ) && host.viewport_runtime().identity() == scene.runtime_identity
    });
    if !accepted {
        return None;
    }
    Some(DockNativeCapturedHostTarget {
        scene,
        host_position: target.host_position,
    })
}

fn native_event_may_target_scene(
    event: &NativeCapturedDragEvent,
    scene: &DockNativeCapturedHostScene,
) -> bool {
    let Some(physical_frame) = event.physical_frame() else {
        return false;
    };
    let PlatformWindowHitStack::Available(observation) = event.window_hit_stack() else {
        return false;
    };
    let global_position = physical_frame.global_position();
    observation.sampled_point() == global_position
        && observation.hits().first().is_some_and(|hit| {
            matches!(
                hit,
                PlatformWindowHit::RegisteredApplication {
                    window,
                    coverage,
                    geometry,
                } if window.window_id() == scene.window_id
                    && coverage.contains(global_position)
                    && geometry.contains_global(global_position)
            )
        })
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
        if active.terminal || !active.start_consumer.is_active() {
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
        update_native_captured_preview(state, &route, &event, target, cx);
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
    let Some(physical_frame) = event.physical_frame() else {
        return DockNativeCapturedTarget::Unavailable;
    };
    let global_position = physical_frame.global_position();
    let observation = match event.window_hit_stack() {
        PlatformWindowHitStack::Unavailable => {
            return DockNativeCapturedTarget::Unavailable;
        }
        PlatformWindowHitStack::Available(observation)
            if observation.sampled_point() == global_position =>
        {
            observation
        }
        PlatformWindowHitStack::Available(_) => return DockNativeCapturedTarget::Unavailable,
    };
    let Some(frontmost) = observation.hits().first() else {
        return DockNativeCapturedTarget::Desktop;
    };
    let (target_window, target_window_position) = match *frontmost {
        PlatformWindowHit::OpaqueBarrier { coverage } => {
            return if coverage.contains(global_position) {
                DockNativeCapturedTarget::Desktop
            } else {
                DockNativeCapturedTarget::Unavailable
            };
        }
        PlatformWindowHit::RegisteredApplication {
            window,
            coverage,
            geometry,
        } => {
            if !coverage.contains(global_position) {
                return DockNativeCapturedTarget::Unavailable;
            }
            if !geometry.contains_global(global_position) {
                return DockNativeCapturedTarget::Desktop;
            }
            let Some(position) = geometry.global_to_local(global_position) else {
                return DockNativeCapturedTarget::Unavailable;
            };
            (window, position)
        }
    };

    let scenes = state
        .borrow()
        .scenes
        .get(&target_window.window_id())
        .cloned()
        .unwrap_or_default();
    if scenes.is_empty() {
        return DockNativeCapturedTarget::Desktop;
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
        Some(None) => return DockNativeCapturedTarget::Desktop,
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
    let suggested_window_bounds = tear_off_geometry.and_then(|geometry| {
        event.physical_frame().and_then(|physical_frame| {
            crate::viewport_runtime::suggested_tear_off_window_bounds_from_native_frame(
                physical_frame,
                geometry,
            )
        })
    });
    DockViewportDropRouteRequest::from_captured_native_route(
        &route.payload,
        route.session.clone(),
        tear_off_geometry,
        suggested_window_bounds,
        source_local_position,
        target,
        event.generation(),
        event.sequence(),
        cx,
    )
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
            let Some(captured_target) = captured_host_target(event, &target) else {
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
        DockNativeCapturedTarget::Desktop => {
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
    cx: &mut App,
) {
    clear_foreign_preview(route, cx);
    let captured_route = match target {
        DockNativeCapturedTarget::Host(target) => captured_host_target(event, &target)
            .map(DockCapturedNativeDropRoute::Host)
            .unwrap_or(DockCapturedNativeDropRoute::Unavailable),
        DockNativeCapturedTarget::Desktop => DockCapturedNativeDropRoute::Desktop,
        DockNativeCapturedTarget::ForeignSurfaceTarget(target) => {
            let Some(target) = captured_host_target(event, &target) else {
                return commit_native_captured_unavailable_release(route, event, cx);
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
            DockCapturedNativeDropRoute::Unavailable
        }
        DockNativeCapturedTarget::Unavailable => DockCapturedNativeDropRoute::Unavailable,
    };
    let request = native_route_request(route, event, captured_route, cx);
    if let Ok(outcome) = route.runtime.commit_payload_drop_from_screen(&request, cx) {
        let _ = crate::viewport_activation::apply_viewport_activation_transaction(
            outcome.activation_transaction(),
            cx,
        );
    }
}

fn commit_native_captured_unavailable_release(
    route: &DockNativeCapturedDragRoute,
    event: &NativeCapturedDragEvent,
    cx: &mut App,
) {
    let request = native_route_request(route, event, DockCapturedNativeDropRoute::Unavailable, cx);
    let _ = route.runtime.commit_payload_drop_from_screen(&request, cx);
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
    let route = route?;
    let route =
        retain_retired_route_release(state, route, PointerCancelReason::CaptureRevoked, cx)?;
    retire_route_cleanup(route, cx)
}

fn schedule_route_retirement(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    route: DockNativeCapturedDragRoute,
    cx: &mut App,
) {
    schedule_route_retirement_with_reason(state, route, PointerCancelReason::CaptureRevoked, cx);
}

fn schedule_route_retirement_with_reason(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    route: DockNativeCapturedDragRoute,
    reason: PointerCancelReason,
    cx: &mut App,
) {
    if let Some(route) = retain_retired_route_release(state, route, reason, cx) {
        cx.defer(move |cx| {
            if let Some(payload) = retire_route_cleanup(route, cx) {
                resume_unwind(payload);
            }
        });
    }
}

fn retain_retired_route_release(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    route: DockNativeCapturedDragRoute,
    reason: PointerCancelReason,
    cx: &mut App,
) -> Option<DockNativeCapturedDragRoute> {
    let Some(key) = DockNativeCapturedDragRetiredKey::for_route(&route) else {
        return Some(route);
    };
    let terminal_state = Rc::downgrade(state);
    let Some(barrier) = cx.cancel_native_captured_drag_with_release_barrier(
        key.source_window,
        key.drag_generation,
        reason,
        move |barrier, _, cx| {
            let Some(state) = terminal_state.upgrade() else {
                return;
            };
            let Some(pending) = clear_retired_pending_if_matches(&state, key, barrier) else {
                return;
            };
            if let Some(route) = pending.route
                && let Some(payload) = retire_route_cleanup(route, cx)
            {
                resume_unwind(payload);
            }
        },
    ) else {
        return Some(route);
    };

    insert_retired_pending(state, key, barrier, Some(route));
    None
}

fn insert_retired_pending(
    state: &Rc<RefCell<DockNativeCapturedDragState>>,
    key: DockNativeCapturedDragRetiredKey,
    barrier: NativeCapturedDragReleaseBarrier,
    route: Option<DockNativeCapturedDragRoute>,
) {
    let mut state = state.borrow_mut();
    match state.retired_pending.entry(key) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(DockNativeCapturedDragRetiredPending { barrier, route });
        }
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            debug_assert_eq!(
                entry.get().barrier,
                barrier,
                "one retired Dock route generation must retain one exact native release barrier"
            );
            if entry.get().barrier == barrier && entry.get().route.is_none() {
                entry.get_mut().route = route;
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
        route
            .runtime
            .finish_payload_drag_with_app(&route.session, cx);
    });
    first_panic
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
    event: &NativeCapturedDragEvent,
    target: &DockNativeCapturedHostTarget,
) -> Option<DockCapturedNativeHostTarget> {
    let target_window = event.window_hit_stack().first_registered_window()?;
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
    let Some(captured_target) = captured_host_target(event, &target) else {
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

trait PlatformWindowHitStackExt {
    fn first_registered_window(&self) -> Option<AnyWindowHandle>;
}

impl PlatformWindowHitStackExt for PlatformWindowHitStack {
    fn first_registered_window(&self) -> Option<AnyWindowHandle> {
        match self {
            Self::Available(observation) => match observation.hits().first()? {
                PlatformWindowHit::RegisteredApplication { window, .. } => Some(*window),
                PlatformWindowHit::OpaqueBarrier { .. } => None,
            },
            Self::Unavailable => None,
        }
    }
}
