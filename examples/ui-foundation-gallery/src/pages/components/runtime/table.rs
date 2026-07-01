use super::*;

/// One committed column sizing change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq)]
pub struct TableSampleSizingChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Resized column id.
    pub column_id: String,
    /// Committed resolved width.
    pub width: UiPx,
}

/// One row activation captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleRowActivation {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Activated row id.
    pub row_id: String,
    /// Concrete render key used by the adapter selectors.
    pub render_key: String,
    /// Stable activation kind label.
    pub kind: String,
    /// Final row-model index at activation time.
    pub model_index: usize,
    /// Resolved hierarchy depth at activation time.
    pub depth: usize,
    /// Whether the row is a source tree branch.
    pub tree_branch: bool,
    /// Resolved branch expansion state, when applicable.
    pub tree_expanded: Option<bool>,
    /// Whether the row was selected in caller-owned table state.
    pub selected: bool,
}

/// One source-tree expansion request captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleExpansionToggle {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Toggled row id.
    pub row_id: String,
    /// Desired expanded state after the toggle.
    pub expanded: bool,
    /// Resolved hierarchy depth at toggle time.
    pub depth: usize,
    /// Number of directly loaded child rows at toggle time.
    pub loaded_child_count: usize,
    /// Stable child loading state label at toggle time.
    pub children_load_state: String,
    /// Optional loading or failure message at toggle time.
    pub children_load_message: Option<String>,
}

/// One table-cell edit captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleCellEditChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Edited row id.
    pub row_id: String,
    /// Edited column id.
    pub column_id: String,
    /// Source-row index carried by the edit payload, when available.
    pub source_index: Option<usize>,
    /// Resolved text before the edit.
    pub previous_text: String,
    /// Next controlled text value.
    pub next_text: String,
    /// Result from applying the change to app-owned sample state.
    pub outcome: String,
}

/// Runtime interaction log used by gallery Table smoke tests.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TableSampleRuntimeLog {
    sizing_changes: Vec<TableSampleSizingChange>,
    committed_sizing: BTreeMap<String, TableColumnSizing>,
    row_activations: Vec<TableSampleRowActivation>,
    expansion_toggles: Vec<TableSampleExpansionToggle>,
    expansion_overrides: BTreeMap<String, TableExpansionState>,
    global_filter_changes: Vec<TableSampleGlobalFilterChange>,
    predicate_filter_changes: Vec<TableSamplePredicateFilterChange>,
    filter_overrides: BTreeMap<String, TableState>,
    visibility_changes: Vec<TableSampleColumnVisibilityChange>,
    visibility_overrides: BTreeMap<String, TableColumnVisibilityOverrides>,
    column_order_changes: Vec<TableSampleColumnOrderChange>,
    column_order_overrides: BTreeMap<String, Vec<TableColumnId>>,
    faceted_filter_changes: Vec<TableSampleFacetedFilterChange>,
    range_filter_changes: Vec<TableSampleRangeFilterChange>,
    cell_edit_changes: Vec<TableSampleCellEditChange>,
    cell_edit_overrides: BTreeMap<String, TableState>,
    server_tree_loaded: BTreeMap<String, bool>,
}

/// One global-filter change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleGlobalFilterChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Filter query text.
    pub query: String,
    /// Whether this payload clears the global filter.
    pub cleared: bool,
    /// Filtered row count after the change.
    pub filtered_rows: usize,
    /// Final row count after pagination after the change.
    pub final_rows: usize,
}

/// One predicate-filter change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSamplePredicateFilterChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Filtered column id.
    pub column_id: String,
    /// Stable operator value, when this is not a clear action.
    pub operator: Option<String>,
    /// Raw predicate value text.
    pub value: String,
    /// Whether this payload clears the predicate.
    pub cleared: bool,
    /// Filtered row count after the change.
    pub filtered_rows: usize,
    /// Final row count after pagination after the change.
    pub final_rows: usize,
}

