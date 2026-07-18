//! Slider component.

use crate::a11y::UiA11yElementExt;
use crate::color::{ColorIntent, ColorState};
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    App, ElementId, IntoElement, KeyDownEvent, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, relative,
};
use open_gpui_ui_core::{
    AccessibleAction, Orientation, Role, SemanticDescriptor, Sizable, Size, ThemeTokens, UiPx,
    ui_px,
};
use std::rc::Rc;

/// Resolved slider color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SliderColors {
    track: ColorIntent,
    range: ColorIntent,
    thumb: ColorIntent,
    thumb_border: ColorIntent,
    label: ColorIntent,
    focus_ring: ColorIntent,
}

impl SliderColors {
    /// Resolves slider colors from the shared token bundle.
    pub const fn from_tokens(tokens: ThemeTokens) -> Self {
        Self {
            track: ColorIntent::new(tokens.surface_muted, 0xdfe6dc),
            range: ColorIntent::new(tokens.accent, 0x1f7a66),
            thumb: ColorIntent::new(tokens.surface, 0xffffff),
            thumb_border: ColorIntent::new(tokens.accent, 0x1f7a66),
            label: ColorIntent::new(tokens.text, 0x18202a),
            focus_ring: ColorIntent::with_state(
                tokens.focus_ring,
                ColorState::FocusVisible,
                0x2f80ed,
            ),
        }
    }

    /// Returns the track color intent.
    pub const fn track(self) -> ColorIntent {
        self.track
    }

    /// Returns the filled range color intent.
    pub const fn range(self) -> ColorIntent {
        self.range
    }

    /// Returns the thumb fill color intent.
    pub const fn thumb(self) -> ColorIntent {
        self.thumb
    }

    /// Returns the thumb border color intent.
    pub const fn thumb_border(self) -> ColorIntent {
        self.thumb_border
    }

    /// Returns the label color intent.
    pub const fn label(self) -> ColorIntent {
        self.label
    }

    /// Returns the focus ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved slider metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderMetrics {
    track_height: UiPx,
    thumb_size: UiPx,
    min_width: UiPx,
    label_text_size: UiPx,
}

impl SliderMetrics {
    /// Resolves slider metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            track_height: match size {
                Size::XSmall => ui_px(4.0),
                Size::Small => ui_px(5.0),
                Size::Medium => ui_px(6.0),
                Size::Large => ui_px(7.0),
            },
            thumb_size: match size {
                Size::XSmall => ui_px(14.0),
                Size::Small => ui_px(16.0),
                Size::Medium => ui_px(18.0),
                Size::Large => ui_px(20.0),
            },
            min_width: match size {
                Size::XSmall => ui_px(120.0),
                Size::Small => ui_px(144.0),
                Size::Medium => ui_px(168.0),
                Size::Large => ui_px(192.0),
            },
            label_text_size: size.control_text_px(),
        }
    }

    /// Returns the track height.
    pub const fn track_height(self) -> UiPx {
        self.track_height
    }

    /// Returns the thumb size.
    pub const fn thumb_size(self) -> UiPx {
        self.thumb_size
    }

    /// Returns the recommended minimum width.
    pub const fn min_width(self) -> UiPx {
        self.min_width
    }

    /// Returns the label text size.
    pub const fn label_text_size(self) -> UiPx {
        self.label_text_size
    }
}

/// Slider value-change payload.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderChange {
    previous_value: f32,
    value: f32,
}

impl SliderChange {
    /// Creates a slider change payload.
    pub fn new(previous_value: f32, value: f32) -> Self {
        Self {
            previous_value,
            value,
        }
    }

    /// Returns the previous normalized value.
    pub const fn previous_value(self) -> f32 {
        self.previous_value
    }

    /// Returns the next normalized value.
    pub const fn value(self) -> f32 {
        self.value
    }

    /// Returns whether this change mutates the value.
    pub fn changed(self) -> bool {
        (self.previous_value - self.value).abs() > f32::EPSILON
    }
}

/// Resolved slider state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct SliderState {
    label: String,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    disabled: bool,
    size: Size,
    metrics: SliderMetrics,
    colors: SliderColors,
    focus_ring: FocusRing,
}

