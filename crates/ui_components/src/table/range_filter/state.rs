use crate::popover::PopoverState;
use crate::table::filtering::{
    normalize_table_range_filter_values, parse_table_range_filter_bound,
    table_range_filter_bound_placeholder, table_range_filter_next_filters,
    table_range_filter_trigger_label,
};
use crate::text_input::TextInputState;
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayPlacementAlignment,
    OverlayPlacementSide, Size, TableColumnFacets, TableColumnId, TableFacetRange, TableFilter,
    TableState, ThemeTokens,
};

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
    pub(in crate::table::range_filter) fn resolve(
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
