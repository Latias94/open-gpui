use std::rc::Rc;

use jellyflow::{
    core::{CanvasSize, NodeId as JellyNodeId},
    runtime::schema::NodeSurfaceSlotProjection,
};
use jellyflow_open_gpui::{
    OpenGpuiActionPlan, OpenGpuiBoundsCollector, OpenGpuiControlEventValue, OpenGpuiControlPlan,
    OpenGpuiControlPrimitive, OpenGpuiMeasurementId, OpenGpuiMenuPlan,
    OpenGpuiRepeatableActionPlan, OpenGpuiViewBounds, OpenGpuiViewPoint, OpenGpuiViewSize,
    control_option_key, control_selected_option_key, open_gpui_action_button_element_id,
    open_gpui_action_menu_element_id, open_gpui_control_element_id,
    open_gpui_slot_action_button_element_id,
};
use open_gpui::{
    AnyElement, App, Bounds, KeyDownEvent, MouseButton, MouseDownEvent, Pixels, Window, div,
    measured_element, prelude::*, px,
};
use open_gpui_ui_components::prelude::Sizable;
use open_gpui_ui_components::{
    Badge, BadgeVariant, Button, ButtonVariant, ListboxOption, Menu, MenuItem, NumberInput, Select,
    Slider, Switch, TextInput, Textarea,
};
use open_gpui_ui_core::Size;
use serde_json::Value;

#[derive(Clone)]
pub struct NodeComponentKitActions {
    control_dispatch:
        Rc<dyn Fn(JellyNodeId, OpenGpuiControlPlan, OpenGpuiControlEventValue, &mut App)>,
    menu_dispatch: Rc<dyn Fn(OpenGpuiMenuPlan, String, Option<JellyNodeId>, &mut App)>,
    repeatable_dispatch: Rc<dyn Fn(JellyNodeId, OpenGpuiRepeatableActionPlan, &mut App)>,
}

impl NodeComponentKitActions {
    pub fn new<Control, Menu, Repeatable>(
        control_dispatch: Control,
        menu_dispatch: Menu,
        repeatable_dispatch: Repeatable,
    ) -> Self
    where
        Control:
            Fn(JellyNodeId, OpenGpuiControlPlan, OpenGpuiControlEventValue, &mut App) + 'static,
        Menu: Fn(OpenGpuiMenuPlan, String, Option<JellyNodeId>, &mut App) + 'static,
        Repeatable: Fn(JellyNodeId, OpenGpuiRepeatableActionPlan, &mut App) + 'static,
    {
        Self {
            control_dispatch: Rc::new(control_dispatch),
            menu_dispatch: Rc::new(menu_dispatch),
            repeatable_dispatch: Rc::new(repeatable_dispatch),
        }
    }

    fn dispatch_control(
        &self,
        node_id: JellyNodeId,
        control: OpenGpuiControlPlan,
        event: OpenGpuiControlEventValue,
        cx: &mut App,
    ) {
        (self.control_dispatch)(node_id, control, event, cx);
    }

    fn dispatch_menu(
        &self,
        menu: OpenGpuiMenuPlan,
        action_key: String,
        node_id: Option<JellyNodeId>,
        cx: &mut App,
    ) {
        (self.menu_dispatch)(menu, action_key, node_id, cx);
    }

    fn dispatch_repeatable(
        &self,
        node_id: JellyNodeId,
        action: OpenGpuiRepeatableActionPlan,
        cx: &mut App,
    ) {
        (self.repeatable_dispatch)(node_id, action, cx);
    }
}

pub fn render_measured_region(
    id: OpenGpuiMeasurementId,
    collector: OpenGpuiBoundsCollector,
    child: impl IntoElement,
) -> AnyElement {
    let element_id = id.element_id();
    measured_element(element_id, child, move |_, bounds, global_id, _, _| {
        collector.record_id(id.clone(), gpui_view_bounds(bounds), global_id);
    })
    .into_any_element()
}