/// One column-visibility change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleColumnVisibilityChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Stable visibility action label.
    pub action: String,
    /// Affected column ids.
    pub column_ids: Vec<String>,
    /// Next visibility for the affected columns, if the action sets one.
    pub next_visible: Option<bool>,
    /// Visible column count after the change.
    pub visible_columns: usize,
    /// Hidden column count after the change.
    pub hidden_columns: usize,
}

/// One column-order change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleColumnOrderChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Moved column id.
    pub column_id: String,
    /// Target column id.
    pub target_column_id: String,
    /// Stable insertion placement label.
    pub placement: String,
    /// Shared column region for the move.
    pub region: String,
    /// Full column order after the change.
    pub column_order: Vec<String>,
}

/// One faceted-filter change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSampleFacetedFilterChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Filtered column id.
    pub column_id: String,
    /// Exact categorical tokens selected after the change.
    pub selected_values: Vec<String>,
    /// Token that was toggled, if any.
    pub toggled_value: Option<String>,
    /// Whether the toggled token is selected after the change.
    pub selected: bool,
    /// Filtered row count after the change.
    pub filtered_rows: usize,
    /// Final row count after pagination after the change.
    pub final_rows: usize,
}

/// One range-filter change captured from the rendered gallery `Table` sample.
#[derive(Debug, Clone, PartialEq)]
pub struct TableSampleRangeFilterChange {
    /// Stable gallery sample id.
    pub sample_id: String,
    /// Filtered column id.
    pub column_id: String,
    /// Lower endpoint text.
    pub min_text: String,
    /// Upper endpoint text.
    pub max_text: String,
    /// Parsed lower endpoint after normalization.
    pub min_value: Option<f64>,
    /// Parsed upper endpoint after normalization.
    pub max_value: Option<f64>,
    /// Whether this payload clears the range.
    pub cleared: bool,
    /// Filtered row count after the change.
    pub filtered_rows: usize,
    /// Final row count after pagination after the change.
    pub final_rows: usize,
}

impl Global for TableSampleRuntimeLog {}

impl TableSampleRuntimeLog {
    /// Returns captured sizing changes in event order.
    pub fn sizing_changes(&self) -> &[TableSampleSizingChange] {
        &self.sizing_changes
    }

    /// Returns the latest committed sizing for a sample, if any.
    pub fn committed_sizing(&self, sample_id: &str) -> Option<&TableColumnSizing> {
        self.committed_sizing.get(sample_id)
    }

    /// Returns captured row activations in event order.
    pub fn row_activations(&self) -> &[TableSampleRowActivation] {
        &self.row_activations
    }

    /// Returns captured source-tree expansion requests in event order.
    pub fn expansion_toggles(&self) -> &[TableSampleExpansionToggle] {
        &self.expansion_toggles
    }

    /// Returns the current controlled expansion override for a sample, if any.
    pub fn expansion_override(&self, sample_id: &str) -> Option<&TableExpansionState> {
        self.expansion_overrides.get(sample_id)
    }

    /// Returns captured global-filter changes in event order.
    pub fn global_filter_changes(&self) -> &[TableSampleGlobalFilterChange] {
        &self.global_filter_changes
    }

    /// Returns the current controlled global-filter state for a sample, if any.
    pub fn global_filter_override(&self, sample_id: &str) -> Option<&TableState> {
        self.filter_overrides.get(sample_id)
    }

    /// Returns captured predicate-filter changes in event order.
    pub fn predicate_filter_changes(&self) -> &[TableSamplePredicateFilterChange] {
        &self.predicate_filter_changes
    }

    /// Returns the current controlled predicate-filter state for a sample, if any.
    pub fn predicate_filter_override(&self, sample_id: &str) -> Option<&TableState> {
        self.filter_overrides.get(sample_id)
    }

