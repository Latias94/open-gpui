use open_gpui::{
    AccessibleAction, App, Context, EventEmitter, FocusHandle, Focusable, Global, KeyBinding,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Refineable, Role, SharedString,
    StyleRefinement, TargetedEvent, Window, accesskit::ActionData, div, prelude::*, px, relative,
};

use crate::pointer::{PointerDrag, on_pointer_cancel};

mod scrubber;

pub use scrubber::NumericScrubber;

/// Finite numeric bounds with an optional step grid. Both endpoints remain valid values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericRange {
    min: f64,
    max: f64,
    step: Option<f64>,
    step_origin: f64,
}

impl NumericRange {
    pub fn new(min: f64, max: f64) -> Option<Self> {
        (min < max && (max - min).is_finite()).then_some(Self {
            min,
            max,
            step: None,
            step_origin: min,
        })
    }

    /// Sets a positive finite step. Values halfway between steps round toward the greater value.
    pub fn with_step(mut self, step: f64) -> Option<Self> {
        if !step.is_finite() || step <= 0. {
            return None;
        }
        self.step = Some(step);
        Some(self)
    }

    /// Sets the finite origin of the step grid. It defaults to the minimum and may lie outside
    /// the bounds. Use a meaningful origin such as zero when bounds only limit the value.
    ///
    /// ```
    /// use open_gpui_base::NumericRange;
    ///
    /// let range = NumericRange::new(-1e20, 1e20)
    ///     .unwrap()
    ///     .with_step(0.25)
    ///     .unwrap()
    ///     .with_step_origin(0.0)
    ///     .unwrap();
    /// assert_eq!(range.normalize(0.4), 0.5);
    /// ```
    pub fn with_step_origin(mut self, origin: f64) -> Option<Self> {
        if !origin.is_finite() {
            return None;
        }
        self.step_origin = origin;
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
        let value_phase = nearest_step_offset(value % step, step);
        let origin_phase = nearest_step_offset(self.step_origin % step, step);
        self.clamp(value - nearest_step_offset(value_phase - origin_phase, step))
    }

    fn span(self) -> f64 {
        self.max - self.min
    }
    fn key_step(self) -> f64 {
        self.step
            .unwrap_or((self.span() / 100.).max(f64::from_bits(1)))
    }
}

fn nearest_step_offset(offset: f64, step: f64) -> f64 {
    let complement = step - offset.abs();
    if offset >= complement {
        -complement
    } else if -offset > complement {
        complement
    } else {
        offset
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
        assert_eq!(range.normalize(3.5), 5.);
        assert_eq!(range.normalize(f64::NAN), 2.);
    }

    #[test]
    fn wide_bounds_keep_step_resolution_and_grid_phase() {
        let limit = f64::MAX / 4.;
        let range = NumericRange::new(-limit, limit)
            .and_then(|range| range.with_step(0.125))
            .expect("finite range with a fractional step");
        assert_eq!(range.normalize(0.3), 0.25);
        assert_eq!(range.normalize(-0.1875), -0.125);
        assert_eq!(range.normalize(-limit), -limit);

        let offset_range = NumericRange::new(-1e20, 1e20)
            .and_then(|range| range.with_step(3.))
            .expect("finite range with an offset step grid");
        assert_eq!(offset_range.normalize(0.), -1.);
        assert_eq!(offset_range.normalize(0.5), 2.);
    }

    #[test]
    fn explicit_step_origin_can_be_outside_the_bounds() {
        let range = NumericRange::new(0.03125, f64::MAX)
            .and_then(|range| range.with_step(0.125))
            .expect("finite range with a fractional step");
        assert_eq!(range.normalize(0.22), 0.28125);
        let zero_based = range.with_step_origin(0.).expect("finite origin");
        assert_eq!(zero_based.normalize(0.22), 0.25);
        assert_eq!(zero_based.normalize(0.03125), 0.03125);
        assert!(range.with_step_origin(f64::INFINITY).is_none());
    }
}
