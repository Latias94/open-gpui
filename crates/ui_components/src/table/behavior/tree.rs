use open_gpui_ui_core::{TableResolvedRow, TableRowChildrenLoadState};
/// Source tree and grouped-row behavior summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableTreeSummarySnapshot {
    tree_rows: usize,
    tree_branch_rows: usize,
    unloaded_tree_branches: usize,
    loading_tree_rows: usize,
    failed_tree_rows: usize,
    tree_depth: usize,
}

impl TableTreeSummarySnapshot {
    pub(in crate::table::behavior) fn from_rows(rows: &[TableResolvedRow]) -> Self {
        Self {
            tree_rows: rows.iter().filter(|row| row.tree().is_some()).count(),
            tree_branch_rows: rows.iter().filter(|row| row.is_tree_branch()).count(),
            unloaded_tree_branches: rows
                .iter()
                .filter(|row| {
                    row.is_tree_branch()
                        && row.loaded_child_count() == 0
                        && row
                            .children_load_state()
                            .is_some_and(|state| *state == TableRowChildrenLoadState::Idle)
                })
                .count(),
            loading_tree_rows: rows
                .iter()
                .filter(|row| {
                    row.children_load_state()
                        .is_some_and(TableRowChildrenLoadState::is_loading)
                })
                .count(),
            failed_tree_rows: rows
                .iter()
                .filter(|row| {
                    row.children_load_state()
                        .is_some_and(TableRowChildrenLoadState::is_failed)
                })
                .count(),
            tree_depth: rows.iter().map(TableResolvedRow::depth).max().unwrap_or(0),
        }
    }

    /// Returns source tree row count.
    pub const fn tree_rows(self) -> usize {
        self.tree_rows
    }

    /// Returns source tree branch row count.
    pub const fn tree_branch_rows(self) -> usize {
        self.tree_branch_rows
    }

    /// Returns unloaded idle branch count.
    pub const fn unloaded_tree_branches(self) -> usize {
        self.unloaded_tree_branches
    }

    /// Returns loading branch count.
    pub const fn loading_tree_rows(self) -> usize {
        self.loading_tree_rows
    }

    /// Returns failed branch count.
    pub const fn failed_tree_rows(self) -> usize {
        self.failed_tree_rows
    }

    /// Returns maximum source tree depth.
    pub const fn tree_depth(self) -> usize {
        self.tree_depth
    }
}
