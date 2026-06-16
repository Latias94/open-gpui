//! Alert dialog component.

use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, anchored,
    deferred, div, point, px,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy,
    OverlayFocusTarget, OverlayLayerKind, OverlayPresence, Role, Sizable, Size, ThemeTokens,
};

use crate::button::ButtonVariant;
use crate::color::ColorIntent;
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::overlay::{
    GpuiOverlayAdapterConfig, GpuiOverlayState, escape_open_change, outside_press_open_change,
};
use crate::theme::ThemeResolver;

const CANCEL_FOCUS_TARGET: &str = "alert-dialog.cancel";
const ACTION_FOCUS_TARGET: &str = "alert-dialog.action";

type OpenChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Alert dialog open-state ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertDialogOpenMode {
    /// Open state is owned by the component adapter after initialization.
    #[default]
    Uncontrolled,
    /// Open state is provided by the caller.
    Controlled,
}

/// Semantic intent for the primary alert dialog action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertDialogIntent {
    /// Standard confirmation action.
    #[default]
    Default,
    /// Destructive confirmation action.
    Destructive,
}

impl AlertDialogIntent {
    /// Returns the button variant used for the primary action.
    pub const fn action_variant(self) -> ButtonVariant {
        match self {
            Self::Default => ButtonVariant::Default,
            Self::Destructive => ButtonVariant::Destructive,
        }
    }
}

/// Stable action slot inside an alert dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertDialogActionKind {
    /// Cancel or safe-dismiss action.
    Cancel,
    /// Primary confirmation action.
    Action,
}

/// Resolved alert dialog action metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertDialogActionState {
    kind: AlertDialogActionKind,
    label: SharedString,
    disabled: bool,
    variant: ButtonVariant,
    default_focus: bool,
}

impl AlertDialogActionState {
    /// Creates resolved action metadata.
    pub fn new(
        kind: AlertDialogActionKind,
        label: impl Into<SharedString>,
        disabled: bool,
        variant: ButtonVariant,
        default_focus: bool,
    ) -> Self {
        Self {
            kind,
            label: label.into(),
            disabled,
            variant,
            default_focus,
        }
    }

    /// Returns the action slot.
    pub const fn kind(&self) -> AlertDialogActionKind {
        self.kind
    }

    /// Returns the visible and accessible action label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether this action is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns the visual button variant.
    pub const fn variant(&self) -> ButtonVariant {
        self.variant
    }

    /// Returns whether this action is the default initial focus target.
    pub const fn default_focus(&self) -> bool {
        self.default_focus
    }

    /// Returns the accessibility role.
    pub const fn role(&self) -> Role {
        Role::Button
    }
}

