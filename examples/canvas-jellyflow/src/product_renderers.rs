use jellyflow::core::NodeId as JellyNodeId;
use jellyflow_open_gpui::{
    OpenGpuiActionPlan, OpenGpuiBoundsCollector, OpenGpuiControlPlan, OpenGpuiDynamicPortPolicy,
    OpenGpuiMenuPlan, OpenGpuiNodeRendererContext, OpenGpuiNodeRendererHostContext,
    OpenGpuiNodeRendererRegistry, OpenGpuiRepeatableActionPlan, OpenGpuiRepeatableItemLayout,
    OpenGpuiRepeatableSurfaceLayout, open_gpui_custom_action_missing_element_id,
    open_gpui_custom_renderer_badge_element_id, open_gpui_custom_repeatables_badge_element_id,
    open_gpui_custom_slots_badge_element_id, open_gpui_repeatable_add_action_element_id,
    open_gpui_repeatable_item_element_id, open_gpui_repeatable_remove_action_element_id,
    open_gpui_repeatable_reorder_action_element_id,
};
use open_gpui::{AnyElement, MouseButton, Pixels, WeakEntity, div, prelude::*, px, rgb};
use open_gpui_ui_components::prelude::Sizable;
use open_gpui_ui_components::{Badge, BadgeVariant, ButtonVariant};
use open_gpui_ui_core::Size;
use serde_json::Value;

use crate::{
    GpuiNodeRendererServices, GpuiNodeRendererTable, JellyflowCanvasView, demo_repeatable_add_item,
    dispatch_node_drag_surface_mouse_down, node_component_kit,
    node_component_kit::NodeComponentKitActions, node_component_kit_actions,
};

const CARD_PAD: f32 = 10.0;
const HEADER_HEIGHT: f32 = 24.0;
const TITLE_ROW_HEIGHT: f32 = 34.0;
const PREVIEW_ROW_HEIGHT: f32 = 46.0;
const PORT_RAIL_HEIGHT: f32 = 24.0;
const CONTROL_ROW_HEIGHT: f32 = 34.0;
const CONTROL_CHIP_HEIGHT: f32 = 30.0;
const REPEATABLE_CHIP_HEIGHT: f32 = 34.0;
const REPEATABLE_ROW_HEIGHT: f32 = 34.0;
const REPEATABLE_ADD_WIDTH: f32 = 96.0;
const SECTION_GAP: f32 = 6.0;
const CONTROL_GROUP_GAP: f32 = 8.0;
const BODY_TOP: f32 = CARD_PAD + HEADER_HEIGHT + SECTION_GAP;
const TITLE_NEXT_TOP: f32 = BODY_TOP + TITLE_ROW_HEIGHT + SECTION_GAP;
const PREVIEW_NEXT_TOP: f32 = BODY_TOP + PREVIEW_ROW_HEIGHT + CONTROL_GROUP_GAP;
const SECOND_CONTROL_ROW_TOP: f32 = PREVIEW_NEXT_TOP + CONTROL_ROW_HEIGHT + SECTION_GAP;
const DECISION_CHIP_ROW_TOP: f32 = SECOND_CONTROL_ROW_TOP + CONTROL_ROW_HEIGHT + CARD_PAD;
const SHADER_INPUT_RAIL_TOP: f32 = TITLE_NEXT_TOP;
const SHADER_INPUT_CHIPS_TOP: f32 = SHADER_INPUT_RAIL_TOP + PORT_RAIL_HEIGHT + CONTROL_GROUP_GAP;
const SHADER_CONTROL_ROW_TOP: f32 =
    SHADER_INPUT_CHIPS_TOP + REPEATABLE_CHIP_HEIGHT + CONTROL_GROUP_GAP;
