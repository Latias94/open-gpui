//! Table component backed by renderer-neutral row-model and virtualizer contracts.

use crate::a11y::UiA11yElementExt;
use crate::button::{Button, ButtonVariant};
use crate::checkbox::Checkbox;
use crate::color::ColorIntent;
use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use crate::listbox::ListboxOption;
use crate::popover::{Popover, PopoverState};
use crate::scroll_area::ScrollArea;
use crate::select::{Select, SelectState};
use crate::text_input::{TextInput, TextInputState};
use crate::textarea::Textarea;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, Context, CursorStyle, DragMoveEvent, Empty, Entity, FocusHandle,
    Font, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, Modifiers, MouseButton,
    ParentElement, Pixels, RenderOnce, ScrollHandle, ScrollWheelEvent, SharedString,
    StatefulInteractiveElement, Styled, TextRun, Window, div, point, px, rems, rgb, rgba,
};
use open_gpui_ui_core::{
    FocusRestoreIntent, GridViewport2D, InitialFocusIntent, OutsidePressPolicy,
    OverlayPlacementAlignment, OverlayPlacementSide, Role, Sizable, Size, TableCellEditor,
    TableCellValue, TableColumn, TableColumnFacets, TableColumnId, TableColumnRegion,
    TableColumnResizeDirection, TableColumnResizeMode, TableColumnResizeState, TableColumnSizing,
    TableColumnVisibilityOverrides, TableColumnWidthPolicy, TableExpansionMode,
    TableExpansionState, TableFacetRange, TableFilter, TableGlobalFacetSummary,
    TableNumericFilterOperator, TableResolvedColumnSizing, TableResolvedRow, TableResolvedState,
    TableRowChildrenLoadState, TableRowId, TableRowRegion, TableSelectOption, TableSelectionMode,
    TableSelectionPolicy, TableSelectionSummary, TableSort, TableSortDirection, TableStageMode,
    TableState, TableStateCacheKey, TableTextFilterOperator, TableTreeRow, ThemeTokens, Toggled,
    UiPx, VirtualizerItemKey, VirtualizerItemMeasurement, VirtualizerRange,
    VirtualizerResolvedState, VirtualizerSnapshot, VirtualizerState, drag_table_column_resize,
    end_table_column_resize, ui_px,
};
pub use open_gpui_ui_core::{
    TableResolvedHeaderCell, TableResolvedHeaderGroup, TableResolvedHeaderGroupRegions,
    TableResolvedHeaderKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

type TableSortHandler = Rc<dyn Fn(TableHeaderAction, &mut Window, &mut App)>;
type TableColumnSizingHandler = Rc<dyn Fn(TableColumnSizingChange, &mut Window, &mut App)>;
type TableColumnOrderHandler = Rc<dyn Fn(TableColumnOrderChange, &mut Window, &mut App)>;
type TableRowActivationHandler = Rc<dyn Fn(TableRowActivation, &mut Window, &mut App)>;
type TableRowExpansionHandler = Rc<dyn Fn(TableRowExpansionToggle, &mut Window, &mut App)>;
type TableRowSelectionHandler = Rc<dyn Fn(TableRowSelectionChange, &mut Window, &mut App)>;
type TableFacetedFilterChangeHandler = Rc<dyn Fn(TableFacetedFilterChange, &mut Window, &mut App)>;
type TableRangeFilterChangeHandler = Rc<dyn Fn(TableRangeFilterChange, &mut Window, &mut App)>;
type TableColumnVisibilityChangeHandler =
    Rc<dyn Fn(TableColumnVisibilityChange, &mut Window, &mut App)>;
type TableGlobalFilterChangeHandler = Rc<dyn Fn(TableGlobalFilterChange, &mut Window, &mut App)>;
type TablePredicateFilterChangeHandler =
    Rc<dyn Fn(TablePredicateFilterChange, &mut Window, &mut App)>;
type TableCellEditHandler = Rc<dyn Fn(TableCellEditChange, &mut Window, &mut App)>;

#[derive(Debug, Clone, PartialEq)]
struct TableContentFitMeasureKey {
    state_key: TableStateCacheKey,
    font: Font,
    font_size: Pixels,
    cell_padding_x: UiPx,
    sample_set: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct TableContentFitMeasureCache {
    key: Option<TableContentFitMeasureKey>,
    widths: BTreeMap<TableColumnId, UiPx>,
}

impl TableContentFitMeasureCache {
    fn widths_for(
        &mut self,
        key: TableContentFitMeasureKey,
        columns: &[TableColumnRenderPlan],
        rendered_rows: &[&TableResolvedRow],
        metrics: TableMetrics,
        window: &Window,
    ) -> &BTreeMap<TableColumnId, UiPx> {
        let needs_refresh = self.key.as_ref() != Some(&key);
        if needs_refresh {
            let measured =
                measure_table_content_fit_widths(columns, rendered_rows, metrics, window);
            for (column_id, width) in measured {
                self.widths
                    .entry(column_id)
                    .and_modify(|existing| *existing = (*existing).max(width))
                    .or_insert(width);
            }
            self.key = Some(key);
        }

        &self.widths
    }
}

fn measure_table_content_fit_widths(
    columns: &[TableColumnRenderPlan],
    rendered_rows: &[&TableResolvedRow],
    metrics: TableMetrics,
    window: &Window,
) -> BTreeMap<TableColumnId, UiPx> {
    let mut widths = BTreeMap::new();
    let font = window.text_style().font();
    let font_size = table_content_fit_text_size(window);
    let padding_x = metrics.cell_padding_x();
    let tree_affordance_column_id = columns.first().map(|column| column.id().clone());

    for column in columns
        .iter()
        .filter(|column| column.width_policy() == TableColumnWidthPolicy::ContentFit)
    {
        let mut measured = measure_table_header_text_width(
            window,
            column.label(),
            column.sort_direction(),
            font.clone(),
            font_size,
        );
        for row in rendered_rows {
            if let Some(value) = row.cell(column.id()) {
                let value_text = value.filter_text();
                let mut value_width =
                    measure_table_text_width(window, &value_text, font.clone(), font_size);
                if tree_affordance_column_id
                    .as_ref()
                    .is_some_and(|tree_column_id| {
                        tree_column_id == column.id() && row.tree().is_some()
                    })
                {
                    value_width = value_width + ui_px(18.0) + ui_px(16.0) * row.depth() as f32;
                }
                measured = measured.max(value_width);
            }
        }

        let measured = measured + padding_x * 2.0;
        widths.insert(column.id().clone(), measured);
    }

    widths
}

fn measure_table_header_text_width(
    window: &Window,
    label: &str,
    sort_direction: Option<TableSortDirection>,
    font: Font,
    font_size: Pixels,
) -> UiPx {
    let mut text = label.to_owned();
    if let Some(direction) = sort_direction {
        text.push_str(match direction {
            TableSortDirection::Ascending => " ↑",
            TableSortDirection::Descending => " ↓",
        });
    }

    measure_table_text_width(window, &text, font.bold(), font_size)
}

fn content_fit_measure_key(
    state_key: TableStateCacheKey,
    metrics: TableMetrics,
    columns: &[TableColumnRenderPlan],
    rendered_rows: &[&TableResolvedRow],
    window: &Window,
) -> TableContentFitMeasureKey {
    let mut sample_set = Vec::new();
    let font = window.text_style().font();
    let font_size = table_content_fit_text_size(window);
    sample_set.push(format!("size:{}", metrics.size().as_str()));
    sample_set.extend(
        columns
            .iter()
            .filter(|column| column.width_policy() == TableColumnWidthPolicy::ContentFit)
            .map(|column| format!("column:{}", column.id().as_str())),
    );
    sample_set.extend(rendered_rows.iter().flat_map(|row| {
        let row_key = table_content_fit_row_sample_key(row);
        let row_depth = row.depth();
        let row_has_tree = row.tree().is_some();
        columns
            .iter()
            .filter(|column| column.width_policy() == TableColumnWidthPolicy::ContentFit)
            .map(move |column| {
                let value = row
                    .cell(column.id())
                    .map(TableCellValue::filter_text)
                    .unwrap_or_default();
                format!(
                    "row:{row_key}|depth:{row_depth}|tree:{row_has_tree}|column:{}|value:{}",
                    column.id().as_str(),
                    value
                )
            })
    }));

    TableContentFitMeasureKey {
        state_key,
        font,
        font_size,
        cell_padding_x: metrics.cell_padding_x(),
        sample_set,
    }
}

fn table_content_fit_rendered_rows<'a>(
    table: &'a TableResolvedState,
    virtualizer: &'a VirtualizerResolvedState,
) -> Vec<&'a TableResolvedRow> {
    let mut rows = Vec::with_capacity(
        table.top_rows().len() + virtualizer.items().len() + table.bottom_rows().len(),
    );
    rows.extend(table.top_rows());
    rows.extend(
        virtualizer
            .items()
            .iter()
            .filter_map(|measurement| table.center_rows().get(measurement.index())),
    );
    rows.extend(table.bottom_rows());
    rows
}

fn measure_table_text_width(window: &Window, text: &str, font: Font, font_size: Pixels) -> UiPx {
    if text.is_empty() {
        return UiPx::ZERO;
    }

    let shaped = window.text_system().shape_line(
        text.to_owned().into(),
        font_size,
        &[TextRun {
            len: text.len(),
            font,
            color: window.text_style().color,
            background_color: None,
            underline: None,
            strikethrough: None,
        }],
        None,
    );
    ui_px(shaped.width().as_f32())
}

fn table_content_fit_text_size(window: &Window) -> Pixels {
    rems(0.75).to_pixels(window.rem_size())
}

fn table_content_fit_row_sample_key(row: &TableResolvedRow) -> String {
    row.source_index()
        .map(|index| format!("{index}:{}", row.id().as_str()))
        .unwrap_or_else(|| row.id().as_str().to_owned())
}

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

/// Controlled payload emitted when a table numeric range filter changes.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRangeFilterChange {
    column_id: TableColumnId,
    min_text: String,
    max_text: String,
    cleared: bool,
}

impl TableRangeFilterChange {
    /// Creates a range-change payload from the current endpoint text.
    pub fn new(
        column_id: impl Into<TableColumnId>,
        min_text: impl Into<String>,
        max_text: impl Into<String>,
    ) -> Self {
        Self {
            column_id: column_id.into(),
            min_text: min_text.into(),
            max_text: max_text.into(),
            cleared: false,
        }
    }

    /// Creates a payload that clears this column's numeric range filter.
    pub fn clear(column_id: impl Into<TableColumnId>) -> Self {
        Self {
            column_id: column_id.into(),
            min_text: String::new(),
            max_text: String::new(),
            cleared: true,
        }
    }

    /// Returns the filtered column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the lower endpoint text exactly as entered.
    pub fn min_text(&self) -> &str {
        &self.min_text
    }

    /// Returns the upper endpoint text exactly as entered.
    pub fn max_text(&self) -> &str {
        &self.max_text
    }

    /// Returns the parsed lower endpoint after normalization.
    pub fn min_value(&self) -> Option<f64> {
        self.normalized_values().0
    }

    /// Returns the parsed upper endpoint after normalization.
    pub fn max_value(&self) -> Option<f64> {
        self.normalized_values().1
    }

    /// Returns true when this payload was created by a clear action.
    pub const fn cleared(&self) -> bool {
        self.cleared
    }

    /// Returns true when the payload carries at least one finite numeric endpoint.
    pub fn active(&self) -> bool {
        let (min, max) = self.normalized_values();
        min.is_some() || max.is_some()
    }

    /// Returns the next column-filter list while preserving unrelated filters.
    pub fn next_filters(&self, filters: impl IntoIterator<Item = TableFilter>) -> Vec<TableFilter> {
        table_range_filter_next_filters(
            filters,
            &self.column_id,
            self.min_value(),
            self.max_value(),
        )
    }

    /// Applies this range change to a table state and resets pagination to the first page.
    pub fn apply_to(&self, state: TableState) -> TableState {
        let next_filters = if self.cleared {
            table_range_filter_next_filters(
                state.filters().iter().cloned(),
                &self.column_id,
                None,
                None,
            )
        } else {
            self.next_filters(state.filters().iter().cloned())
        };
        let next_pagination = state.pagination().with_page_index(0);

        state
            .with_filters(next_filters)
            .with_pagination(next_pagination)
    }

    fn normalized_values(&self) -> (Option<f64>, Option<f64>) {
        normalize_table_range_filter_values(
            parse_table_range_filter_bound(&self.min_text),
            parse_table_range_filter_bound(&self.max_text),
        )
    }
}

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

/// Controlled payload emitted when a table global text filter changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableGlobalFilterChange {
    query: String,
    cleared: bool,
}

