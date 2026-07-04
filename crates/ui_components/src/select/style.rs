use open_gpui_ui_core::{Size, UiPx, ui_px};

use crate::color::ColorIntent;

/// Resolved select color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectColors {
    pub(crate) trigger_background: ColorIntent,
    pub(crate) trigger_hover_background: ColorIntent,
    pub(crate) trigger_foreground: ColorIntent,
    pub(crate) trigger_placeholder_foreground: ColorIntent,
    pub(crate) trigger_border: ColorIntent,
    pub(crate) content_background: ColorIntent,
    pub(crate) content_foreground: ColorIntent,
    pub(crate) content_border: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl SelectColors {
    /// Returns trigger background color intent.
    pub const fn trigger_background(self) -> ColorIntent {
        self.trigger_background
    }

    /// Returns trigger hover background color intent.
    pub const fn trigger_hover_background(self) -> ColorIntent {
        self.trigger_hover_background
    }

    /// Returns trigger foreground color intent.
    pub const fn trigger_foreground(self) -> ColorIntent {
        self.trigger_foreground
    }

    /// Returns placeholder foreground color intent.
    pub const fn trigger_placeholder_foreground(self) -> ColorIntent {
        self.trigger_placeholder_foreground
    }

    /// Returns trigger border color intent.
    pub const fn trigger_border(self) -> ColorIntent {
        self.trigger_border
    }

    /// Returns content background color intent.
    pub const fn content_background(self) -> ColorIntent {
        self.content_background
    }

    /// Returns content foreground color intent.
    pub const fn content_foreground(self) -> ColorIntent {
        self.content_foreground
    }

    /// Returns content border color intent.
    pub const fn content_border(self) -> ColorIntent {
        self.content_border
    }

    /// Returns trigger focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved select metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectMetrics {
    trigger_height: UiPx,
    trigger_padding_x: UiPx,
    trigger_padding_y: UiPx,
    content_padding: UiPx,
    radius: UiPx,
    text_size: UiPx,
    min_width: UiPx,
    max_width: UiPx,
    max_height: UiPx,
}

impl SelectMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            trigger_height: size.button_h(),
            trigger_padding_x: size.button_px(),
            trigger_padding_y: size.button_py(),
            content_padding: ui_px(4.0),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
            min_width: ui_px(220.0),
            max_width: ui_px(360.0),
            max_height: match size {
                Size::XSmall => ui_px(180.0),
                Size::Small => ui_px(220.0),
                Size::Medium => ui_px(260.0),
                Size::Large => ui_px(320.0),
            },
        }
    }

    /// Returns trigger height.
    pub const fn trigger_height(self) -> UiPx {
        self.trigger_height
    }

    /// Returns trigger horizontal padding.
    pub const fn trigger_padding_x(self) -> UiPx {
        self.trigger_padding_x
    }

    /// Returns trigger vertical padding.
    pub const fn trigger_padding_y(self) -> UiPx {
        self.trigger_padding_y
    }

    /// Returns content padding.
    pub const fn content_padding(self) -> UiPx {
        self.content_padding
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }

    /// Returns minimum content width.
    pub const fn min_width(self) -> UiPx {
        self.min_width
    }

    /// Returns maximum content width.
    pub const fn max_width(self) -> UiPx {
        self.max_width
    }

    /// Returns maximum content height.
    pub const fn max_height(self) -> UiPx {
        self.max_height
    }
}
