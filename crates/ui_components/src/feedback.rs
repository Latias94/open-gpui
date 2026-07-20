//! Quiet feedback surfaces for application shells.

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    div,
};
use open_gpui_ui_core::{
    LivePoliteness, Role, SemanticDescriptor, Sizable, Size, ThemeTokens, UiPx, ui_px,
};

/// Semantic intent for quiet shell feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FeedbackIntent {
    /// Neutral information or empty-state copy.
    #[default]
    Neutral,
    /// Informational cue.
    Info,
    /// Successful or healthy state.
    Success,
    /// Warning or degraded state that does not need interruption.
    Warning,
    /// Error or unresolved state.
    Danger,
}

impl FeedbackIntent {
    /// Returns the stable intent label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Danger => "danger",
        }
    }
}

/// Resolved feedback color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedbackColors {
    pub(crate) background: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) muted_foreground: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) marker: ColorIntent,
}

impl FeedbackColors {
    /// Returns surface background color intent.
    pub const fn background(self) -> ColorIntent {
        self.background
    }

    /// Returns primary foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns supporting foreground color intent.
    pub const fn muted_foreground(self) -> ColorIntent {
        self.muted_foreground
    }

    /// Returns border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }

    /// Returns marker color intent.
    pub const fn marker(self) -> ColorIntent {
        self.marker
    }
}

/// Resolved status-cue metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusCueMetrics {
    min_height: UiPx,
    marker_size: UiPx,
    padding_x: UiPx,
    padding_y: UiPx,
    gap: UiPx,
    radius: UiPx,
    text_size: UiPx,
}

impl StatusCueMetrics {
    /// Resolves status-cue metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            min_height: size.button_h(),
            marker_size: match size {
                Size::XSmall => ui_px(6.0),
                Size::Small => ui_px(7.0),
                Size::Medium => ui_px(8.0),
                Size::Large => ui_px(9.0),
            },
            padding_x: size.button_px(),
            padding_y: size.button_py(),
            gap: ui_px(8.0),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
        }
    }

    /// Returns minimum cue height.
    pub const fn min_height(self) -> UiPx {
        self.min_height
    }

    /// Returns marker diameter.
    pub const fn marker_size(self) -> UiPx {
        self.marker_size
    }

    /// Returns horizontal padding.
    pub const fn padding_x(self) -> UiPx {
        self.padding_x
    }

    /// Returns vertical padding.
    pub const fn padding_y(self) -> UiPx {
        self.padding_y
    }

    /// Returns content gap.
    pub const fn gap(self) -> UiPx {
        self.gap
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }
}

/// Resolved status-cue state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusCueState {
    intent: FeedbackIntent,
    label: String,
    live: LivePoliteness,
    live_atomic: bool,
    busy: bool,
    size: Size,
    metrics: StatusCueMetrics,
    colors: FeedbackColors,
}

impl StatusCueState {
    /// Resolves public state for a compact status cue.
    pub fn resolve(
        intent: FeedbackIntent,
        label: impl Into<String>,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        Self::resolve_with_overrides(intent, label, size, tokens, None, None, false)
    }

    fn resolve_with_overrides(
        intent: FeedbackIntent,
        label: impl Into<String>,
        size: Size,
        tokens: ThemeTokens,
        live: Option<LivePoliteness>,
        live_atomic: Option<bool>,
        busy: bool,
    ) -> Self {
        let default_live = if intent == FeedbackIntent::Danger {
            LivePoliteness::Assertive
        } else {
            LivePoliteness::Polite
        };
        Self {
            intent,
            label: label.into(),
            live: live.unwrap_or(default_live),
            live_atomic: live_atomic.unwrap_or(true),
            busy,
            size,
            metrics: StatusCueMetrics::from_size(size),
            colors: ThemeResolver::feedback_colors(tokens, intent),
        }
    }

    /// Returns feedback intent.
    pub const fn intent(&self) -> FeedbackIntent {
        self.intent
    }

    /// Returns visible and accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether this cue is display-only.
    pub const fn display_only(&self) -> bool {
        true
    }

    /// Returns accessibility role.
    pub const fn role(&self) -> Role {
        match self.intent {
            FeedbackIntent::Danger => Role::Alert,
            FeedbackIntent::Neutral
            | FeedbackIntent::Info
            | FeedbackIntent::Success
            | FeedbackIntent::Warning => Role::Status,
        }
    }

    /// Returns the live-region priority resolved for this cue.
    pub const fn live(&self) -> LivePoliteness {
        self.live
    }

    /// Returns whether the complete live-region value is announced atomically.
    pub const fn live_atomic(&self) -> bool {
        self.live_atomic
    }

    /// Returns this state with an explicit live-region priority.
    pub fn with_live(mut self, live: LivePoliteness) -> Self {
        self.live = live;
        self
    }

    /// Returns this state with explicit live-region atomicity.
    pub fn with_live_atomic(mut self, live_atomic: bool) -> Self {
        self.live_atomic = live_atomic;
        self
    }

