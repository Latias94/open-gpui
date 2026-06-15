//! Sizing and density foundation page metadata.

use open_gpui::Pixels;
use open_gpui_ui_core::{Density, Size};

/// Page title.
pub const TITLE: &str = "Sizing & Density";
/// Page summary.
pub const SUMMARY: &str = "Shared control sizes and shell density choices.";
/// Foundation signals rendered by this page.
pub const SIGNALS: &[&str] = &[
    "Density::Compact",
    "Density::Comfortable",
    "Density::Spacious",
    "Size::button_h()",
    "Size::control_radius()",
];

/// One size row rendered by the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SizeSample {
    /// Size vocabulary entry.
    pub size: Size,
    /// Stable label for the size.
    pub label: &'static str,
    /// Default button height.
    pub button_h: Pixels,
    /// Default input height.
    pub input_h: Pixels,
    /// Default icon button size.
    pub icon_button_size: Pixels,
    /// Default control radius.
    pub radius: Pixels,
}

impl SizeSample {
    const fn new(size: Size) -> Self {
        Self {
            size,
            label: size.as_str(),
            button_h: size.button_h(),
            input_h: size.input_h(),
            icon_button_size: size.icon_button_size(),
            radius: size.control_radius(),
        }
    }
}

/// One density row rendered by the gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DensitySample {
    /// Density vocabulary entry.
    pub density: Density,
    /// Stable label for the density.
    pub label: &'static str,
    /// Default size derived from the density.
    pub default_size: Size,
}

impl DensitySample {
    const fn new(density: Density, label: &'static str) -> Self {
        Self {
            density,
            label,
            default_size: density.default_size(),
        }
    }
}

/// Canonical size samples.
pub const SIZE_SAMPLES: [SizeSample; 4] = [
    SizeSample::new(Size::XSmall),
    SizeSample::new(Size::Small),
    SizeSample::new(Size::Medium),
    SizeSample::new(Size::Large),
];

/// Canonical density samples.
pub const DENSITY_SAMPLES: [DensitySample; 3] = [
    DensitySample::new(Density::Compact, "compact"),
    DensitySample::new(Density::Comfortable, "comfortable"),
    DensitySample::new(Density::Spacious, "spacious"),
];