const SHADER_OUTPUT_RAIL_TOP: f32 = SHADER_CONTROL_ROW_TOP + CONTROL_CHIP_HEIGHT + 14.0;
const ERD_PRIMARY_ROW_TOP: f32 = TITLE_NEXT_TOP;
const ERD_CONTROL_CHIPS_TOP: f32 = ERD_PRIMARY_ROW_TOP + CONTROL_ROW_HEIGHT + CONTROL_GROUP_GAP;
const ERD_COLUMNS_TOP: f32 = ERD_CONTROL_CHIPS_TOP + CONTROL_CHIP_HEIGHT + 12.0;
const TOPIC_TITLE_CONTROL_TOP: f32 = BODY_TOP + TITLE_ROW_HEIGHT + CONTROL_GROUP_GAP;
const TOPIC_SUMMARY_CONTROL_TOP: f32 = TOPIC_TITLE_CONTROL_TOP + CONTROL_ROW_HEIGHT + SECTION_GAP;
const SOURCE_TITLE_CONTROL_TOP: f32 = PREVIEW_NEXT_TOP;
const SOURCE_ASSET_CONTROL_TOP: f32 = SOURCE_TITLE_CONTROL_TOP + CONTROL_ROW_HEIGHT + SECTION_GAP;
const ANCHOR_TOP: f32 = BODY_TOP + SECTION_GAP;

const PRODUCT_RENDERERS: [(&str, &str); 5] = [
    ("decision-card", "Dify workflow decision card"),
    ("shader-card", "Shader graph material card"),
    ("table-card", "ERD table editor card"),
    ("topic-card", "Mind-map topic card"),
    ("source-card", "Knowledge source card"),
];

pub(crate) fn demo_node_renderer_registry() -> OpenGpuiNodeRendererRegistry {
    OpenGpuiNodeRendererRegistry::new().with_renderers(PRODUCT_RENDERERS)
}

pub(crate) fn demo_custom_node_renderers() -> GpuiNodeRendererTable {
    let mut renderers = GpuiNodeRendererTable::new();
    renderers.insert("decision-card".to_owned(), Box::new(render_decision_card));
    renderers.insert("shader-card".to_owned(), Box::new(render_shader_card));
    renderers.insert("table-card".to_owned(), Box::new(render_table_card));
    renderers.insert("topic-card".to_owned(), Box::new(render_topic_card));
    renderers.insert("source-card".to_owned(), Box::new(render_source_card));
    renderers
}

fn render_decision_card(
    host: &OpenGpuiNodeRendererHostContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    let context = host.semantic();
    let collector = host.services().collector.clone();
    let actions = actions_for_host(host);
    let prompt_control = context.control("control.prompt");
    let model_control = context.control("control.model");
    let temperature_control = context.control("control.temperature");
    let stream_control = context.control("control.stream");
    let primary_action = context
        .toolbar_menu
        .actions
        .iter()
        .find(|action| action.key == "action.llm.run")
        .or_else(|| context.toolbar_menu.actions.first());

    product_card(
        context,
        rgb(0x0f766e),
        rgb(0xf8fafc),
        host.services().view.clone(),
    )
    .child(product_header(
        context,
        "Dify node",
        "workflow",
        rgb(0x0f766e),
        host.services().view.clone(),
    ))
    .child(node_component_kit::render_measured_region(
        context.slot_measurement_id("field.prompt"),
        collector.clone(),
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(px(BODY_TOP))
            .right(px(CARD_PAD))
            .h(px(PREVIEW_ROW_HEIGHT))
            .rounded_sm()
            .bg(rgb(0xecfeff))
            .px_2()
            .py_1()
            .overflow_hidden()
            .child(text_line(context.title.clone(), rgb(0x0f172a), true))
            .child(
                div()
                    .text_xs()
                    .line_height(px(14.0))
                    .truncate()
                    .text_color(rgb(0x475569))
                    .child(context.summary.clone().unwrap_or_default()),
            ),
    ))
    .child(measured_anchor(
        context,
        "field.prompt",
        collector.clone(),
        true,
    ))
    .child(measured_anchor(
        context,
        "field.completion",
        collector.clone(),
        false,
    ))
    .child(render_control_row_at(
        context,
        "field.prompt",
        prompt_control.as_ref(),
        0,
        px(PREVIEW_NEXT_TOP),
        collector.clone(),
        &actions,
    ))
    .child(render_control_row_at(
        context,
        "badge.model",
        model_control.as_ref(),
        1,
        px(SECOND_CONTROL_ROW_TOP),
        collector.clone(),
        &actions,
    ))
    .child(
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(px(DECISION_CHIP_ROW_TOP))
            .right(px(CARD_PAD))
            .h(px(CONTROL_CHIP_HEIGHT))
            .flex()
            .items_center()
            .justify_between()
            .gap_1()
            .overflow_hidden()
            .child(render_control_chip(
                context,
                "config.model",
                temperature_control.as_ref(),
                2,
                collector.clone(),
                &actions,
            ))
            .child(render_control_chip(
                context,
                "config.model",
                stream_control.as_ref(),
                3,
                collector,
                &actions,
            ))
            .child(render_primary_action(
                context.node_id,
                &context.toolbar_menu,
                primary_action,
                &actions,
            )),
    )
    .child(product_footer(context))
    .into_any_element()
}