    /// Returns this state with an explicit busy fact.
    pub fn with_busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Returns whether the status region is waiting for a related operation to settle.
    pub const fn busy(&self) -> bool {
        self.busy
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> StatusCueMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> FeedbackColors {
        self.colors
    }
}

/// A concrete GPUI status cue component.
#[derive(IntoElement)]
pub struct StatusCue {
    id: ElementId,
    label: SharedString,
    intent: FeedbackIntent,
    live: Option<LivePoliteness>,
    live_atomic: Option<bool>,
    busy: bool,
    size: Size,
    tokens: ThemeTokens,
}

impl StatusCue {
    /// Creates a status cue.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            intent: FeedbackIntent::Neutral,
            live: None,
            live_atomic: None,
            busy: false,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
        }
    }

    /// Applies semantic feedback intent.
    pub fn intent(mut self, intent: FeedbackIntent) -> Self {
        self.intent = intent;
        self
    }

    /// Overrides the default live-region priority for this cue.
    pub fn live(mut self, live: LivePoliteness) -> Self {
        self.live = Some(live);
        self
    }

    /// Overrides whether the complete live-region value is announced atomically.
    pub fn live_atomic(mut self, live_atomic: bool) -> Self {
        self.live_atomic = Some(live_atomic);
        self
    }

    /// Marks the status region as waiting for a related operation to settle.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns resolved status-cue state.
    pub fn state(&self) -> StatusCueState {
        StatusCueState::resolve_with_overrides(
            self.intent,
            self.label.to_string(),
            self.size,
            self.tokens,
            self.live,
            self.live_atomic,
            self.busy,
        )
    }
}

impl Sizable for StatusCue {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for StatusCue {
    fn render(self, window: &mut open_gpui::Window, cx: &mut open_gpui::App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let debug_id = self.id.to_string();
        let semantics = SemanticDescriptor::new(state.role())
            .with_live_text(state.label())
            .with_live(state.live())
            .with_live_atomic(state.live_atomic())
            .with_busy(state.busy());

        div()
            .id(self.id)
            .debug_selector(move || format!("status-cue:{debug_id}:root"))
            .min_h(gpui_px_from_ui(metrics.min_height()))
            .px(gpui_px_from_ui(metrics.padding_x()))
            .py(gpui_px_from_ui(metrics.padding_y()))
            .flex()
            .items_center()
            .gap(gpui_px_from_ui(metrics.gap()))
            .rounded(gpui_px_from_ui(metrics.radius()))
            .border_1()
            .border_color(theme.resolve(colors.border()))
            .bg(theme.resolve(colors.background()))
            .text_color(theme.resolve(colors.foreground()))
            .text_size(gpui_px_from_ui(metrics.text_size()))
            .line_height(gpui_px_from_ui(metrics.text_size()))
            .ui_semantics(&semantics)
            .child(
                div()
                    .w(gpui_px_from_ui(metrics.marker_size()))
                    .h(gpui_px_from_ui(metrics.marker_size()))
                    .rounded(gpui_px_from_ui(ui_px(999.0)))
                    .bg(theme.resolve(colors.marker())),
            )
            .child(self.label)
    }
}

/// Resolved empty-state metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmptyStateMetrics {
    padding: UiPx,
    gap: UiPx,
    radius: UiPx,
    max_width: UiPx,
    title_size: UiPx,
    description_size: UiPx,
}

impl EmptyStateMetrics {
    /// Resolves empty-state metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            padding: match size {
                Size::XSmall | Size::Small => ui_px(16.0),
                Size::Medium => ui_px(20.0),
                Size::Large => ui_px(24.0),
            },
            gap: ui_px(8.0),
            radius: size.control_radius(),
            max_width: match size {
                Size::XSmall | Size::Small => ui_px(320.0),
                Size::Medium => ui_px(380.0),
                Size::Large => ui_px(440.0),
            },
            title_size: match size {
                Size::XSmall | Size::Small => ui_px(13.0),
                Size::Medium => ui_px(14.0),
                Size::Large => ui_px(16.0),
            },
            description_size: size.control_text_px(),
        }
    }

    /// Returns surface padding.
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

    /// Returns maximum content width.
    pub const fn max_width(self) -> UiPx {
        self.max_width
    }

    /// Returns title text size.
    pub const fn title_size(self) -> UiPx {
        self.title_size
    }

    /// Returns description text size.
    pub const fn description_size(self) -> UiPx {
        self.description_size
    }
}

/// Resolved empty-state state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct EmptyStateState {
    intent: FeedbackIntent,
    title: String,
    description: Option<String>,
    size: Size,
    metrics: EmptyStateMetrics,
    colors: FeedbackColors,
}

impl EmptyStateState {
    /// Resolves public state for an empty state.
    pub fn resolve(
        intent: FeedbackIntent,
        title: impl Into<String>,
        description: Option<impl Into<String>>,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        Self {
            intent,
            title: title.into(),
            description: description.map(Into::into),
            size,
            metrics: EmptyStateMetrics::from_size(size),
            colors: ThemeResolver::feedback_colors(tokens, intent),
        }
    }

