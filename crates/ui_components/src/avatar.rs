//! Avatar identity primitive.

use crate::a11y::UiA11yElementExt;
use crate::color::ColorIntent;
use crate::geometry::gpui_px_from_ui;
use crate::theme::ThemeResolver;
use open_gpui::{
    ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, div,
};
use open_gpui_ui_core::{Role, Sizable, Size, ThemeTokens, UiPx, ui_px};

/// Renderer-neutral avatar source metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvatarSource {
    uri: String,
}

impl AvatarSource {
    /// Creates a new avatar source metadata value.
    pub fn new(uri: impl Into<String>) -> Self {
        Self { uri: uri.into() }
    }

    /// Returns the source URI supplied by the caller.
    pub fn uri(&self) -> &str {
        &self.uri
    }
}

/// Resolved avatar color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvatarColors {
    pub(crate) background: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) border: ColorIntent,
}

impl AvatarColors {
    /// Returns the avatar fallback background color intent.
    pub const fn background(self) -> ColorIntent {
        self.background
    }

    /// Returns the avatar fallback text color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns the avatar border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }
}

/// Resolved avatar metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AvatarMetrics {
    diameter: UiPx,
    radius: UiPx,
    text_size: UiPx,
}

impl AvatarMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        let diameter = match size {
            Size::XSmall => ui_px(24.0),
            Size::Small => ui_px(28.0),
            Size::Medium => ui_px(32.0),
            Size::Large => ui_px(40.0),
        };

        Self {
            diameter,
            radius: ui_px(diameter.as_f32() / 2.0),
            text_size: match size {
                Size::XSmall => ui_px(10.0),
                Size::Small => ui_px(11.0),
                Size::Medium => ui_px(12.0),
                Size::Large => ui_px(14.0),
            },
        }
    }

    /// Returns the square avatar diameter.
    pub const fn diameter(self) -> UiPx {
        self.diameter
    }

    /// Returns the circular avatar radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns fallback initials text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }
}

/// Resolved avatar state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct AvatarState {
    name: String,
    fallback: String,
    source: Option<AvatarSource>,
    accessible_label: String,
    size: Size,
    metrics: AvatarMetrics,
    colors: AvatarColors,
}

impl AvatarState {
    /// Resolves the public state for an avatar.
    pub fn resolve(
        name: impl Into<String>,
        fallback: Option<String>,
        source: Option<AvatarSource>,
        accessible_label: Option<String>,
        size: Size,
        tokens: ThemeTokens,
    ) -> Self {
        let name = name.into();
        let fallback = resolve_fallback(&name, fallback);
        let accessible_label = resolve_accessible_label(&name, accessible_label);

        Self {
            name,
            fallback,
            source,
            accessible_label,
            size,
            metrics: AvatarMetrics::from_size(size),
            colors: ThemeResolver::avatar_colors(tokens),
        }
    }

    /// Returns the display name used for fallback and default accessibility metadata.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns fallback initials or explicit fallback text.
    pub fn fallback(&self) -> &str {
        &self.fallback
    }

    /// Returns optional renderer-neutral source metadata.
    pub fn source(&self) -> Option<&AvatarSource> {
        self.source.as_ref()
    }

    /// Returns whether source metadata was supplied.
    pub const fn has_source(&self) -> bool {
        self.source.is_some()
    }

    /// Returns the resolved accessible label.
    pub fn accessible_label(&self) -> &str {
        &self.accessible_label
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the accessibility role used by the first GPUI adapter slice.
    pub const fn role(&self) -> Role {
        Role::Label
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> AvatarMetrics {
        self.metrics
    }

    /// Returns resolved color intents.
    pub const fn colors(&self) -> AvatarColors {
        self.colors
    }
}

/// A concrete GPUI avatar component.
#[derive(IntoElement)]
pub struct Avatar {
    id: ElementId,
    name: SharedString,
    fallback: Option<SharedString>,
    source: Option<AvatarSource>,
    accessible_label: Option<SharedString>,
    size: Size,
    tokens: ThemeTokens,
}

impl Avatar {
    /// Creates a new avatar with a display name.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            fallback: None,
            source: None,
            accessible_label: None,
            size: Size::Medium,
            tokens: ThemeTokens::default(),
        }
    }

    /// Applies renderer-neutral source metadata.
    pub fn source(mut self, uri: impl Into<String>) -> Self {
        self.source = Some(AvatarSource::new(uri));
        self
    }

    /// Applies explicit fallback text.
    pub fn fallback(mut self, fallback: impl Into<SharedString>) -> Self {
        self.fallback = Some(fallback.into());
        self
    }

    /// Applies an explicit accessible label.
    pub fn accessible_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns the resolved avatar state.
    pub fn state(&self) -> AvatarState {
        AvatarState::resolve(
            self.name.to_string(),
            self.fallback.as_ref().map(ToString::to_string),
            self.source.clone(),
            self.accessible_label.as_ref().map(ToString::to_string),
            self.size,
            self.tokens,
        )
    }
}

impl Sizable for Avatar {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut open_gpui::Window, _cx: &mut open_gpui::App) -> impl IntoElement {
        let state = self.state();
        let metrics = state.metrics();
        let colors = state.colors();
        let debug_id = self.id.to_string();
        let label = state.accessible_label().to_owned();
        let fallback = state.fallback().to_owned();

        div()
            .id(self.id)
            .debug_selector(move || format!("avatar:{debug_id}:root"))
            .w(gpui_px_from_ui(metrics.diameter()))
            .h(gpui_px_from_ui(metrics.diameter()))
            .min_w(gpui_px_from_ui(metrics.diameter()))
            .min_h(gpui_px_from_ui(metrics.diameter()))
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .overflow_hidden()
            .rounded(gpui_px_from_ui(metrics.radius()))
            .border_1()
            .border_color(ThemeResolver::resolve(colors.border()))
            .bg(ThemeResolver::resolve(colors.background()))
            .text_color(ThemeResolver::resolve(colors.foreground()))
            .text_size(gpui_px_from_ui(metrics.text_size()))
            .line_height(gpui_px_from_ui(metrics.text_size()))
            .ui_role(state.role())
            .aria_label(label)
            .child(fallback)
    }
}

fn resolve_fallback(name: &str, explicit: Option<String>) -> String {
    if let Some(fallback) = explicit.map(|value| value.trim().to_owned()) {
        if !fallback.is_empty() {
            return fallback;
        }
    }

    derive_initials(name)
}

fn resolve_accessible_label(name: &str, explicit: Option<String>) -> String {
    if let Some(label) = explicit.map(|value| value.trim().to_owned()) {
        if !label.is_empty() {
            return label;
        }
    }

    let label = name.trim();
    if label.is_empty() {
        "Avatar".to_owned()
    } else {
        label.to_owned()
    }
}

fn derive_initials(name: &str) -> String {
    let mut parts = name.split_whitespace().filter(|part| !part.is_empty());
    let Some(first_part) = parts.next() else {
        return "?".to_owned();
    };

    let mut initials = String::new();
    if let Some(first) = first_part.chars().next() {
        initials.extend(first.to_uppercase());
    }

    if let Some(second_part) = parts.next() {
        if let Some(second) = second_part.chars().next() {
            initials.extend(second.to_uppercase());
        }
    } else if let Some(second) = first_part.chars().nth(1) {
        initials.extend(second.to_uppercase());
    }

    if initials.is_empty() {
        "?".to_owned()
    } else {
        initials
    }
}
