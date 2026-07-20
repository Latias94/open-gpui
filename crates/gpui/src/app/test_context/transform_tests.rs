use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
};

use crate::{
    AnyView, App, AppContext, Bounds, Context, Entity, FocusClaimOutcome, FocusHandle, Hitbox,
    HitboxBehavior, InputHandler, InteractiveElement, IntoElement, Modifiers, MouseButton,
    ParentElement, Pixels, Point, PointerCaptureHandle, PrepaintPublicationId, Render, Role,
    ScaledPixels, ScrollDelta, ScrollWheelEvent, ScrollWheelIntent, StatefulInteractiveElement,
    StyleRefinement, Styled, SubtreeTransform, SubtreeTransformError, SubtreeTransformExt,
    SubtreeTransformOrigin, TestAppContext, UTF16Selection, VisualContext, Window, canvas,
    deferred, div, fill, point, px, red, size, window_portal,
};

struct TransformAccessibilityView;

struct NumericFailureView {
    prepaints: Rc<Cell<usize>>,
}

struct CachedTransformChild {
    renders: Rc<Cell<usize>>,
    clicks: Rc<Cell<usize>>,
}

struct CachedTransformRoot {
    child: Entity<CachedTransformChild>,
    translated: bool,
}

struct DeferredTransformView {
    inherited: Rc<RefCell<Option<Hitbox>>>,
    portal: Rc<RefCell<Option<Hitbox>>>,
}

struct LateNumericFailureView {
    commits: Rc<Cell<usize>>,
    clicks: Rc<Cell<usize>>,
}

struct LateInvalidFocusFallbackView {
    committed_focus: FocusHandle,
    rejected_focus: FocusHandle,
}

struct LateInvalidDragSourceView {
    fail_late: Rc<Cell<bool>>,
    preview_paints: Rc<Cell<usize>>,
}

struct LateInvalidDragPreview {
    paints: Rc<Cell<usize>>,
}

struct PrepaintWindowTransactionView {
    publication: PrepaintPublicationId,
    publish: bool,
    rollback: bool,
    fail_late: bool,
    commits: Rc<Cell<usize>>,
    discards: Rc<Cell<usize>>,
}

struct DeferredLateNumericFailureView {
    commits: Rc<Cell<usize>>,
    clicks: Rc<Cell<usize>>,
}

struct InvalidatedCachedChild {
    renders: Rc<Cell<usize>>,
}

struct InvalidatedCachedRoot {
    child: Entity<InvalidatedCachedChild>,
}

struct CachedAncestorWithInvalidDescendant {
    renders: Rc<Cell<usize>>,
}

struct CachedInvalidDescendantRoot {
    child: Entity<CachedAncestorWithInvalidDescendant>,
}

#[derive(Clone)]
struct TransformImeInputHandler {
    bounds: Bounds<Pixels>,
}

struct CachedTransformImeChild {
    focus: FocusHandle,
}

struct TransformImeRoot {
    child: Entity<CachedTransformImeChild>,
    focus: FocusHandle,
    translated: bool,
}

struct TransformDragPreview;

impl Render for TransformDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(1.0)).h(px(1.0))
    }
}

type PointerCoordinateRecord = (Point<Pixels>, Point<Pixels>, Point<Pixels>);

struct NonuniformInteractionView {
    mouse_down: Rc<RefCell<Option<PointerCoordinateRecord>>>,
    click: Rc<RefCell<Option<PointerCoordinateRecord>>>,
    wheel: Rc<RefCell<Vec<(ScrollDelta, ScrollDelta)>>>,
}

struct TransformPointerCaptureView {
    capture: PointerCaptureHandle,
    translated: bool,
    moves: Rc<RefCell<Vec<PointerCoordinateRecord>>>,
}

impl Render for TransformPointerCaptureView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let capture = self.capture;
        let moves = self.moves.clone();
        let translation_x = if self.translated { 200.0 } else { 100.0 };
        let transform = SubtreeTransform::try_new(
            size(2.0, 2.0),
            point(px(translation_x), px(50.0)),
            SubtreeTransformOrigin::TOP_LEFT,
        )
        .unwrap();

        div().size_full().child(
            div()
                .id("transform-pointer-capture-target")
                .absolute()
                .left(px(20.0))
                .top(px(30.0))
                .w(px(100.0))
                .h(px(50.0))
                .track_pointer_capture(&self.capture)
                .on_mouse_down(MouseButton::Left, move |_, window, _| {
                    window
                        .capture_pointer(&capture, MouseButton::Left)
                        .expect("the transformed target should own its pointer session");
                })
                .on_mouse_move(move |event, _, _| {
                    moves.borrow_mut().push((
                        event.window_event().position,
                        event.target_local_position().unwrap(),
                        event.target_layout_position().unwrap(),
                    ));
                })
                .with_subtree_transform(transform),
        )
    }
}

impl Render for NonuniformInteractionView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mouse_down = self.mouse_down.clone();
        let click = self.click.clone();
        let wheel = self.wheel.clone();
        let transform = SubtreeTransform::try_new(
            size(2.0, 4.0),
            point(px(100.0), px(50.0)),
            SubtreeTransformOrigin::TOP_LEFT,
        )
        .unwrap();

        div().size_full().child(
            div()
                .id("nonuniform-interaction-target")
                .absolute()
                .left(px(20.0))
                .top(px(30.0))
                .w(px(100.0))
                .h(px(50.0))
                .on_mouse_down(MouseButton::Left, move |event, _, _| {
                    *mouse_down.borrow_mut() = Some((
                        event.window_event().position,
                        event.target_local_position().unwrap(),
                        event.target_layout_position().unwrap(),
                    ));
                })
                .on_click(move |event, _, _| {
                    *click.borrow_mut() = Some((
                        event.window_event().position(),
                        event.target_local_position().unwrap(),
                        event.target_layout_position().unwrap(),
                    ));
                })
                .on_scroll_wheel(move |event, _, _| {
                    wheel.borrow_mut().push((
                        event.window_event().delta,
                        event.target_local_delta().unwrap(),
                    ));
                    ScrollWheelIntent::allow_default()
                })
                .with_subtree_transform(transform),
        )
    }
}

