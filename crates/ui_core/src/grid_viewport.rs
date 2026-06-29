//! Renderer-neutral two-axis viewport contracts for table-like grids.

use crate::geometry::UiPx;
use crate::virtualizer::{VirtualizerRange, VirtualizerResolvedState, VirtualizerState};

/// Combined row and column viewport metadata for a grid body.
#[derive(Debug, Clone, PartialEq)]
pub struct GridViewport2D {
    row_virtualizer: VirtualizerResolvedState,
    column_virtualizer: VirtualizerResolvedState,
}

impl GridViewport2D {
    /// Creates a combined viewport from resolved row and column virtualizers.
    pub const fn new(
        row_virtualizer: VirtualizerResolvedState,
        column_virtualizer: VirtualizerResolvedState,
    ) -> Self {
        Self {
            row_virtualizer,
            column_virtualizer,
        }
    }

    /// Returns the resolved row virtualizer.
    pub const fn row_virtualizer(&self) -> &VirtualizerResolvedState {
        &self.row_virtualizer
    }

    /// Returns the resolved column virtualizer.
    pub const fn column_virtualizer(&self) -> &VirtualizerResolvedState {
        &self.column_virtualizer
    }

    /// Returns the resolved row range.
    pub const fn row_range(&self) -> &VirtualizerRange {
        self.row_virtualizer.visible_range()
    }

    /// Returns the resolved column range.
    pub const fn column_range(&self) -> &VirtualizerRange {
        self.column_virtualizer.visible_range()
    }

    /// Returns the row overscan range.
    pub const fn row_overscan_range(&self) -> &VirtualizerRange {
        self.row_virtualizer.overscan_range()
    }

    /// Returns the column overscan range.
    pub const fn column_overscan_range(&self) -> &VirtualizerRange {
        self.column_virtualizer.overscan_range()
    }

    /// Returns the clamped horizontal scroll offset.
    pub const fn scroll_x(&self) -> UiPx {
        self.column_virtualizer.scroll_offset()
    }

    /// Returns the clamped vertical scroll offset.
    pub const fn scroll_y(&self) -> UiPx {
        self.row_virtualizer.scroll_offset()
    }

    /// Returns the total width of the resolved column axis.
    pub const fn total_width(&self) -> UiPx {
        self.column_virtualizer.total_size()
    }

    /// Returns the total height of the resolved row axis.
    pub const fn total_height(&self) -> UiPx {
        self.row_virtualizer.total_size()
    }

    /// Returns the row overscan budget.
    pub const fn row_overscan(&self) -> usize {
        self.row_virtualizer.overscan()
    }

    /// Returns the column overscan budget.
    pub const fn column_overscan(&self) -> usize {
        self.column_virtualizer.overscan()
    }
}

/// Resolves a two-axis viewport from row and column virtualizer state.
pub fn resolve_grid_viewport_2d(
    row_virtualizer: &VirtualizerState,
    column_virtualizer: &VirtualizerState,
) -> GridViewport2D {
    GridViewport2D::new(
        resolve_axis(row_virtualizer),
        resolve_axis(column_virtualizer),
    )
}

fn resolve_axis(state: &VirtualizerState) -> VirtualizerResolvedState {
    let resolved = state.resolve();
    let clamped_scroll_offset = clamp_scroll_offset(
        resolved.scroll_offset(),
        resolved.total_size(),
        resolved.viewport_extent(),
    );

    if clamped_scroll_offset == resolved.scroll_offset() {
        resolved
    } else {
        state
            .clone()
            .with_scroll_offset(clamped_scroll_offset)
            .resolve()
    }
}

