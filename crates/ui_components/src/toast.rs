//! Toast notification stack component.

use crate::a11y::UiA11yElementExt;
use crate::feedback::{FeedbackColors, FeedbackIntent};
use crate::focus::{FocusRing, focus_ring_shadow};
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, UiPx, ui_px};
use std::rc::Rc;
use std::time::Duration;

const DEFAULT_MAX_VISIBLE_TOASTS: usize = 3;
const DEFAULT_TOAST_TIMEOUT: Duration = Duration::from_secs(5);

/// Semantic intent for a toast.
pub type ToastIntent = FeedbackIntent;

/// Resolved toast color intents.
pub type ToastColors = FeedbackColors;

/// Pure descriptor for one toast notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    id: String,
    title: String,
    description: Option<String>,
    intent: ToastIntent,
    timeout: Option<Duration>,
    elapsed: Duration,
    action_label: Option<String>,
    dismissible: bool,
}

impl Toast {
    /// Creates a toast descriptor with a stable id and title.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            intent: ToastIntent::Neutral,
            timeout: Some(DEFAULT_TOAST_TIMEOUT),
            elapsed: Duration::ZERO,
            action_label: None,
            dismissible: true,
        }
    }

    /// Adds supporting description copy.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Applies semantic feedback intent.
    pub fn intent(mut self, intent: ToastIntent) -> Self {
        self.intent = intent;
        self
    }

    /// Applies an auto-dismiss timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Pins the toast until explicitly dismissed.
    pub fn pinned(mut self) -> Self {
        self.timeout = None;
        self
    }

    /// Records elapsed lifetime for pure timeout resolution.
    pub fn elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = elapsed;
        self
    }

    /// Adds a single action affordance.
    pub fn action(mut self, label: impl Into<String>) -> Self {
        self.action_label = Some(label.into());
        self
    }

    /// Sets whether the toast can be manually dismissed.
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    /// Returns the stable toast id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the toast title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns supporting description copy.
    pub fn description_text(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Returns feedback intent.
    pub const fn intent_value(&self) -> ToastIntent {
        self.intent
    }

    /// Returns auto-dismiss timeout.
    pub const fn timeout_value(&self) -> Option<Duration> {
        self.timeout
    }

    /// Returns elapsed lifetime.
    pub const fn elapsed_value(&self) -> Duration {
        self.elapsed
    }

    /// Returns action label.
    pub fn action_label(&self) -> Option<&str> {
        self.action_label.as_deref()
    }

    /// Returns whether manual dismiss is enabled.
    pub const fn dismissible_value(&self) -> bool {
        self.dismissible
    }

    /// Returns whether the toast has reached its timeout.
    pub fn timed_out(&self) -> bool {
        self.timeout
            .map(|timeout| self.elapsed >= timeout)
            .unwrap_or(false)
    }

    /// Returns remaining timeout duration.
    pub fn remaining_timeout(&self) -> Option<Duration> {
        self.timeout
            .map(|timeout| timeout.saturating_sub(self.elapsed))
    }
}

/// Reason a toast was dismissed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastDismissReason {
    /// The user explicitly dismissed the toast.
    Manual,
    /// The toast reached its timeout.
    Timeout,
}

impl ToastDismissReason {
    /// Returns the stable reason label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Timeout => "timeout",
        }
    }
}

/// Toast action payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastAction {
    id: String,
    label: String,
}

impl ToastAction {
    /// Creates a toast action payload.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }

    /// Returns the toast id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the action label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Toast dismiss payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastDismiss {
    id: String,
    reason: ToastDismissReason,
}

impl ToastDismiss {
    /// Creates a toast dismiss payload.
    pub fn new(id: impl Into<String>, reason: ToastDismissReason) -> Self {
        Self {
            id: id.into(),
            reason,
        }
    }

    /// Returns the toast id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the dismiss reason.
    pub const fn reason(&self) -> ToastDismissReason {
        self.reason
    }
}

