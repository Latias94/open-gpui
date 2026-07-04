//! Command palette component built from search input, grouped command items, and listbox state.

mod descriptor;
mod model;
mod render_plan;
mod runtime;
mod style;

use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, point, px,
};
use open_gpui_command::CommandDescriptor;
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, OutsidePressPolicy, Role, Sizable,
    Size, ThemeTokens, UiPx,
};

use crate::a11y::UiA11yElementExt;
use crate::focus::focus_ring_shadow_with_theme;
use crate::overlay::{
    emit_overlay_open_change, gpui_full_window_overlay_layer, gpui_overlay_state,
    resolve_overlay_open_state, set_overlay_open,
};
use crate::text_editing::TextEditingPolicy;
use crate::text_input::TextInputDisplayMode;
use crate::text_input::adapter::TextInputController;
use crate::theme::ThemeResolver;
pub use descriptor::{
    CommandGroupDescriptor, CommandIndexSnapshot, CommandIndexSnapshotMode, CommandItemDescriptor,
    CommandLoadingState, CommandMatchSource, CommandOpenMode, CommandPaletteController,
    CommandPaletteControllerUpdate, CommandPalettePendingProviderRequest, CommandPaletteProjection,
    CommandProviderPaletteProjection, CommandQueryMode, CommandSelectionMode, CommandStatusIntent,
    CommandStatusItem,
};
pub use model::{
    CommandDialogState, CommandGroupState, CommandItemState, CommandNavigationBehavior,
    CommandSelectedChipState, CommandSelection, CommandSelectionChange, CommandState,
};
pub use render_plan::{CommandBehaviorSnapshot, CommandRowBehaviorSnapshot};
pub(crate) use render_plan::{CommandRenderPlan, CommandRowRenderPlan};
use runtime::{
    CommandOpenChangeHandler, CommandQueryChangeHandler, CommandRuntime,
    CommandSelectedValuesChangeHandler, CommandSelectionHandler, command_content_element,
    command_dialog_layer_element, command_scroll_reset_key,
};
pub use style::{CommandColors, CommandMetrics};
pub(crate) use style::{DEFAULT_COMMAND_VIEWPORT_ITEM_COUNT, nonnegative_px};

/// A concrete GPUI command surface.
#[derive(IntoElement)]
pub struct Command {
    id: ElementId,
    label: SharedString,
    placeholder: SharedString,
    trigger_label: SharedString,
    items: Vec<CommandItem>,
    groups: Vec<CommandGroup>,
    index_snapshot: Option<CommandIndexSnapshot>,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    dialog_enabled: bool,
    navigation_behavior: CommandNavigationBehavior,
    query: Option<String>,
    default_query: String,
    selection_mode: CommandSelectionMode,
    selected_value: Option<String>,
    selected_values: Option<Vec<String>>,
    active_value: Option<String>,
    viewport_item_count: usize,
    metrics: CommandMetrics,
    loading_state: Option<CommandLoadingState>,
    status_items: Vec<CommandStatusItem>,
    empty_label: SharedString,
    dialog_title: Option<String>,
    dialog_description: Option<String>,
    outside_press_policy: OutsidePressPolicy,
    escape_key_policy: EscapeKeyPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<CommandOpenChangeHandler>,
    on_query_change: Option<CommandQueryChangeHandler>,
    on_select: Option<CommandSelectionHandler>,
    on_selected_values_change: Option<CommandSelectedValuesChangeHandler>,
}