fn render_shader_card(
    host: &OpenGpuiNodeRendererHostContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    let context = host.semantic();
    let collector = host.services().collector.clone();
    let actions = actions_for_host(host);
    let factor_control = context.control("control.factor");
    let texture_control = context.control("control.texture");
    let property_control = context.control("control.property.name");
    let shader_inputs = repeatable_items_for(context, "shader.inputs");
    let missing_ports = shader_inputs
        .iter()
        .filter(|item| {
            item.projection.dynamic_port_policy == OpenGpuiDynamicPortPolicy::MissingGraphPort
        })
        .count();

    product_card(
        context,
        rgb(0x7c3aed),
        rgb(0x111827),
        host.services().view.clone(),
    )
    .child(product_header(
        context,
        "Shader graph",
        if missing_ports == 0 {
            "ports bound"
        } else {
            "missing ports"
        },
        rgb(0xa78bfa),
        host.services().view.clone(),
    ))
    .child(
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(px(BODY_TOP))
            .right(px(CARD_PAD))
            .h(px(TITLE_ROW_HEIGHT))
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .overflow_hidden()
            .child(text_line(context.title.clone(), rgb(0xf8fafc), true))
            .child(
                Badge::new(
                    open_gpui_custom_slots_badge_element_id(context.node_id),
                    format!("{} dyn", shader_inputs.len()),
                )
                .variant(BadgeVariant::Default)
                .with_size(Size::XSmall),
            ),
    )
    .child(render_port_rail(
        context,
        "rail.inputs",
        "inputs",
        px(SHADER_INPUT_RAIL_TOP),
        collector.clone(),
        rgb(0x312e81),
    ))
    .child(render_shader_inputs(
        context,
        &shader_inputs,
        px(SHADER_INPUT_CHIPS_TOP),
        collector.clone(),
        &actions,
    ))
    .child(
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(px(SHADER_CONTROL_ROW_TOP))
            .right(px(CARD_PAD))
            .h(px(CONTROL_CHIP_HEIGHT))
            .flex()
            .items_center()
            .gap_1()
            .overflow_hidden()
            .child(render_control_chip(
                context,
                "config.factor",
                factor_control.as_ref().or(texture_control.as_ref()),
                0,
                collector.clone(),
                &actions,
            ))
            .child(render_control_chip(
                context,
                "property",
                property_control.as_ref(),
                1,
                collector.clone(),
                &actions,
            ))
            .child(render_repeatable_add(
                context,
                context
                    .repeatables
                    .iter()
                    .find(|repeatable| repeatable.projection.key == "shader.inputs"),
                &actions,
            )),
    )
    .child(render_port_rail(
        context,
        "rail.outputs",
        "outputs",
        px(SHADER_OUTPUT_RAIL_TOP),
        collector,
        rgb(0x1e293b),
    ))
    .into_any_element()
}

