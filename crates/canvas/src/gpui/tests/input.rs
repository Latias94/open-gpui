use super::*;

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
