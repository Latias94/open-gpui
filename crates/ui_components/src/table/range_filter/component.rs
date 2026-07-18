use std::rc::Rc;

use crate::popover::Popover;
use crate::table::filtering::{normalize_table_range_filter_values, table_range_filter_value_text};
use crate::theme::ThemeResolver;
use open_gpui::{App, IntoElement, RenderOnce, SharedString, Window};
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayPlacementAlignment,
    OverlayPlacementSide, Sizable, Size, TableColumnFacets, TableColumnId, ThemeTokens,
};

use super::TableRangeFilterChangeHandler;
use super::render::table_range_filter_content_element;
use super::state::{TableRangeFilterChange, TableRangeFilterState};

#[derive(Debug, Clone)]
pub(in crate::table::range_filter) struct TableRangeFilterRuntime {
    pub(in crate::table::range_filter) min_text: String,
    pub(in crate::table::range_filter) max_text: String,
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
        let theme = ThemeResolver::current(window, cx);
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
            &theme,
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
            popover = popover.on_open_change(move |intent, window, cx| {
                on_open_change(intent.desired_open(), window, cx);
            });
        }

        popover
    }
}