/// Resolved toast metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToastMetrics {
    min_width: UiPx,
    max_width: UiPx,
    padding: UiPx,
    gap: UiPx,
    radius: UiPx,
    marker_size: UiPx,
    title_size: UiPx,
    description_size: UiPx,
    action_height: UiPx,
    dismiss_size: UiPx,
}

impl ToastMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            min_width: match size {
                Size::XSmall | Size::Small => ui_px(260.0),
                Size::Medium => ui_px(320.0),
                Size::Large => ui_px(360.0),
            },
            max_width: match size {
                Size::XSmall | Size::Small => ui_px(320.0),
                Size::Medium => ui_px(400.0),
                Size::Large => ui_px(460.0),
            },
            padding: size.button_px(),
            gap: ui_px(8.0),
            radius: size.control_radius(),
            marker_size: match size {
                Size::XSmall => ui_px(6.0),
                Size::Small => ui_px(7.0),
                Size::Medium => ui_px(8.0),
                Size::Large => ui_px(9.0),
            },
            title_size: size.control_text_px(),
            description_size: size.control_text_px(),
            action_height: size.button_h(),
            dismiss_size: size.icon_button_size(),
        }
    }

    /// Returns minimum toast width.
    pub const fn min_width(self) -> UiPx {
        self.min_width
    }

    /// Returns maximum toast width.
    pub const fn max_width(self) -> UiPx {
        self.max_width
    }

    /// Returns toast padding.
    pub const fn padding(self) -> UiPx {
        self.padding
    }

    /// Returns content gap.
    pub const fn gap(self) -> UiPx {
        self.gap
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns marker diameter.
    pub const fn marker_size(self) -> UiPx {
        self.marker_size
    }

    /// Returns title text size.
    pub const fn title_size(self) -> UiPx {
        self.title_size
    }

    /// Returns description text size.
    pub const fn description_size(self) -> UiPx {
        self.description_size
    }

    /// Returns action button height.
    pub const fn action_height(self) -> UiPx {
        self.action_height
    }

    /// Returns dismiss affordance size.
    pub const fn dismiss_size(self) -> UiPx {
        self.dismiss_size
    }
}

/// Resolved toast item state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastState {
    source_index: usize,
    stack_index: usize,
    id: String,
    title: String,
    description: Option<String>,
    intent: ToastIntent,
    timeout: Option<Duration>,
    remaining_timeout: Option<Duration>,
    action_label: Option<String>,
    dismissible: bool,
    metrics: ToastMetrics,
    colors: ToastColors,
    focus_ring: FocusRing,
}

impl ToastState {
    /// Resolves public state for a toast item.
    pub fn resolve(
        source_index: usize,
        stack_index: usize,
        toast: Toast,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        let remaining_timeout = toast.remaining_timeout();
        let colors = ThemeResolver::feedback_colors(tokens, toast.intent);

        Self {
            source_index,
            stack_index,
            id: toast.id,
            title: toast.title,
            description: toast.description,
            intent: toast.intent,
            timeout: toast.timeout,
            remaining_timeout,
            action_label: toast.action_label,
            dismissible: toast.dismissible,
            metrics: ToastMetrics::from_size(size),
            colors,
            focus_ring: FocusRing::from_color(colors.marker()),
        }
    }

    /// Returns the source queue index.
    pub const fn source_index(&self) -> usize {
        self.source_index
    }

    /// Returns the visible stack index.
    pub const fn stack_index(&self) -> usize {
        self.stack_index
    }

    /// Returns the stable toast id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the toast title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns supporting description copy.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Returns feedback intent.
    pub const fn intent(&self) -> ToastIntent {
        self.intent
    }

    /// Returns auto-dismiss timeout.
    pub const fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Returns remaining timeout duration.
    pub const fn remaining_timeout(&self) -> Option<Duration> {
        self.remaining_timeout
    }