impl Command {
    /// Creates an inline command surface.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        let size = Size::Medium;
        Self {
            id: id.into(),
            label: label.into(),
            placeholder: "Search commands".into(),
            trigger_label: "Open command menu".into(),
            items: Vec::new(),
            groups: Vec::new(),
            index_snapshot: None,
            size,
            disabled: false,
            open: None,
            default_open: false,
            dialog_enabled: false,
            navigation_behavior: CommandNavigationBehavior::default(),
            query: None,
            default_query: String::new(),
            selection_mode: CommandSelectionMode::Single,
            selected_value: None,
            selected_values: None,
            active_value: None,
            viewport_item_count: DEFAULT_COMMAND_VIEWPORT_ITEM_COUNT,
            metrics: CommandMetrics::from_size(size),
            loading_state: None,
            status_items: Vec::new(),
            empty_label: "No commands".into(),
            dialog_title: None,
            dialog_description: None,
            outside_press_policy: OutsidePressPolicy::DismissAndConsume,
            escape_key_policy: EscapeKeyPolicy::Dismiss,
            initial_focus_intent: InitialFocusIntent::FirstFocusable,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_open_change: None,
            on_query_change: None,
            on_select: None,
            on_selected_values_change: None,
        }
    }

    /// Applies placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Applies dialog trigger label.
    pub fn trigger_label(mut self, label: impl Into<SharedString>) -> Self {
        self.trigger_label = label.into();
        self
    }

    /// Adds one standalone command item.
    pub fn item(mut self, item: CommandItem) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many standalone command items.
    pub fn items(mut self, items: impl IntoIterator<Item = CommandItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Adds one command group.
    pub fn group(mut self, group: CommandGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Adds many command groups.
    pub fn groups(mut self, groups: impl IntoIterator<Item = CommandGroup>) -> Self {
        self.groups.extend(groups);
        self
    }

    /// Applies a caller-owned command index snapshot.
    pub fn index_snapshot(mut self, snapshot: CommandIndexSnapshot) -> Self {
        self.index_snapshot = Some(snapshot);
        self
    }

    /// Applies a provider-backed refresh projection to the command query and index snapshot.
    pub fn provider_refresh_projection(
        mut self,
        projection: &open_gpui_command::CommandProviderRefreshProjection,
    ) -> Self {
        let palette_projection =
            CommandProviderPaletteProjection::from_refresh_projection(projection);
        self.query =
            Some(TextEditingPolicy::single_line().normalize_text(palette_projection.query()));
        self.status_items = palette_projection.status_items().to_vec();
        self.index_snapshot = Some(palette_projection.into_index_snapshot());
        self
    }

    /// Applies an app-owned command-center palette projection to the command query and snapshot.
    pub fn palette_projection(mut self, projection: &CommandPaletteProjection) -> Self {
        self.query = Some(TextEditingPolicy::single_line().normalize_text(projection.query()));
        self.index_snapshot = Some(projection.index_snapshot().clone());
        self.status_items = projection.status_items().to_vec();
        self
    }

    /// Marks the command surface as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies controlled dialog open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Applies uncontrolled initial dialog open state.
    pub fn default_open(mut self, default_open: bool) -> Self {
        self.default_open = default_open;
        self
    }

    /// Enables dialog presentation with a title.
    pub fn dialog(mut self, title: impl Into<String>) -> Self {
        self.dialog_enabled = true;
        self.dialog_title = Some(title.into());
        self
    }

    /// Enables or disables dialog presentation.
    pub fn dialog_enabled(mut self, enabled: bool) -> Self {
        self.dialog_enabled = enabled;
        if !enabled {
            self.dialog_title = None;
            self.dialog_description = None;
        }
        self
    }

    /// Applies optional dialog description text.
    pub fn dialog_description(mut self, description: impl Into<String>) -> Self {
        self.dialog_description = Some(description.into());
        self
    }

    /// Applies command keyboard navigation behavior.
    pub fn navigation_behavior(mut self, behavior: CommandNavigationBehavior) -> Self {
        self.navigation_behavior = behavior;
        self
    }

    /// Enables or disables loop navigation across the first and last command rows.
    pub fn loop_navigation(mut self, enabled: bool) -> Self {
        self.navigation_behavior = self.navigation_behavior.with_loop_navigation(enabled);
        self
    }

    /// Enables or disables group-jump navigation aliases.
    pub fn group_navigation(mut self, enabled: bool) -> Self {
        self.navigation_behavior = self.navigation_behavior.with_group_navigation(enabled);
        self
    }

    /// Applies controlled search query text.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        let query = query.into();
        self.query = Some(TextEditingPolicy::single_line().normalize_text(query.as_str()));
        self
    }

    /// Applies the default search query for adapter-owned input state.
    pub fn default_query(mut self, query: impl Into<String>) -> Self {
        let query = query.into();
        self.default_query = TextEditingPolicy::single_line().normalize_text(query.as_str());
        self
    }

    /// Applies command selection behavior.
    pub fn selection_mode(mut self, mode: CommandSelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// Enables or disables persistent multi-selection behavior.
    pub fn multi_select(mut self, enabled: bool) -> Self {
        self.selection_mode = if enabled {
            CommandSelectionMode::Multiple
        } else {
            CommandSelectionMode::Single
        };
        self
    }

    /// Applies selected item value.
    pub fn selected(mut self, value: impl Into<String>) -> Self {
        self.selected_value = Some(value.into());
        self
    }

    /// Applies controlled selected values for multi-selection.
    pub fn selected_values(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.selection_mode = CommandSelectionMode::Multiple;
        self.selected_values = Some(values.into_iter().map(Into::into).collect());
        self
    }

    /// Applies active item value.
    pub fn active(mut self, value: impl Into<String>) -> Self {
        self.active_value = Some(value.into());
        self
    }

    /// Applies the estimated number of command rows visible in the result viewport.
    pub fn viewport_item_count(mut self, count: usize) -> Self {
        self.viewport_item_count = count.max(1);
        self
    }

    /// Applies the fixed command result row height.
    pub fn row_height(mut self, row_height: UiPx) -> Self {
        self.metrics = self.metrics.with_row_height(row_height);
        self
    }

    /// Applies the command result overscan row budget.
    pub fn overscan(mut self, overscan: usize) -> Self {
        self.metrics = self.metrics.with_overscan_count(overscan);
        self
    }

    /// Applies loading metadata.
    pub fn loading(mut self, message: impl Into<String>, progress_percent: Option<u8>) -> Self {
        self.loading_state = Some(CommandLoadingState::new(message, progress_percent));
        self
    }

    /// Clears loading metadata.
    pub fn idle(mut self) -> Self {
        self.loading_state = None;
        self
    }

    /// Adds one command palette status item.
    pub fn status_item(mut self, item: CommandStatusItem) -> Self {
        if !item.is_empty() {
            self.status_items.push(item);
        }
        self
    }

    /// Adds many command palette status items.
    pub fn status_items(mut self, items: impl IntoIterator<Item = CommandStatusItem>) -> Self {
        self.status_items
            .extend(items.into_iter().filter(|item| !item.is_empty()));
        self
    }

    /// Clears command palette status items.
    pub fn clear_status_items(mut self) -> Self {
        self.status_items.clear();
        self
    }

    /// Applies empty-state label.
    pub fn empty_label(mut self, label: impl Into<SharedString>) -> Self {
        self.empty_label = label.into();
        self
    }

    /// Applies outside-press policy.
    pub fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = policy;
        self
    }

    /// Applies Escape key policy.
    pub fn escape_key_policy(mut self, policy: EscapeKeyPolicy) -> Self {
        self.escape_key_policy = policy;
        self
    }

    /// Applies initial focus intent.
    pub fn initial_focus_intent(mut self, intent: InitialFocusIntent) -> Self {
        self.initial_focus_intent = intent;
        self
    }

    /// Applies focus restore intent.
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

    /// Registers a query-change handler with the next sanitized query text.
    pub fn on_query_change(
        mut self,
        handler: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_query_change = Some(Rc::new(handler));
        self
    }

    /// Registers a command selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(CommandSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Registers a selected-values change handler for multi-selection.
    pub fn on_selected_values_change(
        mut self,
        handler: impl Fn(CommandSelectionChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selected_values_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved command state.
    pub fn state(&self) -> CommandState {
        let query_mode = if self.query.is_some() {
            CommandQueryMode::Controlled
        } else {
            CommandQueryMode::Uncontrolled
        };
        let query = self.query.as_deref().unwrap_or(self.default_query.as_str());
        let selected_values = self.selected_values.clone().unwrap_or_default().into_iter();

        self.resolve_state_with_inputs(
            self.open,
            query,
            query_mode,
            self.selected_value.as_deref(),
            selected_values,
            self.active_value.as_deref(),
        )
    }

    /// Returns the default command behavior snapshot at the viewport origin.
    pub fn behavior_snapshot(&self) -> CommandBehaviorSnapshot {
        self.behavior_snapshot_with_viewport(
            UiPx::ZERO,
            self.metrics.row_height() * self.viewport_item_count as f32,
        )
    }

    /// Resolves the command behavior snapshot for a viewport.
    pub fn behavior_snapshot_with_viewport(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> CommandBehaviorSnapshot {
        let plan = self.render_plan_with_viewport(scroll_offset, viewport_extent);
        CommandBehaviorSnapshot::from_render_plan(&plan)
    }

    fn render_plan_with_viewport(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> CommandRenderPlan {
        let state = self.state();
        CommandRenderPlan::resolve(
            self.id.to_string(),
            format!("{}-listbox", self.id),
            state,
            scroll_offset,
            viewport_extent,
        )
    }
}

impl Command {
    fn resolve_state_with_inputs(
        &self,
        open: Option<bool>,
        query: &str,
        query_mode: CommandQueryMode,
        selected_value: Option<&str>,
        selected_values: impl IntoIterator<Item = impl Into<String>>,
        active_value: Option<&str>,
    ) -> CommandState {
        let state = if let Some(index_snapshot) = self.index_snapshot.clone() {
            CommandState::resolve_from_index_snapshot(
                self.size,
                self.disabled,
                open,
                self.default_open,
                self.dialog_enabled,
                self.label.to_string(),
                self.placeholder.to_string(),
                query,
                query_mode,
                self.selection_mode,
                selected_value,
                selected_values,
                active_value,
                self.loading_state.clone(),
                self.empty_label.to_string(),
                self.dialog_title.clone(),
                self.dialog_description.clone(),
                index_snapshot,
                self.outside_press_policy,
                self.escape_key_policy,
                self.initial_focus_intent.clone(),
                self.focus_restore_intent.clone(),
                self.tokens,
            )
        } else {
            CommandState::resolve(
                self.size,
                self.disabled,
                open,
                self.default_open,
                self.dialog_enabled,
                self.label.to_string(),
                self.placeholder.to_string(),
                query,
                query_mode,
                self.selection_mode,
                selected_value,
                selected_values,
                active_value,
                self.loading_state.clone(),
                self.empty_label.to_string(),
                self.dialog_title.clone(),
                self.dialog_description.clone(),
                self.groups.iter().map(CommandGroup::descriptor),
                self.items.iter().map(CommandItem::descriptor),
                self.outside_press_policy,
                self.escape_key_policy,
                self.initial_focus_intent.clone(),
                self.focus_restore_intent.clone(),
                self.tokens,
            )
        };

        state
            .with_metrics(self.metrics)
            .with_navigation_behavior(self.navigation_behavior)
            .with_status_items(self.status_items.clone())
    }
}

impl Sizable for Command {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self.metrics = CommandMetrics::from_size(size);
        self
    }
}

impl RenderOnce for Command {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(cx);
        let initial_query = self
            .query
            .clone()
            .unwrap_or_else(|| self.default_query.clone());
        let initial_selected_values = self
            .selected_values
            .clone()
            .unwrap_or_else(|| self.selected_value.iter().cloned().collect());
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| {
            CommandRuntime::new(
                self.default_open,
                self.active_value.clone(),
                self.selected_value.clone(),
                initial_selected_values.clone(),
                initial_query.clone(),
            )
        });
        let input_state_key: ElementId = (self.id.clone(), "input-state").into();
        let input_controller = window.use_keyed_state(input_state_key, cx, |_, cx| {
            let mut input = TextInputController::with_value(initial_query.clone(), cx);
            input.set_placeholder(self.placeholder.clone(), cx);
            input
        });
        let runtime_state = runtime.read(cx).clone();
        let scroll_handle = runtime_state.scroll_handle.clone();
        let open_state = resolve_overlay_open_state(self.open, runtime_state.open);
        let resolved_open = open_state.open();
        if open_state.runtime_changed() {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let query_mode = if self.query.is_some() {
            CommandQueryMode::Controlled
        } else {
            CommandQueryMode::Uncontrolled
        };
        let controller_query = input_controller.read(cx).value().to_owned();
        let query = self
            .query
            .as_deref()
            .unwrap_or(controller_query.as_str())
            .to_owned();
        let selected_value = self
            .selected_value
            .as_deref()
            .or(runtime_state.selected_value.as_deref());
        let selected_values = self
            .selected_values
            .clone()
            .unwrap_or_else(|| runtime_state.selected_values.clone());
        let active_value = self
            .active_value
            .as_deref()
            .or(runtime_state.active_value.as_deref())
            .or(selected_value);
        let state = self.resolve_state_with_inputs(
            Some(resolved_open),
            query.as_str(),
            query_mode,
            selected_value,
            selected_values.iter().cloned(),
            active_value,
        );
        let scroll_reset_key = command_scroll_reset_key(&state);
        if runtime_state.scroll_reset_key != scroll_reset_key {
            scroll_handle.set_offset(point(px(0.0), px(0.0)));
            runtime.update(cx, |runtime, _| {
                runtime.scroll_reset_key = scroll_reset_key.clone();
            });
        }
        let query_change_handler = self.on_query_change.clone();
        input_controller.update(cx, |controller, _cx| {
            let controlled_query =
                (query_mode == CommandQueryMode::Controlled).then(|| query.as_str());
            controller.sync_adapter_state(
                controlled_query,
                Some(self.placeholder.clone()),
                state.disabled(),
                false,
                TextInputDisplayMode::Plain,
                query_change_handler.clone(),
            );
        });
        let id = self.id;
        let debug_id = id.to_string();
        let trigger_id: ElementId = (id.clone(), "trigger").into();
        let input_id: ElementId = (id.clone(), "input").into();
        let content_id: ElementId = (id.clone(), "content").into();
        let listbox_id: ElementId = (id.clone(), "listbox").into();
        let viewport_extent = ui_px_from_gpui(scroll_handle.bounds().size.height);
        let scroll_offset =
            UiPx::new((-ui_px_from_gpui(scroll_handle.offset().y).as_f32()).max(0.0));
        let metrics = state.metrics();
        let colors = state.colors();
        let disabled = state.disabled();
        let focus_ring = state.focus_ring();
        let dialog_state = state.dialog().cloned();
        let dialog_open = dialog_state.clone().filter(|_| state.open());
        let dialog_overlay_adapter = dialog_state
            .as_ref()
            .map(|dialog| gpui_overlay_state(dialog.overlay()))
            .unwrap_or_else(|| gpui_overlay_state(state.overlay()));
        let viewport = window.viewport_size();
        let dialog_enabled = self.dialog_enabled;
        let trigger_label = self.trigger_label;
        let on_open_change = self.on_open_change;
        let on_query_change = query_change_handler;
        let on_select = self.on_select;
        let on_selected_values_change = self.on_selected_values_change;
        let tokens = self.tokens;
        let trigger_focus_shadow = focus_ring_shadow_with_theme(focus_ring, &theme);

        div()
            .id(id)
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("command:{debug_id}:root")
            })
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .gap_2()
            .when(dialog_state.is_some(), |this| {
                let runtime = runtime.clone();
                let on_open_change = on_open_change.clone();
                let trigger_label = trigger_label.clone();
                this.child(
                    div()
                        .id(trigger_id)
                        .debug_selector({
                            let debug_id = debug_id.clone();
                            move || format!("command:{debug_id}:trigger")
                        })
                        .min_h(gpui_px_from_ui(state.size().button_h()))
                        .px(gpui_px_from_ui(state.size().button_px()))
                        .py(gpui_px_from_ui(state.size().button_py()))
                        .rounded(gpui_px_from_ui(metrics.radius()))
                        .border_1()
                        .border_color(theme.resolve(colors.border()))
                        .bg(theme.resolve(colors.surface()))
                        .text_color(theme.resolve(colors.foreground()))
                        .focusable()
                        .tab_stop(!disabled)
                        .ui_role(Role::Button)
                        .aria_label(trigger_label.clone())
                        .aria_expanded(state.open())
                        .aria_disabled(disabled)
                        .focus_visible(move |style| style.shadow(trigger_focus_shadow.clone()))
                        .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                        .when(!disabled, |this| {
                            this.cursor_pointer().on_click(
                                move |_event: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    runtime.update(cx, |runtime, _| {
                                        set_overlay_open(&mut runtime.open, true);
                                    });
                                    emit_overlay_open_change(
                                        true,
                                        on_open_change.as_deref(),
                                        window,
                                        cx,
                                    );
                                },
                            )
                        })
                        .child(trigger_label),
                )
            })
            .when(!dialog_enabled, |this| {
                this.child(command_content_element(
                    content_id.clone(),
                    input_id.clone(),
                    listbox_id.clone(),
                    debug_id.clone(),
                    state.clone(),
                    scroll_handle.clone(),
                    viewport_extent,
                    scroll_offset,
                    input_controller.clone(),
                    runtime.clone(),
                    on_open_change.clone(),
                    on_query_change.clone(),
                    on_select.clone(),
                    on_selected_values_change.clone(),
                    tokens,
                    &theme,
                ))
            })
            .when_some(dialog_open, |this, dialog_state| {
                this.child(gpui_full_window_overlay_layer(
                    &dialog_overlay_adapter,
                    command_dialog_layer_element(
                        content_id,
                        input_id,
                        listbox_id,
                        debug_id,
                        state,
                        scroll_handle,
                        viewport_extent,
                        scroll_offset,
                        dialog_state,
                        viewport,
                        input_controller,
                        runtime,
                        on_open_change,
                        on_query_change,
                        on_select,
                        on_selected_values_change,
                        tokens,
                        &theme,
                    ),
                ))
            })
    }
}