type DragStartRecord = (
    u32,
    Point<Pixels>,
    Point<Pixels>,
    Point<Pixels>,
    Point<Pixels>,
);
type DragTargetRecord = (u32, Point<Pixels>, Point<Pixels>, Point<Pixels>);

struct NonuniformDragView {
    drag_start: Rc<RefCell<Option<DragStartRecord>>>,
    drag_move: Rc<RefCell<Option<DragTargetRecord>>>,
    drop: Rc<RefCell<Option<DragTargetRecord>>>,
}

impl Render for NonuniformDragView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let drag_start = self.drag_start.clone();
        let drag_move = self.drag_move.clone();
        let drop = self.drop.clone();
        let transform = SubtreeTransform::try_new(
            size(2.0, 4.0),
            point(px(100.0), px(50.0)),
            SubtreeTransformOrigin::TOP_LEFT,
        )
        .unwrap();

        div().size_full().child(
            div()
                .id("nonuniform-drag-target")
                .absolute()
                .left(px(20.0))
                .top(px(30.0))
                .w(px(100.0))
                .h(px(50.0))
                .on_drag(7_u32, move |value, geometry, _, cx| {
                    *drag_start.borrow_mut() = Some((
                        *value,
                        geometry.window_position(),
                        geometry.target_local_position().unwrap(),
                        geometry.target_layout_position().unwrap(),
                        geometry.window_preview_offset(),
                    ));
                    cx.new(|_| TransformDragPreview)
                })
                .on_drag_move::<u32>(move |event, _, _| {
                    *drag_move.borrow_mut() = Some((
                        *event.drag(),
                        event.window_position(),
                        event.target_local_position().unwrap(),
                        event.target_layout_position().unwrap(),
                    ));
                })
                .on_drop::<u32>(move |event, _, _| {
                    *drop.borrow_mut() = Some((
                        *event.value(),
                        event.pointer().window_event().position,
                        event.pointer().target_local_position().unwrap(),
                        event.pointer().target_layout_position().unwrap(),
                    ));
                })
                .with_subtree_transform(transform),
        )
    }
}

impl Render for TransformAccessibilityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("transformed-accessibility-node")
            .role(Role::Button)
            .aria_label("Transformed accessibility node")
            .w(px(40.0))
            .h(px(20.0))
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2.0, 3.0),
                    point(px(5.0), px(-7.0)),
                    SubtreeTransformOrigin::try_pixels(point(px(10.0), px(5.0))).unwrap(),
                )
                .unwrap(),
            )
    }
}

impl Render for NumericFailureView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let prepaints = self.prepaints.clone();
        canvas(
            move |bounds, window, _| {
                prepaints.set(prepaints.get() + 1);
                window.insert_hitbox(bounds, HitboxBehavior::Normal);
            },
            |_, _, _, _| {},
        )
        .w(px(40.0))
        .h(px(20.0))
        .with_subtree_transform(
            SubtreeTransform::try_new(
                size(2.0, 1.0),
                Point::default(),
                SubtreeTransformOrigin::TOP_LEFT,
            )
            .unwrap(),
        )
        .with_subtree_transform(
            SubtreeTransform::try_new(
                size(f32::MAX, 1.0),
                Point::default(),
                SubtreeTransformOrigin::TOP_LEFT,
            )
            .unwrap(),
        )
    }
}

impl Render for CachedTransformChild {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let clicks = self.clicks.clone();
        div()
            .id("cached-transform-child")
            .debug_selector(|| "cached-transform-child".to_owned())
            .size_full()
            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                clicks.set(clicks.get() + 1);
            })
    }
}

impl Render for CachedTransformRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let translation = if self.translated {
            point(px(100.0), px(100.0))
        } else {
            Point::default()
        };
        AnyView::from(self.child.clone())
            .cached(StyleRefinement::default().w(px(20.0)).h(px(20.0)))
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(1.0, 1.0),
                    translation,
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            )
    }
}

impl Render for DeferredTransformView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let inherited = self.inherited.clone();
        let portal = self.portal.clone();
        div()
            .w(px(40.0))
            .child(deferred(
                canvas(
                    move |bounds, window, _| {
                        *inherited.borrow_mut() =
                            Some(window.insert_hitbox(bounds, HitboxBehavior::Normal));
                    },
                    |_, _, _, _| {},
                )
                .w(px(20.0))
                .h(px(10.0)),
            ))
            .child(window_portal(
                canvas(
                    move |bounds, window, _| {
                        *portal.borrow_mut() =
                            Some(window.insert_hitbox(bounds, HitboxBehavior::Normal));
                    },
                    |_, _, _, _| {},
                )
                .w(px(20.0))
                .h(px(10.0)),
            ))
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2.0, 3.0),
                    point(px(100.0), px(50.0)),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            )
    }
}

impl Render for LateNumericFailureView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let commits = self.commits.clone();
        let clicks = self.clicks.clone();
        div()
            .id("late-transform-failure")
            .role(Role::Button)
            .aria_label("Late transform failure")
            .focusable()
            .w(px(40.0))
            .h(px(20.0))
            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                clicks.set(clicks.get() + 1);
            })
            .child(
                canvas(
                    move |_, window, _| {
                        let commits = commits.clone();
                        window.record_prepaint_commit(move |_, _| {
                            commits.set(commits.get() + 1);
                        });
                    },
                    |bounds, _, window, _| {
                        window.paint_quad(fill(bounds, red()));
                        window.paint_quad(fill(
                            Bounds::new(point(px(f32::MAX), px(0.0)), size(px(10.0), px(10.0))),
                            red(),
                        ));
                    },
                )
                .size_full(),
            )
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2.0, 2.0),
                    Point::default(),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            )
    }
}

