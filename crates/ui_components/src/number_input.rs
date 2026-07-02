//! Number input component.

use crate::a11y::UiA11yElementExt;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::geometry::gpui_px_from_ui;
use crate::slider::{normalize_bounds, normalize_numeric_value, normalize_step};
use crate::text_input::{TextInputColors, TextInputMetrics};
use crate::theme::{ThemeContext, ThemeResolver};
use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, KeyDownEvent, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{AccessibleAction, Role, Sizable, Size, ThemeTokens};
use std::rc::Rc;

/// Resolved number-input color intents.
pub type NumberInputColors = TextInputColors;

/// Resolved number-input metrics.
pub type NumberInputMetrics = TextInputMetrics;

/// Number-input step action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberInputStepAction {
    /// Increment by one step.
    Increment,
    /// Decrement by one step.
    Decrement,
    /// Jump to the minimum.
    Minimum,
    /// Jump to the maximum.
    Maximum,
}

impl NumberInputStepAction {
    /// Returns a stable action label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Increment => "increment",
            Self::Decrement => "decrement",
            Self::Minimum => "minimum",
            Self::Maximum => "maximum",
        }
    }
}

/// Number-input value-change payload.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumberInputChange {
    action: NumberInputStepAction,
    previous_value: f32,
    value: f32,
}

impl NumberInputChange {
    /// Creates a number-input change payload.
    pub fn new(action: NumberInputStepAction, previous_value: f32, value: f32) -> Self {
        Self {
            action,
            previous_value,
            value,
        }
    }

