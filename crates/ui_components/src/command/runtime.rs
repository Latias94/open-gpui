use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, ElementId, Entity, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, ScrollHandle, StatefulInteractiveElement, Styled, Window,
    div, px, rgba,
};
use open_gpui_ui_core::{
    AccessibleAction, Role, SemanticDescriptor, Sizable, ThemeTokens, UiPx, ui_px,
};

use crate::a11y::UiA11yElementExt;
use crate::choice_overlay_runtime::{
    ChoiceOverlayRuntimeState, commit_registered_choice_overlay_single_value,
};
use crate::color::{ColorIntent, ColorState};
use crate::geometry::gpui_px_from_ui;
use crate::overlay::{
    OverlayInsideRegionId, OverlayLayerBinding, OverlayOpenIntent, WindowOverlayRuntime,
};
use crate::scroll_area::ScrollArea;
use crate::scroll_surface::{ScrollSurfaceRevealStrategy, ScrollSurfaceRuntime, reveal_fixed_row};
use crate::text_input::TextInput;
use crate::text_input::adapter::TextInputController;
use crate::theme::ThemeContext;

use super::render_plan::resolve_command_viewport_extent;
use super::{
    CommandColors, CommandMetrics, CommandRenderPlan, CommandRowRenderPlan, CommandSelection,
    CommandSelectionChange, CommandSelectionMode, CommandState, CommandStatusIntent,
    DEFAULT_COMMAND_VIEWPORT_ITEM_COUNT, nonnegative_px,
};

pub(super) type CommandOpenChangeHandler = Rc<dyn Fn(OverlayOpenIntent, &mut Window, &mut App)>;
pub(super) type CommandQueryChangeHandler = Rc<dyn Fn(String, &mut Window, &mut App)>;
pub(super) type CommandSelectionHandler = Rc<dyn Fn(CommandSelection, &mut Window, &mut App)>;
pub(super) type CommandSelectedValuesChangeHandler =
    Rc<dyn Fn(CommandSelectionChange, &mut Window, &mut App)>;

#[derive(Clone)]
pub(super) struct CommandRuntime {
    pub(super) open: bool,
    pub(super) active_value: Option<String>,
    pub(super) selected_value: Option<String>,
    pub(super) selected_values: Vec<String>,
    pub(super) scroll_surface: ScrollSurfaceRuntime,
    pub(super) overlay_binding: Option<OverlayLayerBinding>,
}

impl CommandRuntime {
    pub(super) fn new(
        open: bool,
        active_value: Option<String>,
        selected_value: Option<String>,
        selected_values: Vec<String>,
    ) -> Self {
        Self {
            open,
            active_value,
            selected_value,
            selected_values,
            scroll_surface: ScrollSurfaceRuntime::new(None),
            overlay_binding: None,
        }
    }
}

impl ChoiceOverlayRuntimeState for CommandRuntime {
    fn commit_single_value(&mut self, value: String) {
        self.selected_value = Some(value.clone());
        self.active_value = Some(value);
    }
}

