use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crate::{
    AnyView, AppContext as _, Bounds, Context, Empty, Entity, Hitbox, HitboxBehavior,
    InteractiveElement as _, IntoElement, ParentElement as _, PortalAnchorError,
    PortalAnchorExt as _, PortalAnchorHandle, PortalAnchorSnapshot, Render, StyleRefinement,
    Styled as _, SubtreePresentation, SubtreePresentationExt as _, SubtreeTransform,
    SubtreeTransformExt as _, SubtreeTransformOrigin, TestAppContext, VisualContext as _, Window,
    anchored, canvas, deferred, div, fill, point, portal_anchor_follower, px, red, size,
    window_portal,
};

#[open_gpui::test]
fn deferred_depth_limit_accepts_a_chain_that_finishes_on_the_tenth_round(cx: &mut TestAppContext) {
    struct TenRoundDeferredView;

    impl Render for TenRoundDeferredView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let mut child = div()
                .debug_selector(|| "ten-round-deferred-leaf".to_owned())
                .w(px(10.0))
                .h(px(10.0))
                .into_any_element();
            for _ in 0..10 {
                child = deferred(child).into_any_element();
            }
            child
        }
    }

    let (_, cx) = cx.add_window_view(|_, _| TenRoundDeferredView);
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(cx.debug_bounds("ten-round-deferred-leaf").is_some());
}

#[derive(Clone, Copy)]
enum AnchorTargetOrder {
    BeforeFollowers,
    AfterFollowers,
}

struct AnchorLifecycleView {
    handle: PortalAnchorHandle,
    target_order: AnchorTargetOrder,
    target_present: bool,
    target_presentation: SubtreePresentation,
    follower_count: usize,
    observations: Rc<RefCell<Vec<Option<PortalAnchorSnapshot>>>>,
}

impl Render for AnchorLifecycleView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div().relative().size_full();
        let target = self.target_present.then(|| {
            div()
                .absolute()
                .left(px(20.0))
                .top(px(30.0))
                .w(px(100.0))
                .h(px(50.0))
                .track_portal_anchor(&self.handle)
                .with_subtree_presentation(self.target_presentation)
                .into_any_element()
        });
        let followers = (0..self.follower_count)
            .map(|_| {
                let handle = self.handle;
                let observations = self.observations.clone();
                canvas(
                    move |_, window, _| {
                        let snapshot = window
                            .resolve_portal_anchor(&handle, |snapshot, _| snapshot)
                            .expect("same-window portal anchor resolution should succeed");
                        observations.borrow_mut().push(snapshot);
                    },
                    |_, _, _, _| {},
                )
                .w(px(1.0))
                .h(px(1.0))
                .into_any_element()
            })
            .collect::<Vec<_>>();

        match self.target_order {
            AnchorTargetOrder::BeforeFollowers => {
                if let Some(target) = target {
                    root = root.child(target);
                }
                for follower in followers {
                    root = root.child(follower);
                }
            }
            AnchorTargetOrder::AfterFollowers => {
                for follower in followers {
                    root = root.child(follower);
                }
                if let Some(target) = target {
                    root = root.child(target);
                }
            }
        }
        root
    }
}

struct InnerScopedAnchorView {
    handle: PortalAnchorHandle,
    presentation: SubtreePresentation,
}

impl Render for InnerScopedAnchorView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .left(px(20.0))
            .top(px(30.0))
            .w(px(100.0))
            .h(px(50.0))
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2.0, 4.0),
                    point(px(100.0), px(50.0)),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .expect("the inner target transform should be valid"),
            )
            .with_subtree_presentation(self.presentation)
            .track_portal_anchor(&self.handle)
    }
}

struct DirectBindingView {
    handle: PortalAnchorHandle,
    rollback_first: bool,
    bind_results: Rc<RefCell<Vec<Result<(), PortalAnchorError>>>>,
    resolved: Rc<RefCell<Option<PortalAnchorSnapshot>>>,
}

impl Render for DirectBindingView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle;
        let rollback_first = self.rollback_first;
        let bind_results = self.bind_results.clone();
        let resolved = self.resolved.clone();
        canvas(
            move |bounds, window, _| {
                if rollback_first {
                    let rejected: Result<(), ()> = window.transact(|window| {
                        window
                            .bind_portal_anchor(&handle, bounds)
                            .expect("the rolled-back binding should initially be unique");
                        Err(())
                    });
                    assert!(rejected.is_err());
                }

                bind_results
                    .borrow_mut()
                    .push(window.bind_portal_anchor(&handle, bounds));
                if !rollback_first {
                    bind_results
                        .borrow_mut()
                        .push(window.bind_portal_anchor(&handle, bounds));
                }
                *resolved.borrow_mut() = window
                    .resolve_portal_anchor(&handle, |snapshot, _| snapshot)
                    .expect("the direct binding should remain resolvable");
            },
            |_, _, _, _| {},
        )
        .w(px(20.0))
        .h(px(10.0))
    }
}