/// Resolved alert dialog color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlertDialogColors {
    pub(crate) barrier: ColorIntent,
    pub(crate) surface: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) muted_foreground: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) trigger_background: ColorIntent,
    pub(crate) trigger_hover_background: ColorIntent,
    pub(crate) trigger_foreground: ColorIntent,
    pub(crate) trigger_border: ColorIntent,
    pub(crate) action_background: ColorIntent,
    pub(crate) action_hover_background: ColorIntent,
    pub(crate) action_foreground: ColorIntent,
    pub(crate) action_border: ColorIntent,
    pub(crate) cancel_background: ColorIntent,
    pub(crate) cancel_hover_background: ColorIntent,
    pub(crate) cancel_foreground: ColorIntent,
    pub(crate) cancel_border: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl AlertDialogColors {
    /// Returns barrier color intent.
    pub const fn barrier(self) -> ColorIntent {
        self.barrier
    }

    /// Returns surface color intent.
    pub const fn surface(self) -> ColorIntent {
        self.surface
    }

    /// Returns foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns muted foreground color intent.
    pub const fn muted_foreground(self) -> ColorIntent {
        self.muted_foreground
    }

    /// Returns border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }

    /// Returns trigger background color intent.
    pub const fn trigger_background(self) -> ColorIntent {
        self.trigger_background
    }

    /// Returns trigger hover background color intent.
    pub const fn trigger_hover_background(self) -> ColorIntent {
        self.trigger_hover_background
    }

    /// Returns trigger foreground color intent.
    pub const fn trigger_foreground(self) -> ColorIntent {
        self.trigger_foreground
    }

    /// Returns trigger border color intent.
    pub const fn trigger_border(self) -> ColorIntent {
        self.trigger_border
    }

    /// Returns primary action background color intent.
    pub const fn action_background(self) -> ColorIntent {
        self.action_background
    }

    /// Returns primary action hover background color intent.
    pub const fn action_hover_background(self) -> ColorIntent {
        self.action_hover_background
    }

    /// Returns primary action foreground color intent.
    pub const fn action_foreground(self) -> ColorIntent {
        self.action_foreground
    }

    /// Returns primary action border color intent.
    pub const fn action_border(self) -> ColorIntent {
        self.action_border
    }

    /// Returns cancel action background color intent.
    pub const fn cancel_background(self) -> ColorIntent {
        self.cancel_background
    }

    /// Returns cancel action hover background color intent.
    pub const fn cancel_hover_background(self) -> ColorIntent {
        self.cancel_hover_background
    }

    /// Returns cancel action foreground color intent.
    pub const fn cancel_foreground(self) -> ColorIntent {
        self.cancel_foreground
    }

    /// Returns cancel action border color intent.
    pub const fn cancel_border(self) -> ColorIntent {
        self.cancel_border
    }

    /// Returns focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved alert dialog metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlertDialogMetrics {
    trigger_height: open_gpui::Pixels,
    trigger_padding_x: open_gpui::Pixels,
    trigger_padding_y: open_gpui::Pixels,
    action_height: open_gpui::Pixels,
    action_padding_x: open_gpui::Pixels,
    action_padding_y: open_gpui::Pixels,
    padding: open_gpui::Pixels,
    radius: open_gpui::Pixels,
    title_size: open_gpui::Pixels,
    text_size: open_gpui::Pixels,
    width: open_gpui::Pixels,
    max_width: open_gpui::Pixels,
    action_gap: open_gpui::Pixels,
}

impl AlertDialogMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            trigger_height: size.button_h(),
            trigger_padding_x: size.button_px(),
            trigger_padding_y: size.button_py(),
            action_height: size.button_h(),
            action_padding_x: size.button_px(),
            action_padding_y: size.button_py(),
            padding: size.button_px(),
            radius: size.control_radius(),
            title_size: px(18.0),
            text_size: size.control_text_px(),
            width: px(440.0),
            max_width: px(580.0),
            action_gap: px(8.0),
        }
    }

    /// Returns trigger height.
    pub const fn trigger_height(self) -> open_gpui::Pixels {
        self.trigger_height
    }

    /// Returns trigger horizontal padding.
    pub const fn trigger_padding_x(self) -> open_gpui::Pixels {
        self.trigger_padding_x
    }

    /// Returns trigger vertical padding.
    pub const fn trigger_padding_y(self) -> open_gpui::Pixels {
        self.trigger_padding_y
    }

    /// Returns action height.
    pub const fn action_height(self) -> open_gpui::Pixels {
        self.action_height
    }

    /// Returns action horizontal padding.
    pub const fn action_padding_x(self) -> open_gpui::Pixels {
        self.action_padding_x
    }

    /// Returns action vertical padding.
    pub const fn action_padding_y(self) -> open_gpui::Pixels {
        self.action_padding_y
    }

    /// Returns surface padding.
    pub const fn padding(self) -> open_gpui::Pixels {
        self.padding
    }

    /// Returns corner radius.
    pub const fn radius(self) -> open_gpui::Pixels {
        self.radius
    }

    /// Returns title text size.
    pub const fn title_size(self) -> open_gpui::Pixels {
        self.title_size
    }

    /// Returns body text size.
    pub const fn text_size(self) -> open_gpui::Pixels {
        self.text_size
    }

    /// Returns preferred surface width.
    pub const fn width(self) -> open_gpui::Pixels {
        self.width
    }

    /// Returns maximum surface width.
    pub const fn max_width(self) -> open_gpui::Pixels {
        self.max_width
    }

    /// Returns gap between actions.
    pub const fn action_gap(self) -> open_gpui::Pixels {
        self.action_gap
    }
}

/// Resolved alert dialog state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct AlertDialogState {
    size: Size,
    disabled: bool,
    open: bool,
    default_open: bool,
    open_mode: AlertDialogOpenMode,
    title: SharedString,
    description: SharedString,
    intent: AlertDialogIntent,
    cancel: AlertDialogActionState,
    action: AlertDialogActionState,
    outside_press_policy: OutsidePressPolicy,
    escape_key_policy: EscapeKeyPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    trigger_selected: bool,
    metrics: AlertDialogMetrics,
    colors: AlertDialogColors,
    focus_ring: FocusRing,
    overlay: GpuiOverlayState,
}

