use jellyflow::core::{CanvasSize, NodeId as JellyNodeId};
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
const TITLE_ROW_HEIGHT: f32 = 36.0;
const PREVIEW_ROW_HEIGHT: f32 = 54.0;
const PORT_RAIL_HEIGHT: f32 = 26.0;
const CONTROL_ROW_HEIGHT: f32 = 40.0;
const PROMPT_CONTROL_ROW_HEIGHT: f32 = 48.0;
const CONTROL_CHIP_HEIGHT: f32 = 34.0;
const REPEATABLE_CHIP_HEIGHT: f32 = 36.0;
const REPEATABLE_ROW_HEIGHT: f32 = 38.0;
const REPEATABLE_ADD_WIDTH: f32 = 96.0;
const SECTION_GAP: f32 = 6.0;
const BODY_TOP: f32 = CARD_PAD + HEADER_HEIGHT + SECTION_GAP;
const ANCHOR_TOP: f32 = BODY_TOP + SECTION_GAP;

#[derive(Clone, Copy)]
struct ProductLayoutRegion {
    top: Pixels,
    height: Pixels,
    mode: node_component_kit::AdaptiveNodeLayoutMode,
}

impl ProductLayoutRegion {
    fn from_adaptive(region: node_component_kit::AdaptiveNodeLayoutRegion) -> Self {
        Self {
            top: px(region.top),
            height: px(region.height),
            mode: region.mode,
        }
    }
}

fn adaptive_mode_min(
    left: node_component_kit::AdaptiveNodeLayoutMode,
    right: node_component_kit::AdaptiveNodeLayoutMode,
) -> node_component_kit::AdaptiveNodeLayoutMode {
    use node_component_kit::AdaptiveNodeLayoutMode::{Compact, Full, Shell};
    match (left, right) {
        (Shell, _) | (_, Shell) => Shell,
        (Compact, _) | (_, Compact) => Compact,
        (Full, Full) => Full,
    }
}

fn product_layout_stack(
    node_size: CanvasSize,
    footer_height: f32,
) -> node_component_kit::AdaptiveNodeLayoutStack {
    node_component_kit::AdaptiveNodeLayoutStack::new(
        node_size,
        CARD_PAD,
        HEADER_HEIGHT,
        footer_height,
        SECTION_GAP,
    )
}

fn reserve_product_region(
    layout: &mut node_component_kit::AdaptiveNodeLayoutStack,
    key: &'static str,
    full_height: f32,
    compact_height: f32,
) -> ProductLayoutRegion {
    ProductLayoutRegion::from_adaptive(layout.reserve_region(key, full_height, compact_height))
}

struct DecisionCardLayout {
    preview: ProductLayoutRegion,
    prompt_control: ProductLayoutRegion,
    model_control: ProductLayoutRegion,
    chip_row: ProductLayoutRegion,
}

fn decision_card_layout(node_size: CanvasSize) -> DecisionCardLayout {
    let mut layout = product_layout_stack(node_size, PORT_RAIL_HEIGHT);
    DecisionCardLayout {
        preview: reserve_product_region(&mut layout, "preview", PREVIEW_ROW_HEIGHT, 36.0),
        prompt_control: reserve_product_region(
            &mut layout,
            "prompt-control",
            PROMPT_CONTROL_ROW_HEIGHT,
            32.0,
        ),
        model_control: reserve_product_region(
            &mut layout,
            "model-control",
            CONTROL_ROW_HEIGHT,
            30.0,
        ),
        chip_row: reserve_product_region(&mut layout, "chip-row", CONTROL_CHIP_HEIGHT, 24.0),
    }
}

struct ShaderCardLayout {
    title: ProductLayoutRegion,
    input_rail: ProductLayoutRegion,
    input_chips: ProductLayoutRegion,
    control_row: ProductLayoutRegion,
    output_rail: ProductLayoutRegion,
}