struct ForeignBindingView {
    handle: PortalAnchorHandle,
    error: Rc<RefCell<Option<PortalAnchorError>>>,
}

impl Render for ForeignBindingView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle;
        let error = self.error.clone();
        canvas(
            move |bounds, window, _| {
                *error.borrow_mut() = window.bind_portal_anchor(&handle, bounds).err();
            },
            |_, _, _, _| {},
        )
        .w(px(20.0))
        .h(px(10.0))
    }
}

struct CachedAnchorChild {
    handle: PortalAnchorHandle,
    renders: Rc<Cell<usize>>,
}

impl Render for CachedAnchorChild {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().size_full().track_portal_anchor(&self.handle)
    }
}

struct CachedAnchorRoot {
    child: Entity<CachedAnchorChild>,
    handle: PortalAnchorHandle,
    translation_x: f32,
    presentation: SubtreePresentation,
    clip_width: f32,
}

struct CachedInnerScopedAnchorChild {
    renders: Rc<Cell<usize>>,
}

impl Render for CachedInnerScopedAnchorChild {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div()
            .size_full()
            .child(
                div()
                    .w(px(10.0))
                    .h(px(10.0))
                    .with_subtree_presentation(SubtreePresentation::Hidden),
            )
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2.0, 4.0),
                    point(px(100.0), px(50.0)),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .expect("the cached inner target transform should be valid"),
            )
            .with_subtree_presentation(SubtreePresentation::Inert)
    }
}

struct CachedOuterAnchorRoot {
    child: Entity<CachedInnerScopedAnchorChild>,
    handle: PortalAnchorHandle,
}

impl Render for CachedOuterAnchorRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().relative().size_full().child(
            AnyView::from(self.child.clone())
                .cached(StyleRefinement::default().w(px(100.0)).h(px(50.0)))
                .track_portal_anchor(&self.handle),
        )
    }
}

struct CachedPortalFollower {
    handle: PortalAnchorHandle,
    renders: Rc<Cell<usize>>,
}

impl Render for CachedPortalFollower {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        portal_anchor_follower(&self.handle, |snapshot, _, _| {
            snapshot.map(|_| {
                div()
                    .debug_selector(|| "cached-portal-follower-surface".to_owned())
                    .w(px(12.0))
                    .h(px(12.0))
                    .into_any_element()
            })
        })
    }
}

struct CachedDirectDeferredFollower {
    handle: PortalAnchorHandle,
    renders: Rc<Cell<usize>>,
    observations: Rc<RefCell<Vec<Option<PortalAnchorSnapshot>>>>,
}

impl Render for CachedDirectDeferredFollower {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let handle = self.handle;
        let observations = self.observations.clone();
        deferred(
            canvas(
                move |_, window, _| {
                    observations.borrow_mut().push(
                        window
                            .resolve_portal_anchor(&handle, |snapshot, _| snapshot)
                            .expect("cached direct deferred resolution should remain valid"),
                    );
                },
                |_, _, _, _| {},
            )
            .w(px(1.0))
            .h(px(1.0)),
        )
    }
}

struct CachedFollowerRoot {
    handle: PortalAnchorHandle,
    target_present: bool,
    follower: Entity<CachedPortalFollower>,
    direct_follower: Entity<CachedDirectDeferredFollower>,
}

impl Render for CachedFollowerRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div().relative().size_full();
        if self.target_present {
            root = root.child(
                div()
                    .absolute()
                    .left(px(20.0))
                    .top(px(30.0))
                    .w(px(100.0))
                    .h(px(50.0))
                    .track_portal_anchor(&self.handle),
            );
        }
        root.child(
            AnyView::from(self.follower.clone())
                .cached(StyleRefinement::default().w(px(1.0)).h(px(1.0))),
        )
        .child(
            AnyView::from(self.direct_follower.clone())
                .cached(StyleRefinement::default().w(px(1.0)).h(px(1.0))),
        )
    }
}

#[derive(Clone, Copy)]
enum DuplicateReplayOrder {
    LiveBeforeCache,
    CacheBeforeLive,
}

struct CachedDuplicateRoot {
    child: Entity<CachedAnchorChild>,
    handle: PortalAnchorHandle,
    live_enabled: bool,
    order: DuplicateReplayOrder,
    bind_errors: Rc<RefCell<Vec<Option<PortalAnchorError>>>>,
    resolve_errors: Rc<RefCell<Vec<Option<PortalAnchorError>>>>,
}

