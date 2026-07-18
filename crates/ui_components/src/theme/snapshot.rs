use std::sync::Arc;

use open_gpui::{Rgba, rgb};
use open_gpui_motion::MotionPreference;
use open_gpui_ui_core::{Density, ThemeDesignScales, TokenKey};

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

/// Immutable owned Theme v1 payload before runtime authority selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeSnapshot {
    mode: ThemeMode,
    source_revision: u64,
    colors: Arc<[ThemeColor]>,
    design_scales: ThemeDesignScales,
}

impl ThemeSnapshot {
    /// Creates a complete owned Theme v1 payload.
    pub(super) fn new(
        mode: ThemeMode,
        source_revision: u64,
        colors: impl Into<Arc<[ThemeColor]>>,
        design_scales: ThemeDesignScales,
    ) -> Self {
        Self {
            mode,
            source_revision,
            colors: colors.into(),
            design_scales,
        }
    }

    /// Returns the color mode for this snapshot.
    pub const fn mode(&self) -> ThemeMode {
        self.mode
    }

    /// Returns source-file revision metadata.
    pub const fn source_revision(&self) -> u64 {
        self.source_revision
    }

    /// Returns the raw color table.
    pub fn colors(&self) -> &[ThemeColor] {
        &self.colors
    }

    /// Returns the complete non-color design scales.
    pub const fn design_scales(&self) -> ThemeDesignScales {
        self.design_scales
    }

    /// Returns the theme density default.
    pub const fn density(&self) -> Density {
        self.design_scales.density()
    }

    /// Returns the theme motion policy.
    pub const fn motion_preference(&self) -> MotionPreference {
        self.design_scales.motion()
    }

    /// Returns whether two payloads have identical effective visual and behavior content.
    ///
    /// Source revision is metadata and therefore intentionally excluded.
    pub fn has_same_effective_content(&self, other: &Self) -> bool {
        self.mode == other.mode
            && self.colors == other.colors
            && self.design_scales == other.design_scales
    }

    /// Looks up an RGB color for a token and state.
    pub fn color_rgb(&self, token: TokenKey, state: ColorState) -> Option<u32> {
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
    pub fn resolve_rgb(&self, intent: ColorIntent) -> u32 {
        self.color_rgb(intent.token(), intent.state())
            .unwrap_or_else(|| intent.fallback_rgb())
    }

    /// Resolves a color intent to a concrete GPUI color.
    pub fn resolve(&self, intent: ColorIntent) -> Rgba {
        rgb(self.resolve_rgb(intent))
    }
}
