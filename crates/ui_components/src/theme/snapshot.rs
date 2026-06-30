use open_gpui::{Rgba, rgb};
use open_gpui_ui_core::TokenKey;

use crate::color::{ColorIntent, ColorState};

/// A stable color mode supplied by a component theme snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    /// Light color mode.
    #[default]
    Light,
    /// Dark color mode.
    Dark,
    /// High contrast color mode.
    HighContrast,
}

impl ThemeMode {
    /// Returns the stable mode label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::HighContrast => "high-contrast",
        }
    }
}

/// One runtime theme table color entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeColor {
    token: TokenKey,
    state: ColorState,
    rgb: u32,
}

impl ThemeColor {
    /// Creates a theme color entry for a semantic token and state.
    pub const fn new(token: TokenKey, state: ColorState, rgb: u32) -> Self {
        Self { token, state, rgb }
    }

    /// Returns the semantic token key.
    pub const fn token(self) -> TokenKey {
        self.token
    }

    /// Returns the component color state.
    pub const fn state(self) -> ColorState {
        self.state
    }

    /// Returns the RGB color without alpha.
    pub const fn rgb(self) -> u32 {
        self.rgb
    }
}

/// Immutable runtime view of a component theme table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeSnapshot<'a> {
    mode: ThemeMode,
    revision: u64,
    colors: &'a [ThemeColor],
}

impl<'a> ThemeSnapshot<'a> {
    /// Creates a theme snapshot from a mode, revision, and color table.
    pub const fn new(mode: ThemeMode, revision: u64, colors: &'a [ThemeColor]) -> Self {
        Self {
            mode,
            revision,
            colors,
        }
    }

    /// Returns the color mode for this snapshot.
    pub const fn mode(self) -> ThemeMode {
        self.mode
    }

    /// Returns the revision used by callers to invalidate cached resolutions.
    pub const fn revision(self) -> u64 {
        self.revision
    }

    /// Returns the raw color table.
    pub const fn colors(self) -> &'a [ThemeColor] {
        self.colors
    }

    /// Looks up an RGB color for a token and state.
    pub fn color_rgb(self, token: TokenKey, state: ColorState) -> Option<u32> {
        self.colors
            .iter()
            .find(|entry| entry.token == token && entry.state == state)
            .map(|entry| entry.rgb)
            .or_else(|| {
                self.colors
                    .iter()
                    .find(|entry| entry.token == token && entry.state == ColorState::Default)
                    .map(|entry| entry.rgb)
            })
    }

    /// Resolves a color intent to an RGB value, falling back to the intent RGB when needed.
    pub fn resolve_rgb(self, intent: ColorIntent) -> u32 {
        self.color_rgb(intent.token(), intent.state())
            .unwrap_or_else(|| intent.fallback_rgb())
    }

    /// Resolves a color intent to a concrete GPUI color.
    pub fn resolve(self, intent: ColorIntent) -> Rgba {
        rgb(self.resolve_rgb(intent))
    }
}
