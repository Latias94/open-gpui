use open_gpui_ui_core::{Size, ThemeTokens, UiPx, ui_px};

use crate::{color::ColorIntent, theme::ThemeResolver};

use super::TreeItemState;

/// Theme-aware row paint without access to tree geometry or interaction ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeRowAppearance {
    background: ColorIntent,
    foreground: ColorIntent,
    hover_background: ColorIntent,
    focus_ring: ColorIntent,
}

impl TreeRowAppearance {
    /// Resolves the shared collection recipe for the item's accepted state.
    pub fn from_item(item: &TreeItemState) -> Self {
        let colors = ThemeResolver::listbox_colors(ThemeTokens::default());
        Self {
            background: if item.selected() {
                colors.option_selected_background()
            } else if item.focused() {
                colors.option_active_background()
            } else {
                colors.option_background()
            },
            foreground: if item.disabled() {
                colors.option_disabled_foreground()
            } else {
                colors.foreground()
            },
            hover_background: colors.option_hover_background(),
            focus_ring: colors.focus_ring(),
        }
    }

    /// Applies the row background intent, resolved in the nearest theme scope.
    pub const fn with_background(mut self, background: ColorIntent) -> Self {
        self.background = background;
        self
    }

    /// Applies the row foreground intent.
    pub const fn with_foreground(mut self, foreground: ColorIntent) -> Self {
        self.foreground = foreground;
        self
    }

    /// Applies the hover background intent.
    pub const fn with_hover_background(mut self, background: ColorIntent) -> Self {
        self.hover_background = background;
        self
    }

    /// Applies the focus ring intent.
    pub const fn with_focus_ring(mut self, focus_ring: ColorIntent) -> Self {
        self.focus_ring = focus_ring;
        self
    }

    /// Returns the row background intent.
    pub const fn background(self) -> ColorIntent {
        self.background
    }

    /// Returns the row foreground intent.
    pub const fn foreground(self) -> ColorIntent {
        self.foreground
    }

    /// Returns the hover background intent.
    pub const fn hover_background(self) -> ColorIntent {
        self.hover_background
    }

    /// Returns the focus ring intent.
    pub const fn focus_ring(self) -> ColorIntent {
        self.focus_ring
    }
}

/// Resolved tree metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeMetrics {
    row_height: UiPx,
    indent_width: UiPx,
    row_padding_x: UiPx,
    row_padding_y: UiPx,
    text_size: UiPx,
}

impl TreeMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            row_height: size.list_row_h(),
            indent_width: match size {
                Size::XSmall | Size::Small => ui_px(14.0),
                Size::Medium | Size::Large => ui_px(16.0),
            },
            row_padding_x: size.list_px(),
            row_padding_y: size.list_py(),
            text_size: size.control_text_px(),
        }
    }

    /// Returns the row height.
    pub const fn row_height(self) -> UiPx {
        self.row_height
    }

    /// Returns the indentation applied per depth level.
    pub const fn indent_width(self) -> UiPx {
        self.indent_width
    }

    /// Returns row horizontal padding.
    pub const fn row_padding_x(self) -> UiPx {
        self.row_padding_x
    }

    /// Returns row vertical padding.
    pub const fn row_padding_y(self) -> UiPx {
        self.row_padding_y
    }

    /// Returns row text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }
}
