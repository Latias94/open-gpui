use open_gpui_ui_core::{Size, UiPx, ui_px};

use crate::color::ColorIntent;
/// Resolved menu color intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuColors {
    pub(crate) surface: ColorIntent,
    pub(crate) foreground: ColorIntent,
    pub(crate) border: ColorIntent,
    pub(crate) item_background: ColorIntent,
    pub(crate) item_hover_background: ColorIntent,
    pub(crate) item_focus_background: ColorIntent,
    pub(crate) item_disabled_foreground: ColorIntent,
    pub(crate) separator: ColorIntent,
    pub(crate) trigger_background: ColorIntent,
    pub(crate) trigger_hover_background: ColorIntent,
    pub(crate) trigger_foreground: ColorIntent,
    pub(crate) trigger_border: ColorIntent,
    pub(crate) focus_ring: ColorIntent,
}

impl MenuColors {
    /// Returns menu surface color intent.
    pub const fn surface(self) -> ColorIntent {
        self.surface
    }

    /// Returns menu foreground color intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns menu border color intent.
    pub const fn border(self) -> ColorIntent {
        self.border
    }

    /// Returns default menu item background color intent.
    pub const fn item_background(self) -> ColorIntent {
        self.item_background
    }

    /// Returns hovered menu item background color intent.
    pub const fn item_hover_background(self) -> ColorIntent {
        self.item_hover_background
    }

    /// Returns focused menu item background color intent.
    pub const fn item_focus_background(self) -> ColorIntent {
        self.item_focus_background
    }

    /// Returns disabled menu item foreground color intent.
    pub const fn item_disabled_foreground(self) -> ColorIntent {
        self.item_disabled_foreground
    }

    /// Returns separator color intent.
    pub const fn separator(self) -> ColorIntent {
        self.separator
    }

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

    /// Returns trigger border color intent.
    pub const fn trigger_border(self) -> ColorIntent {
        self.trigger_border
    }

    /// Returns focus-ring color intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved menu metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuMetrics {
    trigger_height: UiPx,
    trigger_padding_x: UiPx,
    trigger_padding_y: UiPx,
    surface_padding: UiPx,
    item_height: UiPx,
    item_padding_x: UiPx,
    item_padding_y: UiPx,
    separator_height: UiPx,
    radius: UiPx,
    text_size: UiPx,
    min_width: UiPx,
    max_width: UiPx,
    max_height: UiPx,
    submenu_indent: UiPx,
}

impl MenuMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            trigger_height: size.button_h(),
            trigger_padding_x: size.button_px(),
            trigger_padding_y: size.button_py(),
            surface_padding: ui_px(6.0),
            item_height: size.button_h(),
            item_padding_x: size.button_px(),
            item_padding_y: ui_px(6.0),
            separator_height: ui_px(1.0),
            radius: size.control_radius(),
            text_size: size.control_text_px(),
            min_width: ui_px(180.0),
            max_width: ui_px(320.0),
            max_height: ui_px(280.0),
            submenu_indent: match size {
                Size::XSmall | Size::Small => ui_px(14.0),
                Size::Medium | Size::Large => ui_px(18.0),
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

    /// Returns menu surface padding.
    pub const fn surface_padding(self) -> UiPx {
        self.surface_padding
    }

    /// Returns menu item height.
    pub const fn item_height(self) -> UiPx {
        self.item_height
    }

    /// Returns menu item horizontal padding.
    pub const fn item_padding_x(self) -> UiPx {
        self.item_padding_x
    }

    /// Returns menu item vertical padding.
    pub const fn item_padding_y(self) -> UiPx {
        self.item_padding_y
    }

    /// Returns separator height.
    pub const fn separator_height(self) -> UiPx {
        self.separator_height
    }

    /// Returns corner radius.
    pub const fn radius(self) -> UiPx {
        self.radius
    }

    /// Returns text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }

    /// Returns minimum menu width.
    pub const fn min_width(self) -> UiPx {
        self.min_width
    }

    /// Returns maximum menu width.
    pub const fn max_width(self) -> UiPx {
        self.max_width
    }

    /// Returns maximum menu surface height before local scrolling.
    pub const fn max_height(self) -> UiPx {
        self.max_height
    }

    /// Returns additional indentation per submenu depth.
    pub const fn submenu_indent(self) -> UiPx {
        self.submenu_indent
    }
}