#[derive(Clone)]
pub(super) struct CommandRegisteredOverlay {
    window_runtime: WindowOverlayRuntime,
    binding: OverlayLayerBinding,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn command_dialog_layer_element(
    content_id: ElementId,
    input_id: ElementId,
    listbox_id: ElementId,
    debug_id: String,
    state: CommandState,
    scroll_handle: ScrollHandle,
    viewport_extent: UiPx,
    scroll_offset: UiPx,
    window_overlay_runtime: WindowOverlayRuntime,
    overlay_binding: OverlayLayerBinding,
    viewport: open_gpui::Size<Pixels>,
    input_controller: Entity<TextInputController>,
    runtime: Entity<CommandRuntime>,
    on_query_change: Option<CommandQueryChangeHandler>,
    on_select: Option<CommandSelectionHandler>,
    on_selected_values_change: Option<CommandSelectedValuesChangeHandler>,
    tokens: ThemeTokens,
    theme: &ThemeContext,
) -> impl IntoElement {
    let metrics = state.metrics();
    let x = ((viewport.width - gpui_px_from_ui(metrics.max_width())) / 2.0).max(px(12.0));
    let y = (viewport.height / 10.0).max(px(24.0));
    let registered_overlay = CommandRegisteredOverlay {
        window_runtime: window_overlay_runtime.clone(),
        binding: overlay_binding.clone(),
    };
    let surface_id: ElementId = (content_id.clone(), "surface").into();
    let surface_debug_id = debug_id.clone();

    div()
        .id((content_id.clone(), "layer"))
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .w(viewport.width)
        .h(viewport.height)
        .bg(rgba(0x00000033))
        .occlude()
        .child(
            window_overlay_runtime.surface(
                &overlay_binding,
                OverlayInsideRegionId::new("surface"),
                format!("command:{debug_id}:surface-region"),
                div()
                    .id(surface_id)
                    .debug_selector(move || format!("command:{surface_debug_id}:surface"))
                    .absolute()
                    .left(x)
                    .top(y)
                    .tab_group()
                    .focusable()
                    .track_focus(overlay_binding.surface_focus())
                    .child(command_content_element(
                        content_id,
                        input_id,
                        listbox_id,
                        debug_id,
                        state,
                        scroll_handle,
                        viewport_extent,
                        scroll_offset,
                        input_controller,
                        runtime,
                        on_query_change,
                        on_select,
                        on_selected_values_change,
                        tokens,
                        Some(registered_overlay),
                        theme,
                    )),
            ),
        )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn command_content_element(
    content_id: ElementId,
    input_id: ElementId,
    listbox_id: ElementId,
    debug_id: String,
    state: CommandState,
    scroll_handle: ScrollHandle,
    viewport_extent: UiPx,
    scroll_offset: UiPx,
    input_controller: Entity<TextInputController>,
    runtime: Entity<CommandRuntime>,
    on_query_change: Option<CommandQueryChangeHandler>,
    on_select: Option<CommandSelectionHandler>,
    on_selected_values_change: Option<CommandSelectedValuesChangeHandler>,
    tokens: ThemeTokens,
    registered_overlay: Option<CommandRegisteredOverlay>,
    theme: &ThemeContext,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let query = state.query().to_owned();
    let label = state.label().to_owned();
    let selected_values = state.selected_values().to_vec();
    let selection_mode = state.selection_mode();
    let dialog_state = state.dialog().cloned();
    let scroll_viewport_id = state.scroll_area().viewport_id().to_owned();
    let plan = CommandRenderPlan::resolve(
        debug_id.clone(),
        listbox_id.to_string(),
        state.clone(),
        scroll_offset,
        viewport_extent,
    );
    let plan_rows = plan.rows().to_vec();
    let total_size = plan.virtualizer().total_size();
    let loading_id: ElementId = (content_id.clone(), "loading").into();
    let status_id: ElementId = (content_id.clone(), "status").into();
    let chips_id: ElementId = (content_id.clone(), "selected-chips").into();
    let selected_chips = state.selected_chips().to_vec();
    let status_items = state.status_items().to_vec();
    let key_state = state.clone();
    let key_runtime = runtime.clone();
    let key_on_select = on_select.clone();
    let key_on_selected_values_change = on_selected_values_change.clone();
    let key_selected_values = selected_values.clone();
    let key_selection_mode = selection_mode;
    let key_scroll_handle = scroll_handle.clone();
    let key_registered_overlay = registered_overlay.clone();
    let content_debug_id = debug_id.clone();
    let mut command_input = TextInput::new(input_id, state.label().to_owned())
        .controller(input_controller)
        .placeholder(state.placeholder().to_owned())
        .value(query)
        .disabled(state.disabled())
        .tokens(tokens)
        .with_size(state.size());
    if let Some(on_query_change) = on_query_change.clone() {
        command_input = command_input.on_change(move |query, window, cx| {
            on_query_change(query, window, cx);
        });
    }
    let content_role = dialog_state
        .as_ref()
        .map_or_else(|| state.content_role(), |dialog| dialog.role());
    let mut content_semantics = SemanticDescriptor::new(content_role).with_label(label.as_ref());
    if dialog_state.is_some() {
        content_semantics = content_semantics.with_modal(true);
    }

    div()
        .id(content_id)
        .debug_selector(move || format!("command:{content_debug_id}:content"))
        .min_w(gpui_px_from_ui(metrics.min_width()))
        .max_w(gpui_px_from_ui(metrics.max_width()))
        .p(gpui_px_from_ui(metrics.padding()))
        .flex()
        .flex_col()
        .gap_2()
        .rounded(gpui_px_from_ui(metrics.radius()))
        .border_1()
        .border_color(theme.resolve(colors.border()))
        .bg(theme.resolve(colors.surface()))
        .text_color(theme.resolve(colors.foreground()))
        .shadow_lg()
        .when(dialog_state.is_some(), |this| this.occlude())
        .ui_semantics(&content_semantics)
        .on_scroll_wheel(|_, _, _| open_gpui::ScrollWheelIntent::handled().stop_propagation())
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let key = command_key_down_event_key(event);
            if event.prefer_character_input {
                return;
            }

            match command_keyboard_action(&key_state, key, viewport_extent) {
                CommandKeyboardAction::Navigate(target) => {
                    cx.stop_propagation();
                    window.prevent_default();
                    key_runtime.update(cx, |runtime, _| {
                        runtime.active_value = Some(target.value.clone());
                    });
                    scroll_command_item_into_view(&key_scroll_handle, &key_state, target.index);
                }
                CommandKeyboardAction::Select(selection) => {
                    cx.stop_propagation();
                    window.prevent_default();
                    let selection_index = selection.index();
                    handle_command_selection(
                        key_registered_overlay.clone(),
                        key_runtime.clone(),
                        key_selection_mode,
                        &key_selected_values,
                        key_on_select.clone(),
                        key_on_selected_values_change.clone(),
                        selection,
                        window,
                        cx,
                    );
                    scroll_command_item_into_view(&key_scroll_handle, &key_state, selection_index);
                }
                CommandKeyboardAction::Ignore => {}
            }
        })
        .child(command_input)
        .when(!selected_chips.is_empty(), |this| {
            this.child(selected_chips.into_iter().fold(
                div().id(chips_id).flex().flex_wrap().gap_1(),
                |row, chip| {
                    let chip_value = chip.value().to_owned();
                    let chip_id = format!("command-selected-chip:{chip_value}");
                    let chip_debug_id = debug_id.clone();
                    row.child(
                        div()
                            .id(chip_id)
                            .debug_selector(move || {
                                format!("command:{chip_debug_id}:selected-chip:{chip_value}")
                            })
                            .px(gpui_px_from_ui(state.size().button_py()))
                            .py(px(1.0))
                            .rounded(gpui_px_from_ui(state.size().control_radius()))
                            .border_1()
                            .border_color(theme.resolve(colors.border()))
                            .text_color(theme.resolve(colors.foreground()))
                            .child(chip.label().to_owned()),
                    )
                },
            ))
        })
        .when_some(state.loading().cloned(), |this, loading| {
            let semantics = SemanticDescriptor::new(loading.role()).with_label(loading.message());
            this.child(
                div()
                    .id(loading_id)
                    .text_color(theme.resolve(colors.muted_foreground()))
                    .ui_semantics(&semantics)
                    .child(loading.message().to_owned()),
            )
        })
        .when(!status_items.is_empty(), |this| {
            let status_debug_id = debug_id.clone();
            this.child(
                status_items.into_iter().enumerate().fold(
                    div()
                        .id(status_id)
                        .debug_selector(move || format!("command:{status_debug_id}:status"))
                        .flex()
                        .flex_col()
                        .gap_1(),
                    |list, (index, item)| {
                        let foreground =
                            theme.resolve(command_status_foreground(item.intent(), colors, tokens));
                        let item_debug_id = debug_id.clone();
                        let message = item.message().to_owned();
                        let semantics = SemanticDescriptor::new(item.role()).with_label(&message);
                        list.child(
                            div()
                                .id(format!("command-status:{index}"))
                                .debug_selector(move || {
                                    format!("command:{item_debug_id}:status:{index}")
                                })
                                .px(gpui_px_from_ui(metrics.padding()))
                                .py(px(2.0))
                                .rounded(gpui_px_from_ui(state.size().control_radius()))
                                .border_1()
                                .border_color(foreground)
                                .text_xs()
                                .text_color(foreground)
                                .ui_semantics(&semantics)
                                .child(message),
                        )
                    },
                ),
            )
        })
        .h(gpui_px_from_ui(metrics.max_height()))
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .overflow_hidden()
                .on_scroll_wheel(|_, _, _| {
                    open_gpui::ScrollWheelIntent::handled().stop_propagation()
                })
                .child(
                    ScrollArea::new(
                        scroll_viewport_id,
                        render_command_results_body(
                            &debug_id,
                            &plan,
                            &plan_rows,
                            total_size,
                            registered_overlay.clone(),
                            runtime.clone(),
                            selection_mode,
                            selected_values.clone(),
                            on_select,
                            on_selected_values_change,
                            theme,
                        ),
                    )
                    .vertical()
                    .scroll_handle(&scroll_handle)
                    .preserve_scroll()
                    .with_size(state.size()),
                ),
        )
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CommandKeyboardAction {
    Navigate(CommandNavigationTarget),
    Select(CommandSelection),
    Ignore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandNavigationTarget {
    index: usize,
    value: String,
}

fn command_keyboard_action(
    state: &CommandState,
    key: &str,
    viewport_extent: UiPx,
) -> CommandKeyboardAction {
    if state.disabled() {
        return CommandKeyboardAction::Ignore;
    }

    if let Some(target) = command_navigation_target(state, key, viewport_extent) {
        return CommandKeyboardAction::Navigate(CommandNavigationTarget {
            index: target.index(),
            value: target.value().to_owned(),
        });
    }

    if let Some(selection) = state.activation_for_key(key) {
        return CommandKeyboardAction::Select(selection);
    }

    CommandKeyboardAction::Ignore
}

fn command_navigation_target<'a>(
    state: &'a CommandState,
    key: &str,
    viewport_extent: UiPx,
) -> Option<&'a crate::listbox::ListboxOptionState> {
    let key = command_navigation_key(key);
    let current = state.listbox().active_index()?;
    let options = state.listbox().options();
    if current >= options.len() {
        return None;
    }

    match key {
        "home" => command_first_focusable_option(options),
        "end" => command_last_focusable_option(options),
        "up" => command_adjacent_focusable_option(options, current, false, state.loop_navigation()),
        "down" => {
            command_adjacent_focusable_option(options, current, true, state.loop_navigation())
        }
        "pageup" => {
            let page_step = command_page_step(state, viewport_extent).max(1);
            command_focusable_option_near(options, current.saturating_sub(page_step), false)
        }
        "pagedown" => {
            let page_step = command_page_step(state, viewport_extent).max(1);
            command_focusable_option_near(
                options,
                current.saturating_add(page_step).min(options.len() - 1),
                true,
            )
        }
        "alt-up" if state.group_navigation() => {
            command_group_navigation_target(options, current, false, state.loop_navigation())
        }
        "alt-down" if state.group_navigation() => {
            command_group_navigation_target(options, current, true, state.loop_navigation())
        }
        _ => None,
    }
}

