use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use open_gpui::{
    Capslock, DispatchEventResult, ExternalPaths, FileDropEvent, KeyDownEvent, KeyUpEvent,
    Keystroke, Modifiers, ModifiersChangedEvent, MouseDownEvent, MouseExitEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, PlatformInput, Point, PointerCancelEvent, PointerCancelReason,
    ScrollDelta, ScrollWheelEvent, TouchPhase, point, px,
};
use smallvec::smallvec;
use wasm_bindgen::prelude::*;

use crate::{
    pointer_session::{
        ClickState, WebPointerButtonChange, WebPointerCaptureCommand, WebPointerCaptureState,
        WebPointerTransition, dom_buttons_to_pressed_button, dom_mouse_button_to_gpui,
    },
    window::{WebWindowCallbacks, WebWindowInner},
};

pub struct WebEventListeners {
    #[allow(dead_code)]
    closures: Vec<Closure<dyn FnMut(JsValue)>>,
}

impl WebWindowInner {
    pub fn register_event_listeners(self: &Rc<Self>) -> WebEventListeners {
        let mut closures = vec![
            self.register_pointer_down(),
            self.register_pointer_up(),
            self.register_pointer_cancel(),
            self.register_lost_pointer_capture(),
            self.register_pointer_move(),
            self.register_pointer_leave(),
            self.register_wheel(),
            self.register_context_menu(),
            self.register_dragover(),
            self.register_drop(),
            self.register_dragleave(),
            self.register_key_down(),
            self.register_key_up(),
            self.register_composition_start(),
            self.register_composition_update(),
            self.register_composition_end(),
            self.register_focus(),
            self.register_blur(),
            self.register_pointer_enter(),
            self.register_pointer_leave_hover(),
        ];
        closures.extend(self.register_visibility_change());
        closures.extend(self.register_appearance_change());

        WebEventListeners { closures }
    }

    fn listen(
        self: &Rc<Self>,
        event_name: &str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> Closure<dyn FnMut(JsValue)> {
        let closure = Closure::<dyn FnMut(JsValue)>::new(handler);
        self.canvas
            .add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref())
            .ok();
        closure
    }

    fn listen_input(
        self: &Rc<Self>,
        event_name: &str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> Closure<dyn FnMut(JsValue)> {
        let closure = Closure::<dyn FnMut(JsValue)>::new(handler);
        self.input_element
            .add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref())
            .ok();
        closure
    }

    /// Registers a listener with `{passive: false}` so that `preventDefault()` works.
    /// Needed for events like `wheel` which are passive by default in modern browsers.
    fn listen_non_passive(
        self: &Rc<Self>,
        event_name: &str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> Closure<dyn FnMut(JsValue)> {
        let closure = Closure::<dyn FnMut(JsValue)>::new(handler);
        let canvas_js: &JsValue = self.canvas.as_ref();
        let callback_js: &JsValue = closure.as_ref();
        let options = js_sys::Object::new();
        js_sys::Reflect::set(&options, &"passive".into(), &false.into()).ok();
        if let Ok(add_fn_val) = js_sys::Reflect::get(canvas_js, &"addEventListener".into()) {
            if let Ok(add_fn) = add_fn_val.dyn_into::<js_sys::Function>() {
                add_fn
                    .call3(canvas_js, &event_name.into(), callback_js, &options)
                    .ok();
            }
        }
        closure
    }

    fn dispatch_input(&self, input: PlatformInput) -> Option<DispatchEventResult> {
        dispatch_web_input(&self.callbacks, input)
    }

    fn pointer_input_boundary(&self) -> WebPointerInputBoundary<'_> {
        WebPointerInputBoundary {
            pointer_capture: &self.pointer_capture,
            click_state: &self.click_state,
            callbacks: &self.callbacks,
        }
    }

    fn apply_pointer_capture_command(&self, command: WebPointerCaptureCommand) {
        match command {
            WebPointerCaptureCommand::None => {}
            WebPointerCaptureCommand::Set(pointer_id) => {
                self.canvas.set_pointer_capture(pointer_id).ok();
            }
            WebPointerCaptureCommand::Release(pointer_id) => {
                self.canvas.release_pointer_capture(pointer_id).ok();
            }
        }
    }

