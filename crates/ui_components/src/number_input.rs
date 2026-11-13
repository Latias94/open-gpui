//! Editable scalar control with local drafts and explicit preview, commit, and cancel phases.

mod controller;
mod editing;

pub use controller::NumberInputController;
pub use editing::{
    NumberInputCancel, NumberInputCancelReason, NumberInputCommit, NumberInputDraft,
    NumberInputDraftValue, NumberInputEditSource, NumberInputPreview,
};

use controller::{NumberInputHandlers, NumberInputMount};

use crate::a11y::UiA11yElementExt;
use crate::activation::{ActivationBinding, ActivationKeyPolicy, ActivationSource};
use crate::focus::FocusRing;
use crate::form_control::FormControlState;
use crate::geometry::gpui_px_from_ui;
use crate::slider::normalize_bounds;
use crate::text_input::{
    TextInput, TextInputColors, TextInputMetrics, adapter::TextInputController,
};
use crate::theme::{ThemeContext, ThemeResolver};
use open_gpui::prelude::*;
use open_gpui::{
    App, ElementId, Entity, Focusable, IntoElement, KeyDownEvent, MouseButton, ParentElement,
    RenderOnce, SharedString, Styled, Window, canvas, div, px,
};
use open_gpui_ui_core::{AccessibleAction, Role, SemanticDescriptor, Sizable, Size, ThemeTokens};
use std::rc::Rc;

/// Resolved number-input color intents.
pub type NumberInputColors = TextInputColors;

/// Resolved number-input metrics.
pub type NumberInputMetrics = TextInputMetrics;

/// Number-input value-change action.
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
    /// Set an explicit numeric value.
    SetValue,
}

impl NumberInputStepAction {
    /// Returns a stable action label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Increment => "increment",
            Self::Decrement => "decrement",
            Self::Minimum => "minimum",
            Self::Maximum => "maximum",
            Self::SetValue => "set-value",
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
        self.previous_value != self.value
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
    control: FormControlState,
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
        let step = if step.is_finite() && step > 0.0 {
            step
        } else {
            1.0
        };
        let value = normalize_number_value(value, min, max, step);
        let colors = ThemeResolver::text_input_colors(tokens, disabled, read_only, invalid);
        let control =
            FormControlState::resolve(size, disabled, read_only, invalid, required, false);

