use std::collections::BTreeSet;

use crate::popover::PopoverState;
use crate::table::filtering::{
    normalize_table_faceted_query, table_facet_value_label, table_faceted_filter_next_filters,
    table_faceted_option_matches, table_faceted_selected_labels, table_faceted_trigger_label,
};
use crate::text_input::TextInputState;
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayPlacementAlignment,
    OverlayPlacementSide, Size, TableColumnFacets, TableColumnId, TableFilter, TableState,
    ThemeTokens,
};

/// One visible option in a table faceted filter recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableFacetedFilterOptionState {
    value: String,
    label: String,
    count: usize,
    selected: bool,
}

impl TableFacetedFilterOptionState {
    pub(in crate::table) fn new(
        value: String,
        label: String,
        count: usize,
        selected: bool,
    ) -> Self {
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
    pub(in crate::table::faceted_filter) fn resolve(
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