fn render_table_card(
    host: &OpenGpuiNodeRendererHostContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    let context = host.semantic();
    let collector = host.services().collector.clone();
    let actions = actions_for_host(host);
    let columns = repeatable_items_for(context, "table.columns");
    let primary_key = context.control("control.primary_key.name");
    let field_name = context.control("control.field.name");
    let field_type = context.control("control.field.type");
    let foreign_key = context.control("control.foreign_key.binding");

    product_card(
        context,
        rgb(0x2563eb),
        rgb(0xf8fafc),
        host.services().view.clone(),
    )
    .child(product_header(
        context,
        "ERD table",
        "schema",
        rgb(0x2563eb),
        host.services().view.clone(),
    ))
    .child(
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(px(BODY_TOP))
            .right(px(CARD_PAD))
            .h(px(TITLE_ROW_HEIGHT))
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .overflow_hidden()
            .child(text_line(context.title.clone(), rgb(0x111827), true))
            .child(
                Badge::new(
                    open_gpui_custom_repeatables_badge_element_id(context.node_id),
                    format!("{} columns", columns.len()),
                )
                .variant(BadgeVariant::Secondary)
                .with_size(Size::XSmall),
            ),
    )
    .child(render_control_row_at(
        context,
        "field.primary_key",
        primary_key.as_ref(),
        0,
        px(ERD_PRIMARY_ROW_TOP),
        collector.clone(),
        &actions,
    ))
    .child(
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(px(ERD_CONTROL_CHIPS_TOP))
            .right(px(CARD_PAD))
            .h(px(CONTROL_CHIP_HEIGHT))
            .flex()
            .items_center()
            .gap_1()
            .overflow_hidden()
            .child(render_control_chip(
                context,
                "field.field",
                field_name.as_ref(),
                1,
                collector.clone(),
                &actions,
            ))
            .child(render_control_chip(
                context,
                "field.field",
                field_type.as_ref(),
                2,
                collector.clone(),
                &actions,
            ))
            .child(render_control_chip(
                context,
                "field.foreign_key",
                foreign_key.as_ref(),
                3,
                collector.clone(),
                &actions,
            )),
    )
    .child(render_table_columns(
        context,
        &columns,
        px(ERD_COLUMNS_TOP),
        collector,
        &actions,
    ))
    .child(render_repeatable_add(
        context,
        context
            .repeatables
            .iter()
            .find(|repeatable| repeatable.projection.key == "table.columns"),
        &actions,
    ))
    .into_any_element()
}

fn render_topic_card(
    host: &OpenGpuiNodeRendererHostContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    let context = host.semantic();
    let collector = host.services().collector.clone();
    let actions = actions_for_host(host);
    let title_control = context.control("control.topic.title");
    let summary_control = context.control("control.topic.summary");

    product_card(
        context,
        rgb(0x8b5cf6),
        rgb(0xf5f3ff),
        host.services().view.clone(),
    )
    .child(product_header(
        context,
        "Mind map",
        "topic",
        rgb(0x7c3aed),
        host.services().view.clone(),
    ))
    .child(node_component_kit::render_measured_region(
        context.slot_measurement_id("header.main"),
        collector.clone(),
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(px(BODY_TOP))
            .right(px(CARD_PAD))
            .h(px(TITLE_ROW_HEIGHT))
            .rounded_sm()
            .bg(rgb(0xffffff))
            .px_2()
            .flex()
            .items_center()
            .overflow_hidden()
            .child(text_line(context.title.clone(), rgb(0x111827), true)),
    ))
    .child(render_control_row_at(
        context,
        "header.main",
        title_control.as_ref(),
        0,
        px(TOPIC_TITLE_CONTROL_TOP),
        collector.clone(),
        &actions,
    ))
    .child(render_control_row_at(
        context,
        "body.summary",
        summary_control.as_ref(),
        1,
        px(TOPIC_SUMMARY_CONTROL_TOP),
        collector.clone(),
        &actions,
    ))
    .child(measured_anchor(context, "body.summary", collector, false))
    .into_any_element()
}

fn render_source_card(
    host: &OpenGpuiNodeRendererHostContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    let context = host.semantic();
    let collector = host.services().collector.clone();
    let actions = actions_for_host(host);
    let title_control = context.control("control.source.title");
    let asset_control = context.control("control.source.asset");

    product_card(
        context,
        rgb(0x0891b2),
        rgb(0xecfeff),
        host.services().view.clone(),
    )
    .child(product_header(
        context,
        "Knowledge",
        "source",
        rgb(0x0e7490),
        host.services().view.clone(),
    ))
    .child(node_component_kit::render_measured_region(
        context.slot_measurement_id("preview.main"),
        collector.clone(),
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(px(BODY_TOP))
            .right(px(CARD_PAD))
            .h(px(PREVIEW_ROW_HEIGHT))
            .rounded_sm()
            .bg(rgb(0xffffff))
            .px_2()
            .py_1()
            .overflow_hidden()
            .child(text_line(context.title.clone(), rgb(0x0f172a), true))
            .child(
                div()
                    .text_xs()
                    .line_height(px(14.0))
                    .truncate()
                    .text_color(rgb(0x475569))
                    .child(
                        json_path_label(&context.node_data, &["preview"])
                            .unwrap_or_else(|| "No preview".to_owned()),
                    ),
            ),
    ))
    .child(render_control_row_at(
        context,
        "header.main",
        title_control.as_ref(),
        0,
        px(SOURCE_TITLE_CONTROL_TOP),
        collector.clone(),
        &actions,
    ))
    .child(render_control_row_at(
        context,
        "preview.main",
        asset_control.as_ref(),
        1,
        px(SOURCE_ASSET_CONTROL_TOP),
        collector.clone(),
        &actions,
    ))
    .child(measured_anchor(context, "preview.main", collector, false))
    .into_any_element()
}

