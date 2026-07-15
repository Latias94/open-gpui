use std::rc::Rc;

use open_gpui::{App, Window};
use open_gpui_ui_core::{
    TableExpansionState, TableResolvedRow, TableRowChildrenLoadState, TableRowId, TableRowIdentity,
    TableSelectionMode, TableSelectionPolicy,
};

use super::modifiers::TableInputModifiers;
use crate::table::{TableRowRenderPlan, TableRowSelectionHandler};

/// Row activation source for table row callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRowActivationKind {
    /// A standard pointer click activated the row.
    Click,
    /// A repeated pointer click activated the row.
    DoubleClick,
    /// Enter or Space activated the focused row.
    Keyboard,
}

impl TableRowActivationKind {
    /// Returns a stable label for logs and tests.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Click => "click",
            Self::DoubleClick => "double-click",
            Self::Keyboard => "keyboard",
        }
    }
}

/// Common row metadata carried by interactive table row callbacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRowAction {
    identity: TableRowIdentity,
    model_index: usize,
    source_index: Option<usize>,
    depth: usize,
    selected: bool,
    tree_branch: bool,
    tree_expanded: Option<bool>,
    loaded_child_count: usize,
    children_load_state: Option<TableRowChildrenLoadState>,
    modifiers: TableInputModifiers,
}

impl TableRowAction {
    pub(in crate::table) fn from_render_plan(
        row: &TableRowRenderPlan,
        modifiers: TableInputModifiers,
    ) -> Self {
        Self::from_resolved_row(row.row(), row.model_index(), modifiers)
    }

    pub(in crate::table) fn from_resolved_row(
        row: &TableResolvedRow,
        model_index: usize,
        modifiers: TableInputModifiers,
    ) -> Self {
        Self {
            identity: row.identity().clone(),
            model_index,
            source_index: row.source_index(),
            depth: row.depth(),
            selected: row.selected(),
            tree_branch: row.is_tree_branch(),
            tree_expanded: row.tree_expanded(),
            loaded_child_count: row.loaded_child_count(),
            children_load_state: row.children_load_state().cloned(),
            modifiers,
        }
    }

    /// Returns the authoritative resolved row identity.
    pub const fn identity(&self) -> &TableRowIdentity {
        &self.identity
    }

    /// Returns the caller-owned business row id for source-backed rows.
    pub const fn source_row_id(&self) -> Option<&TableRowId> {
        self.identity.source_row_id()
    }

    /// Returns this row's zero-based index in the final row model.
    pub const fn model_index(&self) -> usize {
        self.model_index
    }

    /// Returns the source-row preorder index, when this is a source row.
    pub const fn source_index(&self) -> Option<usize> {
        self.source_index
    }

    /// Returns the row depth in the resolved hierarchy.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns whether this row is selected by caller-owned table state.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether this row is a source tree branch.
    pub const fn tree_branch(&self) -> bool {
        self.tree_branch
    }

    /// Returns the resolved expanded state for source tree branches.
    pub const fn tree_expanded(&self) -> Option<bool> {
        self.tree_expanded
    }

    /// Returns the number of directly loaded child rows.
    pub const fn loaded_child_count(&self) -> usize {
        self.loaded_child_count
    }

    /// Returns source-row child loading metadata.
    pub fn children_load_state(&self) -> Option<&TableRowChildrenLoadState> {
        self.children_load_state.as_ref()
    }

    /// Returns modifier keys captured from the triggering input event.
    pub const fn modifiers(&self) -> TableInputModifiers {
        self.modifiers
    }
}

/// Controlled payload emitted when a table row is activated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRowActivation {
    action: TableRowAction,
    kind: TableRowActivationKind,
}

impl TableRowActivation {
    pub(in crate::table) fn new(action: TableRowAction, kind: TableRowActivationKind) -> Self {
        Self { action, kind }
    }

    /// Returns common row metadata.
    pub const fn action(&self) -> &TableRowAction {
        &self.action
    }

    /// Returns the source of the activation.
    pub const fn kind(&self) -> TableRowActivationKind {
        self.kind
    }

    /// Returns the authoritative activated row identity.
    pub const fn identity(&self) -> &TableRowIdentity {
        self.action.identity()
    }

    /// Returns the caller-owned business id for source-backed activations.
    pub const fn source_row_id(&self) -> Option<&TableRowId> {
        self.action.source_row_id()
    }
}

/// Controlled payload emitted when a table row expansion toggle is requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRowExpansionToggle {
    action: TableRowAction,
    expanded: bool,
}

impl TableRowExpansionToggle {
    pub(in crate::table) fn new(action: TableRowAction, expanded: bool) -> Self {
        Self { action, expanded }
    }