    /// Returns feedback intent.
    pub const fn intent(&self) -> FeedbackIntent {
        self.intent
    }

    /// Returns visible and accessible title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns supporting description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Returns foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns accessibility role.
    pub const fn role(&self) -> Role {
        Role::Section
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> EmptyStateMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> FeedbackColors {
        self.colors
    }
}

/// A concrete GPUI empty-state component.
#[derive(IntoElement)]
pub struct EmptyState {
    id: ElementId,
    title: SharedString,
    description: Option<SharedString>,
    intent: FeedbackIntent,
    size: Size,
    tokens: ThemeTokens,
}

impl EmptyState {
    /// Creates an empty state with a title.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            intent: FeedbackIntent::Neutral,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
        }
    }

    /// Applies supporting description copy.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Applies semantic feedback intent.
    pub fn intent(mut self, intent: FeedbackIntent) -> Self {
        self.intent = intent;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns resolved empty-state state.
    pub fn state(&self) -> EmptyStateState {
        EmptyStateState::resolve(
            self.intent,
            self.title.to_string(),
            self.description.as_ref().map(SharedString::to_string),
            self.size,
            self.tokens,
        )
    }
}

impl Sizable for EmptyState {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for EmptyState {
    fn render(self, window: &mut open_gpui::Window, cx: &mut open_gpui::App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let debug_id = self.id.to_string();
        let semantics = match state.description() {
            Some(description) => SemanticDescriptor::new(state.role())
                .with_label(state.title())
                .with_description(description),
            None => SemanticDescriptor::new(state.role()).with_label(state.title()),
        };

        div()
            .id(self.id)
            .debug_selector(move || format!("empty-state:{debug_id}:root"))
            .max_w(gpui_px_from_ui(metrics.max_width()))
            .p(gpui_px_from_ui(metrics.padding()))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(gpui_px_from_ui(metrics.gap()))
            .rounded(gpui_px_from_ui(metrics.radius()))
            .border_1()
            .border_color(theme.resolve(colors.border()))
            .bg(theme.resolve(colors.background()))
            .text_color(theme.resolve(colors.foreground()))
            .ui_semantics(&semantics)
            .child(
                div()
                    .text_size(gpui_px_from_ui(metrics.title_size()))
                    .line_height(gpui_px_from_ui(metrics.title_size()))
                    .font_weight(open_gpui::FontWeight::BOLD)
                    .child(self.title),
            )
            .when_some(self.description, |this, description| {
                this.child(
                    div()
                        .text_center()
                        .text_color(theme.resolve(colors.muted_foreground()))
                        .text_size(gpui_px_from_ui(metrics.description_size()))
                        .line_height(gpui_px_from_ui(metrics.description_size()))
                        .child(description),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_intent_labels_are_stable() {
        assert_eq!(FeedbackIntent::Neutral.as_str(), "neutral");
        assert_eq!(FeedbackIntent::Info.as_str(), "info");
        assert_eq!(FeedbackIntent::Success.as_str(), "success");
        assert_eq!(FeedbackIntent::Warning.as_str(), "warning");
        assert_eq!(FeedbackIntent::Danger.as_str(), "danger");
    }

    #[test]
    fn status_cue_state_preserves_quiet_feedback_contract() {
        let state = StatusCueState::resolve(
            FeedbackIntent::Warning,
            "3 anchors need review",
            Size::Small,
            ThemeTokens::default(),
        );

        assert_eq!(state.intent(), FeedbackIntent::Warning);
        assert_eq!(state.label(), "3 anchors need review");
        assert_eq!(state.role(), Role::Status);
        assert_eq!(state.live(), LivePoliteness::Polite);
        assert!(state.live_atomic());
        assert!(!state.busy());
        assert!(state.display_only());
        assert_eq!(state.metrics().marker_size(), ui_px(7.0));

        let danger = StatusCue::new("danger", "Connection failed")
            .intent(FeedbackIntent::Danger)
            .state();
        assert_eq!(danger.role(), Role::Alert);
        assert_eq!(danger.live(), LivePoliteness::Assertive);

        let quiet = StatusCue::new("quiet", "Static example")
            .live(LivePoliteness::Off)
            .live_atomic(false)
            .busy(true)
            .state();
        assert_eq!(quiet.role(), Role::Status);
        assert_eq!(quiet.live(), LivePoliteness::Off);
        assert!(!quiet.live_atomic());
        assert!(quiet.busy());
    }

    #[test]
    fn empty_state_state_accepts_optional_description() {
        let state = EmptyStateState::resolve(
            FeedbackIntent::Neutral,
            "No search results",
            Some("Try another term"),
            Size::Medium,
            ThemeTokens::default(),
        );

        assert_eq!(state.intent(), FeedbackIntent::Neutral);
        assert_eq!(state.title(), "No search results");
        assert_eq!(state.description(), Some("Try another term"));
        assert_eq!(state.role(), Role::Section);
        assert_eq!(state.metrics().max_width(), ui_px(380.0));
    }
}
