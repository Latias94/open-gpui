//! Token foundation page metadata.

use open_gpui_ui_core::{ThemeTokens, TokenKey, semantic};

/// Page title.
pub const TITLE: &str = "Tokens";
/// Page summary.
pub const SUMMARY: &str = "Semantic theme keys that future styled components must resolve.";
/// Foundation signals rendered by this page.
pub const SIGNALS: &[&str] = &[
    "ThemeTokens::default()",
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
    [
        TokenSample::new("Surface", tokens.surface, 0xffffff),
        TokenSample::new("Muted surface", tokens.surface_muted, 0xf6f7f2),
        TokenSample::new("Border", tokens.border, 0xd6d8ce),
        TokenSample::new("Text", tokens.text, 0x18202a),
        TokenSample::new("Muted text", tokens.text_muted, 0x5a6472),
        TokenSample::new("Accent", tokens.accent, 0x1f7a66),
        TokenSample::new("Accent foreground", tokens.accent_foreground, 0xffffff),
        TokenSample::new("Focus ring", tokens.focus_ring, 0x2f80ed),
        TokenSample::new("Destructive", tokens.destructive, 0xc24132),
        TokenSample::new(
            "Destructive foreground",
            tokens.destructive_foreground,
            0xffffff,
        ),
        TokenSample::new("Overlay", tokens.overlay, 0x263240),
        TokenSample::new("Modal overlay", tokens.modal_overlay, 0x111827),
    ]
}

/// Returns true when the token set matches the default semantic registry.
pub fn matches_semantic_registry(tokens: ThemeTokens) -> bool {
    tokens.surface == semantic::SURFACE
        && tokens.surface_muted == semantic::SURFACE_MUTED
        && tokens.border == semantic::BORDER
        && tokens.text == semantic::TEXT
        && tokens.text_muted == semantic::TEXT_MUTED
        && tokens.accent == semantic::ACCENT
        && tokens.accent_foreground == semantic::ACCENT_FOREGROUND
        && tokens.focus_ring == semantic::FOCUS_RING
        && tokens.destructive == semantic::DESTRUCTIVE
        && tokens.destructive_foreground == semantic::DESTRUCTIVE_FOREGROUND
        && tokens.overlay == semantic::OVERLAY
        && tokens.modal_overlay == semantic::MODAL_OVERLAY
}