impl Render for CachedDuplicateRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let cached = AnyView::from(self.child.clone())
            .cached(StyleRefinement::default().w(px(20.0)).h(px(10.0)))
            .into_any_element();
        let live = self.live_enabled.then(|| {
            let handle = self.handle;
            let bind_errors = self.bind_errors.clone();
            canvas(
                move |bounds, window, _| {
                    bind_errors
                        .borrow_mut()
                        .push(window.bind_portal_anchor(&handle, bounds).err());
                },
                |_, _, _, _| {},
            )
            .absolute()
            .w(px(20.0))
            .h(px(10.0))
            .into_any_element()
        });
        let handle = self.handle;
        let resolve_errors = self.resolve_errors.clone();
        let follower = canvas(
            move |_, window, _| {
                resolve_errors
                    .borrow_mut()
                    .push(window.resolve_portal_anchor(&handle, |_, _| ()).err());
            },
            |_, _, _, _| {},
        )
        .absolute()
        .w(px(1.0))
        .h(px(1.0));

        let mut root = div().relative().size_full();
        match self.order {
            DuplicateReplayOrder::LiveBeforeCache => {
                if let Some(live) = live {
                    root = root.child(live);
                }
                root = root.child(cached);
            }
            DuplicateReplayOrder::CacheBeforeLive => {
                root = root.child(cached);
                if let Some(live) = live {
                    root = root.child(live);
                }
            }
        }
        root.child(follower)
    }
}

impl Render for CachedAnchorRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let transform = SubtreeTransform::try_new(
            size(2.0, 4.0),
            point(px(self.translation_x), px(50.0)),
            SubtreeTransformOrigin::TOP_LEFT,
        )
        .expect("the cache test transform should be valid");
        div()
            .relative()
            .w(px(self.clip_width))
            .h(px(100.0))
            .overflow_hidden()
            .child(
                AnyView::from(self.child.clone())
                    .cached(StyleRefinement::default().w(px(100.0)).h(px(50.0)))
                    .with_subtree_transform(transform)
                    .with_subtree_presentation(self.presentation),
            )
    }
}

struct AnchorValidityView {
    handle: PortalAnchorHandle,
    target_fails_late: bool,
    follower_fails_late: bool,
    first_commits: Rc<Cell<usize>>,
    second_commits: Rc<Cell<usize>>,
}

struct PortalFollowerGeometryView {
    handle: PortalAnchorHandle,
    observed: Rc<RefCell<Option<PortalAnchorSnapshot>>>,
    surface_hitbox: Rc<RefCell<Option<Hitbox>>>,
}

impl Render for PortalFollowerGeometryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle;
        let observed = self.observed.clone();
        let surface_hitbox = self.surface_hitbox.clone();
        let follower = portal_anchor_follower(&handle, move |snapshot, _, _| {
            *observed.borrow_mut() = snapshot;
            snapshot.map(|snapshot| {
                let position = snapshot.geometry().displayed_bounds().bottom_right();
                anchored()
                    .position(position)
                    .child(
                        canvas(
                            move |bounds, window, _| {
                                *surface_hitbox.borrow_mut() =
                                    Some(window.insert_hitbox(bounds, HitboxBehavior::Normal));
                            },
                            |_, _, _, _| {},
                        )
                        .w(px(10.0))
                        .h(px(10.0)),
                    )
                    .into_any_element()
            })
        })
        .priority(7);
        let target = div()
            .absolute()
            .left(px(20.0))
            .top(px(30.0))
            .w(px(100.0))
            .h(px(50.0))
            .track_portal_anchor(&handle);

        div()
            .relative()
            .w(px(40.0))
            .h(px(40.0))
            .overflow_hidden()
            .child(follower)
            .child(target)
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2.0, 4.0),
                    point(px(100.0), px(50.0)),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .expect("the portal follower transform should be valid"),
            )
    }
}

struct DeferredClipView {
    inherited: Rc<RefCell<Option<Hitbox>>>,
    portal: Rc<RefCell<Option<Hitbox>>>,
}

impl Render for DeferredClipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let inherited = self.inherited.clone();
        let portal = self.portal.clone();
        div()
            .w(px(40.0))
            .h(px(30.0))
            .overflow_hidden()
            .child(deferred(
                canvas(
                    move |bounds, window, _| {
                        *inherited.borrow_mut() =
                            Some(window.insert_hitbox(bounds, HitboxBehavior::Normal));
                    },
                    |_, _, _, _| {},
                )
                .w(px(100.0))
                .h(px(100.0)),
            ))
            .child(window_portal(
                canvas(
                    move |bounds, window, _| {
                        *portal.borrow_mut() =
                            Some(window.insert_hitbox(bounds, HitboxBehavior::Normal));
                    },
                    |_, _, _, _| {},
                )
                .w(px(100.0))
                .h(px(100.0)),
            ))
    }
}

