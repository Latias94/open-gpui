use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use open_gpui::{
    Capslock, ClipboardEntry, ClipboardItem, ClipboardString, DispatchEventResult, ExternalPaths,
    FileDropEvent, GestureTuning, Image, ImageFormat, KeyDownEvent, KeyUpEvent, Keystroke,
    Modifiers, ModifiersChangedEvent, MouseDownEvent, MouseExitEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, PlatformInput, Point, PointerCancelEvent, PointerCancelReason, ScrollDelta,
    ScrollEvent, ScrollSource, TouchEvent, TouchPhase, point, px,
};
use smallvec::smallvec;
use wasm_bindgen::prelude::*;

pub(crate) use crate::interaction::WebInteractionGate;

use crate::{
    pointer_session::{
        ClickState, WebPointerButtonChange, WebPointerCaptureCommand, WebPointerCaptureState,
        WebPointerTransition, dom_buttons_to_pressed_button, dom_mouse_button_to_gpui,
    },
    window::{WebWindowCallbacks, WebWindowInner},
};

pub struct WebEventListeners {
    _listeners: Vec<WebEventListener>,
}

impl WebInteractionGate {
    fn dispatch_input(
        &self,
        callbacks: &RefCell<WebWindowCallbacks>,
        input: PlatformInput,
    ) -> DispatchEventResult {
        let input_callback = callbacks.borrow().input.clone();
        self.dispatch_callback(&input_callback, input)
    }
}

pub(crate) struct WebEventListener {
    target: web_sys::EventTarget,
    event_name: &'static str,
    closure: Rc<Closure<dyn FnMut(JsValue)>>,
    capture: bool,
    registered: bool,
}

impl WebEventListener {
    fn register(
        target: &web_sys::EventTarget,
        event_name: &'static str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> Self {
        let closure = Rc::new(Closure::<dyn FnMut(JsValue)>::new(handler));
        let registered = target
            .add_event_listener_with_callback(event_name, closure.as_ref().as_ref().unchecked_ref())
            .is_ok();

        Self {
            target: target.clone(),
            event_name,
            closure,
            capture: false,
            registered,
        }
    }

    fn register_non_passive(
        target: &web_sys::EventTarget,
        event_name: &'static str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> Self {
        let closure = Rc::new(Closure::<dyn FnMut(JsValue)>::new(handler));
        let target_js: &JsValue = target.as_ref();
        let callback_js: &JsValue = closure.as_ref().as_ref();
        let options = js_sys::Object::new();
        js_sys::Reflect::set(&options, &"passive".into(), &false.into()).ok();
        let registered = js_sys::Reflect::get(target_js, &"addEventListener".into())
            .ok()
            .and_then(|add| add.dyn_into::<js_sys::Function>().ok())
            .and_then(|add| {
                add.call3(target_js, &event_name.into(), callback_js, &options)
                    .ok()
            })
            .is_some();

        Self {
            target: target.clone(),
            event_name,
            closure,
            capture: false,
            registered,
        }
    }

    pub(crate) fn register_shared_capture(
        target: &web_sys::EventTarget,
        event_name: &'static str,
        closure: Rc<Closure<dyn FnMut(JsValue)>>,
    ) -> Result<Self, JsValue> {
        target.add_event_listener_with_callback_and_bool(
            event_name,
            closure.as_ref().as_ref().unchecked_ref(),
            true,
        )?;
        Ok(Self {
            target: target.clone(),
            event_name,
            closure,
            capture: true,
            registered: true,
        })
    }

    pub(crate) fn event_name(&self) -> &'static str {
        self.event_name
    }
}

impl Drop for WebEventListener {
    fn drop(&mut self) {
        if self.registered {
            if self.capture {
                self.target
                    .remove_event_listener_with_callback_and_bool(
                        self.event_name,
                        self.closure.as_ref().as_ref().unchecked_ref(),
                        true,
                    )
                    .ok();
            } else {
                // DOM removal matches type, callback, and capture; passive need not be repeated.
                self.target
                    .remove_event_listener_with_callback(
                        self.event_name,
                        self.closure.as_ref().as_ref().unchecked_ref(),
                    )
                    .ok();
            }
        }
    }
}

impl WebWindowInner {
    pub fn register_event_listeners(self: &Rc<Self>) -> WebEventListeners {
        let mut listeners = vec![
            self.register_pointer_down(),
            self.register_pointer_up(),
            self.register_pointer_cancel(),
            self.register_lost_pointer_capture(),
            self.register_touch_end(),
            self.register_pointer_move(),
            self.register_pointer_leave(),
            self.register_wheel(),
            self.register_context_menu(),
            self.register_dragover(),
            self.register_drop(),
            self.register_dragleave(),
            self.register_key_down(),
            self.register_key_up(),
            self.register_paste(),
            self.register_composition_start(),
            self.register_composition_update(),
            self.register_composition_end(),
            self.register_focus(),
            self.register_blur(),
            self.register_pointer_enter(),
            self.register_pointer_leave_hover(),
        ];
        listeners.extend(self.register_global_window_activation_change());
        listeners.extend(self.register_global_visibility_change());
        listeners.extend(self.register_global_appearance_change());

        WebEventListeners {
            _listeners: listeners,
        }
    }