fn command_navigation_key(key: &str) -> &str {
    match key {
        "ctrl-j" | "ctrl-n" => "down",
        "ctrl-k" | "ctrl-p" => "up",
        "ctrl-d" => "pagedown",
        "ctrl-u" => "pageup",
        _ => key,
    }
}

fn command_key_down_event_key(event: &KeyDownEvent) -> &str {
    let key = event.keystroke.key.as_str();
    let modifiers = event.keystroke.modifiers;
    if modifiers.control && modifiers.number_of_modifiers() == 1 {
        return match key {
            "j" | "n" => "down",
            "k" | "p" => "up",
            "d" => "pagedown",
            "u" => "pageup",
            _ => key,
        };
    }
    if modifiers.alt && modifiers.number_of_modifiers() == 1 {
        return match key {
            "up" => "alt-up",
            "down" => "alt-down",
            _ => key,
        };
    }
    key
}

fn command_first_focusable_option(
    options: &[crate::listbox::ListboxOptionState],
) -> Option<&crate::listbox::ListboxOptionState> {
    options.iter().find(|option| option.focusable())
}

fn command_last_focusable_option(
    options: &[crate::listbox::ListboxOptionState],
) -> Option<&crate::listbox::ListboxOptionState> {
    options.iter().rev().find(|option| option.focusable())
}