impl AlertDialogState {
    /// Resolves the public state for an alert dialog.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        size: Size,
        disabled: bool,
        open: Option<bool>,
        default_open: bool,
        title: SharedString,
        description: SharedString,
        intent: AlertDialogIntent,
        cancel_label: SharedString,
        cancel_disabled: bool,
        action_label: SharedString,
        action_disabled: bool,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let open_mode = if open.is_some() {
            AlertDialogOpenMode::Controlled
        } else {
            AlertDialogOpenMode::Uncontrolled
        };
        Self::resolve_with_open_mode(
            size,
            disabled,
            open.unwrap_or(default_open),
            default_open,
            open_mode,
            title,
            description,
            intent,
            cancel_label,
            cancel_disabled,
            action_label,
            action_disabled,
            outside_press_policy,
            escape_key_policy,
            initial_focus_intent,
            focus_restore_intent,
            tokens,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_with_open_mode(
        size: Size,
        disabled: bool,
        open: bool,
        default_open: bool,
        open_mode: AlertDialogOpenMode,
        title: SharedString,
        description: SharedString,
        intent: AlertDialogIntent,
        cancel_label: SharedString,
        cancel_disabled: bool,
        action_label: SharedString,
        action_disabled: bool,
        outside_press_policy: OutsidePressPolicy,
        escape_key_policy: EscapeKeyPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let open = open && !disabled;
        let presence = OverlayPresence::from_open(open);
        let overlay = GpuiOverlayAdapterConfig::new(OverlayLayerKind::Modal, presence)
            .outside_press_policy(outside_press_policy)
            .escape_key_policy(escape_key_policy)
            .initial_focus_intent(initial_focus_intent.clone())
            .focus_restore_intent(focus_restore_intent.clone())
            .state();
        let colors = ThemeResolver::alert_dialog_colors(tokens, intent, open);
        let cancel_disabled = cancel_disabled || disabled;
        let action_disabled = action_disabled || disabled;
        let default_focus = alert_dialog_default_focus_kind(
            &initial_focus_intent,
            !cancel_disabled,
            !action_disabled,
        );

        Self {
            size,
            disabled,
            open,
            default_open,
            open_mode,
            title,
            description,
            intent,
            cancel: AlertDialogActionState::new(
                AlertDialogActionKind::Cancel,
                cancel_label,
                cancel_disabled,
                ButtonVariant::Secondary,
                default_focus == Some(AlertDialogActionKind::Cancel),
            ),
            action: AlertDialogActionState::new(
                AlertDialogActionKind::Action,
                action_label,
                action_disabled,
                intent.action_variant(),
                default_focus == Some(AlertDialogActionKind::Action),
            ),
            outside_press_policy,
            escape_key_policy,
            initial_focus_intent,
            focus_restore_intent,
            trigger_selected: open,
            metrics: AlertDialogMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.focus_ring()),
            overlay,
        }
    }

    /// Returns foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the trigger is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether alert dialog content is open.
    pub const fn open(&self) -> bool {
        self.open
    }

    /// Returns uncontrolled initial open state.
    pub const fn default_open(&self) -> bool {
        self.default_open
    }

    /// Returns open-state ownership.
    pub const fn open_mode(&self) -> AlertDialogOpenMode {
        self.open_mode
    }

    /// Returns alert dialog title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns alert dialog description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the semantic primary-action intent.
    pub const fn intent(&self) -> AlertDialogIntent {
        self.intent
    }

    /// Returns cancel action metadata.
    pub const fn cancel(&self) -> &AlertDialogActionState {
        &self.cancel
    }

    /// Returns primary action metadata.
    pub const fn action(&self) -> &AlertDialogActionState {
        &self.action
    }

    /// Returns outside-press policy.
    pub const fn outside_press_policy(&self) -> OutsidePressPolicy {
        self.outside_press_policy
    }

    /// Returns Escape-key policy.
    pub const fn escape_key_policy(&self) -> EscapeKeyPolicy {
        self.escape_key_policy
    }

    /// Returns initial focus intent.
    pub const fn initial_focus_intent(&self) -> &InitialFocusIntent {
        &self.initial_focus_intent
    }