        Self {
            label: label.into(),
            value,
            min,
            max,
            step,
            control,
            metrics: NumberInputMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns this state with asynchronous activity updated.
    pub const fn with_busy(mut self, busy: bool) -> Self {
        self.control = self.control.with_busy(busy);
        self
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

    /// Returns the shared form-control state.
    pub const fn control_state(&self) -> FormControlState {
        self.control
    }

    /// Returns whether the input is disabled.
    pub const fn disabled(&self) -> bool {
        self.control.disabled()
    }

    /// Returns whether the input is read-only.
    pub const fn read_only(&self) -> bool {
        self.control.read_only()
    }

    /// Returns whether the input is invalid.
    pub const fn invalid(&self) -> bool {
        self.control.invalid()
    }

    /// Returns whether asynchronous work is pending for this input.
    pub const fn busy(&self) -> bool {
        self.control.busy()
    }

    /// Returns whether the input is required.
    pub const fn required(&self) -> bool {
        self.control.required()
    }

    /// Returns whether value-change handlers should run.
    pub const fn input_enabled(&self) -> bool {
        self.control.input_enabled()
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        self.control.activation_enabled()
    }

    /// Returns whether the element should be included in tab traversal.
    pub const fn tab_stop_enabled(&self) -> bool {
        self.control.tab_stop_enabled()
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.control.size()
    }

    /// Returns the text shown inside the input shell.
    pub fn display_value(&self) -> String {
        self.value.to_string()
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
            NumberInputStepAction::Increment => self.value as f64 + self.step as f64,
            NumberInputStepAction::Decrement => self.value as f64 - self.step as f64,
            NumberInputStepAction::Minimum => self.min as f64,
            NumberInputStepAction::Maximum => self.max as f64,
            NumberInputStepAction::SetValue => return None,
        };

        Some(NumberInputChange::new(
            action,
            self.value,
            normalize_number_value(
                next.clamp(self.min as f64, self.max as f64) as f32,
                self.min,
                self.max,
                self.step,
            ),
        ))
    }

    /// Returns an arrow-key step change. Home and End remain native text-selection commands.
    pub fn keyboard_change(&self, key: &str) -> Option<NumberInputChange> {
        let action = match key {
            "up" => NumberInputStepAction::Increment,
            "down" => NumberInputStepAction::Decrement,
            _ => return None,
        };

        self.step_change(action)
    }

    fn set_value_change(&self, requested_value: f64) -> Option<NumberInputChange> {
        if !self.input_enabled() {
            return None;
        }

        if !requested_value.is_finite() {
            return None;
        }
        let requested_value = normalize_number_value(
            requested_value.clamp(self.min as f64, self.max as f64) as f32,
            self.min,
            self.max,
            self.step,
        );
        Some(NumberInputChange::new(
            NumberInputStepAction::SetValue,
            self.value,
            requested_value,
        ))
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

/// A concrete GPUI scalar editor composed with `TextInput`.
///
/// Drafts and scrub previews stay local until submission. For controlled inputs, a value
/// change is a proposal: echo it through `value` on the next render to accept it. A different
/// external value while editing cancels the draft; an unchanged value preserves it and its IME
/// composition. Applications retain ownership of validation, units, and history grouping.
#[derive(IntoElement)]
pub struct NumberInput {
    id: ElementId,
    label: SharedString,
    value: Option<f32>,
    default_value: f32,
    min: f32,
    max: f32,
    step: f32,
    control: FormControlState,
    tokens: ThemeTokens,
    controller: Option<Entity<NumberInputController>>,
    handlers: NumberInputHandlers,
    scrub: bool,
    scrub_pixels_per_step: f32,
    stepper: bool,
}

impl NumberInput {
    /// Creates a new number input.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: None,
            default_value: 0.0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            control: FormControlState::default(),
            tokens: ThemeTokens::default(),
            controller: None,
            handlers: NumberInputHandlers::default(),
            scrub: true,
            scrub_pixels_per_step: 8.0,
            stepper: true,
        }
    }

    /// Sets the controlled value.
    pub fn value(mut self, value: f32) -> Self {
        self.value = Some(value);
        self
    }

    /// Sets the initial value for an input whose numeric value is locally owned.
    pub fn default_value(mut self, value: f32) -> Self {
        self.default_value = value;
        self
    }

    /// Uses persistent editing state owned by the application.
    pub fn controller(mut self, controller: Entity<NumberInputController>) -> Self {
        self.controller = Some(controller);
        self
    }

    /// Shows or hides the horizontal scrub handle without changing text selection behavior.
    pub fn scrub(mut self, enabled: bool) -> Self {
        self.scrub = enabled;
        self
    }

    /// Sets the horizontal pointer distance per numeric step; invalid distances use eight pixels.
    pub fn scrub_pixels_per_step(mut self, pixels: f32) -> Self {
        self.scrub_pixels_per_step = if pixels.is_finite() && pixels > 0.0 {
            pixels
        } else {
            8.0
        };
        self
    }

    /// Shows or hides the step buttons; keyboard and accessibility stepping remain available.
    pub fn stepper(mut self, enabled: bool) -> Self {
        self.stepper = enabled;
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
        self.control = self.control.with_disabled(disabled);
        self
    }

    /// Marks the input as read-only.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.control = self.control.with_read_only(read_only);
        self
    }