    /// Returns the action that produced this change.
    pub const fn action(self) -> NumberInputStepAction {
        self.action
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

/// Resolved number-input state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberInputState {
    label: String,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    disabled: bool,
    read_only: bool,
    invalid: bool,
    required: bool,
    size: Size,
    metrics: NumberInputMetrics,
    colors: NumberInputColors,
    focus_ring: FocusRing,
}

impl NumberInputState {
    /// Resolves the public state for a number input.
    pub fn resolve(
        label: impl Into<String>,
        value: f32,
        min: f32,
        max: f32,
        step: f32,
        disabled: bool,
        read_only: bool,
        invalid: bool,
        required: bool,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        let (min, max) = normalize_bounds(min, max);
        let step = normalize_step(step);
        let value = normalize_numeric_value(value, min, max, step);
        let colors = ThemeResolver::text_input_colors(tokens, disabled, read_only, invalid);

        Self {
            label: label.into(),
            value,
            min,
            max,
            step,
            disabled,
            read_only,
            invalid,
            required,
            size,
            metrics: NumberInputMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns the accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the normalized value.
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

    /// Returns the positive step.
    pub const fn step(&self) -> f32 {
        self.step
    }

    /// Returns whether the input is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the input is read-only.
    pub const fn read_only(&self) -> bool {
        self.read_only
    }

    /// Returns whether the input is invalid.
    pub const fn invalid(&self) -> bool {
        self.invalid
    }

    /// Returns whether the input is required.
    pub const fn required(&self) -> bool {
        self.required
    }

    /// Returns whether value-change handlers should run.
    pub const fn input_enabled(&self) -> bool {
        !self.disabled && !self.read_only
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        self.input_enabled()
    }

    /// Returns whether the element should be included in tab traversal.
    pub const fn tab_stop_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the text shown inside the input shell.
    pub fn display_value(&self) -> String {
        let rounded = self.value.round();
        if (self.value - rounded).abs() <= f32::EPSILON {
            format!("{rounded:.0}")
        } else {
            format!("{}", self.value)
        }
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::SpinButton
    }

    /// Returns a step change for an action.
    pub fn step_change(&self, action: NumberInputStepAction) -> Option<NumberInputChange> {
        if !self.input_enabled() {
            return None;
        }

        let next = match action {
            NumberInputStepAction::Increment => self.value + self.step,
            NumberInputStepAction::Decrement => self.value - self.step,
            NumberInputStepAction::Minimum => self.min,
            NumberInputStepAction::Maximum => self.max,
        };

        Some(NumberInputChange::new(
            action,
            self.value,
            normalize_numeric_value(next, self.min, self.max, self.step),
        ))
    }

    /// Returns a keyboard step change for a key.
    pub fn keyboard_change(&self, key: &str) -> Option<NumberInputChange> {
        let action = match key {
            "up" => NumberInputStepAction::Increment,
            "down" => NumberInputStepAction::Decrement,
            "home" => NumberInputStepAction::Minimum,
            "end" => NumberInputStepAction::Maximum,
            _ => return None,
        };

        self.step_change(action)
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> NumberInputMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> NumberInputColors {
        self.colors
    }

    /// Returns resolved focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }
}

/// A concrete GPUI number input shell.
#[derive(IntoElement)]
pub struct NumberInput {
    id: ElementId,
    label: SharedString,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    disabled: bool,
    read_only: bool,
    invalid: bool,
    required: bool,
    size: Size,
    tokens: ThemeTokens,
    on_change: Option<Rc<dyn Fn(NumberInputChange, &mut Window, &mut App)>>,
}

impl NumberInput {
    /// Creates a new number input.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: 0.0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            disabled: false,
            read_only: false,
            invalid: false,
            required: false,
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

    /// Marks the input as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the input as read-only.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Marks the input as invalid.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Marks the input as required.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
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
        handler: impl Fn(NumberInputChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved number-input state.
    pub fn state(&self) -> NumberInputState {
        NumberInputState::resolve(
            self.label.to_string(),
            self.value,
            self.min,
            self.max,
            self.step,
            self.disabled,
            self.read_only,
            self.invalid,
            self.required,
            self.size,
            self.tokens,
        )
    }
}

impl Sizable for NumberInput {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for NumberInput {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let label = self.label.clone();
        let debug_id = self.id.to_string();
        let on_change_for_keyboard = self.on_change.clone();
        let on_change_for_increment = self.on_change.clone();
        let on_change_for_decrement = self.on_change.clone();
        let focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);

        div()
            .id(self.id)
            .debug_selector(move || format!("number-input:{debug_id}:root"))
            .min_h(gpui_px_from_ui(metrics.height()))
            .px(gpui_px_from_ui(metrics.padding_x()))
            .py(gpui_px_from_ui(metrics.padding_y()))
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .rounded(gpui_px_from_ui(metrics.radius()))
            .border_1()
            .border_color(theme.resolve(colors.border()))
            .bg(theme.resolve(colors.background()))
            .text_color(theme.resolve(colors.foreground()))
            .text_size(gpui_px_from_ui(metrics.text_size()))
            .line_height(gpui_px_from_ui(metrics.text_size()))
            .focusable()
            .tab_stop(state.tab_stop_enabled())
            .ui_role(state.role())
            .aria_label(label)
            .aria_numeric_value(state.value() as f64)
            .aria_min_numeric_value(state.min() as f64)
            .aria_max_numeric_value(state.max() as f64)
            .focus_visible(move |style| style.shadow(focus_shadow))
            .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
            .when(state.input_enabled(), |this| this.cursor_text())
            .when_some(
                on_change_for_keyboard.filter(|_| state.input_enabled()),
                |this, on_change| {
                    let key_state = state.clone();
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
                        let action_state = state.clone();
                        let on_change = on_change.clone();
                        move |_, window, cx| {
                            if let Some(change) =
                                action_state.step_change(NumberInputStepAction::Increment)
                                && change.changed()
                            {
                                on_change(change, window, cx);
                            }
                        }
                    })
                    .on_ui_a11y_action(AccessibleAction::Decrement, {
                        let action_state = state.clone();
                        move |_, window, cx| {
                            if let Some(change) =
                                action_state.step_change(NumberInputStepAction::Decrement)
                                && change.changed()
                            {
                                on_change(change, window, cx);
                            }
                        }
                    })
                },
            )
            .child(div().flex_1().child(state.display_value()))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(number_step_button(
                        "number-step-up",
                        "+",
                        state.clone(),
                        NumberInputStepAction::Increment,
                        on_change_for_increment,
                        &theme,
                    ))
                    .child(number_step_button(
                        "number-step-down",
                        "-",
                        state,
                        NumberInputStepAction::Decrement,
                        on_change_for_decrement,
                        &theme,
                    )),
            )
    }
}

fn number_step_button(
    id: &'static str,
    label: &'static str,
    state: NumberInputState,
    action: NumberInputStepAction,
    on_change: Option<Rc<dyn Fn(NumberInputChange, &mut Window, &mut App)>>,
    theme: &ThemeContext,
) -> impl IntoElement {
    div()
        .id(id)
        .size(gpui_px_from_ui(ui_px_button_size(state.size())))
        .flex()
        .items_center()
        .justify_center()
        .rounded(gpui_px_from_ui(state.size().control_radius()))
        .border_1()
        .border_color(theme.resolve(state.colors().border()))
        .when(state.input_enabled(), |this| this.cursor_pointer())
        .when(!state.input_enabled(), |this| {
            this.opacity(0.56).cursor_not_allowed()
        })
        .when_some(
            on_change.filter(|_| state.input_enabled()),
            move |this, on_change| {
                this.on_click(move |_: &ClickEvent, window, cx| {
                    if let Some(change) = state.step_change(action)
                        && change.changed()
                    {
                        on_change(change, window, cx);
                    }
                    cx.stop_propagation();
                })
            },
        )
        .child(label)
}

fn ui_px_button_size(size: Size) -> open_gpui_ui_core::UiPx {
    match size {
        Size::XSmall => open_gpui_ui_core::ui_px(14.0),
        Size::Small => open_gpui_ui_core::ui_px(16.0),
        Size::Medium => open_gpui_ui_core::ui_px(18.0),
        Size::Large => open_gpui_ui_core::ui_px(20.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::semantic;

    #[test]
    fn number_input_clamps_and_snaps_value() {
        let state = NumberInput::new("quantity", "Quantity")
            .min(1.0)
            .max(9.0)
            .step(2.0)
            .value(8.2)
            .state();

        assert_eq!(state.role(), Role::SpinButton);
        assert_eq!(state.value(), 9.0);
        assert_eq!(state.display_value(), "9");
        assert_eq!(state.colors().border().token(), semantic::BORDER);
        assert!(state.activation_enabled());
    }

    #[test]
    fn number_input_keyboard_changes_emit_once_per_step() {
        let state = NumberInput::new("quantity", "Quantity")
            .min(0.0)
            .max(10.0)
            .step(2.0)
            .value(4.0)
            .state();

        let increment = state
            .keyboard_change("up")
            .expect("up should increment number input");
        assert_eq!(increment.action(), NumberInputStepAction::Increment);
        assert_eq!(increment.previous_value(), 4.0);
        assert_eq!(increment.value(), 6.0);

        let minimum = state
            .keyboard_change("home")
            .expect("home should jump to min");
        assert_eq!(minimum.action(), NumberInputStepAction::Minimum);
        assert_eq!(minimum.value(), 0.0);
        assert_eq!(state.keyboard_change("left"), None);
    }

    #[test]
    fn read_only_and_disabled_number_inputs_do_not_step() {
        let read_only = NumberInput::new("quantity", "Quantity")
            .read_only(true)
            .state();
        let disabled = NumberInput::new("quantity", "Quantity")
            .disabled(true)
            .state();

        assert!(!read_only.activation_enabled());
        assert!(read_only.tab_stop_enabled());
        assert_eq!(read_only.keyboard_change("up"), None);
        assert!(!disabled.activation_enabled());
        assert!(!disabled.tab_stop_enabled());
        assert_eq!(disabled.keyboard_change("up"), None);
    }
}
