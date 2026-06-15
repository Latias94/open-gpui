//! Shared component color intents.

use open_gpui_ui_core::TokenKey;

/// Stable component color intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorIntent {
    token: TokenKey,
    fallback_rgb: u32,
}

impl ColorIntent {
    /// Creates a color intent from a token key and temporary RGB fallback.
    pub const fn new(token: TokenKey, fallback_rgb: u32) -> Self {
        Self {
            token,
            fallback_rgb,
        }
    }

    /// Returns the semantic token key.
    pub const fn token(self) -> TokenKey {
        self.token
    }

    /// Returns the temporary fallback RGB value.
    pub const fn fallback_rgb(self) -> u32 {
        self.fallback_rgb
    }
}