impl Render for AnchorValidityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let target_fails_late = self.target_fails_late;
        let target = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                window.paint_quad(fill(bounds, red()));
                if target_fails_late {
                    window.paint_quad(fill(
                        Bounds::new(point(px(f32::MAX), px(0.0)), size(px(10.0), px(10.0))),
                        red(),
                    ));
                }
            },
        )
        .w(px(20.0))
        .h(px(10.0))
        .track_portal_anchor(&self.handle)
        .with_subtree_transform(
            SubtreeTransform::try_new(
                size(2.0, 2.0),
                point(px(0.0), px(0.0)),
                SubtreeTransformOrigin::TOP_LEFT,
            )
            .expect("the target transform should be valid"),
        );

        let first_handle = self.handle;
        let first_commits = self.first_commits.clone();
        let follower_fails_late = self.follower_fails_late;
        let first_follower = canvas(
            move |_, window, _| {
                window
                    .resolve_portal_anchor(&first_handle, |snapshot, window| {
                        assert!(snapshot.is_some());
                        let first_commits = first_commits.clone();
                        window.record_prepaint_commit(move |_, _| {
                            first_commits.set(first_commits.get() + 1);
                        });
                    })
                    .expect("the first follower should resolve its target");
            },
            move |_, _, window, _| {
                if follower_fails_late {
                    window
                        .resolve_portal_anchor(&first_handle, |snapshot, window| {
                            assert!(snapshot.is_some());
                            window.paint_quad(fill(
                                Bounds::new(point(px(f32::MAX), px(0.0)), size(px(10.0), px(10.0))),
                                red(),
                            ));
                        })
                        .expect("the failing follower should retain target access");
                }
            },
        )
        .w(px(1.0))
        .h(px(1.0))
        .with_subtree_transform(
            SubtreeTransform::try_new(
                size(2.0, 2.0),
                point(px(0.0), px(0.0)),
                SubtreeTransformOrigin::TOP_LEFT,
            )
            .expect("the follower transform should be valid"),
        );

        let second_handle = self.handle;
        let second_commits = self.second_commits.clone();
        let second_follower = canvas(
            move |_, window, _| {
                window
                    .resolve_portal_anchor(&second_handle, |snapshot, window| {
                        assert!(snapshot.is_some());
                        let second_commits = second_commits.clone();
                        window.record_prepaint_commit(move |_, _| {
                            second_commits.set(second_commits.get() + 1);
                        });
                    })
                    .expect("the second follower should resolve its target");
            },
            |_, _, _, _| {},
        )
        .w(px(1.0))
        .h(px(1.0));

        div()
            .relative()
            .size_full()
            .child(target)
            .child(first_follower)
            .child(second_follower)
    }
}

fn resolve_committed(
    cx: &mut crate::VisualTestContext,
    handle: PortalAnchorHandle,
) -> Option<PortalAnchorSnapshot> {
    cx.update(|window, _| {
        window
            .resolve_portal_anchor(&handle, |snapshot, _| snapshot)
            .expect("same-window committed resolution should succeed")
    })
}

#[open_gpui::test]
fn portal_anchor_candidate_order_and_multiple_followers_are_explicit(cx: &mut TestAppContext) {
    let observations = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |window, _| AnchorLifecycleView {
            handle: window.new_portal_anchor(),
            target_order: AnchorTargetOrder::AfterFollowers,
            target_present: true,
            target_presentation: SubtreePresentation::Visible,
            follower_count: 1,
            observations,
        }
    });

    observations.borrow_mut().clear();
    cx.update(|window, cx| window.draw(cx).clear());
    let handle = cx.update_window_entity(&view, |view, _, _| view.handle);
    assert_eq!(observations.borrow().as_slice(), &[None]);
    let first_committed =
        resolve_committed(cx, handle).expect("the completed target should commit");

    observations.borrow_mut().clear();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        observations.borrow().as_slice(),
        &[None],
        "a current-frame miss must not fall back to the previous committed target"
    );
    let second_committed =
        resolve_committed(cx, handle).expect("the later target should still commit");
    assert!(second_committed.frame_generation() > first_committed.frame_generation());

    cx.update_window_entity(&view, |view, _, cx| {
        view.target_order = AnchorTargetOrder::BeforeFollowers;
        view.follower_count = 2;
        cx.notify();
    });
    observations.borrow_mut().clear();
    cx.update(|window, cx| window.draw(cx).clear());

    let observations = observations.borrow();
    assert_eq!(observations.len(), 2);
    assert!(observations.iter().all(Option::is_some));
    assert_eq!(observations[0], observations[1]);
    assert!(
        observations[0]
            .expect("the current candidate should exist")
            .frame_generation()
            > second_committed.frame_generation()
    );
    assert_eq!(
        observations[0],
        resolve_committed(cx, handle),
        "the candidate must become the exact committed snapshot"
    );
}

