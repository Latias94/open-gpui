use std::rc::Rc;

use crate::button::{Button, ButtonVariant};
use crate::popover::{Popover, PopoverState};
use crate::text_input::{TextInput, TextInputState};
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    App, Entity, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayPlacementAlignment,
    OverlayPlacementSide, Sizable, Size, TableColumnFacets, TableColumnId, TableFacetRange,
    TableFilter, TableState, ThemeTokens,
};

use super::filtering::{
    normalize_table_range_filter_values, parse_table_range_filter_bound,
    table_range_filter_bound_placeholder, table_range_filter_next_filters,
    table_range_filter_trigger_label, table_range_filter_value_text,
};

type TableRangeFilterChangeHandler = Rc<dyn Fn(TableRangeFilterChange, &mut Window, &mut App)>;
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