fn actions_for_host(
    host: &OpenGpuiNodeRendererHostContext<'_, GpuiNodeRendererServices>,
) -> NodeComponentKitActions {
    node_component_kit_actions(host.services().view.clone())
}

fn product_card(
    context: &OpenGpuiNodeRendererContext,
    accent: open_gpui::Rgba,
    fill: open_gpui::Rgba,
    view: WeakEntity<JellyflowCanvasView>,
) -> open_gpui::Div {
    div()
        .size_full()
        .relative()
        .rounded_sm()
        .border_1()
        .border_color(if context.state.selected {
            rgb(0x2563eb)
        } else {
            accent
        })
        .bg(fill)
        .overflow_hidden()
        .shadow_sm()
        .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
            if dispatch_node_drag_surface_mouse_down(view.clone(), event, cx) {
                cx.stop_propagation();
            }
        })
}

fn product_header(
    context: &OpenGpuiNodeRendererContext,
    family: &'static str,
    status: &'static str,
    accent: open_gpui::Rgba,
    view: WeakEntity<JellyflowCanvasView>,
) -> AnyElement {
    div()
        .absolute()
        .left(px(CARD_PAD))
        .top(px(CARD_PAD))
        .right(px(CARD_PAD))
        .h(px(HEADER_HEIGHT))
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .overflow_hidden()
        .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
            if dispatch_node_drag_surface_mouse_down(view.clone(), event, cx) {
                cx.stop_propagation();
            }
        })
        .child(
            Badge::new(
                open_gpui_custom_renderer_badge_element_id(context.node_id, &context.renderer_key),
                family,
            )
            .variant(BadgeVariant::Default)
            .with_size(Size::XSmall),
        )
        .child(
            div()
                .text_xs()
                .truncate()
                .min_w(px(0.0))
                .text_color(accent)
                .child(status),
        )
        .into_any_element()
}

fn product_footer(context: &OpenGpuiNodeRendererContext) -> AnyElement {
    div()
        .absolute()
        .left(px(CARD_PAD))
        .bottom(px(CARD_PAD))
        .right(px(CARD_PAD))
        .h(px(PORT_RAIL_HEIGHT))
        .flex()
        .items_center()
        .gap_1()
        .overflow_hidden()
        .child(
            Badge::new(
                open_gpui_custom_slots_badge_element_id(context.node_id),
                format!("{} slots", context.surface_layout.slots.len()),
            )
            .variant(BadgeVariant::Secondary)
            .with_size(Size::XSmall),
        )
        .child(
            Badge::new(
                open_gpui_custom_repeatables_badge_element_id(context.node_id),
                format!("{} repeatables", context.repeatable_items.len()),
            )
            .variant(BadgeVariant::Outline)
            .with_size(Size::XSmall),
        )
        .into_any_element()
}

fn render_primary_action(
    node_id: JellyNodeId,
    menu: &OpenGpuiMenuPlan,
    action: Option<&OpenGpuiActionPlan>,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    action
        .map(|action| {
            node_component_kit::render_dispatch_action_button(
                menu,
                action,
                0,
                Some(node_id),
                actions,
            )
        })
        .unwrap_or_else(|| {
            Badge::new(
                open_gpui_custom_action_missing_element_id(node_id),
                "no action",
            )
            .variant(BadgeVariant::Outline)
            .with_size(Size::XSmall)
            .into_any_element()
        })
}

fn render_control_row_at(
    context: &OpenGpuiNodeRendererContext,
    slot_key: &str,
    control: Option<&OpenGpuiControlPlan>,
    index: usize,
    top: Pixels,
    collector: OpenGpuiBoundsCollector,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let Some(control) = control else {
        return div().into_any_element();
    };
    node_component_kit::render_measured_region(
        context.control_measurement_id(slot_key, control.key.clone()),
        collector,
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(top)
            .right(px(CARD_PAD))
            .h(px(CONTROL_ROW_HEIGHT))
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .rounded_sm()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xcbd5e1))
            .px_2()
            .overflow_hidden()
            .child(
                div()
                    .text_xs()
                    .truncate()
                    .min_w(px(0.0))
                    .text_color(rgb(0x334155))
                    .child(control.label.clone()),
            )
            .child(div().max_w(px(176.0)).overflow_hidden().child(
                node_component_kit::render_control_plan(
                    context.node_id,
                    "product-row",
                    control,
                    index,
                    actions,
                ),
            )),
    )
}