    /// Marks the input as invalid.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.control = self.control.with_invalid(invalid);
        self
    }

    /// Marks the input as required.
    pub fn required(mut self, required: bool) -> Self {
        self.control = self.control.with_required(required);
        self
    }

    /// Marks the input as having pending asynchronous work.
    pub fn busy(mut self, busy: bool) -> Self {
        self.control = self.control.with_busy(busy);
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a normalized value proposal handler.
    ///
    /// Previews do not invoke this handler. A controlled owner accepts a proposal by returning
    /// it through `value` on the next render; keeping the old value rejects the proposal.
    pub fn on_change(
        mut self,
        handler: impl Fn(NumberInputChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.change = Some(Rc::new(handler));
        self
    }

    /// Observes exact local drafts without accepting values or creating history entries.
    pub fn on_preview(
        mut self,
        handler: impl Fn(NumberInputPreview, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.preview = Some(Rc::new(handler));
        self
    }

    /// Observes the final accepted or rejected result of a commit request.
    pub fn on_commit(
        mut self,
        handler: impl Fn(NumberInputCommit, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.commit = Some(Rc::new(handler));
        self
    }

    /// Observes edit cancellation, including invalid submission and pointer capture loss.
    pub fn on_cancel(
        mut self,
        handler: impl Fn(NumberInputCancel, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.cancel = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved number-input state.
    pub fn state(&self) -> NumberInputState {
        NumberInputState::resolve(
            self.label.to_string(),
            self.value.unwrap_or(self.default_value),
            self.min,
            self.max,
            self.step,
            self.control.disabled(),
            self.control.read_only(),
            self.control.invalid(),
            self.control.required(),
            self.control.size(),
            self.tokens,
        )
        .with_busy(self.control.busy())
    }
}

impl Sizable for NumberInput {
    fn with_size(mut self, size: Size) -> Self {
        self.control = self.control.with_size(size);
        self
    }
}

impl RenderOnce for NumberInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let mut state = self.state();
        let mount = window.use_keyed_state((self.id.clone(), "number-mount"), cx, |window, cx| {
            let controller = self.controller.clone().unwrap_or_else(|| {
                cx.new(|cx| NumberInputController::new(state.value(), window, cx))
            });
            NumberInputMount::new(controller, cx)
        });
        let controller = mount.update(cx, |mount, cx| mount.controller(self.controller, cx));
        controller.update(cx, |controller, cx| {
            controller.sync(state.clone(), self.value, self.handlers, window, cx);
        });
        let runtime = controller.read(cx);
        state.value = runtime.value();
        let input = runtime.text_input().clone();
        let capture = runtime.capture();
        let pending = runtime.pending_submission();
        let scrubbing = runtime.is_scrubbing();
        let composing = runtime.is_composing(cx);
        let draft_invalid = matches!(runtime.draft().parsed(), NumberInputDraftValue::Invalid);
        let metrics = state.metrics();
        let colors = state.colors();
        let disabled = state.disabled();
        let debug_id = self.id.to_string();
        let input_id: ElementId = (self.id.clone(), "text").into();
        let input_label = format!("{} value", self.label);
        let pointer_editing_enabled = state.input_enabled() && !pending && !composing;
        let step_buttons = self.stepper.then(|| {
            let mut step_state = state.clone();
            step_state.control = step_state
                .control
                .with_disabled(!pointer_editing_enabled || scrubbing);
            let buttons = [
                ("number-step-up", "+", NumberInputStepAction::Increment),
                ("number-step-down", "-", NumberInputStepAction::Decrement),
            ]
            .map(|(id, label, action)| {
                let step_controller = controller.clone();
                let activation = ActivationBinding::new(
                    window,
                    cx,
                    (self.id.clone(), id),
                    step_state.input_enabled(),
                    ActivationKeyPolicy::EnterOrSpace,
                    move |activation, window, cx| {
                        let source = match activation.source() {
                            ActivationSource::Accessibility => NumberInputEditSource::Accessibility,
                            ActivationSource::Keyboard(_) => NumberInputEditSource::Keyboard,
                            ActivationSource::Pointer | ActivationSource::Programmatic => {
                                NumberInputEditSource::Stepper
                            }
                        };
                        step_controller.update(cx, |controller, cx| {
                            controller.step_from(action, source, window, cx);
                        });
                    },
                );
                number_step_button(
                    id,
                    label,
                    &debug_id,
                    &step_state,
                    controller.clone(),
                    activation,
                    &theme,
                )
                .into_any_element()
            });
            div().flex().flex_col().gap_1().children(buttons)
        });
        let actions: &[AccessibleAction] = if state.input_enabled() {
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
            .with_required(state.required())
            .with_invalid(state.invalid() || draft_invalid)
            .with_busy(state.busy())
            .with_read_only(state.read_only())
            .with_disabled(disabled)
            .with_actions(actions);

        div()
            .id(self.id)
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("number-input:{debug_id}:root")
            })
            .min_w(px(0.0))
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
            .ui_semantics(&semantics)
            .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
            .when(state.input_enabled(), |this| this.cursor_text())
            .on_ui_a11y_action(AccessibleAction::Focus, {
                let input = input.clone();
                move |_, window, cx| {
                    if !disabled {
                        window.focus(&input.focus_handle(cx), cx);
                    }
                }
            })
            .on_mouse_down(MouseButton::Left, {
                let input = input.clone();
                move |_, window, cx| {
                    if !disabled {
                        window.focus(&input.focus_handle(cx), cx);
                    }
                }
            })
            .capture_key_down({
                let controller = controller.clone();
                move |event: &KeyDownEvent, window, cx| {
                    if event.prefer_character_input
                        || window.default_prevented()
                        || event.keystroke.modifiers.modified()
                        || controller.read(cx).is_composing(cx)
                    {
                        return;
                    }
                    let handled = controller.update(cx, |controller, cx| {
                        match event.keystroke.key.as_str() {
                            "enter" => controller.commit(window, cx),
                            "escape" => {
                                controller.cancel(NumberInputCancelReason::Escape, window, cx)
                            }
                            "up" => controller.step(NumberInputStepAction::Increment, window, cx),
                            "down" => controller.step(NumberInputStepAction::Decrement, window, cx),
                            _ => false,
                        }
                    });
                    if handled {
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                }
            })
            .on_ui_a11y_action(AccessibleAction::Increment, {
                let controller = controller.clone();
                move |_, window, cx| {
                    controller.update(cx, |controller, cx| {
                        controller.step_from(
                            NumberInputStepAction::Increment,
                            NumberInputEditSource::Accessibility,
                            window,
                            cx,
                        );
                    });
                }
            })
            .on_ui_a11y_action(AccessibleAction::Decrement, {
                let controller = controller.clone();
                move |_, window, cx| {
                    controller.update(cx, |controller, cx| {
                        controller.step_from(
                            NumberInputStepAction::Decrement,
                            NumberInputEditSource::Accessibility,
                            window,
                            cx,
                        );
                    });
                }
            })
            .on_ui_a11y_action(AccessibleAction::SetValue, {
                let controller = controller.clone();
                move |data, window, cx| {
                    if let Some(open_gpui::accesskit::ActionData::NumericValue(value)) = data {
                        controller.update(cx, |controller, cx| {
                            controller.set_accessible_value(*value, window, cx)
                        });
                    }
                }
            })
            .when(self.scrub, |this| {
                this.child(number_scrub_handle(
                    &debug_id,
                    controller.clone(),
                    capture,
                    pointer_editing_enabled,
                    self.scrub_pixels_per_step,
                ))
            })
            .child(
                div().min_w(px(0.0)).flex_1().child(
                    TextInput::new(input_id, input_label)
                        .controller(input)
                        .disabled(disabled)
                        .read_only(state.read_only() || pending || scrubbing)
                        .invalid(state.invalid() || draft_invalid)
                        .required(state.required())
                        .busy(state.busy())
                        .with_size(state.size())
                        .tokens(self.tokens),
                ),
            )
            .children(step_buttons)
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        window.on_pointer_cancel(move |_, phase, window, cx| {
                            if phase == open_gpui::DispatchPhase::Capture
                                && controller.read(cx).is_scrubbing()
                            {
                                controller.update(cx, |controller, cx| {
                                    controller.cancel(
                                        NumberInputCancelReason::PointerCancelled,
                                        window,
                                        cx,
                                    );
                                });
                            }
                        });
                    },
                )
                .absolute()
                .size(px(0.0)),
            )
    }
}

