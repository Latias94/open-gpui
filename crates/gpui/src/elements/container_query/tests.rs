use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use super::container_query;
use crate::{
    AppContext as _, Bounds, Context, Corners, Hitbox, HitboxBehavior, IntoElement,
    MeasuredElementSnapshot, ParentElement, Pixels, Render, Size, Styled, SubtreeClip,
    SubtreeClipExt, SubtreePresentation, SubtreePresentationExt, SubtreeTransform,
    SubtreeTransformExt, SubtreeTransformOrigin, TestAppContext, VisualContext as _, Window,
    canvas, deferred, div, fill, measured_element, point, px, red, size,
};

struct AllocatedSizeView {
    measurements: Rc<RefCell<Vec<(Size<Pixels>, MeasuredElementSnapshot)>>>,
}

impl Render for AllocatedSizeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let measurements = self.measurements.clone();
        div()
            .size_full()
            .flex()
            .p(px(10.))
            .child(div().w(px(80.)).h_full().flex_shrink_0())
            .child(
                container_query(move |allocated, _, _| {
                    let height = if allocated.width < px(200.) { 24. } else { 48. };
                    measured_element(
                        "responsive-content",
                        div().w(px(1000.)).h(px(height)),
                        move |snapshot, _| {
                            measurements.borrow_mut().push((allocated, snapshot));
                        },
                    )
                })
                .flex_1()
                .min_w_0(),
            )
    }
}

#[crate::test]
fn container_query_builds_from_the_current_flex_allocation(cx: &mut TestAppContext) {
    let measurements = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(320.), px(200.)), |_, _| AllocatedSizeView {
        measurements: measurements.clone(),
    });
    measurements.borrow_mut().clear();
    let generation = cx
        .update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
            window.rendered_frame.generation
        })
        .unwrap();

    let (allocated, first) = measurements
        .borrow()
        .last()
        .cloned()
        .expect("the first frame should contain the responsive child");
    assert_eq!(allocated, size(px(220.), px(180.)));
    assert_eq!(first.frame_generation(), generation);
    assert_eq!(
        first.geometry().layout_bounds(),
        Bounds::new(point(px(90.), px(10.)), size(px(1000.), px(48.)))
    );

    cx.simulate_window_resize(window.into(), size(px(200.), px(180.)));
    measurements.borrow_mut().clear();
    let generation = cx
        .update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
            window.rendered_frame.generation
        })
        .unwrap();

    let (allocated, resized) = measurements
        .borrow()
        .last()
        .cloned()
        .expect("the resized frame should contain the rebuilt responsive child");
    assert_eq!(allocated, size(px(100.), px(160.)));
    assert_eq!(resized.frame_generation(), generation);
    assert_eq!(
        resized.geometry().layout_bounds(),
        Bounds::new(point(px(90.), px(10.)), size(px(1000.), px(24.)))
    );
    assert!(resized.frame_generation() > first.frame_generation());
}

struct ScopedQueryView {
    presentation: SubtreePresentation,
    fail_late: bool,
    builds: Rc<Cell<usize>>,
    commits: Rc<Cell<usize>>,
    hitbox: Rc<RefCell<Option<Hitbox>>>,
    painted_presentation: Rc<Cell<Option<SubtreePresentation>>>,
}

impl Render for ScopedQueryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let builds = self.builds.clone();
        let commits = self.commits.clone();
        let hitbox = self.hitbox.clone();
        let painted_presentation = self.painted_presentation.clone();
        let presentation = self.presentation;
        let fail_late = self.fail_late;
        let radius = size(px(20.), px(20.));

        div().size_full().child(
            container_query(move |allocated, window, _| {
                builds.set(builds.get() + 1);
                assert_eq!(allocated, size(px(100.), px(80.)));
                assert_eq!(window.subtree_presentation(), presentation);
                deferred(
                    canvas(
                        move |bounds, window, _| {
                            *hitbox.borrow_mut() =
                                Some(window.insert_hitbox(bounds, HitboxBehavior::Normal));
                            window.record_prepaint_commit(move |_, _| {
                                commits.set(commits.get() + 1);
                            });
                        },
                        move |bounds, _, window, _| {
                            painted_presentation.set(Some(window.subtree_presentation()));
                            window.paint_quad(fill(bounds, red()));
                            if fail_late {
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(px(f32::MAX), px(0.)),
                                        size(px(10.), px(10.)),
                                    ),
                                    red(),
                                ));
                            }
                        },
                    )
                    .size_full(),
                )
            })
            .absolute()
            .left(px(20.))
            .top(px(30.))
            .w(px(100.))
            .h(px(80.))
            .with_subtree_clip(
                SubtreeClip::try_own_rounded_border_box(Corners {
                    top_left: radius,
                    top_right: radius,
                    bottom_right: radius,
                    bottom_left: radius,
                })
                .unwrap(),
            )
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2., 3.),
                    point(px(100.), px(50.)),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            )
            .with_subtree_presentation(presentation),
        )
    }
}