pub fn gpui_view_bounds(bounds: Bounds<Pixels>) -> OpenGpuiViewBounds {
    OpenGpuiViewBounds::new(
        OpenGpuiViewPoint::new(bounds.origin.x.as_f32(), bounds.origin.y.as_f32()),
        OpenGpuiViewSize::new(bounds.size.width.as_f32(), bounds.size.height.as_f32()),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdaptiveNodeLayoutMode {
    Full,
    Compact,
    Shell,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveNodeLayoutRegion {
    pub key: String,
    pub top: f32,
    pub height: f32,
    pub mode: AdaptiveNodeLayoutMode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveRepeatableLayoutPlan {
    pub region: AdaptiveNodeLayoutRegion,
    pub visible_items: usize,
    pub hidden_items: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveTextPlan {
    pub mode: AdaptiveNodeLayoutMode,
    pub estimated_lines: usize,
    pub visible_lines: usize,
    pub hidden_lines: usize,
    pub overflow_indicator_required: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveControlRowPlan {
    pub mode: AdaptiveNodeLayoutMode,
    pub label_width: f32,
    pub control_width: f32,
    pub label_overflow: bool,
    pub value_overflow: bool,
    pub clipped: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveNodeLayoutStack {
    cursor_y: f32,
    bottom_y: f32,
    gap: f32,
    regions: Vec<AdaptiveNodeLayoutRegion>,
}

impl AdaptiveNodeLayoutStack {
    pub fn new(
        node_size: CanvasSize,
        pad: f32,
        header_height: f32,
        footer_height: f32,
        gap: f32,
    ) -> Self {
        let cursor_y = pad + header_height + gap;
        let bottom_y = (node_size.height - pad - footer_height).max(cursor_y);
        Self {
            cursor_y,
            bottom_y,
            gap,
            regions: Vec::new(),
        }
    }

    pub fn from_available_height(available_height: f32, gap: f32) -> Self {
        Self {
            cursor_y: 0.0,
            bottom_y: available_height.max(0.0),
            gap,
            regions: Vec::new(),
        }
    }

    pub fn available_height(&self) -> f32 {
        (self.bottom_y - self.cursor_y).max(0.0)
    }

    pub fn regions(&self) -> &[AdaptiveNodeLayoutRegion] {
        &self.regions
    }

    pub fn reserve_region(
        &mut self,
        key: impl Into<String>,
        full_height: f32,
        compact_height: f32,
    ) -> AdaptiveNodeLayoutRegion {
        let available = self.available_height();
        let mode = if available >= full_height {
            AdaptiveNodeLayoutMode::Full
        } else if available >= compact_height {
            AdaptiveNodeLayoutMode::Compact
        } else {
            AdaptiveNodeLayoutMode::Shell
        };
        let height = match mode {
            AdaptiveNodeLayoutMode::Full => full_height,
            AdaptiveNodeLayoutMode::Compact => available.min(full_height),
            AdaptiveNodeLayoutMode::Shell => available,
        }
        .max(0.0);
        self.push_region(key, height, mode)
    }

    pub fn reserve_repeatable_list(
        &mut self,
        key: impl Into<String>,
        item_count: usize,
        max_visible_items: usize,
        row_height: f32,
        row_gap: f32,
        overflow_indicator_height: f32,
    ) -> AdaptiveRepeatableLayoutPlan {
        let key = key.into();
        let available = self.available_height();
        let visible_items = repeatable_visible_items_for_height(
            available,
            item_count,
            max_visible_items,
            row_height,
            row_gap,
            overflow_indicator_height,
        );
        let hidden_items = item_count.saturating_sub(visible_items);
        let needed_height = repeatable_list_height(
            visible_items,
            hidden_items,
            row_height,
            row_gap,
            overflow_indicator_height,
        );
        let mode = if hidden_items == 0 && visible_items == item_count {
            AdaptiveNodeLayoutMode::Full
        } else if visible_items > 0 {
            AdaptiveNodeLayoutMode::Compact
        } else {
            AdaptiveNodeLayoutMode::Shell
        };
        let region = self.push_region(key, needed_height.min(available), mode);

        AdaptiveRepeatableLayoutPlan {
            region,
            visible_items,
            hidden_items,
        }
    }

    fn push_region(
        &mut self,
        key: impl Into<String>,
        height: f32,
        mode: AdaptiveNodeLayoutMode,
    ) -> AdaptiveNodeLayoutRegion {
        let region = AdaptiveNodeLayoutRegion {
            key: key.into(),
            top: self.cursor_y,
            height,
            mode,
        };
        self.cursor_y = (self.cursor_y + height + self.gap).min(self.bottom_y);
        self.regions.push(region.clone());
        region
    }
}

pub fn adaptive_repeatable_list_plan(
    key: impl Into<String>,
    available_height: f32,
    item_count: usize,
    max_visible_items: usize,
    row_height: f32,
    row_gap: f32,
    overflow_indicator_height: f32,
) -> AdaptiveRepeatableLayoutPlan {
    AdaptiveNodeLayoutStack::from_available_height(available_height, row_gap)
        .reserve_repeatable_list(
            key,
            item_count,
            max_visible_items,
            row_height,
            row_gap,
            overflow_indicator_height,
        )
}

pub fn adaptive_text_plan(
    text: &str,
    available_width: f32,
    available_height: f32,
    full_line_budget: usize,
    compact_line_budget: usize,
) -> AdaptiveTextPlan {
    let estimated_lines = estimated_text_lines(text, available_width);
    let height_line_capacity = (available_height / 16.0).floor().max(0.0) as usize;
    let visible_capacity = height_line_capacity.min(full_line_budget.max(1));
    let mode = if visible_capacity == 0 {
        AdaptiveNodeLayoutMode::Shell
    } else if estimated_lines <= visible_capacity && visible_capacity >= full_line_budget {
        AdaptiveNodeLayoutMode::Full
    } else if visible_capacity >= compact_line_budget.max(1) {
        AdaptiveNodeLayoutMode::Compact
    } else {
        AdaptiveNodeLayoutMode::Shell
    };
    let visible_lines = match mode {
        AdaptiveNodeLayoutMode::Full => estimated_lines.min(full_line_budget.max(1)),
        AdaptiveNodeLayoutMode::Compact => {
            estimated_lines.min(visible_capacity.max(1).min(compact_line_budget.max(1)))
        }
        AdaptiveNodeLayoutMode::Shell => {
            usize::from(!text.trim().is_empty() && visible_capacity > 0)
        }
    };
    let hidden_lines = estimated_lines.saturating_sub(visible_lines);

    AdaptiveTextPlan {
        mode,
        estimated_lines,
        visible_lines,
        hidden_lines,
        overflow_indicator_required: hidden_lines > 0,
    }
}

pub fn adaptive_control_row_plan(
    available_width: f32,
    available_height: f32,
    label: &str,
    value: &str,
) -> AdaptiveControlRowPlan {
    let mode = if available_height >= 38.0 && available_width >= 260.0 {
        AdaptiveNodeLayoutMode::Full
    } else if available_height >= 28.0 && available_width >= 176.0 {
        AdaptiveNodeLayoutMode::Compact
    } else {
        AdaptiveNodeLayoutMode::Shell
    };
    let (label_width, control_width) = match mode {
        AdaptiveNodeLayoutMode::Full => {
            let control_width = (available_width * 0.58).clamp(156.0, 210.0);
            (
                (available_width - control_width - 12.0).max(72.0),
                control_width,
            )
        }
        AdaptiveNodeLayoutMode::Compact => {
            let control_width = (available_width * 0.62).clamp(112.0, 168.0);
            (
                (available_width - control_width - 8.0).max(48.0),
                control_width,
            )
        }
        AdaptiveNodeLayoutMode::Shell => (available_width.max(0.0), 0.0),
    };
    let label_overflow = text_needs_more_width(label, label_width, 7.0);
    let value_overflow =
        mode != AdaptiveNodeLayoutMode::Shell && text_needs_more_width(value, control_width, 7.0);

    AdaptiveControlRowPlan {
        mode,
        label_width,
        control_width,
        label_overflow,
        value_overflow,
        clipped: matches!(mode, AdaptiveNodeLayoutMode::Shell) && available_height < 18.0,
    }
}

fn estimated_text_lines(text: &str, available_width: f32) -> usize {
    let chars_per_line = (available_width / 7.0).floor().max(8.0) as usize;
    let char_count = text.chars().filter(|ch| !ch.is_control()).count().max(1);
    char_count.div_ceil(chars_per_line).max(1)
}

fn text_needs_more_width(text: &str, available_width: f32, average_char_width: f32) -> bool {
    let available_chars = (available_width / average_char_width).floor().max(1.0) as usize;
    text.chars().filter(|ch| !ch.is_control()).count() > available_chars
}

fn repeatable_visible_items_for_height(
    available_height: f32,
    item_count: usize,
    max_visible_items: usize,
    row_height: f32,
    row_gap: f32,
    overflow_indicator_height: f32,
) -> usize {
    let max_visible = item_count.min(max_visible_items);
    (0..=max_visible)
        .rev()
        .find(|visible_items| {
            repeatable_list_height(
                *visible_items,
                item_count.saturating_sub(*visible_items),
                row_height,
                row_gap,
                overflow_indicator_height,
            ) <= available_height.max(0.0)
        })
        .unwrap_or(0)
}

fn repeatable_list_height(
    visible_items: usize,
    hidden_items: usize,
    row_height: f32,
    row_gap: f32,
    overflow_indicator_height: f32,
) -> f32 {
    let row_count = visible_items + usize::from(hidden_items > 0);
    if row_count == 0 {
        return 0.0;
    }

    let visible_height = visible_items as f32 * row_height;
    let overflow_height = if hidden_items > 0 {
        overflow_indicator_height
    } else {
        0.0
    };
    visible_height + overflow_height + row_gap * row_count.saturating_sub(1) as f32
}

pub fn render_interactive_control_region(child: AnyElement) -> AnyElement {
    div()
        .block_mouse_except_scroll()
        .on_mouse_down(MouseButton::Left, |event: &MouseDownEvent, _window, cx| {
            cx.stop_propagation();
            let _ = event;
        })
        .on_key_down(|_: &KeyDownEvent, _window, cx| {
            cx.stop_propagation();
        })
        .child(child)
        .into_any_element()
}

pub fn render_control_plan(
    node_id: JellyNodeId,
    control_scope: &str,
    control: &OpenGpuiControlPlan,
    index: usize,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let id = open_gpui_control_element_id(node_id, control_scope, &control.key, index);
    let read_only = control_component_read_only(control);
    let disabled = control_component_disabled(control);
    let interaction_disabled = control_component_interaction_disabled(control);
    let label = control.label.clone();
    let value = control_value_label(control);
    let control_plan = control.clone();

    let element = match control.primitive {
        OpenGpuiControlPrimitive::TextInput => TextInput::new(id, label)
            .value(value)
            .placeholder(control.placeholder.clone().unwrap_or_default())
            .disabled(disabled)
            .read_only(read_only)
            .on_change(control_text_change_handler(
                node_id,
                control_plan.clone(),
                actions.clone(),
            ))
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::TextArea => Textarea::new(id, label)
            .value(value)
            .placeholder(control.placeholder.clone().unwrap_or_default())
            .rows(2)
            .disabled(disabled)
            .read_only(read_only)
            .on_change(control_text_change_handler(
                node_id,
                control_plan.clone(),
                actions.clone(),
            ))
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::NumberInput => NumberInput::new(id, label)
            .value(control_number_value(control))
            .disabled(disabled)
            .read_only(read_only)
            .on_change(control_number_change_handler(
                node_id,
                control_plan.clone(),
                actions.clone(),
            ))
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::Select | OpenGpuiControlPrimitive::MultiSelect => {
            let selected = control_selected_option_key(control).unwrap_or_default();
            Select::new(id, label)
                .options(control_options(control))
                .placeholder(
                    control
                        .placeholder
                        .clone()
                        .unwrap_or_else(|| "Select".to_string()),
                )
                .selected(selected)
                .disabled(interaction_disabled || control.options.is_empty())
                .on_select(control_select_change_handler(
                    node_id,
                    control_plan.clone(),
                    actions.clone(),
                ))
                .with_size(Size::XSmall)
                .into_any_element()
        }
        OpenGpuiControlPrimitive::Switch => Switch::new(id)
            .label(label)
            .checked(control_bool_value(control))
            .disabled(interaction_disabled)
            .on_change(control_bool_change_handler(
                node_id,
                control_plan.clone(),
                actions.clone(),
            ))
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::Slider => Slider::new(id, label)
            .value(control_number_value(control))
            .disabled(interaction_disabled)
            .on_change(control_slider_change_handler(
                node_id,
                control_plan.clone(),
                actions.clone(),
            ))
            .with_size(Size::XSmall)
            .into_any_element(),
        OpenGpuiControlPrimitive::CodeEditor | OpenGpuiControlPrimitive::ColorSwatch => {
            Badge::new(id, format!("{}: {}", control.label, value))
                .variant(BadgeVariant::Default)
                .with_size(Size::XSmall)
                .into_any_element()
        }
        OpenGpuiControlPrimitive::AssetPickerStub
        | OpenGpuiControlPrimitive::VariablePickerStub
        | OpenGpuiControlPrimitive::PortBindingDisplay => {
            Button::new(id, format!("{}*", control.label))
                .variant(ButtonVariant::Secondary)
                .disabled(true)
                .with_size(Size::XSmall)
                .into_any_element()
        }
    };

    render_interactive_control_region(element)
}

pub fn render_dispatch_action_button(
    menu: &OpenGpuiMenuPlan,
    action: &OpenGpuiActionPlan,
    index: usize,
    node_id: Option<JellyNodeId>,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let action_key = action.key.clone();
    let menu = menu.clone();
    let actions = actions.clone();
    let mut button = Button::new(
        open_gpui_action_button_element_id(node_id, &menu.key, &action.key, index),
        action_button_label(action),
    )
    .variant(action_button_variant(action, index))
    .disabled(!action.dispatchable())
    .with_size(Size::XSmall);

    if action.dispatchable() {
        button = button.on_click(move |event, _window, cx| {
            cx.stop_propagation();
            let _ = event;
            actions.dispatch_menu(menu.clone(), action_key.clone(), node_id, cx);
        });
    }

    render_interactive_control_region(button.into_any_element())
}

pub fn render_action_menu(
    menu: &OpenGpuiMenuPlan,
    id_suffix: &str,
    node_id: Option<JellyNodeId>,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let items = menu
        .actions
        .iter()
        .map(|action| {
            MenuItem::action(action.key.clone(), action_menu_item_label(action))
                .disabled(!action.dispatchable())
        })
        .collect::<Vec<_>>();

    let menu = Menu::new(
        open_gpui_action_menu_element_id(node_id, &menu.key, id_suffix),
        format!("{} {}", menu.label, menu.actions.len()),
    )
    .items(items)
    .disabled(menu.actions.is_empty())
    .on_select({
        let menu = menu.clone();
        let actions = actions.clone();
        move |selection, _window, cx| {
            actions.dispatch_menu(menu.clone(), selection.value().to_owned(), node_id, cx);
        }
    })
    .with_size(Size::XSmall)
    .into_any_element();

    render_interactive_control_region(menu)
}

pub fn repeatable_action_button(
    node_id: JellyNodeId,
    id: String,
    label: &'static str,
    variant: ButtonVariant,
    disabled: bool,
    action: OpenGpuiRepeatableActionPlan,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let mut button = Button::new(id, label)
        .variant(variant)
        .disabled(disabled)
        .with_size(Size::XSmall);

    if !disabled {
        let actions = actions.clone();
        button = button.on_click(move |event, _window, cx| {
            cx.stop_propagation();
            let _ = event;
            actions.dispatch_repeatable(node_id, action.clone(), cx);
        });
    }

    render_interactive_control_region(button.into_any_element())
}

pub fn render_action_buttons(
    node_id: JellyNodeId,
    slot: &NodeSurfaceSlotProjection,
    value: &str,
) -> impl IntoElement {
    let actions = value
        .split(['·', ',', '[', ']'])
        .filter(|action| !action.trim().is_empty() && *action != "-")
        .take(2)
        .enumerate()
        .map(|(index, action)| {
            Button::new(
                open_gpui_slot_action_button_element_id(node_id, &slot.key, index),
                action.trim().to_owned(),
            )
            .variant(if index == 0 {
                ButtonVariant::Default
            } else {
                ButtonVariant::Secondary
            })
            .with_size(Size::XSmall)
            .into_any_element()
        })
        .collect::<Vec<_>>();

    div()
        .flex()
        .items_center()
        .justify_end()
        .gap_1()
        .min_w(px(0.0))
        .overflow_hidden()
        .children(actions)
}

fn control_text_change_handler(
    node_id: JellyNodeId,
    control: OpenGpuiControlPlan,
    actions: NodeComponentKitActions,
) -> impl Fn(String, &mut Window, &mut App) + 'static {
    move |value, _window, cx| {
        actions.dispatch_control(
            node_id,
            control.clone(),
            OpenGpuiControlEventValue::Text(value),
            cx,
        );
    }
}

fn control_number_change_handler(
    node_id: JellyNodeId,
    control: OpenGpuiControlPlan,
    actions: NodeComponentKitActions,
) -> impl Fn(open_gpui_ui_components::NumberInputChange, &mut Window, &mut App) + 'static {
    move |change, _window, cx| {
        if change.changed() {
            actions.dispatch_control(
                node_id,
                control.clone(),
                OpenGpuiControlEventValue::Number(change.value() as f64),
                cx,
            );
        }
    }
}

fn control_slider_change_handler(
    node_id: JellyNodeId,
    control: OpenGpuiControlPlan,
    actions: NodeComponentKitActions,
) -> impl Fn(open_gpui_ui_components::SliderChange, &mut Window, &mut App) + 'static {
    move |change, _window, cx| {
        if change.changed() {
            actions.dispatch_control(
                node_id,
                control.clone(),
                OpenGpuiControlEventValue::Number(change.value() as f64),
                cx,
            );
        }
    }
}

fn control_bool_change_handler(
    node_id: JellyNodeId,
    control: OpenGpuiControlPlan,
    actions: NodeComponentKitActions,
) -> impl Fn(bool, &open_gpui::ClickEvent, &mut Window, &mut App) + 'static {
    move |checked, _event, _window, cx| {
        actions.dispatch_control(
            node_id,
            control.clone(),
            OpenGpuiControlEventValue::Bool(checked),
            cx,
        );
    }
}

fn control_select_change_handler(
    node_id: JellyNodeId,
    control: OpenGpuiControlPlan,
    actions: NodeComponentKitActions,
) -> impl Fn(open_gpui_ui_components::SelectSelection, &mut Window, &mut App) + 'static {
    move |selection, _window, cx| {
        actions.dispatch_control(
            node_id,
            control.clone(),
            OpenGpuiControlEventValue::SelectOptionKey(selection.value().to_owned()),
            cx,
        );
    }
}

fn control_options(control: &OpenGpuiControlPlan) -> Vec<ListboxOption> {
    control
        .options
        .iter()
        .map(|option| {
            ListboxOption::new(control_option_key(option), option.label.clone())
                .disabled(option.disabled)
        })
        .collect()
}

pub(crate) fn control_component_disabled(control: &OpenGpuiControlPlan) -> bool {
    control.disabled_reason.is_some() || control.is_partial_stub()
}

pub(crate) fn control_component_read_only(control: &OpenGpuiControlPlan) -> bool {
    control.read_only || !control.is_editable()
}

pub(crate) fn control_component_interaction_disabled(control: &OpenGpuiControlPlan) -> bool {
    control_component_disabled(control) || control_component_read_only(control)
}

fn control_value_label(control: &OpenGpuiControlPlan) -> String {
    control
        .value
        .as_ref()
        .map(json_value_label)
        .unwrap_or_default()
}

fn control_number_value(control: &OpenGpuiControlPlan) -> f32 {
    control
        .value
        .as_ref()
        .and_then(|value| match value {
            Value::Number(number) => number.as_f64(),
            Value::String(text) => text.parse::<f64>().ok(),
            _ => None,
        })
        .unwrap_or_default() as f32
}

fn control_bool_value(control: &OpenGpuiControlPlan) -> bool {
    control
        .value
        .as_ref()
        .and_then(|value| match value {
            Value::Bool(value) => Some(*value),
            Value::String(text) => match text.as_str() {
                "true" | "yes" | "on" | "1" => Some(true),
                "false" | "no" | "off" | "0" => Some(false),
                _ => None,
            },
            _ => None,
        })
        .unwrap_or_default()
}

fn json_value_label(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(values) => values
            .iter()
            .map(json_value_label)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(_) => value.to_string(),
    }
}

fn action_button_variant(action: &OpenGpuiActionPlan, index: usize) -> ButtonVariant {
    if action.danger {
        ButtonVariant::Destructive
    } else if index == 0 {
        ButtonVariant::Default
    } else {
        ButtonVariant::Secondary
    }
}

fn action_button_label(action: &OpenGpuiActionPlan) -> String {
    action
        .icon_key
        .as_ref()
        .map(|icon| format!("{icon} {}", action.label))
        .unwrap_or_else(|| action.label.clone())
}

fn action_menu_item_label(action: &OpenGpuiActionPlan) -> String {
    match (&action.shortcut, &action.disabled_reason) {
        (Some(shortcut), Some(reason)) => format!("{} · {} · {}", action.label, shortcut, reason),
        (Some(shortcut), None) => format!("{} · {}", action.label, shortcut),
        (None, Some(reason)) => format!("{} · {}", action.label, reason),
        (None, None) => action.label.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adaptive_layout_stack_downgrades_regions_before_overflowing_node() {
        let mut layout = AdaptiveNodeLayoutStack::new(
            CanvasSize {
                width: 320.0,
                height: 150.0,
            },
            10.0,
            24.0,
            10.0,
            6.0,
        );

        let preview = layout.reserve_region("preview", 54.0, 32.0);
        let control = layout.reserve_region("control", 40.0, 28.0);
        let shell = layout.reserve_region("actions", 34.0, 24.0);

        assert_eq!(preview.mode, AdaptiveNodeLayoutMode::Full);
        assert_eq!(control.mode, AdaptiveNodeLayoutMode::Compact);
        assert_eq!(shell.mode, AdaptiveNodeLayoutMode::Shell);
        assert!(
            layout
                .regions()
                .iter()
                .all(|region| region.top + region.height <= 130.0)
        );
    }

    #[test]
    fn adaptive_repeatable_plan_reserves_overflow_indicator_height() {
        let plan = adaptive_repeatable_list_plan("table.columns", 90.0, 5, 4, 30.0, 4.0, 22.0);

        assert_eq!(plan.visible_items, 2);
        assert_eq!(plan.hidden_items, 3);
        assert_eq!(plan.region.mode, AdaptiveNodeLayoutMode::Compact);
        assert!(plan.region.height <= 90.0);
    }

    #[test]
    fn adaptive_repeatable_plan_shells_when_rows_cannot_fit() {
        let plan = adaptive_repeatable_list_plan("shader.inputs", 18.0, 3, 3, 30.0, 4.0, 22.0);

        assert_eq!(plan.visible_items, 0);
        assert_eq!(plan.hidden_items, 3);
        assert_eq!(plan.region.mode, AdaptiveNodeLayoutMode::Shell);
    }

    #[test]
    fn adaptive_text_plan_clamps_long_text_with_overflow_evidence() {
        let plan = adaptive_text_plan(
            "A long prompt that needs more than one rendered line in a compact product node",
            120.0,
            32.0,
            3,
            1,
        );

        assert_eq!(plan.mode, AdaptiveNodeLayoutMode::Compact);
        assert_eq!(plan.visible_lines, 1);
        assert!(plan.hidden_lines > 0);
        assert!(plan.overflow_indicator_required);
    }

    #[test]
    fn adaptive_text_plan_shells_when_no_line_can_fit() {
        let plan = adaptive_text_plan("source preview", 160.0, 8.0, 2, 1);

        assert_eq!(plan.mode, AdaptiveNodeLayoutMode::Shell);
        assert_eq!(plan.visible_lines, 0);
        assert!(plan.overflow_indicator_required);
    }

    #[test]
    fn adaptive_control_row_plan_allocates_compact_widths_without_clipping() {
        let plan = adaptive_control_row_plan(
            210.0,
            30.0,
            "Very long field label",
            "very-long-selected-model-name",
        );

        assert_eq!(plan.mode, AdaptiveNodeLayoutMode::Compact);
        assert!(plan.label_width >= 48.0);
        assert!(plan.control_width >= 112.0);
        assert!(!plan.clipped);
        assert!(plan.label_overflow || plan.value_overflow);
    }
}
