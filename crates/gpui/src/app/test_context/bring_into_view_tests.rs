use std::{cell::RefCell, rc::Rc, time::Duration};

use accesskit::{Action as AccessibleAction, ActionRequest, Role, TreeId};
use open_gpui_motion::{
    MotionDuration, MotionEasing, MotionIntent, MotionPreference, MotionTransition,
};

use super::accessibility_tests::accessibility_scope;

use crate::{
    AccessibilityTreeScope, BringIntoViewAlignment, BringIntoViewCancelReason, BringIntoViewError,
    BringIntoViewMargins, BringIntoViewOptions, BringIntoViewOutcome, Context,
    DeferredBringIntoViewGuard, Empty, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, RevealTargetExt as _, RevealTargetHandle, ScrollHandle,
    ScrollViewportChangeSource, ScrollViewportProgrammaticSource, StatefulInteractiveElement as _,
    Styled as _, SubtreePresentation, SubtreePresentationExt as _, SubtreeTransform,
    SubtreeTransformExt as _, SubtreeTransformOrigin, TestAppContext, VisualContext as _, Window,
    div, point, px, size, util::FluentBuilder as _, window_portal,
};

struct SingleScrollRevealView {
    target: RevealTargetHandle,
    scroll: ScrollHandle,
    target_presentation: SubtreePresentation,
}

impl Render for SingleScrollRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .child(
                div().relative().w(px(300.0)).h(px(300.0)).child(
                    div()
                        .id("single-scroll-reveal-target")
                        .role(Role::Button)
                        .aria_label("Single scroll reveal target")
                        .absolute()
                        .left(px(220.0))
                        .top(px(220.0))
                        .w(px(20.0))
                        .h(px(20.0))
                        .track_reveal_target(&self.target)
                        .with_subtree_presentation(self.target_presentation),
                ),
            )
    }
}

struct FocusScrollRevealView {
    scroll: ScrollHandle,
    visible_focus: FocusHandle,
    offscreen_focus: FocusHandle,
}

struct GuardedFocusRevealView {
    scroll: ScrollHandle,
    scroll_chain_anchor: RevealTargetHandle,
    offscreen_focus: FocusHandle,
}

struct TransformedScrollRevealView {
    target: RevealTargetHandle,
    scroll: ScrollHandle,
}

impl Render for TransformedScrollRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .child(
                div().relative().w(px(300.0)).h(px(300.0)).child(
                    div()
                        .absolute()
                        .left(px(220.0))
                        .top(px(220.0))
                        .w(px(20.0))
                        .h(px(20.0))
                        .track_reveal_target(&self.target),
                ),
            )
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2.0, 4.0),
                    point(px(0.0), px(0.0)),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .expect("the non-uniform container transform should be valid"),
            )
    }
}

impl Render for FocusScrollRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .child(
                div()
                    .relative()
                    .w(px(300.0))
                    .h(px(300.0))
                    .child(
                        div()
                            .id("visible-focus-reveal-target")
                            .absolute()
                            .left(px(10.0))
                            .top(px(10.0))
                            .w(px(20.0))
                            .h(px(20.0))
                            .focusable()
                            .track_focus(&self.visible_focus),
                    )
                    .child(
                        div()
                            .id("offscreen-focus-reveal-target")
                            .absolute()
                            .left(px(220.0))
                            .top(px(220.0))
                            .w(px(20.0))
                            .h(px(20.0))
                            .focusable()
                            .track_focus(&self.offscreen_focus),
                    ),
            )
    }
}

impl Render for GuardedFocusRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .child(
                div()
                    .relative()
                    .w(px(300.0))
                    .h(px(300.0))
                    .child(
                        div()
                            .id("guarded-focus-scroll-chain-anchor")
                            .absolute()
                            .left(px(0.0))
                            .top(px(0.0))
                            .w(px(1.0))
                            .h(px(1.0))
                            .track_reveal_target(&self.scroll_chain_anchor),
                    )
                    .child(
                        div()
                            .id("guarded-offscreen-focus-reveal-target")
                            .absolute()
                            .left(px(220.0))
                            .top(px(220.0))
                            .w(px(20.0))
                            .h(px(20.0))
                            .focusable()
                            .track_focus(&self.offscreen_focus),
                    ),
            )
    }
}

struct NestedScrollRevealView {
    target: RevealTargetHandle,
    outer: ScrollHandle,
    inner: ScrollHandle,
    commits: Rc<RefCell<Vec<(&'static str, u64)>>>,
}

struct DeferredGuardNestedScrollRevealView {
    target: RevealTargetHandle,
    outer: ScrollHandle,
    inner: ScrollHandle,
    options: BringIntoViewOptions,
    target_is_mounted: bool,
    outer_all_axes: bool,
    captured_guard: Rc<RefCell<Option<DeferredBringIntoViewGuard>>>,
}

struct NestedTransformedScrollRevealView {
    target: RevealTargetHandle,
    outer: ScrollHandle,
    inner: ScrollHandle,
}

impl Render for NestedTransformedScrollRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let inner_transform = SubtreeTransform::try_new(
            size(1.15, 0.9),
            point(px(0.0), px(0.0)),
            SubtreeTransformOrigin::TOP_LEFT,
        )
        .unwrap();
        let target_transform = SubtreeTransform::try_new(
            size(1.1, 0.85),
            point(px(6.0), px(-4.0)),
            SubtreeTransformOrigin::CENTER,
        )
        .unwrap();
        let target = div()
            .absolute()
            .left(px(330.0))
            .top(px(270.0))
            .w(px(48.0))
            .h(px(36.0))
            .with_subtree_transform(target_transform)
            .track_reveal_target(&self.target);
        let inner = div()
            .absolute()
            .left(px(350.0))
            .top(px(300.0))
            .w(px(150.0))
            .h(px(120.0))
            .overflow_scroll()
            .track_scroll(&self.inner)
            .child(div().relative().w(px(420.0)).h(px(340.0)).child(target))
            .with_subtree_transform(inner_transform);
        div()
            .relative()
            .w(px(280.0))
            .h(px(190.0))
            .overflow_scroll()
            .track_scroll(&self.outer)
            .child(div().relative().w(px(640.0)).h(px(540.0)).child(inner))
    }
}

struct PositionedScrollRevealView {
    target: RevealTargetHandle,
    scroll: ScrollHandle,
    mounted: bool,
    content_width: crate::Pixels,
    content_height: crate::Pixels,
    target_left: crate::Pixels,
    target_top: crate::Pixels,
    target_width: crate::Pixels,
    target_height: crate::Pixels,
}

impl Render for PositionedScrollRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let content = div()
            .relative()
            .w(self.content_width)
            .h(self.content_height)
            .when(self.mounted, |content| {
                content.child(
                    div()
                        .absolute()
                        .left(self.target_left)
                        .top(self.target_top)
                        .w(self.target_width)
                        .h(self.target_height)
                        .track_reveal_target(&self.target),
                )
            });
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .child(content)
    }
}