    /// Returns focus restore intent.
    pub const fn focus_restore_intent(&self) -> &FocusRestoreIntent {
        &self.focus_restore_intent
    }

    /// Returns whether the trigger should present selected/expanded state.
    pub const fn trigger_selected(&self) -> bool {
        self.trigger_selected
    }

    /// Returns trigger role.
    pub const fn trigger_role(&self) -> Role {
        Role::Button
    }

    /// Returns content role.
    pub const fn content_role(&self) -> Role {
        Role::AlertDialog
    }

    /// Returns whether activation handlers should run.
    pub const fn activation_enabled(&self) -> bool {
        !self.disabled
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> AlertDialogMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> AlertDialogColors {
        self.colors
    }

    /// Returns resolved focus-ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }

    /// Returns resolved overlay adapter state.
    pub const fn overlay(&self) -> &GpuiOverlayState {
        &self.overlay
    }
}

/// A concrete GPUI alert dialog component.
#[derive(IntoElement)]
pub struct AlertDialog {
    id: ElementId,
    trigger_label: SharedString,
    title: SharedString,
    description: SharedString,
    cancel_label: SharedString,
    action_label: SharedString,
    intent: AlertDialogIntent,
    size: Size,
    disabled: bool,
    cancel_disabled: bool,
    action_disabled: bool,
    open: Option<bool>,
    default_open: bool,
    outside_press_policy: OutsidePressPolicy,
    escape_key_policy: EscapeKeyPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_cancel: Option<ActionHandler>,
    on_action: Option<ActionHandler>,
    on_open_change: Option<OpenChangeHandler>,
}

#[derive(Debug, Clone)]
struct AlertDialogRuntime {
    open: bool,
    trigger_focus: FocusHandle,
    cancel_focus: FocusHandle,
    action_focus: FocusHandle,
}

impl AlertDialog {
    /// Creates an alert dialog with title, description, and a primary action label.
    pub fn new(
        id: impl Into<ElementId>,
        trigger_label: impl Into<SharedString>,
        title: impl Into<SharedString>,
        description: impl Into<SharedString>,
        action_label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            trigger_label: trigger_label.into(),
            title: title.into(),
            description: description.into(),
            cancel_label: "Cancel".into(),
            action_label: action_label.into(),
            intent: AlertDialogIntent::Default,
            size: Size::Medium,
            disabled: false,
            cancel_disabled: false,
            action_disabled: false,
            open: None,
            default_open: false,
            outside_press_policy: OutsidePressPolicy::Consume,
            escape_key_policy: EscapeKeyPolicy::Dismiss,
            initial_focus_intent: InitialFocusIntent::TargetOrFirstFocusable(
                OverlayFocusTarget::new(CANCEL_FOCUS_TARGET),
            ),
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_cancel: None,
            on_action: None,
            on_open_change: None,
        }
    }

    /// Applies a semantic primary-action intent.
    pub fn intent(mut self, intent: AlertDialogIntent) -> Self {
        self.intent = intent;
        self
    }

    /// Applies the visible cancel action label.
    pub fn cancel_label(mut self, label: impl Into<SharedString>) -> Self {
        self.cancel_label = label.into();
        self
    }

    /// Marks the alert dialog trigger as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the cancel action as disabled.
    pub fn cancel_disabled(mut self, disabled: bool) -> Self {
        self.cancel_disabled = disabled;
        self
    }

    /// Marks the primary action as disabled.
    pub fn action_disabled(mut self, disabled: bool) -> Self {
        self.action_disabled = disabled;
        self
    }

    /// Applies controlled open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Applies uncontrolled initial open state.
    pub fn default_open(mut self, default_open: bool) -> Self {
        self.default_open = default_open;
        self
    }

    /// Applies outside-press policy.
    pub fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = policy;
        self
    }

    /// Applies Escape-key policy.
    pub fn escape_key_policy(mut self, policy: EscapeKeyPolicy) -> Self {
        self.escape_key_policy = policy;
        self
    }

    /// Applies initial focus intent.
    pub fn initial_focus_intent(mut self, intent: InitialFocusIntent) -> Self {
        self.initial_focus_intent = intent;
        self
    }

    /// Applies focus restore intent.
    pub fn focus_restore_intent(mut self, intent: FocusRestoreIntent) -> Self {
        self.focus_restore_intent = intent;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a cancel action handler.
    pub fn on_cancel(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }

    /// Registers a primary action handler.
    pub fn on_action(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }

    /// Registers an open-change handler with the next open value.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved alert dialog state.
    pub fn state(&self) -> AlertDialogState {
        AlertDialogState::resolve(
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.title.clone(),
            self.description.clone(),
            self.intent,
            self.cancel_label.clone(),
            self.cancel_disabled,
            self.action_label.clone(),
            self.action_disabled,
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        )
    }
}

