//! Token vocabulary for the Open GPUI component ecosystem.

use std::fmt;

use open_gpui_motion::MotionPreference;

use crate::{Density, Size, SizeScale, UiPx, ui_px};

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

/// Typography values admitted to the complete theme contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeTypographyScale {
    control_text: SizeScale,
    control_line_height: SizeScale,
}

impl ThemeTypographyScale {
    /// Creates control text-size and line-height scales.
    pub const fn new(control_text: SizeScale, control_line_height: SizeScale) -> Self {
        Self {
            control_text,
            control_line_height,
        }
    }

    /// Returns the control text-size scale.
    pub const fn control_text(self) -> SizeScale {
        self.control_text
    }

    /// Returns the control line-height scale.
    pub const fn control_line_height(self) -> SizeScale {
        self.control_line_height
    }
}

/// Spacing values admitted to the complete theme contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeSpacingScale {
    control_inline: SizeScale,
    control_block: SizeScale,
}

impl ThemeSpacingScale {
    /// Creates inline and block control-spacing scales.
    pub const fn new(control_inline: SizeScale, control_block: SizeScale) -> Self {
        Self {
            control_inline,
            control_block,
        }
    }

    /// Returns the inline control-spacing scale.
    pub const fn control_inline(self) -> SizeScale {
        self.control_inline
    }

    /// Returns the block control-spacing scale.
    pub const fn control_block(self) -> SizeScale {
        self.control_block
    }
}

/// Radius values admitted to the complete theme contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeRadiusScale {
    control: SizeScale,
}

impl ThemeRadiusScale {
    /// Creates a semantic control-radius scale.
    pub const fn new(control: SizeScale) -> Self {
        Self { control }
    }

    /// Returns the semantic control-radius scale.
    pub const fn control(self) -> SizeScale {
        self.control
    }
}

/// One renderer-neutral layer in a semantic elevation recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeElevationLayer {
    offset_x: i16,
    offset_y: i16,
    blur_radius: u16,
    spread_radius: i16,
    opacity_percent: u8,
}

impl ThemeElevationLayer {
    /// Creates one elevation layer from logical-pixel values and black opacity percent.
    pub const fn new(
        offset_x: i16,
        offset_y: i16,
        blur_radius: u16,
        spread_radius: i16,
        opacity_percent: u8,
    ) -> Self {
        Self {
            offset_x,
            offset_y,
            blur_radius,
            spread_radius,
            opacity_percent,
        }
    }

    /// Returns the horizontal offset.
    pub const fn offset_x(self) -> UiPx {
        ui_px(self.offset_x as f32)
    }

    /// Returns the vertical offset.
    pub const fn offset_y(self) -> UiPx {
        ui_px(self.offset_y as f32)
    }

    /// Returns the blur radius.
    pub const fn blur_radius(self) -> UiPx {
        ui_px(self.blur_radius as f32)
    }

    /// Returns the spread radius.
    pub const fn spread_radius(self) -> UiPx {
        ui_px(self.spread_radius as f32)
    }

    /// Returns black opacity as a percentage from zero through one hundred.
    pub const fn opacity_percent(self) -> u8 {
        self.opacity_percent
    }

    /// Returns raw schema values in offset-x, offset-y, blur, spread, and opacity order.
    pub const fn raw_values(self) -> (i16, i16, u16, i16, u8) {
        (
            self.offset_x,
            self.offset_y,
            self.blur_radius,
            self.spread_radius,
            self.opacity_percent,
        )
    }
}

/// Elevation values admitted to the complete theme contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeElevationScale {
    overlay: [ThemeElevationLayer; 2],
}

impl ThemeElevationScale {
    /// Creates the two-layer elevated overlay recipe.
    pub const fn new(overlay: [ThemeElevationLayer; 2]) -> Self {
        Self { overlay }
    }