struct ParallelScrollRevealView {
    left_target: RevealTargetHandle,
    right_target: RevealTargetHandle,
    left_scroll: ScrollHandle,
    right_scroll: ScrollHandle,
}

struct DisjointFocusRevealView {
    left_outer: ScrollHandle,
    left_inner: ScrollHandle,
    left_focus: FocusHandle,
    right_outer: ScrollHandle,
    right_inner: ScrollHandle,
    right_focus: FocusHandle,
}

impl Render for DisjointFocusRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let chain = |prefix: &'static str,
                     outer: &ScrollHandle,
                     inner: &ScrollHandle,
                     focus: &FocusHandle| {
            div()
                .relative()
                .w(px(100.0))
                .h(px(100.0))
                .overflow_scroll()
                .track_scroll(outer)
                .child(
                    div().relative().w(px(320.0)).h(px(320.0)).child(
                        div()
                            .absolute()
                            .left(px(150.0))
                            .top(px(150.0))
                            .w(px(80.0))
                            .h(px(80.0))
                            .overflow_scroll()
                            .track_scroll(inner)
                            .child(
                                div().relative().w(px(240.0)).h(px(240.0)).child(
                                    div()
                                        .id(format!("{prefix}-focus-target"))
                                        .absolute()
                                        .left(px(200.0))
                                        .top(px(200.0))
                                        .w(px(20.0))
                                        .h(px(20.0))
                                        .focusable()
                                        .track_focus(focus),
                                ),
                            ),
                    ),
                )
        };

        div()
            .flex()
            .gap(px(20.0))
            .child(chain(
                "left",
                &self.left_outer,
                &self.left_inner,
                &self.left_focus,
            ))
            .child(chain(
                "right",
                &self.right_outer,
                &self.right_inner,
                &self.right_focus,
            ))
    }
}

struct FocusListenerRevealView {
    scroll: ScrollHandle,
    focus: FocusHandle,
    application_target: RevealTargetHandle,
}

impl Render for FocusListenerRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .child(
                div()
                    .relative()
                    .w(px(300.0))
                    .h(px(300.0))
                    .child(
                        div()
                            .absolute()
                            .left(px(10.0))
                            .top(px(10.0))
                            .w(px(20.0))
                            .h(px(20.0))
                            .track_reveal_target(&self.application_target),
                    )
                    .child(
                        div()
                            .id("focus-listener-reveal-target")
                            .absolute()
                            .left(px(220.0))
                            .top(px(220.0))
                            .w(px(20.0))
                            .h(px(20.0))
                            .focusable()
                            .track_focus(&self.focus),
                    ),
            )
    }
}

struct FractionalNearestRevealView {
    target: RevealTargetHandle,
    scroll: ScrollHandle,
    content_width: crate::Pixels,
    target_left: crate::Pixels,
    target_width: crate::Pixels,
}

impl Render for FractionalNearestRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let transform = SubtreeTransform::try_new(
            size(1.0, 1.0),
            point(px(0.2), px(0.0)),
            SubtreeTransformOrigin::TOP_LEFT,
        )
        .expect("fractional translation should be valid");
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .child(
                div().relative().w(self.content_width).h(px(100.0)).child(
                    div()
                        .absolute()
                        .left(self.target_left)
                        .top(px(10.0))
                        .w(self.target_width)
                        .h(px(10.0))
                        .with_subtree_transform(transform)
                        .track_reveal_target(&self.target),
                ),
            )
    }
}

struct AccessibilityScopedRevealView {
    target: RevealTargetHandle,
    scroll: ScrollHandle,
    exclude_target: bool,
}

impl Render for AccessibilityScopedRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let scope = if self.exclude_target {
            AccessibilityTreeScope::Excluded
        } else {
            AccessibilityTreeScope::Unrestricted
        };
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .child(
                div()
                    .relative()
                    .w(px(300.0))
                    .h(px(300.0))
                    .child(accessibility_scope(
                        scope,
                        div()
                            .id("stale-accessibility-reveal-target")
                            .role(Role::Button)
                            .aria_label("Stale accessibility reveal target")
                            .absolute()
                            .left(px(220.0))
                            .top(px(220.0))
                            .w(px(20.0))
                            .h(px(20.0))
                            .track_reveal_target(&self.target),
                    )),
            )
            .when(self.exclude_target, |root| {
                root.child(accessibility_scope(
                    AccessibilityTreeScope::ModalRoot,
                    div()
                        .id("stale-accessibility-modal-root")
                        .role(Role::Dialog)
                        .aria_label("Stale accessibility modal root")
                        .aria_modal(true),
                ))
            })
    }
}

impl Render for ParallelScrollRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let scroll_port = |scroll: &ScrollHandle, target: &RevealTargetHandle| {
            div()
                .relative()
                .w(px(100.0))
                .h(px(100.0))
                .overflow_scroll()
                .track_scroll(scroll)
                .child(
                    div().relative().w(px(300.0)).h(px(300.0)).child(
                        div()
                            .absolute()
                            .left(px(220.0))
                            .top(px(220.0))
                            .w(px(20.0))
                            .h(px(20.0))
                            .track_reveal_target(target),
                    ),
                )
        };
        div()
            .flex()
            .gap(px(20.0))
            .child(scroll_port(&self.left_scroll, &self.left_target))
            .child(scroll_port(&self.right_scroll, &self.right_target))
    }
}

struct SharedScrollRevealView {
    first_target: RevealTargetHandle,
    second_target: RevealTargetHandle,
    scroll: ScrollHandle,
}

impl Render for SharedScrollRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .child(
                div()
                    .relative()
                    .w(px(300.0))
                    .h(px(300.0))
                    .child(
                        div()
                            .absolute()
                            .left(px(180.0))
                            .top(px(180.0))
                            .w(px(20.0))
                            .h(px(20.0))
                            .track_reveal_target(&self.first_target),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(220.0))
                            .top(px(220.0))
                            .w(px(20.0))
                            .h(px(20.0))
                            .track_reveal_target(&self.second_target),
                    ),
            )
    }
}

struct PortalScrollRevealView {
    target: RevealTargetHandle,
    scroll: ScrollHandle,
}

struct BorderedScrollRevealView {
    target: RevealTargetHandle,
    scroll: ScrollHandle,
}

impl Render for BorderedScrollRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .border_1()
            .border_color(crate::rgb(0x000000))
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .child(
                div().relative().w(px(300.0)).h(px(300.0)).child(
                    div()
                        .absolute()
                        .left(px(90.0))
                        .top(px(90.0))
                        .w(px(10.0))
                        .h(px(10.0))
                        .track_reveal_target(&self.target),
                ),
            )
    }
}

impl Render for PortalScrollRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .child(
                div().relative().w(px(300.0)).h(px(300.0)).child(
                    window_portal(
                        div()
                            .absolute()
                            .left(px(220.0))
                            .top(px(220.0))
                            .w(px(20.0))
                            .h(px(20.0))
                            .track_reveal_target(&self.target),
                    )
                    .with_priority(1),
                ),
            )
    }
}