impl SliderState {
    /// Resolves the public slider state.
    pub fn resolve(
        label: impl Into<String>,
        value: f32,
        min: f32,
        max: f32,
        step: f32,
        disabled: bool,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        let (min, max) = normalize_bounds(min, max);
        let step = normalize_step(step);
        let value = normalize_numeric_value(value, min, max, step);
        let colors = SliderColors::from_tokens(tokens);

        Self {
            label: label.into(),
            value,
            min,
            max,
            step,
            disabled,
            size,
            metrics: SliderMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns the visible and accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the clamped and snapped value.
    pub const fn value(&self) -> f32 {
        self.value
    }

    /// Returns the minimum value.
    pub const fn min(&self) -> f32 {
        self.min
    }

    /// Returns the maximum value.
    pub const fn max(&self) -> f32 {
        self.max
    }

    /// Returns the positive step value.
    pub const fn step(&self) -> f32 {
        self.step
    }

    /// Returns whether the slider is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the normalized value fraction in the `0..=1` range.
    pub fn normalized_value(&self) -> f32 {
        let span = self.max - self.min;
        if span <= f32::EPSILON {
            0.0
        } else {
            ((self.value - self.min) / span).clamp(0.0, 1.0)
        }
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::Slider
    }

    /// Returns the keyboard step change for a key.
    pub fn keyboard_change(&self, key: &str) -> Option<SliderChange> {
        if self.disabled {
            return None;
        }

        let next = match key {
            "left" | "down" => self.value - self.step,
            "right" | "up" => self.value + self.step,
            "home" => self.min,
            "end" => self.max,
            "pagedown" => self.value - self.step * 10.0,
            "pageup" => self.value + self.step * 10.0,
            _ => return None,
        };

        Some(SliderChange::new(
            self.value,
            normalize_numeric_value(next, self.min, self.max, self.step),
        ))
    }

    fn set_value_change(&self, requested_value: f64) -> Option<SliderChange> {
        if !self.activation_enabled() {
            return None;
        }

        let requested_value =
            normalize_requested_numeric_value(requested_value, self.min, self.max, self.step)?;
        Some(SliderChange::new(self.value, requested_value))
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> SliderMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> SliderColors {
        self.colors
    }

    /// Returns resolved focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }
}

/// A concrete GPUI slider component.
#[derive(IntoElement)]
pub struct Slider {
    id: ElementId,
    label: SharedString,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    disabled: bool,
    size: Size,
    tokens: ThemeTokens,
    on_change: Option<Rc<dyn Fn(SliderChange, &mut Window, &mut App)>>,
}

impl Slider {
    /// Creates a new slider.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: 0.0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            disabled: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            on_change: None,
        }
    }

    /// Sets the controlled value.
    pub fn value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }

    /// Sets the minimum value.
    pub fn min(mut self, min: f32) -> Self {
        self.min = min;
        self
    }

    /// Sets the maximum value.
    pub fn max(mut self, max: f32) -> Self {
        self.max = max;
        self
    }

    /// Sets the positive step value.
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Marks the slider as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a value-change handler.
    pub fn on_change(
        mut self,
        handler: impl Fn(SliderChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved slider state.
    pub fn state(&self) -> SliderState {
        SliderState::resolve(
            self.label.to_string(),
            self.value,
            self.min,
            self.max,
            self.step,
            self.disabled,
            self.size,
            self.tokens,
        )
    }
}

impl Sizable for Slider {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let state = Rc::new(self.state());
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
        let disabled = state.disabled();
        let label = self.label.clone();
        let value_fraction = state.normalized_value();
        let debug_id = self.id.to_string();
        let actions: &[AccessibleAction] = if self.on_change.is_some() {
            &[
                AccessibleAction::Focus,
                AccessibleAction::Increment,
                AccessibleAction::Decrement,
                AccessibleAction::SetValue,
            ]
        } else {
            &[AccessibleAction::Focus]
        };
        let semantics = SemanticDescriptor::new(state.role())
            .with_label(state.label())
            .with_numeric_value(state.value() as f64)
            .with_min_numeric_value(state.min() as f64)
            .with_max_numeric_value(state.max() as f64)
            .with_orientation(Orientation::Horizontal)
            .with_disabled(disabled)
            .with_actions(actions);

        div()
            .id(self.id)
            .debug_selector(move || format!("slider:{debug_id}:root"))
            .min_w(gpui_px_from_ui(metrics.min_width()))
            .flex()
            .flex_col()
            .gap_2()
            .text_color(theme.resolve(colors.label()))
            .text_size(gpui_px_from_ui(metrics.label_text_size()))
            .focusable()
            .tab_stop(!disabled)
            .ui_semantics(&semantics)
            .focus_visible(move |style| style.shadow(focus_shadow.clone()))
            .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
            .when(!disabled, |this| this.cursor_pointer())
            .when_some(self.on_change.filter(|_| !disabled), |this, on_change| {
                let key_state = Rc::clone(&state);
                let key_on_change = on_change.clone();
                this.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    let Some(change) = key_state.keyboard_change(event.keystroke.key.as_str())
                    else {
                        return;
                    };
                    if change.changed() {
                        key_on_change(change, window, cx);
                    }
                    cx.stop_propagation();
                })
                .on_ui_a11y_action(AccessibleAction::Increment, {
                    let action_state = Rc::clone(&state);
                    let on_change = on_change.clone();
                    move |_, window, cx| {
                        if let Some(change) = action_state.keyboard_change("up")
                            && change.changed()
                        {
                            on_change(change, window, cx);
                        }
                    }
                })
                .on_ui_a11y_action(AccessibleAction::Decrement, {
                    let action_state = Rc::clone(&state);
                    let on_change = on_change.clone();
                    move |_, window, cx| {
                        if let Some(change) = action_state.keyboard_change("down")
                            && change.changed()
                        {
                            on_change(change, window, cx);
                        }
                    }
                })
                .on_ui_a11y_action(AccessibleAction::SetValue, {
                    let action_state = Rc::clone(&state);
                    move |data, window, cx| {
                        let Some(open_gpui::accesskit::ActionData::NumericValue(value)) = data
                        else {
                            return;
                        };
                        if let Some(change) = action_state.set_value_change(*value)
                            && change.changed()
                        {
                            on_change(change, window, cx);
                        }
                    }
                })
            })
            .child(label)
            .child(
                div()
                    .relative()
                    .h(gpui_px_from_ui(metrics.thumb_size()))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .h(gpui_px_from_ui(metrics.track_height()))
                            .w_full()
                            .rounded(gpui_px_from_ui(metrics.track_height()))
                            .overflow_hidden()
                            .bg(theme.resolve(colors.track()))
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(value_fraction))
                                    .bg(theme.resolve(colors.range())),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(relative(value_fraction))
                            .top(px(0.0))
                            .size(gpui_px_from_ui(metrics.thumb_size()))
                            .rounded(gpui_px_from_ui(metrics.thumb_size()))
                            .border_1()
                            .border_color(theme.resolve(colors.thumb_border()))
                            .bg(theme.resolve(colors.thumb())),
                    ),
            )
    }
}

