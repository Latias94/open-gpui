use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use accesskit::{Action as AccessibleAction, ActionRequest, TreeId};

use crate::{
    AppContext, Bounds, Context, Corners, Hitbox, HitboxBehavior, HitboxId, InteractiveElement,
    IntoElement, ListAlignment, ListHorizontalSizingBehavior, ListState, Modifiers, ParentElement,
    Pixels, Point, PortalAnchorExt, PortalAnchorHandle, Render, Role, StatefulInteractiveElement,
    Styled, SubtreeClip, SubtreeClipExt, TestAppContext, UniformListScrollHandle, Window, canvas,
    deferred, div, list, point, portal_anchor_follower, px, rems, size, uniform_list,
    window_portal,
};

fn circular_clip(radius: f32) -> SubtreeClip {
    let radius = size(px(radius), px(radius));
    SubtreeClip::try_own_rounded_border_box(Corners {
        top_left: radius,
        top_right: radius,
        bottom_right: radius,
        bottom_left: radius,
    })
    .unwrap()
}

struct RoundedClipAccessibilityView {
    activations: usize,
}

struct PublicRoundedClipView {
    clicks: Rc<Cell<usize>>,
    layout_bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,
}

struct OverflowAccessibilityView {
    scrollable: bool,
    clicks: Rc<Cell<usize>>,
}

struct MixedOverflowAccessibilityView;

struct DeferredClipAccessibilityView;

struct DeferredOverflowAccessibilityView;

struct NestedDeferredOverflowAccessibilityView;

struct MultipleAnonymousClipRootsAccessibilityView;

struct VirtualizedClipAccessibilityView {
    list_state: ListState,
}

struct RevealableVirtualizedAccessibilityView {
    list_state: ListState,
    uniform_scroll: UniformListScrollHandle,
    horizontal_uniform_scroll: UniformListScrollHandle,
}

struct WindowPortalAccessibilityView;

struct PortalAnchorAccessibilityView {
    handle: PortalAnchorHandle,
}

impl Render for OverflowAccessibilityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let label = if self.scrollable {
            "Scrollable offscreen accessibility node"
        } else {
            "Hidden offscreen accessibility node"
        };
        let clicks = self.clicks.clone();
        let owner_label = if self.scrollable {
            "Scrollable accessibility clip owner"
        } else {
            "Hidden accessibility clip owner"
        };
        let host = div()
            .id("overflow-accessibility-owner")
            .role(Role::Group)
            .aria_label(owner_label)
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .child(
                div().relative().w(px(300.0)).h(px(100.0)).child(
                    div()
                        .id("overflow-accessibility-target")
                        .role(Role::Button)
                        .aria_label(label)
                        .absolute()
                        .left(px(220.0))
                        .top(Pixels::ZERO)
                        .w(px(20.0))
                        .h(px(20.0))
                        .on_click(move |_, _, _| clicks.set(clicks.get() + 1)),
                ),
            );
        if self.scrollable {
            host.overflow_scroll()
        } else {
            host.overflow_hidden()
        }
    }
}

impl Render for MixedOverflowAccessibilityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("mixed-overflow-accessibility-owner")
            .role(Role::Group)
            .aria_label("Mixed-axis accessibility clip owner")
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_hidden()
            .overflow_x_scroll()
            .child(
                div()
                    .id("mixed-overflow-x-target")
                    .role(Role::Button)
                    .aria_label("Horizontally scrollable offscreen node")
                    .absolute()
                    .left(px(220.0))
                    .top(px(10.0))
                    .w(px(20.0))
                    .h(px(20.0)),
            )
            .child(
                div()
                    .id("mixed-overflow-y-target")
                    .role(Role::Button)
                    .aria_label("Vertically hidden offscreen node")
                    .absolute()
                    .left(px(10.0))
                    .top(px(220.0))
                    .w(px(20.0))
                    .h(px(20.0)),
            )
    }
}

impl Render for DeferredClipAccessibilityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .child(deferred(
                div()
                    .id("deferred-accessibility-clip-owner")
                    .role(Role::Group)
                    .aria_label("Deferred accessibility clip owner")
                    .size_full(),
            ))
            .with_subtree_clip(circular_clip(50.0))
    }
}

impl Render for DeferredOverflowAccessibilityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("deferred-overflow-accessibility-owner")
            .role(Role::Group)
            .aria_label("Deferred overflow accessibility owner")
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_scroll()
            .child(
                div()
                    .id("deferred-overflow-accessibility-before")
                    .role(Role::Button)
                    .aria_label("Deferred overflow accessibility before")
                    .w(px(20.0))
                    .h(px(20.0)),
            )
            .child(
                deferred(
                    div()
                        .id("deferred-overflow-accessibility-child")
                        .role(Role::Button)
                        .aria_label("Deferred overflow accessibility child")
                        .w(px(20.0))
                        .h(px(20.0)),
                )
                .priority(10),
            )
            .child(deferred(
                div()
                    .id("deferred-overflow-accessibility-second-child")
                    .role(Role::Button)
                    .aria_label("Deferred overflow accessibility second child")
                    .w(px(20.0))
                    .h(px(20.0)),
            ))
            .child(
                div()
                    .id("deferred-overflow-accessibility-after")
                    .role(Role::Button)
                    .aria_label("Deferred overflow accessibility after")
                    .w(px(20.0))
                    .h(px(20.0)),
            )
    }
}

impl Render for NestedDeferredOverflowAccessibilityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("nested-deferred-overflow-accessibility-owner")
            .role(Role::Group)
            .aria_label("Nested deferred overflow accessibility owner")
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_scroll()
            .child(
                div()
                    .id("nested-deferred-overflow-accessibility-before")
                    .role(Role::Button)
                    .aria_label("Nested deferred overflow accessibility before")
                    .w(px(20.0))
                    .h(px(20.0)),
            )
            .child(deferred(deferred(
                div()
                    .id("nested-deferred-overflow-accessibility-child")
                    .role(Role::Button)
                    .aria_label("Nested deferred overflow accessibility child")
                    .w(px(20.0))
                    .h(px(20.0)),
            )))
            .child(
                div()
                    .id("nested-deferred-overflow-accessibility-after")
                    .role(Role::Button)
                    .aria_label("Nested deferred overflow accessibility after")
                    .w(px(20.0))
                    .h(px(20.0)),
            )
    }
}

impl Render for MultipleAnonymousClipRootsAccessibilityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .child(
                div()
                    .id("first-anonymous-clip-root")
                    .role(Role::Group)
                    .aria_label("First anonymous clip root")
                    .w(px(20.0))
                    .h(px(20.0)),
            )
            .child(
                div()
                    .id("second-anonymous-clip-root")
                    .role(Role::Group)
                    .aria_label("Second anonymous clip root")
                    .w(px(20.0))
                    .h(px(20.0)),
            )
            .with_subtree_clip(circular_clip(50.0))
    }
}

impl Render for VirtualizedClipAccessibilityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .gap(px(10.0))
            .child(
                list(self.list_state.clone(), |index, _, _| {
                    div()
                        .id(format!("variable-list-accessibility-item-{index}"))
                        .role(Role::Group)
                        .aria_label(format!("Variable list item {index}"))
                        .w(px(100.0))
                        .h(px(20.0))
                        .into_any_element()
                })
                .w(px(100.0))
                .h(px(40.0)),
            )
            .child(
                uniform_list("uniform-accessibility-list", 2, |range, _, _| {
                    range
                        .map(|index| {
                            div()
                                .id(format!("uniform-list-accessibility-item-{index}"))
                                .role(Role::Group)
                                .aria_label(format!("Uniform list item {index}"))
                                .w(px(100.0))
                                .h(px(20.0))
                        })
                        .collect::<Vec<_>>()
                })
                .w(px(100.0))
                .h(px(40.0)),
            )
    }
}

impl Render for RevealableVirtualizedAccessibilityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(320.0))
            .h(px(100.0))
            .flex()
            .gap(px(10.0))
            .overflow_hidden()
            .child(
                list(self.list_state.clone(), |_, _, _| {
                    div()
                        .relative()
                        .w(px(100.0))
                        .h(px(300.0))
                        .child(
                            div()
                                .id("revealable-variable-list-target")
                                .role(Role::Button)
                                .aria_label("Revealable variable list target")
                                .absolute()
                                .left(px(10.0))
                                .top(px(220.0))
                                .w(px(20.0))
                                .h(px(20.0)),
                        )
                        .into_any_element()
                })
                .w(px(100.0))
                .h(px(100.0)),
            )
            .child(
                uniform_list("revealable-horizontal-uniform-list", 1, |range, _, _| {
                    range
                        .map(|_| {
                            div().relative().w(px(300.0)).h(px(100.0)).child(
                                div()
                                    .id("revealable-horizontal-uniform-list-target")
                                    .role(Role::Button)
                                    .aria_label("Revealable horizontal uniform list target")
                                    .absolute()
                                    .left(px(220.0))
                                    .top(px(10.0))
                                    .w(px(20.0))
                                    .h(px(20.0)),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
                .track_scroll(&self.horizontal_uniform_scroll)
                .w(px(100.0))
                .h(px(100.0)),
            )
            .child(
                uniform_list("revealable-uniform-list", 1, |range, _, _| {
                    range
                        .map(|_| {
                            div().relative().w(px(100.0)).h(px(300.0)).child(
                                div()
                                    .id("revealable-uniform-list-target")
                                    .role(Role::Button)
                                    .aria_label("Revealable uniform list target")
                                    .absolute()
                                    .left(px(10.0))
                                    .top(px(220.0))
                                    .w(px(20.0))
                                    .h(px(20.0)),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .track_scroll(&self.uniform_scroll)
                .w(px(100.0))
                .h(px(100.0)),
            )
    }
}

impl Render for WindowPortalAccessibilityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let owner = div()
            .id("window-portal-accessibility-owner")
            .role(Role::Group)
            .aria_label("Window portal accessibility owner")
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_hidden()
            .child(
                deferred(window_portal(
                    div()
                        .id("window-portal-accessibility-child")
                        .role(Role::Button)
                        .aria_label("Window portal accessibility child")
                        .w(px(20.0))
                        .h(px(20.0)),
                ))
                .priority(10),
            );
        div().child(owner).child(
            div()
                .id("window-portal-accessibility-after")
                .role(Role::Button)
                .aria_label("Window portal accessibility following sibling")
                .w(px(20.0))
                .h(px(20.0)),
        )
    }
}

impl Render for PortalAnchorAccessibilityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle;
        let owner = div()
            .id("portal-anchor-accessibility-owner")
            .role(Role::Group)
            .aria_label("Portal anchor accessibility owner")
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_hidden()
            .child(portal_anchor_follower(&handle, |snapshot, _, _| {
                snapshot.map(|_| {
                    div()
                        .id("portal-anchor-accessibility-child")
                        .role(Role::Button)
                        .aria_label("Portal anchor accessibility child")
                        .w(px(20.0))
                        .h(px(20.0))
                        .into_any_element()
                })
            }))
            .child(div().w(px(20.0)).h(px(20.0)).track_portal_anchor(&handle));
        div().child(owner).child(
            div()
                .id("portal-anchor-accessibility-after")
                .role(Role::Button)
                .aria_label("Portal anchor accessibility following sibling")
                .w(px(20.0))
                .h(px(20.0)),
        )
    }
}

impl Render for PublicRoundedClipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let clicks = self.clicks.clone();
        let layout_bounds = self.layout_bounds.clone();
        div().relative().size_full().child(
            div()
                .id("public-rounded-clip-target")
                .absolute()
                .left(px(20.0))
                .top(px(30.0))
                .w(px(100.0))
                .h(px(100.0))
                .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
                .child(
                    canvas(
                        move |bounds, _, _| *layout_bounds.borrow_mut() = Some(bounds),
                        |_, _, _, _| {},
                    )
                    .size_full(),
                )
                .with_subtree_clip(circular_clip(50.0)),
        )
    }
}

impl Render for RoundedClipAccessibilityView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        div()
            .id("rounded-accessibility-clip-owner")
            .role(Role::Group)
            .aria_label("Rounded accessibility clip owner")
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .child(
                div()
                    .id("fully-clipped-accessibility-node")
                    .absolute()
                    .left(Pixels::ZERO)
                    .top(Pixels::ZERO)
                    .w(px(5.0))
                    .h(px(5.0))
                    .role(Role::Button)
                    .aria_label("Fully clipped accessibility node"),
            )
            .child(
                div()
                    .id("partial-rounded-accessibility-node")
                    .absolute()
                    .left(Pixels::ZERO)
                    .top(Pixels::ZERO)
                    .w(px(20.0))
                    .h(px(20.0))
                    .role(Role::Button)
                    .aria_label("Partially clipped accessibility node")
                    .aria_value(format!("activation-{}", self.activations))
                    .aria_action(AccessibleAction::Click)
                    .on_click(move |_, _, cx| {
                        this.update(cx, |this, cx| {
                            this.activations += 1;
                            cx.notify();
                        })
                        .ok();
                    }),
            )
            .with_subtree_clip(circular_clip(50.0))
    }
}