impl Render for LateInvalidFocusFallbackView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .child(
                div()
                    .id("late-focus-fallback")
                    .w(px(40.0))
                    .h(px(20.0))
                    .focusable()
                    .track_focus(&self.committed_focus),
            )
            .child(
                div()
                    .id("late-invalid-focus-target")
                    .w(px(40.0))
                    .h(px(20.0))
                    .focusable()
                    .track_focus(&self.rejected_focus)
                    .child(
                        canvas(
                            |_, _, _| {},
                            |bounds, _, window, _| {
                                window.paint_quad(fill(bounds, red()));
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(px(f32::MAX), px(0.0)),
                                        size(px(10.0), px(10.0)),
                                    ),
                                    red(),
                                ));
                            },
                        )
                        .size_full(),
                    )
                    .with_subtree_transform(
                        SubtreeTransform::try_new(
                            size(2.0, 2.0),
                            Point::default(),
                            SubtreeTransformOrigin::TOP_LEFT,
                        )
                        .unwrap(),
                    ),
            )
    }
}

impl Render for LateInvalidDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let paints = self.paints.clone();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                paints.set(paints.get() + 1);
                window.paint_quad(fill(bounds, red()));
            },
        )
        .w(px(20.0))
        .h(px(20.0))
    }
}

impl Render for LateInvalidDragSourceView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let fail_late = self.fail_late.clone();
        let preview_paints = self.preview_paints.clone();
        div()
            .id("late-invalid-drag-source")
            .w(px(100.0))
            .h(px(100.0))
            .on_drag(17_u32, move |_, _, _, cx| {
                let paints = preview_paints.clone();
                cx.new(|_| LateInvalidDragPreview { paints })
            })
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        window.paint_quad(fill(bounds, red()));
                        if fail_late.get() {
                            window.paint_quad(fill(
                                Bounds::new(point(px(f32::MAX), px(0.0)), size(px(10.0), px(10.0))),
                                red(),
                            ));
                        }
                    },
                )
                .size_full(),
            )
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2.0, 2.0),
                    Point::default(),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            )
    }
}

impl Render for PrepaintWindowTransactionView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let publication = self.publication;
        let publish = self.publish;
        let rollback = self.rollback;
        let commits = self.commits.clone();
        let discards = self.discards.clone();
        let fail_late = self.fail_late;
        let mut root = div().w(px(40.0)).h(px(20.0));
        if publish {
            root = root.child(
                canvas(
                    move |_, window, _| {
                        let record = |window: &mut Window| {
                            let commits = commits.clone();
                            let discards = discards.clone();
                            window.record_prepaint_window_transaction(
                                publication,
                                move |_, _: &mut Window, _: &mut App| {
                                    commits.set(commits.get() + 1)
                                },
                                move |_, _: &mut Window, _: &mut App| {
                                    discards.set(discards.get() + 1)
                                },
                            );
                        };
                        if rollback {
                            let result: Result<(), ()> = window.transact(|window| {
                                record(window);
                                Err(())
                            });
                            debug_assert!(result.is_err());
                        } else {
                            record(window);
                        }
                    },
                    move |bounds, _, window, _| {
                        window.paint_quad(fill(bounds, red()));
                        if fail_late {
                            window.paint_quad(fill(
                                Bounds::new(point(px(f32::MAX), px(0.0)), size(px(10.0), px(10.0))),
                                red(),
                            ));
                        }
                    },
                )
                .size_full(),
            );
        }
        root.with_subtree_transform(
            SubtreeTransform::try_new(
                size(2.0, 2.0),
                Point::default(),
                SubtreeTransformOrigin::TOP_LEFT,
            )
            .unwrap(),
        )
    }
}

impl Render for DeferredLateNumericFailureView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let commits = self.commits.clone();
        let clicks = self.clicks.clone();
        deferred(
            div()
                .id("deferred-late-transform-failure")
                .role(Role::Button)
                .aria_label("Deferred late transform failure")
                .focusable()
                .w(px(40.0))
                .h(px(20.0))
                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                    clicks.set(clicks.get() + 1);
                })
                .child(
                    canvas(
                        move |_, window, _| {
                            let commits = commits.clone();
                            window.record_prepaint_commit(move |_, _| {
                                commits.set(commits.get() + 1);
                            });
                        },
                        |bounds, _, window, _| {
                            window.paint_quad(fill(bounds, red()));
                            window.paint_quad(fill(
                                Bounds::new(point(px(f32::MAX), px(0.0)), size(px(10.0), px(10.0))),
                                red(),
                            ));
                        },
                    )
                    .size_full(),
                ),
        )
        .with_subtree_transform(
            SubtreeTransform::try_new(
                size(2.0, 2.0),
                Point::default(),
                SubtreeTransformOrigin::TOP_LEFT,
            )
            .unwrap(),
        )
    }
}

impl Render for InvalidatedCachedChild {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().size_full()
    }
}

impl Render for InvalidatedCachedRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(40.0))
            .h(px(20.0))
            .child(AnyView::from(self.child.clone()).cached(StyleRefinement::default().size_full()))
            .child(canvas(
                |_, window, _| {
                    window.insert_hitbox(
                        Bounds::new(point(px(f32::MAX), px(0.0)), size(px(10.0), px(10.0))),
                        HitboxBehavior::Normal,
                    );
                },
                |_, _, _, _| {},
            ))
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2.0, 2.0),
                    Point::default(),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            )
    }
}

