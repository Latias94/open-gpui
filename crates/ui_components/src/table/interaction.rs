use std::rc::Rc;

use open_gpui::{App, Entity, Modifiers, Window};
use open_gpui_ui_core::{
    TableColumn, TableColumnId, TableColumnRegion, TableColumnSizing, TableExpansionState,
    TableRowChildrenLoadState, TableRowId, TableSelectionMode, TableSelectionPolicy, TableSort,
    TableSortDirection, TableState, UiPx,
};

use super::{TableRowRenderPlan, TableRowSelectionHandler, TableRuntime};

/// Relative placement for a controlled table column reorder change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableColumnOrderPlacement {
    /// Place the moved column before the target column.
    Before,
    /// Place the moved column after the target column.
    After,
}

impl TableColumnOrderPlacement {
    /// Returns a stable placement label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

/// Controlled payload emitted when a table column reorder is committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumnOrderChange {
    column_id: TableColumnId,
    target_column_id: TableColumnId,
    placement: TableColumnOrderPlacement,
    source_region: TableColumnRegion,
    target_region: TableColumnRegion,
}

impl TableColumnOrderChange {
    /// Creates a payload that moves one column before another within the same region.
    pub fn move_before(
        column_id: impl Into<TableColumnId>,
        target_column_id: impl Into<TableColumnId>,
        region: TableColumnRegion,
    ) -> Self {
        Self {
            column_id: column_id.into(),
            target_column_id: target_column_id.into(),
            placement: TableColumnOrderPlacement::Before,
            source_region: region,
            target_region: region,
        }
    }

    /// Creates a payload that moves one column after another within the same region.
    pub fn move_after(
        column_id: impl Into<TableColumnId>,
        target_column_id: impl Into<TableColumnId>,
        region: TableColumnRegion,
    ) -> Self {
        Self {
            column_id: column_id.into(),
            target_column_id: target_column_id.into(),
            placement: TableColumnOrderPlacement::After,
            source_region: region,
            target_region: region,
        }
    }

    /// Returns the moved column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the target column identity.
    pub const fn target_column_id(&self) -> &TableColumnId {
        &self.target_column_id
    }

    /// Returns the requested insertion placement.
    pub const fn placement(&self) -> TableColumnOrderPlacement {
        self.placement
    }

    /// Returns the source column region at drag start.
    pub const fn source_region(&self) -> TableColumnRegion {
        self.source_region
    }

    /// Returns the target column region at drop time.
    pub const fn target_region(&self) -> TableColumnRegion {
        self.target_region
    }

    /// Applies this reorder request to a table state.
    pub fn apply_to(&self, state: TableState) -> TableState {
        if self.source_region != self.target_region {
            return state;
        }

        let current_order = effective_table_column_order(&state);
        let Some(next_order) = reorder_table_column_order(
            current_order,
            self.column_id.clone(),
            self.target_column_id.clone(),
            self.placement,
        ) else {
            return state;
        };

        state.with_column_order(next_order)
    }

    /// Applies this reorder request to an explicit column-order list.
    pub fn apply_to_order<I>(&self, column_order: I) -> Vec<TableColumnId>
    where
        I: IntoIterator<Item = TableColumnId>,
    {
        let column_order = column_order.into_iter().collect::<Vec<_>>();
        reorder_table_column_order(
            column_order.clone(),
            self.column_id.clone(),
            self.target_column_id.clone(),
            self.placement,
        )
        .unwrap_or(column_order)
    }
}

/// Sort request emitted by an interactive table column header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableHeaderAction {
    column_id: TableColumnId,
    label: String,
    current_direction: Option<TableSortDirection>,
    next_direction: Option<TableSortDirection>,
    next_sorting: Vec<TableSort>,
}

impl TableHeaderAction {
    pub(super) fn for_column(
        column: &TableColumn,
        current_direction: Option<TableSortDirection>,
    ) -> Self {
        let next_direction = match current_direction {
            None => Some(TableSortDirection::Ascending),
            Some(TableSortDirection::Ascending) => Some(TableSortDirection::Descending),
            Some(TableSortDirection::Descending) => None,
        };
        let next_sorting = next_direction
            .map(|direction| vec![TableSort::new(column.id().clone(), direction)])
            .unwrap_or_default();

        Self {
            column_id: column.id().clone(),
            label: column.label().to_owned(),
            current_direction,
            next_direction,
            next_sorting,
        }
    }

    /// Returns the activated column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the activated column label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the currently resolved sort direction for the column.
    pub const fn current_direction(&self) -> Option<TableSortDirection> {
        self.current_direction
    }

    /// Returns the direction that should be applied by the next state update.
    pub const fn next_direction(&self) -> Option<TableSortDirection> {
        self.next_direction
    }

    /// Returns the next single-column sorting state.
    pub fn next_sorting(&self) -> &[TableSort] {
        &self.next_sorting
    }

    /// Applies this header action to a table state.
    pub fn apply_to(&self, state: TableState) -> TableState {
        state.with_sorting(self.next_sorting.clone())
    }
}

/// Controlled payload emitted when a table column resize commits.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnSizingChange {
    column_id: TableColumnId,
    width: UiPx,
    sizing: TableColumnSizing,
}

impl TableColumnSizingChange {
    /// Creates a committed resize payload.
    pub fn new(
        column_id: impl Into<TableColumnId>,
        width: UiPx,
        sizing: TableColumnSizing,
    ) -> Self {
        Self {
            column_id: column_id.into(),
            width,
            sizing,
        }
    }