fn number_step_button(
    id: &'static str,
    label: &'static str,
    debug_id: &str,
    state: &NumberInputState,
    controller: Entity<NumberInputController>,
    activation: ActivationBinding,
    theme: &ThemeContext,
) -> impl IntoElement {
    let debug_selector = format!("number-input:{debug_id}:{id}");
    let enabled = state.input_enabled();
    let accessible_label = format!(
        "{} {}",
        if label == "+" {
            "Increment"
        } else {
            "Decrement"
        },
        state.label()
    );
    let semantics = SemanticDescriptor::new(Role::Button)
        .with_label(&accessible_label)
        .with_disabled(!enabled)
        .with_actions(if enabled {
            &[AccessibleAction::Click]
        } else {
            &[]
        });
    let button = div()
        .id(id)
        .debug_selector(move || debug_selector.clone())
        .size(gpui_px_from_ui(ui_px_button_size(state.size())))
        .flex()
        .items_center()
        .justify_center()
        .rounded(gpui_px_from_ui(state.size().control_radius()))
        .border_1()
        .border_color(theme.resolve(state.colors().border()))
        .ui_semantics(&semantics)
        .when(state.input_enabled(), |this| this.cursor_pointer())
        .when(!state.input_enabled(), |this| {
            this.opacity(0.56).cursor_not_allowed()
        })
        .when(enabled, |this| {
            this.on_mouse_down(MouseButton::Left, {
                let controller = controller.clone();
                move |_, window, cx| {
                    window.focus(&controller.focus_handle(cx), cx);
                    window.prevent_default();
                    cx.stop_propagation();
                }
            })
        })
        .child(label);
    activation.bind_pointer_and_accessibility(button)
}