impl Sizable for AlertDialog {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for AlertDialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, cx| AlertDialogRuntime {
            open: self.default_open,
            trigger_focus: cx.focus_handle(),
            cancel_focus: cx.focus_handle(),
            action_focus: cx.focus_handle(),
        });
        let runtime_open = runtime.read(cx).open;
        let controlled_open = self.open;
        let resolved_open = controlled_open.unwrap_or(runtime_open);

        if controlled_open.is_some() && runtime_open != resolved_open {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let state = AlertDialogState::resolve(
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.title.clone(),
            self.description.clone(),
            self.intent,
            self.cancel_label.clone(),
            self.cancel_disabled,
            self.action_label.clone(),
            self.action_disabled,
            self.outside_press_policy,
            self.escape_key_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        let state = if state.open() == resolved_open {
            state
        } else {
            AlertDialogState::resolve_with_open_mode(
                self.size,
                self.disabled,
                resolved_open,
                self.default_open,
                state.open_mode(),
                self.title.clone(),
                self.description.clone(),
                self.intent,
                self.cancel_label.clone(),
                self.cancel_disabled,
                self.action_label.clone(),
                self.action_disabled,
                self.outside_press_policy,
                self.escape_key_policy,
                self.initial_focus_intent.clone(),
                self.focus_restore_intent.clone(),
                self.tokens,
            )
        };
        let viewport = window.viewport_size();
        let id = self.id;
        let trigger_id: ElementId = (id.clone(), "trigger").into();
        let content_id: ElementId = (id.clone(), "content").into();
        let trigger_label = self.trigger_label;
        let metrics = state.metrics();
        let colors = state.colors();
        let focus_ring = state.focus_ring();
        let disabled = state.disabled();
        let open = state.open();
        let trigger_focus = runtime.read(cx).trigger_focus.clone();
        let cancel_focus = runtime.read(cx).cancel_focus.clone();
        let action_focus = runtime.read(cx).action_focus.clone();
        let on_cancel = self.on_cancel;
        let on_action = self.on_action;
        let on_open_change = self.on_open_change;

        div()
            .id(id)
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div()
                    .id(trigger_id)
                    .min_h(metrics.trigger_height())
                    .px(metrics.trigger_padding_x())
                    .py(metrics.trigger_padding_y())
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(metrics.radius())
                    .border_1()
                    .border_color(ThemeResolver::resolve(colors.trigger_border()))
                    .bg(ThemeResolver::resolve(colors.trigger_background()))
                    .text_color(ThemeResolver::resolve(colors.trigger_foreground()))
                    .text_size(metrics.text_size())
                    .line_height(metrics.text_size())
                    .focusable()
                    .track_focus(&trigger_focus)
                    .tab_stop(!disabled)
                    .role(state.trigger_role())
                    .aria_label(trigger_label.clone())
                    .aria_selected(state.trigger_selected())
                    .aria_expanded(open)
                    .aria_disabled(disabled)
                    .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
                    .when(open, |this| {
                        let runtime = runtime.clone();
                        let on_open_change = on_open_change.clone();
                        let focus_restore = state.focus_restore_intent().clone();
                        let escape_policy = state.overlay().policy().clone();
                        this.on_key_down(move |event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key.as_str() == "escape"
                                && escape_open_change(&escape_policy).is_some()
                            {
                                cx.stop_propagation();
                                window.prevent_default();
                                close_alert_dialog(
                                    runtime.clone(),
                                    focus_restore.clone(),
                                    on_open_change.clone(),
                                    window,
                                    cx,
                                );
                            }
                        })
                    })
                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                    .when(!disabled, |this| {
                        let runtime = runtime.clone();
                        let on_open_change = on_open_change.clone();
                        let initial_focus = state.initial_focus_intent().clone();
                        let focus_state = state.clone();
                        this.cursor_pointer()
                            .hover(move |style| {
                                style.bg(ThemeResolver::resolve(colors.trigger_hover_background()))
                            })
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                let next_open = !open;
                                runtime.update(cx, |runtime, _| {
                                    runtime.open = next_open;
                                });
                                if next_open
                                    && let Some(focus) = alert_dialog_initial_focus_handle(
                                        &runtime,
                                        &focus_state,
                                        &initial_focus,
                                        cx,
                                    )
                                {
                                    window.defer(cx, move |window, cx| focus.focus(window, cx));
                                }
                                if let Some(on_open_change) = on_open_change.as_ref() {
                                    on_open_change(next_open, window, cx);
                                }
                            })
                    })
                    .child(trigger_label),
            )
            .when(open, |this| {
                this.child(
                    deferred(
                        anchored()
                            .position(point(px(0.0), px(0.0)))
                            .snap_to_window()
                            .child(alert_dialog_layer_element(
                                content_id.clone(),
                                state.clone(),
                                viewport,
                                runtime.clone(),
                                cancel_focus.clone(),
                                action_focus.clone(),
                                on_cancel.clone(),
                                on_action.clone(),
                                on_open_change.clone(),
                            )),
                    )
                    .priority(state.overlay().deferred_priority()),
                )
            })
    }
}