    fn apply_pointer_transition(
        &self,
        next_state: WebPointerCaptureState,
        transition: WebPointerTransition,
    ) {
        self.pointer_input_boundary()
            .apply_pointer_transition(next_state, transition, |command| {
                self.apply_pointer_capture_command(command)
            });
    }

    pub(crate) fn cleanup_pointer_capture(&self) {
        let (next_state, transition) = self.pointer_capture.get().cleanup();
        self.apply_pointer_transition(next_state, transition);
    }

    fn register_pointer_down(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerdown", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            event.prevent_default();
            this.input_element.focus().ok();

            let event = WebPointerEventData {
                pointer_id: event.pointer_id(),
                button: event.button(),
                buttons: event.buttons(),
                position: pointer_position_in_element(&event),
                modifiers: modifiers_from_mouse_event(&event, this.is_mac),
                click_time: js_sys::Date::now(),
            };
            this.pointer_input_boundary().handle_pointer_down(
                event,
                |command| this.apply_pointer_capture_command(command),
                |position, modifiers| {
                    let mut current_state = this.state.borrow_mut();
                    current_state.mouse_position = position;
                    current_state.modifiers = modifiers;
                },
            );
        })
    }

    fn register_pointer_up(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerup", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            event.prevent_default();

            let event = WebPointerEventData {
                pointer_id: event.pointer_id(),
                button: event.button(),
                buttons: event.buttons(),
                position: pointer_position_in_element(&event),
                modifiers: modifiers_from_mouse_event(&event, this.is_mac),
                click_time: js_sys::Date::now(),
            };
            this.pointer_input_boundary().handle_pointer_up(
                event,
                |command| this.apply_pointer_capture_command(command),
                |position, modifiers| {
                    let mut current_state = this.state.borrow_mut();
                    current_state.mouse_position = position;
                    current_state.modifiers = modifiers;
                },
            );
        })
    }

    fn register_pointer_cancel(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointercancel", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            event.prevent_default();
            let (next_state, transition) = this
                .pointer_capture
                .get()
                .pointer_cancel(event.pointer_id());
            this.apply_pointer_transition(next_state, transition);
        })
    }

    fn register_lost_pointer_capture(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("lostpointercapture", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            let (next_state, transition) = this
                .pointer_capture
                .get()
                .pointer_capture_lost(event.pointer_id());
            this.apply_pointer_transition(next_state, transition);
        })
    }

    fn register_pointer_move(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointermove", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            event.prevent_default();

            let event = WebPointerEventData {
                pointer_id: event.pointer_id(),
                button: event.button(),
                buttons: event.buttons(),
                position: pointer_position_in_element(&event),
                modifiers: modifiers_from_mouse_event(&event, this.is_mac),
                click_time: js_sys::Date::now(),
            };
            this.pointer_input_boundary().handle_pointer_motion(
                event,
                WebPointerMotionKind::Moved,
                |command| this.apply_pointer_capture_command(command),
                |position, modifiers| {
                    let mut current_state = this.state.borrow_mut();
                    current_state.mouse_position = position;
                    current_state.modifiers = modifiers;
                },
            );
        })
    }

    fn register_pointer_leave(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerleave", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();

            let event = WebPointerEventData {
                pointer_id: event.pointer_id(),
                button: event.button(),
                buttons: event.buttons(),
                position: pointer_position_in_element(&event),
                modifiers: modifiers_from_mouse_event(&event, this.is_mac),
                click_time: js_sys::Date::now(),
            };
            this.pointer_input_boundary().handle_pointer_motion(
                event,
                WebPointerMotionKind::Exited,
                |command| this.apply_pointer_capture_command(command),
                |position, modifiers| {
                    let mut current_state = this.state.borrow_mut();
                    current_state.mouse_position = position;
                    current_state.modifiers = modifiers;
                },
            );
        })
    }

    fn register_wheel(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_non_passive("wheel", move |event: JsValue| {
            let event: web_sys::WheelEvent = event.unchecked_into();
            event.prevent_default();

            let mouse_event: &web_sys::MouseEvent = event.as_ref();
            let position = mouse_position_in_element(mouse_event);
            let modifiers = modifiers_from_wheel_event(mouse_event, this.is_mac);

            let delta_mode = event.delta_mode();
            let delta = if delta_mode == 1 {
                ScrollDelta::Lines(point(-event.delta_x() as f32, -event.delta_y() as f32))
            } else {
                ScrollDelta::Pixels(point(
                    px(-event.delta_x() as f32),
                    px(-event.delta_y() as f32),
                ))
            };

            {
                let mut current_state = this.state.borrow_mut();
                current_state.modifiers = modifiers;
            }

            this.dispatch_input(PlatformInput::ScrollWheel(ScrollWheelEvent {
                position,
                delta,
                modifiers,
                touch_phase: TouchPhase::Moved,
            }));
        })
    }

    fn register_context_menu(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        self.listen("contextmenu", move |event: JsValue| {
            let event: web_sys::Event = event.unchecked_into();
            event.prevent_default();
        })
    }

    fn register_dragover(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("dragover", move |event: JsValue| {
            let event: web_sys::DragEvent = event.unchecked_into();
            event.prevent_default();

            let mouse_event: &web_sys::MouseEvent = event.as_ref();
            let position = mouse_position_in_element(mouse_event);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
            }

            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Pending { position }));
        })
    }

    fn register_drop(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("drop", move |event: JsValue| {
            let event: web_sys::DragEvent = event.unchecked_into();
            event.prevent_default();

            let mouse_event: &web_sys::MouseEvent = event.as_ref();
            let position = mouse_position_in_element(mouse_event);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
            }

            let paths = extract_file_paths_from_drag(&event);

            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Entered {
                position,
                paths: ExternalPaths(paths),
            }));

            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Submit { position }));
        })
    }

    fn register_dragleave(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("dragleave", move |_event: JsValue| {
            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Exited));
        })
    }

    fn register_key_down(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("keydown", move |event: JsValue| {
            let event: web_sys::KeyboardEvent = event.unchecked_into();

            let modifiers = modifiers_from_keyboard_event(&event, this.is_mac);
            let capslock = capslock_from_keyboard_event(&event);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.modifiers = modifiers;
                current_state.capslock = capslock;
            }

            this.dispatch_input(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                modifiers,
                capslock,
            }));

            let key = dom_key_to_gpui_key(&event);

            if is_modifier_only_key(&key) {
                return;
            }

            event.prevent_default();

            let is_held = event.repeat();
            let key_char = compute_key_char(&event, &key, &modifiers);

            let keystroke = Keystroke {
                modifiers,
                key,
                key_char: key_char.clone(),
            };

            let result = this.dispatch_input(PlatformInput::KeyDown(KeyDownEvent {
                keystroke,
                is_held,
                prefer_character_input: false,
            }));

            if let Some(result) = result {
                if !result.propagate {
                    return;
                }
            }

            if this.is_composing.get() || event.is_composing() {
                return;
            }

            if modifiers.is_subset_of(&Modifiers::shift()) {
                if let Some(text) = key_char {
                    this.with_input_handler(|handler| {
                        handler.replace_text_in_range(None, &text);
                    });
                }
            }
        })
    }

    fn register_key_up(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("keyup", move |event: JsValue| {
            let event: web_sys::KeyboardEvent = event.unchecked_into();

            let modifiers = modifiers_from_keyboard_event(&event, this.is_mac);
            let capslock = capslock_from_keyboard_event(&event);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.modifiers = modifiers;
                current_state.capslock = capslock;
            }

            this.dispatch_input(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                modifiers,
                capslock,
            }));

            let key = dom_key_to_gpui_key(&event);

            if is_modifier_only_key(&key) {
                return;
            }

            event.prevent_default();

            let key_char = compute_key_char(&event, &key, &modifiers);

            let keystroke = Keystroke {
                modifiers,
                key,
                key_char,
            };

            this.dispatch_input(PlatformInput::KeyUp(KeyUpEvent { keystroke }));
        })
    }

    fn register_composition_start(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("compositionstart", move |_event: JsValue| {
            this.is_composing.set(true);
        })
    }

    fn register_composition_update(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("compositionupdate", move |event: JsValue| {
            let event: web_sys::CompositionEvent = event.unchecked_into();
            let data = event.data().unwrap_or_default();
            this.is_composing.set(true);
            this.with_input_handler(|handler| {
                handler.replace_and_mark_text_in_range(None, &data, None);
            });
        })
    }

    fn register_composition_end(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("compositionend", move |event: JsValue| {
            let event: web_sys::CompositionEvent = event.unchecked_into();
            let data = event.data().unwrap_or_default();
            this.is_composing.set(false);
            this.with_input_handler(|handler| {
                handler.replace_text_in_range(None, &data);
                handler.unmark_text();
            });
            this.input_element.set_value("");
        })
    }

    fn register_focus(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("focus", move |_event: JsValue| {
            {
                let mut state = this.state.borrow_mut();
                state.is_active = true;
            }
            this.dispatch_active_status_change(true);
        })
    }

    fn register_blur(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("blur", move |_event: JsValue| {
            this.cleanup_pointer_capture();
            {
                let mut state = this.state.borrow_mut();
                state.is_active = false;
            }
            this.dispatch_active_status_change(false);
        })
    }

    fn register_pointer_enter(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerenter", move |_event: JsValue| {
            {
                let mut state = this.state.borrow_mut();
                state.is_hovered = true;
            }
            this.dispatch_hover_status_change(true);
        })
    }

    fn register_pointer_leave_hover(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerleave", move |_event: JsValue| {
            {
                let mut state = this.state.borrow_mut();
                state.is_hovered = false;
            }
            this.dispatch_hover_status_change(false);
        })
    }
}