#[open_gpui::test]
fn public_subtree_clip_preserves_layout_and_clips_pointer_hits(cx: &mut TestAppContext) {
    let clicks = Rc::new(Cell::new(0));
    let layout_bounds = Rc::new(RefCell::new(None));
    let (_view, visual) = cx.add_window_view({
        let clicks = clicks.clone();
        let layout_bounds = layout_bounds.clone();
        move |_, _| PublicRoundedClipView {
            clicks,
            layout_bounds,
        }
    });
    visual.update(|window, cx| window.draw(cx).clear());

    assert_eq!(
        *layout_bounds.borrow(),
        Some(Bounds::new(
            point(px(20.0), px(30.0)),
            size(px(100.0), px(100.0)),
        ))
    );
    visual.simulate_click(point(px(21.0), px(31.0)), Modifiers::none());
    assert_eq!(clicks.get(), 0);
    visual.simulate_click(point(px(70.0), px(80.0)), Modifiers::none());
    assert_eq!(clicks.get(), 1);
}

#[open_gpui::test]
fn prepared_clip_scope_restores_parent_after_early_return(cx: &mut TestAppContext) {
    let scoped = Rc::new(RefCell::new(None::<Hitbox>));
    let sibling = Rc::new(RefCell::new(None::<Hitbox>));
    let visual = cx.add_empty_window();
    visual.draw(Point::default(), size(px(100.0), px(100.0)), {
        let scoped = scoped.clone();
        let sibling = sibling.clone();
        move |_, _| {
            let scoped = scoped.clone();
            let sibling = sibling.clone();
            canvas(
                move |bounds, window, _| {
                    let prepared = window.prepare_subtree_clip(&circular_clip(50.0), bounds);
                    let clipped_hitbox = window
                        .with_prepared_subtree_clip(&prepared, |window| {
                            return window.insert_hitbox(bounds, HitboxBehavior::Normal);
                        })
                        .expect("a freshly prepared clip should enter");
                    *scoped.borrow_mut() = Some(clipped_hitbox);
                    *sibling.borrow_mut() =
                        Some(window.insert_hitbox(bounds, HitboxBehavior::Normal));
                },
                |_, _, _, _| {},
            )
            .size_full()
        }
    });

    let corner = point(px(1.0), px(1.0));
    assert!(
        !scoped
            .borrow()
            .as_ref()
            .expect("scoped hitbox missing")
            .contains_window_point(corner)
    );
    assert!(
        sibling
            .borrow()
            .as_ref()
            .expect("sibling hitbox missing")
            .contains_window_point(corner)
    );
}