fn alert_dialog_layer_element(
    content_id: ElementId,
    state: AlertDialogState,
    viewport: open_gpui::Size<open_gpui::Pixels>,
    runtime: Entity<AlertDialogRuntime>,
    cancel_focus: FocusHandle,
    action_focus: FocusHandle,
    on_cancel: Option<ActionHandler>,
    on_action: Option<ActionHandler>,
    on_open_change: Option<OpenChangeHandler>,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let outside_change = outside_press_open_change(state.overlay().policy());
    let escape_change = escape_open_change(state.overlay().policy());
    let x = ((viewport.width - metrics.width()) / 2.0).max(px(12.0));
    let y = (viewport.height / 10.0).max(px(24.0));

    div()
        .id(content_id)
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .w(viewport.width)
        .h(viewport.height)
        .bg(ThemeResolver::resolve(colors.barrier()))
        .occlude()
        .on_any_mouse_down(|_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .when(outside_change.is_some(), |this| {
            let runtime = runtime.clone();
            let on_open_change = on_open_change.clone();
            let focus_restore = state.focus_restore_intent().clone();
            this.on_click(move |_: &ClickEvent, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
                close_alert_dialog(
                    runtime.clone(),
                    focus_restore.clone(),
                    on_open_change.clone(),
                    window,
                    cx,
                );
            })
        })
        .child(
            div()
                .id("alert-dialog-surface")
                .absolute()
                .left(x)
                .top(y)
                .w(metrics.width())
                .max_w(metrics.max_width())
                .p(metrics.padding())
                .flex()
                .flex_col()
                .gap_3()
                .rounded(metrics.radius())
                .border_1()
                .border_color(ThemeResolver::resolve(colors.border()))
                .bg(ThemeResolver::resolve(colors.surface()))
                .text_color(ThemeResolver::resolve(colors.foreground()))
                .text_size(metrics.text_size())
                .line_height(metrics.text_size())
                .shadow_lg()
                .occlude()
                .on_any_mouse_down(|_, _, cx| {
                    cx.stop_propagation();
                })
                .tab_group()
                .focusable()
                .role(state.content_role())
                .aria_label(state.title().to_owned())
                .on_key_down({
                    let runtime = runtime.clone();
                    let on_open_change = on_open_change.clone();
                    let focus_restore = state.focus_restore_intent().clone();
                    move |event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key.as_str() == "escape" && escape_change.is_some() {
                            cx.stop_propagation();
                            window.prevent_default();
                            close_alert_dialog(
                                runtime.clone(),
                                focus_restore.clone(),
                                on_open_change.clone(),
                                window,
                                cx,
                            );
                        }
                    }
                })
                .child(
                    div()
                        .text_size(metrics.title_size())
                        .font_weight(open_gpui::FontWeight::BOLD)
                        .line_height(px(24.0))
                        .child(state.title().to_owned()),
                )
                .child(
                    div()
                        .text_color(ThemeResolver::resolve(colors.muted_foreground()))
                        .child(state.description().to_owned()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(metrics.action_gap())
                        .child(alert_dialog_cancel_button(
                            &state,
                            runtime.clone(),
                            cancel_focus.clone(),
                            on_cancel,
                            on_open_change.clone(),
                        ))
                        .child(alert_dialog_action_button(
                            &state,
                            runtime.clone(),
                            action_focus.clone(),
                            on_action,
                            on_open_change,
                        )),
                ),
        )
}