#[open_gpui::test]
fn portal_anchor_dependent_cached_views_reresolve_after_target_unmount(cx: &mut TestAppContext) {
    let follower_renders = Rc::new(Cell::new(0));
    let direct_renders = Rc::new(Cell::new(0));
    let direct_observations = Rc::new(RefCell::new(Vec::new()));
    let (root, cx) = cx.add_window_view({
        let follower_renders = follower_renders.clone();
        let direct_renders = direct_renders.clone();
        let direct_observations = direct_observations.clone();
        move |window, cx| {
            let handle = window.new_portal_anchor();
            CachedFollowerRoot {
                handle,
                target_present: true,
                follower: cx.new(|_| CachedPortalFollower {
                    handle,
                    renders: follower_renders,
                }),
                direct_follower: cx.new(|_| CachedDirectDeferredFollower {
                    handle,
                    renders: direct_renders,
                    observations: direct_observations,
                }),
            }
        }
    });
    let handle = cx.update_window_entity(&root, |root, _, _| root.handle);

    cx.update(|window, cx| window.draw(cx).clear());
    assert!(resolve_committed(cx, handle).is_some());
    assert!(cx.debug_bounds("cached-portal-follower-surface").is_some());
    assert!(direct_observations.borrow().last().unwrap().is_some());
    for _ in 0..2 {
        let prior_follower_renders = follower_renders.get();
        let prior_direct_renders = direct_renders.get();
        let prior_direct_observations = direct_observations.borrow().len();
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(resolve_committed(cx, handle).is_some());
        assert!(cx.debug_bounds("cached-portal-follower-surface").is_some());
        assert!(follower_renders.get() > prior_follower_renders);
        assert!(direct_renders.get() > prior_direct_renders);
        assert!(direct_observations.borrow().len() > prior_direct_observations);
        assert!(direct_observations.borrow().last().unwrap().is_some());
    }
    let linked_follower_renders = follower_renders.get();
    let linked_direct_renders = direct_renders.get();
    let linked_direct_observations = direct_observations.borrow().len();

    cx.update_window_entity(&root, |root, _, cx| {
        root.target_present = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(resolve_committed(cx, handle), None);
    assert!(
        cx.debug_bounds("cached-portal-follower-surface").is_none(),
        "a cached follower must not replay its prior surface after the target unmounts"
    );
    assert!(follower_renders.get() > linked_follower_renders);
    assert!(direct_renders.get() > linked_direct_renders);
    assert!(direct_observations.borrow().len() > linked_direct_observations);
    assert_eq!(*direct_observations.borrow().last().unwrap(), None);

    cx.update_window_entity(&root, |root, _, cx| {
        root.target_present = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(resolve_committed(cx, handle).is_some());
    assert!(cx.debug_bounds("cached-portal-follower-surface").is_some());
    assert!(direct_observations.borrow().last().unwrap().is_some());
}

#[open_gpui::test]
fn cached_and_live_bindings_share_one_duplicate_authority(cx: &mut TestAppContext) {
    for order in [
        DuplicateReplayOrder::LiveBeforeCache,
        DuplicateReplayOrder::CacheBeforeLive,
    ] {
        let renders = Rc::new(Cell::new(0));
        let bind_errors = Rc::new(RefCell::new(Vec::new()));
        let resolve_errors = Rc::new(RefCell::new(Vec::new()));
        let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
            let renders = renders.clone();
            let bind_errors = bind_errors.clone();
            let resolve_errors = resolve_errors.clone();
            move |window, cx| {
                let handle = window.new_portal_anchor();
                CachedDuplicateRoot {
                    child: cx.new(|_| CachedAnchorChild { handle, renders }),
                    handle,
                    live_enabled: false,
                    order,
                    bind_errors,
                    resolve_errors,
                }
            }
        });
        let view = typed_window
            .root(cx)
            .expect("the duplicate replay view should exist");
        let window: crate::AnyWindowHandle = typed_window.into();

        bind_errors.borrow_mut().clear();
        resolve_errors.borrow_mut().clear();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear())
            .expect("the duplicate replay window should remain open");
        let initial_renders = renders.get();

        view.update(cx, |view, cx| {
            view.live_enabled = true;
            cx.notify();
        });
        bind_errors.borrow_mut().clear();
        resolve_errors.borrow_mut().clear();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear())
            .expect("the duplicate replay window should remain open");

        assert_eq!(
            renders.get(),
            initial_renders,
            "the cached target must be replayed instead of rebuilt"
        );
        match order {
            DuplicateReplayOrder::LiveBeforeCache => {
                assert_eq!(bind_errors.borrow().as_slice(), &[None]);
                assert!(matches!(
                    resolve_errors.borrow().as_slice(),
                    [Some(PortalAnchorError::HandleAlreadyBound { .. })]
                ));
            }
            DuplicateReplayOrder::CacheBeforeLive => {
                assert!(matches!(
                    bind_errors.borrow().as_slice(),
                    [Some(PortalAnchorError::HandleAlreadyBound { .. })]
                ));
                assert_eq!(resolve_errors.borrow().as_slice(), &[None]);
            }
        }
    }
}