#[open_gpui::test]
fn committed_point_target_rank_respects_order_occlusion_and_exact_clips(cx: &mut TestAppContext) {
    let hitboxes = Rc::new(RefCell::new(None::<[HitboxId; 7]>));
    let visual = cx.add_empty_window();
    visual.draw(Point::default(), size(px(300.0), px(120.0)), {
        let hitboxes = hitboxes.clone();
        move |_, _| {
            canvas(
                move |_, window, _| {
                    let overlap = Bounds::new(point(px(10.0), px(10.0)), size(px(40.0), px(40.0)));
                    let overlap_back = window.insert_hitbox(overlap, HitboxBehavior::Normal).id;
                    let overlap_front = window.insert_hitbox(overlap, HitboxBehavior::Normal).id;

                    let blocked = Bounds::new(point(px(60.0), px(10.0)), size(px(40.0), px(40.0)));
                    let blocked_back = window.insert_hitbox(blocked, HitboxBehavior::Normal).id;
                    let blocking_front =
                        window.insert_hitbox(blocked, HitboxBehavior::BlockMouse).id;

                    let scroll_blocked =
                        Bounds::new(point(px(110.0), px(10.0)), size(px(40.0), px(40.0)));
                    let scroll_blocked_back = window
                        .insert_hitbox(scroll_blocked, HitboxBehavior::Normal)
                        .id;
                    let scroll_blocking_front = window
                        .insert_hitbox(scroll_blocked, HitboxBehavior::BlockMouseExceptScroll)
                        .id;

                    let clipped_bounds =
                        Bounds::new(point(px(170.0), px(10.0)), size(px(100.0), px(100.0)));
                    let prepared =
                        window.prepare_subtree_clip(&circular_clip(50.0), clipped_bounds);
                    let clipped = window
                        .with_prepared_subtree_clip(&prepared, |window| {
                            window.insert_hitbox(clipped_bounds, HitboxBehavior::Normal)
                        })
                        .expect("a freshly prepared clip should enter")
                        .id;

                    *hitboxes.borrow_mut() = Some([
                        overlap_back,
                        overlap_front,
                        blocked_back,
                        blocking_front,
                        scroll_blocked_back,
                        scroll_blocking_front,
                        clipped,
                    ]);
                },
                |_, _, _, _| {},
            )
            .size_full()
        }
    });

    let [
        overlap_back,
        overlap_front,
        blocked_back,
        blocking_front,
        scroll_blocked_back,
        scroll_blocking_front,
        clipped,
    ] = hitboxes
        .borrow()
        .as_ref()
        .copied()
        .expect("the committed frame should publish all test hitboxes");

    visual.update(|window, _| {
        let overlap_point = point(px(20.0), px(20.0));
        assert_eq!(
            overlap_front.window_point_target_rank(overlap_point, window),
            Some(0)
        );
        assert_eq!(
            overlap_back.window_point_target_rank(overlap_point, window),
            Some(1)
        );

        let blocked_point = point(px(70.0), px(20.0));
        assert_eq!(
            blocking_front.window_point_target_rank(blocked_point, window),
            Some(0)
        );
        assert_eq!(
            blocked_back.window_point_target_rank(blocked_point, window),
            None
        );

        let scroll_blocked_point = point(px(120.0), px(20.0));
        assert_eq!(
            scroll_blocking_front.window_point_target_rank(scroll_blocked_point, window),
            Some(0)
        );
        assert_eq!(
            scroll_blocked_back.window_point_target_rank(scroll_blocked_point, window),
            None
        );

        assert_eq!(
            clipped.window_point_target_rank(point(px(220.0), px(60.0)), window),
            Some(0)
        );
        assert_eq!(
            clipped.window_point_target_rank(point(px(171.0), px(11.0)), window),
            None
        );
    });
}

#[open_gpui::test]
fn invalid_overflow_clip_suppresses_descendant_prepaint_without_panicking(cx: &mut TestAppContext) {
    let child_prepaints = Rc::new(Cell::new(0));
    let visual = cx.add_empty_window();
    visual.draw(Point::default(), size(px(100.0), px(100.0)), {
        let child_prepaints = child_prepaints.clone();
        move |_, _| {
            div()
                .size_full()
                .overflow_hidden()
                .rounded(rems(f32::MAX))
                .child(
                    canvas(
                        move |_, _, _| child_prepaints.set(child_prepaints.get() + 1),
                        |_, _, _, _| {},
                    )
                    .size_full(),
                )
        }
    });

    assert_eq!(
        child_prepaints.get(),
        0,
        "an unrepresentable overflow clip must suppress the complete descendant transaction"
    );
}