    /// Returns captured column-visibility changes in event order.
    pub fn visibility_changes(&self) -> &[TableSampleColumnVisibilityChange] {
        &self.visibility_changes
    }

    /// Returns the current controlled column-visibility state for a sample, if any.
    pub fn visibility_override(&self, sample_id: &str) -> Option<&TableColumnVisibilityOverrides> {
        self.visibility_overrides.get(sample_id)
    }

    /// Returns captured column-order changes in event order.
    pub fn column_order_changes(&self) -> &[TableSampleColumnOrderChange] {
        &self.column_order_changes
    }

    /// Returns the current controlled column-order state for a sample, if any.
    pub fn column_order_override(&self, sample_id: &str) -> Option<&[TableColumnId]> {
        self.column_order_overrides
            .get(sample_id)
            .map(Vec::as_slice)
    }

    /// Returns captured faceted filter changes in event order.
    pub fn faceted_filter_changes(&self) -> &[TableSampleFacetedFilterChange] {
        &self.faceted_filter_changes
    }

    /// Returns the current controlled faceted filter state for a sample, if any.
    pub fn faceted_filter_override(&self, sample_id: &str) -> Option<&TableState> {
        self.filter_overrides.get(sample_id)
    }

    /// Returns captured range filter changes in event order.
    pub fn range_filter_changes(&self) -> &[TableSampleRangeFilterChange] {
        &self.range_filter_changes
    }

    /// Returns the current controlled range filter state for a sample, if any.
    pub fn range_filter_override(&self, sample_id: &str) -> Option<&TableState> {
        self.filter_overrides.get(sample_id)
    }

    /// Returns captured text-cell edits in event order.
    pub fn cell_edit_changes(&self) -> &[TableSampleCellEditChange] {
        &self.cell_edit_changes
    }

    /// Returns the current controlled cell-edit state for a sample, if any.
    pub fn cell_edit_override(&self, sample_id: &str) -> Option<&TableState> {
        self.cell_edit_overrides.get(sample_id)
    }

    /// Clears captured interactions.
    pub fn clear(&mut self) {
        self.sizing_changes.clear();
        self.committed_sizing.clear();
        self.row_activations.clear();
        self.expansion_toggles.clear();
        self.expansion_overrides.clear();
        self.global_filter_changes.clear();
        self.predicate_filter_changes.clear();
        self.filter_overrides.clear();
        self.visibility_changes.clear();
        self.visibility_overrides.clear();
        self.column_order_changes.clear();
        self.column_order_overrides.clear();
        self.faceted_filter_changes.clear();
        self.range_filter_changes.clear();
        self.cell_edit_changes.clear();
        self.cell_edit_overrides.clear();
        self.server_tree_loaded.clear();
    }
}

