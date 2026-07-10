//! Runtime interaction logs for rendered component samples.

use open_gpui::{App, AppContext, BorrowAppContext, Global};
use open_gpui_ui_components::{
    TableCellEditApplyOutcome, TableCellEditChange, TableColumnOrderChange,
    TableColumnSizingChange, TableColumnVisibilityChange, TableFacetedFilterChange,
    TableGlobalFilterChange, TablePredicateFilterChange, TableRangeFilterChange,
    TableRowActivation, TableRowExpansionToggle, TreeItemDescriptor, TreeMove, apply_tree_move,
};
use open_gpui_ui_core::{
    TableColumnId, TableColumnSizing, TableColumnVisibilityOverrides, TableExpansionState,
    TableRowChildrenLoadState, TableState, UiPx,
};
use std::collections::BTreeMap;

use super::samples::{TableSample, server_tree_table_state};

/// Deterministic read-only runtime log for gallery integration samples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleRuntimeLog<T> {
    sample_id: &'static str,
    entries: Vec<T>,
}

impl<T> SampleRuntimeLog<T> {
    /// Creates a sample runtime log.
    pub fn new(sample_id: &'static str, entries: impl Into<Vec<T>>) -> Self {
        Self {
            sample_id,
            entries: entries.into(),
        }
    }

    /// Returns the stable sample id.
    pub const fn sample_id(&self) -> &'static str {
        self.sample_id
    }

    /// Returns the deterministic log entries.
    pub fn entries(&self) -> &[T] {
        &self.entries
    }

    /// Returns the number of deterministic entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true when this log has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[path = "runtime/form.rs"]
mod form;
#[path = "runtime/resource.rs"]
mod resource;
#[path = "runtime/table.rs"]
mod table;
#[path = "runtime/tree.rs"]
mod tree;
#[path = "runtime/virtualized_list.rs"]
mod virtualized_list;

pub use form::{
    FormSampleRuntimeAction, FormSampleRuntimeCompletion, FormSampleRuntimeEvent,
    FormSampleRuntimeLog, form_sample_runtime_log,
};
pub use resource::{
    ResourceSampleRuntimeEvent, ResourceSampleRuntimeLog, resource_sample_runtime_log,
};
pub use table::{
    TableSampleCellEditChange, TableSampleColumnOrderChange, TableSampleColumnVisibilityChange,
    TableSampleExpansionToggle, TableSampleFacetedFilterChange, TableSampleGlobalFilterChange,
    TableSamplePredicateFilterChange, TableSampleRangeFilterChange, TableSampleRowActivation,
    TableSampleRuntimeLog, TableSampleSizingChange, current_table_sample_expansion,
    current_table_sample_sizing, record_table_cell_edit_change, record_table_column_order_change,
    record_table_column_visibility_change, record_table_expansion_request,
    record_table_faceted_filter_change, record_table_global_filter_change,
    record_table_predicate_filter_change, record_table_range_filter_change,
    record_table_row_activation, record_table_sizing_change, table_sample_state_with_runtime,
};
pub use tree::{
    TreeSampleMoveEvent, TreeSampleRuntimeLog, TreeSampleSelection, TreeSampleToggleEvent,
    current_tree_sample_items, record_tree_move, record_tree_selection, record_tree_toggle,
};
pub use virtualized_list::{
    VirtualizedListSampleActivation, VirtualizedListSampleNestedAction,
    VirtualizedListSampleRuntimeLog, record_virtualized_list_activation,
    record_virtualized_list_nested_action,
};