fn render_control_chip(
    context: &OpenGpuiNodeRendererContext,
    slot_key: &str,
    control: Option<&OpenGpuiControlPlan>,
    index: usize,
    collector: OpenGpuiBoundsCollector,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let Some(control) = control else {
        return div().into_any_element();
    };
    node_component_kit::render_measured_region(
        context.control_measurement_id(slot_key, control.key.clone()),
        collector,
        div()
            .h(px(CONTROL_CHIP_HEIGHT))
            .max_w(px(144.0))
            .overflow_hidden()
            .child(node_component_kit::render_control_plan(
                context.node_id,
                slot_key,
                control,
                index,
                actions,
            )),
    )
}

fn render_port_rail(
    context: &OpenGpuiNodeRendererContext,
    slot_key: &str,
    label: &'static str,
    top: Pixels,
    collector: OpenGpuiBoundsCollector,
    fill: open_gpui::Rgba,
) -> AnyElement {
    let value = context
        .surface_slots
        .iter()
        .find(|slot| slot.key == slot_key)
        .map(|slot| slot.value.clone())
        .unwrap_or_default();

    node_component_kit::render_measured_region(
        context.slot_measurement_id(slot_key),
        collector,
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(top)
            .right(px(CARD_PAD))
            .h(px(PORT_RAIL_HEIGHT))
            .rounded_sm()
            .bg(fill)
            .px_2()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .overflow_hidden()
            .child(text_line(label.to_owned(), rgb(0xf8fafc), false))
            .child(text_line(value, rgb(0xcbd5e1), false)),
    )
}

fn render_shader_inputs(
    context: &OpenGpuiNodeRendererContext,
    items: &[&OpenGpuiRepeatableItemLayout],
    top: Pixels,
    collector: OpenGpuiBoundsCollector,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let visible_limit = context.surface_preset.repeatable_visible_items_or(3);
    let hidden_count = items.len().saturating_sub(visible_limit);

    div()
        .absolute()
        .left(px(CARD_PAD))
        .top(top)
        .right(px(CARD_PAD))
        .h(px(REPEATABLE_CHIP_HEIGHT))
        .flex()
        .items_center()
        .gap_1()
        .overflow_hidden()
        .children(
            items
                .iter()
                .take(visible_limit)
                .map(|item| render_repeatable_item_chip(context, item, collector.clone(), actions)),
        )
        .child(render_repeatable_overflow_indicator(
            context,
            "shader.inputs",
            hidden_count,
        ))
        .into_any_element()
}

fn render_table_columns(
    context: &OpenGpuiNodeRendererContext,
    items: &[&OpenGpuiRepeatableItemLayout],
    top: Pixels,
    collector: OpenGpuiBoundsCollector,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let visible_limit = table_visible_repeatable_limit(context);
    let hidden_count = items.len().saturating_sub(visible_limit);

    div()
        .absolute()
        .left(px(CARD_PAD))
        .top(top)
        .right(px(CARD_PAD))
        .bottom(px(CARD_PAD))
        .flex()
        .flex_col()
        .gap_1()
        .overflow_hidden()
        .children(
            items
                .iter()
                .take(visible_limit)
                .map(|item| render_repeatable_item_row(context, item, collector.clone(), actions)),
        )
        .child(render_repeatable_overflow_indicator(
            context,
            "table.columns",
            hidden_count,
        ))
        .into_any_element()
}

fn table_visible_repeatable_limit(context: &OpenGpuiNodeRendererContext) -> usize {
    let budget_limit = context.surface_preset.repeatable_visible_items_or(3);
    let available_height = (context.node_size.height - ERD_COLUMNS_TOP - CARD_PAD).max(0.0);
    let row_stride = REPEATABLE_ROW_HEIGHT + 4.0;
    let fitting_rows = (available_height / row_stride).floor().max(1.0) as usize;
    budget_limit.min(fitting_rows)
}