    /// Returns common row metadata.
    pub const fn action(&self) -> &TableRowAction {
        &self.action
    }

    /// Returns the resolved row identity whose expansion should change.
    pub const fn identity(&self) -> &TableRowIdentity {
        self.action.identity()
    }

    /// Returns the caller-owned business id for source-backed rows.
    pub const fn source_row_id(&self) -> Option<&TableRowId> {
        self.action.source_row_id()
    }

    /// Returns the desired expanded state after the toggle.
    pub const fn expanded(&self) -> bool {
        self.expanded
    }

    /// Returns the number of directly loaded child rows.
    pub const fn loaded_child_count(&self) -> usize {
        self.action.loaded_child_count()
    }

    /// Returns source-row child loading metadata.
    pub fn children_load_state(&self) -> Option<&TableRowChildrenLoadState> {
        self.action.children_load_state()
    }
}

/// Selection scope used by table selection requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableSelectionScope {
    /// Only the current row changes.
    #[default]
    Row,
    /// Every selectable row in the model changes.
    AllRows,
    /// Every selectable row in the current page changes.
    PageRows,
}

impl TableSelectionScope {
    /// Returns a stable label for the scope.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::AllRows => "all-rows",
            Self::PageRows => "page-rows",
        }
    }
}

/// Controlled payload emitted when a table row selection changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRowSelectionChange {
    action: TableRowAction,
    selection_mode: TableSelectionMode,
    selected: bool,
    scope: TableSelectionScope,
    current_selection: Vec<TableRowId>,
}

impl TableRowSelectionChange {
    fn new(
        action: TableRowAction,
        selection_mode: TableSelectionMode,
        selected: bool,
        scope: TableSelectionScope,
        current_selection: impl IntoIterator<Item = impl Into<TableRowId>>,
    ) -> Self {
        Self {
            action,
            selection_mode,
            selected,
            scope,
            current_selection: current_selection.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns common row metadata.
    pub const fn action(&self) -> &TableRowAction {
        &self.action
    }

    /// Returns the resolved row identity whose selection changed.
    pub const fn identity(&self) -> &TableRowIdentity {
        self.action.identity()
    }

    /// Returns the caller-owned business id for the selected source row.
    pub const fn source_row_id(&self) -> Option<&TableRowId> {
        self.action.source_row_id()
    }

    /// Returns the selection mode used for this row surface.
    pub const fn selection_mode(&self) -> TableSelectionMode {
        self.selection_mode
    }

    /// Returns whether the row is selected after the change.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns the requested selection scope.
    pub const fn scope(&self) -> TableSelectionScope {
        self.scope
    }

    /// Returns the current selected row ids after the change.
    pub fn current_selection(&self) -> &[TableRowId] {
        &self.current_selection
    }
}

pub(in crate::table) fn request_table_row_selection_change(
    action: &TableRowAction,
    selection_policy: TableSelectionPolicy,
    scope: TableSelectionScope,
    selected_row_ids: Rc<Vec<TableRowId>>,
    on_row_selection_change: Option<TableRowSelectionHandler>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(row_id) = action.source_row_id() else {
        return false;
    };
    let current_selected = action.selected();
    let selection_mode = selection_policy.selection_mode();
    let next_selection = if selection_mode.is_single() {
        true
    } else {
        !current_selected
    };

    if selection_mode.is_single() && current_selected {
        return false;
    }

    let next_selection_ids = if next_selection {
        if selection_mode.is_single() {
            vec![row_id.clone()]
        } else {
            let mut next_selection_ids = selected_row_ids.as_ref().clone();
            next_selection_ids.push(row_id.clone());
            next_selection_ids
        }
    } else {
        selected_row_ids
            .iter()
            .filter(|candidate| *candidate != row_id)
            .cloned()
            .collect()
    };

    let change = TableRowSelectionChange::new(
        action.clone(),
        selection_mode,
        next_selection,
        scope,
        next_selection_ids,
    );

    if let Some(on_row_selection_change) = on_row_selection_change.as_ref() {
        on_row_selection_change(change, window, cx);
        return true;
    }

    false
}

pub(in crate::table) fn toggle_table_expansion(
    expansion: TableExpansionState,
    identity: TableRowIdentity,
    expanded: bool,
) -> TableExpansionState {
    match expansion {
        TableExpansionState::All if expanded => TableExpansionState::All,
        TableExpansionState::All => TableExpansionState::default(),
        TableExpansionState::Rows(mut rows) => {
            if expanded {
                rows.insert(identity);
            } else {
                rows.remove(&identity);
            }
            TableExpansionState::Rows(rows)
        }
    }
}