fn dom_key_to_gpui_key(event: &web_sys::KeyboardEvent) -> String {
    let key = event.key();
    match key.as_str() {
        "Enter" => "enter".to_string(),
        "Backspace" => "backspace".to_string(),
        "Tab" => "tab".to_string(),
        "Escape" => "escape".to_string(),
        "Delete" => "delete".to_string(),
        " " => "space".to_string(),
        "ArrowLeft" => "left".to_string(),
        "ArrowRight" => "right".to_string(),
        "ArrowUp" => "up".to_string(),
        "ArrowDown" => "down".to_string(),
        "Home" => "home".to_string(),
        "End" => "end".to_string(),
        "PageUp" => "pageup".to_string(),
        "PageDown" => "pagedown".to_string(),
        "Insert" => "insert".to_string(),
        "Control" => "control".to_string(),
        "Alt" => "alt".to_string(),
        "Shift" => "shift".to_string(),
        "Meta" => "platform".to_string(),
        "CapsLock" => "capslock".to_string(),
        other => {
            if let Some(rest) = other.strip_prefix('F') {
                if let Ok(number) = rest.parse::<u8>() {
                    if (1..=35).contains(&number) {
                        return format!("f{number}");
                    }
                }
            }
            other.to_lowercase()
        }
    }
}

