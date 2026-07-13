//! Checkbox component.

use crate::geometry::gpui_px_from_ui;
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{
    AccessibleAction, Role, SemanticDescriptor, Sizable, Size, ThemeTokens, Toggled, UiPx, ui_px,
};

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow_with_theme};
use crate::theme::ThemeResolver;

/// Resolved checkbox metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckboxMetrics {
    box_size: UiPx,
    box_radius: UiPx,
    indicator_size: UiPx,
    mixed_bar_height: UiPx,
    label_gap: UiPx,
    label_text_size: UiPx,
}

impl CheckboxMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        let box_size = match size {
            Size::XSmall => ui_px(16.0),
            Size::Small => ui_px(18.0),
            Size::Medium => ui_px(20.0),
            Size::Large => ui_px(22.0),
        };
        let indicator_size = match size {
            Size::XSmall => ui_px(8.0),
            Size::Small => ui_px(9.0),
            Size::Medium => ui_px(10.0),
            Size::Large => ui_px(11.0),
        };

        Self {
            box_size,
            box_radius: ui_px(4.0),
            indicator_size,
            mixed_bar_height: ui_px(2.0),
            label_gap: ui_px(8.0),
            label_text_size: size.control_text_px(),
        }
    }

    /// Returns the checkbox box size.
    pub const fn box_size(self) -> UiPx {
        self.box_size
    }

    /// Returns the corner radius.
    pub const fn box_radius(self) -> UiPx {
        self.box_radius
    }

    /// Returns the indicator size.
    pub const fn indicator_size(self) -> UiPx {
        self.indicator_size
    }

    /// Returns the mixed-state bar height.
    pub const fn mixed_bar_height(self) -> UiPx {
        self.mixed_bar_height
    }

    /// Returns the gap between the box and the label.
    pub const fn label_gap(self) -> UiPx {
        self.label_gap
    }

    /// Returns the label text size.
    pub const fn label_text_size(self) -> UiPx {
        self.label_text_size
    }
}

/// Resolved checkbox color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckboxColors {
    pub(crate) background: ColorIntent,
    pub(crate) hover_background: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) indicator: ColorIntent,
    pub(crate) label: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl CheckboxColors {
    /// Returns the box background color intent.
    pub const fn background(self) -> ColorIntent {
        self.background
    }

    /// Returns the hover background color intent.
    pub const fn hover_background(self) -> ColorIntent {
        self.hover_background
    }

    /// Returns the border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }

    /// Returns the indicator color intent.
    pub const fn indicator(self) -> ColorIntent {
        self.indicator
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

/// Resolved checkbox state used by tests, demos, and rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckboxState {
    checked: bool,
    indeterminate: bool,
    size: Size,
    disabled: bool,
    required: bool,
    invalid: bool,
    busy: bool,
    metrics: CheckboxMetrics,
    colors: CheckboxColors,
    focus_ring: FocusRing,
}

impl CheckboxState {
    /// Resolves the public state for a checkbox.
    pub fn resolve(
        checked: bool,
        indeterminate: bool,
        size: Size,
        disabled: bool,
        required: bool,
        invalid: bool,
        tokens: ThemeTokens,
    ) -> Self {
        let colors =
            ThemeResolver::checkbox_colors(tokens, checked, indeterminate, disabled, invalid);

        Self {
            checked,
            indeterminate,
            size,
            disabled,
            required,
            invalid,
            busy: false,
            metrics: CheckboxMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
        }
    }

    /// Returns whether the checkbox is checked.
    pub const fn checked(self) -> bool {
        self.checked
    }

    /// Returns whether the checkbox is indeterminate.
    pub const fn indeterminate(self) -> bool {
        self.indeterminate
    }

    /// Returns the foundation size.
    pub const fn size(self) -> Size {
        self.size
    }

    /// Returns whether the checkbox is disabled.
    pub const fn disabled(self) -> bool {
        self.disabled
    }

    /// Returns whether the checkbox is required.
    pub const fn required(self) -> bool {
        self.required
    }

    /// Returns whether the checkbox is invalid.
    pub const fn invalid(self) -> bool {
        self.invalid
    }

    /// Returns this state with asynchronous activity updated.
    pub const fn with_busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Returns whether asynchronous work is pending for this checkbox.
    pub const fn busy(self) -> bool {
        self.busy
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(self) -> bool {
        !self.disabled
    }

    /// Returns whether the element should be included in tab traversal.
    pub const fn tab_stop_enabled(self) -> bool {
        !self.disabled
    }

    /// Returns the accessibility role.
    pub const fn role(self) -> Role {
        Role::CheckBox
    }

    /// Returns the accessibility toggled state.
    pub const fn toggled(self) -> Toggled {
        if self.indeterminate {
            Toggled::Mixed
        } else if self.checked {
            Toggled::True
        } else {
            Toggled::False
        }
    }

    /// Returns resolved metrics.
    pub const fn metrics(self) -> CheckboxMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(self) -> CheckboxColors {
        self.colors
    }

    /// Returns resolved focus ring metadata.
    pub const fn focus_ring(self) -> FocusRing {
        self.focus_ring
    }
}

/// A concrete GPUI checkbox component.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    label: Option<SharedString>,
    aria_label: Option<SharedString>,
    checked: bool,
    indeterminate: bool,
    disabled: bool,
    required: bool,
    invalid: bool,
    busy: bool,
    size: Size,
    tokens: ThemeTokens,
    on_toggle: Option<Rc<dyn Fn(Toggled, &ClickEvent, &mut Window, &mut App)>>,
}

impl Checkbox {
    /// Creates a new checkbox with an id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            aria_label: None,
            checked: false,
            indeterminate: false,
            disabled: false,
            required: false,
            invalid: false,
            busy: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
            on_toggle: None,
        }
    }

    /// Sets the visible label.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the accessible label without rendering visible text.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Marks the checkbox as checked.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Marks the checkbox as indeterminate.
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.indeterminate = indeterminate;
        self
    }

    /// Sets the tri-state value directly.
    pub fn checked_state(mut self, toggled: Toggled) -> Self {
        match toggled {
            Toggled::True => {
                self.checked = true;
                self.indeterminate = false;
            }
            Toggled::False => {
                self.checked = false;
                self.indeterminate = false;
            }
            Toggled::Mixed => {
                self.checked = false;
                self.indeterminate = true;
            }
        }
        self
    }

    /// Marks the checkbox as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the checkbox as required.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Marks the checkbox as invalid.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Marks the checkbox as having pending asynchronous work.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a toggle handler with the next tri-state value.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(Toggled, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved checkbox state.
    pub fn state(&self) -> CheckboxState {
        CheckboxState::resolve(
            self.checked,
            self.indeterminate,
            self.size,
            self.disabled,
            self.required,
            self.invalid,
            self.tokens,
        )
        .with_busy(self.busy)
    }
}