#[open_gpui::test]
fn completed_frames_unlink_hidden_or_absent_targets_and_allow_rebinding(cx: &mut TestAppContext) {
    let observations = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |window, _| AnchorLifecycleView {
            handle: window.new_portal_anchor(),
            target_order: AnchorTargetOrder::BeforeFollowers,
            target_present: true,
            target_presentation: SubtreePresentation::Inert,
            follower_count: 1,
            observations,
        }
    });
    let handle = cx.update_window_entity(&view, |view, _, _| view.handle);

    cx.update(|window, cx| window.draw(cx).clear());
    let inert =
        resolve_committed(cx, handle).expect("an inert target remains geometrically linked");
    assert_eq!(inert.presentation(), SubtreePresentation::Inert);

    cx.update_window_entity(&view, |view, _, cx| {
        view.target_presentation = SubtreePresentation::Hidden;
        cx.notify();
    });
    observations.borrow_mut().clear();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(observations.borrow().as_slice(), &[None]);
    assert_eq!(resolve_committed(cx, handle), None);

    cx.update_window_entity(&view, |view, _, cx| {
        view.target_presentation = SubtreePresentation::Visible;
        view.target_present = false;
        cx.notify();
    });
    observations.borrow_mut().clear();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(observations.borrow().as_slice(), &[None]);
    assert_eq!(resolve_committed(cx, handle), None);

    cx.update_window_entity(&view, |view, _, cx| {
        view.target_present = true;
        cx.notify();
    });
    observations.borrow_mut().clear();
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(observations.borrow()[0].is_some());
    assert!(resolve_committed(cx, handle).is_some());
}

#[open_gpui::test]
fn portal_anchor_captures_target_scopes_independent_of_wrapper_order(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, _| InnerScopedAnchorView {
        handle: window.new_portal_anchor(),
        presentation: SubtreePresentation::Inert,
    });
    let handle = cx.update_window_entity(&view, |view, _, _| view.handle);

    cx.update(|window, cx| window.draw(cx).clear());
    let inert = resolve_committed(cx, handle).expect("the inner-scoped target should commit");
    assert_eq!(inert.presentation(), SubtreePresentation::Inert);
    assert_eq!(
        inert.geometry().displayed_bounds(),
        Bounds::new(point(px(100.0), px(50.0)), size(px(200.0), px(200.0)))
    );

    cx.update_window_entity(&view, |view, _, cx| {
        view.presentation = SubtreePresentation::Hidden;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(resolve_committed(cx, handle), None);
}

#[open_gpui::test]
fn duplicate_binding_is_typed_and_failed_transactions_restore_uniqueness(cx: &mut TestAppContext) {
    let duplicate_results = Rc::new(RefCell::new(Vec::new()));
    let duplicate_resolved = Rc::new(RefCell::new(None));
    let (duplicate_view, cx) = cx.add_window_view({
        let duplicate_results = duplicate_results.clone();
        let duplicate_resolved = duplicate_resolved.clone();
        move |window, _| DirectBindingView {
            handle: window.new_portal_anchor(),
            rollback_first: false,
            bind_results: duplicate_results,
            resolved: duplicate_resolved,
        }
    });
    duplicate_results.borrow_mut().clear();
    cx.update(|window, cx| window.draw(cx).clear());
    let duplicate_handle = cx.update_window_entity(&duplicate_view, |view, _, _| view.handle);
    assert_eq!(duplicate_results.borrow()[0], Ok(()));
    assert_eq!(
        duplicate_results.borrow()[1],
        Err(PortalAnchorError::HandleAlreadyBound {
            handle: duplicate_handle
        })
    );
    assert!(duplicate_resolved.borrow().is_some());

    let rollback_results = Rc::new(RefCell::new(Vec::new()));
    let rollback_resolved = Rc::new(RefCell::new(None));
    let rollback_window: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let rollback_results = rollback_results.clone();
            let rollback_resolved = rollback_resolved.clone();
            move |window, _| DirectBindingView {
                handle: window.new_portal_anchor(),
                rollback_first: true,
                bind_results: rollback_results,
                resolved: rollback_resolved,
            }
        })
        .into();
    rollback_results.borrow_mut().clear();
    cx.update_window(rollback_window, |_, window, cx| window.draw(cx).clear())
        .expect("the rollback test window should remain open");
    assert_eq!(rollback_results.borrow().as_slice(), &[Ok(())]);
    assert!(rollback_resolved.borrow().is_some());
}