fn shader_card_layout(node_size: CanvasSize) -> ShaderCardLayout {
    let mut layout = product_layout_stack(node_size, 0.0);
    ShaderCardLayout {
        title: reserve_product_region(&mut layout, "title", TITLE_ROW_HEIGHT, 28.0),
        input_rail: reserve_product_region(&mut layout, "input-rail", PORT_RAIL_HEIGHT, 20.0),
        input_chips: reserve_product_region(
            &mut layout,
            "input-chips",
            REPEATABLE_CHIP_HEIGHT,
            24.0,
        ),
        control_row: reserve_product_region(&mut layout, "control-row", CONTROL_CHIP_HEIGHT, 24.0),
        output_rail: reserve_product_region(&mut layout, "output-rail", PORT_RAIL_HEIGHT, 20.0),
    }
}

struct TableCardLayout {
    title: ProductLayoutRegion,
    primary_control: ProductLayoutRegion,
    chip_row: ProductLayoutRegion,
    columns_top: Pixels,
}

fn table_card_layout(node_size: CanvasSize) -> TableCardLayout {
    let mut layout = product_layout_stack(node_size, 0.0);
    let title = reserve_product_region(&mut layout, "title", TITLE_ROW_HEIGHT, 28.0);
    let primary_control =
        reserve_product_region(&mut layout, "primary-control", CONTROL_ROW_HEIGHT, 30.0);
    let chip_row = reserve_product_region(&mut layout, "chip-row", CONTROL_CHIP_HEIGHT, 24.0);
    let columns_top = px(layout
        .regions()
        .last()
        .map_or(BODY_TOP, |region| region.top + region.height + SECTION_GAP));
    TableCardLayout {
        title,
        primary_control,
        chip_row,
        columns_top,
    }
}

struct TopicCardLayout {
    title: ProductLayoutRegion,
    title_control: ProductLayoutRegion,
    summary_control: ProductLayoutRegion,
}

fn topic_card_layout(node_size: CanvasSize) -> TopicCardLayout {
    let mut layout = product_layout_stack(node_size, 0.0);
    TopicCardLayout {
        title: reserve_product_region(&mut layout, "title", TITLE_ROW_HEIGHT, 28.0),
        title_control: reserve_product_region(
            &mut layout,
            "title-control",
            CONTROL_ROW_HEIGHT,
            30.0,
        ),
        summary_control: reserve_product_region(
            &mut layout,
            "summary-control",
            CONTROL_ROW_HEIGHT,
            30.0,
        ),
    }
}

struct SourceCardLayout {
    preview: ProductLayoutRegion,
    title_control: ProductLayoutRegion,
    asset_control: ProductLayoutRegion,
}