    /// Returns the elevated overlay layers.
    pub const fn overlay(self) -> [ThemeElevationLayer; 2] {
        self.overlay
    }
}

/// Complete immutable non-color design values carried by Theme v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeDesignScales {
    typography: ThemeTypographyScale,
    spacing: ThemeSpacingScale,
    radius: ThemeRadiusScale,
    elevation: ThemeElevationScale,
    density: Density,
    motion: MotionPreference,
}

impl ThemeDesignScales {
    /// Returns the built-in Theme v1 baseline used when no render context is available.
    pub const fn baseline() -> Self {
        Self::new(
            ThemeTypographyScale::new(
                SizeScale::new(12, 13, 13, 14),
                SizeScale::new(12, 13, 13, 14),
            ),
            ThemeSpacingScale::new(SizeScale::new(8, 10, 12, 14), SizeScale::new(4, 5, 6, 7)),
            ThemeRadiusScale::new(SizeScale::new(6, 6, 8, 8)),
            ThemeElevationScale::new([
                ThemeElevationLayer::new(0, 10, 15, -3, 10),
                ThemeElevationLayer::new(0, 4, 6, -4, 10),
            ]),
            Density::Comfortable,
            MotionPreference::Animated,
        )
    }

    /// Creates a complete design-scale payload.
    pub const fn new(
        typography: ThemeTypographyScale,
        spacing: ThemeSpacingScale,
        radius: ThemeRadiusScale,
        elevation: ThemeElevationScale,
        density: Density,
        motion: MotionPreference,
    ) -> Self {
        Self {
            typography,
            spacing,
            radius,
            elevation,
            density,
            motion,
        }
    }

    /// Returns the typography scale.
    pub const fn typography(self) -> ThemeTypographyScale {
        self.typography
    }

    /// Returns the spacing scale.
    pub const fn spacing(self) -> ThemeSpacingScale {
        self.spacing
    }

    /// Returns the radius scale.
    pub const fn radius(self) -> ThemeRadiusScale {
        self.radius
    }

    /// Returns the elevation scale.
    pub const fn elevation(self) -> ThemeElevationScale {
        self.elevation
    }

    /// Returns the theme density default.
    pub const fn density(self) -> Density {
        self.density
    }

    /// Returns the theme motion policy.
    pub const fn motion(self) -> MotionPreference {
        self.motion
    }

    /// Resolves an explicit size or the theme density default.
    pub const fn resolve_size(self, explicit: Option<Size>) -> Size {
        match explicit {
            Some(size) => size,
            None => self.density.default_size(),
        }
    }

    /// Merges a component request with the theme accessibility floor.
    pub const fn resolve_motion(self, explicit: Option<MotionPreference>) -> MotionPreference {
        if matches!(self.motion, MotionPreference::Reduced)
            || matches!(explicit, Some(MotionPreference::Reduced))
        {
            MotionPreference::Reduced
        } else {
            MotionPreference::Animated
        }
    }
}

impl Default for ThemeDesignScales {
    fn default() -> Self {
        Self::baseline()
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

    #[test]
    fn complete_design_scales_resolve_size_and_strict_motion_policy() {
        let comfortable = ThemeDesignScales::default();
        assert_eq!(comfortable.resolve_size(None), Size::Medium);
        assert_eq!(comfortable.resolve_size(Some(Size::Large)), Size::Large);
        assert_eq!(
            comfortable.resolve_motion(Some(MotionPreference::Reduced)),
            MotionPreference::Reduced
        );

        let reduced = ThemeDesignScales::new(
            comfortable.typography(),
            comfortable.spacing(),
            comfortable.radius(),
            comfortable.elevation(),
            Density::Compact,
            MotionPreference::Reduced,
        );
        assert_eq!(reduced.resolve_size(None), Size::Small);
        assert_eq!(
            reduced.resolve_motion(Some(MotionPreference::Animated)),
            MotionPreference::Reduced
        );
    }
}