/// Returns the current committed sizing for a gallery `Table` sample.
pub fn current_table_sample_sizing(
    sample_id: impl Into<String>,
    fallback: &TableColumnSizing,
    cx: &impl AppContext,
) -> TableColumnSizing {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.committed_sizing
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Returns the current controlled expansion state for a gallery `Table` sample.
pub fn current_table_sample_expansion(
    sample_id: impl Into<String>,
    fallback: &TableExpansionState,
    cx: &impl AppContext,
) -> TableExpansionState {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.expansion_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Returns the current controlled faceted-filter state for a gallery `Table` sample.
pub fn current_table_sample_faceted_filter_state(
    sample_id: impl Into<String>,
    fallback: &TableState,
    cx: &impl AppContext,
) -> TableState {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Returns the current controlled range-filter state for a gallery `Table` sample.
pub fn current_table_sample_range_filter_state(
    sample_id: impl Into<String>,
    fallback: &TableState,
    cx: &impl AppContext,
) -> TableState {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Returns the current controlled global-filter state for a gallery `Table` sample.
pub fn current_table_sample_global_filter_state(
    sample_id: impl Into<String>,
    fallback: &TableState,
    cx: &impl AppContext,
) -> TableState {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Returns the current controlled predicate-filter state for a gallery `Table` sample.
pub fn current_table_sample_predicate_filter_state(
    sample_id: impl Into<String>,
    fallback: &TableState,
    cx: &impl AppContext,
) -> TableState {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Returns the current controlled column-visibility overrides for a gallery `Table` sample.
pub fn current_table_sample_column_visibility_overrides(
    sample_id: impl Into<String>,
    fallback: &TableColumnVisibilityOverrides,
    cx: &impl AppContext,
) -> TableColumnVisibilityOverrides {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.visibility_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

fn table_state_effective_column_order(state: &TableState) -> Vec<TableColumnId> {
    if state.column_order().is_empty() {
        state
            .columns()
            .iter()
            .map(|column| column.id().clone())
            .collect()
    } else {
        state.column_order().to_vec()
    }
}

/// Returns the current controlled column-order state for a gallery `Table` sample.
pub fn current_table_sample_column_order(
    sample_id: impl Into<String>,
    fallback: &TableState,
    cx: &impl AppContext,
) -> Vec<TableColumnId> {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.column_order_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| table_state_effective_column_order(fallback))
    })
}

/// Returns the current controlled text-cell edit state for a gallery `Table` sample.
pub fn current_table_sample_cell_edit_state(
    sample_id: impl Into<String>,
    fallback: &TableState,
    cx: &impl AppContext,
) -> TableState {
    let sample_id = sample_id.into();
    cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone())
    })
}

/// Applies a resolved expansion state to a sample table state.
pub fn table_state_with_expansion(state: TableState, expansion: TableExpansionState) -> TableState {
    match expansion {
        TableExpansionState::All => state.with_all_rows_expanded(),
        TableExpansionState::Rows(rows) => state.with_expanded_rows(rows),
    }
}

/// Applies current gallery runtime overrides to a table sample state.
pub fn table_sample_state_with_runtime(
    sample: &TableSample,
    sizing: TableColumnSizing,
    expansion: TableExpansionState,
    cx: &impl AppContext,
) -> TableState {
    let state = current_table_sample_global_filter_state(sample.id, &sample.state, cx);
    let state = current_table_sample_predicate_filter_state(sample.id, &state, cx);
    let state = current_table_sample_faceted_filter_state(sample.id, &state, cx);
    let state = current_table_sample_range_filter_state(sample.id, &state, cx);
    let state = current_table_sample_cell_edit_state(sample.id, &state, cx);
    let loaded_server_tree = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.server_tree_loaded
            .get(sample.id)
            .copied()
            .unwrap_or(false)
    });
    let state = if sample.id == "server-tree" && loaded_server_tree {
        server_tree_table_state(true)
    } else {
        state
    };
    let column_order = current_table_sample_column_order(sample.id, &state, cx);
    let state = state.with_column_order(column_order);
    let visibility =
        current_table_sample_column_visibility_overrides(sample.id, state.column_visibility(), cx);
    let state = state.with_column_visibility(visibility);

    table_state_with_expansion(state.with_column_sizing(sizing), expansion)
}

/// Records a gallery `Table` sizing commit in app-global sample state.
pub fn record_table_sizing_change(
    sample_id: impl Into<String>,
    change: &TableColumnSizingChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.sizing_changes.push(TableSampleSizingChange {
            sample_id: sample_id.clone(),
            column_id: change.column_id().as_str().to_owned(),
            width: change.width(),
        });
        log.committed_sizing
            .insert(sample_id, change.sizing().clone());
    });
}

