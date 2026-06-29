use crate::popover::PopoverState;
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayPlacementAlignment,
    OverlayPlacementSide, Size, TableColumn, TableColumnId, TableColumnVisibilityOverrides,
    TableState, ThemeTokens,
};

/// Kind of table column-visibility change emitted by the visibility recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableColumnVisibilityAction {
    /// One column was toggled to a specific visibility.
    ToggleColumn,
    /// All hideable columns should be made visible.
    ShowAll,
    /// Runtime overrides should reset to descriptor defaults.
    Reset,
}

impl TableColumnVisibilityAction {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ToggleColumn => "toggle_column",
            Self::ShowAll => "show_all",
            Self::Reset => "reset",
        }
    }
}

/// Controlled payload emitted when table column visibility changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumnVisibilityChange {
    action: TableColumnVisibilityAction,
    column_ids: Vec<TableColumnId>,
    next_visible: Option<bool>,
}

impl TableColumnVisibilityChange {
    /// Creates a single-column visibility toggle payload.
    pub fn new(column_id: impl Into<TableColumnId>, next_visible: bool) -> Self {
        Self {
            action: TableColumnVisibilityAction::ToggleColumn,
            column_ids: vec![column_id.into()],
            next_visible: Some(next_visible),
        }
    }

    /// Creates a payload that shows the supplied hideable columns.
    pub fn show_all(column_ids: impl IntoIterator<Item = impl Into<TableColumnId>>) -> Self {
        Self {
            action: TableColumnVisibilityAction::ShowAll,
            column_ids: column_ids.into_iter().map(Into::into).collect(),
            next_visible: Some(true),
        }
    }

    /// Creates a payload that clears runtime visibility overrides.
    pub fn reset() -> Self {
        Self {
            action: TableColumnVisibilityAction::Reset,
            column_ids: Vec::new(),
            next_visible: None,
        }
    }

    /// Returns the change kind.
    pub const fn action(&self) -> TableColumnVisibilityAction {
        self.action
    }

    /// Returns affected column ids.
    pub fn column_ids(&self) -> &[TableColumnId] {
        &self.column_ids
    }

    /// Returns the affected column id for single-column changes.
    pub fn column_id(&self) -> Option<&TableColumnId> {
        (self.column_ids.len() == 1).then(|| &self.column_ids[0])
    }

    /// Returns the next visibility for set/show-all changes.
    pub const fn next_visible(&self) -> Option<bool> {
        self.next_visible
    }

    /// Applies this visibility change while preserving unrelated table state.
    pub fn apply_to(&self, state: TableState) -> TableState {
        let visibility = match self.action {
            TableColumnVisibilityAction::Reset => state.column_visibility().clone().clear(),
            TableColumnVisibilityAction::ToggleColumn | TableColumnVisibilityAction::ShowAll => {
                let Some(next_visible) = self.next_visible else {
                    return state;
                };
                self.column_ids.iter().cloned().fold(
                    state.column_visibility().clone(),
                    |visibility, column_id| visibility.with_visibility(column_id, next_visible),
                )
            }
        };

        state.with_column_visibility(visibility)
    }
}

/// One column row in a table column-visibility recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumnVisibilityItemState {
    column_id: TableColumnId,
    label: String,
    checked: bool,
    hideable: bool,
}

impl TableColumnVisibilityItemState {
    pub(in crate::table::column_visibility) fn new(
        column: &TableColumn,
        visibility: &TableColumnVisibilityOverrides,
    ) -> Self {
        Self {
            column_id: column.id().clone(),
            label: column.label().to_owned(),
            checked: visibility.is_visible(column),
            hideable: column.hideable(),
        }
    }

    /// Returns the stable column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the visible column label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether this column is effectively visible.
    pub const fn checked(&self) -> bool {
        self.checked
    }

    /// Returns whether user-facing controls may hide this column.
    pub const fn hideable(&self) -> bool {
        self.hideable
    }

    /// Returns whether this row should be disabled in visibility controls.
    pub const fn disabled(&self) -> bool {
        !self.hideable
    }
}