impl TableGlobalFilterChange {
    /// Creates a global-filter payload from the current query text.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            cleared: false,
        }
    }

    /// Creates a payload that clears the table global filter.
    pub fn clear() -> Self {
        Self {
            query: String::new(),
            cleared: true,
        }
    }

    /// Returns the query text exactly as entered by the user.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns true when this payload was created by a clear action.
    pub const fn cleared(&self) -> bool {
        self.cleared
    }

    /// Returns whether this payload carries a non-empty global query after trimming.
    pub fn active(&self) -> bool {
        !self.cleared && !self.query.trim().is_empty()
    }

    /// Applies this global-filter change to a table state and resets pagination to the first page.
    pub fn apply_to(&self, state: TableState) -> TableState {
        let next_pagination = state.pagination().with_page_index(0);
        let state = if self.active() {
            state.with_global_filter(self.query.clone())
        } else {
            state.without_global_filter()
        };

        state.with_pagination(next_pagination)
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

/// Controlled payload emitted when an editable table cell changes.
#[derive(Debug, Clone, PartialEq)]
pub struct TableCellEditChange {
    action: TableRowAction,
    column_id: TableColumnId,
    previous_value: TableCellValue,
    next_value: TableCellValue,
    previous_text: String,
    next_text: String,
}

impl TableCellEditChange {
    fn new(
        action: TableRowAction,
        column_id: impl Into<TableColumnId>,
        previous_value: impl Into<TableCellValue>,
        next_value: impl Into<TableCellValue>,
    ) -> Self {
        let previous_value = previous_value.into();
        let next_value = next_value.into();
        Self {
            action,
            column_id: column_id.into(),
            previous_text: previous_value.filter_text(),
            next_text: next_value.filter_text(),
            previous_value,
            next_value,
        }
    }

    /// Creates an editable cell payload from stable row and column ids.
    pub fn for_row(
        row_id: impl Into<TableRowId>,
        column_id: impl Into<TableColumnId>,
        previous_value: impl Into<TableCellValue>,
        next_value: impl Into<TableCellValue>,
    ) -> Self {
        let row_id = row_id.into();
        let previous_value = previous_value.into();
        let next_value = next_value.into();
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
            previous_text: previous_value.filter_text(),
            next_text: next_value.filter_text(),
            previous_value,
            next_value,
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

    /// Returns the resolved value before the edit.
    pub const fn previous_value(&self) -> &TableCellValue {
        &self.previous_value
    }

    /// Returns the resolved text before the edit.
    pub fn previous_text(&self) -> &str {
        &self.previous_text
    }

    /// Returns the resolved value after the edit.
    pub const fn next_value(&self) -> &TableCellValue {
        &self.next_value
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
                    &self.next_value,
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
    next_value: &TableCellValue,
    outcome: &mut TableCellEditApplyOutcome,
) -> open_gpui_ui_core::TableRow {
    if row.id() == row_id {
        *outcome = if row.cell(column_id).is_some() {
            TableCellEditApplyOutcome::Updated
        } else {
            TableCellEditApplyOutcome::CellNotFound
        };

        if outcome.updated() {
            return row.with_cell(column_id.clone(), next_value.clone());
        }
        return row;
    }

    let children = row
        .children()
        .iter()
        .cloned()
        .map(|child| apply_table_cell_edit_to_row(child, row_id, column_id, next_value, outcome))
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

/// Resolved renderer-neutral state for a table numeric range filter recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRangeFilterState {
    id: String,
    label: String,
    column_id: TableColumnId,
    min_text: String,
    max_text: String,
    min_value: Option<f64>,
    max_value: Option<f64>,
    facet_range: Option<TableFacetRange>,
    trigger_label: String,
    min_placeholder: String,
    max_placeholder: String,
    clear_label: String,
    popover: PopoverState,
    min_input: TextInputState,
    max_input: TextInputState,
}

impl TableRangeFilterState {
    #[allow(clippy::too_many_arguments)]
    fn resolve(
        id: impl Into<String>,
        label: impl Into<String>,
        column_id: TableColumnId,
        facets: Option<&TableColumnFacets>,
        min_text: impl Into<String>,
        max_text: impl Into<String>,
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
        let min_text = min_text.into();
        let max_text = max_text.into();
        let clear_label = clear_label.into();
        let (min_value, max_value) = normalize_table_range_filter_values(
            parse_table_range_filter_bound(&min_text),
            parse_table_range_filter_bound(&max_text),
        );
        let facet_range = facets.and_then(TableColumnFacets::numeric_range);
        let trigger_label = table_range_filter_trigger_label(&label, min_value, max_value);
        let min_placeholder =
            table_range_filter_bound_placeholder("Min", facet_range.map(TableFacetRange::min));
        let max_placeholder =
            table_range_filter_bound_placeholder("Max", facet_range.map(TableFacetRange::max));
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
        let min_input = TextInputState::resolve(
            min_text.clone(),
            Some(min_placeholder.clone()),
            size,
            disabled,
            false,
            false,
            false,
            true,
            tokens,
        );
        let max_input = TextInputState::resolve(
            max_text.clone(),
            Some(max_placeholder.clone()),
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
            min_text,
            max_text,
            min_value,
            max_value,
            facet_range,
            trigger_label,
            min_placeholder,
            max_placeholder,
            clear_label,
            popover,
            min_input,
            max_input,
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

    /// Returns lower endpoint text.
    pub fn min_text(&self) -> &str {
        &self.min_text
    }

    /// Returns upper endpoint text.
    pub fn max_text(&self) -> &str {
        &self.max_text
    }

    /// Returns parsed lower endpoint.
    pub const fn min_value(&self) -> Option<f64> {
        self.min_value
    }

    /// Returns parsed upper endpoint.
    pub const fn max_value(&self) -> Option<f64> {
        self.max_value
    }

    /// Returns the visible facet range metadata, when available.
    pub const fn facet_range(&self) -> Option<TableFacetRange> {
        self.facet_range
    }

    /// Returns the trigger label including active range bounds.
    pub fn trigger_label(&self) -> &str {
        &self.trigger_label
    }

    /// Returns the lower-bound placeholder.
    pub fn min_placeholder(&self) -> &str {
        &self.min_placeholder
    }

    /// Returns the upper-bound placeholder.
    pub fn max_placeholder(&self) -> &str {
        &self.max_placeholder
    }

    /// Returns whether the range filter currently has parsed bounds.
    pub const fn active(&self) -> bool {
        self.min_value.is_some() || self.max_value.is_some()
    }

    /// Returns whether clear should be enabled.
    pub fn clear_enabled(&self) -> bool {
        self.active() || !self.min_text.trim().is_empty() || !self.max_text.trim().is_empty()
    }

    /// Returns the clear-all label.
    pub fn clear_label(&self) -> &str {
        &self.clear_label
    }

    /// Returns resolved popover state.
    pub const fn popover(&self) -> &PopoverState {
        &self.popover
    }

    /// Returns resolved lower-bound input state.
    pub const fn min_input(&self) -> &TextInputState {
        &self.min_input
    }

    /// Returns resolved upper-bound input state.
    pub const fn max_input(&self) -> &TextInputState {
        &self.max_input
    }
}

#[derive(Debug, Clone)]
struct TableRangeFilterRuntime {
    min_text: String,
    max_text: String,
}

/// A Popover + min/max text input recipe for one numeric table column.
#[derive(IntoElement)]
pub struct TableRangeFilter {
    id: String,
    label: SharedString,
    column_id: TableColumnId,
    facets: Option<TableColumnFacets>,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    default_min_text: String,
    default_max_text: String,
    clear_label: SharedString,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_change: Option<TableRangeFilterChangeHandler>,
}

impl TableRangeFilter {
    /// Creates a numeric range filter recipe for one table column.
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
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            default_min_text: String::new(),
            default_max_text: String::new(),
            clear_label: "Clear range".into(),
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Start,
            outside_press_policy: OutsidePressPolicy::DismissAndPassThrough,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_open_change: None,
            on_change: None,
        }
    }

    /// Applies resolved facet metadata for this numeric column.
    pub fn facets(mut self, facets: TableColumnFacets) -> Self {
        self.facets = Some(facets);
        self
    }

    /// Seeds endpoint text from the current selected numeric range.
    pub fn range(mut self, min: Option<f64>, max: Option<f64>) -> Self {
        let (min, max) = normalize_table_range_filter_values(min, max);
        self.default_min_text = table_range_filter_value_text(min);
        self.default_max_text = table_range_filter_value_text(max);
        self
    }

    /// Applies default lower-bound endpoint text.
    pub fn default_min_text(mut self, text: impl Into<String>) -> Self {
        self.default_min_text = text.into();
        self
    }

    /// Applies default upper-bound endpoint text.
    pub fn default_max_text(mut self, text: impl Into<String>) -> Self {
        self.default_max_text = text.into();
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

    /// Registers a range-change handler.
    pub fn on_change(
        mut self,
        handler: impl Fn(TableRangeFilterChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved recipe state from the default endpoint text.
    pub fn state(&self) -> TableRangeFilterState {
        TableRangeFilterState::resolve(
            self.id.clone(),
            self.label.to_string(),
            self.column_id.clone(),
            self.facets.as_ref(),
            self.default_min_text.clone(),
            self.default_max_text.clone(),
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

impl Sizable for TableRangeFilter {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for TableRangeFilter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime_id = format!("{}-runtime", self.id);
        let runtime = window.use_keyed_state(runtime_id, cx, |_, _| TableRangeFilterRuntime {
            min_text: self.default_min_text.clone(),
            max_text: self.default_max_text.clone(),
        });
        let min_text = runtime.read(cx).min_text.clone();
        let max_text = runtime.read(cx).max_text.clone();
        let state = TableRangeFilterState::resolve(
            self.id.clone(),
            self.label.clone(),
            self.column_id.clone(),
            self.facets.as_ref(),
            min_text,
            max_text,
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
        let content = table_range_filter_content_element(
            format!("{}-content", self.id),
            format!("{}-min", self.id),
            format!("{}-max", self.id),
            format!("{}-clear", self.id),
            state.clone(),
            runtime,
            self.on_change.clone(),
            self.column_id.clone(),
            self.size,
            self.tokens,
        );
        let summary_text = if state.active() {
            state.trigger_label().to_owned()
        } else {
            state.label().to_owned()
        };

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

/// One column row in a table column-visibility recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumnVisibilityItemState {
    column_id: TableColumnId,
    label: String,
    checked: bool,
    hideable: bool,
}

impl TableColumnVisibilityItemState {
    fn new(column: &TableColumn, visibility: &TableColumnVisibilityOverrides) -> Self {
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
    fn resolve(
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

#[derive(Debug, Clone)]
struct TableColumnVisibilityRuntime {
    visibility: TableColumnVisibilityOverrides,
}

/// A Popover + checkbox-list recipe for controlling visible table columns.
#[derive(IntoElement)]
pub struct TableColumnVisibility {
    id: String,
    label: SharedString,
    columns: Vec<TableColumn>,
    visibility: Option<TableColumnVisibilityOverrides>,
    default_visibility: TableColumnVisibilityOverrides,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    viewport_item_count: usize,
    empty_label: SharedString,
    show_all_label: SharedString,
    reset_label: SharedString,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    on_change: Option<TableColumnVisibilityChangeHandler>,
}

impl TableColumnVisibility {
    /// Creates a column-visibility recipe.
    pub fn new(id: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            columns: Vec::new(),
            visibility: None,
            default_visibility: TableColumnVisibilityOverrides::default(),
            size: Size::Medium,
            disabled: false,
            open: None,
            default_open: false,
            viewport_item_count: 8,
            empty_label: "No columns".into(),
            show_all_label: "Show all".into(),
            reset_label: "Reset".into(),
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Start,
            outside_press_policy: OutsidePressPolicy::DismissAndPassThrough,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_open_change: None,
            on_change: None,
        }
    }

    /// Applies the column descriptors to list in this control.
    pub fn columns(mut self, columns: impl IntoIterator<Item = TableColumn>) -> Self {
        self.columns = columns.into_iter().collect();
        self
    }

    /// Applies a controlled runtime visibility override state.
    pub fn visibility(mut self, visibility: TableColumnVisibilityOverrides) -> Self {
        self.visibility = Some(visibility);
        self
    }

    /// Applies the default visibility overrides for adapter-owned state.
    pub fn default_visibility(mut self, visibility: TableColumnVisibilityOverrides) -> Self {
        self.default_visibility = visibility;
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

    /// Applies the empty-state label.
    pub fn empty_label(mut self, label: impl Into<SharedString>) -> Self {
        self.empty_label = label.into();
        self
    }

    /// Applies the show-all button label.
    pub fn show_all_label(mut self, label: impl Into<SharedString>) -> Self {
        self.show_all_label = label.into();
        self
    }

    /// Applies the reset button label.
    pub fn reset_label(mut self, label: impl Into<SharedString>) -> Self {
        self.reset_label = label.into();
        self
    }

    /// Marks the trigger and content controls as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies the estimated number of column rows visible in the popup.
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

    /// Registers a column-visibility change handler.
    pub fn on_change(
        mut self,
        handler: impl Fn(TableColumnVisibilityChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved recipe state.
    pub fn state(&self) -> TableColumnVisibilityState {
        let visibility = self.visibility.as_ref().unwrap_or(&self.default_visibility);
        TableColumnVisibilityState::resolve(
            self.id.clone(),
            self.label.to_string(),
            &self.columns,
            visibility,
            self.empty_label.to_string(),
            self.show_all_label.to_string(),
            self.reset_label.to_string(),
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

impl Sizable for TableColumnVisibility {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for TableColumnVisibility {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime_id = format!("{}-runtime", self.id);
        let runtime = window.use_keyed_state(runtime_id, cx, |_, _| TableColumnVisibilityRuntime {
            visibility: self.default_visibility.clone(),
        });
        let controlled_visibility = self.visibility.clone();
        let runtime_visibility = runtime.read(cx).visibility.clone();
        let visibility = controlled_visibility.clone().unwrap_or(runtime_visibility);

        if controlled_visibility.is_some() && runtime.read(cx).visibility != visibility {
            runtime.update(cx, |runtime, _| {
                runtime.visibility = visibility.clone();
            });
        }

        let state = TableColumnVisibilityState::resolve(
            self.id.clone(),
            self.label.clone(),
            &self.columns,
            &visibility,
            self.empty_label.clone(),
            self.show_all_label.clone(),
            self.reset_label.clone(),
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
        let content = table_column_visibility_content_element(
            format!("{}-content", self.id),
            format!("{}-items", self.id),
            state.clone(),
            runtime,
            self.on_change.clone(),
            self.size.list_row_h() * self.viewport_item_count as f32,
            self.size,
        );
        let summary_text = state.trigger_label().to_owned();

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

/// Resolved renderer-neutral state for a table global text filter recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct TableGlobalFilterState {
    id: String,
    label: String,
    query: String,
    placeholder: String,
    clear_label: String,
    input: TextInputState,
}

impl TableGlobalFilterState {
    fn resolve(
        id: impl Into<String>,
        label: impl Into<String>,
        query: impl Into<String>,
        placeholder: impl Into<String>,
        clear_label: impl Into<String>,
        size: Size,
        disabled: bool,
        tokens: ThemeTokens,
    ) -> Self {
        let query = query.into();
        let placeholder = placeholder.into();
        let input = TextInputState::resolve(
            query.clone(),
            Some(placeholder.clone()),
            size,
            disabled,
            false,
            false,
            false,
            true,
            tokens,
        );

        Self {
            id: id.into(),
            label: label.into(),
            query,
            placeholder,
            clear_label: clear_label.into(),
            input,
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

    /// Returns the current global query text.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns whether the global filter is active after trimming whitespace.
    pub fn active(&self) -> bool {
        !self.query.trim().is_empty()
    }

    /// Returns whether the clear action should be available.
    pub fn clear_enabled(&self) -> bool {
        !self.query.is_empty()
    }

    /// Returns the input placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Returns the clear button label.
    pub fn clear_label(&self) -> &str {
        &self.clear_label
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.input.size()
    }

    /// Returns whether the filter input and clear action are disabled.
    pub const fn disabled(&self) -> bool {
        self.input.disabled()
    }

    /// Returns resolved text input state.
    pub const fn input(&self) -> &TextInputState {
        &self.input
    }
}

#[derive(Debug, Clone)]
struct TableGlobalFilterRuntime {
    query: String,
}

/// A compact text input recipe for controlling a table global filter.
#[derive(IntoElement)]
pub struct TableGlobalFilter {
    id: String,
    label: SharedString,
    query: Option<String>,
    default_query: String,
    placeholder: SharedString,
    clear_label: SharedString,
    size: Size,
    disabled: bool,
    tokens: ThemeTokens,
    on_change: Option<TableGlobalFilterChangeHandler>,
}

impl TableGlobalFilter {
    /// Creates a global filter recipe for a table.
    pub fn new(id: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            query: None,
            default_query: String::new(),
            placeholder: "Search rows".into(),
            clear_label: "Clear search".into(),
            size: Size::Medium,
            disabled: false,
            tokens: ThemeTokens::default(),
            on_change: None,
        }
    }

    /// Applies controlled query text.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Applies the default query for adapter-owned input state.
    pub fn default_query(mut self, query: impl Into<String>) -> Self {
        self.default_query = query.into();
        self
    }

    /// Applies input placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Applies the clear button label.
    pub fn clear_label(mut self, label: impl Into<SharedString>) -> Self {
        self.clear_label = label.into();
        self
    }

    /// Marks the filter input and clear action as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a global-filter query-change handler.
    pub fn on_change(
        mut self,
        handler: impl Fn(TableGlobalFilterChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved recipe state.
    pub fn state(&self) -> TableGlobalFilterState {
        let query = self.query.as_deref().unwrap_or(self.default_query.as_str());
        TableGlobalFilterState::resolve(
            self.id.clone(),
            self.label.to_string(),
            query,
            self.placeholder.to_string(),
            self.clear_label.to_string(),
            self.size,
            self.disabled,
            self.tokens,
        )
    }
}

impl Sizable for TableGlobalFilter {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for TableGlobalFilter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime_id = format!("{}-runtime", self.id);
        let runtime = window.use_keyed_state(runtime_id, cx, |_, _| TableGlobalFilterRuntime {
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

        let state = TableGlobalFilterState::resolve(
            self.id.clone(),
            self.label.clone(),
            query.clone(),
            self.placeholder.clone(),
            self.clear_label.clone(),
            self.size,
            self.disabled,
            self.tokens,
        );
        let debug_id = state.id().to_owned();
        let label = state.label().to_owned();
        let placeholder = state.placeholder().to_owned();
        let clear_label = state.clear_label().to_owned();
        let clear_enabled = state.clear_enabled();
        let disabled = state.disabled();
        let size = state.size();
        let text_color = ThemeResolver::resolve(state.input().colors().foreground());
        let input_id = format!("{}-input", self.id);
        let clear_id = format!("{}-clear", self.id);
        let runtime_for_input = runtime.clone();
        let runtime_for_clear = runtime.clone();
        let on_change_for_input = self.on_change.clone();
        let on_change_for_clear = self.on_change.clone();

        div()
            .id(self.id)
            .debug_selector(move || format!("table-global-filter:{debug_id}:root"))
            .min_w(px(0.0))
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .text_size(gpui_px_from_ui(size.control_text_px()))
            .text_color(text_color)
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.clone()),
            )
            .child(
                div().min_w(px(0.0)).flex_1().child(
                    TextInput::new(input_id, label)
                        .with_size(size)
                        .value(query)
                        .placeholder(placeholder)
                        .disabled(disabled)
                        .tokens(self.tokens)
                        .on_change(move |next_query, window, cx| {
                            runtime_for_input.update(cx, |runtime, _| {
                                runtime.query = next_query.clone();
                            });
                            if let Some(on_change) = on_change_for_input.as_ref() {
                                on_change(TableGlobalFilterChange::new(next_query), window, cx);
                            }
                        }),
                ),
            )
            .when(clear_enabled, |this| {
                this.child(
                    Button::new(clear_id, clear_label)
                        .variant(ButtonVariant::Ghost)
                        .with_size(size)
                        .disabled(disabled)
                        .on_click(move |_, window, cx| {
                            runtime_for_clear.update(cx, |runtime, _| {
                                runtime.query.clear();
                            });
                            if let Some(on_change) = on_change_for_clear.as_ref() {
                                on_change(TableGlobalFilterChange::clear(), window, cx);
                            }
                        }),
                )
            })
    }
}

/// Supported predicate operator families for the table predicate filter recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TablePredicateFilterOperator {
    /// A text predicate backed by [`TableTextFilterOperator`].
    Text(TableTextFilterOperator),
    /// A numeric predicate backed by [`TableNumericFilterOperator`].
    Number(TableNumericFilterOperator),
}

impl TablePredicateFilterOperator {
    /// Creates a text operator wrapper.
    pub const fn text(operator: TableTextFilterOperator) -> Self {
        Self::Text(operator)
    }

    /// Creates a numeric operator wrapper.
    pub const fn number(operator: TableNumericFilterOperator) -> Self {
        Self::Number(operator)
    }

    /// Returns a stable operator value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text(TableTextFilterOperator::Contains) => "text:contains",
            Self::Text(TableTextFilterOperator::NotContains) => "text:not_contains",
            Self::Text(TableTextFilterOperator::Equals) => "text:equals",
            Self::Text(TableTextFilterOperator::NotEquals) => "text:not_equals",
            Self::Text(TableTextFilterOperator::StartsWith) => "text:starts_with",
            Self::Text(TableTextFilterOperator::EndsWith) => "text:ends_with",
            Self::Number(TableNumericFilterOperator::GreaterThan) => "number:greater_than",
            Self::Number(TableNumericFilterOperator::GreaterThanOrEqual) => {
                "number:greater_than_or_equal"
            }
            Self::Number(TableNumericFilterOperator::LessThan) => "number:less_than",
            Self::Number(TableNumericFilterOperator::LessThanOrEqual) => {
                "number:less_than_or_equal"
            }
        }
    }

    /// Returns the visible operator label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Text(TableTextFilterOperator::Contains) => "Contains",
            Self::Text(TableTextFilterOperator::NotContains) => "Does not contain",
            Self::Text(TableTextFilterOperator::Equals) => "Equals",
            Self::Text(TableTextFilterOperator::NotEquals) => "Does not equal",
            Self::Text(TableTextFilterOperator::StartsWith) => "Starts with",
            Self::Text(TableTextFilterOperator::EndsWith) => "Ends with",
            Self::Number(TableNumericFilterOperator::GreaterThan) => "Greater than",
            Self::Number(TableNumericFilterOperator::GreaterThanOrEqual) => "Greater than or equal",
            Self::Number(TableNumericFilterOperator::LessThan) => "Less than",
            Self::Number(TableNumericFilterOperator::LessThanOrEqual) => "Less than or equal",
        }
    }

    /// Returns the wrapped text operator, when available.
    pub const fn text_operator(self) -> Option<TableTextFilterOperator> {
        match self {
            Self::Text(operator) => Some(operator),
            Self::Number(_) => None,
        }
    }

    /// Returns the wrapped numeric operator, when available.
    pub const fn numeric_operator(self) -> Option<TableNumericFilterOperator> {
        match self {
            Self::Text(_) => None,
            Self::Number(operator) => Some(operator),
        }
    }

    /// Resolves a stable operator wrapper from the serialized value.
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "text:contains" => Some(Self::text(TableTextFilterOperator::Contains)),
            "text:not_contains" => Some(Self::text(TableTextFilterOperator::NotContains)),
            "text:equals" => Some(Self::text(TableTextFilterOperator::Equals)),
            "text:not_equals" => Some(Self::text(TableTextFilterOperator::NotEquals)),
            "text:starts_with" => Some(Self::text(TableTextFilterOperator::StartsWith)),
            "text:ends_with" => Some(Self::text(TableTextFilterOperator::EndsWith)),
            "number:greater_than" => Some(Self::number(TableNumericFilterOperator::GreaterThan)),
            "number:greater_than_or_equal" => {
                Some(Self::number(TableNumericFilterOperator::GreaterThanOrEqual))
            }
            "number:less_than" => Some(Self::number(TableNumericFilterOperator::LessThan)),
            "number:less_than_or_equal" => {
                Some(Self::number(TableNumericFilterOperator::LessThanOrEqual))
            }
            _ => None,
        }
    }

    /// Builds the matching table filter for a supplied value.
    pub fn filter(self, column_id: impl Into<TableColumnId>, value: &str) -> Option<TableFilter> {
        let column_id = column_id.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }

        match self {
            Self::Text(operator) => Some(TableFilter::text(column_id, operator, trimmed)),
            Self::Number(operator) => trimmed
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .and_then(|value| TableFilter::number_comparison(column_id, operator, value)),
        }
    }
}

/// One selectable operator row in a table predicate filter recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TablePredicateFilterOperatorOptionState {
    operator: TablePredicateFilterOperator,
    label: String,
    selected: bool,
}

impl TablePredicateFilterOperatorOptionState {
    fn new(
        operator: TablePredicateFilterOperator,
        label: impl Into<String>,
        selected: bool,
    ) -> Self {
        Self {
            operator,
            label: label.into(),
            selected,
        }
    }

    /// Returns the resolved operator.
    pub const fn operator(&self) -> TablePredicateFilterOperator {
        self.operator
    }

    /// Returns the stable serialized operator value.
    pub fn value(&self) -> &'static str {
        self.operator.as_str()
    }

    /// Returns the visible label for this operator option.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether this option is currently selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }
}

/// Controlled payload emitted when a table predicate filter changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TablePredicateFilterChange {
    column_id: TableColumnId,
    operator: Option<TablePredicateFilterOperator>,
    value: String,
    cleared: bool,
}

impl TablePredicateFilterChange {
    /// Creates a predicate-change payload from the current operator and value.
    pub fn new(
        column_id: impl Into<TableColumnId>,
        operator: TablePredicateFilterOperator,
        value: impl Into<String>,
    ) -> Self {
        Self {
            column_id: column_id.into(),
            operator: Some(operator),
            value: value.into(),
            cleared: false,
        }
    }

    /// Creates a payload that clears this column's predicate filter.
    pub fn clear(column_id: impl Into<TableColumnId>) -> Self {
        Self {
            column_id: column_id.into(),
            operator: None,
            value: String::new(),
            cleared: true,
        }
    }

    /// Returns the filtered column identity.
    pub const fn column_id(&self) -> &TableColumnId {
        &self.column_id
    }

    /// Returns the selected operator, when present.
    pub const fn operator(&self) -> Option<TablePredicateFilterOperator> {
        self.operator
    }

    /// Returns the raw value exactly as entered.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns true when this payload was created by a clear action.
    pub const fn cleared(&self) -> bool {
        self.cleared
    }

    /// Returns whether this payload resolves to an active table filter.
    pub fn active(&self) -> bool {
        self.filter().is_some()
    }

    /// Returns the next filter, when the payload resolves to one.
    pub fn filter(&self) -> Option<TableFilter> {
        if self.cleared {
            return None;
        }

        self.operator
            .and_then(|operator| operator.filter(self.column_id.clone(), &self.value))
    }

    /// Returns the next column-filter list while preserving unrelated filters.
    pub fn next_filters(&self, filters: impl IntoIterator<Item = TableFilter>) -> Vec<TableFilter> {
        table_predicate_filter_next_filters(filters, &self.column_id, self.filter())
    }

    /// Applies this predicate change to a table state and resets pagination to the first page.
    pub fn apply_to(&self, state: TableState) -> TableState {
        let next_filters = self.next_filters(state.filters().iter().cloned());
        let next_pagination = state.pagination().with_page_index(0);

        state
            .with_filters(next_filters)
            .with_pagination(next_pagination)
    }
}

/// Resolved renderer-neutral state for a table predicate filter recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct TablePredicateFilterState {
    id: String,
    label: String,
    column_id: TableColumnId,
    operator: TablePredicateFilterOperator,
    value: String,
    placeholder: String,
    clear_label: String,
    operator_options: Vec<TablePredicateFilterOperatorOptionState>,
    select: SelectState,
    input: TextInputState,
}

impl TablePredicateFilterState {
    #[allow(clippy::too_many_arguments)]
    fn resolve(
        id: impl Into<String>,
        label: impl Into<String>,
        column_id: TableColumnId,
        operator: TablePredicateFilterOperator,
        value: impl Into<String>,
        operator_options: impl IntoIterator<Item = (TablePredicateFilterOperator, SharedString)>,
        placeholder: impl Into<String>,
        clear_label: impl Into<String>,
        size: Size,
        disabled: bool,
        tokens: ThemeTokens,
    ) -> Self {
        let id = id.into();
        let label = label.into();
        let value = value.into();
        let placeholder = placeholder.into();
        let clear_label = clear_label.into();
        let operator_options = table_predicate_filter_operator_options(operator, operator_options);
        let select = Select::new(format!("{id}-operator"), format!("{label} operator"))
            .options(
                operator_options
                    .iter()
                    .map(|option| ListboxOption::new(option.value(), option.label().to_owned()))
                    .collect::<Vec<_>>(),
            )
            .selected(operator.as_str())
            .placeholder("Operator")
            .with_size(size)
            .disabled(disabled)
            .tokens(tokens)
            .state();
        let input = TextInputState::resolve(
            value.clone(),
            Some(placeholder.clone()),
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
            operator,
            value,
            placeholder,
            clear_label,
            operator_options,
            select,
            input,
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

    /// Returns the current operator.
    pub const fn operator(&self) -> TablePredicateFilterOperator {
        self.operator
    }

    /// Returns the raw value exactly as entered.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the input placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Returns the clear button label.
    pub fn clear_label(&self) -> &str {
        &self.clear_label
    }

    /// Returns whether the predicate currently resolves to an active filter.
    pub fn active(&self) -> bool {
        self.operator
            .filter(self.column_id.clone(), &self.value)
            .is_some()
    }

    /// Returns whether the clear action should be available.
    pub fn clear_enabled(&self) -> bool {
        !self.value.trim().is_empty()
    }

    /// Returns the available operator options in stable order.
    pub fn operator_options(&self) -> &[TablePredicateFilterOperatorOptionState] {
        &self.operator_options
    }

    /// Returns resolved select state for the operator control.
    pub const fn select(&self) -> &SelectState {
        &self.select
    }

    /// Returns resolved text input state.
    pub const fn input(&self) -> &TextInputState {
        &self.input
    }

    /// Returns the foundation size from the nested controls.
    pub const fn size(&self) -> Size {
        self.input.size()
    }

    /// Returns whether the predicate controls are disabled.
    pub const fn disabled(&self) -> bool {
        self.input.disabled()
    }
}

#[derive(Debug, Clone)]
struct TablePredicateFilterRuntime {
    operator: TablePredicateFilterOperator,
    value: String,
}

/// A compact operator select + text input recipe for one table column predicate.
#[derive(IntoElement)]
pub struct TablePredicateFilter {
    id: String,
    label: SharedString,
    column_id: TableColumnId,
    operator: Option<TablePredicateFilterOperator>,
    default_operator: TablePredicateFilterOperator,
    value: Option<String>,
    default_value: String,
    operator_options: Vec<(TablePredicateFilterOperator, SharedString)>,
    placeholder: SharedString,
    clear_label: SharedString,
    size: Size,
    disabled: bool,
    tokens: ThemeTokens,
    on_change: Option<TablePredicateFilterChangeHandler>,
}

impl TablePredicateFilter {
    /// Creates a predicate filter recipe for one table column.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        column_id: impl Into<TableColumnId>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            column_id: column_id.into(),
            operator: None,
            default_operator: TablePredicateFilterOperator::text(TableTextFilterOperator::Contains),
            value: None,
            default_value: String::new(),
            operator_options: Vec::new(),
            placeholder: "Filter value".into(),
            clear_label: "Clear filter".into(),
            size: Size::Medium,
            disabled: false,
            tokens: ThemeTokens::default(),
            on_change: None,
        }
    }

    /// Applies controlled operator state.
    pub fn operator(mut self, operator: TablePredicateFilterOperator) -> Self {
        self.operator = Some(operator);
        self
    }

    /// Applies the default operator for adapter-owned state.
    pub fn default_operator(mut self, operator: TablePredicateFilterOperator) -> Self {
        self.default_operator = operator;
        self
    }

    /// Applies controlled value text.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Applies the default value for adapter-owned state.
    pub fn default_value(mut self, value: impl Into<String>) -> Self {
        self.default_value = value.into();
        self
    }

    /// Adds one operator option with an explicit label.
    pub fn operator_option(
        mut self,
        operator: TablePredicateFilterOperator,
        label: impl Into<SharedString>,
    ) -> Self {
        self.operator_options.push((operator, label.into()));
        self
    }

    /// Adds many operator options using the stable operator defaults.
    pub fn operators(
        mut self,
        operators: impl IntoIterator<Item = TablePredicateFilterOperator>,
    ) -> Self {
        self.operator_options
            .extend(operators.into_iter().map(|operator| {
                let label = operator.label();
                (operator, SharedString::from(label))
            }));
        self
    }

    /// Applies input placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Applies the clear button label.
    pub fn clear_label(mut self, label: impl Into<SharedString>) -> Self {
        self.clear_label = label.into();
        self
    }

    /// Marks the filter controls as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers a predicate-change handler.
    pub fn on_change(
        mut self,
        handler: impl Fn(TablePredicateFilterChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved recipe state.
    pub fn state(&self) -> TablePredicateFilterState {
        let operator = self.operator.unwrap_or(self.default_operator);
        let value = self.value.as_deref().unwrap_or(self.default_value.as_str());
        TablePredicateFilterState::resolve(
            self.id.clone(),
            self.label.to_string(),
            self.column_id.clone(),
            operator,
            value,
            self.operator_options.clone(),
            self.placeholder.to_string(),
            self.clear_label.to_string(),
            self.size,
            self.disabled,
            self.tokens,
        )
    }
}

impl Sizable for TablePredicateFilter {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for TablePredicateFilter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime_id = format!("{}-runtime", self.id);
        let runtime = window.use_keyed_state(runtime_id, cx, |_, _| TablePredicateFilterRuntime {
            operator: self.default_operator,
            value: self.default_value.clone(),
        });
        let runtime_state = runtime.read(cx).clone();
        let controlled_operator = self.operator;
        let controlled_value = self.value.clone();
        let operator = controlled_operator.unwrap_or(runtime_state.operator);
        let value = controlled_value.clone().unwrap_or(runtime_state.value);

        if controlled_operator.is_some() && runtime.read(cx).operator != operator {
            runtime.update(cx, |runtime, _| {
                runtime.operator = operator;
            });
        }
        if controlled_value.is_some() && runtime.read(cx).value != value {
            runtime.update(cx, |runtime, _| {
                runtime.value = value.clone();
            });
        }

        let state = TablePredicateFilterState::resolve(
            self.id.clone(),
            self.label.clone(),
            self.column_id.clone(),
            operator,
            value.clone(),
            self.operator_options.clone(),
            self.placeholder.clone(),
            self.clear_label.clone(),
            self.size,
            self.disabled,
            self.tokens,
        );
        let debug_id = state.id().to_owned();
        let label = state.label().to_owned();
        let placeholder = state.placeholder().to_owned();
        let clear_label = state.clear_label().to_owned();
        let disabled = state.disabled();
        let size = state.size();
        let select_id = format!("{}-operator", self.id);
        let input_id = format!("{}-value", self.id);
        let clear_id = format!("{}-clear", self.id);
        let column_id_for_select = self.column_id.clone();
        let column_id_for_input = self.column_id.clone();
        let column_id_for_clear = self.column_id.clone();
        let runtime_for_select = runtime.clone();
        let runtime_for_input = runtime.clone();
        let runtime_for_clear = runtime.clone();
        let on_change_for_select = self.on_change.clone();
        let on_change_for_input = self.on_change.clone();
        let on_change_for_clear = self.on_change.clone();
        let select_label = format!("{label} operator");

        div()
            .id(self.id)
            .debug_selector(move || format!("table-predicate-filter:{debug_id}:root"))
            .min_w(px(0.0))
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .text_size(gpui_px_from_ui(size.control_text_px()))
            .text_color(ThemeResolver::resolve(state.input().colors().foreground()))
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.clone()),
            )
            .child(
                Select::new(select_id, select_label)
                    .with_size(size)
                    .selected(state.operator().as_str())
                    .options(
                        state
                            .operator_options()
                            .iter()
                            .map(|option| {
                                ListboxOption::new(option.value(), option.label().to_owned())
                            })
                            .collect::<Vec<_>>(),
                    )
                    .disabled(disabled)
                    .tokens(self.tokens)
                    .on_select(move |selection, window, cx| {
                        let Some(next_operator) =
                            TablePredicateFilterOperator::from_str(selection.value())
                        else {
                            return;
                        };
                        runtime_for_select.update(cx, |runtime, _| {
                            runtime.operator = next_operator;
                        });
                        if let Some(on_change) = on_change_for_select.as_ref() {
                            on_change(
                                TablePredicateFilterChange::new(
                                    column_id_for_select.clone(),
                                    next_operator,
                                    runtime_for_select.read(cx).value.clone(),
                                ),
                                window,
                                cx,
                            );
                        }
                    }),
            )
            .child(
                div().min_w(px(0.0)).flex_1().child(
                    TextInput::new(input_id, label)
                        .with_size(size)
                        .value(value)
                        .placeholder(placeholder)
                        .disabled(disabled)
                        .tokens(self.tokens)
                        .on_change(move |next_value, window, cx| {
                            runtime_for_input.update(cx, |runtime, _| {
                                runtime.value = next_value.clone();
                            });
                            if let Some(on_change) = on_change_for_input.as_ref() {
                                on_change(
                                    TablePredicateFilterChange::new(
                                        column_id_for_input.clone(),
                                        runtime_for_input.read(cx).operator,
                                        next_value,
                                    ),
                                    window,
                                    cx,
                                );
                            }
                        }),
                ),
            )
            .when(state.clear_enabled(), |this| {
                this.child(
                    Button::new(clear_id, clear_label)
                        .variant(ButtonVariant::Ghost)
                        .with_size(size)
                        .disabled(disabled)
                        .on_click(move |_, window, cx| {
                            runtime_for_clear.update(cx, |runtime, _| {
                                runtime.value.clear();
                            });
                            if let Some(on_change) = on_change_for_clear.as_ref() {
                                on_change(
                                    TablePredicateFilterChange::clear(column_id_for_clear.clone()),
                                    window,
                                    cx,
                                );
                            }
                        }),
                )
            })
    }
}

fn table_predicate_filter_operator_options(
    selected_operator: TablePredicateFilterOperator,
    configured: impl IntoIterator<Item = (TablePredicateFilterOperator, SharedString)>,
) -> Vec<TablePredicateFilterOperatorOptionState> {
    let mut options = configured
        .into_iter()
        .map(|(operator, label)| {
            TablePredicateFilterOperatorOptionState::new(
                operator,
                label.to_string(),
                operator == selected_operator,
            )
        })
        .collect::<Vec<_>>();

    if options.is_empty() {
        options = default_table_predicate_filter_operator_options(selected_operator);
    } else if !options
        .iter()
        .any(|option| option.operator() == selected_operator)
    {
        options.insert(
            0,
            TablePredicateFilterOperatorOptionState::new(
                selected_operator,
                selected_operator.label(),
                true,
            ),
        );
    }

    options
}

fn default_table_predicate_filter_operator_options(
    selected_operator: TablePredicateFilterOperator,
) -> Vec<TablePredicateFilterOperatorOptionState> {
    [
        TablePredicateFilterOperator::text(TableTextFilterOperator::Contains),
        TablePredicateFilterOperator::text(TableTextFilterOperator::NotContains),
        TablePredicateFilterOperator::text(TableTextFilterOperator::Equals),
        TablePredicateFilterOperator::text(TableTextFilterOperator::NotEquals),
        TablePredicateFilterOperator::text(TableTextFilterOperator::StartsWith),
        TablePredicateFilterOperator::text(TableTextFilterOperator::EndsWith),
        TablePredicateFilterOperator::number(TableNumericFilterOperator::GreaterThan),
        TablePredicateFilterOperator::number(TableNumericFilterOperator::GreaterThanOrEqual),
        TablePredicateFilterOperator::number(TableNumericFilterOperator::LessThan),
        TablePredicateFilterOperator::number(TableNumericFilterOperator::LessThanOrEqual),
    ]
    .into_iter()
    .map(|operator| {
        TablePredicateFilterOperatorOptionState::new(
            operator,
            operator.label(),
            operator == selected_operator,
        )
    })
    .collect()
}

fn table_predicate_filter_next_filters(
    filters: impl IntoIterator<Item = TableFilter>,
    column_id: &TableColumnId,
    filter: Option<TableFilter>,
) -> Vec<TableFilter> {
    let mut next = filters
        .into_iter()
        .filter(|filter| {
            if filter.column() != column_id {
                return true;
            }

            filter.text_predicate().is_none() && filter.number_comparison_value().is_none()
        })
        .collect::<Vec<_>>();

    if let Some(filter) = filter {
        next.push(filter);
    }

    next
}

/// Resolved renderer-neutral state for a table toolbar recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableToolbarState {
    id: String,
    label: String,
    size: Size,
    primary_control_count: usize,
    secondary_control_count: usize,
    summary: Option<String>,
    tokens: ThemeTokens,
}

impl TableToolbarState {
    fn resolve(
        id: impl Into<String>,
        label: impl Into<String>,
        size: Size,
        primary_control_count: usize,
        secondary_control_count: usize,
        summary: Option<impl Into<String>>,
        tokens: ThemeTokens,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            size,
            primary_control_count,
            secondary_control_count,
            summary: summary.map(Into::into),
            tokens,
        }
    }

    /// Returns stable recipe id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the visible or accessible toolbar label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the foundation size used for toolbar text and child recipes.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the number of primary controls in the first toolbar row.
    pub const fn primary_control_count(&self) -> usize {
        self.primary_control_count
    }

    /// Returns the number of secondary controls in the second toolbar row.
    pub const fn secondary_control_count(&self) -> usize {
        self.secondary_control_count
    }

    /// Returns the total number of slotted controls.
    pub const fn control_count(&self) -> usize {
        self.primary_control_count + self.secondary_control_count
    }

    /// Returns whether the toolbar has at least one slotted control.
    pub const fn has_controls(&self) -> bool {
        self.control_count() > 0
    }

    /// Returns the optional trailing summary text.
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    /// Returns whether the toolbar exposes a trailing summary.
    pub const fn has_summary(&self) -> bool {
        self.summary.is_some()
    }

    /// Returns accessibility role.
    pub const fn role(&self) -> Role {
        Role::Toolbar
    }

    /// Returns the token bundle used to resolve toolbar text colors.
    pub const fn tokens(&self) -> ThemeTokens {
        self.tokens
    }

    /// Returns the foreground color intent for toolbar labels and controls.
    pub const fn foreground(&self) -> ColorIntent {
        ColorIntent::new(self.tokens.text, 0x18202a)
    }

    /// Returns the muted foreground color intent for summary text.
    pub const fn muted_foreground(&self) -> ColorIntent {
        ColorIntent::new(self.tokens.text_muted, 0x5a6472)
    }
}

/// A table toolbar recipe for composing table filter controls and summary text.
#[derive(IntoElement)]
pub struct TableToolbar {
    id: String,
    label: SharedString,
    size: Size,
    primary_controls: Vec<AnyElement>,
    secondary_controls: Vec<AnyElement>,
    summary: Option<SharedString>,
    tokens: ThemeTokens,
}

impl TableToolbar {
    /// Creates a table toolbar recipe.
    pub fn new(id: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            size: Size::Medium,
            primary_controls: Vec::new(),
            secondary_controls: Vec::new(),
            summary: None,
            tokens: ThemeTokens::default(),
        }
    }

    /// Adds a primary control to the first toolbar row.
    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.primary_controls.push(control.into_any_element());
        self
    }

    /// Adds primary controls to the first toolbar row.
    pub fn controls(mut self, controls: impl IntoIterator<Item = impl IntoElement>) -> Self {
        for control in controls {
            self = self.control(control);
        }
        self
    }

    /// Adds a secondary control to the second toolbar row.
    pub fn secondary_control(mut self, control: impl IntoElement) -> Self {
        self.secondary_controls.push(control.into_any_element());
        self
    }

    /// Adds secondary controls to the second toolbar row.
    pub fn secondary_controls(
        mut self,
        controls: impl IntoIterator<Item = impl IntoElement>,
    ) -> Self {
        for control in controls {
            self = self.secondary_control(control);
        }
        self
    }

    /// Applies trailing summary text.
    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Returns resolved recipe state without exposing renderer-owned child elements.
    pub fn state(&self) -> TableToolbarState {
        TableToolbarState::resolve(
            self.id.clone(),
            self.label.to_string(),
            self.size,
            self.primary_controls.len(),
            self.secondary_controls.len(),
            self.summary.as_ref().map(ToString::to_string),
            self.tokens,
        )
    }
}

impl Sizable for TableToolbar {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for TableToolbar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let state = self.state();
        let debug_id = state.id().to_owned();
        let primary_debug_id = debug_id.clone();
        let secondary_debug_id = debug_id.clone();
        let summary_debug_id = debug_id.clone();
        let label = state.label().to_owned();
        let text_color = ThemeResolver::resolve(state.foreground());
        let summary_text_color = ThemeResolver::resolve(state.muted_foreground());
        let size = state.size();
        let has_primary_controls = state.primary_control_count() > 0;
        let has_secondary_controls = state.secondary_control_count() > 0;
        let has_summary = state.has_summary();
        let primary_controls = self.primary_controls;
        let secondary_controls = self.secondary_controls;
        let summary = self.summary;

        div()
            .id(self.id)
            .debug_selector(move || format!("table-toolbar:{debug_id}:root"))
            .ui_role(state.role())
            .aria_label(label)
            .min_w(px(0.0))
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .text_size(gpui_px_from_ui(size.control_text_px()))
            .text_color(text_color)
            .when(has_primary_controls, |this| {
                this.child(
                    div()
                        .debug_selector(move || {
                            format!("table-toolbar:{primary_debug_id}:primary-controls")
                        })
                        .min_w(px(0.0))
                        .w_full()
                        .flex()
                        .items_center()
                        .gap_2()
                        .flex_wrap()
                        .children(primary_controls),
                )
            })
            .when(has_secondary_controls || has_summary, |this| {
                this.child(
                    div()
                        .min_w(px(0.0))
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .flex_wrap()
                        .when(has_secondary_controls, |row| {
                            row.child(
                                div()
                                    .debug_selector(move || {
                                        format!(
                                            "table-toolbar:{secondary_debug_id}:secondary-controls"
                                        )
                                    })
                                    .min_w(px(0.0))
                                    .flex()
                                    .items_start()
                                    .gap_3()
                                    .flex_wrap()
                                    .children(secondary_controls),
                            )
                        })
                        .when_some(summary, |row, summary| {
                            row.child(
                                div()
                                    .debug_selector(move || {
                                        format!("table-toolbar:{summary_debug_id}:summary")
                                    })
                                    .flex_none()
                                    .text_xs()
                                    .text_color(summary_text_color)
                                    .child(summary),
                            )
                        }),
                )
            })
    }
}

fn table_column_visibility_content_element(
    content_id: String,
    items_id: String,
    state: TableColumnVisibilityState,
    runtime: Entity<TableColumnVisibilityRuntime>,
    on_change: Option<TableColumnVisibilityChangeHandler>,
    items_height: UiPx,
    size: Size,
) -> impl IntoElement {
    let disabled = state.popover().disabled();
    let content_debug_id = state.id().to_owned();
    let count_text = format!("{}/{} visible", state.visible_count(), state.item_count());
    let items = state.items().to_vec();
    let hideable_column_ids = state
        .items()
        .iter()
        .filter(|item| item.hideable())
        .map(|item| item.column_id().clone())
        .collect::<Vec<_>>();
    let show_all_enabled = state.show_all_enabled();
    let reset_enabled = state.reset_enabled();
    let show_all_label = state.show_all_label().to_owned();
    let reset_label = state.reset_label().to_owned();
    let empty_label = state.empty_label().to_owned();
    let show_all_debug_id = state.id().to_owned();
    let reset_debug_id = state.id().to_owned();
    let runtime_for_show_all = runtime.clone();
    let runtime_for_reset = runtime.clone();
    let on_change_for_show_all = on_change.clone();
    let on_change_for_reset = on_change.clone();
    let show_all_ids = hideable_column_ids.clone();
    let show_all_change_ids = hideable_column_ids;
    let body = if state.empty() {
        div()
            .min_w(px(0.0))
            .py(px(4.0))
            .text_sm()
            .opacity(0.72)
            .child(empty_label)
            .into_any_element()
    } else {
        div()
            .flex_1()
            .min_h(px(0.0))
            .h(gpui_px_from_ui(items_height))
            .overflow_hidden()
            .child(
                ScrollArea::new(
                    items_id,
                    table_column_visibility_items_element(
                        state.clone(),
                        items,
                        runtime,
                        on_change,
                        disabled,
                    ),
                )
                .vertical()
                .with_size(size),
            )
            .into_any_element()
    };

    div()
        .id(content_id)
        .debug_selector(move || format!("table-column-visibility:{content_debug_id}:content"))
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
                .child(div().flex_none().text_xs().opacity(0.72).child(count_text)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .child(
                    div()
                        .debug_selector(move || {
                            format!("table-column-visibility:{show_all_debug_id}:show-all")
                        })
                        .child(
                            Button::new(format!("{}-show-all", state.id()), show_all_label)
                                .variant(ButtonVariant::Ghost)
                                .with_size(size)
                                .disabled(disabled || !show_all_enabled)
                                .on_click(move |_, window, cx| {
                                    runtime_for_show_all.update(cx, |runtime, _| {
                                        runtime.visibility = show_all_ids.iter().cloned().fold(
                                            runtime.visibility.clone(),
                                            |visibility, column_id| {
                                                visibility.with_visibility(column_id, true)
                                            },
                                        );
                                    });
                                    if let Some(on_change) = on_change_for_show_all.as_ref() {
                                        on_change(
                                            TableColumnVisibilityChange::show_all(
                                                show_all_change_ids.clone(),
                                            ),
                                            window,
                                            cx,
                                        );
                                    }
                                }),
                        ),
                )
                .child(
                    div()
                        .debug_selector(move || {
                            format!("table-column-visibility:{reset_debug_id}:reset")
                        })
                        .child(
                            Button::new(format!("{}-reset", state.id()), reset_label)
                                .variant(ButtonVariant::Ghost)
                                .with_size(size)
                                .disabled(disabled || !reset_enabled)
                                .on_click(move |_, window, cx| {
                                    runtime_for_reset.update(cx, |runtime, _| {
                                        runtime.visibility =
                                            TableColumnVisibilityOverrides::default();
                                    });
                                    if let Some(on_change) = on_change_for_reset.as_ref() {
                                        on_change(TableColumnVisibilityChange::reset(), window, cx);
                                    }
                                }),
                        ),
                ),
        )
        .child(body)
}

fn table_column_visibility_items_element(
    state: TableColumnVisibilityState,
    items: Vec<TableColumnVisibilityItemState>,
    runtime: Entity<TableColumnVisibilityRuntime>,
    on_change: Option<TableColumnVisibilityChangeHandler>,
    disabled: bool,
) -> impl IntoElement {
    items.into_iter().fold(
        div().flex().flex_col().gap_1().min_w(px(0.0)),
        |list, item| {
            let column_id = item.column_id().clone();
            let column_id_for_checkbox = column_id.clone();
            let column_id_text = column_id.as_str().to_owned();
            let column_id_text_for_row = column_id_text.clone();
            let label = item.label().to_owned();
            let checked = item.checked();
            let row_disabled = disabled || item.disabled();
            let next_checked = !checked;
            let runtime_for_row = runtime.clone();
            let runtime_for_checkbox = runtime.clone();
            let on_change_for_row = on_change.clone();
            let on_change_for_checkbox = on_change.clone();
            let column_id_for_row = column_id.clone();
            let debug_id = state.id().to_owned();
            let row_id = format!("{}-column-row-{column_id_text}", state.id());
            let checkbox_id = format!("{}-column-{column_id_text}", state.id());

            list.child(
                div()
                    .id(row_id)
                    .debug_selector(move || {
                        format!(
                            "table-column-visibility:{debug_id}:column:{column_id_text_for_row}"
                        )
                    })
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .rounded(px(6.0))
                    .px(px(6.0))
                    .py(px(4.0))
                    .when(row_disabled, |this| this.opacity(0.56))
                    .when(!row_disabled, move |this| {
                        this.cursor_pointer()
                            .hover(|style| style.bg(rgba(0x00000010)))
                            .on_click(move |_, window, cx| {
                                runtime_for_row.update(cx, |runtime, _| {
                                    runtime.visibility = runtime
                                        .visibility
                                        .clone()
                                        .with_visibility(column_id_for_row.clone(), next_checked);
                                });
                                if let Some(on_change) = on_change_for_row.as_ref() {
                                    on_change(
                                        TableColumnVisibilityChange::new(
                                            column_id_for_row.clone(),
                                            next_checked,
                                        ),
                                        window,
                                        cx,
                                    );
                                }
                            })
                    })
                    .child(
                        Checkbox::new(checkbox_id)
                            .label(label)
                            .checked(checked)
                            .disabled(row_disabled)
                            .on_toggle(move |toggled, _event, window, cx| {
                                let next_visible = matches!(toggled, Toggled::True);
                                runtime_for_checkbox.update(cx, |runtime, _| {
                                    runtime.visibility =
                                        runtime.visibility.clone().with_visibility(
                                            column_id_for_checkbox.clone(),
                                            next_visible,
                                        );
                                });
                                if let Some(on_change) = on_change_for_checkbox.as_ref() {
                                    on_change(
                                        TableColumnVisibilityChange::new(
                                            column_id_for_checkbox.clone(),
                                            next_visible,
                                        ),
                                        window,
                                        cx,
                                    );
                                }
                            }),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_xs()
                            .opacity(0.72)
                            .child(if row_disabled {
                                "Locked".to_string()
                            } else if checked {
                                "Visible".to_string()
                            } else {
                                "Hidden".to_string()
                            }),
                    ),
            )
        },
    )
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

fn table_range_filter_content_element(
    content_id: String,
    min_id: String,
    max_id: String,
    clear_id: String,
    state: TableRangeFilterState,
    runtime: Entity<TableRangeFilterRuntime>,
    on_change: Option<TableRangeFilterChangeHandler>,
    column_id: TableColumnId,
    size: Size,
    tokens: ThemeTokens,
) -> impl IntoElement {
    let disabled = state.popover().disabled();
    let min_text = state.min_text().to_owned();
    let max_text = state.max_text().to_owned();
    let clear_enabled = state.clear_enabled();
    let clear_label = state.clear_label().to_owned();
    let min_placeholder = state.min_placeholder().to_owned();
    let max_placeholder = state.max_placeholder().to_owned();
    let facet_range_text = state.facet_range().map(|range| {
        format!(
            "{} - {}",
            table_range_filter_value_text(Some(range.min())),
            table_range_filter_value_text(Some(range.max()))
        )
    });
    let runtime_for_min = runtime.clone();
    let runtime_for_max = runtime.clone();
    let on_change_for_min = on_change.clone();
    let on_change_for_max = on_change.clone();
    let column_id_for_min = column_id.clone();
    let column_id_for_max = column_id.clone();
    let content_debug_id = state.id().to_owned();

    div()
        .id(content_id)
        .debug_selector(move || format!("table-range-filter:{content_debug_id}:content"))
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
                .when_some(facet_range_text, |this, text| {
                    this.child(div().flex_none().text_xs().opacity(0.72).child(text))
                }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    TextInput::new(min_id, format!("{} minimum", state.label()))
                        .value(min_text.clone())
                        .placeholder(min_placeholder)
                        .disabled(disabled)
                        .with_size(size)
                        .tokens(tokens)
                        .on_change(move |next_min, window, cx| {
                            runtime_for_min.update(cx, |runtime, _| {
                                runtime.min_text = next_min.clone();
                            });
                            if let Some(on_change) = on_change_for_min.as_ref() {
                                on_change(
                                    TableRangeFilterChange::new(
                                        column_id_for_min.clone(),
                                        next_min,
                                        runtime_for_min.read(cx).max_text.clone(),
                                    ),
                                    window,
                                    cx,
                                );
                            }
                        }),
                )
                .child(
                    TextInput::new(max_id, format!("{} maximum", state.label()))
                        .value(max_text.clone())
                        .placeholder(max_placeholder)
                        .disabled(disabled)
                        .with_size(size)
                        .tokens(tokens)
                        .on_change(move |next_max, window, cx| {
                            runtime_for_max.update(cx, |runtime, _| {
                                runtime.max_text = next_max.clone();
                            });
                            if let Some(on_change) = on_change_for_max.as_ref() {
                                on_change(
                                    TableRangeFilterChange::new(
                                        column_id_for_max.clone(),
                                        runtime_for_max.read(cx).min_text.clone(),
                                        next_max,
                                    ),
                                    window,
                                    cx,
                                );
                            }
                        }),
                ),
        )
        .when(clear_enabled, |this| {
            let runtime_for_clear = runtime.clone();
            let on_change_for_clear = on_change.clone();
            let column_id_for_clear = column_id.clone();
            this.child(
                div().flex().justify_end().child(
                    Button::new(clear_id, clear_label)
                        .variant(ButtonVariant::Ghost)
                        .with_size(size)
                        .disabled(disabled)
                        .on_click(move |_, window, cx| {
                            runtime_for_clear.update(cx, |runtime, _| {
                                runtime.min_text.clear();
                                runtime.max_text.clear();
                            });
                            if let Some(on_change) = on_change_for_clear.as_ref() {
                                on_change(
                                    TableRangeFilterChange::clear(column_id_for_clear.clone()),
                                    window,
                                    cx,
                                );
                            }
                        }),
                ),
            )
        })
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

fn table_range_filter_next_filters(
    filters: impl IntoIterator<Item = TableFilter>,
    column_id: &TableColumnId,
    min: Option<f64>,
    max: Option<f64>,
) -> Vec<TableFilter> {
    let mut next = filters
        .into_iter()
        .filter(|filter| filter.column() != column_id || filter.number_range_bounds().is_none())
        .collect::<Vec<_>>();

    if let Some(filter) = TableFilter::number_range(column_id.clone(), min, max) {
        next.push(filter);
    }

    next
}

fn parse_table_range_filter_bound(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    trimmed
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(|value| if value == 0.0 { 0.0 } else { value })
}

fn normalize_table_range_filter_values(
    min: Option<f64>,
    max: Option<f64>,
) -> (Option<f64>, Option<f64>) {
    match (min, max) {
        (Some(left), Some(right)) if left > right => (Some(right), Some(left)),
        values => values,
    }
}

fn table_range_filter_value_text(value: Option<f64>) -> String {
    value
        .map(|value| {
            if value.fract() == 0.0 {
                format!("{value:.0}")
            } else {
                value.to_string()
            }
        })
        .unwrap_or_default()
}

fn table_range_filter_trigger_label(label: &str, min: Option<f64>, max: Option<f64>) -> String {
    match (min, max) {
        (Some(min), Some(max)) => format!(
            "{label}: {}-{}",
            table_range_filter_value_text(Some(min)),
            table_range_filter_value_text(Some(max))
        ),
        (Some(min), None) => {
            format!("{label}: >= {}", table_range_filter_value_text(Some(min)))
        }
        (None, Some(max)) => {
            format!("{label}: <= {}", table_range_filter_value_text(Some(max)))
        }
        (None, None) => label.to_owned(),
    }
}

fn table_column_visibility_trigger_label(label: &str, hidden_count: usize) -> String {
    if hidden_count == 0 {
        label.to_owned()
    } else {
        format!("{label}: {hidden_count} hidden")
    }
}

fn table_range_filter_bound_placeholder(prefix: &str, value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{prefix} ({})", table_range_filter_value_text(Some(value))),
        None => prefix.to_owned(),
    }
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
    editor: Option<TableCellEditor>,
    select_options: Vec<TableSelectOption>,
    width_policy: TableColumnWidthPolicy,
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
            editor: column.editor(),
            select_options: column.select_options().to_vec(),
            width_policy: column.width_policy(),
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
        self.editor.is_some()
    }

    /// Returns the configured editor for leaf cells in this column.
    pub const fn editor(&self) -> Option<TableCellEditor> {
        self.editor
    }

    /// Returns the fixed select options configured for this column.
    pub fn select_options(&self) -> &[TableSelectOption] {
        &self.select_options
    }

    /// Returns the configured width policy for this column.
    pub const fn width_policy(&self) -> TableColumnWidthPolicy {
        self.width_policy
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

    fn with_width(mut self, width: UiPx) -> Self {
        self.width = width.max(self.min_width).min(self.max_width);
        self
    }

    fn with_offsets(mut self, start: UiPx, after: UiPx) -> Self {
        self.start = start;
        self.after = after;
        self
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

/// One resolved table header cell in render-plan form.
#[derive(Debug, Clone, PartialEq)]
pub struct TableHeaderCellRenderPlan {
    id: String,
    label: String,
    region: TableColumnRegion,
    depth: usize,
    index: usize,
    kind: TableResolvedHeaderKind,
    col_span: usize,
    row_span: usize,
    width: UiPx,
    start: UiPx,
    leaf_column_ids: Vec<TableColumnId>,
    sub_header_ids: Vec<String>,
    sort_direction: Option<TableSortDirection>,
    sort_action: Option<TableHeaderAction>,
    resizable: bool,
}

impl TableHeaderCellRenderPlan {
    fn from_resolved(
        table_id: &str,
        cell: &TableResolvedHeaderCell,
        columns_by_id: &BTreeMap<TableColumnId, &TableColumnRenderPlan>,
    ) -> Self {
        let leaf_column_ids = cell.leaf_column_ids().to_vec();
        let width = leaf_column_ids.iter().fold(UiPx::ZERO, |total, column_id| {
            total
                + columns_by_id
                    .get(column_id)
                    .copied()
                    .map(|column| column.width())
                    .unwrap_or(UiPx::ZERO)
        });
        let sort_source = leaf_column_ids
            .first()
            .and_then(|column_id| columns_by_id.get(column_id).copied());
        let leaf_header = cell.kind().is_leaf() && leaf_column_ids.len() == 1;
        let sort_direction = leaf_header
            .then(|| sort_source.and_then(|column| column.sort_direction()))
            .flatten();
        let sort_action = leaf_header
            .then(|| sort_source.and_then(|column| column.sort_action().cloned()))
            .flatten();
        let resizable = leaf_header
            .then(|| sort_source.map(|column| column.resizable()))
            .flatten()
            .unwrap_or(false);
        let start = leaf_column_ids
            .first()
            .and_then(|column_id| columns_by_id.get(column_id).copied())
            .map(TableColumnRenderPlan::start)
            .unwrap_or(UiPx::ZERO);

        Self {
            id: header_cell_render_id(table_id, cell),
            label: cell.label().to_owned(),
            region: cell.region(),
            depth: cell.depth(),
            index: cell.index(),
            kind: cell.kind(),
            col_span: cell.col_span(),
            row_span: cell.row_span(),
            width,
            start,
            leaf_column_ids,
            sub_header_ids: cell.sub_header_ids().to_vec(),
            sort_direction,
            sort_action,
            resizable,
        }
    }

    /// Returns the stable render identity for this header cell.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the visible header label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the render region for this header cell.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns the header row depth.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the index within the row.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the resolved header kind.
    pub const fn kind(&self) -> TableResolvedHeaderKind {
        self.kind
    }

    /// Returns the leaf-column span.
    pub const fn col_span(&self) -> usize {
        self.col_span
    }

    /// Returns the row span.
    pub const fn row_span(&self) -> usize {
        self.row_span
    }

    /// Returns the summed width of visible leaf coverage.
    pub const fn width(&self) -> UiPx {
        self.width
    }

    /// Returns the horizontal start offset within the render lane.
    pub const fn start(&self) -> UiPx {
        self.start
    }

    /// Returns the visible leaf ids covered by this cell.
    pub fn leaf_column_ids(&self) -> &[TableColumnId] {
        &self.leaf_column_ids
    }

    /// Returns direct child header ids.
    pub fn sub_header_ids(&self) -> &[String] {
        &self.sub_header_ids
    }

    /// Returns the resolved sort direction for leaf headers.
    pub const fn sort_direction(&self) -> Option<TableSortDirection> {
        self.sort_direction
    }

    /// Returns the emitted sort action for leaf headers.
    pub const fn sort_action(&self) -> Option<&TableHeaderAction> {
        self.sort_action.as_ref()
    }

    /// Returns whether this leaf header is resizable.
    pub const fn resizable(&self) -> bool {
        self.resizable
    }
}

/// One header row in a render region.
#[derive(Debug, Clone, PartialEq)]
pub struct TableHeaderGroupRenderPlan {
    id: String,
    region: TableColumnRegion,
    depth: usize,
    headers: Vec<TableHeaderCellRenderPlan>,
    total_width: UiPx,
}

impl TableHeaderGroupRenderPlan {
    fn from_resolved(
        table_id: &str,
        region: TableColumnRegion,
        group: &TableResolvedHeaderGroup,
        columns_by_id: &BTreeMap<TableColumnId, &TableColumnRenderPlan>,
    ) -> Self {
        let headers = group
            .headers()
            .iter()
            .map(|cell| TableHeaderCellRenderPlan::from_resolved(table_id, cell, columns_by_id))
            .collect::<Vec<_>>();
        let total_width = headers
            .iter()
            .fold(UiPx::ZERO, |total, header| total + header.width());

        Self {
            id: format!(
                "table:{}:header-group:{}:{}",
                table_id,
                region.as_str(),
                group.depth()
            ),
            region,
            depth: group.depth(),
            headers,
            total_width,
        }
    }

    /// Returns the stable render identity for this header row.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the render region for this header row.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns the row depth.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the header cells in this row.
    pub fn headers(&self) -> &[TableHeaderCellRenderPlan] {
        &self.headers
    }

    /// Returns the summed width of this header row.
    pub const fn total_width(&self) -> UiPx {
        self.total_width
    }
}

/// Header rows for one render region.
#[derive(Debug, Clone, PartialEq)]
pub struct TableHeaderGroupRegionRenderPlan {
    region: TableColumnRegion,
    groups: Vec<TableHeaderGroupRenderPlan>,
    total_width: UiPx,
}

impl TableHeaderGroupRegionRenderPlan {
    fn from_resolved(
        table_id: &str,
        region: TableColumnRegion,
        groups: &[TableResolvedHeaderGroup],
        columns_by_id: &BTreeMap<TableColumnId, &TableColumnRenderPlan>,
        total_width: UiPx,
    ) -> Self {
        let groups = groups
            .iter()
            .map(|group| {
                TableHeaderGroupRenderPlan::from_resolved(table_id, region, group, columns_by_id)
            })
            .collect::<Vec<_>>();

        Self {
            region,
            groups,
            total_width,
        }
    }

    /// Returns the render region.
    pub const fn region(&self) -> TableColumnRegion {
        self.region
    }

    /// Returns header rows in this region.
    pub fn groups(&self) -> &[TableHeaderGroupRenderPlan] {
        &self.groups
    }

    /// Returns the number of header rows in this region.
    pub fn header_row_count(&self) -> usize {
        self.groups.len()
    }

    /// Returns the summed width of this region.
    pub const fn total_width(&self) -> UiPx {
        self.total_width
    }

    /// Returns the header row at a given depth, if present.
    pub fn group_at_depth(&self, depth: usize) -> Option<&TableHeaderGroupRenderPlan> {
        self.groups.iter().find(|group| group.depth() == depth)
    }
}

/// Header rows split into render regions.
#[derive(Debug, Clone, PartialEq)]
pub struct TableHeaderGroupRegionsRenderPlan {
    left: TableHeaderGroupRegionRenderPlan,
    center: TableHeaderGroupRegionRenderPlan,
    right: TableHeaderGroupRegionRenderPlan,
}

impl TableHeaderGroupRegionsRenderPlan {
    fn from_resolved(
        table_id: &str,
        header_groups: &open_gpui_ui_core::TableResolvedHeaderGroupRegions,
        columns: &[TableColumnRenderPlan],
        column_regions: &[TableColumnRegionRenderPlan],
    ) -> Self {
        let columns_by_id = columns
            .iter()
            .map(|column| (column.id().clone(), column))
            .collect::<BTreeMap<_, _>>();
        let region_width = |region: TableColumnRegion| {
            column_regions
                .iter()
                .find(|plan| plan.region() == region)
                .map(TableColumnRegionRenderPlan::total_width)
                .unwrap_or(UiPx::ZERO)
        };

        Self {
            left: TableHeaderGroupRegionRenderPlan::from_resolved(
                table_id,
                TableColumnRegion::Left,
                header_groups.left(),
                &columns_by_id,
                region_width(TableColumnRegion::Left),
            ),
            center: TableHeaderGroupRegionRenderPlan::from_resolved(
                table_id,
                TableColumnRegion::Center,
                header_groups.center(),
                &columns_by_id,
                region_width(TableColumnRegion::Center),
            ),
            right: TableHeaderGroupRegionRenderPlan::from_resolved(
                table_id,
                TableColumnRegion::Right,
                header_groups.right(),
                &columns_by_id,
                region_width(TableColumnRegion::Right),
            ),
        }
    }

    /// Returns the left-pinned header rows.
    pub fn left(&self) -> &TableHeaderGroupRegionRenderPlan {
        &self.left
    }

    /// Returns the center header rows.
    pub fn center(&self) -> &TableHeaderGroupRegionRenderPlan {
        &self.center
    }

    /// Returns the right-pinned header rows.
    pub fn right(&self) -> &TableHeaderGroupRegionRenderPlan {
        &self.right
    }

    /// Returns header rows for a region.
    pub fn region(&self, region: TableColumnRegion) -> &TableHeaderGroupRegionRenderPlan {
        match region {
            TableColumnRegion::Left => self.left(),
            TableColumnRegion::Center => self.center(),
            TableColumnRegion::Right => self.right(),
        }
    }

    /// Returns the maximum header row count across regions.
    pub fn row_count(&self) -> usize {
        self.left
            .header_row_count()
            .max(self.center.header_row_count())
            .max(self.right.header_row_count())
    }

    /// Returns whether no header rows exist.
    pub fn is_empty(&self) -> bool {
        self.row_count() == 0
    }

    /// Returns a shared header row at the given depth for a region family.
    pub fn group_at_depth(
        &self,
        region: TableColumnRegion,
        depth: usize,
    ) -> Option<&TableHeaderGroupRenderPlan> {
        self.region(region).group_at_depth(depth)
    }
}

fn header_cell_render_id(table_id: &str, cell: &TableResolvedHeaderCell) -> String {
    match cell.kind() {
        TableResolvedHeaderKind::Leaf => {
            format!("table:{table_id}:header:{}", cell.source_id())
        }
        TableResolvedHeaderKind::Group => format!(
            "table:{table_id}:header-group:{}:{}:{}",
            cell.region().as_str(),
            cell.depth(),
            cell.source_id()
        ),
        TableResolvedHeaderKind::Placeholder => format!(
            "table:{table_id}:header-placeholder:{}:{}:{}",
            cell.region().as_str(),
            cell.depth(),
            cell.placeholder_id().unwrap_or(cell.source_id())
        ),
    }
}

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
    value: Option<TableCellValue>,
    text: String,
    select_options: Vec<TableSelectOption>,
    region: TableColumnRegion,
    aria_column_index: usize,
    role: Role,
    width: UiPx,
    editor: Option<TableCellEditor>,
}

impl TableCellRenderPlan {
    fn new(
        column: &TableColumnRenderPlan,
        row: &TableResolvedRow,
        value: Option<&TableCellValue>,
    ) -> Self {
        let value = value.cloned();
        let editor = if row.is_leaf() {
            match (column.editor(), value.as_ref()) {
                (Some(TableCellEditor::Checkbox), Some(TableCellValue::Bool(_))) => {
                    Some(TableCellEditor::Checkbox)
                }
                (Some(TableCellEditor::Select), Some(_)) => Some(TableCellEditor::Select),
                (Some(TableCellEditor::Text), Some(_))
                | (Some(TableCellEditor::MultilineText { .. }), Some(_)) => column.editor(),
                _ => None,
            }
        } else {
            None
        };
        let select_options = if matches!(editor, Some(TableCellEditor::Select)) {
            column.select_options().to_vec()
        } else {
            Vec::new()
        };
        let text = resolved_table_cell_text(value.as_ref(), &select_options);
        Self {
            column_id: column.id().clone(),
            value,
            text,
            select_options,
            region: column.region(),
            aria_column_index: column.aria_column_index(),
            role: Role::Cell,
            width: column.width(),
            editor,
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

    /// Returns the select options configured for this resolved leaf cell.
    pub fn select_options(&self) -> &[TableSelectOption] {
        &self.select_options
    }

    /// Returns the resolved scalar value for this cell, when present.
    pub fn value(&self) -> Option<&TableCellValue> {
        self.value.as_ref()
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

    /// Returns whether this resolved leaf cell should render an editor.
    pub const fn text_editable(&self) -> bool {
        self.editor.is_some()
    }

    /// Returns the editor configured for this resolved leaf cell.
    pub const fn editor(&self) -> Option<TableCellEditor> {
        self.editor
    }
}

fn resolved_table_cell_text(
    value: Option<&TableCellValue>,
    select_options: &[TableSelectOption],
) -> String {
    let Some(value) = value else {
        return String::new();
    };

    let raw_text = value.filter_text();
    if select_options.is_empty() {
        return raw_text;
    }

    select_options
        .iter()
        .find(|option| option.value() == raw_text)
        .map(|option| option.label().to_owned())
        .unwrap_or(raw_text)
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
    row_measure_mode: TableRowMeasureMode,
    table: Rc<TableResolvedState>,
    virtualizer: VirtualizerResolvedState,
    content_fit_widths: BTreeMap<TableColumnId, UiPx>,
    columns: Vec<TableColumnRenderPlan>,
    column_regions: Vec<TableColumnRegionRenderPlan>,
    header_groups: TableHeaderGroupRegionsRenderPlan,
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
        row_measure_mode: TableRowMeasureMode,
        state: &TableState,
        table: Rc<TableResolvedState>,
        virtualizer: VirtualizerResolvedState,
        columns: Vec<TableColumnRenderPlan>,
        content_fit_widths: BTreeMap<TableColumnId, UiPx>,
        center_scroll_offset: Option<UiPx>,
        center_viewport_extent: Option<UiPx>,
        row_measurements: &BTreeMap<String, UiPx>,
    ) -> Self {
        let columns =
            apply_table_content_fit_widths(columns, &content_fit_widths, state.column_sizing());
        let column_regions = resolve_column_region_render_plans(&columns);
        let header_groups = TableHeaderGroupRegionsRenderPlan::from_resolved(
            &table_id,
            table.header_groups(),
            &columns,
            &column_regions,
        );
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
        let top_rows = row_render_plans(
            table.top_rows(),
            TableRowRegion::Top,
            row_measure_mode,
            row_measurements,
            metrics.row_height(),
            &columns,
            &duplicate_row_ids,
            0,
            UiPx::ZERO,
        );
        let rows = virtualized_center_row_render_plans(
            table.center_rows(),
            virtualizer.items(),
            &columns,
            &duplicate_row_ids,
            top_row_count,
        );
        let top_height = top_rows
            .iter()
            .fold(UiPx::ZERO, |total, row| total + row.virtual_size());
        let bottom_rows = row_render_plans(
            table.bottom_rows(),
            TableRowRegion::Bottom,
            row_measure_mode,
            row_measurements,
            metrics.row_height(),
            &columns,
            &duplicate_row_ids,
            top_row_count + center_total_row_count,
            top_height + virtualizer.total_size(),
        );
        let pagination = state.pagination();
        let selection_summary = table.final_selection_summary();

        Self {
            table_id,
            label,
            metrics,
            row_measure_mode,
            table,
            virtualizer,
            content_fit_widths,
            columns,
            column_regions,
            header_groups,
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

    /// Returns the row height ownership mode.
    pub const fn row_measure_mode(&self) -> TableRowMeasureMode {
        self.row_measure_mode
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

    /// Returns resolved facet metadata for the global filter context.
    pub fn global_facet_summary(&self) -> &TableGlobalFacetSummary {
        self.table.global_facet_summary()
    }

    /// Returns the resolved renderer-neutral virtualizer state.
    pub const fn virtualizer(&self) -> &VirtualizerResolvedState {
        &self.virtualizer
    }

    /// Returns visible columns in render order.
    pub fn columns(&self) -> &[TableColumnRenderPlan] {
        &self.columns
    }

    /// Returns the measured content-fit widths that informed this render plan.
    pub fn content_fit_widths(&self) -> &BTreeMap<TableColumnId, UiPx> {
        &self.content_fit_widths
    }

    /// Returns visible columns split into render regions.
    pub fn column_regions(&self) -> &[TableColumnRegionRenderPlan] {
        &self.column_regions
    }

    /// Returns nested header groups split into render regions.
    pub fn header_groups(&self) -> &TableHeaderGroupRegionsRenderPlan {
        &self.header_groups
    }

    /// Returns left-pinned header rows.
    pub fn left_header_groups(&self) -> &TableHeaderGroupRegionRenderPlan {
        self.header_groups.left()
    }

    /// Returns center header rows.
    pub fn center_header_groups(&self) -> &TableHeaderGroupRegionRenderPlan {
        self.header_groups.center()
    }

    /// Returns right-pinned header rows.
    pub fn right_header_groups(&self) -> &TableHeaderGroupRegionRenderPlan {
        self.header_groups.right()
    }

    /// Returns the maximum header row count across all regions.
    pub fn header_row_count(&self) -> usize {
        self.header_groups.row_count()
    }

    /// Returns the total height reserved for the table header band.
    pub fn sticky_header_band_height(&self) -> UiPx {
        self.metrics.header_height() * self.header_row_count().max(1) as f32
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

fn row_render_plans(
    rows: &[TableResolvedRow],
    region: TableRowRegion,
    row_measure_mode: TableRowMeasureMode,
    row_measurements: &BTreeMap<String, UiPx>,
    fallback_row_height: UiPx,
    columns: &[TableColumnRenderPlan],
    duplicate_row_ids: &BTreeSet<TableRowId>,
    model_index_start: usize,
    start_offset: UiPx,
) -> Vec<TableRowRenderPlan> {
    let mut cursor = start_offset;
    rows.iter()
        .enumerate()
        .map(|(region_index, row)| {
            let row = row.clone();
            let render_key = row_render_key(&row, duplicate_row_ids);
            let model_index = model_index_start + region_index;
            let row_height = if row_measure_mode.measured() {
                row_measurements
                    .get(&render_key)
                    .copied()
                    .unwrap_or(fallback_row_height)
            } else {
                fallback_row_height
            };
            let measured =
                row_measure_mode.measured() && row_measurements.contains_key(&render_key);
            let measurement = VirtualizerItemMeasurement::new(
                region_index,
                VirtualizerItemKey::new(render_key.clone()),
                cursor,
                row_height,
                measured,
            );
            cursor = measurement.end();
            TableRowRenderPlan::new(row, region, render_key, model_index, measurement, columns)
        })
        .collect()
}

fn measured_virtualizer_state(
    rows: &[TableResolvedRow],
    row_measure_mode: TableRowMeasureMode,
    row_measurements: &BTreeMap<String, UiPx>,
    fallback_row_height: UiPx,
    overscan: usize,
    scroll_offset: UiPx,
    viewport_extent: UiPx,
    duplicate_row_ids: &BTreeSet<TableRowId>,
) -> VirtualizerResolvedState {
    let mut state = VirtualizerState::new(rows.len(), fallback_row_height)
        .with_viewport_extent(viewport_extent)
        .with_overscan(overscan)
        .with_scroll_offset(scroll_offset);

    let item_keys = rows
        .iter()
        .map(|row| VirtualizerItemKey::new(row_render_key(row, duplicate_row_ids)))
        .collect::<Vec<_>>();
    state = state.with_item_keys(item_keys);

    if row_measure_mode.measured() {
        return state.resolve_known_size_window(|index| {
            let row = &rows[index];
            let render_key = row_render_key(row, duplicate_row_ids);
            (
                VirtualizerItemKey::new(render_key.clone()),
                row_measurements
                    .get(&render_key)
                    .copied()
                    .unwrap_or(fallback_row_height),
            )
        });
    }

    state.resolve_fixed_window(|index| {
        let row = &rows[index];
        VirtualizerItemKey::new(row_render_key(row, duplicate_row_ids))
    })
}

fn table_rows_virtual_size(rows: &[TableRowRenderPlan]) -> UiPx {
    rows.iter()
        .fold(UiPx::ZERO, |total, row| total + row.virtual_size())
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
    content_fit: TableContentFitMeasureCache,
    row_measurements: BTreeMap<String, UiPx>,
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

    fn set_row_measurement(&mut self, render_key: String, height: UiPx, cx: &mut Context<Self>) {
        let height = nonnegative_px(height);
        if self.row_measurements.get(&render_key).copied() != Some(height) {
            self.row_measurements.insert(render_key, height);
            cx.notify();
        }
    }

    fn clear_row_measurements(&mut self) {
        self.row_measurements.clear();
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct TableColumnOrderDrag {
    table_id: String,
    column_id: TableColumnId,
    region: TableColumnRegion,
}

/// Body row height ownership for table rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableRowMeasureMode {
    /// Body rows keep the shared fixed height contract.
    #[default]
    Fixed,
    /// Body rows may grow to fit their rendered content and feed measurements back into the virtualizer.
    Measured,
}

impl TableRowMeasureMode {
    /// Returns whether the table should measure row heights from rendered content.
    pub const fn measured(self) -> bool {
        matches!(self, Self::Measured)
    }
}

/// A concrete GPUI table renderer using the Open GPUI row-model and virtualizer contracts.
#[derive(IntoElement)]
pub struct Table {
    id: String,
    label: SharedString,
    state: TableState,
    metrics: TableMetrics,
    row_measure_mode: TableRowMeasureMode,
    snapshot: Option<VirtualizerSnapshot>,
    default_focused_row: Option<TableRowId>,
    on_sort_requested: Option<TableSortHandler>,
    on_column_order_change: Option<TableColumnOrderHandler>,
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
            row_measure_mode: TableRowMeasureMode::default(),
            snapshot: None,
            default_focused_row: None,
            on_sort_requested: None,
            on_column_order_change: None,
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

    /// Applies the body row height ownership mode.
    pub fn row_measure_mode(mut self, row_measure_mode: TableRowMeasureMode) -> Self {
        self.row_measure_mode = row_measure_mode;
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

    /// Registers a handler for controlled column reorder changes.
    pub fn on_column_order_change(
        mut self,
        handler: impl Fn(TableColumnOrderChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_column_order_change = Some(Rc::new(handler));
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
        let duplicate_row_ids = table
            .duplicate_row_ids()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let virtualizer = if self.row_measure_mode.measured() {
            measured_virtualizer_state(
                table.center_rows(),
                self.row_measure_mode,
                &BTreeMap::new(),
                metrics.row_height(),
                metrics.overscan(),
                nonnegative_px(scroll_offset),
                metrics.viewport_extent(),
                &duplicate_row_ids,
            )
        } else {
            self.resolve_virtualizer(&table, metrics, scroll_offset)
        };

        TableRenderPlan::resolve(
            self.id.clone(),
            self.label.to_string(),
            metrics,
            self.row_measure_mode,
            &self.state,
            table,
            virtualizer,
            columns,
            BTreeMap::new(),
            None,
            None,
            &BTreeMap::new(),
        )
    }

    fn render_plan_with_runtime(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
        horizontal_scroll_handle: ScrollHandle,
        window: &Window,
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
            runtime.clear_row_measurements();
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
        let virtualizer = if self.row_measure_mode.measured() {
            measured_virtualizer_state(
                cache.table.center_rows(),
                self.row_measure_mode,
                &runtime.row_measurements,
                metrics.row_height(),
                metrics.overscan(),
                nonnegative_px(scroll_offset),
                metrics.viewport_extent(),
                &cache.table.duplicate_row_ids().iter().cloned().collect(),
            )
        } else {
            self.resolve_virtualizer(&cache.table, metrics, scroll_offset)
        };
        let rendered_rows = table_content_fit_rendered_rows(&cache.table, &virtualizer);
        let center_scroll_offset =
            ui_px((-ui_px_from_gpui(horizontal_scroll_handle.offset().x).as_f32()).max(0.0));
        let center_viewport_extent = ui_px_from_gpui(horizontal_scroll_handle.bounds().size.width);
        let center_viewport_extent =
            (center_viewport_extent.as_f32() > 0.0).then_some(center_viewport_extent);
        let center_scroll_offset = center_viewport_extent.map(|_| center_scroll_offset);
        let content_fit_widths = runtime
            .content_fit
            .widths_for(
                content_fit_measure_key(
                    cache.key.clone(),
                    metrics,
                    &cache.columns,
                    &rendered_rows,
                    window,
                ),
                &cache.columns,
                &rendered_rows,
                metrics,
                window,
            )
            .clone();

        TableRenderPlan::resolve(
            self.id.clone(),
            self.label.to_string(),
            metrics,
            self.row_measure_mode,
            &state,
            cache.table.clone(),
            virtualizer,
            cache.columns.clone(),
            content_fit_widths,
            center_scroll_offset,
            center_viewport_extent,
            &runtime.row_measurements,
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

fn apply_table_content_fit_widths(
    columns: Vec<TableColumnRenderPlan>,
    measured_widths: &BTreeMap<TableColumnId, UiPx>,
    committed_sizing: &TableColumnSizing,
) -> Vec<TableColumnRenderPlan> {
    let columns = columns
        .into_iter()
        .map(|column| {
            if column.width_policy() != TableColumnWidthPolicy::ContentFit
                || committed_sizing.width(column.id()).is_some()
            {
                return column;
            }

            match measured_widths.get(column.id()).copied() {
                Some(width) => column.with_width(width),
                None => column,
            }
        })
        .collect::<Vec<_>>();

    resolve_table_column_offsets(columns)
}

fn resolve_table_column_offsets(columns: Vec<TableColumnRenderPlan>) -> Vec<TableColumnRenderPlan> {
    let region_totals = TableColumnRegion::ALL
        .into_iter()
        .map(|region| {
            let total = columns
                .iter()
                .filter(|column| column.region() == region)
                .fold(UiPx::ZERO, |total, column| total + column.width());
            (region, total)
        })
        .collect::<BTreeMap<_, _>>();
    let mut region_starts = TableColumnRegion::ALL
        .into_iter()
        .map(|region| (region, UiPx::ZERO))
        .collect::<BTreeMap<_, _>>();

    columns
        .into_iter()
        .map(|column| {
            let region = column.region();
            let start = region_starts.get(&region).copied().unwrap_or(UiPx::ZERO);
            let total_width = region_totals.get(&region).copied().unwrap_or(UiPx::ZERO);
            let after = nonnegative_px(total_width - start - column.width());
            region_starts.insert(region, start + column.width());
            column.with_offsets(start, after)
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
            content_fit: TableContentFitMeasureCache::default(),
            row_measurements: BTreeMap::new(),
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
        let on_column_order_change = self.on_column_order_change.clone();
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
                window,
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
            .relative()
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
            .child(render_table_body(
                &plan,
                scroll_viewport_id,
                horizontal_scroll_handle.clone(),
                scroll_handle.clone(),
                plan.sticky_header_band_height(),
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
            .child(render_table_header(
                &plan,
                on_sort_requested,
                on_column_order_change,
                resize_config,
                horizontal_scroll_handle.clone(),
                plan.sticky_header_band_height(),
            ))
    }
}

fn render_table_header(
    plan: &TableRenderPlan,
    on_sort_requested: Option<TableSortHandler>,
    on_column_order_change: Option<TableColumnOrderHandler>,
    resize_config: TableResizeRenderConfig,
    horizontal_scroll_handle: ScrollHandle,
    header_band_height: UiPx,
) -> impl IntoElement {
    let table_id = plan.table_id().to_owned();
    let metrics = plan.metrics();
    let column_header_role = plan.column_header_role();
    let regions = plan.column_regions().to_vec();
    let header_groups = plan.header_groups().clone();
    let columns_by_id = Rc::new(
        plan.columns()
            .iter()
            .cloned()
            .map(|column| (column.id().clone(), column))
            .collect::<BTreeMap<_, _>>(),
    );
    let pinned_layout = plan.pinned_layout().cloned();
    let center_window = if pinned_layout.is_some() {
        plan.center_column_window().cloned().map(Rc::new)
    } else {
        None
    };
    let rendered_center_leaf_ids = center_window.as_ref().map(|window| {
        window
            .rendered_columns()
            .iter()
            .map(|column| column.id().clone())
            .collect::<BTreeSet<_>>()
    });
    div()
        .id(format!("table:{table_id}:header-row"))
        .debug_selector({
            let table_id = table_id.clone();
            move || format!("table:{table_id}:header-row")
        })
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .h(gpui_px_from_ui(header_band_height))
        .flex()
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xf3f4ef))
        .ui_role(plan.row_role())
        .aria_row_index(1)
        .children(regions.iter().map(move |region_plan| {
            let table_id = table_id.clone();
            let pinned_layout = pinned_layout.clone();
            let center_window = center_window.clone();
            let on_sort_requested = on_sort_requested.clone();
            let on_column_order_change = on_column_order_change.clone();
            let resize_config = resize_config.clone();
            let header_groups = header_groups.clone();
            let columns_by_id = columns_by_id.clone();
            let rendered_center_leaf_ids = rendered_center_leaf_ids.clone();
            let metrics = metrics;
            let column_header_role = column_header_role;
            let region = region_plan.region();
            let region_name = region.as_str().to_owned();
            let active_center_window = (region == TableColumnRegion::Center)
                .then_some(center_window.as_deref())
                .flatten();
            let region_width = active_center_window
                .map(TableCenterColumnWindowPlan::center_width)
                .unwrap_or_else(|| region_plan.total_width());
            let header_region = header_groups.region(region);
            let reorder_enabled =
                on_column_order_change.is_some() && region_plan.columns().len() > 1;
            let mut occupied_leaf_ids = BTreeSet::new();
            let mut header_children = Vec::new();
            for group in header_region.groups() {
                for header in group.headers() {
                    let effective_leaf_ids = header
                        .leaf_column_ids()
                        .iter()
                        .filter(|leaf_id| {
                            if region == TableColumnRegion::Center {
                                rendered_center_leaf_ids
                                    .as_ref()
                                    .is_none_or(|rendered| rendered.contains(*leaf_id))
                            } else {
                                true
                            }
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    if effective_leaf_ids.is_empty() {
                        continue;
                    }
                    if header.kind().is_placeholder()
                        && effective_leaf_ids
                            .iter()
                            .all(|leaf_id| occupied_leaf_ids.contains(leaf_id))
                    {
                        continue;
                    }

                    header_children.push(
                        render_table_header_group_cell(
                            table_id.clone(),
                            metrics,
                            column_header_role,
                            header.clone(),
                            effective_leaf_ids.clone(),
                            columns_by_id.clone(),
                            on_sort_requested.clone(),
                            on_column_order_change.clone(),
                            reorder_enabled,
                            resize_config.clone(),
                        )
                        .into_any_element(),
                    );
                    if header.kind().is_leaf() {
                        occupied_leaf_ids.extend(effective_leaf_ids);
                    }
                }
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
                .relative()
                .h_full()
                .min_w(px(0.0))
                .w(gpui_px_from_ui(region_width))
                .flex_none()
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

fn render_table_header_group_cell(
    table_id: String,
    metrics: TableMetrics,
    column_header_role: Role,
    header: TableHeaderCellRenderPlan,
    effective_leaf_ids: Vec<TableColumnId>,
    columns_by_id: Rc<BTreeMap<TableColumnId, TableColumnRenderPlan>>,
    on_sort_requested: Option<TableSortHandler>,
    on_column_order_change: Option<TableColumnOrderHandler>,
    reorder_enabled: bool,
    resize_config: TableResizeRenderConfig,
) -> AnyElement {
    let header_id = header.id().to_owned();
    let header_kind = header.kind();
    let header_label = header.label().to_owned();
    let is_leaf = header_kind.is_leaf();
    let interactive_sort = is_leaf
        .then(|| header.sort_action().cloned().zip(on_sort_requested))
        .flatten();
    let leaf_column = is_leaf
        .then(|| {
            effective_leaf_ids
                .first()
                .and_then(|column_id| columns_by_id.get(column_id))
                .cloned()
        })
        .flatten();
    let order_drag = reorder_enabled
        .then(|| {
            leaf_column.clone().map(|column| TableColumnOrderDrag {
                table_id: table_id.clone(),
                column_id: column.id().clone(),
                region: column.region(),
            })
        })
        .flatten();
    let order_drop_target = reorder_enabled.then(|| leaf_column.clone()).flatten();
    let order_drop_handler = reorder_enabled.then_some(on_column_order_change).flatten();
    let show_resize_handle = resize_config.enabled && header.resizable();
    let row_span = header.row_span().max(1) as f32;
    let width = effective_leaf_ids
        .iter()
        .fold(UiPx::ZERO, |total, column_id| {
            total
                + columns_by_id
                    .get(column_id)
                    .map(|column| column.width())
                    .unwrap_or(UiPx::ZERO)
        });
    let start = effective_leaf_ids
        .first()
        .and_then(|column_id| columns_by_id.get(column_id))
        .map(|column| column.start())
        .unwrap_or(UiPx::ZERO);
    let aria_column_index = effective_leaf_ids
        .first()
        .and_then(|column_id| columns_by_id.get(column_id))
        .map(|column| column.aria_column_index())
        .unwrap_or(1);
    let sort_suffix = header
        .sort_direction()
        .map(|direction| match direction {
            TableSortDirection::Ascending => " ↑",
            TableSortDirection::Descending => " ↓",
        })
        .unwrap_or("");

    div()
        .id(header_id.clone())
        .debug_selector(move || header_id.clone())
        .absolute()
        .top(gpui_px_from_ui(
            metrics.header_height() * header.depth() as f32,
        ))
        .left(gpui_px_from_ui(start))
        .w(gpui_px_from_ui(width))
        .min_w(gpui_px_from_ui(width))
        .max_w(gpui_px_from_ui(width))
        .flex_none()
        .h(gpui_px_from_ui(metrics.header_height() * row_span))
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
        .aria_label(header.label().to_owned())
        .aria_column_index(aria_column_index)
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
        .child(format!("{}{}", header_label, sort_suffix))
        .when_some(order_drag, |this, drag| {
            this.cursor(CursorStyle::OpenHand)
                .on_drag(drag, |_, _, _, _, cx| cx.new(|_| Empty))
        })
        .when_some(order_drop_target, |this, column| {
            this.when_some(order_drop_handler.clone(), |this, handler| {
                let drop_handle_inset = if show_resize_handle {
                    px(10.0)
                } else {
                    px(0.0)
                };
                let drop_zone_width = px((width.as_f32() * 0.5).max(12.0));

                this.child(render_table_column_order_drop_zone(
                    table_id.clone(),
                    column.clone(),
                    TableColumnOrderPlacement::Before,
                    handler.clone(),
                    drop_zone_width,
                    drop_handle_inset,
                ))
                .child(render_table_column_order_drop_zone(
                    table_id.clone(),
                    column,
                    TableColumnOrderPlacement::After,
                    handler,
                    drop_zone_width,
                    drop_handle_inset,
                ))
            })
        })
        .when(show_resize_handle, |this| {
            this.when_some(leaf_column.clone(), |this, column| {
                this.child(render_table_resize_handle(
                    table_id.clone(),
                    column,
                    resize_config.clone(),
                ))
            })
        })
        .into_any_element()
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

fn render_table_column_order_drop_zone(
    table_id: String,
    target_column: TableColumnRenderPlan,
    placement: TableColumnOrderPlacement,
    handler: TableColumnOrderHandler,
    zone_width: Pixels,
    right_inset: Pixels,
) -> impl IntoElement {
    let target_column_id = target_column.id().clone();
    let target_region = target_column.region();
    let zone_key = target_column_id.as_str().to_owned();
    let placement_key = placement.as_str().to_owned();
    let table_for_can_drop = table_id.clone();
    let table_for_drag_over = table_id.clone();
    let table_for_drop = table_id.clone();
    let target_for_can_drop = target_column_id.clone();
    let target_for_drag_over = target_column_id.clone();
    let target_for_drop = target_column_id.clone();

    div()
        .id(format!(
            "table:{table_id}:header-order-drop:{placement_key}:{zone_key}"
        ))
        .debug_selector(move || {
            format!("table:{table_id}:header-order-drop:{placement_key}:{zone_key}")
        })
        .absolute()
        .top(px(0.0))
        .bottom(px(0.0))
        .when(placement == TableColumnOrderPlacement::Before, |this| {
            this.left(px(0.0)).w(zone_width)
        })
        .when(placement == TableColumnOrderPlacement::After, |this| {
            this.right(right_inset).w(zone_width)
        })
        .can_drop(move |dragged, _, _| {
            dragged
                .downcast_ref::<TableColumnOrderDrag>()
                .is_some_and(|drag| {
                    drag.table_id == table_for_can_drop
                        && drag.region == target_region
                        && drag.column_id != target_for_can_drop
                })
        })
        .drag_over::<TableColumnOrderDrag>(move |style, drag, _, _| {
            if drag.table_id != table_for_drag_over
                || drag.region != target_region
                || drag.column_id == target_for_drag_over
            {
                return style;
            }

            style.bg(rgba(0x1f7a662e))
        })
        .on_drop(move |drag: &TableColumnOrderDrag, window, cx| {
            if drag.table_id != table_for_drop
                || drag.region != target_region
                || drag.column_id == target_for_drop
            {
                return;
            }

            let change = match placement {
                TableColumnOrderPlacement::Before => TableColumnOrderChange::move_before(
                    drag.column_id.clone(),
                    target_column_id.clone(),
                    drag.region,
                ),
                TableColumnOrderPlacement::After => TableColumnOrderChange::move_after(
                    drag.column_id.clone(),
                    target_column_id.clone(),
                    drag.region,
                ),
            };
            handler(change, window, cx);
        })
        .into_any_element()
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
    header_band_height: UiPx,
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
    let top_height = table_rows_virtual_size(&top_rows);
    let center_height = plan.virtualizer().total_size();
    let bottom_height = table_rows_virtual_size(&bottom_rows);
    let measured_rows = plan.row_measure_mode().measured();

    div()
        .flex_1()
        .min_h(px(0.0))
        .overflow_hidden()
        .pt(gpui_px_from_ui(header_band_height))
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
                measured_rows,
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
                        measured_rows,
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
                measured_rows,
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
    measured_rows: bool,
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
                measured_rows,
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
    measured_rows: bool,
) -> impl IntoElement {
    let render_key = row.render_key().to_owned();
    let row_id = row.id().clone();
    let row_for_layout = row.clone();
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

    let row_element = div()
        .on_children_prepainted({
            let runtime = runtime.clone();
            let row_key = render_key.clone();
            move |row_bounds, _window, cx| {
                if measured_rows {
                    let measured_height = row_bounds
                        .iter()
                        .map(|bounds| bounds.size.height)
                        .fold(Pixels::ZERO, Pixels::max);
                    let measured_height = measured_height.ceil();
                    runtime.update(cx, |runtime, cx| {
                        runtime.set_row_measurement(
                            row_key.clone(),
                            ui_px_from_gpui(measured_height),
                            cx,
                        );
                    });
                }
            }
        })
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
        .min_w(px(0.0))
        .flex()
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
                if !event.standard_click() || window.default_prevented() {
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
                            measured_rows,
                        )
                        .into_any_element()
                    }
                }));
                if uses_center_window {
                    region_children.push(render_table_lane_spacer(trailing_spacer_width));
                }

                let mut region_lane = div()
                    .min_w(px(0.0))
                    .flex()
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
                    .children(region_children);

                region_lane = if measured_rows {
                    region_lane.items_start()
                } else {
                    region_lane.h_full().items_center()
                };

                let region_lane = region_lane.into_any_element();

                if let Some(center_scroll_id) = center_scroll_id {
                    div()
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
        .when(!measured_rows, |this| {
            this.h(gpui_px_from_ui(row_for_layout.virtual_size()))
        })
        .into_any_element();
    row_element
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
    measured_rows: bool,
) -> impl IntoElement {
    let column_id = cell.column_id().as_str().to_owned();
    let show_tree_affordance = tree_affordance && tree.is_some();
    let indent = ui_px(16.0) * tree_depth as f32;
    let mut content = Vec::new();
    if show_tree_affordance {
        content.push(
            div()
                .w(gpui_px_from_ui(indent))
                .when(!measured_rows, |this| this.h_full())
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
    let cell_value = cell.value().cloned();
    let cell_text = cell.text().to_owned();
    if let (Some(editor), Some(_)) = (cell.editor(), on_cell_edit_change.as_ref()) {
        let action = TableRowAction::from_render_plan(&row, TableInputModifiers::default());
        let column_id_for_change = cell.column_id().clone();
        let previous_value = cell_value.clone().unwrap_or_default();
        let select_options = cell
            .select_options()
            .iter()
            .map(|option| ListboxOption::new(option.value().to_owned(), option.label().to_owned()))
            .collect::<Vec<_>>();
        let selected_value = cell_value
            .as_ref()
            .map(TableCellValue::filter_text)
            .unwrap_or_default();
        let editor_id = format!("table:{table_id}:cell:{render_key}:{column_id}:editor");
        let editor_label = format!("Edit {column_id} for row {}", row.id().as_str());
        let editor_element = match editor {
            TableCellEditor::Text => {
                let on_change = on_cell_edit_change.clone();
                TextInput::new(editor_id, editor_label)
                    .value(cell_text)
                    .on_change(move |next_text, window, cx| {
                        if let Some(on_change) = on_change.as_ref() {
                            on_change(
                                TableCellEditChange::new(
                                    action.clone(),
                                    column_id_for_change.clone(),
                                    previous_value.clone(),
                                    next_text,
                                ),
                                window,
                                cx,
                            );
                        }
                    })
                    .with_size(metrics.size())
                    .into_any_element()
            }
            TableCellEditor::MultilineText { rows } => {
                let on_change = on_cell_edit_change.clone();
                Textarea::new(editor_id, editor_label)
                    .value(cell_text)
                    .rows(rows)
                    .on_change(move |next_text, window, cx| {
                        if let Some(on_change) = on_change.as_ref() {
                            on_change(
                                TableCellEditChange::new(
                                    action.clone(),
                                    column_id_for_change.clone(),
                                    previous_value.clone(),
                                    next_text,
                                ),
                                window,
                                cx,
                            );
                        }
                    })
                    .with_size(metrics.size())
                    .into_any_element()
            }
            TableCellEditor::Checkbox => {
                let on_change = on_cell_edit_change.clone();
                let checked = matches!(cell_value.as_ref(), Some(TableCellValue::Bool(true)));
                let editor_label = format!("Toggle {column_id} for row {}", row.id().as_str());
                Checkbox::new(editor_id)
                    .aria_label(editor_label)
                    .checked(checked)
                    .on_toggle(move |next_toggled, _, window, cx| {
                        if let Some(on_change) = on_change.as_ref() {
                            on_change(
                                TableCellEditChange::new(
                                    action.clone(),
                                    column_id_for_change.clone(),
                                    previous_value.clone(),
                                    matches!(next_toggled, Toggled::True),
                                ),
                                window,
                                cx,
                            );
                        }
                    })
                    .into_any_element()
            }
            TableCellEditor::Select => {
                let on_change = on_cell_edit_change.clone();
                Select::new(editor_id, editor_label)
                    .full_width(true)
                    .placeholder(cell_text.clone())
                    .selected(selected_value)
                    .options(select_options)
                    .on_select(move |selection, window, cx| {
                        if let Some(on_change) = on_change.as_ref() {
                            on_change(
                                TableCellEditChange::new(
                                    action.clone(),
                                    column_id_for_change.clone(),
                                    previous_value.clone(),
                                    TableCellValue::Text(selection.value().to_owned()),
                                ),
                                window,
                                cx,
                            );
                        }
                    })
                    .into_any_element()
            }
        };
        content.push(
            div()
                .id(format!(
                    "table:{table_id}:cell:{render_key}:{column_id}:editor-shell"
                ))
                .debug_selector({
                    let table_id = table_id.clone();
                    let render_key = render_key.clone();
                    let column_id = column_id.clone();
                    move || format!("table:{table_id}:cell:{render_key}:{column_id}:editor-shell")
                })
                .flex_1()
                .w_full()
                .min_w(px(0.0))
                .overflow_hidden()
                .block_mouse_except_scroll()
                .when(matches!(editor, TableCellEditor::Checkbox), |this| {
                    this.flex().justify_center().items_center()
                })
                .child(editor_element)
                .into_any_element(),
        );
    } else {
        content.push(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .when(measured_rows, |this| this.whitespace_normal())
                .when(!measured_rows, |this| this.truncate())
                .child(cell_text)
                .into_any_element(),
        );
    }

    let cell = div()
        .id(format!("table:{table_id}:cell:{render_key}:{column_id}"))
        .debug_selector(move || format!("table:{table_id}:cell:{render_key}:{column_id}"))
        .w(gpui_px_from_ui(cell.width()))
        .flex_none()
        .flex()
        .when(!measured_rows, |this| this.h_full().items_center())
        .px(gpui_px_from_ui(metrics.cell_padding_x()))
        .border_r_1()
        .border_color(rgb(0xe7e9e1))
        .text_xs()
        .text_color(rgb(0x2f3845))
        .ui_role(cell.role())
        .aria_column_index(cell.aria_column_index())
        .children(content)
        .when(measured_rows, |this| this.whitespace_normal())
        .when(!measured_rows, |this| this.truncate().whitespace_nowrap());

    cell.into_any_element()
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
            if !event.standard_click() || window.default_prevented() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui_ui_core::{TableColumnPinning, TableRow};

    #[test]
    fn apply_table_content_fit_widths_keeps_committed_widths_authoritative() {
        let committed_width = ui_px(72.0);
        let column = TableColumnRenderPlan {
            id: TableColumnId::new("status"),
            label: "Status".to_owned(),
            region: TableColumnRegion::Center,
            aria_column_index: 1,
            sortable: false,
            editor: None,
            select_options: Vec::new(),
            width_policy: TableColumnWidthPolicy::ContentFit,
            sort_direction: None,
            sort_action: None,
            width: committed_width,
            min_width: ui_px(10.0),
            max_width: ui_px(240.0),
            start: UiPx::ZERO,
            after: UiPx::ZERO,
            resizable: true,
        };

        let measured_widths = BTreeMap::from([(TableColumnId::new("status"), ui_px(128.0))]);
        let committed_sizing = TableColumnSizing::new().with_width("status", committed_width);
        let columns =
            apply_table_content_fit_widths(vec![column], &measured_widths, &committed_sizing);

        assert_eq!(columns[0].width(), committed_width);
        assert_eq!(columns[0].start(), UiPx::ZERO);
        assert_eq!(columns[0].after(), UiPx::ZERO);
    }

    #[test]
    fn table_column_order_change_reorders_leaf_columns_without_touching_other_state() {
        let state = TableState::new([TableRow::new("row-a")
            .with_cell("name", "Alpha")
            .with_cell("team", "UI")
            .with_cell("score", 42_usize)])
        .with_columns([
            TableColumn::new("name", "Name"),
            TableColumn::new("team", "Team"),
            TableColumn::new("score", "Score"),
        ])
        .with_column_order(["name", "team", "score"])
        .with_sorting([TableSort::descending("score")])
        .with_column_pinning(TableColumnPinning::new().pinned_left(["name"]));

        let change =
            TableColumnOrderChange::move_before("score", "team", TableColumnRegion::Center);
        let next = change.apply_to(state.clone());

        assert_eq!(
            next.column_order()
                .iter()
                .map(|column_id| column_id.as_str())
                .collect::<Vec<_>>(),
            ["name", "score", "team"]
        );
        assert_eq!(next.sorting(), state.sorting());
        assert_eq!(next.column_pinning(), state.column_pinning());
        assert_eq!(
            change.apply_to_order(state.column_order().iter().cloned()),
            vec![
                TableColumnId::new("name"),
                TableColumnId::new("score"),
                TableColumnId::new("team"),
            ]
        );
    }

    #[test]
    fn measured_virtualizer_uses_cached_row_heights_for_known_rows() {
        let resolved = TableState::new([
            TableRow::new("row-a").with_cell("name", "Alpha"),
            TableRow::new("row-b").with_cell("name", "Beta"),
            TableRow::new("row-c").with_cell("name", "Gamma"),
        ])
        .with_columns([TableColumn::new("name", "Name")])
        .resolve();
        let rows = resolved.center_rows();
        let duplicate_row_ids = resolved
            .duplicate_row_ids()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let measurements = BTreeMap::from([
            (rows[0].id().as_str().to_owned(), ui_px(18.0)),
            (rows[1].id().as_str().to_owned(), ui_px(28.0)),
        ]);

        let resolved = measured_virtualizer_state(
            &rows,
            TableRowMeasureMode::Measured,
            &measurements,
            ui_px(20.0),
            2,
            ui_px(0.0),
            ui_px(60.0),
            &duplicate_row_ids,
        );

        assert_eq!(resolved.total_size(), ui_px(66.0));
        assert_eq!(*resolved.visible_range(), VirtualizerRange::new(0, 3));
        assert_eq!(resolved.items().len(), 3);
        assert_eq!(resolved.measurements()[0].size(), ui_px(18.0));
        assert_eq!(resolved.measurements()[1].size(), ui_px(28.0));
        assert_eq!(resolved.measurements()[2].size(), ui_px(20.0));
        assert_eq!(resolved.measurements()[0].start(), ui_px(0.0));
        assert_eq!(resolved.measurements()[1].start(), ui_px(18.0));
        assert_eq!(resolved.measurements()[2].start(), ui_px(46.0));
    }
}
