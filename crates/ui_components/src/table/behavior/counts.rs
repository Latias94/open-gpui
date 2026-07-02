use open_gpui_ui_core::{TableResolvedState, VirtualizerRange};

use crate::table::render_plan::TableRenderPlan;
/// Row-model and rendered-row counts for a table behavior snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRowCountSnapshot {
    core_rows: usize,
    filtered_rows: usize,
    grouped_rows: usize,
    sorted_rows: usize,
    expanded_rows: usize,
    paginated_rows: usize,
    final_rows: usize,
    pinned_top_rows: usize,
    pinned_center_rows: usize,
    pinned_bottom_rows: usize,
    rendered_rows: usize,
    visible_rows: usize,
    aria_rows: usize,
    selected_rows: usize,
    group_rows: usize,
    leaf_rows: usize,
}

impl TableRowCountSnapshot {
    pub(in crate::table::behavior) fn from_table(
        plan: &TableRenderPlan,
        table: &TableResolvedState,
        group_rows: usize,
    ) -> Self {
        let final_rows = table.final_model().rows().len();
        Self {
            core_rows: table.core_model().rows().len(),
            filtered_rows: table.filtered_model().rows().len(),
            grouped_rows: table.grouped_model().rows().len(),
            sorted_rows: table.sorted_model().rows().len(),
            expanded_rows: table.expanded_model().rows().len(),
            paginated_rows: table.paginated_model().rows().len(),
            final_rows,
            pinned_top_rows: table.top_rows().len(),
            pinned_center_rows: table.center_rows().len(),
            pinned_bottom_rows: table.bottom_rows().len(),
            rendered_rows: plan.rendered_row_count(),
            visible_rows: plan.visible_row_count(),
            aria_rows: plan.aria_row_count(),
            selected_rows: table.final_model().selected_count(),
            group_rows,
            leaf_rows: final_rows.saturating_sub(group_rows),
        }
    }

    /// Returns the untransformed source row count.
    pub const fn core_rows(self) -> usize {
        self.core_rows
    }

    /// Returns the filtered row count.
    pub const fn filtered_rows(self) -> usize {
        self.filtered_rows
    }

    /// Returns the grouped row-model count.
    pub const fn grouped_rows(self) -> usize {
        self.grouped_rows
    }

    /// Returns the sorted row-model count.
    pub const fn sorted_rows(self) -> usize {
        self.sorted_rows
    }

    /// Returns the expanded row-model count.
    pub const fn expanded_rows(self) -> usize {
        self.expanded_rows
    }

    /// Returns the paginated row-model count.
    pub const fn paginated_rows(self) -> usize {
        self.paginated_rows
    }

    /// Returns the final row-model count.
    pub const fn final_rows(self) -> usize {
        self.final_rows
    }

    /// Returns top-pinned row count.
    pub const fn pinned_top_rows(self) -> usize {
        self.pinned_top_rows
    }

    /// Returns center row count after row pinning.
    pub const fn pinned_center_rows(self) -> usize {
        self.pinned_center_rows
    }

    /// Returns bottom-pinned row count.
    pub const fn pinned_bottom_rows(self) -> usize {
        self.pinned_bottom_rows
    }

    /// Returns the number of body rows rendered after overscan.
    pub const fn rendered_rows(self) -> usize {
        self.rendered_rows
    }

    /// Returns the visible body row count before overscan.
    pub const fn visible_rows(self) -> usize {
        self.visible_rows
    }

    /// Returns the accessibility row count including the header row.
    pub const fn aria_rows(self) -> usize {
        self.aria_rows
    }

    /// Returns selected final row count.
    pub const fn selected_rows(self) -> usize {
        self.selected_rows
    }

    /// Returns synthetic group row count in the final model.
    pub const fn group_rows(self) -> usize {
        self.group_rows
    }

    /// Returns leaf row count in the final model.
    pub const fn leaf_rows(self) -> usize {
        self.leaf_rows
    }
}

/// Visible row window summary without exposing virtualizer internals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableVisibleRowsSnapshot {
    visible_range: VirtualizerRange,
    overscan_range: VirtualizerRange,
    rendered_rows: usize,
    visible_rows: usize,
    center_overscan_count: usize,
}

impl TableVisibleRowsSnapshot {
    pub(in crate::table::behavior) fn from_render_plan(
        plan: &TableRenderPlan,
        visible_range: &VirtualizerRange,
        overscan_range: &VirtualizerRange,
    ) -> Self {
        Self {
            visible_range: visible_range.clone(),
            overscan_range: overscan_range.clone(),
            rendered_rows: plan.rendered_row_count(),
            visible_rows: plan.visible_row_count(),
            center_overscan_count: plan.center_overscan_count(),
        }
    }

    /// Returns the visible row range before overscan.
    pub const fn visible_range(&self) -> &VirtualizerRange {
        &self.visible_range
    }

    /// Returns the rendered row range after overscan.
    pub const fn overscan_range(&self) -> &VirtualizerRange {
        &self.overscan_range
    }

    /// Returns the visible range start index.
    pub const fn visible_start(&self) -> usize {
        self.visible_range.start()
    }

    /// Returns the visible range end index.
    pub const fn visible_end(&self) -> usize {
        self.visible_range.end()
    }

    /// Returns the overscan range start index.
    pub const fn overscan_start(&self) -> usize {
        self.overscan_range.start()
    }

    /// Returns the overscan range end index.
    pub const fn overscan_end(&self) -> usize {
        self.overscan_range.end()
    }

    /// Returns the number of body rows rendered after overscan.
    pub const fn rendered_rows(&self) -> usize {
        self.rendered_rows
    }

    /// Returns the visible body row count before overscan.
    pub const fn visible_rows(&self) -> usize {
        self.visible_rows
    }

    /// Returns the center-row overscan budget used by the vertical virtualizer.
    pub const fn center_overscan_count(&self) -> usize {
        self.center_overscan_count
    }
}
