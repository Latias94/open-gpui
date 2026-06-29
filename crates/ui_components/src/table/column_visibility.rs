use std::rc::Rc;

use crate::button::{Button, ButtonVariant};
use crate::checkbox::Checkbox;
use crate::geometry::gpui_px_from_ui;
use crate::popover::{Popover, PopoverState};
use crate::scroll_area::ScrollArea;
use crate::theme::ThemeResolver;
use open_gpui::prelude::*;
use open_gpui::{
    App, Entity, IntoElement, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px, rgba,
};
use open_gpui_ui_core::{
    FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, OverlayPlacementAlignment,
    OverlayPlacementSide, Sizable, Size, TableColumn, TableColumnId,
    TableColumnVisibilityOverrides, TableState, ThemeTokens, Toggled, UiPx,
};

type TableColumnVisibilityChangeHandler =
    Rc<dyn Fn(TableColumnVisibilityChange, &mut Window, &mut App)>;
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

fn table_column_visibility_trigger_label(label: &str, hidden_count: usize) -> String {
    if hidden_count == 0 {
        label.to_owned()
    } else {
        format!("{label}: {hidden_count} hidden")
    }
}
