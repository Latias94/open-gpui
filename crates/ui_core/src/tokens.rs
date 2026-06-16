//! Token vocabulary for the Open GPUI component ecosystem.

use std::fmt;

/// A stable token key used by component themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TokenKey(&'static str);

impl TokenKey {
    /// Creates a new token key.
    pub const fn new(key: &'static str) -> Self {
        Self(key)
    }

    /// Returns the raw token string.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl AsRef<str> for TokenKey {
    fn as_ref(&self) -> &str {
        self.0
    }
}

impl fmt::Display for TokenKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Semantic token names that every theme should be able to provide.
pub mod semantic {
    use super::TokenKey;

    /// Default surface token.
    pub const SURFACE: TokenKey = TokenKey::new("semantic.surface");
    /// Muted surface token.
    pub const SURFACE_MUTED: TokenKey = TokenKey::new("semantic.surface_muted");
    /// Default border token.
    pub const BORDER: TokenKey = TokenKey::new("semantic.border");
    /// Text token.
    pub const TEXT: TokenKey = TokenKey::new("semantic.text");
    /// Muted text token.
    pub const TEXT_MUTED: TokenKey = TokenKey::new("semantic.text_muted");
    /// Accent token.
    pub const ACCENT: TokenKey = TokenKey::new("semantic.accent");
    /// Accent foreground token.
    pub const ACCENT_FOREGROUND: TokenKey = TokenKey::new("semantic.accent_foreground");
    /// Focus ring token.
    pub const FOCUS_RING: TokenKey = TokenKey::new("semantic.focus_ring");
    /// Destructive token.
    pub const DESTRUCTIVE: TokenKey = TokenKey::new("semantic.destructive");
    /// Destructive foreground token.
    pub const DESTRUCTIVE_FOREGROUND: TokenKey = TokenKey::new("semantic.destructive_foreground");
    /// Overlay scrim token.
    pub const OVERLAY: TokenKey = TokenKey::new("semantic.overlay");
    /// Modal scrim token.
    pub const MODAL_OVERLAY: TokenKey = TokenKey::new("semantic.modal_overlay");

    /// All default semantic tokens in `ThemeTokens` field order.
    pub const ALL: [TokenKey; 12] = [
        SURFACE,
        SURFACE_MUTED,
        BORDER,
        TEXT,
        TEXT_MUTED,
        ACCENT,
        ACCENT_FOREGROUND,
        FOCUS_RING,
        DESTRUCTIVE,
        DESTRUCTIVE_FOREGROUND,
        OVERLAY,
        MODAL_OVERLAY,
    ];
}

/// Default token bundle for the first theme surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeTokens {
    /// Default surface token.
    pub surface: TokenKey,
    /// Muted surface token.
    pub surface_muted: TokenKey,
    /// Border token.
    pub border: TokenKey,
    /// Text token.
    pub text: TokenKey,
    /// Muted text token.
    pub text_muted: TokenKey,
    /// Accent token.
    pub accent: TokenKey,
    /// Accent foreground token.
    pub accent_foreground: TokenKey,
    /// Focus ring token.
    pub focus_ring: TokenKey,
    /// Destructive token.
    pub destructive: TokenKey,
    /// Destructive foreground token.
    pub destructive_foreground: TokenKey,
    /// Overlay scrim token.
    pub overlay: TokenKey,
    /// Modal scrim token.
    pub modal_overlay: TokenKey,
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self {
            surface: semantic::SURFACE,
            surface_muted: semantic::SURFACE_MUTED,
            border: semantic::BORDER,
            text: semantic::TEXT,
            text_muted: semantic::TEXT_MUTED,
            accent: semantic::ACCENT,
            accent_foreground: semantic::ACCENT_FOREGROUND,
            focus_ring: semantic::FOCUS_RING,
            destructive: semantic::DESTRUCTIVE,
            destructive_foreground: semantic::DESTRUCTIVE_FOREGROUND,
            overlay: semantic::OVERLAY,
            modal_overlay: semantic::MODAL_OVERLAY,
        }
    }
}

impl ThemeTokens {
    /// Returns the token bundle in stable semantic field order.
    pub const fn all(self) -> [TokenKey; 12] {
        [
            self.surface,
            self.surface_muted,
            self.border,
            self.text,
            self.text_muted,
            self.accent,
            self.accent_foreground,
            self.focus_ring,
            self.destructive,
            self.destructive_foreground,
            self.overlay,
            self.modal_overlay,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_keys_render_raw_names() {
        assert_eq!(semantic::SURFACE.as_str(), "semantic.surface");
        assert_eq!(semantic::FOCUS_RING.to_string(), "semantic.focus_ring");
    }

    #[test]
    fn default_theme_tokens_match_the_semantic_registry() {
        let tokens = ThemeTokens::default();
        assert_eq!(tokens.surface, semantic::SURFACE);
        assert_eq!(tokens.focus_ring, semantic::FOCUS_RING);
        assert_eq!(tokens.modal_overlay, semantic::MODAL_OVERLAY);
    }

    #[test]
    fn theme_tokens_expose_stable_field_order() {
        assert_eq!(ThemeTokens::default().all(), semantic::ALL);
    }
}
