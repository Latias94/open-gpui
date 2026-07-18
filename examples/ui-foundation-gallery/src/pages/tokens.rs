//! Token foundation page metadata.

use open_gpui_ui_components::{ColorState, ThemeMode, ThemeSnapshot};
use open_gpui_ui_core::{ThemeTokens, TokenKey, semantic};

/// Page title.
pub const TITLE: &str = "Tokens";
/// Page summary.
pub const SUMMARY: &str = "Semantic theme keys that future styled components must resolve.";
/// Foundation signals rendered by this page.
pub const SIGNALS: &[&str] = &[
    "ThemeTokens::default()",
    "ThemeSnapshot::light()",
    "ThemeSnapshot::dark()",
    "ThemeSnapshot::high_contrast()",
    "ThemeScope::new(stable_id, context, child)",
    "deferred overlay opening ThemeContext",
    "semantic::SURFACE",
    "semantic::TEXT",
    "semantic::FOCUS_RING",
    "semantic::MODAL_OVERLAY",
];

/// One semantic token shown in the gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenSample {
    /// Human-readable token label.
    pub label: &'static str,
    /// Stable semantic token key.
    pub key: TokenKey,
    /// Preview color used by the gallery shell.
    pub preview_rgb: u32,
}

impl TokenSample {
    const fn new(label: &'static str, key: TokenKey, preview_rgb: u32) -> Self {
        Self {
            label,
            key,
            preview_rgb,
        }
    }
}

/// Returns semantic token samples in the same order as `ThemeTokens`.
pub fn token_samples(tokens: ThemeTokens) -> [TokenSample; 12] {
    token_samples_for_theme(tokens, ThemeSnapshot::light())
}

/// Returns semantic token samples resolved by a theme snapshot.
pub fn token_samples_for_theme(tokens: ThemeTokens, theme: ThemeSnapshot<'_>) -> [TokenSample; 12] {
    [
        TokenSample::new(
            "Surface",
            tokens.surface,
            preview_rgb(theme, tokens.surface),
        ),
        TokenSample::new(
            "Muted surface",
            tokens.surface_muted,
            preview_rgb(theme, tokens.surface_muted),
        ),
        TokenSample::new("Border", tokens.border, preview_rgb(theme, tokens.border)),
        TokenSample::new("Text", tokens.text, preview_rgb(theme, tokens.text)),
        TokenSample::new(
            "Muted text",
            tokens.text_muted,
            preview_rgb(theme, tokens.text_muted),
        ),
        TokenSample::new("Accent", tokens.accent, preview_rgb(theme, tokens.accent)),
        TokenSample::new(
            "Accent foreground",
            tokens.accent_foreground,
            preview_rgb(theme, tokens.accent_foreground),
        ),
        TokenSample::new(
            "Focus ring",
            tokens.focus_ring,
            preview_rgb(theme, tokens.focus_ring),
        ),
        TokenSample::new(
            "Destructive",
            tokens.destructive,
            preview_rgb(theme, tokens.destructive),
        ),
        TokenSample::new(
            "Destructive foreground",
            tokens.destructive_foreground,
            preview_rgb(theme, tokens.destructive_foreground),
        ),
        TokenSample::new(
            "Overlay",
            tokens.overlay,
            preview_rgb(theme, tokens.overlay),
        ),
        TokenSample::new(
            "Modal overlay",
            tokens.modal_overlay,
            preview_rgb(theme, tokens.modal_overlay),
        ),
    ]
}

/// One theme mode summary shown in the gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeModeSample {
    /// Theme mode represented by this sample.
    pub mode: ThemeMode,
    /// Snapshot revision used for cache invalidation.
    pub revision: u64,
    /// Resolved surface color.
    pub surface_rgb: u32,
    /// Resolved text color.
    pub text_rgb: u32,
    /// Resolved accent color.
    pub accent_rgb: u32,
    /// Resolved focus ring color.
    pub focus_ring_rgb: u32,
}

impl ThemeModeSample {
    fn new(tokens: ThemeTokens, theme: ThemeSnapshot<'_>) -> Self {
        Self {
            mode: theme.mode(),
            revision: theme.revision(),
            surface_rgb: preview_rgb(theme, tokens.surface),
            text_rgb: preview_rgb(theme, tokens.text),
            accent_rgb: preview_rgb(theme, tokens.accent),
            focus_ring_rgb: theme
                .color_rgb(tokens.focus_ring, ColorState::FocusVisible)
                .unwrap_or_else(|| preview_rgb(theme, tokens.focus_ring)),
        }
    }
}

/// Returns light, dark, and high-contrast theme metadata.
pub fn theme_mode_samples(tokens: ThemeTokens) -> [ThemeModeSample; 3] {
    [
        ThemeModeSample::new(tokens, ThemeSnapshot::light()),
        ThemeModeSample::new(tokens, ThemeSnapshot::dark()),
        ThemeModeSample::new(tokens, ThemeSnapshot::high_contrast()),
    ]
}

fn preview_rgb(theme: ThemeSnapshot<'_>, token: TokenKey) -> u32 {
    theme
        .color_rgb(token, ColorState::Default)
        .unwrap_or(0xff00ff)
}

/// Returns true when the token set matches the default semantic registry.
pub fn matches_semantic_registry(tokens: ThemeTokens) -> bool {
    tokens.all() == semantic::ALL
}