fn command_adjacent_focusable_option(
    options: &[crate::listbox::ListboxOptionState],
    current: usize,
    forward: bool,
    loop_navigation: bool,
) -> Option<&crate::listbox::ListboxOptionState> {
    let target = crate::roving_focus::next_matching_index(
        options.len(),
        current,
        forward,
        loop_navigation,
        |index| {
            options
                .get(index)
                .is_some_and(crate::listbox::ListboxOptionState::focusable)
        },
    )?;
    options.get(target)
}

fn command_group_navigation_target(
    options: &[crate::listbox::ListboxOptionState],
    current: usize,
    forward: bool,
    loop_navigation: bool,
) -> Option<&crate::listbox::ListboxOptionState> {
    let current_group = options.get(current)?.group_index();
    let target_group = if forward {
        options
            .iter()
            .skip(current + 1)
            .find(|option| option.focusable() && option.group_index() != current_group)
            .or_else(|| {
                loop_navigation
                    .then(|| {
                        options.iter().take(current).find(|option| {
                            option.focusable() && option.group_index() != current_group
                        })
                    })
                    .flatten()
            })
    } else {
        options
            .iter()
            .take(current)
            .rev()
            .find(|option| option.focusable() && option.group_index() != current_group)
            .or_else(|| {
                loop_navigation
                    .then(|| {
                        options.iter().skip(current + 1).rev().find(|option| {
                            option.focusable() && option.group_index() != current_group
                        })
                    })
                    .flatten()
            })
    }?
    .group_index();

    options
        .iter()
        .find(|option| option.focusable() && option.group_index() == target_group)
}

fn command_focusable_option_near(
    options: &[crate::listbox::ListboxOptionState],
    target: usize,
    forward: bool,
) -> Option<&crate::listbox::ListboxOptionState> {
    if forward {
        options
            .iter()
            .skip(target)
            .find(|option| option.focusable())
            .or_else(|| {
                options
                    .iter()
                    .take(target)
                    .rev()
                    .find(|option| option.focusable())
            })
    } else {
        options
            .iter()
            .take(target + 1)
            .rev()
            .find(|option| option.focusable())
            .or_else(|| {
                options
                    .iter()
                    .skip(target + 1)
                    .find(|option| option.focusable())
            })
    }
}

#[allow(clippy::too_many_arguments)]
fn render_command_results_body(
    command_id: &str,
    plan: &CommandRenderPlan,
    rows: &[CommandRowRenderPlan],
    total_size: UiPx,
    registered_overlay: Option<CommandRegisteredOverlay>,
    runtime: Entity<CommandRuntime>,
    selection_mode: CommandSelectionMode,
    selected_values: Vec<String>,
    on_select: Option<CommandSelectionHandler>,
    on_selected_values_change: Option<CommandSelectedValuesChangeHandler>,
    theme: &ThemeContext,
) -> impl IntoElement {
    let command_id = command_id.to_owned();
    let listbox_id = plan.listbox_id().to_owned();
    let state = plan.state().clone();
    let colors = state.colors();
    let metrics = state.metrics();
    let rows = rows.to_vec();
    let listbox_semantics = SemanticDescriptor::new(plan.role())
        .with_label(plan.label())
        .with_disabled(state.disabled());

    div()
        .id(listbox_id.clone())
        .debug_selector({
            let listbox_id = listbox_id.clone();
            move || format!("listbox:{listbox_id}")
        })
        .relative()
        .w_full()
        .h(gpui_px_from_ui(total_size))
        .min_h(gpui_px_from_ui(total_size))
        .p(gpui_px_from_ui(state.listbox().metrics().surface_padding()))
        .text_size(gpui_px_from_ui(state.listbox().metrics().text_size()))
        .line_height(gpui_px_from_ui(state.listbox().metrics().text_size()))
        .text_color(theme.resolve(colors.foreground()))
        .ui_semantics(&listbox_semantics)
        .children(command_result_children(
            &command_id,
            &listbox_id,
            state,
            rows,
            metrics,
            colors,
            registered_overlay,
            runtime,
            selection_mode,
            selected_values,
            on_select,
            on_selected_values_change,
            theme,
        ))
}