impl Render for CachedAncestorWithInvalidDescendant {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div()
            .id("cached-ancestor-invalid-descendant")
            .size_full()
            .on_mouse_down(MouseButton::Left, |_, _, _| {})
            .child(canvas(
                |_, _, _| {},
                |_, _, window, _| {
                    window.paint_quad(fill(
                        Bounds::new(point(px(f32::MAX), px(0.0)), size(px(10.0), px(10.0))),
                        red(),
                    ));
                },
            ))
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2.0, 2.0),
                    Point::default(),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            )
    }
}

impl Render for CachedInvalidDescendantRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        AnyView::from(self.child.clone()).cached(StyleRefinement::default().w(px(40.0)).h(px(20.0)))
    }
}

impl InputHandler for TransformImeInputHandler {
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        Some(String::new())
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        Some(self.bounds)
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        Some(0)
    }
}

impl Render for CachedTransformImeChild {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.clone();
        let handler = TransformImeInputHandler {
            bounds: Bounds::new(point(px(5.0), px(4.0)), size(px(1.0), px(10.0))),
        };
        div()
            .id("cached-transform-ime-child")
            .size_full()
            .focusable()
            .track_focus(&self.focus)
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, cx| window.handle_input(&focus, handler.clone(), cx),
                )
                .size_full(),
            )
    }
}

impl Render for TransformImeRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let translation = if self.translated {
            point(px(100.0), px(50.0))
        } else {
            Point::default()
        };
        AnyView::from(self.child.clone())
            .cached(StyleRefinement::default().w(px(40.0)).h(px(20.0)))
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(2.0, 2.0),
                    translation,
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            )
    }
}

#[open_gpui::test]
fn subtree_transform_preserves_layout_and_projects_hitbox_geometry(cx: &mut TestAppContext) {
    let layout_bounds = Rc::new(RefCell::new(None));
    let hitbox = Rc::new(RefCell::new(None::<Hitbox>));
    let transform = SubtreeTransform::try_new(
        size(2.0, 3.0),
        point(px(5.0), px(-7.0)),
        SubtreeTransformOrigin::try_pixels(point(px(10.0), px(5.0))).unwrap(),
    )
    .unwrap();
    let visual = cx.add_empty_window();

    visual.draw(point(px(100.0), px(200.0)), size(px(40.0), px(20.0)), {
        let layout_bounds = layout_bounds.clone();
        let hitbox = hitbox.clone();
        move |_, _| {
            canvas(
                move |bounds, window, _| {
                    *layout_bounds.borrow_mut() = Some(bounds);
                    let inserted = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    *hitbox.borrow_mut() = Some(inserted);
                },
                |_, _, _, _| {},
            )
            .size_full()
            .with_subtree_transform(transform)
        }
    });

    let expected_layout = Bounds::new(point(px(100.0), px(200.0)), size(px(40.0), px(20.0)));
    assert_eq!(*layout_bounds.borrow(), Some(expected_layout));

    let hitbox = hitbox.borrow();
    let hitbox = hitbox.as_ref().unwrap();
    assert_eq!(hitbox.layout_bounds(), expected_layout);
    assert_eq!(
        hitbox.displayed_bounds(),
        Bounds::new(point(px(95.0), px(183.0)), size(px(80.0), px(60.0)))
    );

    let local_point = point(px(12.0), px(7.0));
    let window_point = point(px(119.0), px(204.0));
    assert_eq!(
        hitbox.local_to_window_point(local_point).unwrap(),
        window_point
    );
    assert_eq!(
        hitbox.window_to_local_point(window_point).unwrap(),
        local_point
    );
    assert_eq!(
        hitbox
            .layout_to_window_point(point(px(112.0), px(207.0)))
            .unwrap(),
        window_point
    );
    assert_eq!(
        hitbox.window_to_layout_point(window_point).unwrap(),
        point(px(112.0), px(207.0))
    );
    assert_eq!(
        hitbox
            .local_to_window_vector(point(px(4.0), px(5.0)))
            .unwrap(),
        point(px(8.0), px(15.0))
    );
    assert_eq!(
        hitbox
            .window_to_local_vector(point(px(8.0), px(15.0)))
            .unwrap(),
        point(px(4.0), px(5.0))
    );
    assert!(hitbox.contains_window_point(window_point));
    assert!(!hitbox.contains_window_point(Point::<Pixels>::default()));
}

#[open_gpui::test]
fn paint_quad_snaps_device_edges_after_transform_projection(cx: &mut TestAppContext) {
    let transform = SubtreeTransform::try_new(
        size(2.0, 2.0),
        point(px(0.2), px(0.2)),
        SubtreeTransformOrigin::TOP_LEFT,
    )
    .unwrap();
    let local_bounds = Bounds::new(point(px(0.2), px(0.2)), size(px(0.5), px(0.5)));
    let visual = cx.add_empty_window();

    visual.draw(Point::default(), size(px(20.0), px(20.0)), move |_, _| {
        canvas(
            |_, _, _| {},
            move |_, _, window, _| window.paint_quad(fill(local_bounds, red())),
        )
        .size_full()
        .with_subtree_transform(transform)
    });

    visual.update(|window, _| {
        let quad = window
            .rendered_frame
            .scene
            .quads
            .first()
            .expect("transformed quad missing");
        assert_eq!(
            quad.transform.try_project_bounds(quad.bounds).unwrap(),
            Bounds::new(
                point(ScaledPixels(1.0), ScaledPixels(1.0)),
                size(ScaledPixels(2.0), ScaledPixels(2.0)),
            )
        );
        assert_eq!(
            quad.bounds,
            Bounds::new(
                point(ScaledPixels(0.4), ScaledPixels(0.4)),
                size(ScaledPixels(1.0), ScaledPixels(1.0)),
            ),
            "local shading bounds must remain unsnapped"
        );
    });
}