fn alert_dialog_cancel_button(
    state: &AlertDialogState,
    runtime: Entity<AlertDialogRuntime>,
    cancel_focus: FocusHandle,
    on_cancel: Option<ActionHandler>,
    on_open_change: Option<OpenChangeHandler>,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let focus_ring = state.focus_ring();
    let cancel = state.cancel().clone();
    let focus_restore = state.focus_restore_intent().clone();

    div()
        .id("alert-dialog-cancel")
        .min_h(metrics.action_height())
        .px(metrics.action_padding_x())
        .py(metrics.action_padding_y())
        .flex()
        .items_center()
        .justify_center()
        .rounded(metrics.radius())
        .border_1()
        .border_color(ThemeResolver::resolve(colors.cancel_border()))
        .bg(ThemeResolver::resolve(colors.cancel_background()))
        .text_color(ThemeResolver::resolve(colors.cancel_foreground()))
        .text_size(metrics.text_size())
        .line_height(metrics.text_size())
        .focusable()
        .track_focus(&cancel_focus)
        .tab_stop(cancel.activation_enabled())
        .role(cancel.role())
        .aria_label(cancel.label().to_owned())
        .aria_disabled(cancel.disabled())
        .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
        .when(cancel.disabled(), |this| {
            this.opacity(0.56).cursor_not_allowed()
        })
        .when(!cancel.disabled(), |this| {
            this.cursor_pointer()
                .hover(move |style| {
                    style.bg(ThemeResolver::resolve(colors.cancel_hover_background()))
                })
                .on_click(move |_event: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    if let Some(on_cancel) = on_cancel.as_ref() {
                        on_cancel(window, cx);
                    }
                    close_alert_dialog(
                        runtime.clone(),
                        focus_restore.clone(),
                        on_open_change.clone(),
                        window,
                        cx,
                    );
                })
        })
        .child(cancel.label().to_owned())
}

fn alert_dialog_action_button(
    state: &AlertDialogState,
    runtime: Entity<AlertDialogRuntime>,
    action_focus: FocusHandle,
    on_action: Option<ActionHandler>,
    on_open_change: Option<OpenChangeHandler>,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let focus_ring = state.focus_ring();
    let action = state.action().clone();
    let focus_restore = state.focus_restore_intent().clone();

    div()
        .id("alert-dialog-action")
        .min_h(metrics.action_height())
        .px(metrics.action_padding_x())
        .py(metrics.action_padding_y())
        .flex()
        .items_center()
        .justify_center()
        .rounded(metrics.radius())
        .border_1()
        .border_color(ThemeResolver::resolve(colors.action_border()))
        .bg(ThemeResolver::resolve(colors.action_background()))
        .text_color(ThemeResolver::resolve(colors.action_foreground()))
        .text_size(metrics.text_size())
        .line_height(metrics.text_size())
        .focusable()
        .track_focus(&action_focus)
        .tab_stop(action.activation_enabled())
        .role(action.role())
        .aria_label(action.label().to_owned())
        .aria_disabled(action.disabled())
        .focus_visible(move |style| style.shadow(focus_ring_shadow(focus_ring)))
        .when(action.disabled(), |this| {
            this.opacity(0.56).cursor_not_allowed()
        })
        .when(!action.disabled(), |this| {
            this.cursor_pointer()
                .hover(move |style| {
                    style.bg(ThemeResolver::resolve(colors.action_hover_background()))
                })
                .on_click(move |_event: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    if let Some(on_action) = on_action.as_ref() {
                        on_action(window, cx);
                    }
                    close_alert_dialog(
                        runtime.clone(),
                        focus_restore.clone(),
                        on_open_change.clone(),
                        window,
                        cx,
                    );
                })
        })
        .child(action.label().to_owned())
}

fn close_alert_dialog(
    runtime: Entity<AlertDialogRuntime>,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<OpenChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let trigger_focus = runtime.read(cx).trigger_focus.clone();
    runtime.update(cx, |runtime, _| {
        runtime.open = false;
    });
    if let Some(on_open_change) = on_open_change.as_ref() {
        on_open_change(false, window, cx);
    }
    if focus_restore_requests_trigger(&focus_restore) {
        trigger_focus.focus(window, cx);
    }
}