fn pointer_button_change_to_platform_input(
    change: WebPointerButtonChange,
    position: Point<Pixels>,
    modifiers: Modifiers,
    click_count: usize,
) -> PlatformInput {
    match change {
        WebPointerButtonChange::Down(button) => PlatformInput::MouseDown(MouseDownEvent {
            button,
            position,
            modifiers,
            click_count,
            first_mouse: false,
        }),
        WebPointerButtonChange::Up(button) => PlatformInput::MouseUp(MouseUpEvent {
            button,
            position,
            modifiers,
            click_count,
        }),
    }
}

fn dispatch_web_input(
    callbacks: &RefCell<WebWindowCallbacks>,
    input: PlatformInput,
) -> Option<DispatchEventResult> {
    let mut callback = callbacks.borrow_mut().input.take();
    let result = callback.as_mut().map(|callback| callback(input));

    if let Some(callback) = callback {
        callbacks.borrow_mut().input = Some(callback);
    }

    result
}

#[derive(Clone, Copy)]
struct WebPointerEventData {
    pointer_id: i32,
    button: i16,
    buttons: u16,
    position: Point<Pixels>,
    modifiers: Modifiers,
    click_time: f64,
}

#[derive(Clone, Copy)]
enum WebPointerMotionKind {
    Moved,
    Exited,
}