#[open_gpui::test]
fn subtree_transform_projects_final_accessibility_bounds_once(cx: &mut TestAppContext) {
    let window = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| {
            TransformAccessibilityView
        })
        .into();

    assert!(cx.activate_accessibility(window));
    let update = cx.latest_accessibility_tree_update(window).unwrap();
    let node = update
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.label() == Some("Transformed accessibility node")).then_some(node)
        })
        .expect("transformed accessibility node missing");

    assert_eq!(
        node.bounds(),
        Some(accesskit::Rect {
            x0: -10.0,
            y0: -34.0,
            x1: 150.0,
            y1: 86.0,
        })
    );
}

#[open_gpui::test]
fn nested_transform_composition_failure_suppresses_the_whole_child_before_prepaint(
    cx: &mut TestAppContext,
) {
    let prepaints = Rc::new(Cell::new(0));
    let window = cx.open_window(size(px(320.0), px(200.0)), {
        let prepaints = prepaints.clone();
        move |_, _| NumericFailureView { prepaints }
    });

    assert_eq!(prepaints.get(), 0);
    window
        .update(cx, |_, window, _| {
            assert!(window.rendered_frame.hitboxes.is_empty());
            assert!(window.rendered_frame.scene.len() == 0);
            assert_eq!(window.rendered_frame.subtree_transform_diagnostics.len(), 1);
        })
        .unwrap();
}

