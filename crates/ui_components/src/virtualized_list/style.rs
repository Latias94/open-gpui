use open_gpui_ui_core::{Size, UiPx};

/// Resolved virtualized-list metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualizedListMetrics {
    row_height: UiPx,
    overscan_count: usize,
}

impl VirtualizedListMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            row_height: size.list_row_h(),
            overscan_count: match size {
                Size::XSmall | Size::Small => 4,
                Size::Medium => 5,
                Size::Large => 6,
            },
        }
    }

    /// Returns the default fixed row height.
    pub const fn row_height(self) -> UiPx {
        self.row_height
    }

    /// Returns the number of rows the adapter should keep beyond the viewport.
    pub const fn overscan_count(self) -> usize {
        self.overscan_count
    }

    /// Returns the same metrics with a different row height.
    pub fn with_row_height(mut self, row_height: UiPx) -> Self {
        self.row_height = nonnegative_px(row_height);
        self
    }

    /// Returns the same metrics with a different overscan budget.
    pub const fn with_overscan_count(mut self, overscan_count: usize) -> Self {
        self.overscan_count = overscan_count;
        self
    }
}

pub(super) const DEFAULT_VIRTUALIZED_LIST_VIEWPORT_ITEM_COUNT: usize = 8;

pub(super) const fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        UiPx::ZERO
    } else {
        value
    }
}
