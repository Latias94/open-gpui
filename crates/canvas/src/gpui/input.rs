use crate::{CanvasEvent, CanvasKey, CanvasKeyModifiers, PointerButton};
use open_gpui::{
    Bounds, Context, DispatchPhase, ElementGeometry, Entity, FocusHandle, Hitbox, KeyDownEvent,
    Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    ScrollDelta, ScrollWheelEvent, Window, px,
};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
enum CanvasInputGeometry {
    WindowBounds(Bounds<Pixels>),
    Element(ElementGeometry),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasInputMapper {
    geometry: CanvasInputGeometry,
    line_height: Pixels,
}

impl CanvasInputMapper {
    pub fn new(bounds: Bounds<Pixels>) -> Self {
        Self {
            geometry: CanvasInputGeometry::WindowBounds(bounds),
            line_height: px(16.0),
        }
    }

    /// Creates a mapper from GPUI's resolved geometry for an element in a transformed subtree.
    pub fn from_element_geometry(geometry: ElementGeometry) -> Self {
        Self {
            geometry: CanvasInputGeometry::Element(geometry),
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

        let delta = match event.delta {
            ScrollDelta::Lines(lines) => ScrollDelta::Lines(lines).pixel_delta(self.line_height),
            ScrollDelta::Pixels(delta) => match self.geometry {
                CanvasInputGeometry::WindowBounds(_) => delta,
                CanvasInputGeometry::Element(geometry) => {
                    geometry.window_to_local_vector(delta).ok()?
                }
            },
        };
        Some(CanvasEvent::Wheel { delta })
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
        let displayed_bounds = match self.geometry {
            CanvasInputGeometry::WindowBounds(bounds) => bounds,
            CanvasInputGeometry::Element(geometry) => geometry.displayed_bounds(),
        };
        if !displayed_bounds.contains(&position) {
            return None;
        }
        self.unbounded_local_position(position)
    }

    fn unbounded_local_position(&self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        match self.geometry {
            CanvasInputGeometry::WindowBounds(bounds) => Some(position - bounds.origin),
            CanvasInputGeometry::Element(geometry) => geometry.window_to_local_point(position).ok(),
        }
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

    pub fn from_element_geometry(geometry: ElementGeometry) -> Self {
        Self {
            mapper: CanvasInputMapper::from_element_geometry(geometry),
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
                position: self.mapper.unbounded_local_position(event.position)?,
                modifiers: CanvasInputMapper::modifiers(event.modifiers),
            });
        }

        self.mapper.mouse_move(event)
    }

    pub fn mouse_up(&self, event: &MouseUpEvent) -> Option<CanvasEvent> {
        if self.pointer_interacting {
            let position = self.mapper.unbounded_local_position(event.position)?;
            return pointer_button(event.button).map(|button| CanvasEvent::PointerUp {
                position,
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
        self.dispatch_event(target, CanvasInputMapper::key_down_event(event), cx);
    }
}

pub fn register_canvas_editor_input<T>(
    entity: Entity<T>,
    focus_handle: FocusHandle,
    hitbox: Hitbox,
    handler: CanvasEditorInputHandler<T>,
    window: &mut Window,
) where
    T: 'static,
{
    let mapper = CanvasEditorInputMapper::from_element_geometry(hitbox.geometry());

    window.on_mouse_event({
        let entity = entity.clone();
        let handler = handler.clone();
        let hitbox = hitbox.clone();
        move |event: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || !hitbox.is_mouse_event_target(window) {
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
        let hitbox = hitbox.clone();
        move |event: &MouseMoveEvent, phase, window, cx| {
            entity.update(cx, |target, cx| {
                let pointer_interacting = handler.pointer_interacting(target);
                if pointer_interacting {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let mapper = mapper.with_pointer_interacting(true);
                    if let Some(event) = mapper.mouse_move(event) {
                        handler.dispatch_event(target, event, cx);
                        cx.stop_propagation();
                    }
                    return;
                }

                if phase == DispatchPhase::Bubble
                    && hitbox.is_mouse_event_target(window)
                    && let Some(event) = mapper.mouse_move(event)
                {
                    handler.dispatch_event(target, event, cx);
                }
            });
        }
    });

    window.on_mouse_event({
        let entity = entity.clone();
        let handler = handler.clone();
        let hitbox = hitbox.clone();
        move |event: &MouseUpEvent, phase, window, cx| {
            entity.update(cx, |target, cx| {
                let pointer_interacting = handler.pointer_interacting(target);
                if pointer_interacting {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let mapper = mapper.with_pointer_interacting(true);
                    if let Some(event) = mapper.mouse_up(event) {
                        handler.dispatch_event(target, event, cx);
                        cx.stop_propagation();
                    }
                    return;
                }

                if phase == DispatchPhase::Bubble
                    && hitbox.is_mouse_event_target(window)
                    && let Some(event) = mapper.mouse_up(event)
                {
                    handler.dispatch_event(target, event, cx);
                }
            });
        }
    });

    window.on_mouse_event({
        let entity = entity.clone();
        let handler = handler.clone();
        move |event: &ScrollWheelEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || !hitbox.should_handle_scroll(window) {
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