    /// Returns whether the toast has an action affordance.
    pub const fn has_action(&self) -> bool {
        self.action_label.is_some()
    }

    /// Returns action label.
    pub fn action_label(&self) -> Option<&str> {
        self.action_label.as_deref()
    }

    /// Returns whether manual dismiss is enabled.
    pub const fn dismissible(&self) -> bool {
        self.dismissible
    }

    /// Returns accessibility role.
    pub const fn role(&self) -> Role {
        Role::Section
    }

    /// Returns action button role.
    pub const fn action_role(&self) -> Role {
        Role::Button
    }

    /// Returns dismiss button role.
    pub const fn dismiss_role(&self) -> Role {
        Role::Button
    }

    /// Returns the action payload.
    pub fn action(&self) -> Option<ToastAction> {
        self.action_label
            .as_ref()
            .map(|label| ToastAction::new(self.id.clone(), label.clone()))
    }

    /// Returns the dismiss payload for a reason when manual dismiss is enabled.
    pub fn dismiss(&self, reason: ToastDismissReason) -> Option<ToastDismiss> {
        (self.dismissible || reason == ToastDismissReason::Timeout)
            .then(|| ToastDismiss::new(self.id.clone(), reason))
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> ToastMetrics {
        self.metrics
    }

    /// Returns resolved colors.
    pub const fn colors(&self) -> ToastColors {
        self.colors
    }

    /// Returns focus ring metadata.
    pub const fn focus_ring(&self) -> FocusRing {
        self.focus_ring
    }
}

/// Resolved toast stack state.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastStackState {
    label: String,
    size: Size,
    max_visible: usize,
    toasts: Vec<Toast>,
    visible_toasts: Vec<ToastState>,
    expired_dismissals: Vec<ToastDismiss>,
    overflow_count: usize,
    metrics: ToastMetrics,
    tokens: ThemeTokens,
}

impl ToastStackState {
    /// Creates an empty toast stack state.
    pub fn new(label: impl Into<String>) -> Self {
        Self::resolve(
            label,
            Size::Medium,
            DEFAULT_MAX_VISIBLE_TOASTS,
            Vec::<Toast>::new(),
            ThemeTokens::default(),
        )
    }

    /// Resolves public state for a toast stack.
    pub fn resolve(
        label: impl Into<String>,
        size: Size,
        max_visible: usize,
        toasts: impl IntoIterator<Item = Toast>,
        tokens: ThemeTokens,
    ) -> Self {
        let label = label.into();
        let max_visible = max_visible.max(1);
        let toasts: Vec<Toast> = toasts.into_iter().collect();
        let active: Vec<(usize, Toast)> = toasts
            .iter()
            .cloned()
            .enumerate()
            .filter(|(_, toast)| !toast.timed_out())
            .collect();
        let expired_dismissals = toasts
            .iter()
            .filter(|toast| toast.timed_out())
            .map(|toast| ToastDismiss::new(toast.id(), ToastDismissReason::Timeout))
            .collect::<Vec<_>>();
        let visible_toasts = active
            .iter()
            .rev()
            .take(max_visible)
            .enumerate()
            .map(|(stack_index, (source_index, toast))| {
                ToastState::resolve(*source_index, stack_index, toast.clone(), size, tokens)
            })
            .collect::<Vec<_>>();
        let overflow_count = active.len().saturating_sub(visible_toasts.len());

        Self {
            label,
            size,
            max_visible,
            toasts,
            visible_toasts,
            expired_dismissals,
            overflow_count,
            metrics: ToastMetrics::from_size(size),
            tokens,
        }
    }

    /// Adds or replaces a toast by id.
    pub fn add(mut self, toast: Toast) -> Self {
        self.toasts.retain(|existing| existing.id() != toast.id());
        self.toasts.push(toast);
        self.rebuild()
    }

    /// Removes one toast by id.
    pub fn dismiss(mut self, id: &str) -> Self {
        self.toasts.retain(|toast| toast.id() != id);
        self.rebuild()
    }