impl Sizable for Checkbox {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let label = self.label.clone();
        let aria_label = self.aria_label.clone();
        let next_toggled = if state.indeterminate() {
            Toggled::True
        } else if state.checked() {
            Toggled::False
        } else {
            Toggled::True
        };
        let label_text = aria_label
            .clone()
            .or_else(|| label.clone())
            .unwrap_or_else(|| SharedString::from("Checkbox"));
        let debug_id = self.id.to_string();
        let focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);
        let actions: &[AccessibleAction] = if self.on_toggle.is_some() {
            &[AccessibleAction::Click, AccessibleAction::Focus]
        } else {
            &[AccessibleAction::Focus]
        };
        let semantics = SemanticDescriptor::new(state.role())
            .with_label(label_text.as_ref())
            .with_toggled(state.toggled())
            .with_required(state.required())
            .with_invalid(state.invalid())
            .with_busy(state.busy())
            .with_disabled(disabled)
            .with_actions(actions);

        div()
            .id(self.id)
            .debug_selector(move || format!("checkbox:{debug_id}:root"))
            .flex()
            .items_center()
            .gap_2()
            .focusable()
            .tab_stop(state.tab_stop_enabled())
            .ui_semantics(&semantics)
            .focus_visible(move |style| style.shadow(focus_shadow.clone()))
            .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
            .when(!disabled, |this| this.cursor_pointer())
            .when_some(
                self.on_toggle.filter(|_| !disabled),
                move |this, on_toggle| {
                    this.on_click(move |event, window, cx| {
                        cx.stop_propagation();
                        on_toggle(next_toggled, event, window, cx);
                    })
                },
            )
            .child(
                div()
                    .w(gpui_px_from_ui(metrics.box_size()))
                    .h(gpui_px_from_ui(metrics.box_size()))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(gpui_px_from_ui(metrics.box_radius()))
                    .border_1()
                    .border_color(theme.resolve(colors.border()))
                    .bg(theme.resolve(colors.background()))
                    .hover(|style| style.bg(theme.resolve(colors.hover_background())))
                    .child({
                        let indicator = if state.indeterminate() {
                            div()
                                .w(gpui_px_from_ui(metrics.indicator_size()))
                                .h(gpui_px_from_ui(metrics.mixed_bar_height()))
                                .rounded(gpui_px_from_ui(metrics.mixed_bar_height()))
                                .bg(theme.resolve(colors.indicator()))
                        } else if state.checked() {
                            div()
                                .w(gpui_px_from_ui(metrics.indicator_size()))
                                .h(gpui_px_from_ui(metrics.indicator_size()))
                                .rounded(gpui_px_from_ui(metrics.indicator_size()))
                                .bg(theme.resolve(colors.indicator()))
                        } else {
                            div().w(open_gpui::px(0.0)).h(open_gpui::px(0.0))
                        };
                        indicator
                    }),
            )
            .when_some(label, |this, label| {
                this.child(
                    div()
                        .text_size(gpui_px_from_ui(metrics.label_text_size()))
                        .line_height(gpui_px_from_ui(metrics.box_size()))
                        .text_color(theme.resolve(colors.label()))
                        .child(label),
                )
            })
    }
}