    fn listen(
        self: &Rc<Self>,
        event_name: &'static str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> WebEventListener {
        let target: &web_sys::EventTarget = self.canvas.as_ref();
        WebEventListener::register(target, event_name, handler)
    }

    fn listen_input(
        self: &Rc<Self>,
        event_name: &'static str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> WebEventListener {
        let target: &web_sys::EventTarget = self.input_element.as_ref();
        WebEventListener::register(target, event_name, handler)
    }

    /// Registers a listener with `{passive: false}` so that `preventDefault()` works.
    /// Needed for events like `wheel` which are passive by default in modern browsers.
    fn listen_non_passive(
        self: &Rc<Self>,
        event_name: &'static str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> WebEventListener {
        let target: &web_sys::EventTarget = self.canvas.as_ref();
        WebEventListener::register_non_passive(target, event_name, handler)
    }

    fn register_global_visibility_change(self: &Rc<Self>) -> Option<WebEventListener> {
        let document = self.browser_window.document()?;
        let target: &web_sys::EventTarget = document.as_ref();
        let this = Rc::clone(self);

        Some(WebEventListener::register(
            target,
            "visibilitychange",
            move |_event: JsValue| this.sync_dom_visibility(),
        ))
    }

    fn register_global_window_activation_change(self: &Rc<Self>) -> [WebEventListener; 2] {
        let target: &web_sys::EventTarget = self.browser_window.as_ref();
        let focused = Rc::clone(self);
        let blurred = Rc::clone(self);

        [
            WebEventListener::register(target, "focus", move |_event: JsValue| {
                focused.sync_dom_activation();
            }),
            WebEventListener::register(target, "blur", move |_event: JsValue| {
                blurred.sync_dom_activation();
            }),
        ]
    }

    fn register_global_appearance_change(self: &Rc<Self>) -> Option<WebEventListener> {
        let media_query = self
            .browser_window
            .match_media("(prefers-color-scheme: dark)")
            .ok()??;
        let target: &web_sys::EventTarget = media_query.as_ref();
        let this = Rc::clone(self);

        Some(WebEventListener::register(
            target,
            "change",
            move |_event: JsValue| this.dispatch_appearance_changed(),
        ))
    }

    fn dispatch_input(&self, input: PlatformInput) -> DispatchEventResult {
        self.interaction_gate.dispatch_input(&self.callbacks, input)
    }

