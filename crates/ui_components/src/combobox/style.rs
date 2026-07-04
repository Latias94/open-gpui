use open_gpui_ui_core::{Size, UiPx, ui_px};

use crate::color::ColorIntent;

/// Resolved combobox color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComboboxColors {
    pub(crate) popup_background: ColorIntent,
    pub(crate) popup_foreground: ColorIntent,
    pub(crate) popup_border: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl ComboboxColors {
    /// Returns popup background color intent.
    pub const fn popup_background(self) -> ColorIntent {
        self.popup_background
    }

    /// Returns popup foreground color intent.
    pub const fn popup_foreground(self) -> ColorIntent {
        self.popup_foreground
    }

    /// Returns popup border color intent.
    pub const fn popup_border(self) -> ColorIntent {
        self.popup_border
    }

    /// Returns focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved combobox metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComboboxMetrics {
    popup_padding: UiPx,
    popup_radius: UiPx,
    popup_min_width: UiPx,
    popup_max_width: UiPx,
    popup_max_height: UiPx,
}

impl ComboboxMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            popup_padding: ui_px(4.0),
            popup_radius: size.control_radius(),
            popup_min_width: ui_px(260.0),
            popup_max_width: ui_px(420.0),
            popup_max_height: match size {
                Size::XSmall => ui_px(180.0),
                Size::Small => ui_px(220.0),
                Size::Medium => ui_px(280.0),
                Size::Large => ui_px(340.0),
            },
        }
    }

    /// Returns popup padding.
    pub const fn popup_padding(self) -> UiPx {
        self.popup_padding
    }

    /// Returns popup corner radius.
    pub const fn popup_radius(self) -> UiPx {
        self.popup_radius
    }

    /// Returns popup minimum width.
    pub const fn popup_min_width(self) -> UiPx {
        self.popup_min_width
    }

    /// Returns popup maximum width.
    pub const fn popup_max_width(self) -> UiPx {
        self.popup_max_width
    }

    /// Returns popup maximum height.
    pub const fn popup_max_height(self) -> UiPx {
        self.popup_max_height
    }
}
