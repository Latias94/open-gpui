use crate::{CanvasEvent, CanvasKey, CanvasKeyModifiers, PointerButton};
use open_gpui::{
    Bounds, Context, DispatchPhase, Entity, FocusHandle, KeyDownEvent, Keystroke, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollWheelEvent,
    Window, px,
};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasInputMapper {
    bounds: Bounds<Pixels>,
    line_height: Pixels,
}

impl CanvasInputMapper {
    pub fn new(bounds: Bounds<Pixels>) -> Self {
        Self {
            bounds,
            line_height: px(16.0),
        }
    }

    pub fn with_line_height(mut self, line_height: Pixels) -> Self {
        self.line_height = line_height;
        self
    }

    pub fn mouse_down(&self, event: &MouseDownEvent) -> Option<CanvasEvent> {
        Some(CanvasEvent::PointerDown {
            position: self.local_position(event.position)?,
            button: pointer_button(event.button)?,
            modifiers: Self::modifiers(event.modifiers),
        })
    }

    pub fn mouse_move(&self, event: &MouseMoveEvent) -> Option<CanvasEvent> {
        Some(CanvasEvent::PointerMove {
            position: self.local_position(event.position)?,
            modifiers: Self::modifiers(event.modifiers),
        })
    }

    pub fn mouse_up(&self, event: &MouseUpEvent) -> Option<CanvasEvent> {
        Some(CanvasEvent::PointerUp {
            position: self.local_position(event.position)?,
            button: pointer_button(event.button)?,
            modifiers: Self::modifiers(event.modifiers),
        })
    }

    pub fn scroll_wheel(&self, event: &ScrollWheelEvent) -> Option<CanvasEvent> {
        if self.local_position(event.position).is_none() {
            return None;
        }

        Some(CanvasEvent::Wheel {
            delta: event.delta.pixel_delta(self.line_height),
        })
    }

    pub fn key_down(&self, event: &KeyDownEvent) -> CanvasEvent {
        Self::key_down_event(event)
    }

    pub fn key_down_event(event: &KeyDownEvent) -> CanvasEvent {
        let key = canvas_key(&event.keystroke);
        if key == CanvasKey::Escape {
            return CanvasEvent::Cancel;
        }

        CanvasEvent::KeyDown {
            key,
            modifiers: Self::modifiers(event.keystroke.modifiers),
            repeat: event.is_held,
        }
    }

    pub fn modifiers(modifiers: Modifiers) -> CanvasKeyModifiers {
        canvas_key_modifiers(modifiers)
    }