fn source_card_layout(node_size: CanvasSize) -> SourceCardLayout {
    let mut layout = product_layout_stack(node_size, 0.0);
    SourceCardLayout {
        preview: reserve_product_region(&mut layout, "preview", PREVIEW_ROW_HEIGHT, 36.0),
        title_control: reserve_product_region(
            &mut layout,
            "title-control",
            CONTROL_ROW_HEIGHT,
            30.0,
        ),
        asset_control: reserve_product_region(
            &mut layout,
            "asset-control",
            CONTROL_ROW_HEIGHT,
            30.0,
        ),
    }
}

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
    let layout = decision_card_layout(context.node_size);
    let summary = context.summary.clone().unwrap_or_default();
    let summary_lines = text_line_clamp_for_region(
        &summary,
        context.node_size.width - CARD_PAD * 2.0,
        layout.preview,
        2,
        1,
    );
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
            .top(layout.preview.top)
            .right(px(CARD_PAD))
            .h(layout.preview.height)
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
                    .line_clamp(summary_lines)
                    .overflow_hidden()
                    .text_color(rgb(0x475569))
                    .child(summary),
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
    .child(render_control_row_with_height_at(
        context,
        "field.prompt",
        prompt_control.as_ref(),
        0,
        layout.prompt_control.top,
        layout.prompt_control.height,
        layout.prompt_control.mode,
        collector.clone(),
        &actions,
    ))
    .child(render_control_row_with_height_at(
        context,
        "badge.model",
        model_control.as_ref(),
        1,
        layout.model_control.top,
        layout.model_control.height,
        layout.model_control.mode,
        collector.clone(),
        &actions,
    ))
    .child(
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(layout.chip_row.top)
            .right(px(CARD_PAD))
            .h(layout.chip_row.height)
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
    let layout = shader_card_layout(context.node_size);
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
            .top(layout.title.top)
            .right(px(CARD_PAD))
            .h(layout.title.height)
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
        layout.input_rail.top,
        layout.input_rail.height,
        collector.clone(),
        rgb(0x312e81),
    ))
    .child(render_shader_inputs(
        context,
        &shader_inputs,
        layout.input_chips.top,
        layout.input_chips.height,
        collector.clone(),
        &actions,
    ))
    .child(
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(layout.control_row.top)
            .right(px(CARD_PAD))
            .h(layout.control_row.height)
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
        layout.output_rail.top,
        layout.output_rail.height,
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
    let layout = table_card_layout(context.node_size);

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
            .top(layout.title.top)
            .right(px(CARD_PAD))
            .h(layout.title.height)
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
    .child(render_control_row_with_height_at(
        context,
        "field.primary_key",
        primary_key.as_ref(),
        0,
        layout.primary_control.top,
        layout.primary_control.height,
        layout.primary_control.mode,
        collector.clone(),
        &actions,
    ))
    .child(
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(layout.chip_row.top)
            .right(px(CARD_PAD))
            .h(layout.chip_row.height)
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
        layout.columns_top,
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
    let layout = topic_card_layout(context.node_size);

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
            .top(layout.title.top)
            .right(px(CARD_PAD))
            .h(layout.title.height)
            .rounded_sm()
            .bg(rgb(0xffffff))
            .px_2()
            .flex()
            .items_center()
            .overflow_hidden()
            .child(text_line(context.title.clone(), rgb(0x111827), true)),
    ))
    .child(render_control_row_with_height_at(
        context,
        "header.main",
        title_control.as_ref(),
        0,
        layout.title_control.top,
        layout.title_control.height,
        layout.title_control.mode,
        collector.clone(),
        &actions,
    ))
    .child(render_control_row_with_height_at(
        context,
        "body.summary",
        summary_control.as_ref(),
        1,
        layout.summary_control.top,
        layout.summary_control.height,
        layout.summary_control.mode,
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
    let layout = source_card_layout(context.node_size);
    let preview = json_path_label(&context.node_data, &["preview"])
        .unwrap_or_else(|| "No preview".to_owned());
    let preview_lines = text_line_clamp_for_region(
        &preview,
        context.node_size.width - CARD_PAD * 2.0,
        layout.preview,
        2,
        1,
    );

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
            .top(layout.preview.top)
            .right(px(CARD_PAD))
            .h(layout.preview.height)
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
                    .line_clamp(preview_lines)
                    .overflow_hidden()
                    .text_color(rgb(0x475569))
                    .child(preview),
            ),
    ))
    .child(render_control_row_with_height_at(
        context,
        "header.main",
        title_control.as_ref(),
        0,
        layout.title_control.top,
        layout.title_control.height,
        layout.title_control.mode,
        collector.clone(),
        &actions,
    ))
    .child(render_control_row_with_height_at(
        context,
        "preview.main",
        asset_control.as_ref(),
        1,
        layout.asset_control.top,
        layout.asset_control.height,
        layout.asset_control.mode,
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

fn render_control_row_with_height_at(
    context: &OpenGpuiNodeRendererContext,
    slot_key: &str,
    control: Option<&OpenGpuiControlPlan>,
    index: usize,
    top: Pixels,
    height: Pixels,
    region_mode: node_component_kit::AdaptiveNodeLayoutMode,
    collector: OpenGpuiBoundsCollector,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let Some(control) = control else {
        return div().into_any_element();
    };
    let available_width = (context.node_size.width - CARD_PAD * 2.0).max(1.0);
    let value = control
        .value
        .as_ref()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let row_plan = node_component_kit::adaptive_control_row_plan(
        available_width,
        height.as_f32(),
        &control.label,
        &value,
    );
    let row_mode = adaptive_mode_min(region_mode, row_plan.mode);
    let show_shell = row_mode == node_component_kit::AdaptiveNodeLayoutMode::Shell;
    let label_width = row_plan.label_width.max(0.0);
    let control_width = row_plan.control_width.max(0.0);

    node_component_kit::render_measured_region(
        context.control_measurement_id(slot_key, control.key.clone()),
        collector,
        div()
            .absolute()
            .left(px(CARD_PAD))
            .top(top)
            .right(px(CARD_PAD))
            .h(height)
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
                    .w(px(label_width))
                    .text_xs()
                    .truncate()
                    .min_w(px(0.0))
                    .text_color(rgb(0x334155))
                    .child(control.label.clone()),
            )
            .child(if show_shell {
                Badge::new(
                    format!(
                        "jellyflow-control-shell:{}:{slot_key}:{index}",
                        context.node_id.0
                    ),
                    if row_plan.label_overflow || row_plan.value_overflow {
                        "more"
                    } else {
                        "set"
                    },
                )
                .variant(BadgeVariant::Outline)
                .with_size(Size::XSmall)
                .into_any_element()
            } else {
                div()
                    .w(px(control_width))
                    .min_w(px(control_width.min(112.0)))
                    .flex_shrink_0()
                    .overflow_hidden()
                    .child(node_component_kit::render_control_plan(
                        context.node_id,
                        "product-row",
                        control,
                        index,
                        actions,
                    ))
                    .into_any_element()
            }),
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
            .flex_1()
            .min_w(px(0.0))
            .max_w(px(168.0))
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
    height: Pixels,
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
            .h(height)
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
    height: Pixels,
    collector: OpenGpuiBoundsCollector,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let visible_limit = shader_visible_repeatable_limit_for_bounds(
        context.node_size.width,
        height.as_f32(),
        items.len(),
        context.surface_preset.repeatable_visible_items_or(3),
    );
    let hidden_count = items.len().saturating_sub(visible_limit);

    div()
        .absolute()
        .left(px(CARD_PAD))
        .top(top)
        .right(px(CARD_PAD))
        .h(height)
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

fn shader_visible_repeatable_limit_for_bounds(
    node_width: f32,
    available_height: f32,
    item_count: usize,
    budget_limit: usize,
) -> usize {
    let width_budget = (node_width - CARD_PAD * 2.0).max(1.0);
    let max_by_width = (width_budget / 104.0).floor().max(1.0) as usize;
    node_component_kit::adaptive_repeatable_list_plan(
        "shader.inputs",
        available_height,
        item_count,
        budget_limit.min(max_by_width),
        REPEATABLE_CHIP_HEIGHT,
        4.0,
        CONTROL_CHIP_HEIGHT,
    )
    .visible_items
    .min(max_by_width)
}

fn render_table_columns(
    context: &OpenGpuiNodeRendererContext,
    items: &[&OpenGpuiRepeatableItemLayout],
    top: Pixels,
    collector: OpenGpuiBoundsCollector,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    let visible_limit = table_visible_repeatable_limit(context, top, items.len());
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

fn table_visible_repeatable_limit(
    context: &OpenGpuiNodeRendererContext,
    top: Pixels,
    item_count: usize,
) -> usize {
    let budget_limit = context.surface_preset.repeatable_visible_items_or(3);
    table_visible_repeatable_limit_for_height(
        context.node_size.height,
        top.as_f32(),
        budget_limit,
        item_count,
    )
}

fn table_visible_repeatable_limit_for_height(
    node_height: f32,
    top: f32,
    budget_limit: usize,
    item_count: usize,
) -> usize {
    let available_height = (node_height - top - CARD_PAD).max(0.0);
    node_component_kit::adaptive_repeatable_list_plan(
        "table.columns",
        available_height,
        item_count,
        budget_limit,
        REPEATABLE_ROW_HEIGHT,
        4.0,
        CONTROL_CHIP_HEIGHT,
    )
    .visible_items
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
            .flex_1()
            .max_w(px(132.0))
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
                    .flex_1()
                    .items_center()
                    .gap_1()
                    .min_w(px(0.0))
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
                    .child(text_line(label.clone(), rgb(0x334155), false)),
            )
            .child(if repeatable_label_needs_overflow_badge(&label) {
                Badge::new(
                    format!(
                        "jellyflow-repeatable-text-overflow:{}:{}",
                        context.node_id.0, item_id
                    ),
                    "more",
                )
                .variant(BadgeVariant::Outline)
                .with_size(Size::XSmall)
                .into_any_element()
            } else {
                div().w(px(0.0)).h(px(0.0)).into_any_element()
            })
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
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

fn repeatable_label_needs_overflow_badge(label: &str) -> bool {
    label.chars().count() > 28
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

fn text_line_clamp_for_region(
    text: &str,
    available_width: f32,
    region: ProductLayoutRegion,
    full_line_budget: usize,
    compact_line_budget: usize,
) -> usize {
    node_component_kit::adaptive_text_plan(
        text,
        available_width,
        region.height.as_f32(),
        full_line_budget,
        compact_line_budget,
    )
    .visible_lines
    .max(1)
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

#[cfg(test)]
mod tests {
    use super::*;
    use jellyflow::{
        core::{CanvasSize, NodeKindKey},
        runtime::schema::NodeKitRegistry,
    };
    use jellyflow_open_gpui::OpenGpuiProductSurfacePreset;

    #[test]
    fn product_renderer_layouts_fit_runtime_readable_budgets() {
        assert_preset_fits_renderer(
            "demo.llm",
            CanvasSize {
                width: 320.0,
                height: decision_card_required_height(),
            },
        );
        assert_preset_fits_renderer(
            "demo.shader.mix",
            CanvasSize {
                width: 340.0,
                height: shader_card_required_height(),
            },
        );
        assert_preset_fits_renderer(
            "demo.table",
            CanvasSize {
                width: 396.0,
                height: table_card_required_height(),
            },
        );
        assert_preset_fits_renderer(
            "demo.topic",
            CanvasSize {
                width: 304.0,
                height: topic_card_required_height(),
            },
        );
        assert_preset_fits_renderer(
            "demo.source",
            CanvasSize {
                width: 312.0,
                height: source_card_required_height(),
            },
        );
    }

    fn layout_probe_size(width: f32) -> CanvasSize {
        CanvasSize {
            width,
            height: 1000.0,
        }
    }

    fn region_bottom(region: ProductLayoutRegion) -> f32 {
        region.top.as_f32() + region.height.as_f32()
    }

    fn decision_card_required_height() -> f32 {
        let layout = decision_card_layout(layout_probe_size(320.0));
        region_bottom(layout.chip_row) + SECTION_GAP + PORT_RAIL_HEIGHT + CARD_PAD
    }

    fn shader_card_required_height() -> f32 {
        let layout = shader_card_layout(layout_probe_size(340.0));
        region_bottom(layout.output_rail) + CARD_PAD
    }

    fn table_card_required_height() -> f32 {
        let layout = table_card_layout(layout_probe_size(396.0));
        layout.columns_top.as_f32() + (REPEATABLE_ROW_HEIGHT + 4.0) * 3.0 + CARD_PAD
    }

    fn topic_card_required_height() -> f32 {
        let layout = topic_card_layout(layout_probe_size(304.0));
        region_bottom(layout.summary_control) + CARD_PAD
    }

    fn source_card_required_height() -> f32 {
        let layout = source_card_layout(layout_probe_size(312.0));
        region_bottom(layout.asset_control) + CARD_PAD
    }

    fn assert_preset_fits_renderer(kind: &str, required: CanvasSize) {
        let registry = NodeKitRegistry::builtin().node_registry();
        let descriptor = registry
            .view_descriptor(&NodeKindKey::new(kind))
            .expect("builtin product descriptor");
        let preset = OpenGpuiProductSurfacePreset::from_descriptor(&descriptor);
        let minimum = preset
            .min_readable_size
            .expect("product renderer should publish min readable size");

        assert!(
            minimum.width >= required.width,
            "{kind} min width {} must fit renderer requirement {}",
            minimum.width,
            required.width
        );
        assert!(
            minimum.height >= required.height,
            "{kind} min height {} must fit renderer requirement {}",
            minimum.height,
            required.height
        );
    }

    #[test]
    fn table_repeatable_limit_accounts_for_overflow_indicator_budget() {
        let columns_top = table_card_layout(layout_probe_size(396.0))
            .columns_top
            .as_f32();
        let reduced_height = columns_top + CARD_PAD + REPEATABLE_ROW_HEIGHT * 2.0 + 4.0;

        assert_eq!(
            table_visible_repeatable_limit_for_height(reduced_height, columns_top, 4, 5),
            1
        );
        assert_eq!(
            table_visible_repeatable_limit_for_height(
                columns_top + CARD_PAD + (REPEATABLE_ROW_HEIGHT + 4.0) * 4.0,
                columns_top,
                4,
                4,
            ),
            4
        );
    }

    #[test]
    fn shader_repeatable_limit_accounts_for_width_and_height() {
        assert_eq!(
            shader_visible_repeatable_limit_for_bounds(220.0, 154.0, 4, 4),
            1
        );
        assert_eq!(
            shader_visible_repeatable_limit_for_bounds(420.0, 154.0, 4, 4),
            3
        );
        assert_eq!(
            shader_visible_repeatable_limit_for_bounds(420.0, 16.0, 4, 4),
            0
        );
    }

    #[test]
    fn product_layout_regions_preserve_compact_and_shell_modes() {
        let decision = decision_card_layout(CanvasSize {
            width: 320.0,
            height: 112.0,
        });

        assert_eq!(
            decision.preview.mode,
            node_component_kit::AdaptiveNodeLayoutMode::Compact
        );
        assert_eq!(
            decision.model_control.mode,
            node_component_kit::AdaptiveNodeLayoutMode::Shell
        );
    }

    #[test]
    fn product_card_layouts_stay_inside_reduced_nodes() {
        let decision_size = CanvasSize {
            width: 320.0,
            height: 210.0,
        };
        assert_layout_stays_inside(
            decision_card_layout(decision_size),
            decision_size.height - CARD_PAD - PORT_RAIL_HEIGHT,
        );
        let shader_size = CanvasSize {
            width: 340.0,
            height: 168.0,
        };
        assert_layout_stays_inside(
            shader_card_layout(shader_size),
            shader_size.height - CARD_PAD,
        );
        let table_size = CanvasSize {
            width: 396.0,
            height: 184.0,
        };
        assert_layout_stays_inside(table_card_layout(table_size), table_size.height - CARD_PAD);
        let topic_size = CanvasSize {
            width: 304.0,
            height: 132.0,
        };
        assert_layout_stays_inside(topic_card_layout(topic_size), topic_size.height - CARD_PAD);
        let source_size = CanvasSize {
            width: 312.0,
            height: 144.0,
        };
        assert_layout_stays_inside(
            source_card_layout(source_size),
            source_size.height - CARD_PAD,
        );
    }

    trait ProductLayoutRegions {
        fn regions(&self) -> Vec<ProductLayoutRegion>;
    }

    impl ProductLayoutRegions for DecisionCardLayout {
        fn regions(&self) -> Vec<ProductLayoutRegion> {
            vec![
                self.preview,
                self.prompt_control,
                self.model_control,
                self.chip_row,
            ]
        }
    }

    impl ProductLayoutRegions for ShaderCardLayout {
        fn regions(&self) -> Vec<ProductLayoutRegion> {
            vec![
                self.title,
                self.input_rail,
                self.input_chips,
                self.control_row,
                self.output_rail,
            ]
        }
    }

    impl ProductLayoutRegions for TopicCardLayout {
        fn regions(&self) -> Vec<ProductLayoutRegion> {
            vec![self.title, self.title_control, self.summary_control]
        }
    }

    impl ProductLayoutRegions for TableCardLayout {
        fn regions(&self) -> Vec<ProductLayoutRegion> {
            vec![self.title, self.primary_control, self.chip_row]
        }
    }

    impl ProductLayoutRegions for SourceCardLayout {
        fn regions(&self) -> Vec<ProductLayoutRegion> {
            vec![self.preview, self.title_control, self.asset_control]
        }
    }

    fn assert_layout_stays_inside(layout: impl ProductLayoutRegions, bottom_y: f32) {
        for region in layout.regions() {
            assert!(region.top.as_f32() >= BODY_TOP);
            assert!(region.height.as_f32() >= 0.0);
            assert!(region_bottom(region).is_finite());
            assert!(region_bottom(region) <= bottom_y + 0.01);
        }
    }
}
