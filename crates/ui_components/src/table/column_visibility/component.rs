use std::rc::Rc;

use open_gpui::{App, IntoElement, RenderOnce, SharedString, Window};
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayPlacementAlignment,
    OverlayPlacementSide, Sizable, Size, TableColumn, TableColumnVisibilityOverrides, ThemeTokens,
};

use crate::popover::Popover;

use super::TableColumnVisibilityChangeHandler;
use super::render::table_column_visibility_content_element;
use super::state::{TableColumnVisibilityChange, TableColumnVisibilityState};

#[derive(Debug, Clone)]
pub(in crate::table::column_visibility) struct TableColumnVisibilityRuntime {
    pub(in crate::table::column_visibility) visibility: TableColumnVisibilityOverrides,
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