    /// Removes all toasts.
    pub fn dismiss_all(mut self) -> Self {
        self.toasts.clear();
        self.rebuild()
    }

    /// Removes toasts that have reached their timeout.
    pub fn prune_expired(mut self) -> Self {
        self.toasts.retain(|toast| !toast.timed_out());
        self.rebuild()
    }

    /// Returns the stack label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns maximum visible toast count.
    pub const fn max_visible(&self) -> usize {
        self.max_visible
    }

    /// Returns accessibility role.
    pub const fn role(&self) -> Role {
        Role::Section
    }

    /// Returns all queued toast descriptors, including expired toasts.
    pub fn toasts(&self) -> &[Toast] {
        &self.toasts
    }

    /// Returns visible, non-expired toast states.
    pub fn visible_toasts(&self) -> &[ToastState] {
        &self.visible_toasts
    }

    /// Returns timeout dismissals discovered during resolution.
    pub fn expired_dismissals(&self) -> &[ToastDismiss] {
        &self.expired_dismissals
    }

    /// Returns how many active toasts are hidden by the visible stack cap.
    pub const fn overflow_count(&self) -> usize {
        self.overflow_count
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> ToastMetrics {
        self.metrics
    }

    fn rebuild(self) -> Self {
        Self::resolve(
            self.label,
            self.size,
            self.max_visible,
            self.toasts,
            self.tokens,
        )
    }
}

type ToastActionHandler = Rc<dyn Fn(ToastAction, &ClickEvent, &mut Window, &mut App)>;
type ToastDismissHandler = Rc<dyn Fn(ToastDismiss, &ClickEvent, &mut Window, &mut App)>;

/// A concrete GPUI toast stack component.
#[derive(IntoElement)]
pub struct ToastStack {
    id: ElementId,
    label: SharedString,
    size: Size,
    max_visible: usize,
    tokens: ThemeTokens,
    toasts: Vec<Toast>,
    on_action: Option<ToastActionHandler>,
    on_dismiss: Option<ToastDismissHandler>,
}

impl ToastStack {
    /// Creates a toast stack with an accessible label.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            size: Size::Medium,
            max_visible: DEFAULT_MAX_VISIBLE_TOASTS,
            tokens: ThemeTokens::default(),
            toasts: Vec::new(),
            on_action: None,
            on_dismiss: None,
        }
    }

    /// Adds one toast descriptor.
    pub fn toast(mut self, toast: Toast) -> Self {
        self.toasts.push(toast);
        self
    }

    /// Adds many toast descriptors.
    pub fn toasts(mut self, toasts: impl IntoIterator<Item = Toast>) -> Self {
        self.toasts.extend(toasts);
        self
    }

    /// Sets the visible stack cap.
    pub fn max_visible(mut self, max_visible: usize) -> Self {
        self.max_visible = max_visible.max(1);
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a toast action handler.
    pub fn on_action(
        mut self,
        handler: impl Fn(ToastAction, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }

    /// Registers a toast dismiss handler.
    pub fn on_dismiss(
        mut self,
        handler: impl Fn(ToastDismiss, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }

    /// Returns the resolved toast stack state.
    pub fn state(&self) -> ToastStackState {
        ToastStackState::resolve(
            self.label.to_string(),
            self.size,
            self.max_visible,
            self.toasts.clone(),
            self.tokens,
        )
    }
}

impl Sizable for ToastStack {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for ToastStack {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let state = self.state();
        let stack_metrics = state.metrics();
        let overflow_colors = ThemeResolver::feedback_colors(state.tokens, ToastIntent::Neutral);
        let debug_id = self.id.to_string();
        let on_action = self.on_action.clone();
        let on_dismiss = self.on_dismiss.clone();

        div()
            .id(self.id)
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("toast-stack:{debug_id}:root")
            })
            .ui_role(state.role())
            .aria_label(self.label)
            .flex()
            .flex_col()
            .items_end()
            .gap(gpui_px_from_ui(stack_metrics.gap()))
            .children(state.visible_toasts().iter().map(|toast| {
                let metrics = toast.metrics();
                let colors = toast.colors();
                let focus_ring = toast.focus_ring();
                let action = toast.action();
                let dismiss = toast.dismiss(ToastDismissReason::Manual);
                let on_action = on_action.clone();
                let on_dismiss = on_dismiss.clone();
                let toast_id = toast.id().to_owned();
                let title = toast.title().to_owned();
                let description = toast.description().map(str::to_owned);
                let action_label = toast.action_label().map(str::to_owned);

                div()
                    .id(format!("toast:{toast_id}"))
                    .debug_selector({
                        let debug_id = debug_id.clone();
                        let toast_id = toast_id.clone();
                        move || format!("toast-stack:{debug_id}:toast:{toast_id}")
                    })
                    .min_w(gpui_px_from_ui(metrics.min_width()))
                    .max_w(gpui_px_from_ui(metrics.max_width()))
                    .p(gpui_px_from_ui(metrics.padding()))
                    .flex()
                    .items_start()
                    .gap(gpui_px_from_ui(metrics.gap()))
                    .rounded(gpui_px_from_ui(metrics.radius()))
                    .border_1()
                    .border_color(ThemeResolver::resolve(colors.border()))
                    .bg(ThemeResolver::resolve(colors.background()))
                    .text_color(ThemeResolver::resolve(colors.foreground()))
                    .ui_role(toast.role())
                    .aria_label(title.clone())
                    .child(
                        div()
                            .mt(gpui_px_from_ui(ui_px(3.0)))
                            .w(gpui_px_from_ui(metrics.marker_size()))
                            .h(gpui_px_from_ui(metrics.marker_size()))
                            .rounded(gpui_px_from_ui(ui_px(999.0)))
                            .bg(ThemeResolver::resolve(colors.marker())),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(gpui_px_from_ui(metrics.title_size()))
                                    .line_height(gpui_px_from_ui(metrics.title_size()))
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(title),
                            )
                            .when_some(description, |this, description| {
                                this.child(
                                    div()
                                        .text_color(ThemeResolver::resolve(
                                            colors.muted_foreground(),
                                        ))
                                        .text_size(gpui_px_from_ui(metrics.description_size()))
                                        .line_height(gpui_px_from_ui(metrics.description_size()))
                                        .child(description),
                                )
                            })
                            .when_some(action_label.zip(action), |this, (label, action)| {
                                this.child(
                                    div()
                                        .id(format!("toast-action:{toast_id}"))
                                        .min_h(gpui_px_from_ui(metrics.action_height()))
                                        .self_start()
                                        .px(gpui_px_from_ui(ui_px(8.0)))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(gpui_px_from_ui(metrics.radius()))
                                        .border_1()
                                        .border_color(ThemeResolver::resolve(colors.border()))
                                        .focusable()
                                        .tab_stop(true)
                                        .ui_role(Role::Button)
                                        .aria_label(label.clone())
                                        .focus_visible(move |style| {
                                            style.shadow(focus_ring_shadow(focus_ring))
                                        })
                                        .cursor_pointer()
                                        .when_some(on_action.clone(), |this, on_action| {
                                            this.on_click(move |event, window, cx| {
                                                cx.stop_propagation();
                                                on_action(action.clone(), event, window, cx);
                                            })
                                        })
                                        .child(label),
                                )
                            }),
                    )
                    .when_some(dismiss, |this, dismiss| {
                        this.child(
                            div()
                                .id(format!("toast-dismiss:{toast_id}"))
                                .size(gpui_px_from_ui(metrics.dismiss_size()))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(gpui_px_from_ui(metrics.radius()))
                                .focusable()
                                .tab_stop(true)
                                .ui_role(Role::Button)
                                .aria_label("Dismiss notification")
                                .focus_visible(move |style| {
                                    style.shadow(focus_ring_shadow(focus_ring))
                                })
                                .cursor_pointer()
                                .when_some(on_dismiss.clone(), |this, on_dismiss| {
                                    this.on_click(move |event, window, cx| {
                                        cx.stop_propagation();
                                        on_dismiss(dismiss.clone(), event, window, cx);
                                    })
                                })
                                .child("x"),
                        )
                    })
                    .into_any_element()
            }))
            .when(state.overflow_count() > 0, |this| {
                this.child(
                    div()
                        .text_size(gpui_px_from_ui(stack_metrics.description_size()))
                        .text_color(ThemeResolver::resolve(overflow_colors.muted_foreground()))
                        .child(format!("+{} more", state.overflow_count())),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::semantic;

    #[test]
    fn toast_descriptor_models_timeout_action_and_dismissibility() {
        let toast = Toast::new("saved", "Saved")
            .description("Settings were saved.")
            .intent(ToastIntent::Success)
            .timeout(Duration::from_secs(2))
            .elapsed(Duration::from_secs(1))
            .action("Undo")
            .dismissible(false);

        assert_eq!(toast.id(), "saved");
        assert_eq!(toast.description_text(), Some("Settings were saved."));
        assert_eq!(toast.intent_value(), ToastIntent::Success);
        assert_eq!(toast.remaining_timeout(), Some(Duration::from_secs(1)));
        assert_eq!(toast.action_label(), Some("Undo"));
        assert!(!toast.dismissible_value());
        assert!(!toast.timed_out());
    }

    #[test]
    fn toast_stack_add_dismiss_and_prune_keep_state_pure() {
        let state = ToastStackState::new("Notifications")
            .add(Toast::new("one", "One"))
            .add(Toast::new("two", "Two").elapsed(Duration::from_secs(6)));

        assert_eq!(state.toasts().len(), 2);
        assert_eq!(state.visible_toasts().len(), 1);
        assert_eq!(state.expired_dismissals()[0].id(), "two");
        assert_eq!(
            state.expired_dismissals()[0].reason(),
            ToastDismissReason::Timeout
        );

        let pruned = state.prune_expired();
        assert_eq!(pruned.toasts().len(), 1);
        assert_eq!(pruned.expired_dismissals(), &[]);

        let dismissed = pruned.dismiss("one");
        assert!(dismissed.toasts().is_empty());
    }

    #[test]
    fn toast_stack_stacks_newest_visible_items_and_tracks_overflow() {
        let state = ToastStack::new("toasts", "Notifications")
            .max_visible(2)
            .toast(Toast::new("one", "One"))
            .toast(Toast::new("two", "Two"))
            .toast(Toast::new("three", "Three").action("Undo"))
            .state();

        assert_eq!(state.role(), Role::Section);
        assert_eq!(state.visible_toasts().len(), 2);
        assert_eq!(state.visible_toasts()[0].id(), "three");
        assert_eq!(state.visible_toasts()[1].id(), "two");
        assert_eq!(state.overflow_count(), 1);
        assert_eq!(state.visible_toasts()[0].action().unwrap().label(), "Undo");
        assert_eq!(
            state.visible_toasts()[0]
                .dismiss(ToastDismissReason::Manual)
                .unwrap()
                .reason(),
            ToastDismissReason::Manual
        );
        assert_eq!(
            state.visible_toasts()[0].colors().marker().token(),
            semantic::TEXT_MUTED
        );
    }

    #[test]
    fn pinned_toasts_do_not_timeout() {
        let state = ToastStack::new("toasts", "Notifications")
            .toast(
                Toast::new("loading", "Loading")
                    .pinned()
                    .elapsed(Duration::from_secs(60)),
            )
            .state();

        assert_eq!(state.visible_toasts().len(), 1);
        assert!(state.expired_dismissals().is_empty());
        assert_eq!(state.visible_toasts()[0].remaining_timeout(), None);
    }
}