fn alert_dialog_default_focus_kind(
    intent: &InitialFocusIntent,
    cancel_enabled: bool,
    action_enabled: bool,
) -> Option<AlertDialogActionKind> {
    let first_focusable = || {
        if cancel_enabled {
            Some(AlertDialogActionKind::Cancel)
        } else if action_enabled {
            Some(AlertDialogActionKind::Action)
        } else {
            None
        }
    };
    let target_focus = |target: &OverlayFocusTarget| match target.as_str() {
        CANCEL_FOCUS_TARGET if cancel_enabled => Some(AlertDialogActionKind::Cancel),
        ACTION_FOCUS_TARGET if action_enabled => Some(AlertDialogActionKind::Action),
        _ => None,
    };

    match intent {
        InitialFocusIntent::None => None,
        InitialFocusIntent::FirstFocusable => first_focusable(),
        InitialFocusIntent::Target(target) => target_focus(target),
        InitialFocusIntent::TargetOrFirstFocusable(target) => {
            target_focus(target).or_else(first_focusable)
        }
    }
}

fn alert_dialog_initial_focus_handle(
    runtime: &Entity<AlertDialogRuntime>,
    state: &AlertDialogState,
    intent: &InitialFocusIntent,
    cx: &App,
) -> Option<FocusHandle> {
    match alert_dialog_default_focus_kind(
        intent,
        state.cancel().activation_enabled(),
        state.action().activation_enabled(),
    ) {
        Some(AlertDialogActionKind::Cancel) => Some(runtime.read(cx).cancel_focus.clone()),
        Some(AlertDialogActionKind::Action) => Some(runtime.read(cx).action_focus.clone()),
        None => None,
    }
}

fn focus_restore_requests_trigger(intent: &FocusRestoreIntent) -> bool {
    matches!(
        intent,
        FocusRestoreIntent::Trigger | FocusRestoreIntent::TriggerOrFallback(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_resolver_preserves_uncontrolled_open_mode_with_runtime_open() {
        let state = AlertDialogState::resolve_with_open_mode(
            Size::Medium,
            false,
            true,
            false,
            AlertDialogOpenMode::Uncontrolled,
            "Confirm".into(),
            "Runtime opened".into(),
            AlertDialogIntent::Default,
            "Cancel".into(),
            false,
            "Continue".into(),
            false,
            OutsidePressPolicy::Consume,
            EscapeKeyPolicy::Dismiss,
            InitialFocusIntent::FirstFocusable,
            FocusRestoreIntent::Trigger,
            ThemeTokens::default(),
        );

        assert!(state.open());
        assert!(!state.default_open());
        assert_eq!(state.open_mode(), AlertDialogOpenMode::Uncontrolled);
    }

    #[test]
    fn alert_dialog_initial_focus_skips_disabled_actions() {
        let state = AlertDialogState::resolve(
            Size::Medium,
            false,
            Some(true),
            false,
            "Confirm".into(),
            "Runtime opened".into(),
            AlertDialogIntent::Destructive,
            "Cancel".into(),
            true,
            "Delete".into(),
            false,
            OutsidePressPolicy::Consume,
            EscapeKeyPolicy::Dismiss,
            InitialFocusIntent::FirstFocusable,
            FocusRestoreIntent::Trigger,
            ThemeTokens::default(),
        );

        assert!(!state.cancel().activation_enabled());
        assert!(state.action().activation_enabled());
        assert!(!state.cancel().default_focus());
        assert!(state.action().default_focus());

        let exact_cancel_state = AlertDialogState::resolve(
            Size::Medium,
            false,
            Some(true),
            false,
            "Confirm".into(),
            "Runtime opened".into(),
            AlertDialogIntent::Default,
            "Cancel".into(),
            true,
            "Continue".into(),
            false,
            OutsidePressPolicy::Consume,
            EscapeKeyPolicy::Dismiss,
            InitialFocusIntent::Target(OverlayFocusTarget::new(CANCEL_FOCUS_TARGET)),
            FocusRestoreIntent::Trigger,
            ThemeTokens::default(),
        );

        assert!(!exact_cancel_state.cancel().default_focus());
        assert!(!exact_cancel_state.action().default_focus());
    }
}