fn render_repeatable_overflow_indicator(
    context: &OpenGpuiNodeRendererContext,
    collection_key: &str,
    hidden_count: usize,
) -> AnyElement {
    if hidden_count == 0 {
        return div().w(px(0.0)).h(px(0.0)).into_any_element();
    }

    Badge::new(
        format!(
            "jellyflow-repeatable-overflow:{}:{collection_key}",
            context.node_id.0
        ),
        format!("+{hidden_count}"),
    )
    .variant(BadgeVariant::Secondary)
    .with_size(Size::XSmall)
    .into_any_element()
}

fn render_repeatable_item_chip(
    context: &OpenGpuiNodeRendererContext,
    item: &OpenGpuiRepeatableItemLayout,
    collector: OpenGpuiBoundsCollector,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let projection = &item.projection;
    let label = repeatable_item_label(&projection.item_data, &projection.label);
    let disabled = projection.remove_disabled_reason.is_some();
    let missing_port =
        projection.dynamic_port_policy == OpenGpuiDynamicPortPolicy::MissingGraphPort;
    let label = if missing_port {
        format!("{label} no port")
    } else {
        label
    };
    let collection_key = projection.collection_key.clone();
    let item_id = projection.item_id.clone();
    let anchor = projection.anchor.clone();

    node_component_kit::render_measured_region(
        context.repeatable_item_measurement_id(projection.slot_key.clone(), item_id.clone()),
        collector.clone(),
        div()
            .h(px(REPEATABLE_CHIP_HEIGHT))
            .min_w(px(84.0))
            .max_w(px(118.0))
            .flex()
            .items_center()
            .gap_1()
            .rounded_sm()
            .bg(if missing_port {
                rgb(0xfffbeb)
            } else {
                rgb(0xede9fe)
            })
            .border_1()
            .border_color(if missing_port {
                rgb(0xf59e0b)
            } else {
                rgb(0xa78bfa)
            })
            .px_1()
            .overflow_hidden()
            .child(text_line(label, rgb(0x111827), false))
            .child(node_component_kit::repeatable_action_button(
                context.node_id,
                open_gpui_repeatable_remove_action_element_id(
                    context.node_id,
                    &collection_key,
                    &item_id,
                ),
                "Del",
                ButtonVariant::Destructive,
                disabled,
                OpenGpuiRepeatableActionPlan::Remove {
                    collection_key,
                    item_id: item_id.clone(),
                },
                actions,
            ))
            .child(hidden_anchor_measurement(context, anchor, collector)),
    )
}

fn render_repeatable_item_row(
    context: &OpenGpuiNodeRendererContext,
    item: &OpenGpuiRepeatableItemLayout,
    collector: OpenGpuiBoundsCollector,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let projection = &item.projection;
    let label = repeatable_item_label(&projection.item_data, &projection.label);
    let ty = json_path_label(&projection.item_data, &["ty"]).unwrap_or_else(|| "field".to_owned());
    let collection_key = projection.collection_key.clone();
    let item_id = projection.item_id.clone();
    let disabled = projection.remove_disabled_reason.is_some();
    let item_index = projection.item_index;
    let anchor = projection.anchor.clone();
    let dynamic_port_policy = projection.dynamic_port_policy;

    node_component_kit::render_measured_region(
        context.repeatable_item_measurement_id(projection.slot_key.clone(), item_id.clone()),
        collector.clone(),
        div()
            .h(px(REPEATABLE_ROW_HEIGHT))
            .flex()
            .items_center()
            .justify_between()
            .gap_1()
            .rounded_sm()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xcbd5e1))
            .px_2()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .overflow_hidden()
                    .child(
                        Badge::new(
                            open_gpui_repeatable_item_element_id(
                                context.node_id,
                                &projection.collection_key,
                                &projection.item_id,
                            ),
                            ty,
                        )
                        .variant(BadgeVariant::Outline)
                        .with_size(Size::XSmall),
                    )
                    .child(text_line(label, rgb(0x334155), false)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(repeatable_port_policy_badge(
                        context,
                        &collection_key,
                        &item_id,
                        dynamic_port_policy,
                    ))
                    .child(node_component_kit::repeatable_action_button(
                        context.node_id,
                        open_gpui_repeatable_reorder_action_element_id(
                            context.node_id,
                            &collection_key,
                            &item_id,
                        ),
                        "Up",
                        ButtonVariant::Secondary,
                        item_index == 0,
                        OpenGpuiRepeatableActionPlan::Reorder {
                            collection_key: collection_key.clone(),
                            item_id: item_id.clone(),
                            to_index: item_index.saturating_sub(1),
                        },
                        actions,
                    ))
                    .child(node_component_kit::repeatable_action_button(
                        context.node_id,
                        open_gpui_repeatable_remove_action_element_id(
                            context.node_id,
                            &collection_key,
                            &item_id,
                        ),
                        "Del",
                        ButtonVariant::Destructive,
                        disabled,
                        OpenGpuiRepeatableActionPlan::Remove {
                            collection_key,
                            item_id: item_id.clone(),
                        },
                        actions,
                    )),
            )
            .child(hidden_anchor_measurement(context, anchor, collector)),
    )
}