/// A concrete GPUI command item.
#[derive(Clone)]
pub struct CommandItem {
    descriptor: CommandItemDescriptor,
}

impl CommandItem {
    /// Creates a selectable command item.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: CommandItemDescriptor::new(value, label.to_string()),
        }
    }

    /// Creates a command item from shared app-command metadata.
    pub fn from_command_descriptor(descriptor: &CommandDescriptor) -> Self {
        Self {
            descriptor: CommandItemDescriptor::from_command_descriptor(descriptor),
        }
    }

    /// Adds one filtering keyword.
    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.keyword(keyword);
        self
    }

    /// Adds a display shortcut label.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.shortcut(shortcut);
        self
    }

    /// Marks the command as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.descriptor = self.descriptor.disabled(disabled);
        self
    }

    /// Marks the command as disabled with a user-displayable reason.
    pub fn disabled_reason(mut self, reason: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.disabled_reason(reason);
        self
    }

    /// Applies caller-owned availability metadata without evaluating it.
    pub fn when(mut self, when: impl Into<String>) -> Self {
        self.descriptor = self.descriptor.when(when);
        self
    }

    /// Returns the pure descriptor.
    pub fn descriptor(&self) -> CommandItemDescriptor {
        self.descriptor.clone()
    }
}

/// A concrete GPUI command group.
#[derive(Clone)]
pub struct CommandGroup {
    descriptor: CommandGroupDescriptor,
    items: Vec<CommandItem>,
}

