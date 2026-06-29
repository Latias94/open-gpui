//! Row-selection policy and summary contracts for renderer-neutral tables.

use std::collections::BTreeSet;

use super::{TableRow, TableRowId};

/// Row-selection cardinality for a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableSelectionMode {
    /// Multiple rows may be selected at once.
    #[default]
    Multiple,
    /// Exactly one row should be selected at a time.
    Single,
}

impl TableSelectionMode {
    /// Returns whether the table is single-select.
    pub const fn is_single(self) -> bool {
        matches!(self, Self::Single)
    }

    /// Returns whether the table permits multiple selected rows.
    pub const fn is_multiple(self) -> bool {
        matches!(self, Self::Multiple)
    }
}

/// How row selection is triggered from the table surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableSelectionActivationMode {
    /// Selection happens through explicit controls such as checkboxes or radios.
    #[default]
    ExplicitControl,
    /// Clicking the row surface toggles selection.
    RowClick,
}

impl TableSelectionActivationMode {
    /// Returns whether row clicks toggle selection.
    pub const fn is_row_click(self) -> bool {
        matches!(self, Self::RowClick)
    }
}

/// Whether selecting a row propagates to its descendants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableSubRowSelectionPolicy {
    /// Child rows stay independent unless they are selected directly.
    #[default]
    Independent,
    /// Selecting a row also selects all of its descendants.
    Descendants,
}

impl TableSubRowSelectionPolicy {
    /// Returns whether descendant rows are selected together with their parent.
    pub const fn propagates_descendants(self) -> bool {
        matches!(self, Self::Descendants)
    }
}

/// Policy for resolving table row selection behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableSelectionPolicy {
    selection_mode: TableSelectionMode,
    activation_mode: TableSelectionActivationMode,
    sub_row_policy: TableSubRowSelectionPolicy,
}

impl TableSelectionPolicy {
    /// Creates a selection policy from explicit mode choices.
    pub const fn new(
        selection_mode: TableSelectionMode,
        activation_mode: TableSelectionActivationMode,
        sub_row_policy: TableSubRowSelectionPolicy,
    ) -> Self {
        Self {
            selection_mode,
            activation_mode,
            sub_row_policy,
        }
    }

    /// Returns the selection cardinality.
    pub const fn selection_mode(self) -> TableSelectionMode {
        self.selection_mode
    }

    /// Returns how selection is triggered from the row surface.
    pub const fn activation_mode(self) -> TableSelectionActivationMode {
        self.activation_mode
    }

    /// Returns how selection propagates to descendant rows.
    pub const fn sub_row_policy(self) -> TableSubRowSelectionPolicy {
        self.sub_row_policy
    }

    /// Applies a selection cardinality.
    pub const fn with_selection_mode(mut self, selection_mode: TableSelectionMode) -> Self {
        self.selection_mode = selection_mode;
        self
    }

    /// Applies a row-surface activation mode.
    pub const fn with_activation_mode(
        mut self,
        activation_mode: TableSelectionActivationMode,
    ) -> Self {
        self.activation_mode = activation_mode;
        self
    }

    /// Applies a descendant-selection policy.
    pub const fn with_sub_row_policy(mut self, sub_row_policy: TableSubRowSelectionPolicy) -> Self {
        self.sub_row_policy = sub_row_policy;
        self
    }

    pub(super) fn resolve_selected_rows(
        self,
        rows: &[TableRow],
        selected_rows: &BTreeSet<TableRowId>,
    ) -> BTreeSet<TableRowId> {
        let mut resolved = self.normalize_selected_rows(selected_rows.iter().cloned());
        if self.selection_mode.is_single() {
            return resolved;
        }
        if self.sub_row_policy.propagates_descendants() {
            collect_descendant_selected_rows(rows, selected_rows, &mut resolved);
        }

        resolved
    }

    pub(super) fn normalize_selected_rows(
        self,
        selected_rows: impl IntoIterator<Item = impl Into<TableRowId>>,
    ) -> BTreeSet<TableRowId> {
        let selected_rows = selected_rows
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        if self.selection_mode.is_single() {
            return selected_rows.into_iter().next().into_iter().collect();
        }

        selected_rows
    }
}

impl Default for TableSelectionPolicy {
    fn default() -> Self {
        Self::new(
            TableSelectionMode::Multiple,
            TableSelectionActivationMode::ExplicitControl,
            TableSubRowSelectionPolicy::Independent,
        )
    }
}

/// The resolved state for one table-selection summary scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableSelectionSummaryState {
    /// No rows in the scope are selected.
    None,
    /// Some but not all rows in the scope are selected.
    Some,
    /// Every row in the scope is selected.
    All,
}

impl TableSelectionSummaryState {
    /// Returns a stable label for the summary state.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Some => "some",
            Self::All => "all",
        }
    }

    /// Returns whether the scope has no selected rows.
    pub const fn is_none(self) -> bool {
        matches!(self, Self::None)
    }

    /// Returns whether the scope has some but not all selected rows.
    pub const fn is_some(self) -> bool {
        matches!(self, Self::Some)
    }

    /// Returns whether the scope has every row selected.
    pub const fn is_all(self) -> bool {
        matches!(self, Self::All)
    }
}

/// Summary of selection across one resolved row-model scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TableSelectionSummary {
    selected_count: usize,
    total_count: usize,
}

impl TableSelectionSummary {
    /// Creates a selection summary from explicit counts.
    pub const fn new(selected_count: usize, total_count: usize) -> Self {
        Self {
            selected_count,
            total_count,
        }
    }

    /// Returns the number of selected rows in the scope.
    pub const fn selected_count(self) -> usize {
        self.selected_count
    }

    /// Returns the total number of rows in the scope.
    pub const fn total_count(self) -> usize {
        self.total_count
    }

    /// Returns the resolved state for the summary.
    pub const fn state(self) -> TableSelectionSummaryState {
        if self.total_count == 0 || self.selected_count == 0 {
            TableSelectionSummaryState::None
        } else if self.selected_count == self.total_count {
            TableSelectionSummaryState::All
        } else {
            TableSelectionSummaryState::Some
        }
    }

    /// Returns whether the scope has no selected rows.
    pub const fn is_none_selected(self) -> bool {
        self.state().is_none()
    }

    /// Returns whether the scope has some but not all selected rows.
    pub const fn is_some_selected(self) -> bool {
        self.state().is_some()
    }

    /// Returns whether the scope has every row selected.
    pub const fn is_all_selected(self) -> bool {
        self.state().is_all()
    }
}

fn collect_descendant_selected_rows(
    rows: &[TableRow],
    selected_rows: &BTreeSet<TableRowId>,
    resolved: &mut BTreeSet<TableRowId>,
) {
    for row in rows {
        if selected_rows.contains(row.id()) {
            collect_all_descendant_rows(row.children(), resolved);
            continue;
        }

        collect_descendant_selected_rows(row.children(), selected_rows, resolved);
    }
}

fn collect_all_descendant_rows(rows: &[TableRow], resolved: &mut BTreeSet<TableRowId>) {
    for row in rows {
        resolved.insert(row.id().clone());
        collect_all_descendant_rows(row.children(), resolved);
    }
}