#[allow(clippy::too_many_arguments)]
fn command_result_children(
    command_id: &str,
    listbox_id: &str,
    state: CommandState,
    rows: Vec<CommandRowRenderPlan>,
    metrics: CommandMetrics,
    colors: CommandColors,
    registered_overlay: Option<CommandRegisteredOverlay>,
    runtime: Entity<CommandRuntime>,
    selection_mode: CommandSelectionMode,
    selected_values: Vec<String>,
    on_select: Option<CommandSelectionHandler>,
    on_selected_values_change: Option<CommandSelectedValuesChangeHandler>,
    theme: &ThemeContext,
) -> Vec<AnyElement> {
    if state.empty() {
        return vec![
            div()
                .debug_selector({
                    let listbox_id = listbox_id.to_owned();
                    move || format!("listbox:{listbox_id}:empty")
                })
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .right(px(0.0))
                .px(gpui_px_from_ui(
                    state.listbox().metrics().option_padding_x(),
                ))
                .py(gpui_px_from_ui(
                    state.listbox().metrics().option_padding_y(),
                ))
                .text_color(theme.resolve(colors.muted_foreground()))
                .child(state.empty_label().to_owned())
                .into_any_element(),
        ];
    }

    rows.into_iter()
        .map(|row| {
            render_command_result_row(
                command_id.to_owned(),
                listbox_id.to_owned(),
                row,
                metrics,
                colors,
                registered_overlay.clone(),
                runtime.clone(),
                selection_mode,
                selected_values.clone(),
                on_select.clone(),
                on_selected_values_change.clone(),
                theme,
            )
            .into_any_element()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn render_command_result_row(
    command_id: String,
    listbox_id: String,
    row: CommandRowRenderPlan,
    metrics: CommandMetrics,
    colors: CommandColors,
    registered_overlay: Option<CommandRegisteredOverlay>,
    runtime: Entity<CommandRuntime>,
    selection_mode: CommandSelectionMode,
    selected_values: Vec<String>,
    on_select: Option<CommandSelectionHandler>,
    on_selected_values_change: Option<CommandSelectedValuesChangeHandler>,
    theme: &ThemeContext,
) -> impl IntoElement {
    let option_value = row.value().to_owned();
    let render_key = row.render_key().to_owned();
    let label = row.label().to_owned();
    let icon_label = row.icon_label().map(str::to_owned);
    let shortcut = row.shortcut().map(str::to_owned);
    let disabled_reason = row.disabled_reason_ref().map(str::to_owned);
    let accessibility_description = row.accessibility_description().map(str::to_owned);
    let selection = CommandSelection::from_item(row.item());
    let disabled = row.disabled();
    let selected = row.selected();
    let active = row.active();
    let position = row.item().position_in_set();
    let group_label = row.group_label().map(str::to_owned);
    let group_label_height = if group_label.is_some() {
        state_group_label_height(metrics)
    } else {
        UiPx::ZERO
    };
    let group_label_color = theme.resolve(colors.muted_foreground());
    let row_background = theme.resolve(command_row_background(active, selected, colors));
    let row_foreground = theme.resolve(if disabled {
        colors.muted_foreground()
    } else {
        colors.foreground()
    });
    let row_hover_background = theme.resolve(command_row_hover_background(colors));
    let shortcut_foreground = theme.resolve(colors.shortcut_foreground());
    let option_aria_label = accessibility_description
        .as_ref()
        .or(disabled_reason.as_ref())
        .map_or_else(
            || label.clone(),
            |description| format!("{label}, {description}"),
        );
    let mut option_semantics = SemanticDescriptor::new(row.role())
        .with_label(&option_aria_label)
        .with_selected(selected)
        .with_disabled(disabled)
        .with_actions(&[AccessibleAction::Click]);
    if let Some(position) = position {
        option_semantics = option_semantics
            .with_position_in_set(position)
            .with_size_of_set(row.item().size_of_set());
    }

    div()
        .id(format!("command-row:{render_key}"))
        .debug_selector({
            let command_id = command_id.clone();
            let render_key = render_key.clone();
            move || format!("command:{command_id}:row:{render_key}")
        })
        .absolute()
        .top(gpui_px_from_ui(row.virtual_start()))
        .left(px(0.0))
        .right(px(0.0))
        .h(gpui_px_from_ui(row.virtual_size()))
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .when_some(group_label, |this, label| {
            let semantics = SemanticDescriptor::new(Role::Group).with_label(label.as_ref());
            this.child(
                div()
                    .id(format!("command-group-label:{render_key}"))
                    .h(gpui_px_from_ui(group_label_height))
                    .px(gpui_px_from_ui(state_group_label_padding_x(metrics)))
                    .flex()
                    .items_center()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(group_label_color)
                    .ui_semantics(&semantics)
                    .child(label),
            )
        })
        .child(
            div()
                .id(format!("listbox-option:{option_value}"))
                .debug_selector({
                    let listbox_id = listbox_id.clone();
                    let option_value = option_value.clone();
                    move || format!("listbox:{listbox_id}:option:{option_value}")
                })
                .h(gpui_px_from_ui(row.virtual_size() - group_label_height))
                .min_h(gpui_px_from_ui(row.virtual_size() - group_label_height))
                .px(gpui_px_from_ui(state_option_padding_x(metrics)))
                .py(gpui_px_from_ui(state_option_padding_y(metrics)))
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .rounded(gpui_px_from_ui(metrics.radius()))
                .bg(row_background)
                .text_color(row_foreground)
                .ui_semantics(&option_semantics)
                .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                .when(!disabled, |this| {
                    this.cursor_pointer()
                        .hover(move |style| style.bg(row_hover_background))
                        .on_click(move |_event: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            window.prevent_default();
                            let Some(selection) = selection.clone() else {
                                return;
                            };
                            handle_command_selection(
                                registered_overlay.clone(),
                                runtime.clone(),
                                selection_mode,
                                &selected_values,
                                on_select.clone(),
                                on_selected_values_change.clone(),
                                selection,
                                window,
                                cx,
                            );
                        })
                })
                .child(
                    div()
                        .min_w(px(0.0))
                        .flex()
                        .flex_1()
                        .items_center()
                        .gap_2()
                        .truncate()
                        .when_some(icon_label, |this, icon_label| {
                            this.child(div().flex_none().child(icon_label))
                        })
                        .child(label),
                )
                .when_some(shortcut, |this, shortcut| {
                    this.child(
                        div()
                            .flex_none()
                            .min_w(gpui_px_from_ui(metrics.shortcut_min_width()))
                            .text_xs()
                            .text_color(shortcut_foreground)
                            .child(shortcut),
                    )
                }),
        )
}

#[allow(clippy::too_many_arguments)]
fn handle_command_selection(
    registered_overlay: Option<CommandRegisteredOverlay>,
    runtime: Entity<CommandRuntime>,
    selection_mode: CommandSelectionMode,
    selected_values: &[String],
    on_select: Option<CommandSelectionHandler>,
    on_selected_values_change: Option<CommandSelectedValuesChangeHandler>,
    selection: CommandSelection,
    window: &mut Window,
    cx: &mut App,
) {
    match selection_mode {
        CommandSelectionMode::Single => {
            if let Some(registered_overlay) = registered_overlay {
                let selected_value = selection.value().to_owned();
                commit_registered_choice_overlay_single_value(
                    &registered_overlay.window_runtime,
                    &registered_overlay.binding,
                    runtime.clone(),
                    selected_value,
                    window,
                    cx,
                    move |window, cx| {
                        if let Some(on_select) = on_select.as_ref() {
                            on_select(selection, window, cx);
                        }
                    },
                );
            } else {
                runtime.update(cx, |runtime, _| {
                    runtime.selected_value = Some(selection.value().to_owned());
                    runtime.active_value = Some(selection.value().to_owned());
                });
                if let Some(on_select) = on_select.as_ref() {
                    on_select(selection, window, cx);
                }
            }
        }
        CommandSelectionMode::Multiple => {
            let change = command_selection_change_after_toggle(selected_values, selection);
            runtime.update(cx, |runtime, _| {
                runtime.active_value = Some(change.toggled().value().to_owned());
                runtime.selected_values = change.values().to_vec();
            });
            if let Some(on_selected_values_change) = on_selected_values_change.as_ref() {
                on_selected_values_change(change, window, cx);
            }
        }
    }
}

fn scroll_command_item_into_view(scroll_handle: &ScrollHandle, state: &CommandState, index: usize) {
    reveal_fixed_row(
        scroll_handle,
        ScrollSurfaceRevealStrategy::Nearest,
        index,
        state.items().len(),
        state.metrics().row_height(),
        Some(resolve_command_viewport_extent(state.metrics(), UiPx::ZERO)),
    );
}

fn command_row_background(active: bool, selected: bool, colors: CommandColors) -> ColorIntent {
    if active {
        ColorIntent::with_state(colors.surface().token(), ColorState::FocusVisible, 0xe8ede6)
    } else if selected {
        ColorIntent::with_state(colors.surface().token(), ColorState::Selected, 0xe8ede6)
    } else {
        colors.surface()
    }
}

fn command_row_hover_background(colors: CommandColors) -> ColorIntent {
    ColorIntent::with_state(colors.surface().token(), ColorState::Hover, 0xf1f5ee)
}

fn command_status_foreground(
    intent: CommandStatusIntent,
    colors: CommandColors,
    tokens: ThemeTokens,
) -> ColorIntent {
    match intent {
        CommandStatusIntent::Info => colors.muted_foreground(),
        CommandStatusIntent::Warning => {
            ColorIntent::with_state(tokens.text_muted, ColorState::Message, 0xbf8700)
        }
        CommandStatusIntent::Error => {
            ColorIntent::with_state(tokens.destructive, ColorState::Invalid, 0xb42318)
        }
    }
}

const fn state_option_padding_x(metrics: CommandMetrics) -> UiPx {
    metrics.padding()
}

const fn state_option_padding_y(_metrics: CommandMetrics) -> UiPx {
    ui_px(3.0)
}

const fn state_group_label_padding_x(metrics: CommandMetrics) -> UiPx {
    metrics.padding()
}

const fn state_group_label_height(metrics: CommandMetrics) -> UiPx {
    metrics.row_height().half()
}

fn command_page_step(state: &CommandState, viewport_extent: UiPx) -> usize {
    let row_height = nonnegative_px(state.metrics().row_height());
    if row_height.as_f32() <= 0.0 {
        return DEFAULT_COMMAND_VIEWPORT_ITEM_COUNT;
    }

    let viewport_extent = resolve_command_viewport_extent(state.metrics(), viewport_extent);
    (viewport_extent.as_f32() / row_height.as_f32())
        .floor()
        .max(1.0) as usize
}

pub(super) fn command_scroll_reset_key(state: &CommandState) -> String {
    format!(
        "{}|{:?}|{}",
        state.query(),
        state.index_mode(),
        state.index_revision().unwrap_or_default()
    )
}

fn command_selection_change_after_toggle(
    selected_values: &[String],
    selection: CommandSelection,
) -> CommandSelectionChange {
    let mut values = selected_values.to_vec();
    let selected = if let Some(index) = values.iter().position(|value| value == selection.value()) {
        values.remove(index);
        false
    } else {
        values.push(selection.value().to_owned());
        true
    };

    CommandSelectionChange::new(values, selection, selected)
}

#[cfg(test)]
mod tests {
    use open_gpui::{KeyDownEvent, Keystroke, Modifiers};
    use open_gpui_ui_core::ui_px;

    use super::*;
    use crate::command::{Command, CommandGroup, CommandItem};

    fn keyboard_state(disabled: bool) -> CommandState {
        Command::new("palette", "Command palette")
            .open(true)
            .disabled(disabled)
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

    fn paged_keyboard_state(selected: &str) -> CommandState {
        Command::new("paged-palette", "Command palette")
            .open(true)
            .row_height(ui_px(20.0))
            .selected(selected)
            .item(CommandItem::new("open-file", "Open File"))
            .item(CommandItem::new("disabled-one", "Disabled One").disabled(true))
            .item(CommandItem::new("disabled-two", "Disabled Two").disabled(true))
            .item(CommandItem::new("close-window", "Close Window"))
            .state()
    }

    fn grouped_keyboard_state(active: &str) -> CommandState {
        grouped_keyboard_command(active).state()
    }

    fn grouped_keyboard_command(active: &str) -> Command {
        Command::new("grouped-palette", "Command palette")
            .open(true)
            .active(active)
            .item(CommandItem::new("global-open", "Global Open"))
            .group(
                CommandGroup::new("file", "File")
                    .item(CommandItem::new("new-file", "New File"))
                    .item(CommandItem::new("close-window", "Close Window")),
            )
            .group(
                CommandGroup::new("view", "View")
                    .item(CommandItem::new("view-sidebar", "View Sidebar"))
                    .item(CommandItem::new("zoom-in", "Zoom In")),
            )
    }

    #[test]
    fn scroll_reset_key_tracks_query_not_selection_navigation() {
        let base = Command::new("palette", "Command palette")
            .open(true)
            .default_query("file")
            .selected("open-file")
            .active("open-file")
            .item(CommandItem::new("open-file", "Open File"))
            .item(CommandItem::new("close-file", "Close File"))
            .state();
        let selection_changed = Command::new("palette", "Command palette")
            .open(true)
            .default_query("file")
            .selected("close-file")
            .active("close-file")
            .item(CommandItem::new("open-file", "Open File"))
            .item(CommandItem::new("close-file", "Close File"))
            .state();
        let query_changed = Command::new("palette", "Command palette")
            .open(true)
            .default_query("close")
            .selected("close-file")
            .active("close-file")
            .item(CommandItem::new("open-file", "Open File"))
            .item(CommandItem::new("close-file", "Close File"))
            .state();

        assert_eq!(
            command_scroll_reset_key(&base),
            command_scroll_reset_key(&selection_changed)
        );
        assert_ne!(
            command_scroll_reset_key(&base),
            command_scroll_reset_key(&query_changed)
        );
    }

    #[test]
    fn keyboard_action_moves_and_selects_active_command() {
        let state = keyboard_state(false);

        assert_eq!(
            command_keyboard_action(&state, "up", ui_px(224.0)),
            CommandKeyboardAction::Navigate(CommandNavigationTarget {
                index: 0,
                value: "open-file".to_string()
            })
        );
        assert_eq!(
            command_keyboard_action(&state, "enter", ui_px(224.0)),
            CommandKeyboardAction::Select(CommandSelection::new(
                1,
                "new-file".to_string(),
                "New File".to_string(),
                Some("Ctrl+N".to_string()),
            ))
        );
    }

    #[test]
    fn keyboard_action_supports_vim_navigation_aliases() {
        let state = keyboard_state(false);
        let down = command_keyboard_action(&state, "down", ui_px(224.0));
        let up = command_keyboard_action(&state, "up", ui_px(224.0));

        assert_eq!(
            command_keyboard_action(&state, "ctrl-j", ui_px(224.0)),
            down
        );
        assert_eq!(
            command_keyboard_action(&state, "ctrl-n", ui_px(224.0)),
            down
        );
        assert_eq!(command_keyboard_action(&state, "ctrl-k", ui_px(224.0)), up);
        assert_eq!(command_keyboard_action(&state, "ctrl-p", ui_px(224.0)), up);
    }

    #[test]
    fn keyboard_action_supports_home_end_and_configurable_looping() {
        let looping_last = paged_keyboard_state("close-window");
        assert_eq!(
            command_keyboard_action(&looping_last, "down", ui_px(40.0)),
            CommandKeyboardAction::Navigate(CommandNavigationTarget {
                index: 0,
                value: "open-file".to_string()
            })
        );
        assert_eq!(
            command_keyboard_action(&looping_last, "home", ui_px(40.0)),
            CommandKeyboardAction::Navigate(CommandNavigationTarget {
                index: 0,
                value: "open-file".to_string()
            })
        );
        let looping_first = paged_keyboard_state("open-file");
        assert_eq!(
            command_keyboard_action(&looping_first, "end", ui_px(40.0)),
            CommandKeyboardAction::Navigate(CommandNavigationTarget {
                index: 3,
                value: "close-window".to_string()
            })
        );

        let bounded_last = Command::new("bounded-palette", "Command palette")
            .open(true)
            .loop_navigation(false)
            .selected("close-window")
            .item(CommandItem::new("open-file", "Open File"))
            .item(CommandItem::new("disabled-one", "Disabled One").disabled(true))
            .item(CommandItem::new("close-window", "Close Window"))
            .state();
        assert_eq!(
            command_keyboard_action(&bounded_last, "down", ui_px(40.0)),
            CommandKeyboardAction::Ignore
        );
        assert_eq!(
            command_keyboard_action(&bounded_last, "home", ui_px(40.0)),
            CommandKeyboardAction::Navigate(CommandNavigationTarget {
                index: 0,
                value: "open-file".to_string()
            })
        );

        let bounded_first = Command::new("bounded-palette", "Command palette")
            .open(true)
            .loop_navigation(false)
            .selected("open-file")
            .item(CommandItem::new("open-file", "Open File"))
            .item(CommandItem::new("close-window", "Close Window"))
            .state();
        assert_eq!(
            command_keyboard_action(&bounded_first, "up", ui_px(40.0)),
            CommandKeyboardAction::Ignore
        );

        let single = Command::new("single-palette", "Command palette")
            .open(true)
            .item(CommandItem::new("open-file", "Open File"))
            .state();
        let current = CommandKeyboardAction::Navigate(CommandNavigationTarget {
            index: 0,
            value: "open-file".to_string(),
        });
        assert_eq!(
            command_keyboard_action(&single, "down", ui_px(40.0)),
            current
        );
        assert_eq!(command_keyboard_action(&single, "up", ui_px(40.0)), current);
    }

    #[test]
    fn keyboard_action_supports_group_navigation_aliases() {
        let file_state = grouped_keyboard_state("new-file");
        assert_eq!(
            command_keyboard_action(&file_state, "alt-down", ui_px(224.0)),
            CommandKeyboardAction::Navigate(CommandNavigationTarget {
                index: 3,
                value: "view-sidebar".to_string()
            })
        );

        let view_state = grouped_keyboard_state("zoom-in");
        assert_eq!(
            command_keyboard_action(&view_state, "alt-up", ui_px(224.0)),
            CommandKeyboardAction::Navigate(CommandNavigationTarget {
                index: 1,
                value: "new-file".to_string()
            })
        );

        let disabled_group_down = grouped_keyboard_command("new-file")
            .group_navigation(false)
            .state();
        assert_eq!(
            command_keyboard_action(&disabled_group_down, "alt-down", ui_px(224.0)),
            CommandKeyboardAction::Ignore
        );

        let disabled_group_up = grouped_keyboard_command("zoom-in")
            .group_navigation(false)
            .state();
        assert_eq!(
            command_keyboard_action(&disabled_group_up, "alt-up", ui_px(224.0)),
            CommandKeyboardAction::Ignore
        );
    }

    #[test]
    fn keyboard_event_key_normalizes_control_navigation_aliases() {
        let event = KeyDownEvent {
            keystroke: Keystroke {
                modifiers: Modifiers {
                    control: true,
                    ..Modifiers::none()
                },
                key: "j".to_string(),
                key_char: None,
            },
            is_held: false,
            prefer_character_input: false,
        };

        assert_eq!(command_key_down_event_key(&event), "down");
    }

    #[test]
    fn keyboard_event_key_names_group_navigation_aliases() {
        let down = KeyDownEvent {
            keystroke: Keystroke {
                modifiers: Modifiers {
                    alt: true,
                    ..Modifiers::none()
                },
                key: "down".to_string(),
                key_char: None,
            },
            is_held: false,
            prefer_character_input: false,
        };
        assert_eq!(command_key_down_event_key(&down), "alt-down");

        let up = KeyDownEvent {
            keystroke: Keystroke {
                modifiers: Modifiers {
                    alt: true,
                    ..Modifiers::none()
                },
                key: "up".to_string(),
                key_char: None,
            },
            is_held: false,
            prefer_character_input: false,
        };
        assert_eq!(command_key_down_event_key(&up), "alt-up");
    }

    #[test]
    fn keyboard_action_pages_to_nearest_focusable_command() {
        let down_state = paged_keyboard_state("open-file");
        assert_eq!(
            command_keyboard_action(&down_state, "pagedown", ui_px(40.0)),
            CommandKeyboardAction::Navigate(CommandNavigationTarget {
                index: 3,
                value: "close-window".to_string()
            })
        );

        let up_state = paged_keyboard_state("close-window");
        assert_eq!(
            command_keyboard_action(&up_state, "pageup", ui_px(40.0)),
            CommandKeyboardAction::Navigate(CommandNavigationTarget {
                index: 0,
                value: "open-file".to_string()
            })
        );
    }

    #[test]
    fn keyboard_action_ignores_disabled_command() {
        let state = keyboard_state(true);

        assert_eq!(
            command_keyboard_action(&state, "down", ui_px(224.0)),
            CommandKeyboardAction::Ignore
        );
        assert_eq!(
            command_keyboard_action(&state, "enter", ui_px(224.0)),
            CommandKeyboardAction::Ignore
        );
    }
}