struct WebPointerInputBoundary<'a> {
    pointer_capture: &'a Cell<WebPointerCaptureState>,
    click_state: &'a RefCell<ClickState>,
    callbacks: &'a RefCell<WebWindowCallbacks>,
}

impl WebPointerInputBoundary<'_> {
    fn apply_pointer_transition(
        &self,
        next_state: WebPointerCaptureState,
        transition: WebPointerTransition,
        mut apply_capture_command: impl FnMut(WebPointerCaptureCommand),
    ) {
        self.pointer_capture.set(next_state);
        apply_capture_command(transition.capture_command);
        if transition.emit_cancel {
            let _ = dispatch_web_input(
                self.callbacks,
                PlatformInput::PointerCanceled(PointerCancelEvent {
                    reason: PointerCancelReason::PlatformCaptureLost,
                }),
            );
        }
    }

    fn dispatch_pointer_button_change(
        &self,
        change: WebPointerButtonChange,
        event: WebPointerEventData,
    ) {
        let click_count = match change {
            WebPointerButtonChange::Down(button) => self.click_state.borrow_mut().register_click(
                button,
                event.position,
                event.click_time,
            ),
            WebPointerButtonChange::Up(_) => self.click_state.borrow().current_count(),
        };
        let _ = dispatch_web_input(
            self.callbacks,
            pointer_button_change_to_platform_input(
                change,
                event.position,
                event.modifiers,
                click_count,
            ),
        );
    }

    fn handle_pointer_down(
        &self,
        event: WebPointerEventData,
        apply_capture_command: impl FnMut(WebPointerCaptureCommand),
        update_pointer_state: impl FnOnce(Point<Pixels>, Modifiers),
    ) {
        let (next_state, transition) =
            self.pointer_capture
                .get()
                .pointer_down(event.pointer_id, event.button, event.buttons);
        self.apply_pointer_transition(next_state, transition, apply_capture_command);
        if !transition.accept_event {
            return;
        }

        update_pointer_state(event.position, event.modifiers);
        self.dispatch_pointer_button_change(
            WebPointerButtonChange::Down(dom_mouse_button_to_gpui(event.button)),
            event,
        );
    }

    fn handle_pointer_up(
        &self,
        event: WebPointerEventData,
        apply_capture_command: impl FnMut(WebPointerCaptureCommand),
        update_pointer_state: impl FnOnce(Point<Pixels>, Modifiers),
    ) {
        let (next_state, transition) =
            self.pointer_capture
                .get()
                .pointer_up(event.pointer_id, event.button, event.buttons);
        self.apply_pointer_transition(next_state, transition, apply_capture_command);
        if !transition.accept_event {
            return;
        }

        update_pointer_state(event.position, event.modifiers);
        self.dispatch_pointer_button_change(
            WebPointerButtonChange::Up(dom_mouse_button_to_gpui(event.button)),
            event,
        );
    }

    fn handle_pointer_motion(
        &self,
        event: WebPointerEventData,
        motion_kind: WebPointerMotionKind,
        apply_capture_command: impl FnMut(WebPointerCaptureCommand),
        update_pointer_state: impl FnOnce(Point<Pixels>, Modifiers),
    ) {
        let current_pressed = dom_buttons_to_pressed_button(event.buttons);
        let (next_state, transition) = self.pointer_capture.get().pointer_motion(
            event.pointer_id,
            event.button,
            event.buttons,
        );
        self.apply_pointer_transition(next_state, transition, apply_capture_command);
        if !transition.accept_event {
            return;
        }

        update_pointer_state(event.position, event.modifiers);
        if let Some(change) = transition.button_change {
            self.dispatch_pointer_button_change(change, event);
        }

        let input = match motion_kind {
            WebPointerMotionKind::Moved => PlatformInput::MouseMove(MouseMoveEvent {
                position: event.position,
                pressed_button: current_pressed,
                modifiers: event.modifiers,
            }),
            WebPointerMotionKind::Exited => PlatformInput::MouseExited(MouseExitEvent {
                position: event.position,
                pressed_button: current_pressed,
                modifiers: event.modifiers,
            }),
        };
        let _ = dispatch_web_input(self.callbacks, input);
    }
}

