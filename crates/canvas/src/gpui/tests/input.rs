use super::*;

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use open_gpui::{
    Context, FocusHandle, Hitbox, HitboxBehavior, IntoElement, Point, Render, Styled,
    SubtreeTransform, SubtreeTransformExt, SubtreeTransformOrigin, TestAppContext, VisualContext,
    Window, canvas,
};

struct TransformedCanvasInputView {
    focus: FocusHandle,
    events: Rc<RefCell<Vec<CanvasEvent>>>,
}

struct LabeledCanvasClipView {
    model: CanvasPaintModel,
    translated: bool,
    prepaint_count: Rc<Cell<usize>>,
    paint_count: Rc<Cell<usize>>,
}

impl Render for LabeledCanvasClipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let prepaint_model = self.model.clone();
        let paint_model = self.model.clone();
        let prepaint_count = self.prepaint_count.clone();
        let paint_count = self.paint_count.clone();
        let translation = if self.translated {
            point(px(20.0), px(12.0))
        } else {
            Point::default()
        };

        canvas(
            move |bounds, window, _| {
                prepaint_count.set(prepaint_count.get() + 1);
                let frame = prepaint_canvas_frame(
                    &prepaint_model,
                    bounds,
                    CanvasPaintOptions::default(),
                    CanvasPaintTheme::default(),
                    window,
                );
                assert_eq!(frame.prepared_label_count(), 1);
                frame
            },
            move |bounds, frame, window, cx| {
                paint_count.set(paint_count.get() + 1);
                paint_canvas_frame(
                    bounds,
                    &paint_model,
                    &frame,
                    CanvasPaintTheme::default(),
                    window,
                    cx,
                );
            },
        )
        .w(px(140.0))
        .h(px(100.0))
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

impl Render for TransformedCanvasInputView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let focus = self.focus.clone();
        let events = self.events.clone();
        let handler = CanvasEditorInputHandler::new(
            |_| false,
            move |_, event, _| events.borrow_mut().push(event),
        );
        let input_hitbox = Rc::new(RefCell::new(None::<Hitbox>));
        let prepaint_input_hitbox = input_hitbox.clone();

        canvas(
            move |bounds, window, _| {
                *prepaint_input_hitbox.borrow_mut() =
                    Some(window.insert_hitbox(bounds, HitboxBehavior::Normal));
            },
            move |_, _, window, _| {
                let hitbox = input_hitbox
                    .borrow_mut()
                    .take()
                    .expect("test canvas input hitbox");
                register_canvas_editor_input(entity, focus, hitbox, handler, window);
            },
        )
        .w(px(100.0))
        .h(px(50.0))
        .with_subtree_transform(
            SubtreeTransform::try_new(
                size(2.0, 4.0),
                point(px(100.0), px(50.0)),
                SubtreeTransformOrigin::TOP_LEFT,
            )
            .unwrap(),
        )
    }
}