#[open_gpui::test]
fn scroll_viewport_keeps_offscreen_semantics_without_a_click_fallback(cx: &mut TestAppContext) {
    let clicks = Rc::new(Cell::new(0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), {
        let clicks = clicks.clone();
        move |_, _| OverflowAccessibilityView {
            scrollable: true,
            clicks,
        }
    });
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let (node_id, node) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Scrollable offscreen accessibility node"))
        .expect("scrollable offscreen node should remain published for ScrollIntoView");
    let owner = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Scrollable accessibility clip owner"))
        .map(|(_, node)| node)
        .expect("scrollable clip owner should remain published");
    assert!(owner.clips_children());
    assert!(node.supports_action(AccessibleAction::Click));
    let node_id = *node_id;
    assert_eq!(
        cx.update_window(window, |_, window, _| {
            window.a11y.published_node_witness(node_id)
        })
        .unwrap(),
        None,
        "an offscreen node must not receive a synthetic pointer witness"
    );

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));
    assert_eq!(
        clicks.get(),
        0,
        "Click fallback must not dispatch through a scroll viewport that rejects the target"
    );
}

#[open_gpui::test]
fn hidden_overflow_excludes_fully_clipped_accessibility_nodes(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        OverflowAccessibilityView {
            scrollable: false,
            clicks: Rc::new(Cell::new(0)),
        }
    });
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let owner = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Hidden accessibility clip owner"))
        .map(|(_, node)| node)
        .expect("hidden clip owner should remain published");
    assert!(owner.clips_children());
    assert!(
        update
            .nodes
            .iter()
            .all(|(_, node)| node.label() != Some("Hidden offscreen accessibility node"))
    );
}

#[open_gpui::test]
fn mixed_overflow_axes_keep_scroll_semantics_and_apply_hidden_exclusion(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(320.0)), |_, _| {
        MixedOverflowAccessibilityView
    });
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let owner = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Mixed-axis accessibility clip owner"))
        .map(|(_, node)| node)
        .expect("mixed-axis clip owner should remain published");
    assert!(owner.clips_children());
    assert!(
        update
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Horizontally scrollable offscreen node"))
    );
    assert!(
        update
            .nodes
            .iter()
            .all(|(_, node)| node.label() != Some("Vertically hidden offscreen node"))
    );
}

#[open_gpui::test]
fn deferred_clip_reestablishes_its_accessibility_owner_boundary(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        DeferredClipAccessibilityView
    });
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let owner = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Deferred accessibility clip owner"))
        .map(|(_, node)| node)
        .expect("deferred clip owner should remain published");
    assert!(owner.clips_children());
}

#[open_gpui::test]
fn deferred_accessibility_root_remains_a_child_of_its_semantic_clip_owner(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        DeferredOverflowAccessibilityView
    });
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let (owner_id, owner) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Deferred overflow accessibility owner"))
        .expect("semantic overflow owner should remain published");
    let (before_id, _) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Deferred overflow accessibility before"))
        .expect("preceding semantic sibling should remain published");
    let (child_id, child) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Deferred overflow accessibility child"))
        .expect("deferred semantic child should remain published");
    let (second_child_id, second_child) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Deferred overflow accessibility second child"))
        .expect("second deferred semantic child should remain published");
    let (after_id, _) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Deferred overflow accessibility after"))
        .expect("following semantic sibling should remain published");

    assert!(owner.clips_children());
    assert_eq!(
        owner.children(),
        &[*before_id, *child_id, *second_child_id, *after_id],
        "deferred priority must not reorder AccessKit siblings"
    );
    assert!(!child.clips_children());
    assert!(!second_child.clips_children());
    assert_ne!(owner_id, child_id);
}

#[open_gpui::test]
fn nested_anonymous_deferred_roots_keep_the_semantic_clip_owner(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        NestedDeferredOverflowAccessibilityView
    });
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let (_, owner) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Nested deferred overflow accessibility owner"))
        .expect("semantic overflow owner should remain published");
    let (before_id, _) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Nested deferred overflow accessibility before"))
        .expect("preceding semantic sibling should remain published");
    let (child_id, child) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Nested deferred overflow accessibility child"))
        .expect("nested deferred semantic child should remain published");
    let (after_id, _) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Nested deferred overflow accessibility after"))
        .expect("following semantic sibling should remain published");

    assert!(owner.clips_children());
    assert_eq!(owner.children(), &[*before_id, *child_id, *after_id]);
    assert!(!child.clips_children());
}