#[open_gpui::test]
fn transformed_hit_testing_uses_displayed_geometry_and_preserves_window_event_coordinates(
    cx: &mut TestAppContext,
) {
    let received_positions = Rc::new(RefCell::new(Vec::new()));
    let transform = SubtreeTransform::try_new(
        size(2.0, 2.0),
        point(px(100.0), px(100.0)),
        SubtreeTransformOrigin::TOP_LEFT,
    )
    .unwrap();
    let visual = cx.add_empty_window();

    visual.draw(point(px(50.0), px(50.0)), size(px(20.0), px(20.0)), {
        let received_positions = received_positions.clone();
        move |_, _| {
            div()
                .id("transformed-pointer-target")
                .size_full()
                .on_mouse_down(MouseButton::Left, move |event, window, _| {
                    assert_eq!(window.mouse_position(), event.window_event().position);
                    received_positions
                        .borrow_mut()
                        .push(event.window_event().position);
                })
                .with_subtree_transform(transform)
        }
    });

    visual.simulate_mouse_down(
        point(px(60.0), px(60.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert!(received_positions.borrow().is_empty());

    let displayed_point = point(px(160.0), px(160.0));
    visual.simulate_mouse_down(displayed_point, MouseButton::Left, Modifiers::none());
    assert_eq!(&*received_positions.borrow(), &[displayed_point]);
}

#[open_gpui::test]
fn nonuniform_transform_exposes_explicit_click_and_wheel_coordinate_spaces(
    cx: &mut TestAppContext,
) {
    let mouse_down = Rc::new(RefCell::new(None));
    let click = Rc::new(RefCell::new(None));
    let wheel = Rc::new(RefCell::new(Vec::new()));
    let (_view, visual) = cx.add_window_view({
        let mouse_down = mouse_down.clone();
        let click = click.clone();
        let wheel = wheel.clone();
        move |_, _| NonuniformInteractionView {
            mouse_down,
            click,
            wheel,
        }
    });
    visual.update(|window, cx| window.draw(cx).clear());

    let displayed_point = point(px(140.0), px(100.0));
    visual.simulate_click(displayed_point, Modifiers::none());
    let expected = Some((
        displayed_point,
        point(px(10.0), px(5.0)),
        point(px(30.0), px(35.0)),
    ));
    assert_eq!(*mouse_down.borrow(), expected);
    assert_eq!(*click.borrow(), expected);

    visual.simulate_event(ScrollWheelEvent {
        position: displayed_point,
        delta: ScrollDelta::Pixels(point(px(20.0), px(40.0))),
        ..Default::default()
    });
    visual.simulate_event(ScrollWheelEvent {
        position: displayed_point,
        delta: ScrollDelta::Lines(point(2.0, 3.0)),
        ..Default::default()
    });

    let wheel = wheel.borrow();
    assert!(matches!(
        wheel[0],
        (
            ScrollDelta::Pixels(raw),
            ScrollDelta::Pixels(local)
        ) if raw == point(px(20.0), px(40.0)) && local == point(px(10.0), px(10.0))
    ));
    assert!(matches!(
        wheel[1],
        (ScrollDelta::Lines(raw), ScrollDelta::Lines(local))
            if raw == point(2.0, 3.0) && local == raw
    ));
}

#[open_gpui::test]
fn nonuniform_transform_keeps_drag_preview_and_target_geometry_distinct(cx: &mut TestAppContext) {
    let drag_start = Rc::new(RefCell::new(None));
    let drag_move = Rc::new(RefCell::new(None));
    let drop = Rc::new(RefCell::new(None));
    let (_view, visual) = cx.add_window_view({
        let drag_start = drag_start.clone();
        let drag_move = drag_move.clone();
        let drop = drop.clone();
        move |_, _| NonuniformDragView {
            drag_start,
            drag_move,
            drop,
        }
    });
    visual.update(|window, _| window.activate_window());
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear());

    let start = point(px(130.0), px(100.0));
    let activation = point(px(140.0), px(112.0));
    let moved = point(px(160.0), px(120.0));
    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(activation, Some(MouseButton::Left), Modifiers::none());
    visual.simulate_mouse_move(moved, Some(MouseButton::Left), Modifiers::none());
    visual.simulate_mouse_up(moved, MouseButton::Left, Modifiers::none());

    assert_eq!(
        *drag_start.borrow(),
        Some((
            7,
            activation,
            point(px(10.0), px(8.0)),
            point(px(30.0), px(38.0)),
            point(px(20.0), px(32.0)),
        ))
    );
    assert_eq!(
        *drag_move.borrow(),
        Some((
            7,
            moved,
            point(px(20.0), px(10.0)),
            point(px(40.0), px(40.0)),
        ))
    );
    assert_eq!(
        *drop.borrow(),
        Some((
            7,
            moved,
            point(px(20.0), px(10.0)),
            point(px(40.0), px(40.0)),
        ))
    );
}

#[open_gpui::test]
fn pointer_capture_uses_the_latest_committed_transform_geometry(cx: &mut TestAppContext) {
    let moves = Rc::new(RefCell::new(Vec::new()));
    let (root, cx) = cx.add_window_view({
        let moves = moves.clone();
        move |window, _| TransformPointerCaptureView {
            capture: window.new_pointer_capture_handle(),
            translated: false,
            moves,
        }
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    cx.simulate_mouse_down(
        point(px(130.0), px(90.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert!(cx.update(|window, _| window.captured_pointer().is_some()));

    cx.update_window_entity(&root, |root, _, cx| {
        root.translated = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let moved = point(px(380.0), px(90.0));
    cx.simulate_mouse_move(moved, Some(MouseButton::Left), Modifiers::none());
    assert_eq!(
        moves.borrow().last().copied(),
        Some((moved, point(px(80.0), px(5.0)), point(px(100.0), px(35.0)),)),
        "captured routing must preserve target identity while rebinding to current geometry"
    );

    cx.simulate_mouse_up(moved, MouseButton::Left, Modifiers::none());
}

#[open_gpui::test]
fn cached_child_is_rebuilt_when_only_its_ancestor_transform_changes(cx: &mut TestAppContext) {
    let renders = Rc::new(Cell::new(0));
    let clicks = Rc::new(Cell::new(0));
    let (root, cx) = cx.add_window_view({
        let renders = renders.clone();
        let clicks = clicks.clone();
        move |_, cx| CachedTransformRoot {
            child: cx.new(|_| CachedTransformChild { renders, clicks }),
            translated: false,
        }
    });

    cx.update(|window, cx| window.draw(cx).clear());
    let initial_renders = renders.get();
    let initial_debug_bounds = cx
        .debug_bounds("cached-transform-child")
        .expect("cached child debug bounds");
    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(renders.get(), initial_renders);

    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert_eq!(clicks.get(), 1);

    cx.update_window_entity(&root, |root, _, cx| {
        root.translated = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert!(renders.get() > initial_renders);
    assert_eq!(
        cx.debug_bounds("cached-transform-child")
            .expect("translated cached child debug bounds")
            .origin,
        initial_debug_bounds.origin + point(px(100.0), px(100.0))
    );

    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert_eq!(clicks.get(), 1);
    cx.simulate_mouse_down(
        point(px(110.0), px(110.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert_eq!(clicks.get(), 2);
}

#[open_gpui::test]
fn ordinary_deferred_inherits_transform_while_window_portal_resets_geometry(
    cx: &mut TestAppContext,
) {
    let inherited = Rc::new(RefCell::new(None));
    let portal = Rc::new(RefCell::new(None));
    let _window = cx.open_window(size(px(320.0), px(200.0)), {
        let inherited = inherited.clone();
        let portal = portal.clone();
        move |_, _| DeferredTransformView { inherited, portal }
    });

    let inherited = inherited.borrow();
    let inherited = inherited
        .as_ref()
        .expect("inherited deferred hitbox missing");
    assert_eq!(inherited.displayed_bounds().size, size(px(40.0), px(30.0)));
    assert_ne!(inherited.displayed_bounds(), inherited.layout_bounds());

    let portal = portal.borrow();
    let portal = portal.as_ref().expect("window portal hitbox missing");
    assert_eq!(portal.displayed_bounds().size, size(px(20.0), px(10.0)));
    assert_eq!(portal.displayed_bounds(), portal.layout_bounds());
}

#[open_gpui::test]
fn deferred_paint_failure_suppresses_every_observable_subtree_channel(cx: &mut TestAppContext) {
    let commits = Rc::new(Cell::new(0));
    let clicks = Rc::new(Cell::new(0));
    let window = cx.open_window(size(px(320.0), px(200.0)), {
        let commits = commits.clone();
        let clicks = clicks.clone();
        move |_, _| DeferredLateNumericFailureView { commits, clicks }
    });
    let any_window = window.into();

    assert_eq!(commits.get(), 0);
    window
        .update(cx, |_, window, _| {
            assert_eq!(window.rendered_frame.scene.len(), 0);
            assert!(
                window
                    .rendered_frame
                    .hit_test(point(px(10.0), px(10.0)))
                    .ids
                    .is_empty()
            );
            assert_eq!(window.rendered_frame.subtree_transform_diagnostics.len(), 1);
        })
        .unwrap();

    window
        .update(cx, |_, window, cx| {
            window.dispatch_event(
                crate::PlatformInput::MouseDown(crate::MouseDownEvent {
                    button: MouseButton::Left,
                    position: point(px(10.0), px(10.0)),
                    modifiers: Modifiers::none(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
        })
        .unwrap();
    assert_eq!(clicks.get(), 0);

    assert!(cx.activate_accessibility(any_window));
    let update = cx.latest_accessibility_tree_update(any_window).unwrap();
    assert!(
        update
            .nodes
            .iter()
            .all(|(_, node)| node.label() != Some("Deferred late transform failure"))
    );
    assert_eq!(commits.get(), 0);
}

#[open_gpui::test]
fn paint_time_numeric_failure_suppresses_every_observable_subtree_channel(cx: &mut TestAppContext) {
    let commits = Rc::new(Cell::new(0));
    let clicks = Rc::new(Cell::new(0));
    let window = cx.open_window(size(px(320.0), px(200.0)), {
        let commits = commits.clone();
        let clicks = clicks.clone();
        move |_, _| LateNumericFailureView { commits, clicks }
    });
    let any_window = window.into();

    assert_eq!(commits.get(), 0);
    window
        .update(cx, |_, window, _| {
            assert_eq!(window.rendered_frame.scene.len(), 0);
            assert!(
                window
                    .rendered_frame
                    .hit_test(point(px(10.0), px(10.0)))
                    .ids
                    .is_empty()
            );
            assert_eq!(window.rendered_frame.subtree_transform_diagnostics.len(), 1);
            assert!(
                window
                    .rendered_frame
                    .hitboxes
                    .iter()
                    .all(|hitbox| !hitbox.is_hovered(window))
            );
        })
        .unwrap();

    window
        .update(cx, |_, window, cx| {
            window.dispatch_event(
                crate::PlatformInput::MouseDown(crate::MouseDownEvent {
                    button: MouseButton::Left,
                    position: point(px(10.0), px(10.0)),
                    modifiers: Modifiers::none(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
        })
        .unwrap();
    assert_eq!(clicks.get(), 0);

    assert!(cx.activate_accessibility(any_window));
    let update = cx.latest_accessibility_tree_update(any_window).unwrap();
    assert!(
        update
            .nodes
            .iter()
            .all(|(_, node)| node.label() != Some("Late transform failure"))
    );
    assert_eq!(commits.get(), 0);
}

#[open_gpui::test]
fn paint_time_invalid_focus_claim_restores_the_committed_focus(cx: &mut TestAppContext) {
    let window = cx.open_window(size(px(320.0), px(200.0)), |_, cx| {
        LateInvalidFocusFallbackView {
            committed_focus: cx.focus_handle(),
            rejected_focus: cx.focus_handle(),
        }
    });
    let view = window.root(cx).unwrap();
    let any_window = window.into();
    cx.run_until_parked();

    let (committed_focus, rejected_focus) = cx.read(|cx| {
        let view = view.read(cx);
        (view.committed_focus.clone(), view.rejected_focus.clone())
    });
    cx.update_window(any_window, |_, window, cx| {
        window.activate_window();
        committed_focus.focus(window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let completion_subscription = cx
        .update_window(any_window, |_, window, cx| {
            let outcomes = outcomes.clone();
            assert!(!window.is_focus_handle_rendered(&rejected_focus));
            let subscription =
                window.focus_with_completion(&rejected_focus, cx, move |outcome, _, _| {
                    outcomes.borrow_mut().push(outcome);
                });
            assert_eq!(window.retained_focus_claim_count_for_test(), 1);
            assert_eq!(window.focused(cx).as_ref(), Some(&committed_focus));
            subscription
        })
        .unwrap();

    cx.run_until_parked();
    cx.update_window(any_window, |_, window, cx| {
        assert_eq!(window.focused(cx).as_ref(), Some(&committed_focus));
        assert!(!window.is_focus_handle_rendered(&rejected_focus));
        assert_eq!(window.retained_focus_claim_count_for_test(), 0);
    })
    .unwrap();
    assert_eq!(outcomes.borrow().as_slice(), &[FocusClaimOutcome::Rejected]);
    drop(completion_subscription);
}

#[open_gpui::test]
fn paint_time_invalid_drag_source_schedules_preview_removal(cx: &mut TestAppContext) {
    let fail_late = Rc::new(Cell::new(false));
    let preview_paints = Rc::new(Cell::new(0));
    let window = cx.open_window(size(px(320.0), px(200.0)), {
        let fail_late = fail_late.clone();
        let preview_paints = preview_paints.clone();
        move |_, _| LateInvalidDragSourceView {
            fail_late,
            preview_paints,
        }
    });
    let any_window = window.into();
    cx.update_window(any_window, |_, window, cx| {
        window.activate_window();
        window.draw(cx).clear();
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(any_window, |_, window, cx| {
        window.draw(cx).clear();
        window.dispatch_event(
            crate::PlatformInput::MouseDown(crate::MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(10.0), px(10.0)),
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            crate::PlatformInput::MouseMove(crate::MouseMoveEvent {
                position: point(px(30.0), px(10.0)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
            cx,
        );
        assert!(cx.has_active_drag());
        assert!(window.captured_pointer().is_some());
    })
    .unwrap();

    fail_late.set(true);
    cx.update_window(any_window, |_, window, cx| {
        assert!(cx.has_active_drag());
        window.refresh();
        window.draw(cx).clear();
        assert!(!cx.has_active_drag());
        assert!(window.captured_pointer().is_none());
        let frame_facts = (
            window.invalidator.is_dirty(),
            window.rendered_frame.scene.len(),
            preview_paints.get(),
        );
        assert!(
            frame_facts.2 > 0,
            "the invalid source frame must paint the preview before late cleanup: {frame_facts:?}"
        );
        assert!(
            frame_facts.0,
            "late drag cleanup must schedule a preview-removal frame: {frame_facts:?}"
        );
        assert!(frame_facts.1 > 0);
    })
    .unwrap();
    let late_frame_preview_paints = preview_paints.get();

    cx.run_until_parked();
    cx.update_window(any_window, |_, window, _| {
        assert_eq!(window.rendered_frame.scene.len(), 0);
    })
    .unwrap();
    assert_eq!(
        preview_paints.get(),
        late_frame_preview_paints,
        "the canceled preview must not paint in the cleanup frame"
    );
}

#[open_gpui::test]
fn prepaint_window_transaction_retracts_invalid_rolled_back_and_absent_publications(
    cx: &mut TestAppContext,
) {
    let commits = Rc::new(Cell::new(0));
    let discards = Rc::new(Cell::new(0));
    let window = cx.open_window(size(px(320.0), px(200.0)), {
        let commits = commits.clone();
        let discards = discards.clone();
        move |_, _| PrepaintWindowTransactionView {
            publication: PrepaintPublicationId::new(),
            publish: true,
            rollback: false,
            fail_late: true,
            commits,
            discards,
        }
    });

    assert_eq!(commits.get(), 0);
    assert_eq!(discards.get(), 1);

    window
        .update(cx, |view, _, cx| {
            view.fail_late = false;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();

    assert_eq!(commits.get(), 1);
    assert_eq!(discards.get(), 1);

    window
        .update(cx, |view, _, cx| {
            view.rollback = true;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();

    assert_eq!(commits.get(), 1);
    assert_eq!(
        discards.get(),
        2,
        "rolling back the next frame must retract the previous publication"
    );

    window
        .update(cx, |view, _, cx| {
            view.rollback = false;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(commits.get(), 2);

    window
        .update(cx, |view, _, cx| {
            view.publish = false;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        discards.get(),
        3,
        "removing the publication subtree must retract its previous frame"
    );

    window.update(cx, |_, _, cx| cx.notify()).unwrap();
    cx.run_until_parked();
    assert_eq!(
        discards.get(),
        3,
        "an absent publication must be retracted only once"
    );
}

#[open_gpui::test]
fn tooltip_hover_rejects_geometry_after_its_transform_scope_fails(cx: &mut TestAppContext) {
    let mut cx = cx.add_empty_window();
    let mouse_position = point(px(20.0), px(20.0));
    cx.simulate_mouse_move(mouse_position, None, Modifiers::none());
    let (tooltip, validity) = cx.update(|window, _| {
        window.install_tooltip_bounds_with_validity_for_test(Bounds::centered_at(
            mouse_position,
            size(px(20.0), px(20.0)),
        ))
    });

    assert!(cx.update(|window, _| tooltip.is_hovered(window)));
    validity.invalidate(SubtreeTransformError::UnrepresentableResult);
    assert!(!cx.update(|window, _| tooltip.is_hovered(window)));
}

#[open_gpui::test]
fn failed_prepaint_transaction_discards_cached_ranges_before_the_next_frame(
    cx: &mut TestAppContext,
) {
    let renders = Rc::new(Cell::new(0));
    let (_root, cx) = cx.add_window_view({
        let renders = renders.clone();
        move |_, cx| InvalidatedCachedRoot {
            child: cx.new(|_| InvalidatedCachedChild { renders }),
        }
    });

    cx.update(|window, cx| window.draw(cx).clear());
    let first_frame_renders = renders.get();
    assert!(first_frame_renders > 0);

    cx.update(|window, cx| window.draw(cx).clear());
    assert!(
        renders.get() > first_frame_renders,
        "a failed frame must not publish a cached journal range"
    );
    cx.update(|window, _| {
        assert_eq!(window.rendered_frame.scene.len(), 0);
        assert_eq!(window.rendered_frame.subtree_transform_diagnostics.len(), 1);
    });
}

#[open_gpui::test]
fn cached_ancestor_never_revives_a_failed_transformed_descendant(cx: &mut TestAppContext) {
    let renders = Rc::new(Cell::new(0));
    let (_root, cx) = cx.add_window_view({
        let renders = renders.clone();
        move |_, cx| CachedInvalidDescendantRoot {
            child: cx.new(|_| CachedAncestorWithInvalidDescendant { renders }),
        }
    });

    let first_frame_renders = renders.get();
    assert!(first_frame_renders > 0);
    cx.update(|window, _| {
        assert_eq!(window.rendered_frame.scene.len(), 0);
        assert_eq!(window.rendered_frame.subtree_transform_diagnostics.len(), 1);
    });

    cx.update(|window, cx| window.draw(cx).clear());
    assert_eq!(
        renders.get(),
        first_frame_renders,
        "the outer cached view should replay its journal instead of rerendering"
    );
    cx.update(|window, _| {
        assert_eq!(window.rendered_frame.scene.len(), 0);
        assert!(
            window
                .rendered_frame
                .hitboxes
                .iter()
                .all(|hitbox| !hitbox.is_active())
        );
        assert_eq!(window.rendered_frame.subtree_transform_diagnostics.len(), 1);
    });
}

#[open_gpui::test]
fn transform_only_frame_updates_ime_from_the_new_committed_handler(cx: &mut TestAppContext) {
    let (root, mut cx) = cx.add_window_view(|_, cx| {
        let focus = cx.focus_handle();
        let child_focus = focus.clone();
        TransformImeRoot {
            child: cx.new(move |_| CachedTransformImeChild { focus: child_focus }),
            focus,
            translated: false,
        }
    });

    cx.update(|window, _| window.activate_window());
    cx.update_window_entity(&root, |root, window, cx| {
        root.focus.focus(window, cx);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, _| {
        window
            .platform_window
            .as_test()
            .expect("the test uses TestWindow")
            .clear_ime_position_history();
    });

    cx.update_window_entity(&root, |root, _, cx| {
        root.translated = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let history = cx.update(|window, _| {
        window
            .platform_window
            .as_test()
            .expect("the test uses TestWindow")
            .ime_position_history()
    });
    let expected = Bounds::new(point(px(110.0), px(58.0)), size(px(2.0), px(20.0)));
    assert!(!history.is_empty());
    assert!(
        history.iter().all(|bounds| *bounds == expected),
        "every platform update after the transform change must use the newly committed geometry"
    );
}