impl Render for NestedScrollRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let outer_commits = self.commits.clone();
        let inner_commits = self.commits.clone();
        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_scroll()
            .track_scroll(&self.outer)
            .on_scroll_viewport_changed(move |event, window, _| {
                if event.snapshot().source()
                    == ScrollViewportChangeSource::Programmatic(
                        ScrollViewportProgrammaticSource::BringIntoView,
                    )
                {
                    outer_commits
                        .borrow_mut()
                        .push(("outer", window.rendered_frame_revision()));
                }
            })
            .child(
                div().relative().w(px(320.0)).h(px(320.0)).child(
                    div()
                        .absolute()
                        .left(px(150.0))
                        .top(px(150.0))
                        .w(px(80.0))
                        .h(px(80.0))
                        .overflow_scroll()
                        .track_scroll(&self.inner)
                        .on_scroll_viewport_changed(move |event, window, _| {
                            if event.snapshot().source()
                                == ScrollViewportChangeSource::Programmatic(
                                    ScrollViewportProgrammaticSource::BringIntoView,
                                )
                            {
                                inner_commits
                                    .borrow_mut()
                                    .push(("inner", window.rendered_frame_revision()));
                            }
                        })
                        .child(
                            div().relative().w(px(240.0)).h(px(240.0)).child(
                                div()
                                    .absolute()
                                    .left(px(200.0))
                                    .top(px(200.0))
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .track_reveal_target(&self.target),
                            ),
                        ),
                ),
            )
    }
}

impl Render for DeferredGuardNestedScrollRevealView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let target = self.target;
        let options = self.options;
        let captured_guard = self.captured_guard.clone();
        let mut target_container = div().relative().w(px(240.0)).h(px(240.0));
        target_container = target_container.on_children_prepainted(move |_, window, _| {
            let guard = window
                .capture_deferred_bring_into_view_guard(&target, options)
                .expect("the target belongs to this window");
            let pending_guard = Rc::new(RefCell::new(Some(guard)));
            let captured_guard = captured_guard.clone();
            window.record_prepaint_window_commit(move |_, _, _| {
                if let Some(guard) = pending_guard.borrow_mut().take() {
                    captured_guard.borrow_mut().replace(guard);
                }
            });
        });
        let target_container = target_container.when(self.target_is_mounted, |this| {
            this.child(
                div()
                    .absolute()
                    .left(px(200.0))
                    .top(px(200.0))
                    .w(px(20.0))
                    .h(px(20.0))
                    .track_reveal_target(&target),
            )
        });

        div()
            .relative()
            .w(px(100.0))
            .h(px(100.0))
            .overflow_y_scroll()
            .when(self.outer_all_axes, |this| this.overflow_x_scroll())
            .track_scroll(&self.outer)
            .child(
                div().relative().w(px(320.0)).h(px(320.0)).child(
                    div()
                        .absolute()
                        .left(px(150.0))
                        .top(px(150.0))
                        .w(px(80.0))
                        .h(px(80.0))
                        .overflow_scroll()
                        .track_scroll(&self.inner)
                        .child(target_container),
                ),
            )
    }
}