#[open_gpui::test]
fn editor_adapter_inverse_projects_transformed_pointer_and_pixel_wheel_input(
    cx: &mut TestAppContext,
) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let (_view, cx) = cx.add_window_view({
        let events = events.clone();
        move |_, cx| TransformedCanvasInputView {
            focus: cx.focus_handle(),
            events,
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let displayed_position = point(px(140.0), px(90.0));
    cx.simulate_mouse_down(displayed_position, MouseButton::Left, Modifiers::none());
    cx.simulate_event(ScrollWheelEvent {
        position: displayed_position,
        delta: ScrollDelta::Pixels(point(px(20.0), px(40.0))),
        ..ScrollWheelEvent::default()
    });

    assert_eq!(
        events.borrow().as_slice(),
        [
            CanvasEvent::PointerDown {
                position: point(px(20.0), px(10.0)),
                button: PointerButton::Primary,
                modifiers: CanvasKeyModifiers::NONE,
            },
            CanvasEvent::Wheel {
                delta: point(px(10.0), px(10.0)),
            },
        ]
    );
}

#[open_gpui::test]
fn labeled_canvas_reprepares_clip_tokens_for_each_paint_generation(cx: &mut TestAppContext) {
    let mut node = CanvasNode::new(
        "painted",
        point(px(10.0), px(10.0)),
        size(px(110.0), px(70.0)),
    );
    node.kind = "painted-node".to_string();
    let model = CanvasPaintModel::new_with_kind_registry(
        document_fixture().node(node).build(),
        CanvasViewport::default(),
        paint_registry(),
    );
    let prepaint_count = Rc::new(Cell::new(0));
    let paint_count = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view({
        let prepaint_count = prepaint_count.clone();
        let paint_count = paint_count.clone();
        move |_, _| LabeledCanvasClipView {
            model,
            translated: false,
            prepaint_count,
            paint_count,
        }
    });

    cx.update(|window, cx| window.draw(cx).clear());
    let first_revision = cx.update(|window, _| window.rendered_frame_revision());
    let first_prepaint_count = prepaint_count.get();
    let first_paint_count = paint_count.get();
    assert!(first_prepaint_count > 0);
    assert_eq!(first_prepaint_count, first_paint_count);

    cx.update_window_entity(&view, |view, _, cx| {
        view.translated = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    let second_revision = cx.update(|window, _| window.rendered_frame_revision());

    assert!(second_revision > first_revision);
    assert!(prepaint_count.get() > first_prepaint_count);
    assert!(paint_count.get() > first_paint_count);
    assert_eq!(prepaint_count.get(), paint_count.get());
}

#[test]
fn input_mapper_localizes_pointer_events() {
    let mapper = CanvasInputMapper::new(Bounds::new(
        point(px(100.0), px(50.0)),
        size(px(200.0), px(120.0)),
    ));

    assert_eq!(
        mapper.mouse_down(&MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(120.0), px(80.0)),
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
            ..MouseDownEvent::default()
        }),
        Some(CanvasEvent::PointerDown {
            position: point(px(20.0), px(30.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers {
                shift: true,
                ..CanvasKeyModifiers::default()
            },
        })
    );
    assert_eq!(
        mapper.mouse_up(&MouseUpEvent {
            button: MouseButton::Right,
            position: point(px(140.0), px(90.0)),
            ..MouseUpEvent::default()
        }),
        Some(CanvasEvent::PointerUp {
            position: point(px(40.0), px(40.0)),
            button: PointerButton::Secondary,
            modifiers: CanvasKeyModifiers::default(),
        })
    );
    assert_eq!(
        mapper.mouse_move(&MouseMoveEvent {
            position: point(px(150.0), px(95.0)),
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
            ..MouseMoveEvent::default()
        }),
        Some(CanvasEvent::PointerMove {
            position: point(px(50.0), px(45.0)),
            modifiers: CanvasKeyModifiers {
                shift: true,
                ..CanvasKeyModifiers::default()
            },
        })
    );
}

#[test]
fn input_mapper_filters_outside_or_unsupported_pointer_events() {
    let mapper = CanvasInputMapper::new(Bounds::new(
        point(px(100.0), px(50.0)),
        size(px(200.0), px(120.0)),
    ));

    assert_eq!(
        mapper.mouse_down(&MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(20.0), px(80.0)),
            ..MouseDownEvent::default()
        }),
        None
    );
    assert_eq!(
        mapper.mouse_down(&MouseDownEvent {
            button: MouseButton::Navigate(open_gpui::NavigationDirection::Back),
            position: point(px(120.0), px(80.0)),
            ..MouseDownEvent::default()
        }),
        None
    );
}

#[test]
fn editor_input_mapper_keeps_drag_events_after_pointer_leaves_bounds() {
    let mapper = CanvasEditorInputMapper::new(Bounds::new(
        point(px(100.0), px(50.0)),
        size(px(200.0), px(120.0)),
    ))
    .with_pointer_interacting(true);

    assert_eq!(
        mapper.mouse_move(&MouseMoveEvent {
            position: point(px(20.0), px(80.0)),
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
            ..MouseMoveEvent::default()
        }),
        Some(CanvasEvent::PointerMove {
            position: point(px(-80.0), px(30.0)),
            modifiers: CanvasKeyModifiers {
                shift: true,
                ..CanvasKeyModifiers::default()
            },
        })
    );
    assert_eq!(
        mapper.mouse_up(&MouseUpEvent {
            button: MouseButton::Left,
            position: point(px(20.0), px(80.0)),
            ..MouseUpEvent::default()
        }),
        Some(CanvasEvent::PointerUp {
            position: point(px(-80.0), px(30.0)),
            button: PointerButton::Primary,
            modifiers: CanvasKeyModifiers::default(),
        })
    );
}

#[test]
fn editor_input_mapper_filters_outside_events_when_not_dragging() {
    let mapper = CanvasEditorInputMapper::new(Bounds::new(
        point(px(100.0), px(50.0)),
        size(px(200.0), px(120.0)),
    ));

    assert_eq!(
        mapper.mouse_move(&MouseMoveEvent {
            position: point(px(20.0), px(80.0)),
            ..MouseMoveEvent::default()
        }),
        None
    );
    assert_eq!(
        mapper.mouse_up(&MouseUpEvent {
            button: MouseButton::Left,
            position: point(px(20.0), px(80.0)),
            ..MouseUpEvent::default()
        }),
        None
    );
}

#[test]
fn input_mapper_converts_scroll_delta_to_canvas_wheel() {
    let mapper = CanvasInputMapper::new(Bounds::new(
        point(px(100.0), px(50.0)),
        size(px(200.0), px(120.0)),
    ))
    .with_line_height(px(20.0));

    assert_eq!(
        mapper.scroll_wheel(&ScrollWheelEvent {
            position: point(px(120.0), px(80.0)),
            delta: ScrollDelta::Lines(point(1.0, -2.0)),
            ..ScrollWheelEvent::default()
        }),
        Some(CanvasEvent::Wheel {
            delta: point(px(20.0), px(-40.0)),
        })
    );
    assert_eq!(
        mapper.scroll_wheel(&ScrollWheelEvent {
            position: point(px(20.0), px(80.0)),
            delta: ScrollDelta::Pixels(point(px(1.0), px(2.0))),
            ..ScrollWheelEvent::default()
        }),
        None
    );
}

#[test]
fn input_mapper_converts_key_down_events() {
    assert_eq!(
        CanvasInputMapper::key_down_event(&KeyDownEvent {
            keystroke: Keystroke::parse("backspace").unwrap(),
            is_held: false,
            prefer_character_input: false,
        }),
        CanvasEvent::KeyDown {
            key: CanvasKey::Backspace,
            modifiers: CanvasKeyModifiers::default(),
            repeat: false,
        }
    );
    assert_eq!(
        CanvasInputMapper::key_down_event(&KeyDownEvent {
            keystroke: Keystroke::parse("ctrl-a").unwrap(),
            is_held: true,
            prefer_character_input: false,
        }),
        CanvasEvent::KeyDown {
            key: CanvasKey::Character("a".to_string()),
            modifiers: CanvasKeyModifiers {
                control: true,
                ..CanvasKeyModifiers::default()
            },
            repeat: true,
        }
    );
    assert_eq!(
        CanvasInputMapper::key_down_event(&KeyDownEvent {
            keystroke: Keystroke::parse("escape").unwrap(),
            is_held: false,
            prefer_character_input: false,
        }),
        CanvasEvent::Cancel
    );
}
