//! Shared component color intents.

use open_gpui_ui_core::TokenKey;

/// Component color state within a semantic token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ColorState {
    /// Default token value.
    #[default]
    Default,
    /// Hovered token value.
    Hover,
    /// Selected token value.
    Selected,
    /// Disabled token value.
    Disabled,
    /// Read-only token value.
    ReadOnly,
    /// Invalid token value.
    Invalid,
    /// Required marker token value.
    Required,
    /// Placeholder text token value.
    Placeholder,
    /// Supporting message token value.
    Message,
    /// Focus-visible token value.
    FocusVisible,
    /// Non-modal overlay token value.
    Overlay,
    /// Modal overlay token value.
    ModalOverlay,
}

impl ColorState {
    /// Returns the stable state label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Hover => "hover",
            Self::Selected => "selected",
            Self::Disabled => "disabled",
            Self::ReadOnly => "read-only",
            Self::Invalid => "invalid",
            Self::Required => "required",
            Self::Placeholder => "placeholder",
            Self::Message => "message",
            Self::FocusVisible => "focus-visible",
            Self::Overlay => "overlay",
            Self::ModalOverlay => "modal-overlay",
        }
    }
}

/// Stable component color intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorIntent {
    token: TokenKey,
    state: ColorState,
    fallback_rgb: u32,
}

impl ColorIntent {
    /// Creates a color intent from a token key and temporary RGB fallback.
    pub const fn new(token: TokenKey, fallback_rgb: u32) -> Self {
        Self::with_state(token, ColorState::Default, fallback_rgb)
    }

    /// Creates a color intent from a token key, state, and temporary RGB fallback.
    pub const fn with_state(token: TokenKey, state: ColorState, fallback_rgb: u32) -> Self {
        Self {
            token,
            state,
            fallback_rgb,
        }
    }

    /// Returns the semantic token key.
    pub const fn token(self) -> TokenKey {
        self.token
    }

    /// Returns the component color state.
    pub const fn state(self) -> ColorState {
        self.state
    }

    /// Returns the temporary fallback RGB value.
    pub const fn fallback_rgb(self) -> u32 {
        self.fallback_rgb
    }
}