#[open_gpui::test]
fn application_reveal_commits_both_physical_axes(cx: &mut TestAppContext) {
    let outcome = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view(|window, _| SingleScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        target_presentation: SubtreePresentation::Visible,
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (target, scroll) =
        cx.update_window_entity(&view, |view, _, _| (view.target, view.scroll.clone()));

    let observed = outcome.clone();
    let _subscription = cx.update(|window, cx| {
        window
            .bring_into_view_with_completion(
                &target,
                BringIntoViewOptions::nearest(),
                cx,
                move |result, _, _| *observed.borrow_mut() = Some(result),
            )
            .expect("the target belongs to this window")
            .1
    });

    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(scroll.offset(), point(px(-140.0), px(-140.0)));
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert_eq!(
        *outcome.borrow(),
        Some(BringIntoViewOutcome::Completed(
            crate::BringIntoViewCompletion::Revealed
        ))
    );
}

#[open_gpui::test]
fn nested_reveal_commits_inner_before_outer(cx: &mut TestAppContext) {
    let commits = Rc::new(RefCell::new(Vec::new()));
    let (view, cx) = cx.add_window_view({
        let commits = commits.clone();
        move |window, _| NestedScrollRevealView {
            target: window.new_reveal_target(),
            outer: ScrollHandle::new(),
            inner: ScrollHandle::new(),
            commits,
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (target, outer, inner) = cx.update_window_entity(&view, |view, _, _| {
        (view.target, view.outer.clone(), view.inner.clone())
    });

    cx.update(|window, cx| {
        window
            .bring_into_view(&target, BringIntoViewOptions::nearest(), cx)
            .expect("the target belongs to this window");
    });
    assert_eq!(inner.offset(), point(px(-140.0), px(-140.0)));
    assert_eq!(outer.offset(), point(px(0.0), px(0.0)));
    cx.update(|window, cx| window.draw(cx).clear());
    assert_ne!(outer.offset(), point(px(0.0), px(0.0)));
    cx.update(|window, cx| window.draw(cx).clear());
    let commits = commits.borrow();
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].0, "inner");
    assert_eq!(commits[1].0, "outer");
    assert!(commits[0].1 < commits[1].1);
}

#[open_gpui::test]
fn deferred_guard_rejects_an_ancestor_direct_scroll_before_submission(cx: &mut TestAppContext) {
    let captured_guard = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view({
        let captured_guard = captured_guard.clone();
        move |window, _| DeferredGuardNestedScrollRevealView {
            target: window.new_reveal_target(),
            outer: ScrollHandle::new(),
            inner: ScrollHandle::new(),
            options: BringIntoViewOptions::nearest(),
            target_is_mounted: true,
            outer_all_axes: true,
            captured_guard,
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (outer, inner) =
        cx.update_window_entity(&view, |view, _, _| (view.outer.clone(), view.inner.clone()));
    let guard = captured_guard
        .borrow_mut()
        .take()
        .expect("the successful prepaint path should capture the nested target guard");

    outer.set_offset(point(px(-25.0), px(-35.0)));
    let submitted = cx.update(|window, cx| {
        window.try_bring_into_view_with_guard_and_completion(guard, cx, |_, _, _| {})
    });

    assert!(matches!(submitted, Ok(None)));
    assert_eq!(outer.offset(), point(px(-25.0), px(-35.0)));
    assert_eq!(inner.offset(), point(px(0.0), px(0.0)));
}

#[open_gpui::test]
fn deferred_guard_captures_before_target_binds_and_submits_after_binding(cx: &mut TestAppContext) {
    let captured_guard = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view({
        let captured_guard = captured_guard.clone();
        move |window, _| DeferredGuardNestedScrollRevealView {
            target: window.new_reveal_target(),
            outer: ScrollHandle::new(),
            inner: ScrollHandle::new(),
            options: BringIntoViewOptions::nearest(),
            target_is_mounted: false,
            outer_all_axes: true,
            captured_guard,
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let guard = captured_guard
        .borrow_mut()
        .take()
        .expect("prepaint should capture the intended scroll ancestry before the target binds");

    view.update(cx, |view, cx| {
        view.target_is_mounted = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let submitted = cx.update(|window, cx| {
        window.try_bring_into_view_with_guard_and_completion(guard, cx, |_, _, _| {})
    });
    let (_, subscription) = submitted
        .expect("the target belongs to this window")
        .expect("the later binding should match the previously captured scroll chain");
    subscription.detach();
}

#[open_gpui::test]
fn deferred_guard_rejects_a_changed_scroll_axis_before_submission(cx: &mut TestAppContext) {
    let captured_guard = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view({
        let captured_guard = captured_guard.clone();
        move |window, _| DeferredGuardNestedScrollRevealView {
            target: window.new_reveal_target(),
            outer: ScrollHandle::new(),
            inner: ScrollHandle::new(),
            options: BringIntoViewOptions::vertical(BringIntoViewAlignment::Nearest),
            target_is_mounted: true,
            outer_all_axes: false,
            captured_guard,
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let outer = cx.update_window_entity(&view, |view, _, _| view.outer.clone());
    let guard = captured_guard
        .borrow_mut()
        .take()
        .expect("the initial vertical scroll chain should be captured");

    view.update(cx, |view, cx| {
        view.outer_all_axes = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    outer.set_offset(point(px(-25.0), px(0.0)));

    let submitted = cx.update(|window, cx| {
        window.try_bring_into_view_with_guard_and_completion(guard, cx, |_, _, _| {})
    });

    assert!(matches!(submitted, Ok(None)));
    assert_eq!(outer.offset(), point(px(-25.0), px(0.0)));
}

#[open_gpui::test]
fn deferred_guard_preserves_a_direct_scroll_on_an_unrequested_axis(cx: &mut TestAppContext) {
    let captured_guard = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view({
        let captured_guard = captured_guard.clone();
        move |window, _| DeferredGuardNestedScrollRevealView {
            target: window.new_reveal_target(),
            outer: ScrollHandle::new(),
            inner: ScrollHandle::new(),
            options: BringIntoViewOptions::vertical(BringIntoViewAlignment::Nearest),
            target_is_mounted: true,
            outer_all_axes: true,
            captured_guard,
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let outer = cx.update_window_entity(&view, |view, _, _| view.outer.clone());
    let guard = captured_guard
        .borrow_mut()
        .take()
        .expect("the successful prepaint path should capture the nested target guard");

    outer.set_offset(point(px(-25.0), px(0.0)));
    let submitted = cx.update(|window, cx| {
        window.try_bring_into_view_with_guard_and_completion(guard, cx, |_, _, _| {})
    });
    let (_, subscription) = submitted
        .expect("the target belongs to this window")
        .expect("a horizontal direct scroll must not cancel a vertical-only reveal");
    subscription.detach();

    assert_eq!(outer.offset(), point(px(-25.0), px(0.0)));
}

#[open_gpui::test]
fn nested_reveal_continues_outward_across_non_uniform_transforms(cx: &mut TestAppContext) {
    let outcome = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view(|window, _| NestedTransformedScrollRevealView {
        target: window.new_reveal_target(),
        outer: ScrollHandle::new(),
        inner: ScrollHandle::new(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (target, outer, inner) = cx.update_window_entity(&view, |view, _, _| {
        (view.target, view.outer.clone(), view.inner.clone())
    });
    let observed = outcome.clone();
    let _subscription = cx.update(|window, cx| {
        window
            .bring_into_view_with_completion(
                &target,
                BringIntoViewOptions::aligned(BringIntoViewAlignment::Center),
                cx,
                move |outcome, _, _| *observed.borrow_mut() = Some(outcome),
            )
            .unwrap()
            .1
    });
    for _ in 0..5 {
        cx.update(|window, cx| window.draw(cx).clear());
    }
    cx.run_until_parked();

    assert_eq!(inner.offset(), point(px(-270.0), px(-220.0)));
    assert!(
        outer.offset().x < px(0.0) && outer.offset().y < px(0.0),
        "outer offset {:?}, outcome {:?}",
        outer.offset(),
        *outcome.borrow()
    );
    assert_eq!(
        *outcome.borrow(),
        Some(BringIntoViewOutcome::Completed(
            crate::BringIntoViewCompletion::Revealed
        ))
    );
}

#[open_gpui::test]
fn disjoint_scroll_chains_advance_independently(cx: &mut TestAppContext) {
    let left_outcome = Rc::new(RefCell::new(None));
    let right_outcome = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view(|window, _| ParallelScrollRevealView {
        left_target: window.new_reveal_target(),
        right_target: window.new_reveal_target(),
        left_scroll: ScrollHandle::new(),
        right_scroll: ScrollHandle::new(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (left_target, right_target, left_scroll, right_scroll) =
        cx.update_window_entity(&view, |view, _, _| {
            (
                view.left_target,
                view.right_target,
                view.left_scroll.clone(),
                view.right_scroll.clone(),
            )
        });

    let left_observed = left_outcome.clone();
    let right_observed = right_outcome.clone();
    let (_left_subscription, _right_subscription) = cx.update(|window, cx| {
        let left = window
            .bring_into_view_with_completion(
                &left_target,
                BringIntoViewOptions::nearest(),
                cx,
                move |outcome, _, _| *left_observed.borrow_mut() = Some(outcome),
            )
            .unwrap()
            .1;
        let right = window
            .bring_into_view_with_completion(
                &right_target,
                BringIntoViewOptions::nearest(),
                cx,
                move |outcome, _, _| *right_observed.borrow_mut() = Some(outcome),
            )
            .unwrap()
            .1;
        (left, right)
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert_eq!(left_scroll.offset(), point(px(-140.0), px(-140.0)));
    assert_eq!(right_scroll.offset(), point(px(-140.0), px(-140.0)));
    assert_eq!(
        *left_outcome.borrow(),
        Some(BringIntoViewOutcome::Completed(
            crate::BringIntoViewCompletion::Revealed
        ))
    );
    assert_eq!(*right_outcome.borrow(), *left_outcome.borrow());
}

#[open_gpui::test]
fn newer_request_supersedes_an_older_request_on_the_same_chain(cx: &mut TestAppContext) {
    let first_outcome = Rc::new(RefCell::new(None));
    let second_outcome = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view(|window, _| SharedScrollRevealView {
        first_target: window.new_reveal_target(),
        second_target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (first_target, second_target, scroll) = cx.update_window_entity(&view, |view, _, _| {
        (view.first_target, view.second_target, view.scroll.clone())
    });

    let first_observed = first_outcome.clone();
    let second_observed = second_outcome.clone();
    let (_first_subscription, _second_subscription) = cx.update(|window, cx| {
        let first = window
            .bring_into_view_with_completion(
                &first_target,
                BringIntoViewOptions::nearest(),
                cx,
                move |outcome, _, _| *first_observed.borrow_mut() = Some(outcome),
            )
            .unwrap()
            .1;
        let second = window
            .bring_into_view_with_completion(
                &second_target,
                BringIntoViewOptions::nearest(),
                cx,
                move |outcome, _, _| *second_observed.borrow_mut() = Some(outcome),
            )
            .unwrap()
            .1;
        (first, second)
    });
    cx.run_until_parked();

    assert_eq!(scroll.offset(), point(px(-140.0), px(-140.0)));
    assert_eq!(
        *first_outcome.borrow(),
        Some(BringIntoViewOutcome::Cancelled(
            BringIntoViewCancelReason::Superseded
        ))
    );
    assert_eq!(
        *second_outcome.borrow(),
        Some(BringIntoViewOutcome::Completed(
            crate::BringIntoViewCompletion::Revealed
        ))
    );
}

#[open_gpui::test]
fn already_visible_and_no_progress_are_distinct_terminal_outcomes(cx: &mut TestAppContext) {
    let visible_outcome = Rc::new(RefCell::new(None));
    let (visible_view, cx) = cx.add_window_view(|window, _| PositionedScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        mounted: true,
        content_width: px(300.0),
        content_height: px(300.0),
        target_left: px(20.0),
        target_top: px(20.0),
        target_width: px(20.0),
        target_height: px(20.0),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (visible_target, visible_scroll) = cx.update_window_entity(&visible_view, |view, _, _| {
        (view.target, view.scroll.clone())
    });
    let visible_observed = visible_outcome.clone();
    let _visible_subscription = cx.update(|window, cx| {
        window
            .bring_into_view_with_completion(
                &visible_target,
                BringIntoViewOptions::nearest(),
                cx,
                move |outcome, _, _| *visible_observed.borrow_mut() = Some(outcome),
            )
            .unwrap()
            .1
    });
    cx.run_until_parked();
    assert_eq!(visible_scroll.offset(), point(px(0.0), px(0.0)));
    assert_eq!(
        *visible_outcome.borrow(),
        Some(BringIntoViewOutcome::Completed(
            crate::BringIntoViewCompletion::AlreadyVisible
        ))
    );

    let no_progress_outcome = Rc::new(RefCell::new(None));
    let (blocked_view, cx) = cx.add_window_view(|window, _| PositionedScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        mounted: true,
        content_width: px(100.0),
        content_height: px(100.0),
        target_left: px(220.0),
        target_top: px(220.0),
        target_width: px(20.0),
        target_height: px(20.0),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let blocked_target = cx.update_window_entity(&blocked_view, |view, _, _| view.target);
    let no_progress_observed = no_progress_outcome.clone();
    let _blocked_subscription = cx.update(|window, cx| {
        window
            .bring_into_view_with_completion(
                &blocked_target,
                BringIntoViewOptions::nearest(),
                cx,
                move |outcome, _, _| *no_progress_observed.borrow_mut() = Some(outcome),
            )
            .unwrap()
            .1
    });
    cx.run_until_parked();
    assert_eq!(
        *no_progress_outcome.borrow(),
        Some(BringIntoViewOutcome::Cancelled(
            BringIntoViewCancelReason::NoProgress
        ))
    );
}

#[open_gpui::test]
fn target_unmount_and_window_close_cancel_with_exact_reasons(cx: &mut TestAppContext) {
    let unlinked_outcome = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view(|window, _| PositionedScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        mounted: true,
        content_width: px(300.0),
        content_height: px(300.0),
        target_left: px(220.0),
        target_top: px(220.0),
        target_width: px(20.0),
        target_height: px(20.0),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let target = cx.update_window_entity(&view, |view, _, _| view.target);
    let unlinked_observed = unlinked_outcome.clone();
    let _unlinked_subscription = cx.update(|window, cx| {
        let subscription = window
            .bring_into_view_with_completion(
                &target,
                BringIntoViewOptions::nearest(),
                cx,
                move |outcome, _, _| *unlinked_observed.borrow_mut() = Some(outcome),
            )
            .unwrap()
            .1;
        view.update(cx, |view, cx| {
            view.mounted = false;
            cx.notify();
        });
        subscription
    });
    cx.run_until_parked();
    assert_eq!(
        *unlinked_outcome.borrow(),
        Some(BringIntoViewOutcome::Cancelled(
            BringIntoViewCancelReason::TargetUnlinked
        ))
    );

    let closed_outcome = Rc::new(RefCell::new(None));
    let (closed_view, cx) = cx.add_window_view(|window, _| PositionedScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        mounted: true,
        content_width: px(300.0),
        content_height: px(300.0),
        target_left: px(220.0),
        target_top: px(220.0),
        target_width: px(20.0),
        target_height: px(20.0),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let closed_target = cx.update_window_entity(&closed_view, |view, _, _| view.target);
    let closed_observed = closed_outcome.clone();
    let _closed_subscription = cx.update(|window, cx| {
        let subscription = window
            .bring_into_view_with_completion(
                &closed_target,
                BringIntoViewOptions::nearest(),
                cx,
                move |outcome, _, _| *closed_observed.borrow_mut() = Some(outcome),
            )
            .unwrap()
            .1;
        window.remove_window(cx);
        subscription
    });
    cx.run_until_parked();
    assert_eq!(
        *closed_outcome.borrow(),
        Some(BringIntoViewOutcome::Cancelled(
            BringIntoViewCancelReason::WindowClosed
        ))
    );
}

#[open_gpui::test]
fn reduced_motion_finishes_immediately_and_window_portals_reset_scroll_ancestry(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|window, _| SingleScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        target_presentation: SubtreePresentation::Visible,
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (target, scroll) =
        cx.update_window_entity(&view, |view, _, _| (view.target, view.scroll.clone()));
    let transition = MotionTransition::duration(
        MotionIntent::Continuity,
        MotionPreference::Reduced,
        MotionDuration::Custom(Duration::from_millis(100)),
        MotionEasing::Linear,
    );
    cx.update(|window, cx| {
        window
            .bring_into_view(
                &target,
                BringIntoViewOptions::nearest()
                    .with_behavior(crate::BringIntoViewBehavior::Animated(transition)),
                cx,
            )
            .unwrap();
    });
    assert_eq!(scroll.offset(), point(px(-140.0), px(-140.0)));

    let portal_outcome = Rc::new(RefCell::new(None));
    let (portal_view, cx) = cx.add_window_view(|window, _| PortalScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (portal_target, portal_scroll) = cx.update_window_entity(&portal_view, |view, _, _| {
        (view.target, view.scroll.clone())
    });
    let portal_observed = portal_outcome.clone();
    let _portal_subscription = cx.update(|window, cx| {
        window
            .bring_into_view_with_completion(
                &portal_target,
                BringIntoViewOptions::nearest(),
                cx,
                move |outcome, _, _| *portal_observed.borrow_mut() = Some(outcome),
            )
            .unwrap()
            .1
    });
    cx.run_until_parked();
    assert_eq!(portal_scroll.offset(), point(px(0.0), px(0.0)));
    assert_eq!(
        *portal_outcome.borrow(),
        Some(BringIntoViewOutcome::Completed(
            crate::BringIntoViewCompletion::AlreadyVisible
        ))
    );
}

#[open_gpui::test]
fn reveal_uses_the_effective_overflow_clip_inside_a_visible_border(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, _| BorderedScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (target, scroll) =
        cx.update_window_entity(&view, |view, _, _| (view.target, view.scroll.clone()));
    cx.update(|window, cx| {
        window
            .bring_into_view(&target, BringIntoViewOptions::nearest(), cx)
            .unwrap();
    });
    cx.run_until_parked();

    assert_eq!(scroll.offset(), point(px(-2.0), px(-2.0)));
}

#[open_gpui::test]
fn direct_scroll_before_commit_cancels_reveal_without_overwriting_it(cx: &mut TestAppContext) {
    let outcome = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view(|window, _| SingleScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        target_presentation: SubtreePresentation::Visible,
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (target, scroll) =
        cx.update_window_entity(&view, |view, _, _| (view.target, view.scroll.clone()));

    let observed = outcome.clone();
    let _subscription = cx.update(|window, cx| {
        window
            .bring_into_view_with_completion(
                &target,
                BringIntoViewOptions::nearest(),
                cx,
                move |result, _, _| *observed.borrow_mut() = Some(result),
            )
            .expect("the target belongs to this window")
            .1
    });
    scroll.set_offset(point(px(-15.0), px(-25.0)));
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert_eq!(scroll.offset(), point(px(-15.0), px(-25.0)));
    assert_eq!(
        *outcome.borrow(),
        Some(BringIntoViewOutcome::Cancelled(
            BringIntoViewCancelReason::ScrollOverridden
        ))
    );
}

#[open_gpui::test]
fn direct_scroll_on_a_preserved_axis_does_not_cancel_the_requested_axis(cx: &mut TestAppContext) {
    let outcome = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view(|window, _| SingleScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        target_presentation: SubtreePresentation::Visible,
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (target, scroll) =
        cx.update_window_entity(&view, |view, _, _| (view.target, view.scroll.clone()));

    let observed = outcome.clone();
    let _subscription = cx.update(|window, cx| {
        let subscription = window
            .bring_into_view_with_completion(
                &target,
                BringIntoViewOptions::vertical(BringIntoViewAlignment::MaxEdge),
                cx,
                move |result, _, _| *observed.borrow_mut() = Some(result),
            )
            .expect("the target belongs to this window")
            .1;
        scroll.set_offset(point(px(-30.0), px(0.0)));
        subscription
    });

    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert_eq!(scroll.offset(), point(px(-30.0), px(-140.0)));
    assert_eq!(
        *outcome.borrow(),
        Some(BringIntoViewOutcome::Completed(
            crate::BringIntoViewCompletion::Revealed
        ))
    );
}

#[open_gpui::test]
fn suppressed_target_and_wrong_window_fail_closed(cx: &mut TestAppContext) {
    let outcome = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view(|window, _| SingleScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        target_presentation: SubtreePresentation::Inert,
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let target = cx.update_window_entity(&view, |view, _, _| view.target);
    let observed = outcome.clone();
    let _subscription = cx.update(|window, cx| {
        window
            .bring_into_view_with_completion(
                &target,
                BringIntoViewOptions::nearest(),
                cx,
                move |result, _, _| *observed.borrow_mut() = Some(result),
            )
            .expect("the target belongs to this window")
            .1
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert_eq!(
        *outcome.borrow(),
        Some(BringIntoViewOutcome::Cancelled(
            BringIntoViewCancelReason::TargetSuppressed
        ))
    );

    let second = cx.open_window(size(px(100.0), px(100.0)), |_, _| Empty);
    let error = second
        .update(cx, |_, window, cx| {
            window.bring_into_view(&target, BringIntoViewOptions::nearest(), cx)
        })
        .expect("the second window remains open")
        .expect_err("cross-window reveal must be rejected");
    assert!(matches!(error, BringIntoViewError::WrongWindow { .. }));
}

#[open_gpui::test]
fn only_the_winning_focus_claim_reveals(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| FocusScrollRevealView {
        scroll: ScrollHandle::new(),
        visible_focus: cx.focus_handle(),
        offscreen_focus: cx.focus_handle(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (scroll, visible, offscreen) = cx.update_window_entity(&view, |view, _, _| {
        (
            view.scroll.clone(),
            view.visible_focus.clone(),
            view.offscreen_focus.clone(),
        )
    });

    cx.update(|window, cx| {
        offscreen.focus(window, cx);
        visible.focus(window, cx);
    });
    cx.run_until_parked();
    assert_eq!(scroll.offset(), point(px(0.0), px(0.0)));

    cx.update(|window, cx| offscreen.focus(window, cx));
    cx.run_until_parked();
    assert_eq!(scroll.offset(), point(px(-140.0), px(-140.0)));
}

#[open_gpui::test]
fn guarded_focus_reveal_uses_an_uninterrupted_committed_scroll_fence(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| GuardedFocusRevealView {
        scroll: ScrollHandle::new(),
        scroll_chain_anchor: window.new_reveal_target(),
        offscreen_focus: cx.focus_handle(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (scroll, anchor, focus) = cx.update_window_entity(&view, |view, _, _| {
        (
            view.scroll.clone(),
            view.scroll_chain_anchor,
            view.offscreen_focus.clone(),
        )
    });
    let fence = cx.update(|window, _| {
        window
            .capture_committed_scroll_chain_fence(&anchor, BringIntoViewOptions::nearest())
            .expect("the anchor belongs to this window")
            .expect("the committed anchor should have one linked scroll chain")
    });

    cx.update(|window, cx| {
        window
            .focus_with_completion_and_scroll_fence(&focus, fence, cx, |_, _, _| {})
            .detach();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert_eq!(scroll.offset(), point(px(-140.0), px(-140.0)));
}

#[open_gpui::test]
fn guarded_focus_reveal_preserves_intervening_direct_scroll(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| GuardedFocusRevealView {
        scroll: ScrollHandle::new(),
        scroll_chain_anchor: window.new_reveal_target(),
        offscreen_focus: cx.focus_handle(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (scroll, anchor, focus) = cx.update_window_entity(&view, |view, _, _| {
        (
            view.scroll.clone(),
            view.scroll_chain_anchor,
            view.offscreen_focus.clone(),
        )
    });
    let fence = cx.update(|window, _| {
        window
            .capture_committed_scroll_chain_fence(&anchor, BringIntoViewOptions::nearest())
            .expect("the anchor belongs to this window")
            .expect("the committed anchor should have one linked scroll chain")
    });

    cx.update(|window, cx| {
        window
            .focus_with_completion_and_scroll_fence(&focus, fence, cx, |_, _, _| {})
            .detach();
    });
    scroll.set_offset(point(px(-20.0), px(-20.0)));
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert_eq!(scroll.offset(), point(px(-20.0), px(-20.0)));
}

#[open_gpui::test]
fn stale_focus_reveal_stops_after_focus_moves_to_a_disjoint_chain(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| DisjointFocusRevealView {
        left_outer: ScrollHandle::new(),
        left_inner: ScrollHandle::new(),
        left_focus: cx.focus_handle(),
        right_outer: ScrollHandle::new(),
        right_inner: ScrollHandle::new(),
        right_focus: cx.focus_handle(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (left_outer, left_inner, left_focus, right_inner, right_focus) =
        cx.update_window_entity(&view, |view, _, _| {
            (
                view.left_outer.clone(),
                view.left_inner.clone(),
                view.left_focus.clone(),
                view.right_inner.clone(),
                view.right_focus.clone(),
            )
        });

    cx.update(|window, cx| {
        left_focus.focus(window, cx);
        window.draw(cx).clear();
        right_focus.focus(window, cx);
    });
    assert_ne!(left_inner.offset(), point(px(0.0), px(0.0)));
    assert_eq!(left_outer.offset(), point(px(0.0), px(0.0)));

    cx.update(|window, cx| window.draw(cx).clear());

    assert_eq!(
        left_outer.offset(),
        point(px(0.0), px(0.0)),
        "the previous focus must not keep revealing a disjoint ancestry"
    );
    assert_ne!(right_inner.offset(), point(px(0.0), px(0.0)));
}

#[open_gpui::test]
fn committed_focus_listener_application_reveal_supersedes_automatic_focus_reveal(
    cx: &mut TestAppContext,
) {
    let application_outcome = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view(|window, cx| FocusListenerRevealView {
        scroll: ScrollHandle::new(),
        focus: cx.focus_handle(),
        application_target: window.new_reveal_target(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (scroll, focus, application_target) = cx.update_window_entity(&view, |view, _, _| {
        (
            view.scroll.clone(),
            view.focus.clone(),
            view.application_target,
        )
    });
    let observed = application_outcome.clone();
    let listener = cx.update(|window, cx| {
        window.on_focus_committed(&focus, cx, move |window, cx| {
            let observed = observed.clone();
            let subscription = window
                .bring_into_view_with_completion(
                    &application_target,
                    BringIntoViewOptions::nearest(),
                    cx,
                    move |outcome, _, _| *observed.borrow_mut() = Some(outcome),
                )
                .expect("application target belongs to the focused window")
                .1;
            subscription.detach();
        })
    });

    cx.update(|window, cx| focus.focus(window, cx));
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert_eq!(scroll.offset(), point(px(0.0), px(0.0)));
    assert_eq!(
        *application_outcome.borrow(),
        Some(BringIntoViewOutcome::Completed(
            crate::BringIntoViewCompletion::AlreadyVisible
        ))
    );
    drop(listener);
}

#[open_gpui::test]
fn committed_focus_listener_direct_scroll_cancels_automatic_focus_reveal(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| FocusListenerRevealView {
        scroll: ScrollHandle::new(),
        focus: cx.focus_handle(),
        application_target: window.new_reveal_target(),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (scroll, focus) = cx.update_window_entity(&view, |view, _, _| {
        (view.scroll.clone(), view.focus.clone())
    });
    let listener_scroll = scroll.clone();
    let listener = cx.update(|window, cx| {
        window.on_focus_committed(&focus, cx, move |_, _| {
            listener_scroll.set_offset(point(px(-20.0), px(-20.0)));
        })
    });

    cx.update(|window, cx| focus.focus(window, cx));
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| window.draw(cx).clear());

    assert_eq!(scroll.offset(), point(px(-20.0), px(-20.0)));
    drop(listener);
}

#[open_gpui::test]
fn accesskit_scroll_into_view_uses_the_common_authority(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, _| SingleScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        target_presentation: SubtreePresentation::Visible,
    });
    let scroll = cx.update_window_entity(&view, |view, _, _| view.scroll.clone());
    assert!(cx.activate_accessibility());
    let update = cx
        .latest_accessibility_tree_update()
        .expect("accessibility activation should publish the target");
    let (node_id, node) = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Single scroll reveal target"))
        .expect("the semantic target should be published");
    assert!(node.supports_action(AccessibleAction::ScrollIntoView));

    assert!(cx.dispatch_accessibility_action(ActionRequest {
        action: AccessibleAction::ScrollIntoView,
        target_tree: TreeId::ROOT,
        target_node: *node_id,
        data: None,
    }));
    cx.run_until_parked();
    assert_eq!(scroll.offset(), point(px(-140.0), px(-140.0)));
}

#[open_gpui::test]
fn stale_accesskit_reveal_is_cancelled_after_activation_generation_aba(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, _| SingleScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        target_presentation: SubtreePresentation::Visible,
    });
    let scroll = cx.update_window_entity(&view, |view, _, _| view.scroll.clone());
    assert!(cx.activate_accessibility());
    let update = cx.latest_accessibility_tree_update().unwrap();
    let node_id = update
        .nodes
        .iter()
        .find_map(|(id, node)| (node.label() == Some("Single scroll reveal target")).then_some(*id))
        .unwrap();

    cx.update(|window, cx| {
        let generation = window.accessibility_activation_generation_for_test();
        window.handle_a11y_action(
            generation,
            ActionRequest {
                action: AccessibleAction::ScrollIntoView,
                target_tree: TreeId::ROOT,
                target_node: node_id,
                data: None,
            },
            cx,
        );
        window.set_accessibility_active_for_test(false);
        window.set_accessibility_active_for_test(true);
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| window.draw(cx).clear());

    assert_eq!(scroll.offset(), point(px(0.0), px(0.0)));
}

#[open_gpui::test]
fn stale_accesskit_reveal_is_cancelled_when_node_leaves_the_published_tree(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|window, _| AccessibilityScopedRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        exclude_target: false,
    });
    let scroll = cx.update_window_entity(&view, |view, _, _| view.scroll.clone());
    assert!(cx.activate_accessibility());
    let update = cx.latest_accessibility_tree_update().unwrap();
    let node_id = update
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.label() == Some("Stale accessibility reveal target")).then_some(*id)
        })
        .unwrap();

    cx.update(|window, cx| {
        let generation = window.accessibility_activation_generation_for_test();
        window.handle_a11y_action(
            generation,
            ActionRequest {
                action: AccessibleAction::ScrollIntoView,
                target_tree: TreeId::ROOT,
                target_node: node_id,
                data: None,
            },
            cx,
        );
        view.update(cx, |view, cx| {
            view.exclude_target = true;
            cx.notify();
        });
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let update = cx.latest_accessibility_tree_update().unwrap();
    assert!(
        update
            .nodes
            .iter()
            .all(|(_, node)| node.label() != Some("Stale accessibility reveal target")),
        "the delayed request target must be absent from the current published tree"
    );
    cx.update(|window, cx| window.draw(cx).clear());

    assert_eq!(scroll.offset(), point(px(0.0), px(0.0)));
}

#[open_gpui::test]
fn direct_scroll_cancels_an_in_flight_motion_sample(cx: &mut TestAppContext) {
    let outcome = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view(|window, _| SingleScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        target_presentation: SubtreePresentation::Visible,
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (target, scroll) =
        cx.update_window_entity(&view, |view, _, _| (view.target, view.scroll.clone()));
    let transition = MotionTransition::duration(
        MotionIntent::Continuity,
        MotionPreference::Animated,
        MotionDuration::Custom(Duration::from_millis(100)),
        MotionEasing::Linear,
    );
    let observed = outcome.clone();
    let _subscription = cx.update(|window, cx| {
        window
            .bring_into_view_with_completion(
                &target,
                BringIntoViewOptions::nearest()
                    .with_behavior(crate::BringIntoViewBehavior::Animated(transition)),
                cx,
                move |result, _, _| *observed.borrow_mut() = Some(result),
            )
            .expect("the target belongs to this window")
            .1
    });

    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(scroll.offset(), point(px(0.0), px(0.0)));
    cx.background_executor
        .advance_clock(Duration::from_millis(50));
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(scroll.offset(), point(px(-70.0), px(-70.0)));

    scroll.set_offset(point(px(-60.0), px(-60.0)));
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();
    assert_eq!(
        *outcome.borrow(),
        Some(BringIntoViewOutcome::Cancelled(
            BringIntoViewCancelReason::ScrollOverridden
        ))
    );

    cx.background_executor
        .advance_clock(Duration::from_millis(100));
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(scroll.offset(), point(px(-60.0), px(-60.0)));
}

#[open_gpui::test]
fn nearest_reveal_quantizes_fractional_clipping_away_from_zero(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, _| FractionalNearestRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        content_width: px(101.0),
        target_left: px(90.0),
        target_width: px(10.0),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (target, scroll) =
        cx.update_window_entity(&view, |view, _, _| (view.target, view.scroll.clone()));

    cx.update(|window, cx| {
        window
            .bring_into_view(
                &target,
                BringIntoViewOptions::horizontal(BringIntoViewAlignment::Nearest),
                cx,
            )
            .unwrap();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert_eq!(
        scroll.offset().x,
        px(-0.5),
        "any nonzero nearest-edge clipping must move at least one device pixel"
    );
}

#[open_gpui::test]
fn nearest_reveal_reports_no_progress_for_fractional_clipping_at_saturation(
    cx: &mut TestAppContext,
) {
    let outcome = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view(|window, _| FractionalNearestRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        content_width: px(100.0),
        target_left: px(90.0),
        target_width: px(10.0),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let target = cx.update_window_entity(&view, |view, _, _| view.target);
    let observed = outcome.clone();
    let _subscription = cx.update(|window, cx| {
        window
            .bring_into_view_with_completion(
                &target,
                BringIntoViewOptions::horizontal(BringIntoViewAlignment::Nearest),
                cx,
                move |result, _, _| *observed.borrow_mut() = Some(result),
            )
            .unwrap()
            .1
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.run_until_parked();

    assert_eq!(
        *outcome.borrow(),
        Some(BringIntoViewOutcome::Cancelled(
            BringIntoViewCancelReason::NoProgress
        ))
    );
}

#[open_gpui::test]
fn nearest_reveal_terminates_when_no_device_pixel_can_fit_an_equal_width_target(
    cx: &mut TestAppContext,
) {
    let outcome = Rc::new(RefCell::new(None));
    let (view, cx) = cx.add_window_view(|window, _| FractionalNearestRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        content_width: px(101.0),
        target_left: px(0.0),
        target_width: px(100.0),
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let (target, scroll) =
        cx.update_window_entity(&view, |view, _, _| (view.target, view.scroll.clone()));
    let observed = outcome.clone();
    let _subscription = cx.update(|window, cx| {
        window
            .bring_into_view_with_completion(
                &target,
                BringIntoViewOptions::horizontal(BringIntoViewAlignment::Nearest),
                cx,
                move |result, _, _| *observed.borrow_mut() = Some(result),
            )
            .unwrap()
            .1
    });

    for _ in 0..4 {
        cx.update(|window, cx| window.draw(cx).clear());
        cx.run_until_parked();
    }

    assert_eq!(
        *outcome.borrow(),
        Some(BringIntoViewOutcome::Cancelled(
            BringIntoViewCancelReason::NoProgress
        )),
        "an unrepresentable nearest placement must terminate instead of alternating offsets"
    );
    assert_eq!(
        scroll.offset().x,
        px(0.0),
        "the authority should keep the least-clipped device-pixel placement"
    );
}

#[open_gpui::test]
fn reveal_converts_window_delta_through_a_non_uniform_container_transform(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, _| TransformedScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
    });
    let (target, scroll) =
        cx.update_window_entity(&view, |view, _, _| (view.target, view.scroll.clone()));
    cx.update(|window, cx| {
        window
            .bring_into_view(&target, BringIntoViewOptions::nearest(), cx)
            .expect("the target belongs to this window");
    });
    cx.run_until_parked();
    assert_eq!(scroll.offset(), point(px(-140.0), px(-140.0)));
}

#[open_gpui::test]
fn physical_alignment_margins_and_axis_preservation_are_explicit(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, _| SingleScrollRevealView {
        target: window.new_reveal_target(),
        scroll: ScrollHandle::new(),
        target_presentation: SubtreePresentation::Visible,
    });
    let (target, scroll) =
        cx.update_window_entity(&view, |view, _, _| (view.target, view.scroll.clone()));

    cx.update(|window, cx| {
        window
            .bring_into_view(
                &target,
                BringIntoViewOptions::aligned(BringIntoViewAlignment::Center),
                cx,
            )
            .unwrap();
    });
    cx.run_until_parked();
    assert_eq!(scroll.offset(), point(px(-180.0), px(-180.0)));

    scroll.set_offset(point(px(0.0), px(0.0)));
    let margins = BringIntoViewMargins::try_new(px(10.0), px(10.0), px(10.0), px(10.0))
        .expect("finite non-negative margins are valid");
    cx.update(|window, cx| {
        window
            .bring_into_view(
                &target,
                BringIntoViewOptions::aligned(BringIntoViewAlignment::MaxEdge)
                    .with_margins(margins),
                cx,
            )
            .unwrap();
    });
    cx.run_until_parked();
    assert_eq!(scroll.offset(), point(px(-150.0), px(-150.0)));

    scroll.set_offset(point(px(-30.0), px(0.0)));
    cx.update(|window, cx| {
        window
            .bring_into_view(
                &target,
                BringIntoViewOptions::vertical(BringIntoViewAlignment::MaxEdge),
                cx,
            )
            .unwrap();
    });
    cx.run_until_parked();
    assert_eq!(scroll.offset(), point(px(-30.0), px(-140.0)));
}