    pub fn local_position(&self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        self.bounds
            .contains(&position)
            .then(|| position - self.bounds.origin)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasEditorInputMapper {
    mapper: CanvasInputMapper,
    pointer_interacting: bool,
}

impl CanvasEditorInputMapper {
    pub fn new(bounds: Bounds<Pixels>) -> Self {
        Self {
            mapper: CanvasInputMapper::new(bounds),
            pointer_interacting: false,
        }
    }

    pub fn with_line_height(mut self, line_height: Pixels) -> Self {
        self.mapper = self.mapper.with_line_height(line_height);
        self
    }

    pub fn with_pointer_interacting(mut self, pointer_interacting: bool) -> Self {
        self.pointer_interacting = pointer_interacting;
        self
    }

    pub fn mouse_down(&self, event: &MouseDownEvent) -> Option<CanvasEvent> {
        self.mapper.mouse_down(event)
    }

    pub fn mouse_move(&self, event: &MouseMoveEvent) -> Option<CanvasEvent> {
        if self.pointer_interacting {
            return Some(CanvasEvent::PointerMove {
                position: event.position - self.mapper.bounds.origin,
                modifiers: CanvasInputMapper::modifiers(event.modifiers),
            });
        }

        self.mapper.mouse_move(event)
    }

    pub fn mouse_up(&self, event: &MouseUpEvent) -> Option<CanvasEvent> {
        if self.pointer_interacting {
            return pointer_button(event.button).map(|button| CanvasEvent::PointerUp {
                position: event.position - self.mapper.bounds.origin,
                button,
                modifiers: CanvasInputMapper::modifiers(event.modifiers),
            });
        }

        self.mapper.mouse_up(event)
    }

    pub fn scroll_wheel(&self, event: &ScrollWheelEvent) -> Option<CanvasEvent> {
        self.mapper.scroll_wheel(event)
    }
}

pub struct CanvasEditorInputHandler<T> {
    pointer_interacting: Rc<dyn Fn(&T) -> bool>,
    dispatch: Rc<dyn Fn(&mut T, CanvasEvent, &mut Context<T>)>,
}

impl<T> Clone for CanvasEditorInputHandler<T> {
    fn clone(&self) -> Self {
        Self {
            pointer_interacting: self.pointer_interacting.clone(),
            dispatch: self.dispatch.clone(),
        }
    }
}

impl<T> CanvasEditorInputHandler<T> {
    pub fn new(
        pointer_interacting: impl Fn(&T) -> bool + 'static,
        dispatch: impl Fn(&mut T, CanvasEvent, &mut Context<T>) + 'static,
    ) -> Self {
        Self {
            pointer_interacting: Rc::new(pointer_interacting),
            dispatch: Rc::new(dispatch),
        }
    }

    pub fn pointer_interacting(&self, target: &T) -> bool {
        (self.pointer_interacting)(target)
    }

    pub fn dispatch_event(&self, target: &mut T, event: CanvasEvent, cx: &mut Context<T>) {
        (self.dispatch)(target, event, cx)
    }

    pub fn dispatch_key_down(&self, target: &mut T, event: &KeyDownEvent, cx: &mut Context<T>) {
        self.dispatch_event(target, canvas_editor_key_down_event(event), cx);
    }
}

pub fn canvas_editor_key_down_event(event: &KeyDownEvent) -> CanvasEvent {
    CanvasInputMapper::key_down_event(event)
}

pub fn register_canvas_editor_input<T>(
    entity: Entity<T>,
    focus_handle: FocusHandle,
    bounds: Bounds<Pixels>,
    handler: CanvasEditorInputHandler<T>,
    window: &mut Window,
) where
    T: 'static,
{
    let mapper = CanvasEditorInputMapper::new(bounds);

    window.on_mouse_event({
        let entity = entity.clone();
        let handler = handler.clone();
        move |event: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }

            let Some(event) = mapper.mouse_down(event) else {
                return;
            };

            window.focus(&focus_handle, cx);
            entity.update(cx, |target, cx| handler.dispatch_event(target, event, cx));
        }
    });

    window.on_mouse_event({
        let entity = entity.clone();
        let handler = handler.clone();
        move |event: &MouseMoveEvent, phase, _, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }

            entity.update(cx, |target, cx| {
                let mapper = mapper.with_pointer_interacting(handler.pointer_interacting(target));
                if let Some(event) = mapper.mouse_move(event) {
                    handler.dispatch_event(target, event, cx);
                }
            });
        }
    });

    window.on_mouse_event({
        let entity = entity.clone();
        let handler = handler.clone();
        move |event: &MouseUpEvent, phase, _, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }

            entity.update(cx, |target, cx| {
                let mapper = mapper.with_pointer_interacting(handler.pointer_interacting(target));
                if let Some(event) = mapper.mouse_up(event) {
                    handler.dispatch_event(target, event, cx);
                }
            });
        }
    });

    window.on_mouse_event({
        let entity = entity.clone();
        let handler = handler.clone();
        move |event: &ScrollWheelEvent, phase, _, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }

            if let Some(event) = mapper.scroll_wheel(event) {
                entity.update(cx, |target, cx| handler.dispatch_event(target, event, cx));
            }
        }
    });
}

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Navigate(_) => None,
    }
}

fn canvas_key(keystroke: &Keystroke) -> CanvasKey {
    match keystroke.key.as_str() {
        "delete" | "del" => CanvasKey::Delete,
        "backspace" => CanvasKey::Backspace,
        "escape" | "esc" => CanvasKey::Escape,
        "enter" | "return" => CanvasKey::Enter,
        key if key.chars().count() == 1 => CanvasKey::Character(
            keystroke
                .key_char
                .clone()
                .unwrap_or_else(|| key.to_string()),
        ),
        key => CanvasKey::Named(key.to_string()),
    }
}

fn canvas_key_modifiers(modifiers: Modifiers) -> CanvasKeyModifiers {
    CanvasKeyModifiers {
        shift: modifiers.shift,
        alt: modifiers.alt,
        control: modifiers.control,
        platform: modifiers.platform,
        function: modifiers.function,
    }
}