pub(crate) fn normalize_bounds(min: f32, max: f32) -> (f32, f32) {
    let min = if min.is_finite() { min } else { 0.0 };
    let max = if max.is_finite() { max } else { min + 100.0 };
    if min <= max { (min, max) } else { (max, min) }
}

pub(crate) fn normalize_step(step: f32) -> f32 {
    if step.is_finite() && step > f32::EPSILON {
        step
    } else {
        1.0
    }
}

pub(crate) fn normalize_numeric_value(value: f32, min: f32, max: f32, step: f32) -> f32 {
    let value = if value.is_finite() { value } else { min };
    let clamped = value.clamp(min, max);
    let steps = ((clamped - min) / step).round();
    (min + steps * step).clamp(min, max)
}

pub(crate) fn normalize_requested_numeric_value(
    value: f64,
    min: f32,
    max: f32,
    step: f32,
) -> Option<f32> {
    if !value.is_finite() {
        return None;
    }

    let value = value.clamp(min as f64, max as f64) as f32;
    Some(normalize_numeric_value(value, min, max, step))
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::semantic;

    #[test]
    fn slider_clamps_and_snaps_value() {
        let state = Slider::new("volume", "Volume")
            .min(10.0)
            .max(20.0)
            .step(2.5)
            .value(16.2)
            .state();

        assert_eq!(state.role(), Role::Slider);
        assert_eq!(state.value(), 15.0);
        assert_eq!(state.min(), 10.0);
        assert_eq!(state.max(), 20.0);
        assert_eq!(state.step(), 2.5);
        assert_eq!(state.normalized_value(), 0.5);
        assert_eq!(state.colors().range().token(), semantic::ACCENT);
        assert!(state.activation_enabled());
    }

    #[test]
    fn slider_keyboard_change_respects_bounds() {
        let state = Slider::new("volume", "Volume")
            .min(0.0)
            .max(10.0)
            .step(2.0)
            .value(9.0)
            .state();

        assert_eq!(
            state.keyboard_change("right").map(|change| change.value()),
            Some(10.0)
        );
        assert_eq!(
            state.keyboard_change("home").map(|change| change.value()),
            Some(0.0)
        );
        assert_eq!(state.keyboard_change("unknown"), None);
    }

    #[test]
    fn disabled_slider_has_no_keyboard_change() {
        let state = Slider::new("volume", "Volume").disabled(true).state();

        assert!(state.disabled());
        assert!(!state.activation_enabled());
        assert_eq!(state.keyboard_change("right"), None);
    }

    #[test]
    fn slider_set_value_change_normalizes_valid_numeric_requests() {
        let state = Slider::new("volume", "Volume")
            .min(0.0)
            .max(10.0)
            .step(2.0)
            .value(4.0)
            .state();

        let change = state
            .set_value_change(7.1)
            .expect("finite numeric values should produce a change payload");
        assert_eq!(change.previous_value(), 4.0);
        assert_eq!(change.value(), 8.0);
        assert!(state.set_value_change(f64::NAN).is_none());
        assert!(
            Slider::new("disabled", "Disabled")
                .disabled(true)
                .state()
                .set_value_change(50.0)
                .is_none()
        );
    }
}
