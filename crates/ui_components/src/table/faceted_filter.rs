use std::collections::BTreeSet;
use std::rc::Rc;

use crate::button::{Button, ButtonVariant};
use crate::checkbox::Checkbox;
use crate::geometry::gpui_px_from_ui;
use crate::popover::{Popover, PopoverState};
use crate::scroll_area::ScrollArea;
use crate::text_input::{TextInput, TextInputState};
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    App, Entity, IntoElement, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px, rgba,
};
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayPlacementAlignment,
    OverlayPlacementSide, Sizable, Size, TableColumnFacets, TableColumnId, TableFilter, TableState,
    ThemeTokens, Toggled, UiPx,
};

use super::filtering::{
    normalize_table_faceted_query, table_facet_value_label, table_faceted_filter_next_filters,
    table_faceted_option_matches, table_faceted_selected_labels, table_faceted_trigger_label,
};

type TableFacetedFilterChangeHandler = Rc<dyn Fn(TableFacetedFilterChange, &mut Window, &mut App)>;
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