#[crate::test]
fn container_query_deferred_child_retains_fork_scopes_and_commit_validity(cx: &mut TestAppContext) {
    let builds = Rc::new(Cell::new(0));
    let commits = Rc::new(Cell::new(0));
    let hitbox = Rc::new(RefCell::new(None));
    let painted_presentation = Rc::new(Cell::new(None));
    let (view, cx) = cx.add_window_view({
        let builds = builds.clone();
        let commits = commits.clone();
        let hitbox = hitbox.clone();
        let painted_presentation = painted_presentation.clone();
        move |_, _| ScopedQueryView {
            presentation: SubtreePresentation::Visible,
            fail_late: false,
            builds,
            commits,
            hitbox,
            painted_presentation,
        }
    });

    cx.update(|window, cx| window.draw(cx).clear(cx));
    let visible = hitbox.borrow().clone().expect("the deferred query child");
    assert_eq!(
        visible.layout_bounds(),
        Bounds::new(point(px(20.), px(30.)), size(px(100.), px(80.)))
    );
    assert_eq!(
        visible.displayed_bounds(),
        Bounds::new(point(px(120.), px(80.)), size(px(200.), px(240.)))
    );
    assert_eq!(visible.displayed_clip_bounds(), visible.displayed_bounds());
    assert!(!visible.contains_window_point(point(px(121.), px(81.))));
    assert!(visible.is_window_point_target(point(px(220.), px(200.))));
    assert_eq!(
        painted_presentation.get(),
        Some(SubtreePresentation::Visible)
    );

    cx.update_window_entity(&view, |view, _, cx| {
        view.presentation = SubtreePresentation::Inert;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let inert = hitbox
        .borrow()
        .clone()
        .expect("the inert child still paints");
    assert_eq!(inert.displayed_bounds(), visible.displayed_bounds());
    assert!(inert.contains_window_point(point(px(220.), px(200.))));
    assert!(!inert.is_window_point_target(point(px(220.), px(200.))));
    cx.update(|window, _| assert!(window.committed_hitbox(inert.id).is_none()));
    assert_eq!(painted_presentation.get(), Some(SubtreePresentation::Inert));

    let builds_before_hidden = builds.get();
    *hitbox.borrow_mut() = None;
    painted_presentation.set(None);
    cx.update_window_entity(&view, |view, _, cx| {
        view.presentation = SubtreePresentation::Hidden;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(builds.get(), builds_before_hidden);
    assert!(hitbox.borrow().is_none());
    assert_eq!(painted_presentation.get(), None);

    cx.update_window_entity(&view, |view, _, cx| {
        view.presentation = SubtreePresentation::Visible;
        view.fail_late = true;
        cx.notify();
    });
    commits.set(0);
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(commits.get(), 0);
    assert!(
        !hitbox
            .borrow()
            .as_ref()
            .expect("the rejected scope reached prepaint")
            .is_window_point_target(point(px(220.), px(200.)))
    );

    cx.update_window_entity(&view, |view, _, cx| {
        view.fail_late = false;
        cx.notify();
    });
    assert_eq!(commits.get(), 1);
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(commits.get(), 2);
    assert!(
        hitbox
            .borrow()
            .as_ref()
            .expect("the recovered scope should be rebuilt")
            .is_window_point_target(point(px(220.), px(200.)))
    );
}