#[open_gpui::test]
fn portal_anchor_handles_cannot_cross_windows(cx: &mut TestAppContext) {
    let first_handle = Rc::new(Cell::new(None));
    let first_window: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let first_handle = first_handle.clone();
            move |window, _| {
                first_handle.set(Some(window.new_portal_anchor()));
                Empty
            }
        })
        .into();
    let handle = first_handle
        .get()
        .expect("the first window should create a portal anchor handle");
    let bind_error = Rc::new(RefCell::new(None));
    let second_window: crate::AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), {
            let bind_error = bind_error.clone();
            move |_, _| ForeignBindingView {
                handle,
                error: bind_error,
            }
        })
        .into();

    cx.update_window(second_window, |_, window, cx| window.draw(cx).clear())
        .expect("the second window should remain open");
    assert!(matches!(
        *bind_error.borrow(),
        Some(PortalAnchorError::WrongWindow { .. })
    ));
    assert!(matches!(
        cx.update_window(second_window, |_, window, _| window
            .resolve_portal_anchor(&handle, |snapshot, _| snapshot)),
        Ok(Err(PortalAnchorError::WrongWindow { .. }))
    ));
    assert_ne!(first_window, second_window);
}

#[open_gpui::test]
fn cached_anchor_journals_refresh_generation_and_rebuild_for_ancestor_facts(
    cx: &mut TestAppContext,
) {
    let renders = Rc::new(Cell::new(0));
    let (root, cx) = cx.add_window_view({
        let renders = renders.clone();
        move |window, cx| {
            let handle = window.new_portal_anchor();
            CachedAnchorRoot {
                child: cx.new(|_| CachedAnchorChild { handle, renders }),
                handle,
                translation_x: 100.0,
                presentation: SubtreePresentation::Visible,
                clip_width: 100.0,
            }
        }
    });
    let handle = cx.update_window_entity(&root, |root, _, _| root.handle);

    cx.update(|window, cx| window.draw(cx).clear());
    let first = resolve_committed(cx, handle).expect("the first cached anchor should commit");
    let first_renders = renders.get();
    assert_eq!(
        first.geometry().displayed_bounds().origin,
        point(px(100.0), px(50.0))
    );

    cx.update(|window, cx| window.draw(cx).clear());
    let replayed = resolve_committed(cx, handle).expect("the cached anchor should replay");
    assert_eq!(renders.get(), first_renders);
    assert!(replayed.frame_generation() > first.frame_generation());
    assert_eq!(replayed.geometry(), first.geometry());
    assert_eq!(
        replayed.effective_clip_bounds(),
        first.effective_clip_bounds()
    );

    cx.update_window_entity(&root, |root, _, cx| {
        root.translation_x = 200.0;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let translated = resolve_committed(cx, handle).expect("the translated anchor should commit");
    assert!(renders.get() > first_renders);
    assert_eq!(
        translated.geometry().displayed_bounds().origin,
        first.geometry().displayed_bounds().origin + point(px(100.0), px(0.0))
    );

    let translated_renders = renders.get();
    cx.update_window_entity(&root, |root, _, cx| {
        root.presentation = SubtreePresentation::Inert;
        root.clip_width = 40.0;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let constrained = resolve_committed(cx, handle).expect("the inert anchor should remain linked");
    assert!(renders.get() > translated_renders);
    assert_eq!(constrained.presentation(), SubtreePresentation::Inert);
    assert!(
        constrained.effective_clip_bounds().size.width
            < translated.effective_clip_bounds().size.width
    );
}

#[open_gpui::test]
fn outer_anchor_tracker_refreshes_cached_inner_target_scopes(cx: &mut TestAppContext) {
    let renders = Rc::new(Cell::new(0));
    let (root, cx) = cx.add_window_view({
        let renders = renders.clone();
        move |window, cx| CachedOuterAnchorRoot {
            child: cx.new(|_| CachedInnerScopedAnchorChild { renders }),
            handle: window.new_portal_anchor(),
        }
    });
    let handle = cx.update_window_entity(&root, |root, _, _| root.handle);

    cx.update(|window, cx| window.draw(cx).clear());
    let first = resolve_committed(cx, handle).expect("the cached inner target should commit");
    assert_eq!(first.presentation(), SubtreePresentation::Inert);
    assert_eq!(
        first.geometry().displayed_bounds(),
        Bounds::new(point(px(100.0), px(50.0)), size(px(200.0), px(200.0)))
    );
    let first_renders = renders.get();

    cx.update(|window, cx| window.draw(cx).clear());
    let second = resolve_committed(cx, handle).expect("the cached inner target should stay linked");
    assert!(second.frame_generation() > first.frame_generation());
    assert_eq!(second.presentation(), first.presentation());
    assert_eq!(second.geometry(), first.geometry());
    assert!(
        renders.get() > first_renders,
        "an outer tracker must bypass cache replay to recapture target-root scopes"
    );
}

#[open_gpui::test]
fn target_late_failure_suppresses_all_same_frame_followers(cx: &mut TestAppContext) {
    let first_commits = Rc::new(Cell::new(0));
    let second_commits = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view({
        let first_commits = first_commits.clone();
        let second_commits = second_commits.clone();
        move |window, _| AnchorValidityView {
            handle: window.new_portal_anchor(),
            target_fails_late: true,
            follower_fails_late: false,
            first_commits,
            second_commits,
        }
    });
    let handle = cx.update_window_entity(&view, |view, _, _| view.handle);

    first_commits.set(0);
    second_commits.set(0);
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(first_commits.get(), 0);
    assert_eq!(second_commits.get(), 0);
    assert_eq!(resolve_committed(cx, handle), None);
}

#[open_gpui::test]
fn follower_late_failure_does_not_invalidate_target_or_siblings(cx: &mut TestAppContext) {
    let first_commits = Rc::new(Cell::new(0));
    let second_commits = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view({
        let first_commits = first_commits.clone();
        let second_commits = second_commits.clone();
        move |window, _| AnchorValidityView {
            handle: window.new_portal_anchor(),
            target_fails_late: false,
            follower_fails_late: true,
            first_commits,
            second_commits,
        }
    });
    let handle = cx.update_window_entity(&view, |view, _, _| view.handle);

    first_commits.set(0);
    second_commits.set(0);
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(first_commits.get(), 1);
    assert_eq!(second_commits.get(), 1);
    assert!(resolve_committed(cx, handle).is_some());
}

#[open_gpui::test]
fn deferred_follower_waits_for_later_targets_and_resets_geometry_and_clip(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(None));
    let surface_hitbox = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view({
        let observed = observed.clone();
        let surface_hitbox = surface_hitbox.clone();
        move |window, _| PortalFollowerGeometryView {
            handle: window.new_portal_anchor(),
            observed,
            surface_hitbox,
        }
    });
    let handle = cx.update_window_entity(&view, |view, _, _| view.handle);

    *observed.borrow_mut() = None;
    *surface_hitbox.borrow_mut() = None;
    cx.update(|window, cx| window.draw(cx).clear());
    let viewport_bounds =
        cx.update(|window, _| Bounds::new(point(px(0.0), px(0.0)), window.viewport_size()));

    let snapshot = observed
        .borrow()
        .expect("the deferred follower should observe the later ordinary target");
    let hitbox = surface_hitbox.borrow();
    let hitbox = hitbox
        .as_ref()
        .expect("the eligible follower should emit a surface hitbox");
    assert_eq!(
        hitbox.displayed_bounds().origin,
        snapshot.geometry().displayed_bounds().bottom_right()
    );
    assert_eq!(hitbox.displayed_bounds(), hitbox.layout_bounds());
    assert_eq!(hitbox.displayed_clip_bounds(), viewport_bounds);
    assert!(snapshot.effective_clip_bounds().right() < hitbox.displayed_clip_bounds().right());
    assert_eq!(resolve_committed(cx, handle), Some(snapshot));
}

#[open_gpui::test]
fn ordinary_deferred_inherits_effective_clip_while_window_portal_resets_it(
    cx: &mut TestAppContext,
) {
    let inherited = Rc::new(RefCell::new(None));
    let portal = Rc::new(RefCell::new(None));
    let _window = cx.open_window(size(px(320.0), px(200.0)), {
        let inherited = inherited.clone();
        let portal = portal.clone();
        move |_, _| DeferredClipView { inherited, portal }
    });
    cx.run_until_parked();

    let inherited = inherited.borrow();
    let inherited = inherited
        .as_ref()
        .expect("the ordinary deferred hitbox should exist");
    assert_eq!(
        inherited.displayed_clip_bounds().size,
        size(px(40.0), px(30.0))
    );
    let portal = portal.borrow();
    let portal = portal
        .as_ref()
        .expect("the window portal hitbox should exist");
    assert_eq!(
        portal.displayed_clip_bounds(),
        Bounds::new(point(px(0.0), px(0.0)), size(px(320.0), px(200.0)))
    );
}
