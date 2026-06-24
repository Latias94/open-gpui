//! Table component backed by renderer-neutral row-model and virtualizer contracts.

use crate::a11y::UiA11yElementExt;
use crate::button::{Button, ButtonVariant};
use crate::checkbox::Checkbox;
use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use crate::popover::{Popover, PopoverState};
use crate::scroll_area::ScrollArea;
use crate::text_input::{TextInput, TextInputState};
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, Context, CursorStyle, DragMoveEvent, Empty, Entity, FocusHandle,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, Modifiers, MouseButton,
    ParentElement, RenderOnce, ScrollHandle, ScrollWheelEvent, SharedString,
    StatefulInteractiveElement, Styled, Window, div, point, px, rgb, rgba,
};
use open_gpui_ui_core::{
    FocusRestoreIntent, GridViewport2D, InitialFocusIntent, OutsidePressPolicy,
    OverlayPlacementAlignment, OverlayPlacementSide, Role, Sizable, Size, TableCellValue,
    TableColumn, TableColumnFacets, TableColumnId, TableColumnRegion, TableColumnResizeDirection,
    TableColumnResizeMode, TableColumnResizeState, TableColumnSizing, TableExpansionMode,
    TableExpansionState, TableFilter, TableResolvedColumnSizing, TableResolvedRow,
    TableResolvedState, TableRowChildrenLoadState, TableRowId, TableRowRegion, TableSelectionMode,
    TableSelectionPolicy, TableSelectionSummary, TableSort, TableSortDirection, TableStageMode,
    TableState, TableStateCacheKey, TableTreeRow, ThemeTokens, Toggled, UiPx, VirtualizerItemKey,
    VirtualizerItemMeasurement, VirtualizerRange, VirtualizerResolvedState, VirtualizerSnapshot,
    VirtualizerState, drag_table_column_resize, end_table_column_resize, ui_px,
};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

type TableSortHandler = Rc<dyn Fn(TableHeaderAction, &mut Window, &mut App)>;
type TableColumnSizingHandler = Rc<dyn Fn(TableColumnSizingChange, &mut Window, &mut App)>;
type TableRowActivationHandler = Rc<dyn Fn(TableRowActivation, &mut Window, &mut App)>;
type TableRowExpansionHandler = Rc<dyn Fn(TableRowExpansionToggle, &mut Window, &mut App)>;
type TableRowSelectionHandler = Rc<dyn Fn(TableRowSelectionChange, &mut Window, &mut App)>;
type TableFacetedFilterChangeHandler = Rc<dyn Fn(TableFacetedFilterChange, &mut Window, &mut App)>;
type TableCellEditHandler = Rc<dyn Fn(TableCellEditChange, &mut Window, &mut App)>;

/// One visible option in a table faceted filter recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableFacetedFilterOptionState {
    value: String,
    label: String,
    count: usize,
    selected: bool,
}

impl TableFacetedFilterOptionState {
    fn new(value: String, label: String, count: usize, selected: bool) -> Self {
        Self {
            value,
            label,
            count,
            selected,
        }
    }

    /// Returns the exact categorical token used by [`TableFilter::one_of`].
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible option label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the number of rows represented by this facet value.
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns whether this option is currently selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }
}

/// Controlled payload emitted when a table faceted filter selection changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableFacetedFilterChange {
    column_id: TableColumnId,
    selected_values: Vec<String>,
    toggled_value: Option<String>,
    selected: bool,
}

impl TableFacetedFilterChange {
    /// Creates a selection-change payload for one faceted table column.
    pub fn new(
        column_id: impl Into<TableColumnId>,
        selected_values: impl IntoIterator<Item = impl Into<String>>,
        toggled_value: Option<impl Into<String>>,
        selected: bool,
    ) -> Self {
        Self {
            column_id: column_id.into(),
            selected_values: selected_values
                .into_iter()
                .map(Into::into)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            toggled_value: toggled_value.map(Into::into),
            selected,
        }
    }

    /// Creates a payload that clears this column's categorical filter.
    pub fn clear(column_id: impl Into<TableColumnId>) -> Self {
        Self::new(
            column_id,
            std::iter::empty::<String>(),
            None::<String>,
            false,
        )
    }

    /// Returns the faceted column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns selected exact categorical tokens after the change.
    pub fn selected_values(&self) -> &[String] {
        &self.selected_values
    }

    /// Returns the option token that was toggled, or `None` for clear-all changes.
    pub fn toggled_value(&self) -> Option<&str> {
        self.toggled_value.as_deref()
    }

    /// Returns whether the toggled token is selected after the change.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns true when this change clears the column filter.
    pub const fn cleared(&self) -> bool {
        self.toggled_value.is_none() && !self.selected
    }

    /// Returns the next column-filter list while preserving unrelated filters.
    pub fn next_filters(&self, filters: impl IntoIterator<Item = TableFilter>) -> Vec<TableFilter> {
        table_faceted_filter_next_filters(filters, &self.column_id, &self.selected_values)
    }

    /// Applies this filter change to a table state and resets pagination to the first page.
    pub fn apply_to(&self, state: TableState) -> TableState {
        let next_filters = self.next_filters(state.filters().iter().cloned());
        let next_pagination = state.pagination().with_page_index(0);

        state
            .with_filters(next_filters)
            .with_pagination(next_pagination)
    }
}

/// Outcome from applying a controlled table cell edit to app-owned table state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableCellEditApplyOutcome {
    /// The matching source row and cell were updated.
    Updated,
    /// No source row matched the edit payload row id.
    RowNotFound,
    /// The source row exists, but the edited column does not exist on that row.
    CellNotFound,
}

impl TableCellEditApplyOutcome {
    /// Returns true when the state was updated.
    pub const fn updated(self) -> bool {
        matches!(self, Self::Updated)
    }

    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Updated => "updated",
            Self::RowNotFound => "row-not-found",
            Self::CellNotFound => "cell-not-found",
        }
    }
}

/// Controlled payload emitted when an editable table text cell changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCellEditChange {
    action: TableRowAction,
    column_id: TableColumnId,
    previous_text: String,
    next_text: String,
}

impl TableCellEditChange {
    fn new(
        action: TableRowAction,
        column_id: impl Into<TableColumnId>,
        previous_text: impl Into<String>,
        next_text: impl Into<String>,
    ) -> Self {
        Self {
            action,
            column_id: column_id.into(),
            previous_text: previous_text.into(),
            next_text: next_text.into(),
        }
    }

    /// Creates a text-cell edit payload from stable row and column ids.
    pub fn for_row(
        row_id: impl Into<TableRowId>,
        column_id: impl Into<TableColumnId>,
        previous_text: impl Into<String>,
        next_text: impl Into<String>,
    ) -> Self {
        let row_id = row_id.into();
        Self {
            action: TableRowAction {
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
            },
            column_id: column_id.into(),
            previous_text: previous_text.into(),
            next_text: next_text.into(),
        }
    }

    /// Returns common row metadata for the edited cell.
    pub const fn action(&self) -> &TableRowAction {
        &self.action
    }

    /// Returns the stable edited row id.
    pub const fn row_id(&self) -> &TableRowId {
        self.action.row_id()
    }

    /// Returns the unique render key used by the edited row element.
    pub fn render_key(&self) -> &str {
        self.action.render_key()
    }

    /// Returns this row's zero-based index in the final row model.
    pub const fn model_index(&self) -> usize {
        self.action.model_index()
    }

    /// Returns the source-row preorder index, when this is a source row.
    pub const fn source_index(&self) -> Option<usize> {
        self.action.source_index()
    }

    /// Returns the stable edited column id.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the resolved text before the edit.
    pub fn previous_text(&self) -> &str {
        &self.previous_text
    }

    /// Returns the next controlled text value.
    pub fn next_text(&self) -> &str {
        &self.next_text
    }

    /// Applies this edit to a table state and returns an inspectable outcome.
    pub fn apply_to(&self, state: TableState) -> (TableState, TableCellEditApplyOutcome) {
        let mut outcome = TableCellEditApplyOutcome::RowNotFound;
        let rows = state
            .rows()
            .iter()
            .cloned()
            .map(|row| {
                apply_table_cell_edit_to_row(
                    row,
                    self.row_id(),
                    &self.column_id,
                    &self.next_text,
                    &mut outcome,
                )
            })
            .collect::<Vec<_>>();

        if outcome.updated() {
            (state.with_rows(rows), outcome)
        } else {
            (state, outcome)
        }
    }
}

fn apply_table_cell_edit_to_row(
    mut row: open_gpui_ui_core::TableRow,
    row_id: &TableRowId,
    column_id: &TableColumnId,
    next_text: &str,
    outcome: &mut TableCellEditApplyOutcome,
) -> open_gpui_ui_core::TableRow {
    if row.id() == row_id {
        *outcome = if row.cell(column_id).is_some() {
            TableCellEditApplyOutcome::Updated
        } else {
            TableCellEditApplyOutcome::CellNotFound
        };

        if outcome.updated() {
            return row.with_cell(column_id.clone(), next_text.to_owned());
        }
        return row;
    }

    let children = row
        .children()
        .iter()
        .cloned()
        .map(|child| apply_table_cell_edit_to_row(child, row_id, column_id, next_text, outcome))
        .collect::<Vec<_>>();
    row = row.with_replaced_children(children);
    row
}

/// Resolved renderer-neutral state for a table faceted filter recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct TableFacetedFilterState {
    id: String,
    label: String,
    column_id: TableColumnId,
    query: String,
    trigger_label: String,
    selected_values: Vec<String>,
    selected_labels: Vec<String>,
    options: Vec<TableFacetedFilterOptionState>,
    total_option_count: usize,
    empty_label: String,
    clear_label: String,
    popover: PopoverState,
    search_input: TextInputState,
}

