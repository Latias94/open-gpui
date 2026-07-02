use open_gpui_ui_core::{Size, UiPx, ui_px};
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
