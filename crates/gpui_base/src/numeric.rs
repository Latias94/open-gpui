use open_gpui::{
    AccessibleAction, App, Context, EventEmitter, FocusHandle, Focusable, Global, KeyBinding,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Refineable, Role, SharedString,
    StyleRefinement, TargetedEvent, Window, accesskit::ActionData, div, prelude::*, px, relative,
};

use crate::pointer::{PointerDrag, on_pointer_cancel};

mod scrubber;

pub use scrubber::NumericScrubber;

/// Finite numeric bounds with an optional step measured from the minimum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericRange {
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) step: Option<f64>,
}

impl NumericRange {
    pub fn new(min: f64, max: f64) -> Option<Self> {
        (min < max && (max - min).is_finite()).then_some(Self {
            min,
            max,
            step: None,
        })
    }

    pub fn with_step(mut self, step: f64) -> Option<Self> {
        if !step.is_finite() || step <= 0. {
            return None;
        }
        self.step = Some(step);
        Some(self)
    }

    pub fn min(self) -> f64 {
        self.min
    }
    pub fn max(self) -> f64 {
        self.max
    }
    pub fn step(self) -> Option<f64> {
        self.step
    }
    pub fn clamp(self, value: f64) -> f64 {
        if value.is_nan() {
            self.min
        } else {
            value.clamp(self.min, self.max)
        }
    }

    pub fn normalize(self, value: f64) -> f64 {
        let value = self.clamp(value);
        match self.step {
            Some(step) if value > self.min && value < self.max => self.snap(value, step),
            _ => value,
        }
    }

    pub fn fraction(self, value: f64) -> f64 {
        (self.clamp(value) - self.min) / self.span()
    }

    pub fn value_at(self, fraction: f64) -> f64 {
        if fraction >= 1. {
            self.max
        } else {
            self.normalize(self.min + self.span() * fraction.clamp(0., 1.))
        }
    }

    fn snap(self, value: f64, step: f64) -> f64 {
        let remainder = (value - self.min) % step;
        let rounded = if remainder >= step / 2. {
            value + (step - remainder)
        } else {
            value - remainder
        };
        self.clamp(rounded)
    }

    fn span(self) -> f64 {
        self.max - self.min
    }
    fn key_step(self) -> f64 {
        self.step
            .unwrap_or((self.span() / 100.).max(f64::from_bits(1)))
    }
}

/// Numeric changes, terminal commits, and gesture cancellation with the restored value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NumericEvent {
    Change(f64),
    Commit(f64),
    Cancel(f64),
}

/// A horizontal slider with pointer capture, keyboard stepping, and accessibility actions.
pub struct Slider {
    value: f64,
    range: NumericRange,
    focus: FocusHandle,
    drag: PointerDrag<f64>,
    label: SharedString,
    style: StyleRefinement,
    fill_style: StyleRefinement,
}

impl Slider {
    pub fn new(value: f64, range: NumericRange, cx: &mut Context<Self>) -> Self {
        install_keys(cx);
        Self {
            value: range.normalize(value),
            range,
            focus: cx.focus_handle(),
            drag: PointerDrag::default(),
            label: "Value".into(),
            style: StyleRefinement::default(),
            fill_style: StyleRefinement::default(),
        }
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn set_value(&mut self, value: f64, cx: &mut Context<Self>) {
        let value = self.range.normalize(value);
        if self.value != value {
            self.value = value;
            cx.notify();
        }
    }

    pub fn set_label(&mut self, label: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.label = label.into();
        cx.notify();
    }

    pub fn set_style(&mut self, style: StyleRefinement, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }

    pub fn set_fill_style(&mut self, style: StyleRefinement, cx: &mut Context<Self>) {
        self.fill_style = style;
        cx.notify();
    }

    fn change(&mut self, value: f64, cx: &mut Context<Self>) {
        if value != self.value {
            self.value = value;
            cx.emit(NumericEvent::Change(value));
            cx.notify();
        }
    }

    fn commit_value(&mut self, value: f64, cx: &mut Context<Self>) {
        self.change(self.range.normalize(value), cx);
        cx.emit(NumericEvent::Commit(self.value));
    }

    fn update_pointer(&mut self, position: f64, width: f64, cx: &mut Context<Self>) {
        if width > 0. {
            self.change(self.range.value_at(position / width), cx);
        }
    }

    fn pointer_down(
        &mut self,
        event: &TargetedEvent<MouseDownEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Ok(position) = event.target_local_position() else {
            return;
        };
        if self.drag.begin(self.value, window) {
            window.focus(&self.focus, cx);
            self.update_pointer(
                position.x.to_f64(),
                event.target_local_bounds().size.width.to_f64(),
                cx,
            );
            cx.stop_propagation();
        }
    }

    fn pointer_move(
        &mut self,
        event: &TargetedEvent<MouseMoveEvent>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.drag.state.is_some()
            && let Ok(position) = event.target_local_position()
        {
            self.update_pointer(
                position.x.to_f64(),
                event.target_local_bounds().size.width.to_f64(),
                cx,
            );
        }
    }

    fn pointer_up(
        &mut self,
        event: &TargetedEvent<MouseUpEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.drag.take(window).is_some() {
            if let Ok(position) = event.target_local_position() {
                self.update_pointer(
                    position.x.to_f64(),
                    event.target_local_bounds().size.width.to_f64(),
                    cx,
                );
            }
            cx.emit(NumericEvent::Commit(self.value));
        }
    }

    fn cancel_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(value) = self.drag.take(window) {
            self.value = value;
            cx.emit(NumericEvent::Cancel(value));
            cx.notify();
        }
    }