impl TableFacetedFilterState {
    #[allow(clippy::too_many_arguments)]
    fn resolve(
        id: impl Into<String>,
        label: impl Into<String>,
        column_id: TableColumnId,
        facets: Option<&TableColumnFacets>,
        selected_values: &BTreeSet<String>,
        query: impl Into<String>,
        placeholder: impl Into<String>,
        empty_label: impl Into<String>,
        clear_label: impl Into<String>,
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
        let id = id.into();
        let label = label.into();
        let query = query.into();
        let placeholder = placeholder.into();
        let empty_label = empty_label.into();
        let clear_label = clear_label.into();
        let all_options = facets
            .map(|facets| {
                facets
                    .unique_values()
                    .iter()
                    .map(|entry| {
                        let value = entry.value().filter_text();
                        let label = table_facet_value_label(entry.value());
                        TableFacetedFilterOptionState::new(
                            value.clone(),
                            label,
                            entry.count(),
                            selected_values.contains(&value),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let normalized_query = normalize_table_faceted_query(&query);
        let options = all_options
            .iter()
            .filter(|option| table_faceted_option_matches(option, &normalized_query))
            .cloned()
            .collect::<Vec<_>>();
        let selected_labels = table_faceted_selected_labels(&all_options, selected_values);
        let trigger_label = table_faceted_trigger_label(&label, &selected_labels);
        let selected_values = selected_values.iter().cloned().collect::<Vec<_>>();
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
        let search_input = TextInputState::resolve(
            query.clone(),
            Some(placeholder),
            size,
            disabled,
            false,
            false,
            false,
            true,
            tokens,
        );

        Self {
            id,
            label,
            column_id,
            query,
            trigger_label,
            selected_values,
            selected_labels,
            total_option_count: all_options.len(),
            options,
            empty_label,
            clear_label,
            popover,
            search_input,
        }
    }

    /// Returns stable recipe id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the visible filter label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the filtered column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns current search query text.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the trigger label including selected values or selected count.
    pub fn trigger_label(&self) -> &str {
        &self.trigger_label
    }

    /// Returns selected exact categorical tokens.
    pub fn selected_values(&self) -> &[String] {
        &self.selected_values
    }

    /// Returns selected labels in facet display order.
    pub fn selected_labels(&self) -> &[String] {
        &self.selected_labels
    }

    /// Returns currently visible options after applying the search query.
    pub fn options(&self) -> &[TableFacetedFilterOptionState] {
        &self.options
    }

    /// Returns number of options before search filtering.
    pub const fn total_option_count(&self) -> usize {
        self.total_option_count
    }

    /// Returns whether no options are visible for the current query.
    pub fn empty(&self) -> bool {
        self.options.is_empty()
    }

    /// Returns whether clear-all should be enabled.
    pub fn clear_enabled(&self) -> bool {
        !self.selected_values.is_empty()
    }

    /// Returns the empty-state label.
    pub fn empty_label(&self) -> &str {
        &self.empty_label
    }

    /// Returns the clear-all label.
    pub fn clear_label(&self) -> &str {
        &self.clear_label
    }

    /// Returns resolved popover state.
    pub const fn popover(&self) -> &PopoverState {
        &self.popover
    }

    /// Returns resolved search input state.
    pub const fn search_input(&self) -> &TextInputState {
        &self.search_input
    }
}

#[derive(Debug, Clone)]
struct TableFacetedFilterRuntime {
    query: String,
}

/// A Popover + search + checkbox recipe for one categorical table column.
#[derive(IntoElement)]
pub struct TableFacetedFilter {
    id: String,
    label: SharedString,
    column_id: TableColumnId,
    facets: Option<TableColumnFacets>,
    selected_values: BTreeSet<String>,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    query: Option<String>,
    default_query: String,
    placeholder: SharedString,
    empty_label: SharedString,
    clear_label: SharedString,
    viewport_item_count: usize,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_query_change: Option<Rc<dyn Fn(String, &mut Window, &mut App)>>,
    on_change: Option<TableFacetedFilterChangeHandler>,
}

impl TableFacetedFilter {
    /// Creates a faceted filter recipe for one table column.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        column_id: impl Into<TableColumnId>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            column_id: column_id.into(),
            facets: None,
            selected_values: BTreeSet::new(),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            query: None,
            default_query: String::new(),
            placeholder: "Search values".into(),
            empty_label: "No values".into(),
            clear_label: "Clear filters".into(),
            viewport_item_count: 8,
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Start,
            outside_press_policy: OutsidePressPolicy::DismissAndPassThrough,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_open_change: None,
            on_query_change: None,
            on_change: None,
        }
    }

    /// Applies resolved facet metadata for this column.
    pub fn facets(mut self, facets: TableColumnFacets) -> Self {
        self.facets = Some(facets);
        self
    }

    /// Applies current selected exact categorical tokens.
    pub fn selected_values(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.selected_values = values.into_iter().map(Into::into).collect();
        self
    }

    /// Applies controlled popover open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Applies uncontrolled initial popover open state.
    pub fn default_open(mut self, default_open: bool) -> Self {
        self.default_open = default_open;
        self
    }

    /// Applies controlled search query text.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Applies the default query for adapter-owned search input state.
    pub fn default_query(mut self, query: impl Into<String>) -> Self {
        self.default_query = query.into();
        self
    }

    /// Applies search input placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Applies the empty-state label.
    pub fn empty_label(mut self, label: impl Into<SharedString>) -> Self {
        self.empty_label = label.into();
        self
    }

    /// Applies the clear-all button label.
    pub fn clear_label(mut self, label: impl Into<SharedString>) -> Self {
        self.clear_label = label.into();
        self
    }

    /// Marks the filter trigger and content controls as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies the estimated number of option rows visible in the popup.
    pub fn viewport_item_count(mut self, count: usize) -> Self {
        self.viewport_item_count = count.max(1);
        self
    }

    /// Applies preferred popover placement side.
    pub fn placement_side(mut self, side: OverlayPlacementSide) -> Self {
        self.placement_side = side;
        self
    }

    /// Applies preferred popover placement alignment.
    pub fn placement_alignment(mut self, alignment: OverlayPlacementAlignment) -> Self {
        self.placement_alignment = alignment;
        self
    }

    /// Applies outside-press behavior.
    pub fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = policy;
        self
    }

    /// Applies initial focus behavior when the popup opens.
    pub fn initial_focus_intent(mut self, intent: InitialFocusIntent) -> Self {
        self.initial_focus_intent = intent;
        self
    }

    /// Applies focus restoration behavior when the popup closes.
    pub fn focus_restore_intent(mut self, intent: FocusRestoreIntent) -> Self {
        self.focus_restore_intent = intent;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers an open-change handler.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Registers a search query-change handler.
    pub fn on_query_change(
        mut self,
        handler: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_query_change = Some(Rc::new(handler));
        self
    }

    /// Registers a selection-change handler.
    pub fn on_change(
        mut self,
        handler: impl Fn(TableFacetedFilterChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved recipe state.
    pub fn state(&self) -> TableFacetedFilterState {
        let query = self.query.as_deref().unwrap_or(self.default_query.as_str());
        TableFacetedFilterState::resolve(
            self.id.clone(),
            self.label.to_string(),
            self.column_id.clone(),
            self.facets.as_ref(),
            &self.selected_values,
            query,
            self.placeholder.to_string(),
            self.empty_label.to_string(),
            self.clear_label.to_string(),
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        )
    }
}

impl Sizable for TableFacetedFilter {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for TableFacetedFilter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime_id = format!("{}-runtime", self.id);
        let runtime = window.use_keyed_state(runtime_id, cx, |_, _| TableFacetedFilterRuntime {
            query: self.default_query.clone(),
        });
        let controlled_query = self.query.clone();
        let runtime_query = runtime.read(cx).query.clone();
        let query = controlled_query.clone().unwrap_or(runtime_query);

        if controlled_query.is_some() && runtime.read(cx).query != query {
            runtime.update(cx, |runtime, _| {
                runtime.query = query.clone();
            });
        }

        let state = TableFacetedFilterState::resolve(
            self.id.clone(),
            self.label.clone(),
            self.column_id.clone(),
            self.facets.as_ref(),
            &self.selected_values,
            query.clone(),
            self.placeholder.clone(),
            self.empty_label.clone(),
            self.clear_label.clone(),
            self.size,
            self.disabled,
            self.open,
            self.default_open,
            self.placement_side,
            self.placement_alignment,
            self.outside_press_policy,
            self.initial_focus_intent.clone(),
            self.focus_restore_intent.clone(),
            self.tokens,
        );
        let on_open_change = self.on_open_change.clone();
        let on_query_change = self.on_query_change.clone();
        let on_change = self.on_change.clone();
        let column_id = self.column_id.clone();
        let selected_values = self.selected_values.clone();
        let query_runtime = runtime.clone();
        let search_id = format!("{}-search", self.id);
        let options_id = format!("{}-options", self.id);
        let content_id = format!("{}-content", self.id);
        let clear_id = format!("{}-clear", self.id);
        let options_height = self.size.list_row_h() * self.viewport_item_count as f32;
        let summary_text = if state.selected_labels().is_empty() {
            state.label().to_owned()
        } else {
            state.trigger_label().to_owned()
        };
        let content = table_faceted_filter_content_element(
            content_id,
            search_id,
            options_id,
            clear_id,
            state,
            query_runtime,
            on_query_change,
            on_change,
            column_id,
            selected_values,
            options_height,
            self.size,
            self.tokens,
        );

        let mut popover = Popover::element(self.id.clone(), summary_text, content)
            .default_open(self.default_open)
            .disabled(self.disabled)
            .placement_side(self.placement_side)
            .placement_alignment(self.placement_alignment)
            .outside_press_policy(self.outside_press_policy)
            .initial_focus_intent(self.initial_focus_intent)
            .focus_restore_intent(self.focus_restore_intent)
            .tokens(self.tokens);

        if let Some(open) = self.open {
            popover = popover.open(open);
        }

        if let Some(on_open_change) = on_open_change {
            popover = popover.on_open_change(move |open, window, cx| {
                on_open_change(open, window, cx);
            });
        }

        popover
    }
}

fn table_faceted_filter_content_element(
    content_id: String,
    search_id: String,
    options_id: String,
    clear_id: String,
    state: TableFacetedFilterState,
    query_runtime: Entity<TableFacetedFilterRuntime>,
    on_query_change: Option<Rc<dyn Fn(String, &mut Window, &mut App)>>,
    on_change: Option<TableFacetedFilterChangeHandler>,
    column_id: TableColumnId,
    selected_values: BTreeSet<String>,
    options_height: UiPx,
    size: Size,
    tokens: ThemeTokens,
) -> impl IntoElement {
    let disabled = state.popover().disabled();
    let query = state.query().to_owned();
    let options = state.options().to_vec();
    let clear_enabled = state.clear_enabled();
    let clear_label = state.clear_label().to_owned();
    let placeholder = state
        .search_input()
        .placeholder()
        .unwrap_or("Search values")
        .to_owned();
    let selected_summary = if state.selected_labels().is_empty() {
        None
    } else {
        Some(state.selected_labels().join(", "))
    };
    let empty_label = state.empty_label().to_owned();
    let popup_query = query.clone();
    let query_runtime_for_input = query_runtime.clone();
    let query_on_change = on_query_change.clone();
    let state_for_query = state.clone();
    let column_id_for_toggle = column_id.clone();
    let on_change_for_clear = on_change.clone();
    let content_debug_id = state.id().to_owned();

    div()
        .id(content_id)
        .debug_selector(move || format!("table-faceted-filter:{content_debug_id}:content"))
        .min_w(px(0.0))
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .text_color(ThemeResolver::resolve(
            state.popover().colors().foreground(),
        ))
        .on_scroll_wheel(|_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .min_w(px(0.0))
                        .flex_1()
                        .truncate()
                        .child(state.trigger_label().to_owned()),
                )
                .when_some(selected_summary, |this, summary| {
                    this.child(div().flex_none().text_xs().opacity(0.72).child(summary))
                }),
        )
        .child(
            TextInput::new(search_id, state.label().to_owned())
                .with_size(size)
                .value(popup_query)
                .placeholder(placeholder)
                .disabled(disabled)
                .tokens(tokens)
                .on_change(move |next_query, window, cx| {
                    query_runtime_for_input.update(cx, |runtime, _| {
                        runtime.query = next_query.clone();
                    });
                    if let Some(on_query_change) = query_on_change.as_ref() {
                        on_query_change(next_query, window, cx);
                    }
                }),
        )
        .when(clear_enabled, |this| {
            this.child(
                div().flex().justify_end().child(
                    Button::new(clear_id, clear_label)
                        .variant(ButtonVariant::Ghost)
                        .with_size(size)
                        .disabled(disabled)
                        .on_click(move |_, window, cx| {
                            if let Some(on_change) = on_change_for_clear.as_ref() {
                                let change =
                                    TableFacetedFilterChange::clear(column_id_for_toggle.clone());
                                on_change(change, window, cx);
                            }
                        }),
                ),
            )
        })
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .h(gpui_px_from_ui(options_height))
                .overflow_hidden()
                .child(
                    ScrollArea::new(
                        options_id,
                        table_faceted_filter_options_element(
                            state_for_query,
                            options,
                            query_runtime,
                            on_change,
                            column_id,
                            selected_values,
                            empty_label,
                            disabled,
                        ),
                    )
                    .vertical()
                    .reset_on_key(query.clone())
                    .with_size(size),
                ),
        )
}

fn table_faceted_filter_options_element(
    state: TableFacetedFilterState,
    options: Vec<TableFacetedFilterOptionState>,
    query_runtime: Entity<TableFacetedFilterRuntime>,
    on_change: Option<TableFacetedFilterChangeHandler>,
    column_id: TableColumnId,
    selected_values: BTreeSet<String>,
    empty_label: String,
    disabled: bool,
) -> impl IntoElement {
    if options.is_empty() {
        return div()
            .min_w(px(0.0))
            .py(px(4.0))
            .text_sm()
            .opacity(0.72)
            .child(empty_label)
            .into_any_element();
    }

    let query = state.query().to_owned();

    options
        .into_iter()
        .fold(
            div().flex().flex_col().gap_1().min_w(px(0.0)),
            |list, option| {
                let option_value = option.value().to_owned();
                let option_label = option.label().to_owned();
                let option_count = option.count();
                let option_checked = option.selected();
                let option_selected_values = selected_values.clone();
                let on_change = on_change.clone();
                let column_id = column_id.clone();
                let query_runtime_for_toggle = query_runtime.clone();
                let query_for_toggle = query.clone();
                let option_id = format!("{}-option-{option_value}", state.id());
                let row_id = format!("{}-option-row-{option_value}", state.id());
                let option_debug_id = state.id().to_owned();
                let option_debug_value = option_value.clone();
                let row_selected_values = selected_values.clone();
                let row_on_change = on_change.clone();
                let row_column_id = column_id.clone();
                let row_query_runtime = query_runtime.clone();
                let row_query = query.clone();
                let row_option_value = option_value.clone();

                list.child(
                    div()
                        .id(row_id)
                        .debug_selector(move || {
                            format!(
                                "table-faceted-filter:{option_debug_id}:option:{option_debug_value}"
                            )
                        })
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .rounded(px(6.0))
                        .px(px(6.0))
                        .py(px(4.0))
                        .when(disabled, |this| this.opacity(0.56))
                        .when(!disabled, move |this| {
                            this.cursor_pointer()
                                .hover(|style| style.bg(rgba(0x00000010)))
                                .on_click(move |_, window, cx| {
                                    let mut next_values = row_selected_values.clone();
                                    let next_selected = !option_checked;
                                    if next_selected {
                                        next_values.insert(row_option_value.clone());
                                    } else {
                                        next_values.remove(&row_option_value);
                                    }
                                    row_query_runtime.update(cx, |runtime, _| {
                                        runtime.query = row_query.clone();
                                    });
                                    if let Some(on_change) = row_on_change.as_ref() {
                                        let change = TableFacetedFilterChange::new(
                                            row_column_id.clone(),
                                            next_values.into_iter(),
                                            Some(row_option_value.clone()),
                                            next_selected,
                                        );
                                        on_change(change, window, cx);
                                    }
                                })
                        })
                        .child(
                            Checkbox::new(option_id)
                                .label(option_label.clone())
                                .checked(option_checked)
                                .disabled(disabled)
                                .on_toggle(move |toggled, _event, window, cx| {
                                    let mut next_values = option_selected_values.clone();
                                    match toggled {
                                        Toggled::True => {
                                            next_values.insert(option_value.clone());
                                        }
                                        Toggled::False | Toggled::Mixed => {
                                            next_values.remove(&option_value);
                                        }
                                    }
                                    query_runtime_for_toggle.update(cx, |runtime, _| {
                                        runtime.query = query_for_toggle.clone();
                                    });
                                    if let Some(on_change) = on_change.as_ref() {
                                        let change = TableFacetedFilterChange::new(
                                            column_id.clone(),
                                            next_values.into_iter(),
                                            Some(option_value.clone()),
                                            matches!(toggled, Toggled::True),
                                        );
                                        on_change(change, window, cx);
                                    }
                                }),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_xs()
                                .opacity(0.72)
                                .child(option_count.to_string()),
                        ),
                )
            },
        )
        .into_any_element()
}

fn normalize_table_faceted_query(query: &str) -> String {
    query.trim().to_lowercase()
}

fn table_facet_value_label(value: &TableCellValue) -> String {
    let label = value.filter_text();
    if label.is_empty() {
        String::from("(empty)")
    } else {
        label
    }
}

fn table_faceted_option_matches(option: &TableFacetedFilterOptionState, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    option.label().to_lowercase().contains(query) || option.value().to_lowercase().contains(query)
}

fn table_faceted_selected_labels(
    options: &[TableFacetedFilterOptionState],
    selected_values: &BTreeSet<String>,
) -> Vec<String> {
    let mut labels = Vec::new();
    let mut seen = BTreeSet::new();

    for option in options {
        if selected_values.contains(option.value()) && seen.insert(option.value().to_owned()) {
            labels.push(option.label().to_owned());
        }
    }

    for value in selected_values {
        if seen.insert(value.clone()) {
            labels.push(table_faceted_selected_label_for_value(value));
        }
    }

    labels
}

fn table_faceted_selected_label_for_value(value: &str) -> String {
    if value.is_empty() {
        String::from("(empty)")
    } else {
        value.to_owned()
    }
}

fn table_faceted_trigger_label(label: &str, selected_labels: &[String]) -> String {
    match selected_labels.len() {
        0 => label.to_owned(),
        1 => format!("{label}: {}", selected_labels[0]),
        2 => format!("{label}: {}, {}", selected_labels[0], selected_labels[1]),
        count => format!("{label}: {count} selected"),
    }
}

fn table_faceted_filter_next_filters(
    filters: impl IntoIterator<Item = TableFilter>,
    column_id: &TableColumnId,
    selected_values: &[String],
) -> Vec<TableFilter> {
    let mut next = filters
        .into_iter()
        .filter(|filter| filter.column() != column_id)
        .collect::<Vec<_>>();

    if !selected_values.is_empty() {
        next.push(TableFilter::one_of(
            column_id.clone(),
            selected_values.iter().cloned(),
        ));
    }

    next
}

/// Resolved table sizing and virtualization metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableMetrics {
    size: Size,
    header_height: UiPx,
    row_height: UiPx,
    cell_padding_x: UiPx,
    min_column_width: UiPx,
    viewport_extent: UiPx,
    overscan: usize,
}

impl TableMetrics {
    /// Resolves table metrics from the shared component size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            size,
            header_height: size.button_h(),
            row_height: size.list_row_h(),
            cell_padding_x: size.list_px(),
            min_column_width: match size {
                Size::XSmall => ui_px(96.0),
                Size::Small => ui_px(112.0),
                Size::Medium => ui_px(128.0),
                Size::Large => ui_px(144.0),
            },
            viewport_extent: match size {
                Size::XSmall => ui_px(160.0),
                Size::Small => ui_px(200.0),
                Size::Medium => ui_px(240.0),
                Size::Large => ui_px(280.0),
            },
            overscan: match size {
                Size::XSmall | Size::Small => 4,
                Size::Medium => 6,
                Size::Large => 8,
            },
        }
    }

    /// Returns the foundation size.
    pub const fn size(self) -> Size {
        self.size
    }

    /// Returns the fixed header row height.
    pub const fn header_height(self) -> UiPx {
        self.header_height
    }

    /// Returns the estimated body row height used by the virtualizer.
    pub const fn row_height(self) -> UiPx {
        self.row_height
    }

    /// Returns horizontal cell padding.
    pub const fn cell_padding_x(self) -> UiPx {
        self.cell_padding_x
    }

    /// Returns the minimum visual column width.
    pub const fn min_column_width(self) -> UiPx {
        self.min_column_width
    }

    /// Returns the viewport extent used to resolve the virtual window.
    pub const fn viewport_extent(self) -> UiPx {
        self.viewport_extent
    }

    /// Returns the overscan row budget.
    pub const fn overscan(self) -> usize {
        self.overscan
    }
}