/// Resolved renderer-neutral state for a table column-visibility recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnVisibilityState {
    id: String,
    label: String,
    trigger_label: String,
    items: Vec<TableColumnVisibilityItemState>,
    visible_count: usize,
    hidden_count: usize,
    hideable_count: usize,
    all_visible: bool,
    some_visible: bool,
    show_all_enabled: bool,
    reset_enabled: bool,
    empty_label: String,
    show_all_label: String,
    reset_label: String,
    popover: PopoverState,
}

impl TableColumnVisibilityState {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::table::column_visibility) fn resolve(
        id: impl Into<String>,
        label: impl Into<String>,
        columns: &[TableColumn],
        visibility: &TableColumnVisibilityOverrides,
        empty_label: impl Into<String>,
        show_all_label: impl Into<String>,
        reset_label: impl Into<String>,
        size: Size,
        disabled: bool,
        open: Option<bool>,
        default_open: bool,
        placement_side: OverlayPlacementSide,
        placement_alignment: OverlayPlacementAlignment,
        outside_press_policy: OutsidePressPolicy,
        initial_focus_intent: InitialFocusIntent,
        focus_restore_intent: FocusRestoreIntent,
        tokens: ThemeTokens,
    ) -> Self {
        let label = label.into();
        let items = columns
            .iter()
            .map(|column| TableColumnVisibilityItemState::new(column, visibility))
            .collect::<Vec<_>>();
        let visible_count = items.iter().filter(|item| item.checked()).count();
        let hidden_count = items.len().saturating_sub(visible_count);
        let hideable_count = items.iter().filter(|item| item.hideable()).count();
        let all_visible = hidden_count == 0;
        let some_visible = visible_count > 0 && hidden_count > 0;
        let show_all_enabled = items.iter().any(|item| item.hideable() && !item.checked());
        let trigger_label = table_column_visibility_trigger_label(&label, hidden_count);
        let popover = PopoverState::resolve(
            size,
            disabled,
            open,
            default_open,
            placement_side,
            placement_alignment,
            outside_press_policy,
            initial_focus_intent,
            focus_restore_intent,
            tokens,
        );

        Self {
            id: id.into(),
            label,
            trigger_label,
            items,
            visible_count,
            hidden_count,
            hideable_count,
            all_visible,
            some_visible,
            show_all_enabled,
            reset_enabled: !visibility.is_empty(),
            empty_label: empty_label.into(),
            show_all_label: show_all_label.into(),
            reset_label: reset_label.into(),
            popover,
        }
    }

    /// Returns stable recipe id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the visible recipe label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the trigger label including hidden-column count.
    pub fn trigger_label(&self) -> &str {
        &self.trigger_label
    }

    /// Returns item metadata for every supplied column.
    pub fn items(&self) -> &[TableColumnVisibilityItemState] {
        &self.items
    }

    /// Returns number of column items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns whether no column items are available.
    pub fn empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns number of effectively visible columns.
    pub const fn visible_count(&self) -> usize {
        self.visible_count
    }

    /// Returns number of effectively hidden columns.
    pub const fn hidden_count(&self) -> usize {
        self.hidden_count
    }

    /// Returns number of columns that can be hidden by user-facing controls.
    pub const fn hideable_count(&self) -> usize {
        self.hideable_count
    }

    /// Returns true when every supplied column is visible.
    pub const fn all_visible(&self) -> bool {
        self.all_visible
    }

    /// Returns true when at least one, but not all, supplied columns are visible.
    pub const fn some_visible(&self) -> bool {
        self.some_visible
    }

    /// Returns whether the show-all action should be enabled.
    pub const fn show_all_enabled(&self) -> bool {
        self.show_all_enabled
    }

    /// Returns whether the reset action should be enabled.
    pub const fn reset_enabled(&self) -> bool {
        self.reset_enabled
    }

    /// Returns the empty-state label.
    pub fn empty_label(&self) -> &str {
        &self.empty_label
    }

    /// Returns the show-all action label.
    pub fn show_all_label(&self) -> &str {
        &self.show_all_label
    }

    /// Returns the reset action label.
    pub fn reset_label(&self) -> &str {
        &self.reset_label
    }

    /// Returns resolved popover state.
    pub const fn popover(&self) -> &PopoverState {
        &self.popover
    }
}

fn table_column_visibility_trigger_label(label: &str, hidden_count: usize) -> String {
    if hidden_count == 0 {
        label.to_owned()
    } else {
        format!("{label}: {hidden_count} hidden")
    }
}
