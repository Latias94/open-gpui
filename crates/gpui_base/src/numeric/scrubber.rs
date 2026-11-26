use open_gpui::{
    AccessibleAction, App, Context, CursorStyle, Entity, EventEmitter, FocusHandle, Focusable,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Refineable, Role, SharedString,
    StyleRefinement, Subscription, TargetedEvent, Window, WindowId, accesskit::ActionData, div,
    prelude::*,
};

use super::{
    NUMBER_CONTEXT, NumberCommand, NumericEvent, NumericRange, PointerDrag, install_keys,
    on_pointer_cancel,
};
use crate::{TextInput, TextInputEvent};

const DRAG_THRESHOLD: f64 = 3.;

struct Scrub {
    original: f64,
    raw: f64,
    last_x: f64,
    moved: bool,
}

/// Drag horizontally to adjust the value. Double-click or press Enter to edit as text.
/// Shift makes dragging finer, and the platform's primary shortcut modifier snaps to a step.
pub struct NumericScrubber {
    range: NumericRange,
    value: f64,
    units_per_pixel: f64,
    focus: FocusHandle,
    drag: PointerDrag<Scrub>,
    input: Option<Entity<TextInput>>,
    input_subscription: Option<(WindowId, Subscription)>,
    text_entry: bool,
    editing: bool,
    label: SharedString,
    style: StyleRefinement,
}

impl NumericScrubber {
    pub fn new(value: f64, range: NumericRange, cx: &mut Context<Self>) -> Self {
        install_keys(cx);
        let value = range.normalize(value);
        Self {
            range,
            value,
            units_per_pixel: range.key_step(),
            focus: cx.focus_handle(),
            drag: PointerDrag::default(),
            input: None,
            input_subscription: None,
            text_entry: true,
            editing: false,
            label: "Value".into(),
            style: StyleRefinement::default(),
        }
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    /// Configures direct text entry. Dragging and keyboard stepping remain available.
    pub fn with_text_entry(mut self, enabled: bool) -> Self {
        self.text_entry = enabled;
        self
    }

    pub fn set_value(&mut self, value: f64, cx: &mut Context<Self>) {
        let value = self.range.normalize(value);
        if self.value != value {
            self.value = value;
            if let Some(input) = &self.input {
                input.update(cx, |input, cx| input.set_value(value.to_string(), cx));
            }
            cx.notify();
        }
    }

    pub fn set_units_per_pixel(&mut self, units: f64) -> bool {
        if !units.is_finite() || units <= 0. {
            return false;
        }
        self.units_per_pixel = units;
        true
    }

    pub fn set_label(&mut self, label: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.label = label.into();
        if let Some(input) = &self.input {
            input.update(cx, |input, cx| input.set_label(self.label.clone(), cx));
        }
        cx.notify();
    }

    pub fn set_style(&mut self, style: StyleRefinement, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }

    fn change(&mut self, value: f64, cx: &mut Context<Self>) {
        if self.value != value {
            self.value = value;
            cx.emit(NumericEvent::Change(value));
            cx.notify();
        }
    }

    fn commit(&mut self, value: f64, cx: &mut Context<Self>) {
        self.change(self.range.normalize(value), cx);
        cx.emit(NumericEvent::Commit(self.value));
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing || !self.text_entry {
            return;
        }
        self.editing = true;
        let input = self.input.get_or_insert_with(|| {
            cx.new(|cx| {
                let mut input = TextInput::new(self.value.to_string(), cx);
                input.set_label(self.label.clone(), cx);
                input
            })
        });
        let handle = input.update(cx, |input, cx| {
            input.set_value_in(self.value.to_string(), window, cx);
            input.set_invalid(false, cx);
            input.select_all(window, cx);
            input.focus_handle(cx)
        });
        cx.notify();
        window.focus(&handle, cx);
        window.request_virtual_keyboard();
    }

    fn from_text(
        &mut self,
        input: &Entity<TextInput>,
        event: &TextInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editing {
            return;
        }
        let (value, restore_focus) = match event {
            TextInputEvent::Change(_) => {
                input.update(cx, |input, cx| input.set_invalid(false, cx));
                return;
            }
            TextInputEvent::Submit(value) => {
                let Some(value) = parse_value(value) else {
                    input.update(cx, |input, cx| input.set_invalid(true, cx));
                    return;
                };
                (Some(value), true)
            }
            TextInputEvent::Blur => (parse_value(input.read(cx).value()), false),
            TextInputEvent::Cancel => (None, true),
        };
        self.editing = false;
        if let Some(value) = value {
            self.commit(value, cx);
        } else {
            cx.emit(NumericEvent::Cancel(self.value));
        }
        cx.notify();
        if restore_focus {
            window.focus(&self.focus, cx);
        }
    }

    fn pointer_down(
        &mut self,
        event: &TargetedEvent<MouseDownEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(x) = mouse_x(
            event
                .target_local_position()
                .ok()
                .map(|point| point.x.to_f64()),
        ) else {
            return;
        };
        let scrub = Scrub {
            original: self.value,
            raw: self.value,
            last_x: x,
            moved: false,
        };
        if self.drag.begin(scrub, window) {
            window.focus(&self.focus, cx);
            cx.stop_propagation();
        }
    }

    fn pointer_move(
        &mut self,
        event: &TargetedEvent<MouseMoveEvent>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(x) = mouse_x(
            event
                .target_local_position()
                .ok()
                .map(|point| point.x.to_f64()),
        ) else {
            return;
        };
        let modifiers = event.window_event().modifiers;
        self.move_to(x, modifiers.shift, modifiers.secondary(cx), cx);
    }

    fn pointer_up(
        &mut self,
        event: &TargetedEvent<MouseUpEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mouse = event.window_event();
        if let Some(x) = mouse_x(
            event
                .target_local_position()
                .ok()
                .map(|point| point.x.to_f64()),
        ) {
            self.move_to(x, mouse.modifiers.shift, mouse.modifiers.secondary(cx), cx);
        }
        if let Some(drag) = self.drag.take(window) {
            if drag.moved {
                cx.emit(NumericEvent::Commit(self.value));
            } else if mouse.click_count >= 2 {
                self.edit(window, cx);
            }
        }
    }

    fn move_to(&mut self, x: f64, fine: bool, snap: bool, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.state.as_mut() else {
            return;
        };
        let delta = x - drag.last_x;
        if !drag.moved && delta.abs() < DRAG_THRESHOLD {
            return;
        }
        drag.moved = true;
        drag.last_x = x;
        let units = self.units_per_pixel * if fine { 0.1 } else { 1. };
        drag.raw = self.range.clamp(drag.raw + delta * units);
        let value = if snap {
            self.range.snap(drag.raw, self.range.key_step())
        } else if fine {
            drag.raw
        } else {
            self.range.normalize(drag.raw)
        };
        self.change(value, cx);
    }

    fn cancel_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(drag) = self.drag.take(window) {
            self.value = drag.original;
            cx.emit(NumericEvent::Cancel(self.value));
            cx.notify();
        }
    }