#[open_gpui::test]
fn anonymous_clip_marks_each_direct_accessibility_root_as_a_proxy_owner(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        MultipleAnonymousClipRootsAccessibilityView
    });
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    for label in ["First anonymous clip root", "Second anonymous clip root"] {
        let node = update
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .map(|(_, node)| node)
            .unwrap_or_else(|| panic!("{label} should remain published"));
        assert!(node.clips_children(), "{label} should own the proxy clip");
    }
}

#[open_gpui::test]
fn virtualized_list_viewports_mark_visible_accessibility_roots_as_proxy_owners(
    cx: &mut TestAppContext,
) {
    let list_state = ListState::new(2, ListAlignment::Top, px(40.0));
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), move |_, _| {
        VirtualizedClipAccessibilityView {
            list_state: list_state.clone(),
        }
    });
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    for label in [
        "Variable list item 0",
        "Variable list item 1",
        "Uniform list item 0",
        "Uniform list item 1",
    ] {
        let node = update
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .map(|(_, node)| node)
            .unwrap_or_else(|| panic!("{label} should remain published"));
        assert!(
            node.clips_children(),
            "{label} should own its viewport's proxy clip"
        );
    }
}

#[open_gpui::test]
fn virtualized_scroll_viewports_preserve_revealable_offscreen_accessibility_nodes(
    cx: &mut TestAppContext,
) {
    let list_state = ListState::new(1, ListAlignment::Top, px(300.0));
    let observed_list_state = list_state.clone();
    let uniform_scroll = UniformListScrollHandle::new();
    let observed_uniform_scroll = uniform_scroll.clone();
    let horizontal_uniform_scroll = UniformListScrollHandle::new();
    let observed_horizontal_uniform_scroll = horizontal_uniform_scroll.clone();
    let typed_window = cx.open_window(size(px(360.0), px(200.0)), move |_, _| {
        RevealableVirtualizedAccessibilityView {
            list_state: list_state.clone(),
            uniform_scroll: uniform_scroll.clone(),
            horizontal_uniform_scroll: horizontal_uniform_scroll.clone(),
        }
    });
    let window = typed_window.into();
    let scale_factor = cx
        .update_window(window, |_, window, _| window.scale_factor())
        .unwrap();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let targets = [
        ("Revealable variable list target", point(px(10.0), px(80.0))),
        ("Revealable uniform list target", point(px(230.0), px(80.0))),
        (
            "Revealable horizontal uniform list target",
            point(px(190.0), px(10.0)),
        ),
    ]
    .map(|(label, expected_origin)| {
        let (node_id, node) = update
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .unwrap_or_else(|| panic!("{label} should remain published for ScrollIntoView"));
        assert!(node.supports_action(AccessibleAction::ScrollIntoView));
        (*node_id, label, expected_origin)
    });

    for (node_id, label, expected_origin) in targets {
        assert!(cx.dispatch_accessibility_action(
            window,
            ActionRequest {
                action: AccessibleAction::ScrollIntoView,
                target_tree: TreeId::ROOT,
                target_node: node_id,
                data: None,
            },
        ));
        cx.run_until_parked();

        match label {
            "Revealable variable list target" => assert_eq!(
                observed_list_state.scroll_px_offset_for_scrollbar(),
                point(Pixels::ZERO, px(-140.0))
            ),
            "Revealable uniform list target" => assert_eq!(
                observed_uniform_scroll.base_handle().offset(),
                point(Pixels::ZERO, px(-140.0))
            ),
            "Revealable horizontal uniform list target" => assert_eq!(
                observed_horizontal_uniform_scroll.base_handle().offset(),
                point(px(-140.0), Pixels::ZERO)
            ),
            _ => unreachable!(),
        }
        cx.update_window(window, |_, window, cx| window.draw(cx).clear())
            .unwrap();

        let update = cx.latest_accessibility_tree_update(window).unwrap();
        let node = update
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .map(|(_, node)| node)
            .unwrap_or_else(|| panic!("{label} should remain published after ScrollIntoView"));
        assert_eq!(
            node.bounds(),
            Some(accesskit::Rect {
                x0: (expected_origin.x.0 * scale_factor) as f64,
                y0: (expected_origin.y.0 * scale_factor) as f64,
                x1: ((expected_origin.x + px(20.0)).0 * scale_factor) as f64,
                y1: ((expected_origin.y + px(20.0)).0 * scale_factor) as f64,
            }),
            "{label} should finish inside its viewport"
        );
    }
}