    /// Returns the resized column id.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the resolved column width for the resized column.
    pub const fn width(&self) -> UiPx {
        self.width
    }

    /// Returns the next committed sizing map.
    pub const fn sizing(&self) -> &TableColumnSizing {
        &self.sizing
    }
}

/// Renderer-neutral modifier-key snapshot carried by table row callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TableInputModifiers {
    control: bool,
    alt: bool,
    shift: bool,
    platform: bool,
    function: bool,
}

impl TableInputModifiers {
    pub(super) fn from_gpui(modifiers: Modifiers) -> Self {
        Self {
            control: modifiers.control,
            alt: modifiers.alt,
            shift: modifiers.shift,
            platform: modifiers.platform,
            function: modifiers.function,
        }
    }

    /// Returns whether the control key was pressed.
    pub const fn control(self) -> bool {
        self.control
    }

    /// Returns whether the alt key was pressed.
    pub const fn alt(self) -> bool {
        self.alt
    }

    /// Returns whether the shift key was pressed.
    pub const fn shift(self) -> bool {
        self.shift
    }

    /// Returns whether the platform command key was pressed.
    pub const fn platform(self) -> bool {
        self.platform
    }

    /// Returns whether the function key was pressed.
    pub const fn function(self) -> bool {
        self.function
    }

    /// Returns whether any modifier key was pressed.
    pub const fn modified(self) -> bool {
        self.control || self.alt || self.shift || self.platform || self.function
    }
}

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
    row_id: TableRowId,
    render_key: String,
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
    pub(super) fn from_render_plan(
        row: &TableRowRenderPlan,
        modifiers: TableInputModifiers,
    ) -> Self {
        Self {
            row_id: row.id().clone(),
            render_key: row.render_key().to_owned(),
            model_index: row.model_index(),
            source_index: row.row().source_index(),
            depth: row.row().depth(),
            selected: row.selected(),
            tree_branch: row.row().is_tree_branch(),
            tree_expanded: row.row().tree_expanded(),
            loaded_child_count: row.row().loaded_child_count(),
            children_load_state: row.row().children_load_state().cloned(),
            modifiers,
        }
    }

    pub(super) fn for_row(row_id: TableRowId) -> Self {
        Self {
            row_id,
            render_key: String::new(),
            model_index: 0,
            source_index: None,
            depth: 0,
            selected: false,
            tree_branch: false,
            tree_expanded: None,
            loaded_child_count: 0,
            children_load_state: None,
            modifiers: TableInputModifiers::default(),
        }
    }

    /// Returns the stable row id.
    pub const fn row_id(&self) -> &TableRowId {
        &self.row_id
    }

    /// Returns the unique render key used by element ids.
    pub fn render_key(&self) -> &str {
        &self.render_key
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
    pub(super) fn new(action: TableRowAction, kind: TableRowActivationKind) -> Self {
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

    /// Returns the activated row id.
    pub const fn row_id(&self) -> &TableRowId {
        self.action.row_id()
    }
}

/// Controlled payload emitted when a table row expansion toggle is requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRowExpansionToggle {
    action: TableRowAction,
    expanded: bool,
}

impl TableRowExpansionToggle {
    pub(super) fn new(action: TableRowAction, expanded: bool) -> Self {
        Self { action, expanded }
    }

    /// Returns common row metadata.
    pub const fn action(&self) -> &TableRowAction {
        &self.action
    }

    /// Returns the row id whose expansion should change.
    pub const fn row_id(&self) -> &TableRowId {
        self.action.row_id()
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

    /// Returns the row id whose selection changed.
    pub const fn row_id(&self) -> &TableRowId {
        self.action.row_id()
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

pub(super) fn request_table_row_selection_change(
    runtime: &Entity<TableRuntime>,
    action: &TableRowAction,
    selection_policy: TableSelectionPolicy,
    scope: TableSelectionScope,
    selected_row_ids: Rc<Vec<TableRowId>>,
    on_row_selection_change: Option<TableRowSelectionHandler>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
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
            vec![action.row_id().clone()]
        } else {
            let mut next_selection_ids = selected_row_ids.as_ref().clone();
            next_selection_ids.push(action.row_id().clone());
            next_selection_ids
        }
    } else {
        selected_row_ids
            .iter()
            .filter(|row_id| *row_id != action.row_id())
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

    runtime.update(cx, |runtime, cx| {
        runtime.set_selection_anchor(Some(action.row_id().clone()), cx);
    });

    if let Some(on_row_selection_change) = on_row_selection_change.as_ref() {
        on_row_selection_change(change, window, cx);
        return true;
    }

    false
}

pub(super) fn toggle_table_expansion(
    expansion: TableExpansionState,
    row_id: TableRowId,
    expanded: bool,
) -> TableExpansionState {
    match expansion {
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
    }
}

fn effective_table_column_order(state: &TableState) -> Vec<TableColumnId> {
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

fn reorder_table_column_order(
    mut column_order: Vec<TableColumnId>,
    column_id: TableColumnId,
    target_column_id: TableColumnId,
    placement: TableColumnOrderPlacement,
) -> Option<Vec<TableColumnId>> {
    if column_id == target_column_id {
        return None;
    }

    let source_index = column_order.iter().position(|id| id == &column_id)?;
    let _ = column_order.remove(source_index);
    let target_index = column_order.iter().position(|id| id == &target_column_id)?;
    let insert_index = match placement {
        TableColumnOrderPlacement::Before => target_index,
        TableColumnOrderPlacement::After => target_index + 1,
    };
    column_order.insert(insert_index, column_id);

    Some(column_order)
}