fn number_scrub_handle(
    debug_id: &str,
    controller: Entity<NumberInputController>,
    capture: open_gpui::PointerCaptureHandle,
    enabled: bool,
    pixels_per_step: f32,
) -> impl IntoElement {
    let debug_id = debug_id.to_owned();
    div()
        .id("number-scrub")
        .debug_selector(move || format!("number-input:{debug_id}:scrub"))
        .px_1()
        .child("↔")
        .when(enabled, |this| {
            this.cursor(open_gpui::CursorStyle::ResizeLeftRight)
        })
        .when(!enabled, |this| this.cursor_not_allowed().opacity(0.56))
        .track_pointer_capture(&capture)
        .on_mouse_down(MouseButton::Left, {
            let controller = controller.clone();
            move |event, window, cx| {
                if enabled && !window.default_prevented() {
                    let captured = controller.update(cx, |controller, cx| {
                        controller.begin_scrub(
                            f32::from(event.window_event().position.x),
                            window,
                            cx,
                        )
                    });
                    if captured {
                        window.focus(&controller.focus_handle(cx), cx);
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                }
            }
        })
        .on_mouse_move({
            let controller = controller.clone();
            move |event, window, cx| {
                controller.update(cx, |controller, cx| {
                    controller.update_scrub(
                        f32::from(event.window_event().position.x),
                        pixels_per_step,
                        window,
                        cx,
                    );
                });
            }
        })
        .on_mouse_up(MouseButton::Left, move |event, window, cx| {
            controller.update(cx, |controller, cx| {
                controller.update_scrub(
                    f32::from(event.window_event().position.x),
                    pixels_per_step,
                    window,
                    cx,
                );
                controller.finish_scrub(window, cx);
            });
        })
}

fn normalize_number_value(value: f32, min: f32, max: f32, step: f32) -> f32 {
    let value = if value.is_finite() { value } else { min };
    let value = (value as f64).clamp(min as f64, max as f64);
    let step = step as f64;
    let remainder = (value.rem_euclid(step) - (min as f64).rem_euclid(step)).rem_euclid(step);
    let adjustment = if remainder < step / 2.0 {
        -remainder
    } else {
        step - remainder
    };
    (value + adjustment).clamp(min as f64, max as f64) as f32
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
            .step_change(NumberInputStepAction::Minimum)
            .expect("an explicit minimum action should jump to min");
        assert_eq!(minimum.action(), NumberInputStepAction::Minimum);
        assert_eq!(minimum.value(), 0.0);
        assert_eq!(state.keyboard_change("left"), None);
        assert_eq!(state.keyboard_change("home"), None);
        assert_eq!(state.keyboard_change("end"), None);
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

    #[test]
    fn number_input_set_value_change_preserves_action_and_normalizes_value() {
        let state = NumberInput::new("quantity", "Quantity")
            .min(0.0)
            .max(10.0)
            .step(2.0)
            .value(4.0)
            .state();

        let change = state
            .set_value_change(7.1)
            .expect("finite numeric values should produce a change payload");
        assert_eq!(change.action(), NumberInputStepAction::SetValue);
        assert_eq!(change.previous_value(), 4.0);
        assert_eq!(change.value(), 8.0);
        assert_eq!(state.step_change(NumberInputStepAction::SetValue), None);
        assert!(state.set_value_change(f64::INFINITY).is_none());
        assert!(
            NumberInput::new("readonly", "Read only")
                .read_only(true)
                .state()
                .set_value_change(50.0)
                .is_none()
        );
    }
}
