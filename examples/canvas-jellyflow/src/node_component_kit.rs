use std::rc::Rc;

use jellyflow::{core::NodeId as JellyNodeId, runtime::schema::NodeSurfaceSlotProjection};
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

pub fn render_node_internal_interaction_region(child: AnyElement) -> AnyElement {
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

    render_node_internal_interaction_region(element)
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

    button.into_any_element()
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

    Menu::new(
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
    .into_any_element()
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

    render_node_internal_interaction_region(button.into_any_element())
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