    fn pointer_input_boundary(&self) -> WebPointerInputBoundary<'_> {
        WebPointerInputBoundary {
            pointer_capture: &self.pointer_capture,
            click_state: &self.click_state,
            callbacks: &self.callbacks,
            interaction_gate: &self.interaction_gate,
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

    pub(crate) fn cleanup_pointer_capture(&self, reason: PointerCancelReason) {
        self.touch_tap_candidate.set(None);
        let touches = self.touch_state.borrow_mut().cancel_all();
        for (pointer_id, event) in touches {
            self.dispatch_input(PlatformInput::Touch(event));
            self.canvas.release_pointer_capture(pointer_id).ok();
        }
        let (next_state, transition) = self.pointer_capture.get().cleanup(reason);
        self.apply_pointer_transition(next_state, transition);
    }

    fn cancel_touch_pointer(&self, pointer_id: i32) {
        let event = self.touch_state.borrow_mut().cancel(pointer_id);
        let Some(event) = event else {
            return;
        };
        if let Some((candidate, _)) = self.touch_tap_candidate.get()
            && candidate == pointer_id
        {
            self.touch_tap_candidate.set(None);
        }
        self.dispatch_input(PlatformInput::Touch(event));
        self.canvas.release_pointer_capture(pointer_id).ok();
    }

    fn pointer_targets_text_input(&self, position: Point<Pixels>) -> bool {
        self.with_input_handler(|handler| {
            handler.query_accepts_text_input()
                && handler
                    .element_bounds()
                    .is_some_and(|bounds| bounds.contains(&position))
        })
        .unwrap_or(false)
    }

    /// The visual viewport's current height in layout pixels, or zero when
    /// the API is unavailable.
    fn visual_viewport_height(&self) -> f64 {
        self.browser_window
            .visual_viewport()
            .map_or(0.0, |viewport| viewport.height() * viewport.scale())
    }

    /// Infers dismissal only when the visual viewport fills the layout viewport.
    ///
    /// This fallback requires `interactive-widget=resizes-visual` (the modern
    /// browser default). It deliberately makes no inference while zoomed, or
    /// when the host opts into another resize policy. Floating keyboards can
    /// still overlay both viewports; this is not authoritative keyboard state.
    fn keyboard_likely_dismissed(&self) -> bool {
        if !self.touch_input {
            return false;
        }
        let Some(viewport) = self.browser_window.visual_viewport() else {
            return false;
        };
        let Some(document) = self.browser_window.document() else {
            return false;
        };
        if let Ok(Some(meta)) = document.query_selector("meta[name=viewport]")
            && let Some(content) = meta.get_attribute("content")
            && content.split(',').any(|directive| {
                directive.split_once('=').is_some_and(|(key, value)| {
                    key.trim().eq_ignore_ascii_case("interactive-widget")
                        && !value.trim().eq_ignore_ascii_case("resizes-visual")
                })
            })
        {
            return false;
        }
        let Some(root) = document.document_element() else {
            return false;
        };
        (viewport.scale() - 1.).abs() < 0.001
            && viewport.offset_top().abs() < 1.
            && viewport.offset_left().abs() < 1.
            && (viewport.height() - root.client_height() as f64).abs() < 1.
            && (viewport.width() - root.client_width() as f64).abs() < 1.
    }

    fn set_virtual_keyboard_enabled(&self, enabled: bool) {
        if let Err(error) = self
            .input_element
            .set_attribute("inputmode", if enabled { "text" } else { "none" })
        {
            log::warn!("Failed to configure input mode: {error:?}");
        }
    }

    /// Summons the touch keyboard synchronously inside the recognized tap's user gesture.
    pub(crate) fn show_virtual_keyboard(self: &Rc<Self>) {
        if !self.touch_input || !self.allows_pointer_focus() {
            return;
        }
        let was_enabled = self.input_element.get_attribute("inputmode").as_deref() != Some("none");
        self.set_virtual_keyboard_enabled(true);
        // Trigger a focus event only when the keyboard actually needs
        // summoning. Cycling focus on every tap would restart the IME
        // connection right as the keyboard reads the tapped caret's context,
        // racing its word segmentation. But `focus()` on an already-focused
        // element is a no-op, so a dismissed keyboard would otherwise never
        // return for taps that stay within editable content: detect that
        // through the visual viewport and force a fresh focus event.
        if !was_enabled || !self.input_is_focused() || self.keyboard_likely_dismissed() {
            self.focus_touch_input();
        }
    }

    /// Restarts DOM input without synchronously re-entering GPUI activation.
    fn focus_touch_input(self: &Rc<Self>) {
        self.suppress_focus_status_events.set(true);
        if self.input_is_focused() {
            self.input_element.blur().ok();
        }
        self.focus_from_pointer();
        self.suppress_focus_status_events.set(false);

        let callback = wasm_bindgen::closure::Closure::once_into_js({
            let this = Rc::downgrade(self);
            move || {
                if let Some(this) = this.upgrade() {
                    this.sync_dom_activation();
                }
            }
        });
        if let Err(error) = self
            .browser_window
            .set_timeout_with_callback(callback.unchecked_ref())
        {
            log::warn!("failed to defer web window activation: {error:?}");
        }
    }

    /// Cancels touch default handling separately because iOS does not consistently
    /// transfer pointer-event cancellation to the corresponding touch event.
    /// During native scrolling, touchend may be non-cancelable.
    fn register_touch_end(self: &Rc<Self>) -> WebEventListener {
        self.listen_non_passive("touchend", move |event: JsValue| {
            let event: web_sys::Event = event.unchecked_into();
            if event.cancelable() {
                event.prevent_default();
            }
        })
    }

    fn register_pointer_down(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen("pointerdown", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            event.prevent_default();
            if this.interaction_gate.is_quiesced() {
                return;
            }
            let position = pointer_position_in_element(&event);
            this.gesture_start_visual_viewport_height
                .set(this.visual_viewport_height());

            if event.pointer_type() == "touch" {
                // Android can reopen a manually dismissed keyboard on the
                // next touch even without another focus() call. Disarm the
                // still-focused mirror before the browser handles that
                // gesture; only an editable tap may enable its keyboard again.
                if this.keyboard_likely_dismissed() {
                    this.set_virtual_keyboard_enabled(false);
                }
                let Some(touch_id) = this
                    .touch_state
                    .borrow_mut()
                    .start(event.pointer_id(), position)
                else {
                    log::error!("exhausted touch identifiers");
                    return;
                };
                this.canvas.set_pointer_capture(event.pointer_id()).ok();
                this.state.borrow_mut().mouse_position = position;
                if this.touch_tap_candidate.get().is_none() {
                    this.touch_tap_candidate
                        .set(Some((event.pointer_id(), position)));
                }
                this.dispatch_input(PlatformInput::Touch(TouchEvent {
                    id: touch_id,
                    phase: TouchPhase::Started,
                    position,
                    predicted_position: None,
                    force: None,
                }));
                // Keyboard and IME focus intentionally do not change here:
                // whether this touch is a tap or a pan is only known at
                // release, and only a tap may affect them (see
                // `touch_tap_candidate`). The release handler still runs
                // within a user gesture, as keyboard summoning requires.
                return;
            }

            this.focus_from_pointer();

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

    fn register_pointer_up(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen("pointerup", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            event.prevent_default();
            if this.interaction_gate.is_quiesced() {
                return;
            }

            let position = pointer_position_in_element(&event);
            if event.pointer_type() == "touch" {
                let Some(touch_id) = this.touch_state.borrow_mut().end(event.pointer_id()) else {
                    return;
                };
                this.state.borrow_mut().mouse_position = position;
                let completes_tap = match this.touch_tap_candidate.get() {
                    Some((pointer_id, _)) if pointer_id == event.pointer_id() => {
                        this.touch_tap_candidate.set(None);
                        true
                    }
                    _ => false,
                };
                // A recognized tap is dispatched synchronously inside this
                // call, so the text-input check below sees the state the tap
                // produced.
                let dispatch_result = this.dispatch_input(PlatformInput::Touch(TouchEvent {
                    id: touch_id,
                    phase: TouchPhase::Ended,
                    position,
                    predicted_position: None,
                    force: None,
                }));
                this.canvas.release_pointer_capture(event.pointer_id()).ok();

                // A keyboard opening or closing mid-gesture reflows the
                // layout, so the release position no longer refers to the
                // content the user aimed at. Do not summon the keyboard
                // from that stale position, or from pans and flings.
                let viewport_stable = this.gesture_start_visual_viewport_height.get()
                    == this.visual_viewport_height();
                // A consumed tap may navigate to an editor under this same
                // coordinate. Do not reinterpret that navigation as editing.
                // Controls that intend typing can explicitly request a keyboard.
                if completes_tap
                    && viewport_stable
                    && !dispatch_result.default_prevented
                    && this.pointer_targets_text_input(position)
                {
                    this.show_virtual_keyboard();
                }
                return;
            }

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

    fn register_pointer_cancel(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen("pointercancel", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            event.prevent_default();
            if event.pointer_type() == "touch" {
                this.touch_state
                    .borrow_mut()
                    .moved(event.pointer_id(), pointer_position_in_element(&event));
                this.cancel_touch_pointer(event.pointer_id());
                return;
            }
            let (next_state, transition) = this
                .pointer_capture
                .get()
                .pointer_cancel(event.pointer_id());
            this.apply_pointer_transition(next_state, transition);
        })
    }

    fn register_lost_pointer_capture(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen("lostpointercapture", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            this.cancel_touch_pointer(event.pointer_id());
            let (next_state, transition) = this
                .pointer_capture
                .get()
                .pointer_capture_lost(event.pointer_id());
            this.apply_pointer_transition(next_state, transition);
        })
    }

    fn register_pointer_move(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen("pointermove", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            event.prevent_default();
            if this.interaction_gate.is_quiesced() {
                return;
            }
            let position = pointer_position_in_element(&event);
            if event.pointer_type() == "touch" {
                let Some(touch_id) = this
                    .touch_state
                    .borrow_mut()
                    .moved(event.pointer_id(), position)
                else {
                    return;
                };
                this.state.borrow_mut().mouse_position = position;
                // Mirror the slop rule of gpui's tap recognizer: once the
                // touch travels beyond it, its release must not affect the
                // keyboard. Only gpui knows what the gesture truly resolved
                // to; this platform-side shadow exists because the keyboard
                // decision must be made synchronously inside the browser's
                // pointerup handler.
                if let Some((pointer_id, start_position)) = this.touch_tap_candidate.get()
                    && pointer_id == event.pointer_id()
                    && (position - start_position).magnitude()
                        > f64::from(GestureTuning::default().touch_slop)
                {
                    this.touch_tap_candidate.set(None);
                }
                this.dispatch_input(PlatformInput::Touch(TouchEvent {
                    id: touch_id,
                    phase: TouchPhase::Moved,
                    position,
                    predicted_position: predicted_pointer_position(&event, position),
                    force: None,
                }));
                return;
            }

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

    fn register_pointer_leave(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen("pointerleave", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            if event.pointer_type() == "touch" {
                return;
            }

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

    fn register_wheel(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen_non_passive("wheel", move |event: JsValue| {
            let event: web_sys::WheelEvent = event.unchecked_into();
            event.prevent_default();
            if this.interaction_gate.is_quiesced() {
                return;
            }

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

            this.dispatch_input(PlatformInput::Scroll(ScrollEvent {
                position,
                delta,
                modifiers,
                touch_phase: TouchPhase::Moved,
                source: ScrollSource::Pointer,
            }));
        })
    }

    fn register_context_menu(self: &Rc<Self>) -> WebEventListener {
        self.listen("contextmenu", move |event: JsValue| {
            let event: web_sys::Event = event.unchecked_into();
            event.prevent_default();
        })
    }

    fn register_dragover(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen("dragover", move |event: JsValue| {
            let event: web_sys::DragEvent = event.unchecked_into();
            event.prevent_default();
            if this.interaction_gate.is_quiesced() {
                return;
            }

            let mouse_event: &web_sys::MouseEvent = event.as_ref();
            let position = mouse_position_in_element(mouse_event);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
            }

            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Pending { position }));
        })
    }

    fn register_drop(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen("drop", move |event: JsValue| {
            let event: web_sys::DragEvent = event.unchecked_into();
            event.prevent_default();
            if this.interaction_gate.is_quiesced() {
                return;
            }

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

    fn register_dragleave(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen("dragleave", move |_event: JsValue| {
            if this.interaction_gate.is_quiesced() {
                return;
            }
            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Exited));
        })
    }

    fn register_key_down(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen_input("keydown", move |event: JsValue| {
            let event: web_sys::KeyboardEvent = event.unchecked_into();
            if this.interaction_gate.is_quiesced() {
                return;
            }

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
            if this.interaction_gate.is_quiesced() {
                return;
            }

            let key = dom_key_to_gpui_key(&event);

            if is_modifier_only_key(&key) {
                return;
            }

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

            if !result.propagate {
                event.prevent_default();
                return;
            }

            if this.interaction_gate.is_quiesced() {
                return;
            }

            if this.is_composing.get() || event.is_composing() {
                event.prevent_default();
                return;
            }

            if modifiers.is_subset_of(&Modifiers::shift())
                && let Some(text) = key_char
                && this
                    .with_focused_input_handler(|handler| {
                        handler.replace_text_in_range(None, &text)
                    })
                    .is_some()
            {
                event.prevent_default();
            }
        })
    }

    fn register_key_up(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen_input("keyup", move |event: JsValue| {
            let event: web_sys::KeyboardEvent = event.unchecked_into();
            if this.interaction_gate.is_quiesced() {
                return;
            }

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
            if this.interaction_gate.is_quiesced() {
                return;
            }

            let key = dom_key_to_gpui_key(&event);

            if is_modifier_only_key(&key) {
                return;
            }

            let key_char = compute_key_char(&event, &key, &modifiers);

            let keystroke = Keystroke {
                modifiers,
                key,
                key_char,
            };

            let result = this.dispatch_input(PlatformInput::KeyUp(KeyUpEvent { keystroke }));
            if !result.propagate {
                event.prevent_default();
            }
        })
    }

    /// Paste is delivered through the DOM `paste` event rather than
    /// `Platform::read_from_clipboard`: the browser's asynchronous clipboard
    /// read API cannot fit that synchronous signature, while `ClipboardEvent`
    /// exposes `clipboardData` synchronously inside the event. It fires for
    /// any browser-initiated paste (keyboard, menu bar, context menu).
    ///
    /// Text-only pastes reach the input handler synchronously. Pasted image
    /// files only expose their bytes through asynchronous blob reads, so
    /// pastes containing images are delivered once those reads resolve — to
    /// whichever input handler is focused at that point, matching how an
    /// application-level paste action would behave.
    fn register_paste(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen_input("paste", move |event: JsValue| {
            let event: web_sys::ClipboardEvent = event.unchecked_into();
            if !this.input_is_focused() {
                return;
            }
            let Some(clipboard_data) = event.clipboard_data() else {
                return;
            };
            let text = clipboard_data
                .get_data("text/plain")
                .ok()
                .filter(|text| !text.is_empty());

            // File handles must be collected synchronously: the browser
            // clears `clipboardData`'s item list once this handler returns,
            // while the `File`s themselves stay readable afterwards.
            let mut image_files = Vec::new();
            let items = clipboard_data.items();
            for index in 0..items.length() {
                let Some(item) = items.get(index) else {
                    continue;
                };
                if item.kind() != "file" {
                    continue;
                }
                let Some(format) = ImageFormat::from_mime_type(&item.type_()) else {
                    continue;
                };
                if let Ok(Some(file)) = item.get_as_file() {
                    image_files.push((format, file));
                }
            }

            if text.is_none() && image_files.is_empty() {
                return;
            }

            if image_files.is_empty() {
                if let Some(text) = text
                    && this
                        .with_focused_input_handler(|handler| {
                            handler.paste(ClipboardItem::new_string(text));
                        })
                        .is_some()
                {
                    event.prevent_default();
                }
                return;
            }
            event.prevent_default();

            let this = Rc::downgrade(&this);
            wasm_bindgen_futures::spawn_local(async move {
                let mut entries = Vec::new();
                if let Some(text) = text {
                    entries.push(ClipboardEntry::String(ClipboardString::new(text)));
                }
                for (format, file) in image_files {
                    match crate::platform::read_blob_bytes(&file).await {
                        Ok(bytes) => {
                            entries.push(ClipboardEntry::Image(Image::from_bytes(format, bytes)));
                        }
                        Err(error) => {
                            log::error!(
                                "failed to read pasted image: {}",
                                crate::platform::js_error_message(&error)
                            );
                        }
                    }
                }
                if entries.is_empty() {
                    return;
                }
                if let Some(this) = this.upgrade() {
                    let _ = this.with_focused_input_handler(|handler| {
                        handler.paste(ClipboardItem { entries });
                    });
                }
            });
        })
    }

    fn register_composition_start(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen_input("compositionstart", move |_event: JsValue| {
            if this.interaction_gate.is_quiesced() {
                return;
            }
            this.is_composing.set(true);
        })
    }

    fn register_composition_update(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen_input("compositionupdate", move |event: JsValue| {
            let event: web_sys::CompositionEvent = event.unchecked_into();
            if this.interaction_gate.is_quiesced() {
                return;
            }
            let data = event.data().unwrap_or_default();
            this.is_composing.set(true);
            let _ = this.with_input_handler(|handler| {
                handler.replace_and_mark_text_in_range(None, &data, None)
            });
        })
    }

    fn register_composition_end(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen_input("compositionend", move |event: JsValue| {
            let event: web_sys::CompositionEvent = event.unchecked_into();
            if this.interaction_gate.is_quiesced() {
                this.is_composing.set(false);
                this.input_element.set_value("");
                return;
            }
            let data = event.data().unwrap_or_default();
            this.is_composing.set(false);
            let _ = this.with_input_handler(|handler| handler.replace_text_in_range(None, &data));
            let _ = this.with_input_handler(|handler| handler.unmark_text());
            this.input_element.set_value("");
        })
    }

    fn register_focus(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen_input("focus", move |_event: JsValue| this.sync_dom_activation())
    }

    fn register_blur(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen_input("blur", move |_event: JsValue| this.sync_dom_activation())
    }

    fn register_pointer_enter(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen("pointerenter", move |_event: JsValue| {
            if this.interaction_gate.is_quiesced() {
                return;
            }
            {
                let mut state = this.state.borrow_mut();
                state.is_hovered = true;
            }
            this.dispatch_hover_status_change(true);
        })
    }

    fn register_pointer_leave_hover(self: &Rc<Self>) -> WebEventListener {
        let this = Rc::clone(self);
        self.listen("pointerleave", move |_event: JsValue| {
            if this.interaction_gate.is_quiesced() {
                return;
            }
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
    interaction_gate: &'a WebInteractionGate,
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
        if let Some(reason) = transition.cancel_reason {
            let _ = self.interaction_gate.dispatch_input(
                self.callbacks,
                PlatformInput::PointerCanceled(PointerCancelEvent { reason }),
            );
        }
    }

    fn dispatch_pointer_button_change(
        &self,
        change: WebPointerButtonChange,
        event: WebPointerEventData,
    ) {
        if self.interaction_gate.is_quiesced() {
            return;
        }
        let click_count = match change {
            WebPointerButtonChange::Down(button) => self.click_state.borrow_mut().register_click(
                button,
                event.position,
                event.click_time,
            ),
            WebPointerButtonChange::Up(_) => self.click_state.borrow().current_count(),
        };
        let _ = self.interaction_gate.dispatch_input(
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
        if self.interaction_gate.is_quiesced() {
            return;
        }
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
        if self.interaction_gate.is_quiesced() {
            return;
        }
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
        if self.interaction_gate.is_quiesced() {
            return;
        }
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
        let _ = self.interaction_gate.dispatch_input(self.callbacks, input);
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
    matches!(
        platform_info(browser_window).os,
        open_gpui::OperatingSystem::Mac | open_gpui::OperatingSystem::Ios
    )
}

pub(crate) fn platform_info(browser_window: &web_sys::Window) -> open_gpui::PlatformInfo {
    let navigator = browser_window.navigator();
    let user_agent = navigator.user_agent().unwrap_or_default();
    #[allow(deprecated)]
    // navigator.platform() is deprecated but navigator.userAgentData is not widely available yet
    let platform = navigator.platform().unwrap_or_default();
    crate::platform_info::from_navigator_report(
        &user_agent,
        &platform,
        navigator.max_touch_points(),
    )
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

/// How far ahead of the raw pointer position predictions may reach.
///
/// Browsers predict much further (Chrome offers samples out to 25ms), but
/// prediction error grows with the horizon and surfaces as jitter: the
/// emitted pan deltas gain a term proportional to lead x change in velocity,
/// which at long leads visibly reverses direction mid-drag. Measurements
/// show leads up to ~10ms track at or below the raw stream's frame-to-frame
/// variation; AOSP similarly caps touch resampling extrapolation at 8ms
/// (`RESAMPLE_MAX_PREDICTION` in `InputTransport.cpp`).
const MAX_PREDICTION_LEAD_MS: f64 = 10.;

/// The predicted pointer position closest to [`MAX_PREDICTION_LEAD_MS`]
/// ahead of `event`, from `getPredictedEvents()`, or `None` when the browser
/// offers no prediction (Safari lacks the method, Firefox returns an empty
/// array). A prediction further out than the cap is linearly scaled back to
/// it.
///
/// Accessed through `Reflect` because calling a missing method through the
/// web-sys binding would throw, and predicted events' `offsetX`/`offsetY`
/// are unreliable across browsers (their target may be detached), so the
/// position is derived from the client-coordinate delta against the parent
/// event, anchored to the parent's element-relative `position`.
fn predicted_pointer_position(
    event: &web_sys::PointerEvent,
    position: Point<Pixels>,
) -> Option<Point<Pixels>> {
    let method = js_sys::Reflect::get(event, &JsValue::from_str("getPredictedEvents")).ok()?;
    let method = method.dyn_ref::<js_sys::Function>()?;
    let predicted_events: js_sys::Array = method.call0(event).ok()?.dyn_into().ok()?;
    let mut best: Option<(f64, web_sys::PointerEvent)> = None;
    for predicted in predicted_events.iter() {
        let Ok(predicted) = predicted.dyn_into::<web_sys::PointerEvent>() else {
            continue;
        };
        let lead = predicted.time_stamp() - event.time_stamp();
        if lead <= 0. {
            continue;
        }
        let distance_to_cap = (lead - MAX_PREDICTION_LEAD_MS).abs();
        if best
            .as_ref()
            .is_none_or(|(best_distance, _)| distance_to_cap < *best_distance)
        {
            best = Some((distance_to_cap, predicted));
        }
    }
    let (_, predicted) = best?;
    let lead = predicted.time_stamp() - event.time_stamp();
    let scale = (MAX_PREDICTION_LEAD_MS / lead).min(1.) as f32;
    let event: &web_sys::MouseEvent = event.as_ref();
    let predicted: &web_sys::MouseEvent = predicted.as_ref();
    Some(point(
        position.x + px((predicted.client_x() - event.client_x()) as f32 * scale),
        position.y + px((predicted.client_y() - event.client_y()) as f32 * scale),
    ))
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
        DispatchEventResult, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
        PlatformInput, PlatformInputCallback, Point, PointerCancelEvent, PointerCancelReason,
        TestAppContext,
    };

    use super::{
        ClickState, WebEventListener, WebInteractionGate, WebPointerButtonChange,
        WebPointerCaptureCommand, WebPointerCaptureState, WebPointerEventData,
        WebPointerInputBoundary, WebPointerMotionKind, pointer_button_change_to_platform_input,
    };
    use crate::window::WebWindowCallbacks;

    #[test]
    fn visibility_listener_is_unregistered_on_drop() {
        let target = web_sys::EventTarget::new().unwrap();
        let calls = Rc::new(Cell::new(0));
        let listener = WebEventListener::register(&target, "visibilitychange", {
            let calls = calls.clone();
            move |_| calls.set(calls.get() + 1)
        });
        let event = web_sys::Event::new("visibilitychange").unwrap();

        target.dispatch_event(&event).unwrap();
        assert_eq!(calls.get(), 1);

        drop(listener);
        target.dispatch_event(&event).unwrap();
        assert_eq!(calls.get(), 1);
    }

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
        assert_eq!(transition.cancel_reason, None);

        let (_, unchanged) = state.pointer_motion(7, -1, 0);
        assert!(unchanged.button_change.is_none());
    }

    #[test]
    fn lost_capture_with_held_buttons_cancels_once() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, transition) = state.pointer_capture_lost(7);
        assert_eq!(state, WebPointerCaptureState::default());
        assert_eq!(
            transition.cancel_reason,
            Some(PointerCancelReason::PlatformCaptureLost)
        );

        let (_, duplicate) = state.pointer_capture_lost(7);
        assert_eq!(duplicate.cancel_reason, None);
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
        assert_eq!(
            transition.cancel_reason,
            Some(PointerCancelReason::PlatformCaptureLost)
        );

        let (_, duplicate) = state.pointer_capture_lost(7);
        assert_eq!(duplicate.cancel_reason, None);
    }

    #[test]
    fn final_pointer_up_then_lost_capture_does_not_cancel() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, transition) = state.pointer_up(7, 0, 0);
        assert_eq!(transition.cancel_reason, None);

        let (_, lost) = state.pointer_capture_lost(7);
        assert_eq!(lost.cancel_reason, None);
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
        assert_eq!(
            transition.cancel_reason,
            Some(PointerCancelReason::PlatformCaptureLost)
        );
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

    #[open_gpui::test]
    fn pointer_listener_boundary_dispatches_chorded_buttons_through_registered_callback(
        cx: &mut TestAppContext,
    ) {
        #[derive(Debug, Eq, PartialEq)]
        enum ObservedInput {
            Down(MouseButton, usize),
            Up(MouseButton, usize),
            Moved(Option<MouseButton>),
        }

        let observed = Rc::new(RefCell::new(Vec::new()));
        let callbacks = RefCell::new(WebWindowCallbacks::default());
        let input_callback = callbacks.borrow().input.clone();
        input_callback.set(PlatformInputCallback::new(
            cx.to_async(),
            Box::new({
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
            }),
        ));

        let pointer_capture = Cell::new(WebPointerCaptureState::default());
        let click_state = RefCell::new(ClickState::default());
        let interaction_gate = WebInteractionGate::default();
        let boundary = WebPointerInputBoundary {
            pointer_capture: &pointer_capture,
            click_state: &click_state,
            callbacks: &callbacks,
            interaction_gate: &interaction_gate,
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
    fn interaction_gate_suppresses_ordinary_input_but_allows_terminal_pointer_cleanup() {
        #[derive(Debug, Eq, PartialEq)]
        enum ObservedInput {
            Moved,
            Canceled(PointerCancelReason),
        }

        let observed = Rc::new(RefCell::new(Vec::new()));
        let callbacks = RefCell::new(WebWindowCallbacks::default());
        callbacks
            .borrow()
            .input
            .clone()
            .set(PlatformInputCallback::new_unleased_for_test(Box::new({
                let observed = observed.clone();
                move |input| {
                    match input {
                        PlatformInput::MouseMove(_) => {
                            observed.borrow_mut().push(ObservedInput::Moved)
                        }
                        PlatformInput::PointerCanceled(event) => observed
                            .borrow_mut()
                            .push(ObservedInput::Canceled(event.reason)),
                        _ => {}
                    }
                    DispatchEventResult::default()
                }
            })));
        let interaction_gate = WebInteractionGate::default();
        let mouse_move = || {
            PlatformInput::MouseMove(MouseMoveEvent {
                position: Point::default(),
                pressed_button: None,
                modifiers: Modifiers::default(),
            })
        };

        interaction_gate.dispatch_input(&callbacks, mouse_move());
        interaction_gate.quiesce();
        interaction_gate.quiesce();
        interaction_gate.dispatch_input(&callbacks, mouse_move());
        interaction_gate.dispatch_input(
            &callbacks,
            PlatformInput::PointerCanceled(PointerCancelEvent {
                reason: PointerCancelReason::CaptureRevoked,
            }),
        );

        assert_eq!(
            observed.borrow().as_slice(),
            &[
                ObservedInput::Moved,
                ObservedInput::Canceled(PointerCancelReason::CaptureRevoked),
            ]
        );
    }

    #[test]
    fn pointer_boundary_rechecks_quiescence_after_a_reentrant_button_callback() {
        #[derive(Debug, Eq, PartialEq)]
        enum ObservedInput {
            Down(MouseButton),
            Moved,
            Canceled(PointerCancelReason),
        }

        let interaction_gate = Rc::new(WebInteractionGate::default());
        let observed = Rc::new(RefCell::new(Vec::new()));
        let callbacks = RefCell::new(WebWindowCallbacks::default());
        callbacks
            .borrow()
            .input
            .clone()
            .set(PlatformInputCallback::new_unleased_for_test(Box::new({
                let interaction_gate = interaction_gate.clone();
                let observed = observed.clone();
                move |input| {
                    match input {
                        PlatformInput::MouseDown(event) => {
                            observed
                                .borrow_mut()
                                .push(ObservedInput::Down(event.button));
                            if event.button == MouseButton::Right {
                                interaction_gate.quiesce();
                            }
                        }
                        PlatformInput::MouseMove(_) => {
                            observed.borrow_mut().push(ObservedInput::Moved)
                        }
                        PlatformInput::PointerCanceled(event) => observed
                            .borrow_mut()
                            .push(ObservedInput::Canceled(event.reason)),
                        _ => {}
                    }
                    DispatchEventResult::default()
                }
            })));

        let pointer_capture = Cell::new(WebPointerCaptureState::default());
        let click_state = RefCell::new(ClickState::default());
        let boundary = WebPointerInputBoundary {
            pointer_capture: &pointer_capture,
            click_state: &click_state,
            callbacks: &callbacks,
            interaction_gate: interaction_gate.as_ref(),
        };
        let event = |button, buttons, click_time| WebPointerEventData {
            pointer_id: 7,
            button,
            buttons,
            position: Point::default(),
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

        assert_eq!(
            observed.borrow().as_slice(),
            &[
                ObservedInput::Down(MouseButton::Left),
                ObservedInput::Down(MouseButton::Right),
            ],
            "the button callback may revoke authority before the same DOM event emits movement"
        );

        let (next_state, cleanup) = pointer_capture
            .get()
            .cleanup(PointerCancelReason::CaptureRevoked);
        boundary.apply_pointer_transition(next_state, cleanup, |command| {
            capture_commands.push(command)
        });
        boundary.handle_pointer_down(
            event(0, 1, 200.0),
            |command| capture_commands.push(command),
            |_, _| panic!("quiesced pointer input must not update window state"),
        );

        assert_eq!(
            observed.borrow().as_slice(),
            &[
                ObservedInput::Down(MouseButton::Left),
                ObservedInput::Down(MouseButton::Right),
                ObservedInput::Canceled(PointerCancelReason::CaptureRevoked),
            ]
        );
        assert_eq!(
            capture_commands,
            vec![
                WebPointerCaptureCommand::Set(7),
                WebPointerCaptureCommand::None,
                WebPointerCaptureCommand::Release(7),
            ]
        );
        assert_eq!(pointer_capture.get(), WebPointerCaptureState::default());
    }

    #[test]
    fn repeated_cleanup_cancels_only_the_active_session() {
        let (state, _) = WebPointerCaptureState::default().pointer_down(7, 0, 1);
        let (state, cleanup) = state.cleanup(PointerCancelReason::PlatformCaptureLost);
        assert_eq!(
            cleanup.capture_command,
            WebPointerCaptureCommand::Release(7)
        );
        assert_eq!(
            cleanup.cancel_reason,
            Some(PointerCancelReason::PlatformCaptureLost)
        );

        let (_, duplicate_cleanup) = state.cleanup(PointerCancelReason::PlatformCaptureLost);
        assert_eq!(duplicate_cleanup.cancel_reason, None);
    }
}