    fn command(&mut self, command: &NumberCommand, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            NumberCommand::Cancel if self.drag.state.is_some() => self.cancel_drag(window, cx),
            NumberCommand::Cancel => cx.propagate(),
            command if self.drag.state.is_none() => {
                if let Some(value) = command.adjust(self.range, self.value) {
                    self.commit_value(value, cx);
                } else {
                    cx.propagate();
                }
            }
            _ => {}
        }
    }
}

impl Focusable for Slider {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<NumericEvent> for Slider {}

impl Render for Slider {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let capture = self.drag.bind(window);
        let mut root = div().relative().w_full().h(px(16.));
        root.style().refine(&self.style);
        let mut fill = div()
            .absolute()
            .left_0()
            .top_0()
            .h_full()
            .w(relative(self.range.fraction(self.value) as f32))
            .bg(window.text_style().color.opacity(0.3));
        fill.style().refine(&self.fill_style);
        let view = cx.weak_entity();
        let mut root = root
            .id("slider")
            .role(Role::Slider)
            .aria_label(self.label.clone())
            .aria_numeric_value(self.value)
            .aria_min_numeric_value(self.range.min)
            .aria_max_numeric_value(self.range.max)
            .aria_numeric_value_step(self.range.key_step())
            .track_focus(&self.focus)
            .tab_stop(true)
            .track_pointer_capture(&capture)
            .cursor_pointer()
            .key_context(NUMBER_CONTEXT)
            .on_action(cx.listener(Self::command))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::pointer_down))
            .on_mouse_move(cx.listener(Self::pointer_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::pointer_up))
            .on_a11y_action(AccessibleAction::SetValue, move |data, _, cx| {
                if let Some(ActionData::NumericValue(value)) = data {
                    let _ = view.update(cx, |slider, cx| slider.commit_value(*value, cx));
                }
            });
        for (action, command) in [
            (AccessibleAction::Increment, NumberCommand::Step(1.)),
            (AccessibleAction::Decrement, NumberCommand::Step(-1.)),
        ] {
            let view = cx.weak_entity();
            root = root.on_a11y_action(action, move |_, window, cx| {
                let _ = view.update(cx, |slider, cx| slider.command(&command, window, cx));
            });
        }
        root.child(fill)
            .child(on_pointer_cancel(cx, Self::cancel_drag))
    }
}

pub(crate) const NUMBER_CONTEXT: &str = "OpenGpuiBaseNumber";

#[derive(Clone, Default, PartialEq, open_gpui::Action)]
#[action(namespace = open_gpui_base, no_json, no_register)]
pub(crate) enum NumberCommand {
    Step(f64),
    Edge(bool),
    #[default]
    Edit,
    Cancel,
}

impl NumberCommand {
    fn adjust(&self, range: NumericRange, value: f64) -> Option<f64> {
        match self {
            Self::Step(direction) => Some(value + direction * range.key_step()),
            Self::Edge(end) => Some(if *end { range.max } else { range.min }),
            _ => None,
        }
    }
}

struct NumericKeysInstalled;
impl Global for NumericKeysInstalled {}

pub(crate) fn install_keys(cx: &mut App) {
    if cx.has_global::<NumericKeysInstalled>() {
        return;
    }
    cx.set_global(NumericKeysInstalled);
    cx.bind_keys(
        [
            ("left", NumberCommand::Step(-1.)),
            ("down", NumberCommand::Step(-1.)),
            ("right", NumberCommand::Step(1.)),
            ("up", NumberCommand::Step(1.)),
            ("home", NumberCommand::Edge(false)),
            ("end", NumberCommand::Edge(true)),
            ("enter", NumberCommand::Edit),
            ("space", NumberCommand::Edit),
            ("escape", NumberCommand::Cancel),
        ]
        .map(|(key, command)| KeyBinding::new(key, command, Some(NUMBER_CONTEXT))),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_keeps_both_endpoints_and_never_produces_non_finite_values() {
        assert!(NumericRange::new(f64::NAN, 1.).is_none());
        assert!(NumericRange::new(-f64::MAX, f64::MAX).is_none());
        let range = NumericRange::new(2., 10.)
            .and_then(|range| range.with_step(3.))
            .expect("valid range");
        assert_eq!(range.value_at(0.), 2.);
        assert_eq!(range.value_at(1.), 10.);
        assert_eq!(range.normalize(6.6), 8.);
        assert_eq!(range.normalize(f64::NAN), 2.);
    }
}