    fn command(&mut self, command: &NumberCommand, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing {
            cx.propagate();
            return;
        }
        match command {
            NumberCommand::Cancel if self.drag.state.is_some() => self.cancel_drag(window, cx),
            NumberCommand::Cancel => cx.propagate(),
            NumberCommand::Edit if !self.text_entry => cx.propagate(),
            NumberCommand::Edit if self.drag.state.is_none() => self.edit(window, cx),
            command if self.drag.state.is_none() => {
                if let Some(value) = command.adjust(self.range, self.value) {
                    self.commit(value, cx);
                }
            }
            _ => {}
        }
    }
}

impl Focusable for NumericScrubber {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<NumericEvent> for NumericScrubber {}

impl Render for NumericScrubber {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div().relative().min_w_0();
        root.style().refine(&self.style);
        if self.editing
            && let Some(input) = &self.input
        {
            let window_id = window.window_handle().window_id();
            if self
                .input_subscription
                .as_ref()
                .is_none_or(|(id, _)| *id != window_id)
            {
                self.input_subscription =
                    Some((window_id, cx.subscribe_in(input, window, Self::from_text)));
            }
            return root.child(input.clone()).into_any_element();
        }
        let capture = self.drag.bind(window);
        let view = cx.weak_entity();
        let mut root = root
            .id("numeric-scrubber")
            .role(Role::SpinButton)
            .aria_label(self.label.clone())
            .aria_numeric_value(self.value)
            .aria_min_numeric_value(self.range.min)
            .aria_max_numeric_value(self.range.max)
            .aria_numeric_value_step(self.range.key_step())
            .cursor(CursorStyle::ResizeLeftRight)
            .track_focus(&self.focus)
            .tab_stop(true)
            .track_pointer_capture(&capture)
            .key_context(NUMBER_CONTEXT)
            .on_action(cx.listener(Self::command))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::pointer_down))
            .on_mouse_move(cx.listener(Self::pointer_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::pointer_up))
            .on_a11y_action(AccessibleAction::SetValue, move |data, _, cx| {
                if let Some(ActionData::NumericValue(value)) = data {
                    let _ = view.update(cx, |scrubber, cx| scrubber.commit(*value, cx));
                }
            });
        if self.text_entry {
            let view = cx.weak_entity();
            root = root.on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                let _ = view.update(cx, |scrubber, cx| {
                    scrubber.command(&NumberCommand::Edit, window, cx)
                });
            });
        }
        for (action, command) in [
            (AccessibleAction::Increment, NumberCommand::Step(1.)),
            (AccessibleAction::Decrement, NumberCommand::Step(-1.)),
        ] {
            let view = cx.weak_entity();
            root = root.on_a11y_action(action, move |_, window, cx| {
                let _ = view.update(cx, |scrubber, cx| scrubber.command(&command, window, cx));
            });
        }
        root.child(self.value.to_string())
            .child(on_pointer_cancel(cx, Self::cancel_drag))
            .into_any_element()
    }
}

fn mouse_x(value: Option<f64>) -> Option<f64> {
    value.filter(|x| x.is_finite())
}

fn parse_value(text: &str) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_accepts_finite_exponents_and_rejects_incomplete_or_nonfinite_values() {
        assert_eq!(parse_value(" -2.5e2 "), Some(-250.));
        assert_eq!(parse_value("-"), None);
        assert_eq!(parse_value("1e999"), None);
        assert_eq!(parse_value("NaN"), None);
    }
}
