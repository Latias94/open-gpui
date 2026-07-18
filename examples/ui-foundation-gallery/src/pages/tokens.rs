//! Token foundation page metadata.

use open_gpui_motion::MotionPreference;
use open_gpui_ui_components::{ColorState, ThemeContext, ThemeMode};
use open_gpui_ui_core::{Density, ThemeTokens, TokenKey, semantic};

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

/// Returns semantic token samples resolved by a complete theme context.
pub fn token_samples_for_theme(tokens: ThemeTokens, theme: &ThemeContext) -> [TokenSample; 12] {
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
    /// Revision supplied by the theme source as metadata.
    pub source_revision: u64,
    /// Runtime-owned revision used for effective-content invalidation.
    pub effective_revision: u64,
    /// Default control density supplied by the theme.
    pub density: Density,
    /// Motion safety policy supplied by the theme.
    pub motion_policy: MotionPreference,
    /// Control text-size scale in extra-small through large order.
    pub control_text: [u16; 4],
    /// Control radius scale in extra-small through large order.
    pub control_radius: [u16; 4],
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
    fn new(tokens: ThemeTokens, theme: &ThemeContext) -> Self {
        let design = theme.design_scales();
        Self {
            mode: theme.mode(),
            source_revision: theme.source_revision(),
            effective_revision: theme.effective_revision(),
            density: theme.density(),
            motion_policy: theme.motion_preference(),
            control_text: design.typography().control_text().raw_values(),
            control_radius: design.radius().control().raw_values(),
            surface_rgb: preview_rgb(theme, tokens.surface),
            text_rgb: preview_rgb(theme, tokens.text),
            accent_rgb: preview_rgb(theme, tokens.accent),
            focus_ring_rgb: theme
                .snapshot()
                .color_rgb(tokens.focus_ring, ColorState::FocusVisible)
                .unwrap_or_else(|| preview_rgb(theme, tokens.focus_ring)),
        }
    }
}

/// Returns light, dark, and high-contrast theme metadata.
pub fn theme_mode_samples(tokens: ThemeTokens) -> [ThemeModeSample; 3] {
    let light = ThemeContext::light();
    let dark = ThemeContext::dark();
    let high_contrast = ThemeContext::high_contrast();
    [
        ThemeModeSample::new(tokens, &light),
        ThemeModeSample::new(tokens, &dark),
        ThemeModeSample::new(tokens, &high_contrast),
    ]
}

fn preview_rgb(theme: &ThemeContext, token: TokenKey) -> u32 {
    theme
        .snapshot()
        .color_rgb(token, ColorState::Default)
        .unwrap_or(0xff00ff)
}

/// Returns true when the token set matches the default semantic registry.
pub fn matches_semantic_registry(tokens: ThemeTokens) -> bool {
    tokens.all() == semantic::ALL
}