#[open_gpui::test]
fn window_portal_resets_accessibility_parentage_with_its_clip_ancestry(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        WindowPortalAccessibilityView
    });
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let (owner_id, owner) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Window portal accessibility owner"))
        .expect("clipped semantic owner should remain published");
    let (child_id, _) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Window portal accessibility child"))
        .expect("window portal child should remain published");
    let (after_id, _) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Window portal accessibility following sibling"))
        .expect("following root sibling should remain published");
    let root = update
        .nodes
        .iter()
        .find(|(id, _)| *id == accesskit::NodeId(0))
        .map(|(_, node)| node)
        .expect("window root should remain published");

    assert!(owner.clips_children());
    assert!(!owner.children().contains(child_id));
    assert_eq!(root.children(), &[*owner_id, *child_id, *after_id]);
    assert_ne!(owner_id, child_id);
}

#[open_gpui::test]
fn portal_anchor_window_portal_resets_accessibility_parentage(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |window, _| {
        PortalAnchorAccessibilityView {
            handle: window.new_portal_anchor(),
        }
    });
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let (owner_id, owner) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Portal anchor accessibility owner"))
        .expect("clipped semantic owner should remain published");
    let (child_id, child) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Portal anchor accessibility child"))
        .expect("portal follower child should remain published");
    let (after_id, _) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Portal anchor accessibility following sibling"))
        .expect("following root sibling should remain published");
    let root = update
        .nodes
        .iter()
        .find(|(id, _)| *id == accesskit::NodeId(0))
        .map(|(_, node)| node)
        .expect("window root should remain published");

    assert!(owner.clips_children());
    assert!(!owner.children().contains(child_id));
    assert_eq!(root.children(), &[*owner_id, *child_id, *after_id]);
    assert!(!child.clips_children());
    assert_ne!(owner_id, child_id);
}

#[open_gpui::test]
fn accessibility_uses_an_exact_rounded_clip_witness_for_click_fallback(cx: &mut TestAppContext) {
    let typed_window = cx.open_window(size(px(320.0), px(200.0)), |_, _| {
        RoundedClipAccessibilityView { activations: 0 }
    });
    let view = typed_window.root(cx).unwrap();
    let window = typed_window.into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let owner = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Rounded accessibility clip owner"))
        .map(|(_, node)| node)
        .expect("rounded clip owner should remain published");
    assert!(owner.clips_children());
    assert!(
        update
            .nodes
            .iter()
            .all(|(_, node)| { node.label() != Some("Fully clipped accessibility node") })
    );
    let (node_id, node) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Partially clipped accessibility node"))
        .expect("partially visible accessibility node missing");
    assert_eq!(
        node.bounds(),
        Some(accesskit::Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 40.0,
            y1: 40.0,
        })
    );
    assert!(node.supports_action(AccessibleAction::Click));
    let node_id = *node_id;
    let witness = cx
        .update_window(window, |_, window, _| {
            window.a11y.published_node_witness(node_id)
        })
        .unwrap()
        .expect("published rounded node should retain an exact witness");
    let dx = witness.x.0 - 50.0;
    let dy = witness.y.0 - 50.0;
    assert!(dx.mul_add(dx, dy * dy) < 50.0 * 50.0);
    assert!(
        (10.0_f32 - 50.0).mul_add(10.0 - 50.0, (10.0 - 50.0) * (10.0 - 50.0)) > 50.0 * 50.0,
        "the conservative AABB center must remain outside the circular clip"
    );

    assert!(cx.dispatch_accessibility_action(
        window,
        ActionRequest {
            action: AccessibleAction::Click,
            target_tree: TreeId::ROOT,
            target_node: node_id,
            data: None,
        },
    ));
    assert_eq!(cx.read(|cx| view.read(cx).activations), 1);
}