fn repeatable_port_policy_badge(
    context: &OpenGpuiNodeRendererContext,
    collection_key: &str,
    item_id: &str,
    policy: OpenGpuiDynamicPortPolicy,
) -> AnyElement {
    if policy != OpenGpuiDynamicPortPolicy::MissingGraphPort {
        return div().w(px(0.0)).h(px(0.0)).into_any_element();
    }

    Badge::new(
        format!(
            "jellyflow-repeatable-port-policy:{}:{collection_key}:{item_id}",
            context.node_id.0
        ),
        "no port",
    )
    .variant(BadgeVariant::Destructive)
    .with_size(Size::XSmall)
    .into_any_element()
}

fn render_repeatable_add(
    context: &OpenGpuiNodeRendererContext,
    repeatable: Option<&OpenGpuiRepeatableSurfaceLayout>,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let Some(repeatable) = repeatable else {
        return div().into_any_element();
    };
    let collection_key = repeatable.projection.key.clone();
    let add_disabled = repeatable.projection.add_disabled_reason.is_some();
    let item = demo_repeatable_add_item(&collection_key, repeatable.projection.item_count);

    div()
        .max_w(px(REPEATABLE_ADD_WIDTH))
        .overflow_hidden()
        .child(node_component_kit::repeatable_action_button(
            context.node_id,
            open_gpui_repeatable_add_action_element_id(context.node_id, &collection_key),
            "Add",
            ButtonVariant::Secondary,
            add_disabled,
            OpenGpuiRepeatableActionPlan::Add {
                collection_key,
                item,
            },
            actions,
        ))
        .into_any_element()
}

fn measured_anchor(
    context: &OpenGpuiNodeRendererContext,
    anchor_key: &str,
    collector: OpenGpuiBoundsCollector,
    left: bool,
) -> AnyElement {
    node_component_kit::render_measured_region(
        context.anchor_measurement_id(anchor_key),
        collector,
        div()
            .absolute()
            .left(if left { px(0.0) } else { px(9999.0) })
            .right(if left { px(9999.0) } else { px(0.0) })
            .top(px(ANCHOR_TOP))
            .w(px(8.0))
            .h(px(20.0)),
    )
}

fn hidden_anchor_measurement(
    context: &OpenGpuiNodeRendererContext,
    anchor_key: String,
    collector: OpenGpuiBoundsCollector,
) -> AnyElement {
    node_component_kit::render_measured_region(
        context.anchor_measurement_id(anchor_key),
        collector,
        div()
            .absolute()
            .left(px(0.0))
            .top(px(0.0))
            .w(px(1.0))
            .h(px(1.0)),
    )
}

fn repeatable_items_for<'a>(
    context: &'a OpenGpuiNodeRendererContext,
    collection_key: &str,
) -> Vec<&'a OpenGpuiRepeatableItemLayout> {
    context
        .repeatable_items
        .iter()
        .filter(|item| item.projection.collection_key == collection_key)
        .collect()
}

fn text_line(label: String, color: open_gpui::Rgba, strong: bool) -> AnyElement {
    div()
        .text_sm()
        .line_height(px(if strong { 18.0 } else { 16.0 }))
        .truncate()
        .min_w(px(0.0))
        .text_color(color)
        .child(label)
        .into_any_element()
}

fn repeatable_item_label(item_data: &Value, fallback: &str) -> String {
    json_path_label(item_data, &["name"])
        .or_else(|| json_path_label(item_data, &["title"]))
        .unwrap_or_else(|| fallback.to_owned())
}

fn json_path_label(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    match current {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}