/// One resolved table column in render order.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnRenderPlan {
    id: TableColumnId,
    label: String,
    region: TableColumnRegion,
    aria_column_index: usize,
    sortable: bool,
    text_editable: bool,
    sort_direction: Option<TableSortDirection>,
    sort_action: Option<TableHeaderAction>,
    width: UiPx,
    min_width: UiPx,
    max_width: UiPx,
    start: UiPx,
    after: UiPx,
    resizable: bool,
}

impl TableColumnRenderPlan {
    fn new(
        column: &TableColumn,
        sizing: &TableResolvedColumnSizing,
        region: TableColumnRegion,
        aria_column_index: usize,
        sort_direction: Option<TableSortDirection>,
    ) -> Self {
        debug_assert_eq!(sizing.region(), region);

        Self {
            id: column.id().clone(),
            label: column.label().to_owned(),
            region,
            aria_column_index,
            sortable: column.sortable(),
            text_editable: column.text_editable(),
            sort_direction,
            sort_action: column
                .sortable()
                .then(|| TableHeaderAction::for_column(column, sort_direction)),
            width: sizing.width(),
            min_width: sizing.min_width(),
            max_width: sizing.max_width(),
            start: sizing.start(),
            after: sizing.after(),
            resizable: sizing.resizable(),
        }
    }

    /// Returns the stable column identity.
    pub const fn id(&self) -> &TableColumnId {
        &self.id
    }

    /// Returns the visible header label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the resolved pinning region for this column.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns the 1-based accessibility column index.
    pub const fn aria_column_index(&self) -> usize {
        self.aria_column_index
    }

    /// Returns whether this column is sortable in the contract.
    pub const fn sortable(&self) -> bool {
        self.sortable
    }

    /// Returns whether leaf cells in this column render text editors.
    pub const fn text_editable(&self) -> bool {
        self.text_editable
    }

    /// Returns the resolved sort direction for this column, when present.
    pub const fn sort_direction(&self) -> Option<TableSortDirection> {
        self.sort_direction
    }

    /// Returns the header action emitted when this sortable column is activated.
    pub const fn sort_action(&self) -> Option<&TableHeaderAction> {
        self.sort_action.as_ref()
    }

    /// Returns the resolved column width.
    pub const fn width(&self) -> UiPx {
        self.width
    }

    /// Returns the lower width bound.
    pub const fn min_width(&self) -> UiPx {
        self.min_width
    }

    /// Returns the upper width bound.
    pub const fn max_width(&self) -> UiPx {
        self.max_width
    }

    /// Returns the offset from the start edge of this column's region.
    pub const fn start(&self) -> UiPx {
        self.start
    }

    /// Returns the offset from the end edge of this column's region.
    pub const fn after(&self) -> UiPx {
        self.after
    }

    /// Returns whether the column can be resized.
    pub const fn resizable(&self) -> bool {
        self.resizable
    }

    /// Returns the label exposed to assistive technology.
    pub fn accessible_label(&self) -> String {
        match self.sort_direction {
            Some(direction) => format!("{}, sorted {}", self.label, direction.as_str()),
            None => self.label.clone(),
        }
    }
}

/// Resolved table columns for one render lane.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnRegionRenderPlan {
    region: TableColumnRegion,
    columns: Vec<TableColumnRenderPlan>,
    total_width: UiPx,
}

impl TableColumnRegionRenderPlan {
    fn new(region: TableColumnRegion, columns: Vec<TableColumnRenderPlan>) -> Self {
        let total_width = columns
            .iter()
            .fold(UiPx::ZERO, |total, column| total + column.width());
        Self {
            region,
            columns,
            total_width,
        }
    }

    /// Returns the represented column region.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns columns in this region.
    pub fn columns(&self) -> &[TableColumnRenderPlan] {
        &self.columns
    }

    /// Returns the summed resolved width of columns in this region.
    pub const fn total_width(&self) -> UiPx {
        self.total_width
    }
}

/// Adapter layout metadata for sticky pinned table column regions.
#[derive(Debug, Clone, PartialEq)]
pub struct TablePinnedLayoutPlan {
    table_id: String,
    left_width: UiPx,
    center_width: UiPx,
    right_width: UiPx,
    total_width: UiPx,
}

impl TablePinnedLayoutPlan {
    fn from_column_regions(
        table_id: &str,
        regions: &[TableColumnRegionRenderPlan],
        total_width: UiPx,
    ) -> Option<Self> {
        let region_plan = |region| regions.iter().find(|plan| plan.region() == region);
        let left = region_plan(TableColumnRegion::Left);
        let center = region_plan(TableColumnRegion::Center);
        let right = region_plan(TableColumnRegion::Right);
        let has_pinned_columns = left
            .map(|region| !region.columns().is_empty())
            .unwrap_or(false)
            || right
                .map(|region| !region.columns().is_empty())
                .unwrap_or(false);
        if !has_pinned_columns {
            return None;
        }

        Some(Self {
            table_id: table_id.to_owned(),
            left_width: left
                .map(TableColumnRegionRenderPlan::total_width)
                .unwrap_or(UiPx::ZERO),
            center_width: center
                .map(TableColumnRegionRenderPlan::total_width)
                .unwrap_or(UiPx::ZERO),
            right_width: right
                .map(TableColumnRegionRenderPlan::total_width)
                .unwrap_or(UiPx::ZERO),
            total_width,
        })
    }

    /// Returns the table identity this layout plan belongs to.
    pub fn table_id(&self) -> &str {
        &self.table_id
    }

    /// Returns the total width of the left pinned lane.
    pub const fn left_width(&self) -> UiPx {
        self.left_width
    }

    /// Returns the total width of the horizontally scrollable center lane.
    pub const fn center_width(&self) -> UiPx {
        self.center_width
    }

    /// Returns the total width of the right pinned lane.
    pub const fn right_width(&self) -> UiPx {
        self.right_width
    }

    /// Returns the total width across all visible lanes.
    pub const fn total_width(&self) -> UiPx {
        self.total_width
    }

    /// Returns the stable adapter id for the header center scroll viewport.
    pub fn header_center_scroll_id(&self) -> String {
        format!("table:{}:header-center-scroll", self.table_id)
    }

    /// Returns the stable debug selector for the header center scroll viewport.
    pub fn header_center_scroll_selector(&self) -> String {
        format!("scroll-area:{}", self.header_center_scroll_id())
    }

    /// Returns the stable debug selector for one header region lane.
    pub fn header_region_selector(&self, region: TableColumnRegion) -> String {
        format!("table:{}:header-region:{}", self.table_id, region.as_str())
    }

    /// Returns the stable adapter id for one body-row center scroll viewport.
    pub fn row_center_scroll_id(&self, row_render_key: &str) -> String {
        format!("table:{}:row-center-scroll:{row_render_key}", self.table_id)
    }

    /// Returns the stable debug selector for one body-row center scroll viewport.
    pub fn row_center_scroll_selector(&self, row_render_key: &str) -> String {
        format!("scroll-area:{}", self.row_center_scroll_id(row_render_key))
    }

    /// Returns the stable debug selector for one body-row region lane.
    pub fn row_region_selector(&self, row_render_key: &str, region: TableColumnRegion) -> String {
        format!(
            "table:{}:row-region:{row_render_key}:{}",
            self.table_id,
            region.as_str()
        )
    }
}

/// Resolved render metadata for the virtualized center column lane.
#[derive(Debug, Clone, PartialEq)]
pub struct TableCenterColumnWindowPlan {
    virtualizer: VirtualizerResolvedState,
    rendered_columns: Vec<TableColumnRenderPlan>,
    leading_spacer_width: UiPx,
    trailing_spacer_width: UiPx,
}

impl TableCenterColumnWindowPlan {
    /// Resolves a center-column virtual window from resolved center columns.
    pub fn resolve(
        columns: &[TableColumnRenderPlan],
        scroll_offset: UiPx,
        viewport_extent: UiPx,
        overscan: usize,
    ) -> Option<Self> {
        if columns.is_empty() {
            return None;
        }

        let estimated_size = columns
            .first()
            .map(TableColumnRenderPlan::width)
            .unwrap_or(UiPx::ZERO);
        let virtualizer = VirtualizerState::new(columns.len(), estimated_size)
            .with_viewport_extent(nonnegative_px(viewport_extent))
            .with_scroll_offset(nonnegative_px(scroll_offset))
            .with_overscan(overscan)
            .resolve_known_size_window(|index| {
                let column = &columns[index];
                (
                    VirtualizerItemKey::new(column.id().as_str().to_owned()),
                    column.width(),
                )
            });
        let rendered_columns = virtualizer
            .items()
            .iter()
            .filter_map(|measurement| columns.get(measurement.index()).cloned())
            .collect::<Vec<_>>();
        let leading_spacer_width = virtualizer
            .items()
            .first()
            .map(VirtualizerItemMeasurement::start)
            .unwrap_or(UiPx::ZERO);
        let trailing_spacer_width = virtualizer
            .items()
            .last()
            .map(|item| nonnegative_px(virtualizer.total_size() - item.end()))
            .unwrap_or(UiPx::ZERO);

        Some(Self {
            virtualizer,
            rendered_columns,
            leading_spacer_width,
            trailing_spacer_width,
        })
    }

    /// Returns the total width of the center lane.
    pub const fn center_width(&self) -> UiPx {
        self.virtualizer.total_size()
    }

    /// Returns the visible center-column range before overscan.
    pub const fn visible_range(&self) -> &VirtualizerRange {
        self.virtualizer.visible_range()
    }

    /// Returns the rendered center-column range after overscan.
    pub const fn overscan_range(&self) -> &VirtualizerRange {
        self.virtualizer.overscan_range()
    }

    /// Returns the rendered center columns in window order.
    pub fn rendered_columns(&self) -> &[TableColumnRenderPlan] {
        &self.rendered_columns
    }

    /// Returns the rendered center column count.
    pub fn rendered_column_count(&self) -> usize {
        self.rendered_columns.len()
    }

    /// Returns the leading spacer width before the first rendered center column.
    pub const fn leading_spacer_width(&self) -> UiPx {
        self.leading_spacer_width
    }

    /// Returns the trailing spacer width after the last rendered center column.
    pub const fn trailing_spacer_width(&self) -> UiPx {
        self.trailing_spacer_width
    }

    /// Returns whether the center lane is currently virtualized.
    pub fn virtualized(&self) -> bool {
        self.rendered_columns.len() < self.virtualizer.count()
    }

