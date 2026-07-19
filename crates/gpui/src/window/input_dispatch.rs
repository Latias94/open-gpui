use std::sync::Arc;

use crate::{
    AnyDrag, App, AppContext, FileDropEvent, Modifiers, MouseButton, MouseMoveEvent, MouseUpEvent,
    PlatformInput,
};

use super::{InputModality, Window};

pub(super) fn prepare_platform_input(
    window: &mut Window,
    cx: &mut App,
    event: PlatformInput,
) -> PlatformInput {
    update_input_modality(window, &event);

    cx.propagate_event = true;
    window.default_prevented = false;

    normalize_platform_input(window, cx, event)
}

fn update_input_modality(window: &mut Window, event: &PlatformInput) {
    let old_modality = window.last_input_modality;
    window.last_input_modality = match event {
        PlatformInput::KeyDown(_) => InputModality::Keyboard,
        PlatformInput::MouseMove(_) | PlatformInput::MouseDown(_) => InputModality::Mouse,
        _ => window.last_input_modality,
    };
    if window.last_input_modality != old_modality {
        window.refresh();
    }
}

fn normalize_platform_input(
    window: &mut Window,
    cx: &mut App,
    event: PlatformInput,
) -> PlatformInput {
    match event {
        PlatformInput::MouseMove(mouse_move) => {
            window.mouse_position = mouse_move.position;
            window.mouse_in_window = true;
            window.modifiers = mouse_move.modifiers;
            PlatformInput::MouseMove(mouse_move)
        }
        PlatformInput::MouseDown(mouse_down) => {
            window.mouse_position = mouse_down.position;
            window.mouse_in_window = true;
            window.modifiers = mouse_down.modifiers;
            PlatformInput::MouseDown(mouse_down)
        }
        PlatformInput::MouseUp(mouse_up) => {
            window.mouse_position = mouse_up.position;
            window.mouse_in_window = true;
            window.modifiers = mouse_up.modifiers;
            PlatformInput::MouseUp(mouse_up)
        }
        PlatformInput::MousePressure(mouse_pressure) => {
            PlatformInput::MousePressure(mouse_pressure)
        }
        PlatformInput::MouseExited(mouse_exited) => {
            window.mouse_position = mouse_exited.position;
            window.mouse_in_window = false;
            window.modifiers = mouse_exited.modifiers;
            PlatformInput::MouseExited(mouse_exited)
        }
        PlatformInput::PointerCanceled(pointer_canceled) => {
            PlatformInput::PointerCanceled(pointer_canceled)
        }
        PlatformInput::ModifiersChanged(modifiers_changed) => {
            window.modifiers = modifiers_changed.modifiers;
            window.capslock = modifiers_changed.capslock;
            PlatformInput::ModifiersChanged(modifiers_changed)
        }
        PlatformInput::ScrollWheel(scroll_wheel) => {
            window.mouse_position = scroll_wheel.position;
            window.mouse_in_window = true;
            window.modifiers = scroll_wheel.modifiers;
            PlatformInput::ScrollWheel(scroll_wheel)
        }
        PlatformInput::Pinch(pinch) => {
            window.mouse_position = pinch.position;
            window.mouse_in_window = true;
            window.modifiers = pinch.modifiers;
            PlatformInput::Pinch(pinch)
        }
        PlatformInput::FileDrop(file_drop) => normalize_file_drop(window, cx, file_drop),
        PlatformInput::KeyDown(_) | PlatformInput::KeyUp(_) => event,
    }
}

fn normalize_file_drop(window: &mut Window, cx: &mut App, event: FileDropEvent) -> PlatformInput {
    match event {
        FileDropEvent::Entered { position, paths } => {
            window.mouse_position = position;
            window.mouse_in_window = true;
            if cx.active_drag.is_none() {
                cx.active_drag = Some(AnyDrag {
                    window_id: window.window_handle().window_id(),
                    value: Arc::new(paths.clone()),
                    view: cx.new(|_| paths).into(),
                    window_preview_offset: position,
                    cursor_style: None,
                    button: MouseButton::Left,
                });
            }
            PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::default(),
            })
        }
        FileDropEvent::Pending { position } => {
            window.mouse_position = position;
            window.mouse_in_window = true;
            PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::default(),
            })
        }
        FileDropEvent::Submit { position } => {
            cx.activate(true);
            window.mouse_position = position;
            window.mouse_in_window = true;
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
            })
        }
        FileDropEvent::Exited => {
            window.mouse_in_window = false;
            cx.clear_active_drag_for_window(window.window_handle().window_id());
            PlatformInput::FileDrop(FileDropEvent::Exited)
        }
    }
}