/// Records and applies a controlled gallery `Table` column-order change.
pub fn record_table_column_order_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TableColumnOrderChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = table_state_effective_column_order(fallback);
    let next = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current = log
            .column_order_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        change.apply_to_order(current)
    });

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.column_order_changes.push(TableSampleColumnOrderChange {
            sample_id: sample_id.clone(),
            column_id: change.column_id().as_str().to_owned(),
            target_column_id: change.target_column_id().as_str().to_owned(),
            placement: change.placement().as_str().to_owned(),
            region: change.target_region().as_str().to_owned(),
            column_order: next
                .iter()
                .map(|column_id| column_id.as_str().to_owned())
                .collect(),
        });
        log.column_order_overrides.insert(sample_id, next);
    });
}

/// Records a gallery `Table` row activation in app-global sample state.
pub fn record_table_row_activation(
    sample_id: impl Into<String>,
    activation: &TableRowActivation,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let action = activation.action();
    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.row_activations.push(TableSampleRowActivation {
            sample_id,
            row_id: activation.row_id().as_str().to_owned(),
            render_key: action.render_key().to_owned(),
            kind: activation.kind().as_str().to_owned(),
            model_index: action.model_index(),
            depth: action.depth(),
            tree_branch: action.tree_branch(),
            tree_expanded: action.tree_expanded(),
            selected: action.selected(),
        });
    });
}

/// Records and applies a controlled gallery `Table` source-tree expansion request.
pub fn record_table_expansion_request(
    sample_id: impl Into<String>,
    fallback: &TableExpansionState,
    toggle: &TableRowExpansionToggle,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let row_id = toggle.row_id().clone();
    let expanded = toggle.expanded();
    let depth = toggle.action().depth();
    let loaded_child_count = toggle.loaded_child_count();
    let children_load_state = toggle
        .children_load_state()
        .map(TableRowChildrenLoadState::as_str)
        .unwrap_or("none")
        .to_owned();
    let children_load_message = toggle
        .children_load_state()
        .and_then(TableRowChildrenLoadState::message)
        .map(str::to_owned);

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.expansion_toggles.push(TableSampleExpansionToggle {
            sample_id: sample_id.clone(),
            row_id: row_id.as_str().to_owned(),
            expanded,
            depth,
            loaded_child_count,
            children_load_state,
            children_load_message,
        });
        if sample_id == "server-tree" && row_id.as_str() == "server-workspace" && expanded {
            log.server_tree_loaded.insert(sample_id.clone(), true);
        }

        let current = log
            .expansion_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        let next = match current {
            TableExpansionState::All if expanded => TableExpansionState::All,
            TableExpansionState::All => TableExpansionState::default(),
            TableExpansionState::Rows(mut rows) => {
                if expanded {
                    rows.insert(row_id);
                } else {
                    rows.remove(&row_id);
                }
                TableExpansionState::Rows(rows)
            }
        };
        log.expansion_overrides.insert(sample_id, next);
    });
}

/// Records and applies a controlled gallery `Table` faceted-filter change.
pub fn record_table_faceted_filter_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TableFacetedFilterChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let next = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current = log
            .filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        change.apply_to(current)
    });
    let resolved = next.resolve();
    let filtered_rows = resolved.filtered_model().rows().len();
    let final_rows = resolved.final_model().rows().len();

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.faceted_filter_changes
            .push(TableSampleFacetedFilterChange {
                sample_id: sample_id.clone(),
                column_id: change.column_id().as_str().to_owned(),
                selected_values: change.selected_values().to_vec(),
                toggled_value: change.toggled_value().map(str::to_owned),
                selected: change.selected(),
                filtered_rows,
                final_rows,
            });
        log.filter_overrides.insert(sample_id, next);
    });
}

/// Records and applies a controlled gallery `Table` range-filter change.
pub fn record_table_range_filter_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TableRangeFilterChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let next = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current = log
            .filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        change.apply_to(current)
    });
    let resolved = next.resolve();
    let filtered_rows = resolved.filtered_model().rows().len();
    let final_rows = resolved.final_model().rows().len();

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.range_filter_changes.push(TableSampleRangeFilterChange {
            sample_id: sample_id.clone(),
            column_id: change.column_id().as_str().to_owned(),
            min_text: change.min_text().to_owned(),
            max_text: change.max_text().to_owned(),
            min_value: change.min_value(),
            max_value: change.max_value(),
            cleared: change.cleared(),
            filtered_rows,
            final_rows,
        });
        log.filter_overrides.insert(sample_id, next);
    });
}