fn clamp_scroll_offset(scroll_offset: UiPx, total_size: UiPx, viewport_extent: UiPx) -> UiPx {
    let scroll_offset = scroll_offset.max(UiPx::ZERO);
    let viewport_extent = viewport_extent.max(UiPx::ZERO);
    let total_size = total_size.max(UiPx::ZERO);

    if total_size <= viewport_extent {
        UiPx::ZERO
    } else {
        let max_offset = total_size - viewport_extent;
        scroll_offset.min(max_offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::ui_px;
    #[test]
    fn resolve_grid_viewport_2d_clamps_offsets_and_exposes_ranges() {
        let rows = VirtualizerState::new(4, ui_px(20.0))
            .with_item_keys(["row-0", "row-1", "row-2", "row-3"])
            .with_viewport_extent(ui_px(30.0))
            .with_scroll_offset(ui_px(200.0))
            .with_overscan(0);
        let columns = VirtualizerState::new(3, ui_px(10.0))
            .with_item_keys(["col-0", "col-1", "col-2"])
            .with_viewport_extent(ui_px(15.0))
            .with_scroll_offset(ui_px(120.0))
            .with_overscan(0)
            .with_measurement("col-0", ui_px(30.0))
            .with_measurement("col-1", ui_px(40.0))
            .with_measurement("col-2", ui_px(50.0));

        let viewport = resolve_grid_viewport_2d(&rows, &columns);

        assert_eq!(viewport.scroll_y(), ui_px(50.0));
        assert_eq!(viewport.scroll_x(), ui_px(105.0));
        assert_eq!(*viewport.row_range(), VirtualizerRange::new(2, 4));
        assert_eq!(*viewport.column_range(), VirtualizerRange::new(2, 3));
        assert_eq!(viewport.total_height(), ui_px(80.0));
        assert_eq!(viewport.total_width(), ui_px(120.0));
        assert_eq!(*viewport.row_overscan_range(), VirtualizerRange::new(2, 4));
        assert_eq!(
            *viewport.column_overscan_range(),
            VirtualizerRange::new(2, 3)
        );
        assert_eq!(viewport.row_overscan(), 0);
        assert_eq!(viewport.column_overscan(), 0);
        assert_eq!(
            viewport
                .row_virtualizer()
                .items()
                .iter()
                .map(|item| item.key().as_str())
                .collect::<Vec<_>>(),
            ["row-2", "row-3"]
        );
        assert_eq!(
            viewport
                .column_virtualizer()
                .items()
                .iter()
                .map(|item| item.key().as_str())
                .collect::<Vec<_>>(),
            ["col-2"]
        );
    }

    #[test]
    fn overscan_changes_only_overscan_ranges_without_renaming_axis_keys() {
        let rows = VirtualizerState::new(5, ui_px(20.0))
            .with_item_keys(["row-0", "row-1", "row-2", "row-3", "row-4"])
            .with_viewport_extent(ui_px(35.0))
            .with_scroll_offset(ui_px(28.0));
        let wide_rows = rows.clone().with_overscan(2);
        let narrow_rows = rows.with_overscan(0);

        let columns = VirtualizerState::new(5, ui_px(20.0))
            .with_item_keys(["col-0", "col-1", "col-2", "col-3", "col-4"])
            .with_viewport_extent(ui_px(38.0))
            .with_scroll_offset(ui_px(17.0))
            .with_measurement("col-0", ui_px(18.0))
            .with_measurement("col-1", ui_px(22.0))
            .with_measurement("col-2", ui_px(26.0))
            .with_measurement("col-3", ui_px(30.0))
            .with_measurement("col-4", ui_px(34.0));
        let wide_columns = columns.clone().with_overscan(2);
        let narrow_columns = columns.with_overscan(0);

        let narrow = resolve_grid_viewport_2d(&narrow_rows, &narrow_columns);
        let wide = resolve_grid_viewport_2d(&wide_rows, &wide_columns);

        assert_eq!(narrow.row_range(), wide.row_range());
        assert_eq!(narrow.column_range(), wide.column_range());
        assert_ne!(narrow.row_overscan_range(), wide.row_overscan_range());
        assert_ne!(narrow.column_overscan_range(), wide.column_overscan_range());

        let narrow_row_keys = narrow
            .row_virtualizer()
            .items()
            .iter()
            .map(|item| item.key().as_str())
            .collect::<Vec<_>>();
        let wide_row_keys = wide
            .row_virtualizer()
            .items()
            .iter()
            .map(|item| item.key().as_str())
            .collect::<Vec<_>>();
        for key in narrow_row_keys {
            assert!(wide_row_keys.contains(&key));
        }

        let narrow_column_keys = narrow
            .column_virtualizer()
            .items()
            .iter()
            .map(|item| item.key().as_str())
            .collect::<Vec<_>>();
        let wide_column_keys = wide
            .column_virtualizer()
            .items()
            .iter()
            .map(|item| item.key().as_str())
            .collect::<Vec<_>>();
        for key in narrow_column_keys {
            assert!(wide_column_keys.contains(&key));
        }
    }

    #[test]
    fn empty_axes_resolve_to_empty_ranges() {
        let rows = VirtualizerState::new(0, ui_px(20.0)).with_viewport_extent(ui_px(100.0));
        let columns = VirtualizerState::new(0, ui_px(20.0)).with_viewport_extent(ui_px(120.0));

        let viewport = resolve_grid_viewport_2d(&rows, &columns);

        assert!(viewport.row_range().is_empty());
        assert!(viewport.column_range().is_empty());
        assert_eq!(viewport.scroll_x(), UiPx::ZERO);
        assert_eq!(viewport.scroll_y(), UiPx::ZERO);
        assert_eq!(viewport.total_width(), UiPx::ZERO);
        assert_eq!(viewport.total_height(), UiPx::ZERO);
    }

    #[test]
    fn resolved_axes_can_be_combined_directly() {
        let rows = VirtualizerState::new(3, ui_px(24.0))
            .with_item_keys(["row-a", "row-b", "row-c"])
            .with_viewport_extent(ui_px(48.0))
            .resolve();
        let columns = VirtualizerState::new(2, ui_px(30.0))
            .with_item_keys(["col-a", "col-b"])
            .with_viewport_extent(ui_px(60.0))
            .resolve();

        let viewport = GridViewport2D::new(rows.clone(), columns.clone());

        assert_eq!(viewport.row_virtualizer(), &rows);
        assert_eq!(viewport.column_virtualizer(), &columns);
    }
}
