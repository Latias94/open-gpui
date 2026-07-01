use std::collections::BTreeSet;

use open_gpui_ui_core::{TableResolvedState, UiPx};

use crate::table::render_plan::TableRenderPlan;
/// Header behavior summary without exposing header render-plan rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableHeaderSummarySnapshot {
    header_rows: usize,
    visible_group_headers: usize,
    sticky_header_band_height: UiPx,
}

impl TableHeaderSummarySnapshot {
    pub(in crate::table::behavior) fn from_table(
        plan: &TableRenderPlan,
        table: &TableResolvedState,
    ) -> Self {
        let visible_group_headers = table
            .header_groups()
            .all()
            .flat_map(|group| group.headers().iter())
            .filter(|cell| cell.is_group())
            .map(|cell| cell.source_id().to_owned())
            .collect::<BTreeSet<_>>()
            .len();

        Self {
            header_rows: plan.header_row_count(),
            visible_group_headers,
            sticky_header_band_height: plan.sticky_header_band_height(),
        }
    }

    /// Returns the maximum visible header row count across regions.
    pub const fn header_rows(self) -> usize {
        self.header_rows
    }

    /// Returns the number of visible group header identities.
    pub const fn visible_group_headers(self) -> usize {
        self.visible_group_headers
    }

    /// Returns the table header band height.
    pub const fn sticky_header_band_height(self) -> UiPx {
        self.sticky_header_band_height
    }
}