impl CommandGroup {
    /// Creates an empty command group.
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            descriptor: CommandGroupDescriptor::new(value, label.to_string()),
            items: Vec::new(),
        }
    }

    /// Adds one command item.
    pub fn item(mut self, item: CommandItem) -> Self {
        self.items.push(item);
        self
    }

    /// Adds many command items.
    pub fn items(mut self, items: impl IntoIterator<Item = CommandItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Returns the pure descriptor.
    pub fn descriptor(&self) -> CommandGroupDescriptor {
        self.items
            .iter()
            .fold(self.descriptor.clone(), |descriptor, item| {
                descriptor.item(item.descriptor())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyboard_state() -> CommandState {
        Command::new("palette", "Command palette")
            .open(true)
            .default_query("file")
            .selected("new-file")
            .item(CommandItem::new("open-file", "Open File").shortcut("Ctrl+O"))
            .group(
                CommandGroup::new("file", "File")
                    .item(CommandItem::new("new-file", "New File").shortcut("Ctrl+N"))
                    .item(CommandItem::new("close-window", "Close Window").shortcut("Alt+F4")),
            )
            .state()
    }

    #[test]
    fn command_state_exposes_standalone_and_grouped_views() {
        let state = keyboard_state();

        let standalone_values = state
            .standalone_items()
            .map(|item| item.value().to_owned())
            .collect::<Vec<_>>();
        let grouped_values = state
            .grouped_groups()
            .map(|group| group.value().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(standalone_values, vec!["open-file".to_string()]);
        assert_eq!(grouped_values, vec!["file".to_string()]);
        assert_eq!(state.standalone_items().count(), 1);
    }
}