    /// Returns the resolved virtualizer state.
    pub const fn virtualizer(&self) -> &VirtualizerResolvedState {
        &self.virtualizer
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
    fn for_column(column: &TableColumn, current_direction: Option<TableSortDirection>) -> Self {
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
    fn from_gpui(modifiers: Modifiers) -> Self {
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
    fn from_render_plan(row: &TableRowRenderPlan, modifiers: TableInputModifiers) -> Self {
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
    fn new(action: TableRowAction, kind: TableRowActivationKind) -> Self {
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
    fn new(action: TableRowAction, expanded: bool) -> Self {
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

fn request_table_row_selection_change(
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

/// One resolved table cell in render order.
#[derive(Debug, Clone, PartialEq)]
pub struct TableCellRenderPlan {
    column_id: TableColumnId,
    text: String,
    region: TableColumnRegion,
    aria_column_index: usize,
    role: Role,
    width: UiPx,
    text_editable: bool,
}

impl TableCellRenderPlan {
    fn new(
        column: &TableColumnRenderPlan,
        row: &TableResolvedRow,
        value: Option<&TableCellValue>,
    ) -> Self {
        let text_editable = column.text_editable() && row.is_leaf() && value.is_some();
        Self {
            column_id: column.id().clone(),
            text: value.map(TableCellValue::filter_text).unwrap_or_default(),
            region: column.region(),
            aria_column_index: column.aria_column_index(),
            role: Role::Cell,
            width: column.width(),
            text_editable,
        }
    }

    /// Returns the stable column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the display text resolved from the core cell value.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the resolved pinning region for this cell.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns the 1-based accessibility column index.
    pub const fn aria_column_index(&self) -> usize {
        self.aria_column_index
    }

    /// Returns the accessibility role for this cell.
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Returns the resolved width for this body cell.
    pub const fn width(&self) -> UiPx {
        self.width
    }

    /// Returns whether this resolved leaf cell should render a text editor.
    pub const fn text_editable(&self) -> bool {
        self.text_editable
    }
}

/// One resolved virtualized row to render.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRowRenderPlan {
    row: TableResolvedRow,
    region: TableRowRegion,
    render_key: String,
    model_index: usize,
    aria_row_index: usize,
    measurement: VirtualizerItemMeasurement,
    cells: Vec<TableCellRenderPlan>,
    role: Role,
}

impl TableRowRenderPlan {
    fn new(
        row: TableResolvedRow,
        region: TableRowRegion,
        render_key: String,
        model_index: usize,
        measurement: VirtualizerItemMeasurement,
        columns: &[TableColumnRenderPlan],
    ) -> Self {
        let cells = columns
            .iter()
            .map(|column| TableCellRenderPlan::new(column, &row, row.cell(column.id())))
            .collect();

        Self {
            row,
            region,
            render_key,
            model_index,
            aria_row_index: model_index + 2,
            measurement,
            cells,
            role: Role::Row,
        }
    }

    /// Returns the resolved core row.
    pub const fn row(&self) -> &TableResolvedRow {
        &self.row
    }

    /// Returns the stable row id.
    pub const fn id(&self) -> &open_gpui_ui_core::TableRowId {
        self.row.id()
    }

    /// Returns the row-pinning render region.
    pub const fn region(&self) -> TableRowRegion {
        self.region
    }

    /// Returns the unique render key used by element ids and virtualizer items.
    pub fn render_key(&self) -> &str {
        &self.render_key
    }

    /// Returns this row's index in the final row model.
    pub const fn model_index(&self) -> usize {
        self.model_index
    }

    /// Returns this row's index inside its row-pinning region.
    pub const fn region_index(&self) -> usize {
        self.measurement.index()
    }

    /// Returns the 1-based accessibility row index, including the header row.
    pub const fn aria_row_index(&self) -> usize {
        self.aria_row_index
    }

    /// Returns whether the row is selected by stable row id.
    pub const fn selected(&self) -> bool {
        self.row.selected()
    }

    /// Returns this row's resolved hierarchy depth.
    pub const fn depth(&self) -> usize {
        self.row.depth()
    }

    /// Returns whether this rendered row is a source tree branch.
    pub fn is_tree_branch(&self) -> bool {
        self.row.is_tree_branch()
    }

    /// Returns the source tree expansion state for branch rows.
    pub fn tree_expanded(&self) -> Option<bool> {
        self.row.tree_expanded()
    }

    /// Returns the number of directly loaded child rows.
    pub fn loaded_child_count(&self) -> usize {
        self.row.loaded_child_count()
    }

    /// Returns source-row child loading metadata.
    pub fn children_load_state(&self) -> Option<&TableRowChildrenLoadState> {
        self.row.children_load_state()
    }

    /// Returns the virtual row start offset.
    pub const fn virtual_start(&self) -> UiPx {
        self.measurement.start()
    }

    /// Returns the virtual row size.
    pub const fn virtual_size(&self) -> UiPx {
        self.measurement.size()
    }

    /// Returns the cells in visible column order.
    pub fn cells(&self) -> &[TableCellRenderPlan] {
        &self.cells
    }

    /// Returns cells for one column region.
    pub fn cells_for_region(
        &self,
        region: TableColumnRegion,
    ) -> impl Iterator<Item = &TableCellRenderPlan> {
        self.cells
            .iter()
            .filter(move |cell| cell.region() == region)
    }

    /// Returns the accessibility role for this row.
    pub const fn role(&self) -> Role {
        self.role
    }
}

/// Fully resolved render contract for a concrete [`Table`] instance.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRenderPlan {
    table_id: String,
    label: String,
    metrics: TableMetrics,
    table: Rc<TableResolvedState>,
    virtualizer: VirtualizerResolvedState,
    columns: Vec<TableColumnRenderPlan>,
    column_regions: Vec<TableColumnRegionRenderPlan>,
    pinned_layout: Option<TablePinnedLayoutPlan>,
    center_column_window: Option<TableCenterColumnWindowPlan>,
    grid_viewport: Option<GridViewport2D>,
    total_column_width: UiPx,
    filtering_mode: TableStageMode,
    sorting_mode: TableStageMode,
    pagination_mode: TableStageMode,
    pagination_row_count: Option<usize>,
    pagination_page_count: Option<usize>,
    faceting_mode: TableStageMode,
    selection_policy: TableSelectionPolicy,
    selection_summary: TableSelectionSummary,
    aggregation_fn_count: usize,
    top_rows: Vec<TableRowRenderPlan>,
    rows: Vec<TableRowRenderPlan>,
    bottom_rows: Vec<TableRowRenderPlan>,
    role: Role,
    header_row_role: Role,
    column_header_role: Role,
    cell_role: Role,
}

impl TableRenderPlan {
    fn resolve(
        table_id: String,
        label: String,
        metrics: TableMetrics,
        state: &TableState,
        table: Rc<TableResolvedState>,
        virtualizer: VirtualizerResolvedState,
        columns: Vec<TableColumnRenderPlan>,
        center_scroll_offset: Option<UiPx>,
        center_viewport_extent: Option<UiPx>,
    ) -> Self {
        let column_regions = resolve_column_region_render_plans(&columns);
        let total_column_width = column_regions
            .iter()
            .fold(UiPx::ZERO, |total, region| total + region.total_width());
        let pinned_layout = TablePinnedLayoutPlan::from_column_regions(
            &table_id,
            &column_regions,
            total_column_width,
        );
        let center_column_window = resolve_center_column_window(
            &column_regions,
            center_scroll_offset,
            center_viewport_extent,
            metrics.overscan(),
        );
        let grid_viewport = center_column_window.as_ref().map(|center_window| {
            GridViewport2D::new(virtualizer.clone(), center_window.virtualizer().clone())
        });
        let duplicate_row_ids = table
            .duplicate_row_ids()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let top_row_count = table.top_rows().len();
        let center_total_row_count = table.center_rows().len();
        let top_rows = static_row_render_plans(
            table.top_rows(),
            TableRowRegion::Top,
            metrics.row_height(),
            &columns,
            &duplicate_row_ids,
            0,
        );
        let rows = virtualized_center_row_render_plans(
            table.center_rows(),
            virtualizer.items(),
            &columns,
            &duplicate_row_ids,
            top_row_count,
        );
        let bottom_rows = static_row_render_plans(
            table.bottom_rows(),
            TableRowRegion::Bottom,
            metrics.row_height(),
            &columns,
            &duplicate_row_ids,
            top_row_count + center_total_row_count,
        );
        let pagination = state.pagination();
        let selection_summary = table.final_selection_summary();

        Self {
            table_id,
            label,
            metrics,
            table,
            virtualizer,
            columns,
            column_regions,
            pinned_layout,
            center_column_window,
            grid_viewport,
            total_column_width,
            filtering_mode: state.filtering_mode(),
            sorting_mode: state.sorting_mode(),
            pagination_mode: pagination.mode(),
            pagination_row_count: pagination.row_count(),
            pagination_page_count: pagination.page_count(),
            faceting_mode: state.faceting_mode(),
            selection_policy: state.selection_policy(),
            selection_summary,
            aggregation_fn_count: state.aggregation_fn_count(),
            top_rows,
            rows,
            bottom_rows,
            role: Role::Table,
            header_row_role: Role::Row,
            column_header_role: Role::ColumnHeader,
            cell_role: Role::Cell,
        }
    }

    /// Returns the stable table id.
    pub fn table_id(&self) -> &str {
        &self.table_id
    }

    /// Returns the accessible table label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> TableMetrics {
        self.metrics
    }

    /// Returns the resolved renderer-neutral table state.
    pub fn table(&self) -> &TableResolvedState {
        self.table.as_ref()
    }

    /// Returns whether filtering was resolved locally or supplied by the caller.
    pub const fn filtering_mode(&self) -> TableStageMode {
        self.filtering_mode
    }

    /// Returns whether sorting was resolved locally or supplied by the caller.
    pub const fn sorting_mode(&self) -> TableStageMode {
        self.sorting_mode
    }

    /// Returns whether pagination was resolved locally or supplied by the caller.
    pub const fn pagination_mode(&self) -> TableStageMode {
        self.pagination_mode
    }

    /// Returns the server-known total row count, when supplied.
    pub const fn pagination_row_count(&self) -> Option<usize> {
        self.pagination_row_count
    }

    /// Returns the explicit or derived total page count, when supplied.
    pub const fn pagination_page_count(&self) -> Option<usize> {
        self.pagination_page_count
    }

    /// Returns whether faceting was resolved locally or supplied by the caller.
    pub const fn faceting_mode(&self) -> TableStageMode {
        self.faceting_mode
    }

    /// Returns the row-selection policy.
    pub const fn selection_policy(&self) -> TableSelectionPolicy {
        self.selection_policy
    }

    /// Returns the final row-model selection summary.
    pub const fn selection_summary(&self) -> TableSelectionSummary {
        self.selection_summary
    }

    /// Returns the number of named custom aggregation callbacks registered on the table state.
    pub const fn aggregation_fn_count(&self) -> usize {
        self.aggregation_fn_count
    }

    /// Returns resolved facet metadata for configured columns.
    pub fn column_facets(&self) -> &[TableColumnFacets] {
        self.table.column_facets()
    }

    /// Returns resolved facet metadata for one configured column.
    pub fn column_facet(&self, column: &TableColumnId) -> Option<&TableColumnFacets> {
        self.table.column_facet(column)
    }

    /// Returns the resolved renderer-neutral virtualizer state.
    pub const fn virtualizer(&self) -> &VirtualizerResolvedState {
        &self.virtualizer
    }

    /// Returns visible columns in render order.
    pub fn columns(&self) -> &[TableColumnRenderPlan] {
        &self.columns
    }

    /// Returns visible columns split into render regions.
    pub fn column_regions(&self) -> &[TableColumnRegionRenderPlan] {
        &self.column_regions
    }

    /// Returns sticky pinned-column layout metadata, when a split layout is needed.
    pub fn pinned_layout(&self) -> Option<&TablePinnedLayoutPlan> {
        self.pinned_layout.as_ref()
    }

    /// Returns center-column window metadata, when the center lane exists.
    pub fn center_column_window(&self) -> Option<&TableCenterColumnWindowPlan> {
        self.center_column_window.as_ref()
    }

    /// Returns the combined row and center-column viewport when both axes are available.
    pub fn grid_viewport(&self) -> Option<&GridViewport2D> {
        self.grid_viewport.as_ref()
    }

    /// Returns whether this render plan needs split pinned-column layout.
    pub fn uses_split_pinned_layout(&self) -> bool {
        self.pinned_layout.is_some()
    }

    /// Returns the summed resolved width of all visible columns.
    pub const fn total_column_width(&self) -> UiPx {
        self.total_column_width
    }

    /// Returns the summed resolved width of one visible column region.
    pub fn column_region_width(&self, region: TableColumnRegion) -> UiPx {
        self.column_regions
            .iter()
            .find(|plan| plan.region() == region)
            .map(TableColumnRegionRenderPlan::total_width)
            .unwrap_or(UiPx::ZERO)
    }

    /// Returns top-pinned rows in render order.
    pub fn top_rows(&self) -> &[TableRowRenderPlan] {
        &self.top_rows
    }

    /// Returns virtualized center rows in render order.
    pub fn rows(&self) -> &[TableRowRenderPlan] {
        &self.rows
    }

    /// Returns virtualized center rows in render order.
    pub fn center_rows(&self) -> &[TableRowRenderPlan] {
        &self.rows
    }

    /// Returns bottom-pinned rows in render order.
    pub fn bottom_rows(&self) -> &[TableRowRenderPlan] {
        &self.bottom_rows
    }

    /// Returns all currently rendered rows in visual order.
    pub fn rendered_rows(&self) -> impl Iterator<Item = &TableRowRenderPlan> {
        self.top_rows
            .iter()
            .chain(self.rows.iter())
            .chain(self.bottom_rows.iter())
    }

    /// Returns the accessibility role for the table root.
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Returns the accessibility role for row containers.
    pub const fn row_role(&self) -> Role {
        self.header_row_role
    }

    /// Returns the accessibility role for header cells.
    pub const fn column_header_role(&self) -> Role {
        self.column_header_role
    }

    /// Returns the accessibility role for body cells.
    pub const fn cell_role(&self) -> Role {
        self.cell_role
    }

    /// Returns the accessibility row count, including the header row.
    pub fn aria_row_count(&self) -> usize {
        self.table.final_model().rows().len().saturating_add(1)
    }

    /// Returns the accessibility column count.
    pub fn aria_column_count(&self) -> usize {
        self.columns.len()
    }

    /// Returns the number of body rows rendered after overscan.
    pub fn rendered_row_count(&self) -> usize {
        self.top_rows.len() + self.rows.len() + self.bottom_rows.len()
    }

    /// Returns the visible body row count before overscan.
    pub fn visible_row_count(&self) -> usize {
        self.top_rows.len() + self.virtualizer.visible_items().len() + self.bottom_rows.len()
    }
}

fn static_row_render_plans(
    rows: &[TableResolvedRow],
    region: TableRowRegion,
    row_height: UiPx,
    columns: &[TableColumnRenderPlan],
    duplicate_row_ids: &BTreeSet<TableRowId>,
    model_index_start: usize,
) -> Vec<TableRowRenderPlan> {
    rows.iter()
        .enumerate()
        .map(|(region_index, row)| {
            let row = row.clone();
            let render_key = row_render_key(&row, duplicate_row_ids);
            let model_index = model_index_start + region_index;
            let measurement = VirtualizerItemMeasurement::new(
                region_index,
                VirtualizerItemKey::new(render_key.clone()),
                row_height * region_index as f32,
                row_height,
                false,
            );
            TableRowRenderPlan::new(row, region, render_key, model_index, measurement, columns)
        })
        .collect()
}

fn virtualized_center_row_render_plans(
    rows: &[TableResolvedRow],
    measurements: &[VirtualizerItemMeasurement],
    columns: &[TableColumnRenderPlan],
    duplicate_row_ids: &BTreeSet<TableRowId>,
    model_index_start: usize,
) -> Vec<TableRowRenderPlan> {
    measurements
        .iter()
        .filter_map(|measurement| {
            rows.get(measurement.index()).cloned().map(|row| {
                let render_key = row_render_key(&row, duplicate_row_ids);
                let model_index = model_index_start + measurement.index();
                TableRowRenderPlan::new(
                    row,
                    TableRowRegion::Center,
                    render_key,
                    model_index,
                    measurement.clone(),
                    columns,
                )
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
struct TableResolvedCache {
    key: TableStateCacheKey,
    table: Rc<TableResolvedState>,
    columns: Vec<TableColumnRenderPlan>,
}

#[derive(Debug, Clone, Default)]
struct TableRuntime {
    scroll_handle: ScrollHandle,
    horizontal_scroll_handle: ScrollHandle,
    resolved: Option<TableResolvedCache>,
    column_resize: TableColumnResizeState,
    focused_row: Option<TableRowId>,
    focus_handles: BTreeMap<TableRowId, FocusHandle>,
    expansion_override: Option<TableExpansionState>,
    selection_anchor: Option<TableRowId>,
}

impl TableRuntime {
    fn sync_rows(&mut self, plan: &TableRenderPlan, cx: &mut Context<Self>) {
        let rendered_row_ids = plan
            .rendered_rows()
            .map(|row| row.id().clone())
            .collect::<BTreeSet<_>>();
        self.focus_handles
            .retain(|row_id, _| rendered_row_ids.contains(row_id));

        for row in plan.rendered_rows() {
            self.focus_handles
                .entry(row.id().clone())
                .or_insert_with(|| cx.focus_handle());
        }

        if self.focused_row.is_none() {
            self.focused_row = plan.rendered_rows().next().map(|row| row.id().clone());
        }
    }

    fn set_focused(&mut self, row_id: TableRowId, cx: &mut Context<Self>) -> Option<FocusHandle> {
        let changed = self.focused_row.as_ref() != Some(&row_id);
        self.focused_row = Some(row_id.clone());
        if changed {
            cx.notify();
        }
        self.focus_handles.get(&row_id).cloned()
    }

    fn set_expansion_override(&mut self, expansion: TableExpansionState, cx: &mut Context<Self>) {
        if self.expansion_override.as_ref() != Some(&expansion) {
            self.expansion_override = Some(expansion);
            self.resolved = None;
            cx.notify();
        }
    }

    fn set_selection_anchor(&mut self, row_id: Option<TableRowId>, cx: &mut Context<Self>) {
        if self.selection_anchor != row_id {
            self.selection_anchor = row_id;
            cx.notify();
        }
    }
}

#[derive(Clone)]
struct TableResizeRenderConfig {
    table_id: String,
    enabled: bool,
    mode: TableColumnResizeMode,
    direction: TableColumnResizeDirection,
    base_sizing: TableColumnSizing,
    runtime: Entity<TableRuntime>,
    on_change: Option<TableColumnSizingHandler>,
}

#[derive(Debug, Clone, PartialEq)]
struct TableColumnResizeDrag {
    table_id: String,
    column_id: TableColumnId,
    start_width: UiPx,
    column_widths_start: Vec<(TableColumnId, UiPx)>,
    base_sizing: TableColumnSizing,
    mode: TableColumnResizeMode,
    direction: TableColumnResizeDirection,
}

/// A concrete GPUI table renderer using the Open GPUI row-model and virtualizer contracts.
#[derive(IntoElement)]
pub struct Table {
    id: String,
    label: SharedString,
    state: TableState,
    metrics: TableMetrics,
    snapshot: Option<VirtualizerSnapshot>,
    default_focused_row: Option<TableRowId>,
    on_sort_requested: Option<TableSortHandler>,
    enable_column_resizing: bool,
    column_resize_mode: TableColumnResizeMode,
    column_resize_direction: TableColumnResizeDirection,
    on_column_sizing_change: Option<TableColumnSizingHandler>,
    on_row_selection_change: Option<TableRowSelectionHandler>,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
}

impl Table {
    /// Creates a new table renderer from renderer-neutral table state.
    pub fn new(id: impl Into<String>, label: impl Into<SharedString>, state: TableState) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state,
            metrics: TableMetrics::from_size(Size::Medium),
            snapshot: None,
            default_focused_row: None,
            on_sort_requested: None,
            enable_column_resizing: true,
            column_resize_mode: TableColumnResizeMode::default(),
            column_resize_direction: TableColumnResizeDirection::default(),
            on_column_sizing_change: None,
            on_row_selection_change: None,
            on_row_activate: None,
            on_row_expansion_request: None,
            on_cell_edit_change: None,
        }
    }

    /// Applies the accessible table label.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Applies the overscan row budget.
    pub fn overscan(mut self, overscan: usize) -> Self {
        self.metrics.overscan = overscan;
        self
    }

    /// Applies a fixed row height.
    pub fn row_height(mut self, row_height: UiPx) -> Self {
        self.metrics.row_height = nonnegative_px(row_height);
        self
    }

    /// Applies a fixed header height.
    pub fn header_height(mut self, header_height: UiPx) -> Self {
        self.metrics.header_height = nonnegative_px(header_height);
        self
    }

    /// Applies the fallback viewport extent used before layout metrics exist.
    pub fn viewport_extent(mut self, viewport_extent: UiPx) -> Self {
        self.metrics.viewport_extent = nonnegative_px(viewport_extent);
        self
    }

    /// Applies the source-tree expansion mode.
    pub fn expansion_mode(mut self, expansion_mode: TableExpansionMode) -> Self {
        self.state = self.state.clone().with_expansion_mode(expansion_mode);
        self
    }

    /// Applies the minimum visual column width.
    pub fn min_column_width(mut self, min_column_width: UiPx) -> Self {
        self.metrics.min_column_width = nonnegative_px(min_column_width);
        self
    }

    /// Seeds table virtualizer measurements from a snapshot.
    ///
    /// The adapter applies the live `ScrollHandle` offset after restoring snapshot measurements.
    /// One-shot scroll-position restoration belongs to the runtime scroll owner.
    pub fn virtualizer_snapshot(mut self, snapshot: VirtualizerSnapshot) -> Self {
        self.snapshot = Some(snapshot);
        self
    }

    /// Seeds the adapter-owned focused row.
    pub fn default_focused_row(mut self, row_id: impl Into<TableRowId>) -> Self {
        self.default_focused_row = Some(row_id.into());
        self
    }

    /// Registers a handler for sortable column header activation.
    pub fn on_sort_requested(
        mut self,
        handler: impl Fn(TableHeaderAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_sort_requested = Some(Rc::new(handler));
        self
    }

    /// Enables or disables column resizing handles.
    pub fn enable_column_resizing(mut self, enabled: bool) -> Self {
        self.enable_column_resizing = enabled;
        self
    }

    /// Applies the resize commit mode.
    pub fn column_resize_mode(mut self, mode: TableColumnResizeMode) -> Self {
        self.column_resize_mode = mode;
        self
    }

    /// Applies the resize direction used for pointer deltas.
    pub fn column_resize_direction(mut self, direction: TableColumnResizeDirection) -> Self {
        self.column_resize_direction = direction;
        self
    }

    /// Registers a handler for committed column sizing changes.
    pub fn on_column_sizing_change(
        mut self,
        handler: impl Fn(TableColumnSizingChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_column_sizing_change = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for controlled row selection changes.
    pub fn on_row_selection_change(
        mut self,
        handler: impl Fn(TableRowSelectionChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_row_selection_change = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for row activation gestures.
    pub fn on_row_activate(
        mut self,
        handler: impl Fn(TableRowActivation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_row_activate = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for controlled row expansion requests.
    pub fn on_row_expansion_request(
        mut self,
        handler: impl Fn(TableRowExpansionToggle, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_row_expansion_request = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for controlled text-cell edit changes.
    pub fn on_cell_edit_change(
        mut self,
        handler: impl Fn(TableCellEditChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_cell_edit_change = Some(Rc::new(handler));
        self
    }

    /// Returns the renderer-neutral table input.
    pub const fn table_state(&self) -> &TableState {
        &self.state
    }

    /// Returns a default resolved plan at scroll origin.
    pub fn state(&self) -> TableRenderPlan {
        self.render_plan(UiPx::ZERO, self.metrics.viewport_extent())
    }

    /// Resolves table row models and the virtual render window for a viewport.
    pub fn render_plan(&self, scroll_offset: UiPx, viewport_extent: UiPx) -> TableRenderPlan {
        let metrics = self.metrics_for_viewport(viewport_extent);
        let table = Rc::new(self.state.resolve());
        let columns = self.resolve_columns(&table);
        let virtualizer = self.resolve_virtualizer(&table, metrics, scroll_offset);

        TableRenderPlan::resolve(
            self.id.clone(),
            self.label.to_string(),
            metrics,
            &self.state,
            table,
            virtualizer,
            columns,
            None,
            None,
        )
    }

    fn render_plan_with_runtime(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
        horizontal_scroll_handle: ScrollHandle,
        runtime: &mut TableRuntime,
    ) -> TableRenderPlan {
        let metrics = self.metrics_for_viewport(viewport_extent);
        let state = runtime
            .expansion_override
            .as_ref()
            .cloned()
            .map(|expansion| apply_table_expansion(self.state.clone(), expansion))
            .unwrap_or_else(|| self.state.clone());
        let cache_key = state.cache_key();
        let needs_resolve = runtime
            .resolved
            .as_ref()
            .map(|cache| cache.key != cache_key)
            .unwrap_or(true);

        if needs_resolve {
            let table = Rc::new(state.resolve());
            let columns = self.resolve_columns(&table);
            runtime.resolved = Some(TableResolvedCache {
                key: cache_key,
                table,
                columns,
            });
        }

        let cache = runtime
            .resolved
            .as_ref()
            .expect("table runtime cache should be initialized");
        let virtualizer = self.resolve_virtualizer(&cache.table, metrics, scroll_offset);
        let center_scroll_offset =
            ui_px((-ui_px_from_gpui(horizontal_scroll_handle.offset().x).as_f32()).max(0.0));
        let center_viewport_extent = ui_px_from_gpui(horizontal_scroll_handle.bounds().size.width);
        let center_viewport_extent =
            (center_viewport_extent.as_f32() > 0.0).then_some(center_viewport_extent);
        let center_scroll_offset = center_viewport_extent.map(|_| center_scroll_offset);

        TableRenderPlan::resolve(
            self.id.clone(),
            self.label.to_string(),
            metrics,
            &state,
            cache.table.clone(),
            virtualizer,
            cache.columns.clone(),
            center_scroll_offset,
            center_viewport_extent,
        )
    }

    fn metrics_for_viewport(&self, viewport_extent: UiPx) -> TableMetrics {
        let mut metrics = self.metrics;
        let viewport_extent = nonnegative_px(viewport_extent);
        if viewport_extent.as_f32() > 0.0 {
            metrics.viewport_extent = viewport_extent;
        }
        metrics
    }

    fn resolve_virtualizer(
        &self,
        table: &TableResolvedState,
        metrics: TableMetrics,
        scroll_offset: UiPx,
    ) -> VirtualizerResolvedState {
        let center_rows = table.center_rows();
        let duplicate_row_ids = table
            .duplicate_row_ids()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let virtualizer = VirtualizerState::new(center_rows.len(), metrics.row_height())
            .with_viewport_extent(metrics.viewport_extent())
            .with_overscan(metrics.overscan())
            .with_scroll_offset(nonnegative_px(scroll_offset));

        if let Some(snapshot) = self.snapshot.clone() {
            let row_keys = center_rows
                .iter()
                .map(|row| row_render_key(row, &duplicate_row_ids));
            return virtualizer
                .with_item_keys(row_keys)
                .with_snapshot(snapshot)
                .with_scroll_offset(nonnegative_px(scroll_offset))
                .resolve();
        }

        virtualizer.resolve_fixed_window(|index| {
            let row = &center_rows[index];
            VirtualizerItemKey::new(row_render_key(row, &duplicate_row_ids))
        })
    }

    fn resolve_columns(&self, table: &TableResolvedState) -> Vec<TableColumnRenderPlan> {
        let mut aria_column_index = 1;
        let mut columns = Vec::new();
        let visible_regions = table.visible_column_regions();
        let visible_sizing = table.visible_column_sizing();

        for region in TableColumnRegion::ALL {
            for column in visible_regions.region(region) {
                let sizing = visible_sizing
                    .column(column.id())
                    .expect("visible column sizing should resolve for visible columns");
                let sort_direction = self
                    .state
                    .sorting()
                    .iter()
                    .find(|sort| sort.column() == column.id())
                    .map(|sort| sort.direction());
                columns.push(TableColumnRenderPlan::new(
                    column,
                    sizing,
                    region,
                    aria_column_index,
                    sort_direction,
                ));
                aria_column_index += 1;
            }
        }

        columns
    }
}

fn resolve_column_region_render_plans(
    columns: &[TableColumnRenderPlan],
) -> Vec<TableColumnRegionRenderPlan> {
    TableColumnRegion::ALL
        .into_iter()
        .map(|region| {
            TableColumnRegionRenderPlan::new(
                region,
                columns
                    .iter()
                    .filter(|column| column.region() == region)
                    .cloned()
                    .collect(),
            )
        })
        .collect()
}

fn resolve_center_column_window(
    regions: &[TableColumnRegionRenderPlan],
    scroll_offset: Option<UiPx>,
    viewport_extent: Option<UiPx>,
    overscan: usize,
) -> Option<TableCenterColumnWindowPlan> {
    let center = regions
        .iter()
        .find(|plan| plan.region() == TableColumnRegion::Center)?;
    let viewport_extent = viewport_extent.unwrap_or_else(|| center.total_width());

    TableCenterColumnWindowPlan::resolve(
        center.columns(),
        scroll_offset.unwrap_or(UiPx::ZERO),
        viewport_extent,
        overscan,
    )
}

impl Sizable for Table {
    fn with_size(mut self, size: Size) -> Self {
        self.metrics = TableMetrics::from_size(size);
        self
    }
}

impl RenderOnce for Table {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime_id = format!("table:{}:runtime", self.id);
        let default_focused_row = self.default_focused_row.clone();
        let runtime = window.use_keyed_state(runtime_id, cx, |_, _| TableRuntime {
            scroll_handle: ScrollHandle::new(),
            horizontal_scroll_handle: ScrollHandle::new(),
            resolved: None,
            column_resize: TableColumnResizeState::default(),
            focused_row: default_focused_row,
            focus_handles: BTreeMap::new(),
            expansion_override: None,
            selection_anchor: None,
        });
        let scroll_handle = runtime.read(cx).scroll_handle.clone();
        let horizontal_scroll_handle = runtime.read(cx).horizontal_scroll_handle.clone();
        let viewport_extent = ui_px_from_gpui(scroll_handle.bounds().size.height);
        let scroll_offset = ui_px((-ui_px_from_gpui(scroll_handle.offset().y).as_f32()).max(0.0));
        let on_sort_requested = self.on_sort_requested.clone();
        let column_resizing_enabled =
            self.enable_column_resizing && self.on_column_sizing_change.is_some();
        let resize_config = TableResizeRenderConfig {
            table_id: self.id.clone(),
            enabled: column_resizing_enabled,
            mode: self.column_resize_mode,
            direction: self.column_resize_direction,
            base_sizing: self.state.column_sizing().clone(),
            runtime: runtime.clone(),
            on_change: self.on_column_sizing_change.clone(),
        };
        let resize_drag_runtime = resize_config.runtime.clone();
        let resize_drag_config = resize_config.clone();
        let plan = runtime.update(cx, |runtime, cx| {
            let plan = self.render_plan_with_runtime(
                scroll_offset,
                viewport_extent,
                horizontal_scroll_handle.clone(),
                runtime,
            );
            runtime.sync_rows(&plan, cx);
            plan
        });
        let runtime_snapshot = runtime.read(cx).clone();
        let current_expansion = runtime_snapshot
            .expansion_override
            .clone()
            .unwrap_or_else(|| self.state.expansion().clone());
        let table_id = plan.table_id().to_owned();
        let label = plan.label().to_owned();
        let metrics = plan.metrics();
        let scroll_viewport_id = format!("table:{table_id}:body-scroll");
        let selection_policy = plan.selection_policy();
        let selected_row_ids = Rc::new(
            plan.table()
                .core_model()
                .rows()
                .iter()
                .filter(|row| row.selected())
                .map(|row| row.id().clone())
                .collect::<Vec<_>>(),
        );
        let on_row_activate = self.on_row_activate.clone();
        let on_row_selection_change = self.on_row_selection_change.clone();
        let on_row_expansion_request = self.on_row_expansion_request.clone();
        let on_cell_edit_change = self.on_cell_edit_change.clone();

        div()
            .id(self.id)
            .debug_selector({
                let table_id = table_id.clone();
                move || format!("table:{table_id}:root")
            })
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .overflow_hidden()
            .text_size(gpui_px_from_ui(metrics.size().control_text_px()))
            .text_color(rgb(0x2f3845))
            .ui_role(plan.role())
            .aria_label(label)
            .when(plan.aria_row_count() > 0, |this| {
                this.aria_row_count(plan.aria_row_count())
            })
            .when(plan.aria_column_count() > 0, |this| {
                this.aria_column_count(plan.aria_column_count())
            })
            .on_scroll_wheel({
                let scroll_handle = scroll_handle.clone();
                move |event, window, cx| {
                    handle_table_vertical_scroll_wheel(&scroll_handle, event, window);
                    window.prevent_default();
                    cx.stop_propagation();
                }
            })
            .when(resize_config.enabled, |this| {
                this.on_drag_move(
                    move |event: &DragMoveEvent<TableColumnResizeDrag>, window, cx| {
                        handle_table_column_resize_drag(
                            &resize_drag_runtime,
                            &resize_drag_config,
                            event,
                            window,
                            cx,
                        );
                    },
                )
            })
            .child(render_table_header(
                &plan,
                on_sort_requested,
                resize_config,
                horizontal_scroll_handle.clone(),
            ))
            .child(render_table_body(
                &plan,
                scroll_viewport_id,
                horizontal_scroll_handle,
                scroll_handle.clone(),
                runtime.clone(),
                runtime_snapshot,
                current_expansion,
                selection_policy,
                selected_row_ids,
                on_row_selection_change,
                on_row_activate,
                on_row_expansion_request,
                on_cell_edit_change,
            ))
    }
}

fn render_table_header(
    plan: &TableRenderPlan,
    on_sort_requested: Option<TableSortHandler>,
    resize_config: TableResizeRenderConfig,
    horizontal_scroll_handle: ScrollHandle,
) -> impl IntoElement {
    let table_id = plan.table_id().to_owned();
    let metrics = plan.metrics();
    let column_header_role = plan.column_header_role();
    let regions = plan.column_regions().to_vec();
    let pinned_layout = plan.pinned_layout().cloned();
    let center_window = if pinned_layout.is_some() {
        plan.center_column_window().cloned().map(Rc::new)
    } else {
        None
    };

    div()
        .id(format!("table:{table_id}:header-row"))
        .debug_selector({
            let table_id = table_id.clone();
            move || format!("table:{table_id}:header-row")
        })
        .h(gpui_px_from_ui(metrics.header_height()))
        .flex_none()
        .flex()
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xf3f4ef))
        .ui_role(plan.row_role())
        .aria_row_index(1)
        .children(regions.into_iter().map(move |region_plan| {
            let table_id = table_id.clone();
            let center_window = center_window.clone();
            let region = region_plan.region();
            let region_name = region.as_str().to_owned();
            let active_center_window = (region == TableColumnRegion::Center)
                .then_some(center_window.as_deref())
                .flatten();
            let region_width = active_center_window
                .map(TableCenterColumnWindowPlan::center_width)
                .unwrap_or_else(|| region_plan.total_width());
            let columns = active_center_window
                .map(|window| window.rendered_columns().to_vec())
                .unwrap_or_else(|| region_plan.columns().to_vec());
            let mut header_children =
                Vec::with_capacity(columns.len() + usize::from(active_center_window.is_some()) * 2);
            if let Some(window) = active_center_window {
                header_children.push(render_table_lane_spacer(window.leading_spacer_width()));
            }
            header_children.extend(columns.into_iter().map({
                let table_id = table_id.clone();
                let on_sort_requested = on_sort_requested.clone();
                let resize_config = resize_config.clone();
                move |column| {
                    render_table_header_cell(
                        table_id.clone(),
                        metrics,
                        column_header_role,
                        column,
                        on_sort_requested.clone(),
                        resize_config.clone(),
                    )
                    .into_any_element()
                }
            }));
            if let Some(window) = active_center_window {
                header_children.push(render_table_lane_spacer(window.trailing_spacer_width()));
            }
            let center_scroll_id = pinned_layout.as_ref().and_then(|layout| {
                (region == TableColumnRegion::Center && !region_plan.columns().is_empty())
                    .then(|| layout.header_center_scroll_id())
            });

            let region_lane = div()
                .id(format!("table:{table_id}:header-region:{region_name}"))
                .debug_selector({
                    let table_id = table_id.clone();
                    let region_name = region_name.clone();
                    move || format!("table:{table_id}:header-region:{region_name}")
                })
                .h_full()
                .w(gpui_px_from_ui(region_width))
                .flex()
                .items_center()
                .overflow_hidden()
                .children(header_children)
                .into_any_element();

            if let Some(center_scroll_id) = center_scroll_id {
                div()
                    .h_full()
                    .min_w(px(0.0))
                    .flex_1()
                    .child(
                        ScrollArea::new(center_scroll_id, region_lane)
                            .horizontal()
                            .scroll_handle(&horizontal_scroll_handle)
                            .with_size(metrics.size()),
                    )
                    .into_any_element()
            } else {
                region_lane
            }
        }))
}

fn render_table_header_cell(
    table_id: String,
    metrics: TableMetrics,
    column_header_role: Role,
    column: TableColumnRenderPlan,
    on_sort_requested: Option<TableSortHandler>,
    resize_config: TableResizeRenderConfig,
) -> impl IntoElement {
    let column_id = column.id().as_str().to_owned();
    let header_table_id = table_id.clone();
    let header_column_id = column_id.clone();
    let accessible_label = column.accessible_label();
    let sort_action = column.sort_action().cloned();
    let interactive_sort = sort_action.zip(on_sort_requested);
    let show_resize_handle = resize_config.enabled && column.resizable();
    let resize_handle_table_id = table_id.clone();
    let resize_handle_column = column.clone();
    let resize_handle_config = resize_config;
    let sort_suffix = column
        .sort_direction()
        .map(|direction| match direction {
            TableSortDirection::Ascending => " ↑",
            TableSortDirection::Descending => " ↓",
        })
        .unwrap_or("");

    div()
        .id(format!("table:{table_id}:header:{column_id}"))
        .debug_selector(move || format!("table:{header_table_id}:header:{header_column_id}"))
        .w(gpui_px_from_ui(column.width()))
        .min_w(gpui_px_from_ui(column.min_width()))
        .max_w(gpui_px_from_ui(column.max_width()))
        .flex_none()
        .relative()
        .h_full()
        .min_h(px(0.0))
        .flex()
        .items_center()
        .px(gpui_px_from_ui(metrics.cell_padding_x()))
        .border_r_1()
        .border_color(rgb(0xd6d8ce))
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0x3f4a57))
        .truncate()
        .whitespace_nowrap()
        .ui_role(column_header_role)
        .aria_label(accessible_label)
        .aria_column_index(column.aria_column_index())
        .when_some(interactive_sort, |this, (action, handler)| {
            let key_action = action.clone();
            let key_handler = handler.clone();

            this.focusable()
                .tab_stop(true)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0xe9ece3)))
                .on_click(move |_event: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    handler(action.clone(), window, cx);
                })
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    if !matches!(event.keystroke.key.as_str(), "space" | "enter") {
                        return;
                    }

                    cx.stop_propagation();
                    key_handler(key_action.clone(), window, cx);
                })
        })
        .child(format!("{}{}", column.label(), sort_suffix))
        .when(show_resize_handle, |this| {
            this.child(render_table_resize_handle(
                resize_handle_table_id,
                resize_handle_column,
                resize_handle_config,
            ))
        })
}

fn render_table_resize_handle(
    table_id: String,
    column: TableColumnRenderPlan,
    config: TableResizeRenderConfig,
) -> impl IntoElement {
    let column_id = column.id().clone();
    let column_key = column_id.as_str().to_owned();
    let drag = TableColumnResizeDrag {
        table_id: table_id.clone(),
        column_id: column_id.clone(),
        start_width: column.width(),
        column_widths_start: vec![(column_id.clone(), column.width())],
        base_sizing: config.base_sizing.clone(),
        mode: config.mode,
        direction: config.direction,
    };
    let drag_for_mouse_up = drag.clone();
    let drag_for_mouse_up_out = drag.clone();
    let drag_for_drag = drag.clone();
    let drag_table_id = table_id.clone();
    let drag_runtime = config.runtime.clone();
    let mouse_up_runtime = config.runtime.clone();
    let mouse_up_config = config.clone();
    let mouse_up_out_runtime = config.runtime.clone();
    let mouse_up_out_config = config;

    div()
        .id(format!("table:{table_id}:resize:{column_key}"))
        .debug_selector(move || format!("table:{table_id}:resize:{column_key}"))
        .absolute()
        .top(px(0.0))
        .right(px(0.0))
        .h_full()
        .w(px(10.0))
        .cursor(CursorStyle::ResizeColumn)
        .occlude()
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .on_drag(
            drag_for_drag,
            move |drag, cursor_offset, bounds, window, cx| {
                if drag.table_id != drag_table_id {
                    return cx.new(|_| Empty);
                }

                let start_x = ui_px_from_gpui(bounds.origin.x + cursor_offset.x);
                drag_runtime.update(cx, |runtime, _| {
                    runtime.column_resize = TableColumnResizeState::begin(
                        drag.column_id.clone(),
                        start_x,
                        drag.start_width,
                        drag.column_widths_start.clone(),
                    );
                });
                window.prevent_default();
                cx.stop_propagation();
                cx.new(|_| Empty)
            },
        )
        .on_mouse_up(MouseButton::Left, move |event, window, cx| {
            finish_table_column_resize(
                &mouse_up_runtime,
                &mouse_up_config,
                &drag_for_mouse_up,
                ui_px_from_gpui(event.position.x),
                window,
                cx,
            );
        })
        .on_mouse_up_out(MouseButton::Left, move |event, window, cx| {
            finish_table_column_resize(
                &mouse_up_out_runtime,
                &mouse_up_out_config,
                &drag_for_mouse_up_out,
                ui_px_from_gpui(event.position.x),
                window,
                cx,
            );
        })
        .child(
            div()
                .absolute()
                .right(px(0.0))
                .top(px(4.0))
                .bottom(px(4.0))
                .w(px(1.0))
                .bg(rgb(0xc8cdc2)),
        )
}

fn handle_table_column_resize_drag(
    runtime: &Entity<TableRuntime>,
    config: &TableResizeRenderConfig,
    event: &DragMoveEvent<TableColumnResizeDrag>,
    window: &mut Window,
    cx: &mut App,
) {
    let drag = event.drag(cx).clone();
    if drag.table_id != config.table_id {
        return;
    }

    let client_x = ui_px_from_gpui(event.event.position.x);
    let mut committed_change = None;
    runtime.update(cx, |runtime, _| {
        if runtime.column_resize.active_column().is_none() {
            runtime.column_resize = TableColumnResizeState::begin(
                drag.column_id.clone(),
                client_x,
                drag.start_width,
                drag.column_widths_start.clone(),
            );
        }

        let update = drag_table_column_resize(
            drag.mode,
            drag.direction,
            &drag.base_sizing,
            &runtime.column_resize,
            client_x,
        );
        if let Some(sizing) = update.committed_sizing().cloned() {
            committed_change = Some(table_column_sizing_change(&drag, sizing));
        }
        runtime.column_resize = update.state().clone();
    });

    if let (Some(handler), Some(change)) = (&config.on_change, committed_change) {
        handler(change, window, cx);
    }

    window.prevent_default();
    cx.stop_propagation();
    window.refresh();
}

fn finish_table_column_resize(
    runtime: &Entity<TableRuntime>,
    config: &TableResizeRenderConfig,
    drag: &TableColumnResizeDrag,
    client_x: UiPx,
    window: &mut Window,
    cx: &mut App,
) {
    if drag.table_id != config.table_id {
        return;
    }

    let mut committed_change = None;
    let mut handled = false;
    runtime.update(cx, |runtime, _| {
        if !runtime
            .column_resize
            .active_column()
            .is_some_and(|column_id| column_id == &drag.column_id)
        {
            return;
        }
        handled = true;

        let update = end_table_column_resize(
            drag.mode,
            drag.direction,
            &drag.base_sizing,
            &runtime.column_resize,
            Some(client_x),
        );
        if let Some(sizing) = update.committed_sizing().cloned() {
            committed_change = Some(table_column_sizing_change(drag, sizing));
        }
        runtime.column_resize = update.state().clone();
    });

    if !handled {
        return;
    }

    if let (Some(handler), Some(change)) = (&config.on_change, committed_change) {
        handler(change, window, cx);
    }

    window.prevent_default();
    cx.stop_propagation();
    window.refresh();
}

fn table_column_sizing_change(
    drag: &TableColumnResizeDrag,
    sizing: TableColumnSizing,
) -> TableColumnSizingChange {
    let width = sizing.width(&drag.column_id).unwrap_or(drag.start_width);
    TableColumnSizingChange::new(drag.column_id.clone(), width, sizing)
}

fn apply_table_expansion(state: TableState, expansion: TableExpansionState) -> TableState {
    match expansion {
        TableExpansionState::All => state.with_all_rows_expanded(),
        TableExpansionState::Rows(rows) => state.with_expanded_rows(rows),
    }
}

fn toggle_table_expansion(
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

fn render_table_body(
    plan: &TableRenderPlan,
    scroll_viewport_id: String,
    horizontal_scroll_handle: ScrollHandle,
    vertical_scroll_handle: ScrollHandle,
    runtime: Entity<TableRuntime>,
    runtime_snapshot: TableRuntime,
    current_expansion: TableExpansionState,
    selection_policy: TableSelectionPolicy,
    selected_row_ids: Rc<Vec<TableRowId>>,
    on_row_selection_change: Option<TableRowSelectionHandler>,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
) -> impl IntoElement {
    let table_id = plan.table_id().to_owned();
    let metrics = plan.metrics();
    let pinned_layout = plan.pinned_layout().cloned();
    let center_window = if pinned_layout.is_some() {
        plan.center_column_window().cloned().map(Rc::new)
    } else {
        None
    };
    let final_rows = Rc::new(plan.table().final_model().rows().to_vec());
    let top_rows = plan.top_rows().to_vec();
    let center_rows = plan.rows().to_vec();
    let bottom_rows = plan.bottom_rows().to_vec();
    let top_row_count = top_rows.len();
    let center_total_row_count = plan.virtualizer().count();
    let top_height = metrics.row_height() * top_rows.len() as f32;
    let center_height = plan.virtualizer().total_size();
    let bottom_height = metrics.row_height() * bottom_rows.len() as f32;

    div()
        .flex_1()
        .min_h(px(0.0))
        .overflow_hidden()
        .flex()
        .flex_col()
        .when(!top_rows.is_empty(), |this| {
            this.child(render_table_row_band(
                &table_id,
                TableRowRegion::Top,
                metrics,
                top_rows.clone(),
                top_height,
                pinned_layout.clone(),
                center_window.clone(),
                horizontal_scroll_handle.clone(),
                vertical_scroll_handle.clone(),
                runtime.clone(),
                runtime_snapshot.clone(),
                final_rows.clone(),
                top_row_count,
                center_total_row_count,
                current_expansion.clone(),
                selection_policy,
                selected_row_ids.clone(),
                on_row_selection_change.clone(),
                on_row_activate.clone(),
                on_row_expansion_request.clone(),
                on_cell_edit_change.clone(),
            ))
        })
        .child(
            div().flex_1().min_h(px(0.0)).overflow_hidden().child(
                ScrollArea::new(
                    scroll_viewport_id,
                    render_table_row_band(
                        &table_id,
                        TableRowRegion::Center,
                        metrics,
                        center_rows,
                        center_height,
                        pinned_layout.clone(),
                        center_window.clone(),
                        horizontal_scroll_handle.clone(),
                        vertical_scroll_handle.clone(),
                        runtime.clone(),
                        runtime_snapshot.clone(),
                        final_rows.clone(),
                        top_row_count,
                        center_total_row_count,
                        current_expansion.clone(),
                        selection_policy,
                        selected_row_ids.clone(),
                        on_row_selection_change.clone(),
                        on_row_activate.clone(),
                        on_row_expansion_request.clone(),
                        on_cell_edit_change.clone(),
                    ),
                )
                .vertical()
                .scroll_handle(&vertical_scroll_handle)
                .with_size(metrics.size()),
            ),
        )
        .when(!bottom_rows.is_empty(), |this| {
            this.child(render_table_row_band(
                &table_id,
                TableRowRegion::Bottom,
                metrics,
                bottom_rows.clone(),
                bottom_height,
                pinned_layout,
                center_window,
                horizontal_scroll_handle,
                vertical_scroll_handle,
                runtime,
                runtime_snapshot,
                final_rows,
                top_row_count,
                center_total_row_count,
                current_expansion,
                selection_policy,
                selected_row_ids,
                on_row_selection_change,
                on_row_activate,
                on_row_expansion_request,
                on_cell_edit_change,
            ))
        })
}

fn render_table_row_band(
    table_id: &str,
    region: TableRowRegion,
    metrics: TableMetrics,
    rows: Vec<TableRowRenderPlan>,
    height: UiPx,
    pinned_layout: Option<TablePinnedLayoutPlan>,
    center_window: Option<Rc<TableCenterColumnWindowPlan>>,
    horizontal_scroll_handle: ScrollHandle,
    vertical_scroll_handle: ScrollHandle,
    runtime: Entity<TableRuntime>,
    runtime_snapshot: TableRuntime,
    final_rows: Rc<Vec<TableResolvedRow>>,
    top_row_count: usize,
    center_total_row_count: usize,
    current_expansion: TableExpansionState,
    selection_policy: TableSelectionPolicy,
    selected_row_ids: Rc<Vec<TableRowId>>,
    on_row_selection_change: Option<TableRowSelectionHandler>,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
) -> AnyElement {
    let table_id = table_id.to_owned();
    let region_name = region.as_str();
    div()
        .id(format!("table:{table_id}:body:{region_name}"))
        .debug_selector({
            let table_id = table_id.clone();
            move || format!("table:{table_id}:body:{region_name}")
        })
        .relative()
        .w_full()
        .h(gpui_px_from_ui(height))
        .flex_none()
        .children(rows.into_iter().map(move |row| {
            let table_id = table_id.clone();
            let center_window = center_window.clone();
            let focus_handle = runtime_snapshot.focus_handles.get(row.id()).cloned();
            let focused = runtime_snapshot.focused_row.as_ref() == Some(row.id());
            render_table_row(
                table_id,
                row,
                metrics,
                pinned_layout.clone(),
                center_window,
                horizontal_scroll_handle.clone(),
                vertical_scroll_handle.clone(),
                runtime.clone(),
                focus_handle,
                focused,
                final_rows.clone(),
                top_row_count,
                center_total_row_count,
                current_expansion.clone(),
                selection_policy,
                selected_row_ids.clone(),
                on_row_selection_change.clone(),
                on_row_activate.clone(),
                on_row_expansion_request.clone(),
                on_cell_edit_change.clone(),
            )
        }))
        .into_any_element()
}

fn render_table_row(
    table_id: String,
    row: TableRowRenderPlan,
    metrics: TableMetrics,
    pinned_layout: Option<TablePinnedLayoutPlan>,
    center_window: Option<Rc<TableCenterColumnWindowPlan>>,
    horizontal_scroll_handle: ScrollHandle,
    vertical_scroll_handle: ScrollHandle,
    runtime: Entity<TableRuntime>,
    focus_handle: Option<FocusHandle>,
    focused: bool,
    final_rows: Rc<Vec<TableResolvedRow>>,
    top_row_count: usize,
    center_total_row_count: usize,
    current_expansion: TableExpansionState,
    selection_policy: TableSelectionPolicy,
    selected_row_ids: Rc<Vec<TableRowId>>,
    on_row_selection_change: Option<TableRowSelectionHandler>,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
) -> impl IntoElement {
    let render_key = row.render_key().to_owned();
    let row_id = row.id().clone();
    let row_for_click = row.clone();
    let row_for_key = row.clone();
    let tree = row.row().tree().cloned();
    let tree_depth = tree.as_ref().map(TableTreeRow::depth).unwrap_or(0);
    let tree_branch = row.row().is_tree_branch();
    let tree_expanded = row.row().tree_expanded().unwrap_or(false);
    let row_background = if row.row().is_group() {
        rgb(0xf1f4f8)
    } else if row.selected() {
        rgb(0xe7f0ff)
    } else if row.model_index().is_multiple_of(2) {
        rgb(0xffffff)
    } else {
        rgb(0xf8f9f3)
    };
    let region_cells = TableColumnRegion::ALL
        .into_iter()
        .map(|region| {
            let source_cells = row.cells_for_region(region).cloned().collect::<Vec<_>>();
            let active_center_window = (region == TableColumnRegion::Center)
                .then_some(center_window.as_deref())
                .flatten();
            let cells = table_row_region_cells_for_window(&source_cells, active_center_window);
            let region_width = active_center_window
                .map(TableCenterColumnWindowPlan::center_width)
                .unwrap_or_else(|| {
                    source_cells
                        .iter()
                        .fold(UiPx::ZERO, |total, cell| total + cell.width())
                });
            let leading_spacer_width = active_center_window
                .map(TableCenterColumnWindowPlan::leading_spacer_width)
                .unwrap_or(UiPx::ZERO);
            let trailing_spacer_width = active_center_window
                .map(TableCenterColumnWindowPlan::trailing_spacer_width)
                .unwrap_or(UiPx::ZERO);
            (
                region,
                region_width,
                cells,
                !source_cells.is_empty(),
                leading_spacer_width,
                trailing_spacer_width,
                active_center_window.is_some(),
            )
        })
        .collect::<Vec<_>>();
    let tree_affordance_column_id = tree.as_ref().and_then(|_| {
        region_cells.iter().find_map(|(_, _, cells, _, _, _, _)| {
            cells.first().map(|cell| cell.column_id().clone())
        })
    });

    div()
        .id(format!("table:{table_id}:row:{render_key}"))
        .debug_selector({
            let table_id = table_id.clone();
            let render_key = render_key.clone();
            move || format!("table:{table_id}:row:{render_key}")
        })
        .absolute()
        .top(gpui_px_from_ui(row.virtual_start()))
        .left(px(0.0))
        .right(px(0.0))
        .h(gpui_px_from_ui(row.virtual_size()))
        .min_w(px(0.0))
        .flex()
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(rgb(0xe2e4dc))
        .bg(row_background)
        .hover(|this| this.bg(rgb(0xeef2f7)))
        .ui_role(row.role())
        .aria_row_index(row.aria_row_index())
        .aria_selected(row.selected())
        .when(tree_branch, |this| this.aria_expanded(tree_expanded))
        .focusable()
        .tab_stop(focused)
        .when_some(focus_handle.clone(), |this, focus_handle| {
            this.track_focus(&focus_handle)
        })
        .focus_visible(|style| style.border_color(rgb(0x2f80ed)))
        .when(!tree_branch || on_row_activate.is_some(), |this| {
            this.cursor_pointer()
        })
        .on_click({
            let runtime = runtime.clone();
            let focus_handle = focus_handle.clone();
            let selection_policy = selection_policy;
            let selected_row_ids = selected_row_ids.clone();
            let on_row_selection_change = on_row_selection_change.clone();
            let on_row_activate = on_row_activate.clone();
            move |event: &ClickEvent, window, cx| {
                if !event.standard_click() {
                    return;
                }

                cx.stop_propagation();
                window.prevent_default();

                let action = TableRowAction::from_render_plan(
                    &row_for_click,
                    TableInputModifiers::from_gpui(event.modifiers()),
                );
                if selection_policy.activation_mode().is_row_click() {
                    request_table_row_selection_change(
                        &runtime,
                        &action,
                        selection_policy,
                        TableSelectionScope::Row,
                        selected_row_ids.clone(),
                        on_row_selection_change.clone(),
                        window,
                        cx,
                    );
                }

                let activation_kind = if event.click_count() >= 2 {
                    TableRowActivationKind::DoubleClick
                } else {
                    TableRowActivationKind::Click
                };
                runtime.update(cx, |runtime, cx| {
                    runtime.set_focused(row_id.clone(), cx);
                });
                if let Some(focus_handle) = focus_handle.as_ref() {
                    focus_handle.focus(window, cx);
                }
                if let Some(on_row_activate) = on_row_activate.as_ref() {
                    on_row_activate(TableRowActivation::new(action, activation_kind), window, cx);
                }
            }
        })
        .on_key_down({
            let runtime = runtime.clone();
            let on_row_activate = on_row_activate.clone();
            let on_row_expansion_request = on_row_expansion_request.clone();
            let current_expansion_for_key = current_expansion.clone();
            move |event: &KeyDownEvent, window, cx| {
                handle_table_row_key_down(
                    &row_for_key,
                    final_rows.as_ref(),
                    vertical_scroll_handle.clone(),
                    top_row_count,
                    center_total_row_count,
                    &runtime,
                    current_expansion_for_key.clone(),
                    on_row_activate.clone(),
                    on_row_expansion_request.clone(),
                    event,
                    window,
                    cx,
                );
            }
        })
        .children(region_cells.into_iter().map(
            move |(
                region,
                region_width,
                cells,
                has_source_cells,
                leading_spacer_width,
                trailing_spacer_width,
                uses_center_window,
            )| {
                let table_id = table_id.clone();
                let render_key = render_key.clone();
                let region_name = region.as_str().to_owned();
                let center_scroll_id = pinned_layout.as_ref().and_then(|layout| {
                    (region == TableColumnRegion::Center && has_source_cells)
                        .then(|| layout.row_center_scroll_id(&render_key))
                });
                let mut region_children =
                    Vec::with_capacity(cells.len() + usize::from(uses_center_window) * 2);
                if uses_center_window {
                    region_children.push(render_table_lane_spacer(leading_spacer_width));
                }
                let current_expansion_for_cells = current_expansion.clone();
                region_children.extend(cells.into_iter().map({
                    let table_id = table_id.clone();
                    let render_key = render_key.clone();
                    let row = row.clone();
                    let runtime = runtime.clone();
                    let focus_handle = focus_handle.clone();
                    let on_row_expansion_request = on_row_expansion_request.clone();
                    let on_cell_edit_change = on_cell_edit_change.clone();
                    let tree = tree.clone();
                    let tree_affordance_column_id = tree_affordance_column_id.clone();
                    move |cell| {
                        let tree_affordance = tree_affordance_column_id
                            .as_ref()
                            .is_some_and(|column_id| cell.column_id() == column_id);
                        render_table_body_cell(
                            table_id.clone(),
                            render_key.clone(),
                            metrics,
                            cell,
                            row.clone(),
                            tree.clone(),
                            tree_depth,
                            tree_branch,
                            tree_expanded,
                            tree_affordance,
                            runtime.clone(),
                            focus_handle.clone(),
                            current_expansion_for_cells.clone(),
                            on_row_expansion_request.clone(),
                            on_cell_edit_change.clone(),
                        )
                        .into_any_element()
                    }
                }));
                if uses_center_window {
                    region_children.push(render_table_lane_spacer(trailing_spacer_width));
                }

                let region_lane = div()
                    .h_full()
                    .min_w(px(0.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .id(format!(
                        "table:{table_id}:row-region:{render_key}:{region_name}"
                    ))
                    .debug_selector({
                        let table_id = table_id.clone();
                        let render_key = render_key.clone();
                        let region_name = region_name.clone();
                        move || format!("table:{table_id}:row-region:{render_key}:{region_name}")
                    })
                    .w(gpui_px_from_ui(region_width))
                    .flex_none()
                    .children(region_children)
                    .into_any_element();

                if let Some(center_scroll_id) = center_scroll_id {
                    div()
                        .h_full()
                        .min_w(px(0.0))
                        .flex_1()
                        .child(
                            ScrollArea::new(center_scroll_id, region_lane)
                                .horizontal()
                                .scroll_handle(&horizontal_scroll_handle)
                                .with_size(metrics.size()),
                        )
                        .into_any_element()
                } else {
                    region_lane
                }
            },
        ))
}

fn table_row_region_cells_for_window(
    source_cells: &[TableCellRenderPlan],
    center_window: Option<&TableCenterColumnWindowPlan>,
) -> Vec<TableCellRenderPlan> {
    let Some(center_window) = center_window else {
        return source_cells.to_vec();
    };

    let cells_by_column = source_cells
        .iter()
        .map(|cell| (cell.column_id(), cell))
        .collect::<std::collections::BTreeMap<_, _>>();

    center_window
        .rendered_columns()
        .iter()
        .filter_map(|column| {
            cells_by_column
                .get(column.id())
                .map(|cell| (**cell).clone())
        })
        .collect()
}

fn render_table_body_cell(
    table_id: String,
    render_key: String,
    metrics: TableMetrics,
    cell: TableCellRenderPlan,
    row: TableRowRenderPlan,
    tree: Option<TableTreeRow>,
    tree_depth: usize,
    tree_branch: bool,
    tree_expanded: bool,
    tree_affordance: bool,
    runtime: Entity<TableRuntime>,
    focus_handle: Option<FocusHandle>,
    current_expansion: TableExpansionState,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    on_cell_edit_change: Option<TableCellEditHandler>,
) -> impl IntoElement {
    let column_id = cell.column_id().as_str().to_owned();
    let show_tree_affordance = tree_affordance && tree.is_some();
    let indent = ui_px(16.0) * tree_depth as f32;
    let mut content = Vec::new();
    if show_tree_affordance {
        content.push(
            div()
                .w(gpui_px_from_ui(indent))
                .h_full()
                .flex_none()
                .into_any_element(),
        );
        content.push(render_table_tree_toggle(
            table_id.clone(),
            render_key.clone(),
            row.clone(),
            tree_branch,
            tree_expanded,
            runtime,
            focus_handle,
            current_expansion,
            on_row_expansion_request,
        ));
    }
    let cell_text = cell.text().to_owned();
    if cell.text_editable() && on_cell_edit_change.is_some() {
        let action = TableRowAction::from_render_plan(&row, TableInputModifiers::default());
        let column_id_for_change = cell.column_id().clone();
        let previous_text = cell_text.clone();
        let on_change = on_cell_edit_change.clone();
        content.push(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .on_mouse_up(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .child(
                    TextInput::new(
                        format!("table:{table_id}:cell:{render_key}:{column_id}:editor"),
                        format!("Edit {column_id} for row {}", row.id().as_str()),
                    )
                    .value(cell_text)
                    .on_change(move |next_text, window, cx| {
                        if let Some(on_change) = on_change.as_ref() {
                            on_change(
                                TableCellEditChange::new(
                                    action.clone(),
                                    column_id_for_change.clone(),
                                    previous_text.clone(),
                                    next_text,
                                ),
                                window,
                                cx,
                            );
                        }
                    })
                    .with_size(metrics.size()),
                )
                .into_any_element(),
        );
    } else {
        content.push(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .truncate()
                .whitespace_nowrap()
                .child(cell_text)
                .into_any_element(),
        );
    }

    div()
        .id(format!("table:{table_id}:cell:{render_key}:{column_id}"))
        .debug_selector(move || format!("table:{table_id}:cell:{render_key}:{column_id}"))
        .w(gpui_px_from_ui(cell.width()))
        .flex_none()
        .h_full()
        .flex()
        .items_center()
        .px(gpui_px_from_ui(metrics.cell_padding_x()))
        .border_r_1()
        .border_color(rgb(0xe7e9e1))
        .truncate()
        .whitespace_nowrap()
        .text_xs()
        .text_color(rgb(0x2f3845))
        .ui_role(cell.role())
        .aria_column_index(cell.aria_column_index())
        .children(content)
}

fn render_table_tree_toggle(
    table_id: String,
    render_key: String,
    row: TableRowRenderPlan,
    tree_branch: bool,
    tree_expanded: bool,
    runtime: Entity<TableRuntime>,
    focus_handle: Option<FocusHandle>,
    current_expansion: TableExpansionState,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
) -> AnyElement {
    if !tree_branch {
        return div().w(px(18.0)).h(px(18.0)).flex_none().into_any_element();
    }

    let row_id = row.id().clone();
    let row_key = render_key.clone();
    let children_load_state = row
        .children_load_state()
        .cloned()
        .unwrap_or_else(TableRowChildrenLoadState::idle);
    let glyph = match &children_load_state {
        TableRowChildrenLoadState::Loading { .. } => "...",
        TableRowChildrenLoadState::Failed { .. } => "!",
        TableRowChildrenLoadState::Idle if tree_expanded => "v",
        TableRowChildrenLoadState::Idle => ">",
    };
    let aria_label = match &children_load_state {
        TableRowChildrenLoadState::Loading { .. } => {
            format!("Loading children for row {}", row.id().as_str())
        }
        TableRowChildrenLoadState::Failed { .. } => {
            format!("Retry loading row {}", row.id().as_str())
        }
        TableRowChildrenLoadState::Idle if tree_expanded => {
            format!("Collapse row {}", row.id().as_str())
        }
        TableRowChildrenLoadState::Idle => format!("Expand row {}", row.id().as_str()),
    };

    div()
        .id(format!("table:{table_id}:tree-toggle:{render_key}"))
        .debug_selector({
            let table_id = table_id.clone();
            let row_key = row_key.clone();
            move || format!("table:{table_id}:tree-toggle:{row_key}")
        })
        .w(px(18.0))
        .h(px(18.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .text_xs()
        .ui_role(Role::Button)
        .aria_label(aria_label)
        .aria_expanded(tree_expanded)
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0xe8ede6)))
        .on_click(move |event: &ClickEvent, window, cx| {
            if !event.standard_click() {
                return;
            }

            cx.stop_propagation();
            window.prevent_default();

            let next_expansion =
                toggle_table_expansion(current_expansion.clone(), row_id.clone(), !tree_expanded);
            runtime.update(cx, |runtime, cx| {
                runtime.set_focused(row_id.clone(), cx);
                runtime.set_expansion_override(next_expansion.clone(), cx);
            });
            if let Some(focus_handle) = focus_handle.as_ref() {
                focus_handle.focus(window, cx);
            }
            if let Some(on_row_expansion_request) = on_row_expansion_request.as_ref() {
                let action = TableRowAction::from_render_plan(
                    &row,
                    TableInputModifiers::from_gpui(event.modifiers()),
                );
                on_row_expansion_request(
                    TableRowExpansionToggle::new(action, !tree_expanded),
                    window,
                    cx,
                );
            }
        })
        .child(glyph)
        .into_any_element()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TableRowKeyboardAction {
    Focus { index: usize, row_id: TableRowId },
    Toggle { expanded: bool },
    Activate,
}

fn table_row_keyboard_action(
    row: &TableRowRenderPlan,
    final_rows: &[TableResolvedRow],
    key: &str,
) -> Option<TableRowKeyboardAction> {
    let current_index = row.model_index();
    match key {
        "home" if !final_rows.is_empty() => Some(TableRowKeyboardAction::Focus {
            index: 0,
            row_id: final_rows[0].id().clone(),
        }),
        "end" if !final_rows.is_empty() => {
            let index = final_rows.len() - 1;
            Some(TableRowKeyboardAction::Focus {
                index,
                row_id: final_rows[index].id().clone(),
            })
        }
        "up" => current_index.checked_sub(1).and_then(|index| {
            final_rows
                .get(index)
                .map(|target| TableRowKeyboardAction::Focus {
                    index,
                    row_id: target.id().clone(),
                })
        }),
        "down" => {
            let index = current_index + 1;
            final_rows
                .get(index)
                .map(|target| TableRowKeyboardAction::Focus {
                    index,
                    row_id: target.id().clone(),
                })
        }
        "left" if row.row().is_tree_branch() && row.row().tree_expanded() == Some(true) => {
            Some(TableRowKeyboardAction::Toggle { expanded: false })
        }
        "left" => row.row().parent_id().and_then(|parent_id| {
            final_rows
                .iter()
                .position(|candidate| candidate.id() == parent_id)
                .map(|index| TableRowKeyboardAction::Focus {
                    index,
                    row_id: parent_id.clone(),
                })
        }),
        "right" if row.row().is_tree_branch() && row.row().tree_expanded() == Some(false) => {
            Some(TableRowKeyboardAction::Toggle { expanded: true })
        }
        "right" => final_rows
            .get(current_index + 1)
            .filter(|candidate| candidate.parent_id() == Some(row.id()))
            .map(|target| TableRowKeyboardAction::Focus {
                index: current_index + 1,
                row_id: target.id().clone(),
            }),
        "enter" | "space" => Some(TableRowKeyboardAction::Activate),
        _ => None,
    }
}

fn handle_table_row_key_down(
    row: &TableRowRenderPlan,
    final_rows: &[TableResolvedRow],
    vertical_scroll_handle: ScrollHandle,
    top_row_count: usize,
    center_total_row_count: usize,
    runtime: &Entity<TableRuntime>,
    current_expansion: TableExpansionState,
    on_row_activate: Option<TableRowActivationHandler>,
    on_row_expansion_request: Option<TableRowExpansionHandler>,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if event.keystroke.modifiers.modified() {
        return;
    }

    let Some(action) = table_row_keyboard_action(row, final_rows, event.keystroke.key.as_str())
    else {
        return;
    };

    cx.stop_propagation();
    window.prevent_default();

    match action {
        TableRowKeyboardAction::Focus { index, row_id } => {
            let focus_handle = runtime.update(cx, |runtime, cx| runtime.set_focused(row_id, cx));
            if let Some(center_index) = index.checked_sub(top_row_count) {
                if center_index < center_total_row_count {
                    scroll_table_row_into_view(
                        &vertical_scroll_handle,
                        row.virtual_size(),
                        center_total_row_count,
                        center_index,
                    );
                }
            }
            if let Some(focus_handle) = focus_handle {
                focus_handle.focus(window, cx);
            }
            window.refresh();
        }
        TableRowKeyboardAction::Toggle { expanded } => {
            let next_expansion =
                toggle_table_expansion(current_expansion, row.id().clone(), expanded);
            runtime.update(cx, |runtime, cx| {
                runtime.set_focused(row.id().clone(), cx);
                runtime.set_expansion_override(next_expansion.clone(), cx);
            });
            if let Some(on_row_expansion_request) = on_row_expansion_request.as_ref() {
                let action = TableRowAction::from_render_plan(
                    row,
                    TableInputModifiers::from_gpui(event.keystroke.modifiers),
                );
                on_row_expansion_request(
                    TableRowExpansionToggle::new(action, expanded),
                    window,
                    cx,
                );
            }
            window.refresh();
        }
        TableRowKeyboardAction::Activate => {
            runtime.update(cx, |runtime, cx| {
                runtime.set_focused(row.id().clone(), cx);
            });
            if let Some(on_row_activate) = on_row_activate.as_ref() {
                let action = TableRowAction::from_render_plan(
                    row,
                    TableInputModifiers::from_gpui(event.keystroke.modifiers),
                );
                on_row_activate(
                    TableRowActivation::new(action, TableRowActivationKind::Keyboard),
                    window,
                    cx,
                );
            }
            window.refresh();
        }
    }
}

fn scroll_table_row_into_view(
    scroll_handle: &ScrollHandle,
    row_height: UiPx,
    row_count: usize,
    index: usize,
) {
    let viewport_extent = ui_px_from_gpui(scroll_handle.bounds().size.height);
    let row_height = nonnegative_px(row_height);
    if viewport_extent.as_f32() <= 0.0 || row_height.as_f32() <= 0.0 {
        return;
    }

    let total_extent = row_height * row_count as f32;
    let current_scroll_offset =
        UiPx::new((-ui_px_from_gpui(scroll_handle.offset().y).as_f32()).max(0.0));
    let row_start = row_height * index as f32;
    let row_end = row_start + row_height;
    let max_scroll = nonnegative_px(total_extent - viewport_extent);
    let target = if row_start < current_scroll_offset {
        row_start
    } else if row_end > current_scroll_offset + viewport_extent {
        row_end - viewport_extent
    } else {
        current_scroll_offset
    };
    let target = target.max(UiPx::ZERO).min(max_scroll);

    scroll_handle.set_offset(point(px(0.0), -gpui_px_from_ui(target)));
}

fn render_table_lane_spacer(width: UiPx) -> AnyElement {
    div()
        .w(gpui_px_from_ui(width))
        .min_w(px(0.0))
        .flex_none()
        .h_full()
        .min_h(px(0.0))
        .into_any_element()
}

fn row_render_key(
    row: &TableResolvedRow,
    duplicate_row_ids: &BTreeSet<open_gpui_ui_core::TableRowId>,
) -> String {
    if duplicate_row_ids.contains(row.id())
        && let Some(source_index) = row.source_index()
    {
        format!("{}:{}", source_index, row.id().as_str())
    } else {
        row.id().as_str().to_owned()
    }
}

fn handle_table_vertical_scroll_wheel(
    scroll_handle: &ScrollHandle,
    event: &ScrollWheelEvent,
    window: &mut Window,
) {
    let delta = event.delta.pixel_delta(px(16.0));
    if delta.y.abs() <= delta.x.abs() {
        return;
    }

    let current = scroll_handle.offset();
    let max_offset_y = scroll_handle.max_offset().y;
    let next_y = (current.y + delta.y).clamp(-max_offset_y, px(0.0));

    if next_y != current.y {
        scroll_handle.set_offset(point(current.x, next_y));
        window.refresh();
    }
}

const fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        UiPx::ZERO
    } else {
        value
    }
}