/// Records and applies a controlled gallery `Table` global-filter change.
pub fn record_table_global_filter_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TableGlobalFilterChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let next = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current = log
            .filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        change.apply_to(current)
    });
    let resolved = next.resolve();
    let filtered_rows = resolved.filtered_model().rows().len();
    let final_rows = resolved.final_model().rows().len();

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.global_filter_changes
            .push(TableSampleGlobalFilterChange {
                sample_id: sample_id.clone(),
                query: change.query().to_owned(),
                cleared: change.cleared(),
                filtered_rows,
                final_rows,
            });
        log.filter_overrides.insert(sample_id, next);
    });
}

/// Records and applies a controlled gallery `Table` predicate-filter change.
pub fn record_table_predicate_filter_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TablePredicateFilterChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let next = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current = log
            .filter_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        change.apply_to(current)
    });
    let resolved = next.resolve();
    let filtered_rows = resolved.filtered_model().rows().len();
    let final_rows = resolved.final_model().rows().len();

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.predicate_filter_changes
            .push(TableSamplePredicateFilterChange {
                sample_id: sample_id.clone(),
                column_id: change.column_id().as_str().to_owned(),
                operator: change
                    .operator()
                    .map(|operator| operator.as_str().to_owned()),
                value: change.value().to_owned(),
                cleared: change.cleared(),
                filtered_rows,
                final_rows,
            });
        log.filter_overrides.insert(sample_id, next);
    });
}

/// Records and applies a controlled gallery `Table` column-visibility change.
pub fn record_table_column_visibility_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TableColumnVisibilityChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let next = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current_visibility = log
            .visibility_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.column_visibility().clone());
        let current = fallback.clone().with_column_visibility(current_visibility);
        change.apply_to(current)
    });
    let visible_columns = next.visible_columns().len();
    let hidden_columns = next.columns().len().saturating_sub(visible_columns);
    let next_visibility = next.column_visibility().clone();

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.visibility_changes
            .push(TableSampleColumnVisibilityChange {
                sample_id: sample_id.clone(),
                action: change.action().as_str().to_owned(),
                column_ids: change
                    .column_ids()
                    .iter()
                    .map(|column_id| column_id.as_str().to_owned())
                    .collect(),
                next_visible: change.next_visible(),
                visible_columns,
                hidden_columns,
            });
        log.visibility_overrides.insert(sample_id, next_visibility);
    });
}

/// Records and applies a controlled gallery `Table` cell edit.
pub fn record_table_cell_edit_change(
    sample_id: impl Into<String>,
    fallback: &TableState,
    change: &TableCellEditChange,
    cx: &mut App,
) {
    let sample_id = sample_id.into();
    let fallback = fallback.clone();
    let (next, outcome) = cx.read_global::<TableSampleRuntimeLog, _>(|log, _| {
        let current = log
            .cell_edit_overrides
            .get(&sample_id)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        change.apply_to(current)
    });

    cx.update_default_global::<TableSampleRuntimeLog, _>(|log, _| {
        log.cell_edit_changes.push(TableSampleCellEditChange {
            sample_id: sample_id.clone(),
            row_id: change.row_id().as_str().to_owned(),
            column_id: change.column_id().as_str().to_owned(),
            source_index: change.source_index(),
            previous_text: change.previous_text().to_owned(),
            next_text: change.next_text().to_owned(),
            outcome: outcome.as_str().to_owned(),
        });
        if outcome == TableCellEditApplyOutcome::Updated {
            log.cell_edit_overrides.insert(sample_id, next);
        }
    });
}