fn modifiers_from_keyboard_event(event: &web_sys::KeyboardEvent, _is_mac: bool) -> Modifiers {
    Modifiers {
        control: event.ctrl_key(),
        alt: event.alt_key(),
        shift: event.shift_key(),
        platform: event.meta_key(),
        function: false,
    }
}

fn modifiers_from_mouse_event(event: &web_sys::PointerEvent, _is_mac: bool) -> Modifiers {
    let mouse_event: &web_sys::MouseEvent = event.as_ref();
    Modifiers {
        control: mouse_event.ctrl_key(),
        alt: mouse_event.alt_key(),
        shift: mouse_event.shift_key(),
        platform: mouse_event.meta_key(),
        function: false,
    }
}

fn modifiers_from_wheel_event(event: &web_sys::MouseEvent, _is_mac: bool) -> Modifiers {
    Modifiers {
        control: event.ctrl_key(),
        alt: event.alt_key(),
        shift: event.shift_key(),
        platform: event.meta_key(),
        function: false,
    }
}

fn capslock_from_keyboard_event(event: &web_sys::KeyboardEvent) -> Capslock {
    Capslock {
        on: event.get_modifier_state("CapsLock"),
    }
}

pub(crate) fn is_mac_platform(browser_window: &web_sys::Window) -> bool {
    let navigator = browser_window.navigator();

    #[allow(deprecated)]
    // navigator.platform() is deprecated but navigator.userAgentData is not widely available yet
    if let Ok(platform) = navigator.platform() {
        if platform.contains("Mac") {
            return true;
        }
    }

    if let Ok(user_agent) = navigator.user_agent() {
        return user_agent.contains("Mac");
    }

    false
}

fn is_modifier_only_key(key: &str) -> bool {
    matches!(
        key,
        "control" | "alt" | "shift" | "platform" | "capslock" | "compose" | "process"
    )
}

fn compute_key_char(
    event: &web_sys::KeyboardEvent,
    gpui_key: &str,
    modifiers: &Modifiers,
) -> Option<String> {
    if modifiers.platform || modifiers.control {
        return None;
    }

    if is_modifier_only_key(gpui_key) {
        return None;
    }

    if gpui_key == "space" {
        return Some(" ".to_string());
    }

    let raw_key = event.key();

    if raw_key.len() == 1 {
        return Some(raw_key);
    }

    None
}

fn pointer_position_in_element(event: &web_sys::PointerEvent) -> Point<Pixels> {
    let mouse_event: &web_sys::MouseEvent = event.as_ref();
    mouse_position_in_element(mouse_event)
}

fn mouse_position_in_element(event: &web_sys::MouseEvent) -> Point<Pixels> {
    // offset_x/offset_y give position relative to the target element's padding edge
    point(px(event.offset_x() as f32), px(event.offset_y() as f32))
}

