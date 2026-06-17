//! Component sizing vocabulary for the Open GPUI component ecosystem.

use crate::geometry::{UiPx, ui_px};

/// Shared component size vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Size {
    /// Dense controls and compact panels.
    XSmall,
    /// Tight but readable controls.
    Small,
    /// The default component size.
    #[default]
    Medium,
    /// Comfortable or spacious controls.
    Large,
}

impl Size {
    /// Returns a short stable label for the size.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::XSmall => "xs",
            Self::Small => "sm",
            Self::Medium => "md",
            Self::Large => "lg",
        }
    }

    /// Returns the default text size for controls using this size vocabulary.
    pub const fn control_text_px(self) -> UiPx {
        match self {
            Self::XSmall => ui_px(12.0),
            Self::Small => ui_px(13.0),
            Self::Medium => ui_px(13.0),
            Self::Large => ui_px(14.0),
        }
    }

    /// Returns the default corner radius for controls using this size vocabulary.
    pub const fn control_radius(self) -> UiPx {
        match self {
            Self::XSmall => ui_px(6.0),
            Self::Small => ui_px(6.0),
            Self::Medium => ui_px(8.0),
            Self::Large => ui_px(8.0),
        }
    }

    /// Returns horizontal padding for text inputs.
    pub const fn input_px(self) -> UiPx {
        match self {
            Self::XSmall => ui_px(8.0),
            Self::Small => ui_px(10.0),
            Self::Medium => ui_px(12.0),
            Self::Large => ui_px(14.0),
        }
    }

    /// Returns vertical padding for text inputs.
    pub const fn input_py(self) -> UiPx {
        match self {
            Self::XSmall => ui_px(4.0),
            Self::Small => ui_px(5.0),
            Self::Medium => ui_px(6.0),
            Self::Large => ui_px(7.0),
        }
    }

    /// Returns the default control height for text inputs.
    pub const fn input_h(self) -> UiPx {
        match self {
            Self::XSmall => ui_px(24.0),
            Self::Small => ui_px(28.0),
            Self::Medium => ui_px(32.0),
            Self::Large => ui_px(36.0),
        }
    }

    /// Returns horizontal padding for buttons.
    pub const fn button_px(self) -> UiPx {
        match self {
            Self::XSmall => ui_px(8.0),
            Self::Small => ui_px(10.0),
            Self::Medium => ui_px(12.0),
            Self::Large => ui_px(14.0),
        }
    }

    /// Returns vertical padding for buttons.
    pub const fn button_py(self) -> UiPx {
        match self {
            Self::XSmall => ui_px(4.0),
            Self::Small => ui_px(5.0),
            Self::Medium => ui_px(6.0),
            Self::Large => ui_px(7.0),
        }
    }

    /// Returns the default control height for buttons.
    pub const fn button_h(self) -> UiPx {
        match self {
            Self::XSmall => ui_px(24.0),
            Self::Small => ui_px(28.0),
            Self::Medium => ui_px(32.0),
            Self::Large => ui_px(36.0),
        }
    }

    /// Returns the default icon button size.
    pub const fn icon_button_size(self) -> UiPx {
        match self {
            Self::XSmall => ui_px(24.0),
            Self::Small => ui_px(28.0),
            Self::Medium => ui_px(32.0),
            Self::Large => ui_px(36.0),
        }
    }

    /// Returns the default icon glyph size for icon-bearing controls.
    pub const fn icon_size(self) -> UiPx {
        match self {
            Self::XSmall => ui_px(13.0),
            Self::Small => ui_px(14.0),
            Self::Medium => ui_px(15.0),
            Self::Large => ui_px(16.0),
        }
    }

    /// Returns horizontal padding for dense lists.
    pub const fn list_px(self) -> UiPx {
        match self {
            Self::XSmall => ui_px(8.0),
            Self::Small => ui_px(8.0),
            Self::Medium => ui_px(12.0),
            Self::Large => ui_px(12.0),
        }
    }

    /// Returns vertical padding for dense lists.
    pub const fn list_py(self) -> UiPx {
        match self {
            Self::XSmall => ui_px(2.0),
            Self::Small => ui_px(2.0),
            Self::Medium => ui_px(4.0),
            Self::Large => ui_px(8.0),
        }
    }

    /// Returns the default row height for list-like components.
    pub const fn list_row_h(self) -> UiPx {
        match self {
            Self::XSmall => ui_px(24.0),
            Self::Small => ui_px(28.0),
            Self::Medium => ui_px(32.0),
            Self::Large => ui_px(36.0),
        }
    }
}

/// Broad density vocabulary for application shells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    /// Dense, compact surfaces.
    Compact,
    /// Default density for productivity software.
    #[default]
    Comfortable,
    /// Spacious surfaces for touch-first or content-light screens.
    Spacious,
}

impl Density {
    /// Returns the default size associated with this density.
    pub const fn default_size(self) -> Size {
        match self {
            Self::Compact => Size::Small,
            Self::Comfortable => Size::Medium,
            Self::Spacious => Size::Large,
        }
    }
}

/// Shared component API for size configuration.
pub trait Sizable: Sized {
    /// Applies a component size.
    fn with_size(self, size: Size) -> Self;

    /// Sets the component to extra small.
    fn xsmall(self) -> Self {
        self.with_size(Size::XSmall)
    }

    /// Sets the component to small.
    fn small(self) -> Self {
        self.with_size(Size::Small)
    }

    /// Sets the component to medium.
    fn medium(self) -> Self {
        self.with_size(Size::Medium)
    }

    /// Sets the component to large.
    fn large(self) -> Self {
        self.with_size(Size::Large)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_labels_are_stable() {
        assert_eq!(Size::XSmall.as_str(), "xs");
        assert_eq!(Size::Small.as_str(), "sm");
        assert_eq!(Size::Medium.as_str(), "md");
        assert_eq!(Size::Large.as_str(), "lg");
    }

    #[test]
    fn density_maps_to_reasonable_default_sizes() {
        assert_eq!(Density::Compact.default_size(), Size::Small);
        assert_eq!(Density::Comfortable.default_size(), Size::Medium);
        assert_eq!(Density::Spacious.default_size(), Size::Large);
    }

    #[test]
    fn size_defaults_follow_the_expected_scale() {
        assert_eq!(Size::Small.button_h(), ui_px(28.0));
        assert_eq!(Size::Medium.input_h(), ui_px(32.0));
        assert_eq!(Size::Large.icon_button_size(), ui_px(36.0));
        assert_eq!(Size::Medium.icon_size(), ui_px(15.0));
    }

    #[test]
    fn size_trait_helpers_apply_the_requested_size() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct SizedValue(Size);

        impl Sizable for SizedValue {
            fn with_size(self, size: Size) -> Self {
                Self(size)
            }
        }

        assert_eq!(SizedValue(Size::Small).large(), SizedValue(Size::Large));
        assert_eq!(SizedValue(Size::Large).xsmall(), SizedValue(Size::XSmall));
    }
}