fn extract_file_paths_from_drag(
    event: &web_sys::DragEvent,
) -> smallvec::SmallVec<[std::path::PathBuf; 2]> {
    let mut paths = smallvec![];
    let Some(data_transfer) = event.data_transfer() else {
        return paths;
    };
    let file_list = data_transfer.files();
    let Some(files) = file_list else {
        return paths;
    };
    for index in 0..files.length() {
        if let Some(file) = files.get(index) {
            paths.push(std::path::PathBuf::from(file.name()));
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    use open_gpui::{
        DispatchEventResult, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, PlatformInput,
        Point,
    };

    use super::{
        ClickState, WebPointerButtonChange, WebPointerCaptureCommand, WebPointerCaptureState,
        WebPointerEventData, WebPointerInputBoundary, WebPointerMotionKind,
        pointer_button_change_to_platform_input,
    };
    use crate::window::WebWindowCallbacks;

    #[test]
    fn click_count_is_scoped_to_the_changed_button() {
        let mut click_state = ClickState::default();
        let position = Point::default();

        assert_eq!(
            click_state.register_click(MouseButton::Left, position, 100.0),
            1
        );
        assert_eq!(
            click_state.register_click(MouseButton::Left, position, 200.0),
            2
        );
        assert_eq!(
            click_state.register_click(MouseButton::Right, position, 250.0),
            1
        );
    }

    #[test]
    fn pointer_capture_tracks_first_companion_and_final_buttons() {
        let (state, transition) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        assert_eq!(transition.capture_command, WebPointerCaptureCommand::Set(7));
        assert!(transition.accept_event);

        let (state, unchanged) = state.pointer_motion(7, -1, 1);
        assert!(unchanged.button_change.is_none());

        let (state, transition) = state.pointer_motion(7, 2, 3);
        assert_eq!(transition.capture_command, WebPointerCaptureCommand::None);
        assert!(transition.accept_event);
        assert_eq!(
            transition.button_change,
            Some(WebPointerButtonChange::Down(MouseButton::Right))
        );

        let (state, unchanged) = state.pointer_motion(7, -1, 3);
        assert!(unchanged.button_change.is_none());

        let (state, transition) = state.pointer_motion(7, 2, 1);
        assert_eq!(transition.capture_command, WebPointerCaptureCommand::None);
        assert!(transition.accept_event);
        assert_eq!(
            transition.button_change,
            Some(WebPointerButtonChange::Up(MouseButton::Right))
        );

        let (state, transition) = state.pointer_up(7, 0, 0);
        assert_eq!(state, WebPointerCaptureState::default());
        assert_eq!(
            transition.capture_command,
            WebPointerCaptureCommand::Release(7)
        );
        assert!(transition.accept_event);
        assert!(!transition.emit_cancel);

        let (_, unchanged) = state.pointer_motion(7, -1, 0);
        assert!(unchanged.button_change.is_none());
    }

    #[test]
    fn lost_capture_with_held_buttons_cancels_once() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, transition) = state.pointer_capture_lost(7);
        assert_eq!(state, WebPointerCaptureState::default());
        assert!(transition.emit_cancel);

        let (_, duplicate) = state.pointer_capture_lost(7);
        assert!(!duplicate.emit_cancel);
    }

    #[test]
    fn pointer_cancel_then_lost_capture_does_not_duplicate_cancel() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, transition) = state.pointer_cancel(7);
        assert_eq!(state, WebPointerCaptureState::default());
        assert_eq!(
            transition.capture_command,
            WebPointerCaptureCommand::Release(7)
        );
        assert!(transition.emit_cancel);

        let (_, duplicate) = state.pointer_capture_lost(7);
        assert!(!duplicate.emit_cancel);
    }

    #[test]
    fn final_pointer_up_then_lost_capture_does_not_cancel() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, transition) = state.pointer_up(7, 0, 0);
        assert!(!transition.emit_cancel);

        let (_, lost) = state.pointer_capture_lost(7);
        assert!(!lost.emit_cancel);
    }

    #[test]
    fn unexpected_held_button_loss_is_terminal() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, _) = state.pointer_motion(7, 2, 3);
        let (state, transition) = state.pointer_motion(7, -1, 1);

        assert_eq!(state, WebPointerCaptureState::default());
        assert_eq!(
            transition.capture_command,
            WebPointerCaptureCommand::Release(7)
        );
        assert!(!transition.accept_event);
        assert!(transition.emit_cancel);
    }

    #[test]
    fn chorded_button_changes_translate_to_gpui_button_inputs() {
        let position = Point::default();
        let modifiers = Modifiers::default();
        let down = pointer_button_change_to_platform_input(
            WebPointerButtonChange::Down(MouseButton::Right),
            position,
            modifiers,
            2,
        );
        assert!(matches!(
            down,
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Right,
                click_count: 2,
                ..
            })
        ));

        let up = pointer_button_change_to_platform_input(
            WebPointerButtonChange::Up(MouseButton::Right),
            position,
            modifiers,
            2,
        );
        assert!(matches!(
            up,
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Right,
                click_count: 2,
                ..
            })
        ));
    }

    #[test]
    fn pointer_listener_boundary_dispatches_chorded_buttons_through_registered_callback() {
        #[derive(Debug, Eq, PartialEq)]
        enum ObservedInput {
            Down(MouseButton, usize),
            Up(MouseButton, usize),
            Moved(Option<MouseButton>),
        }

        let observed = Rc::new(RefCell::new(Vec::new()));
        let callbacks = RefCell::new(WebWindowCallbacks::default());
        callbacks.borrow_mut().set_input(Box::new({
            let observed = observed.clone();
            move |input| {
                match input {
                    PlatformInput::MouseDown(event) => observed
                        .borrow_mut()
                        .push(ObservedInput::Down(event.button, event.click_count)),
                    PlatformInput::MouseUp(event) => observed
                        .borrow_mut()
                        .push(ObservedInput::Up(event.button, event.click_count)),
                    PlatformInput::MouseMove(event) => observed
                        .borrow_mut()
                        .push(ObservedInput::Moved(event.pressed_button)),
                    _ => {}
                }
                DispatchEventResult::default()
            }
        }));

        let pointer_capture = Cell::new(WebPointerCaptureState::default());
        let click_state = RefCell::new(ClickState::default());
        let boundary = WebPointerInputBoundary {
            pointer_capture: &pointer_capture,
            click_state: &click_state,
            callbacks: &callbacks,
        };
        let position = Point::default();
        let event = |button, buttons, click_time| WebPointerEventData {
            pointer_id: 7,
            button,
            buttons,
            position,
            modifiers: Modifiers::default(),
            click_time,
        };
        let mut capture_commands = Vec::new();

        boundary.handle_pointer_down(
            event(0, 1, 100.0),
            |command| capture_commands.push(command),
            |_, _| {},
        );
        boundary.handle_pointer_motion(
            event(2, 3, 150.0),
            WebPointerMotionKind::Moved,
            |command| capture_commands.push(command),
            |_, _| {},
        );
        boundary.handle_pointer_motion(
            event(2, 1, 200.0),
            WebPointerMotionKind::Moved,
            |command| capture_commands.push(command),
            |_, _| {},
        );
        boundary.handle_pointer_up(
            event(0, 0, 250.0),
            |command| capture_commands.push(command),
            |_, _| {},
        );

        assert_eq!(
            *observed.borrow(),
            vec![
                ObservedInput::Down(MouseButton::Left, 1),
                ObservedInput::Down(MouseButton::Right, 1),
                ObservedInput::Moved(Some(MouseButton::Left)),
                ObservedInput::Up(MouseButton::Right, 1),
                ObservedInput::Moved(Some(MouseButton::Left)),
                ObservedInput::Up(MouseButton::Left, 1),
            ]
        );
        assert_eq!(
            capture_commands,
            vec![
                WebPointerCaptureCommand::Set(7),
                WebPointerCaptureCommand::None,
                WebPointerCaptureCommand::None,
                WebPointerCaptureCommand::Release(7),
            ]
        );
        assert_eq!(pointer_capture.get(), WebPointerCaptureState::default());
    }

    #[test]
    fn repeated_cleanup_cancels_only_the_active_session() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, cleanup) = state.cleanup();
        assert_eq!(
            cleanup.capture_command,
            WebPointerCaptureCommand::Release(7)
        );
        assert!(cleanup.emit_cancel);

        let (_, duplicate_cleanup) = state.cleanup();
        assert!(!duplicate_cleanup.emit_cancel);
    }
}
