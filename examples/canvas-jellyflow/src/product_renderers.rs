use std::collections::BTreeMap;

use jellyflow::core::{CanvasSize, NodeId as JellyNodeId};
use jellyflow_open_gpui::{
    OpenGpuiActionPlan, OpenGpuiBoundsCollector, OpenGpuiDynamicPortPolicy, OpenGpuiMenuPlan,
    OpenGpuiNodeRendererContext, OpenGpuiNodeRendererHostContext, OpenGpuiNodeRendererRegistry,
    OpenGpuiRepeatableActionPlan, OpenGpuiRepeatableItemLayout, OpenGpuiRepeatableSurfaceLayout,
    open_gpui_custom_action_missing_element_id, open_gpui_custom_repeatables_badge_element_id,
    open_gpui_custom_slots_badge_element_id, open_gpui_repeatable_add_action_element_id,
};
use open_gpui::{AnyElement, div, prelude::*, px, rgb};
use open_gpui_ui_components::{BadgeVariant, ButtonVariant};

use crate::{
    GpuiNodeRendererServices, GpuiNodeRendererTable, demo_repeatable_add_item,
    node_component_kit::{
        self, NodeComponentKitActions, PRODUCT_CARD_PAD, PRODUCT_CONTROL_CHIP_HEIGHT,
        PRODUCT_CONTROL_ROW_HEIGHT, PRODUCT_PORT_RAIL_HEIGHT, PRODUCT_PREVIEW_ROW_HEIGHT,
        PRODUCT_PROMPT_CONTROL_ROW_HEIGHT, PRODUCT_REPEATABLE_ADD_WIDTH,
        PRODUCT_REPEATABLE_ROW_HEIGHT, PRODUCT_TITLE_ROW_HEIGHT, ProductLayoutRegion,
        ProductNodeAnchorSide, ProductRepeatableLayoutPlan, product_layout_stack,
        reserve_product_region, reserve_product_repeatable_list,
    },
};

struct DecisionCardLayout {
    preview: ProductLayoutRegion,
    prompt_control: ProductLayoutRegion,
    model_control: ProductLayoutRegion,
    chip_row: ProductLayoutRegion,
}

fn decision_card_layout(node_size: CanvasSize) -> DecisionCardLayout {
    let mut layout = product_layout_stack(node_size, PRODUCT_PORT_RAIL_HEIGHT);
    DecisionCardLayout {
        preview: reserve_product_region(&mut layout, "preview", PRODUCT_PREVIEW_ROW_HEIGHT, 36.0),
        prompt_control: reserve_product_region(
            &mut layout,
            "prompt-control",
            PRODUCT_PROMPT_CONTROL_ROW_HEIGHT,
            32.0,
        ),
        model_control: reserve_product_region(
            &mut layout,
            "model-control",
            PRODUCT_CONTROL_ROW_HEIGHT,
            30.0,
        ),
        chip_row: reserve_product_region(
            &mut layout,
            "chip-row",
            PRODUCT_CONTROL_CHIP_HEIGHT,
            24.0,
        ),
    }
}

struct ShaderCardLayout {
    title: ProductLayoutRegion,
    input_rail: ProductLayoutRegion,
    input_rows: ProductRepeatableLayoutPlan,
    control_row: ProductLayoutRegion,
    output_rail: ProductLayoutRegion,
}

fn shader_card_layout(
    node_size: CanvasSize,
    input_count: usize,
    max_visible_inputs: usize,
) -> ShaderCardLayout {
    let mut layout = product_layout_stack(node_size, 0.0);
    ShaderCardLayout {
        title: reserve_product_region(&mut layout, "title", PRODUCT_TITLE_ROW_HEIGHT, 28.0),
        input_rail: reserve_product_region(
            &mut layout,
            "input-rail",
            PRODUCT_PORT_RAIL_HEIGHT,
            20.0,
        ),
        input_rows: reserve_product_repeatable_list(
            &mut layout,
            "shader.inputs",
            input_count,
            max_visible_inputs,
            PRODUCT_REPEATABLE_ROW_HEIGHT,
            4.0,
            PRODUCT_CONTROL_CHIP_HEIGHT,
        ),
        control_row: reserve_product_region(
            &mut layout,
            "control-row",
            PRODUCT_CONTROL_CHIP_HEIGHT,
            24.0,
        ),
        output_rail: reserve_product_region(
            &mut layout,
            "output-rail",
            PRODUCT_PORT_RAIL_HEIGHT,
            20.0,
        ),
    }
}

struct TableCardLayout {
    title: ProductLayoutRegion,
    primary_control: ProductLayoutRegion,
    chip_row: ProductLayoutRegion,
    columns: ProductRepeatableLayoutPlan,
}

fn table_card_layout(
    node_size: CanvasSize,
    column_count: usize,
    max_visible_columns: usize,
) -> TableCardLayout {
    let mut layout = product_layout_stack(node_size, 0.0);
    let title = reserve_product_region(&mut layout, "title", PRODUCT_TITLE_ROW_HEIGHT, 28.0);
    let primary_control = reserve_product_region(
        &mut layout,
        "primary-control",
        PRODUCT_CONTROL_ROW_HEIGHT,
        30.0,
    );
    let chip_row =
        reserve_product_region(&mut layout, "chip-row", PRODUCT_CONTROL_CHIP_HEIGHT, 24.0);
    let columns = reserve_product_repeatable_list(
        &mut layout,
        "table.columns",
        column_count,
        max_visible_columns,
        PRODUCT_REPEATABLE_ROW_HEIGHT,
        4.0,
        PRODUCT_CONTROL_CHIP_HEIGHT,
    );
    TableCardLayout {
        title,
        primary_control,
        chip_row,
        columns,
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
        title: reserve_product_region(&mut layout, "title", PRODUCT_TITLE_ROW_HEIGHT, 28.0),
        title_control: reserve_product_region(
            &mut layout,
            "title-control",
            PRODUCT_CONTROL_ROW_HEIGHT,
            30.0,
        ),
        summary_control: reserve_product_region(
            &mut layout,
            "summary-control",
            PRODUCT_CONTROL_ROW_HEIGHT,
            30.0,
        ),
    }
}

struct IdeaCardLayout {
    title: ProductLayoutRegion,
    title_control: ProductLayoutRegion,
}

fn idea_card_layout(node_size: CanvasSize) -> IdeaCardLayout {
    let mut layout = product_layout_stack(node_size, 0.0);
    IdeaCardLayout {
        title: reserve_product_region(&mut layout, "title", PRODUCT_TITLE_ROW_HEIGHT, 28.0),
        title_control: reserve_product_region(
            &mut layout,
            "title-control",
            PRODUCT_CONTROL_ROW_HEIGHT,
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
        preview: reserve_product_region(&mut layout, "preview", PRODUCT_PREVIEW_ROW_HEIGHT, 36.0),
        title_control: reserve_product_region(
            &mut layout,
            "title-control",
            PRODUCT_CONTROL_ROW_HEIGHT,
            30.0,
        ),
        asset_control: reserve_product_region(
            &mut layout,
            "asset-control",
            PRODUCT_CONTROL_ROW_HEIGHT,
            30.0,
        ),
    }
}

const PRODUCT_RENDERERS: [(&str, &str); 6] = [
    ("decision-card", "Dify workflow decision card"),
    ("shader-card", "Shader graph material card"),
    ("table-card", "ERD table editor card"),
    ("topic-card", "Mind-map topic card"),
    ("idea-card", "Mind-map idea card"),
    ("source-card", "Knowledge source card"),
];

type GpuiNodeComponentTable = BTreeMap<
    String,
    Box<
        dyn for<'a> Fn(
            &node_component_kit::OpenGpuiNodeComponentContext<'a, GpuiNodeRendererServices>,
        ) -> AnyElement,
    >,
>;

pub(crate) fn demo_node_renderer_registry() -> OpenGpuiNodeRendererRegistry {
    OpenGpuiNodeRendererRegistry::new().with_renderers(PRODUCT_RENDERERS)
}

pub(crate) fn demo_custom_node_renderers() -> GpuiNodeRendererTable {
    demo_node_components()
        .into_iter()
        .map(|(renderer_key, component)| (renderer_key, adapt_node_component(component)))
        .collect()
}

fn demo_node_components() -> GpuiNodeComponentTable {
    let mut renderers = GpuiNodeComponentTable::new();
    renderers.insert(
        "decision-card".to_owned(),
        Box::new(render_decision_card_component),
    );
    renderers.insert(
        "shader-card".to_owned(),
        Box::new(render_shader_card_component),
    );
    renderers.insert(
        "table-card".to_owned(),
        Box::new(render_table_card_component),
    );
    renderers.insert(
        "topic-card".to_owned(),
        Box::new(render_topic_card_component),
    );
    renderers.insert("idea-card".to_owned(), Box::new(render_idea_card_component));
    renderers.insert(
        "source-card".to_owned(),
        Box::new(render_source_card_component),
    );
    renderers
}

fn adapt_node_component(
    component: Box<
        dyn for<'a> Fn(
            &node_component_kit::OpenGpuiNodeComponentContext<'a, GpuiNodeRendererServices>,
        ) -> AnyElement,
    >,
) -> Box<dyn for<'a> Fn(&OpenGpuiNodeRendererHostContext<'a, GpuiNodeRendererServices>) -> AnyElement>
{
    Box::new(move |host| {
        let component_context = node_component_kit::OpenGpuiNodeComponentContext::from_host(host);
        component(&component_context)
    })
}

fn render_decision_card_component(
    component: &node_component_kit::OpenGpuiNodeComponentContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    debug_assert_eq!(component.props().renderer_key, "decision-card");
    render_decision_card(component)
}

fn render_shader_card_component(
    component: &node_component_kit::OpenGpuiNodeComponentContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    debug_assert_eq!(component.props().renderer_key, "shader-card");
    render_shader_card(component)
}

fn render_table_card_component(
    component: &node_component_kit::OpenGpuiNodeComponentContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    debug_assert_eq!(component.props().renderer_key, "table-card");
    render_table_card(component)
}

fn render_topic_card_component(
    component: &node_component_kit::OpenGpuiNodeComponentContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    debug_assert_eq!(component.props().renderer_key, "topic-card");
    render_topic_card(component)
}

fn render_idea_card_component(
    component: &node_component_kit::OpenGpuiNodeComponentContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    debug_assert_eq!(component.props().renderer_key, "idea-card");
    render_idea_card(component)
}

fn render_source_card_component(
    component: &node_component_kit::OpenGpuiNodeComponentContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    debug_assert_eq!(component.props().renderer_key, "source-card");
    render_source_card(component)
}

fn render_decision_card(
    component: &node_component_kit::OpenGpuiNodeComponentContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    let context = component.semantic();
    let collector = component.services().collector.clone();
    let view = component.services().view.clone();
    let actions = actions_for_component(component);
    let prompt_control = context.control("control.prompt");
    let model_control = context.control("control.model");
    let temperature_control = context.control("control.temperature");
    let stream_control = context.control("control.stream");
    let layout = decision_card_layout(context.node_size);
    let summary = context.summary.clone().unwrap_or_default();
    let summary_lines = node_component_kit::product_line_clamp(layout.preview.mode, 2, 1);
    let primary_action = context
        .toolbar_menu
        .actions
        .iter()
        .find(|action| action.key == "action.llm.run")
        .or_else(|| context.toolbar_menu.actions.first());

    node_component_kit::render_product_card(context, rgb(0x0f766e), rgb(0xf8fafc), view.clone())
        .child(node_component_kit::render_product_header(
            context,
            "Dify node",
            "workflow",
            rgb(0x0f766e),
            collector.clone(),
            view,
        ))
        .child(node_component_kit::render_product_panel(
            context,
            "field.prompt",
            collector.clone(),
            layout.preview.top,
            layout.preview.height,
            node_component_kit::ProductPanelStyle {
                background: rgb(0xecfeff),
                title: rgb(0x0f172a),
                body: rgb(0x475569),
                ..Default::default()
            },
            context.title.clone(),
            Some((summary, summary_lines)),
        ))
        .child(node_component_kit::render_product_side_anchor(
            context,
            "field.prompt",
            collector.clone(),
            ProductNodeAnchorSide::Left,
        ))
        .child(node_component_kit::render_product_side_anchor(
            context,
            "field.completion",
            collector.clone(),
            ProductNodeAnchorSide::Right,
        ))
        .child(node_component_kit::render_product_control_row(
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
        .child(node_component_kit::render_product_control_row(
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
                .left(px(PRODUCT_CARD_PAD))
                .top(layout.chip_row.top)
                .right(px(PRODUCT_CARD_PAD))
                .h(layout.chip_row.height)
                .flex()
                .items_center()
                .justify_between()
                .gap_1()
                .overflow_hidden()
                .child(node_component_kit::render_product_control_chip(
                    context,
                    "config.model",
                    temperature_control.as_ref(),
                    2,
                    collector.clone(),
                    &actions,
                ))
                .child(node_component_kit::render_product_control_chip(
                    context,
                    "config.model",
                    stream_control.as_ref(),
                    3,
                    collector.clone(),
                    &actions,
                ))
                .child(render_primary_action(
                    context.node_id,
                    &context.toolbar_menu,
                    primary_action,
                    &actions,
                )),
        )
        .child(node_component_kit::render_product_footer(
            context, collector,
        ))
        .into_any_element()
}

fn render_shader_card(
    component: &node_component_kit::OpenGpuiNodeComponentContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    let context = component.semantic();
    let collector = component.services().collector.clone();
    let view = component.services().view.clone();
    let actions = actions_for_component(component);
    let factor_control = context.control("control.factor");
    let texture_control = context.control("control.texture");
    let property_control = context.control("control.property.name");
    let shader_inputs = repeatable_items_for(context, "shader.inputs");
    let layout = shader_card_layout(
        context.node_size,
        shader_inputs.len(),
        context.surface_preset.repeatable_visible_items_or(3),
    );
    let missing_ports = shader_inputs
        .iter()
        .filter(|item| {
            item.projection.dynamic_port_policy == OpenGpuiDynamicPortPolicy::MissingGraphPort
        })
        .count();

    node_component_kit::render_product_card(context, rgb(0x7c3aed), rgb(0x111827), view.clone())
        .child(node_component_kit::render_product_header(
            context,
            "Shader graph",
            if missing_ports == 0 {
                "ports bound"
            } else {
                "missing ports"
            },
            rgb(0xa78bfa),
            collector.clone(),
            view,
        ))
        .child(
            div()
                .absolute()
                .left(px(PRODUCT_CARD_PAD))
                .top(layout.title.top)
                .right(px(PRODUCT_CARD_PAD))
                .h(layout.title.height)
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .overflow_hidden()
                .child(node_component_kit::render_product_text_line(
                    context.title.clone(),
                    rgb(0xf8fafc),
                    true,
                ))
                .child(node_component_kit::render_product_badge(
                    open_gpui_custom_slots_badge_element_id(context.node_id),
                    format!("{} dyn", shader_inputs.len()),
                    BadgeVariant::Default,
                )),
        )
        .child(node_component_kit::render_product_port_rail(
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
            layout.input_rows,
            collector.clone(),
            &actions,
        ))
        .child(
            div()
                .absolute()
                .left(px(PRODUCT_CARD_PAD))
                .top(layout.control_row.top)
                .right(px(PRODUCT_CARD_PAD))
                .h(layout.control_row.height)
                .flex()
                .items_center()
                .gap_1()
                .overflow_hidden()
                .child(node_component_kit::render_product_control_chip(
                    context,
                    "config.factor",
                    factor_control.as_ref().or(texture_control.as_ref()),
                    0,
                    collector.clone(),
                    &actions,
                ))
                .child(node_component_kit::render_product_control_chip(
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
        .child(node_component_kit::render_product_port_rail(
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
    component: &node_component_kit::OpenGpuiNodeComponentContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    let context = component.semantic();
    let collector = component.services().collector.clone();
    let view = component.services().view.clone();
    let actions = actions_for_component(component);
    let columns = repeatable_items_for(context, "table.columns");
    let primary_key = context.control("control.primary_key.name");
    let field_name = context.control("control.field.name");
    let field_type = context.control("control.field.type");
    let foreign_key = context.control("control.foreign_key.binding");
    let layout = table_card_layout(
        context.node_size,
        columns.len(),
        context.surface_preset.repeatable_visible_items_or(3),
    );

    node_component_kit::render_product_card(context, rgb(0x2563eb), rgb(0xf8fafc), view.clone())
        .child(node_component_kit::render_product_header(
            context,
            "ERD table",
            "schema",
            rgb(0x2563eb),
            collector.clone(),
            view,
        ))
        .child(
            div()
                .absolute()
                .left(px(PRODUCT_CARD_PAD))
                .top(layout.title.top)
                .right(px(PRODUCT_CARD_PAD))
                .h(layout.title.height)
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .overflow_hidden()
                .child(node_component_kit::render_product_text_line(
                    context.title.clone(),
                    rgb(0x111827),
                    true,
                ))
                .child(node_component_kit::render_product_badge(
                    open_gpui_custom_repeatables_badge_element_id(context.node_id),
                    format!("{} columns", columns.len()),
                    BadgeVariant::Secondary,
                )),
        )
        .child(node_component_kit::render_product_control_row(
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
                .left(px(PRODUCT_CARD_PAD))
                .top(layout.chip_row.top)
                .right(px(PRODUCT_CARD_PAD))
                .h(layout.chip_row.height)
                .flex()
                .items_center()
                .gap_1()
                .overflow_hidden()
                .child(node_component_kit::render_product_control_chip(
                    context,
                    "field.field",
                    field_name.as_ref(),
                    1,
                    collector.clone(),
                    &actions,
                ))
                .child(node_component_kit::render_product_control_chip(
                    context,
                    "field.field",
                    field_type.as_ref(),
                    2,
                    collector.clone(),
                    &actions,
                ))
                .child(node_component_kit::render_product_control_chip(
                    context,
                    "field.foreign_key",
                    foreign_key.as_ref(),
                    3,
                    collector.clone(),
                    &actions,
                ))
                .child(render_repeatable_add(
                    context,
                    context
                        .repeatables
                        .iter()
                        .find(|repeatable| repeatable.projection.key == "table.columns"),
                    &actions,
                )),
        )
        .child(render_table_columns(
            context,
            &columns,
            layout.columns,
            collector,
            &actions,
        ))
        .into_any_element()
}

fn render_topic_card(
    component: &node_component_kit::OpenGpuiNodeComponentContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    let context = component.semantic();
    let collector = component.services().collector.clone();
    let view = component.services().view.clone();
    let actions = actions_for_component(component);
    let title_control = context.control("control.topic.title");
    let summary_control = context.control("control.topic.summary");
    let layout = topic_card_layout(context.node_size);

    node_component_kit::render_product_card(context, rgb(0x8b5cf6), rgb(0xf5f3ff), view.clone())
        .child(node_component_kit::render_product_header(
            context,
            "Mind map",
            "topic",
            rgb(0x7c3aed),
            collector.clone(),
            view,
        ))
        .child(node_component_kit::render_product_panel(
            context,
            "header.main",
            collector.clone(),
            layout.title.top,
            layout.title.height,
            node_component_kit::ProductPanelStyle {
                background: rgb(0xffffff),
                title: rgb(0x111827),
                ..Default::default()
            },
            context.title.clone(),
            None,
        ))
        .child(node_component_kit::render_product_control_row(
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
        .child(node_component_kit::render_product_control_row(
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
        .child(node_component_kit::render_product_side_anchor(
            context,
            "body.summary",
            collector,
            ProductNodeAnchorSide::Right,
        ))
        .into_any_element()
}

fn render_idea_card(
    component: &node_component_kit::OpenGpuiNodeComponentContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    let context = component.semantic();
    let collector = component.services().collector.clone();
    let view = component.services().view.clone();
    let actions = actions_for_component(component);
    let title_control = context.control("control.idea.title");
    let layout = idea_card_layout(context.node_size);
    let summary = node_component_kit::json_path_label(&context.node_data, &["summary"]);

    node_component_kit::render_product_card(context, rgb(0xa855f7), rgb(0xfaf5ff), view.clone())
        .child(node_component_kit::render_product_header(
            context,
            "Mind map",
            "idea",
            rgb(0x9333ea),
            collector.clone(),
            view,
        ))
        .child(node_component_kit::render_product_panel(
            context,
            "header.main",
            collector.clone(),
            layout.title.top,
            layout.title.height,
            node_component_kit::ProductPanelStyle {
                background: rgb(0xffffff),
                title: rgb(0x111827),
                body: rgb(0x6b21a8),
                ..Default::default()
            },
            context.title.clone(),
            summary.map(|summary| (summary, 1)),
        ))
        .child(node_component_kit::render_product_control_row(
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
        .child(node_component_kit::render_product_side_anchor(
            context,
            "header.main",
            collector,
            ProductNodeAnchorSide::Right,
        ))
        .into_any_element()
}

fn render_source_card(
    component: &node_component_kit::OpenGpuiNodeComponentContext<'_, GpuiNodeRendererServices>,
) -> AnyElement {
    let context = component.semantic();
    let collector = component.services().collector.clone();
    let view = component.services().view.clone();
    let actions = actions_for_component(component);
    let title_control = context.control("control.source.title");
    let asset_control = context.control("control.source.asset");
    let layout = source_card_layout(context.node_size);
    let preview = node_component_kit::json_path_label(&context.node_data, &["preview"])
        .unwrap_or_else(|| "No preview".to_owned());
    let preview_lines = node_component_kit::product_line_clamp(layout.preview.mode, 2, 1);

    node_component_kit::render_product_card(context, rgb(0x0891b2), rgb(0xecfeff), view.clone())
        .child(node_component_kit::render_product_header(
            context,
            "Knowledge",
            "source",
            rgb(0x0e7490),
            collector.clone(),
            view,
        ))
        .child(node_component_kit::render_product_panel(
            context,
            "preview.main",
            collector.clone(),
            layout.preview.top,
            layout.preview.height,
            node_component_kit::ProductPanelStyle {
                background: rgb(0xffffff),
                title: rgb(0x0f172a),
                body: rgb(0x475569),
                ..Default::default()
            },
            context.title.clone(),
            Some((preview, preview_lines)),
        ))
        .child(node_component_kit::render_product_control_row(
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
        .child(node_component_kit::render_product_control_row(
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
        .child(node_component_kit::render_product_side_anchor(
            context,
            "preview.main",
            collector,
            ProductNodeAnchorSide::Right,
        ))
        .into_any_element()
}

fn actions_for_component(
    component: &node_component_kit::OpenGpuiNodeComponentContext<'_, GpuiNodeRendererServices>,
) -> NodeComponentKitActions {
    component.actions()
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
            node_component_kit::render_product_badge(
                open_gpui_custom_action_missing_element_id(node_id),
                "no action",
                BadgeVariant::Outline,
            )
        })
}

fn render_shader_inputs(
    context: &OpenGpuiNodeRendererContext,
    items: &[&OpenGpuiRepeatableItemLayout],
    plan: ProductRepeatableLayoutPlan,
    collector: OpenGpuiBoundsCollector,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    div()
        .absolute()
        .left(px(PRODUCT_CARD_PAD))
        .top(plan.region.top)
        .right(px(PRODUCT_CARD_PAD))
        .h(plan.region.height)
        .flex()
        .flex_col()
        .gap_1()
        .overflow_hidden()
        .children(items.iter().take(plan.visible_items).map(|item| {
            node_component_kit::render_product_repeatable_row(
                context,
                item,
                collector.clone(),
                actions,
            )
        }))
        .child(node_component_kit::render_product_overflow_affordance(
            context.node_id,
            "shader.inputs",
            plan.hidden_items,
            collector,
        ))
        .into_any_element()
}

fn render_table_columns(
    context: &OpenGpuiNodeRendererContext,
    items: &[&OpenGpuiRepeatableItemLayout],
    plan: ProductRepeatableLayoutPlan,
    collector: OpenGpuiBoundsCollector,
    actions: &NodeComponentKitActions,
) -> AnyElement {
    div()
        .absolute()
        .left(px(PRODUCT_CARD_PAD))
        .top(plan.region.top)
        .right(px(PRODUCT_CARD_PAD))
        .h(plan.region.height)
        .flex()
        .flex_col()
        .gap_1()
        .overflow_hidden()
        .children(items.iter().take(plan.visible_items).map(|item| {
            node_component_kit::render_product_repeatable_row(
                context,
                item,
                collector.clone(),
                actions,
            )
        }))
        .child(node_component_kit::render_product_overflow_affordance(
            context.node_id,
            "table.columns",
            plan.hidden_items,
            collector,
        ))
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
        .max_w(px(PRODUCT_REPEATABLE_ADD_WIDTH))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_component_kit::{PRODUCT_BODY_TOP, PRODUCT_SECTION_GAP};
    use jellyflow::{
        core::{CanvasSize, NodeKindKey},
        runtime::schema::NodeKitRegistry,
    };
    use jellyflow_open_gpui::OpenGpuiProductSurfacePreset;

    #[test]
    fn node_renderer_registry_covers_product_component_keys() {
        let registry = demo_node_renderer_registry();
        let components = demo_node_components();

        for (renderer_key, label) in PRODUCT_RENDERERS {
            assert!(registry.contains(renderer_key));
            let registration = registry
                .registration(renderer_key)
                .expect("product renderer registration");
            assert_eq!(registration.label, label);
            assert!(components.contains_key(renderer_key));
        }
    }

    #[test]
    fn product_renderer_full_layouts_fit_published_preferred_budgets() {
        assert_preferred_size_fits_renderer(
            "demo.llm",
            CanvasSize {
                width: 320.0,
                height: decision_card_required_height(),
            },
        );
        assert_preferred_size_fits_renderer(
            "demo.shader.mix",
            CanvasSize {
                width: 340.0,
                height: shader_card_required_height(),
            },
        );
        assert_preferred_size_fits_renderer(
            "demo.table",
            CanvasSize {
                width: 396.0,
                height: table_card_required_height(),
            },
        );
        assert_preferred_size_fits_renderer(
            "demo.topic",
            CanvasSize {
                width: 304.0,
                height: topic_card_required_height(),
            },
        );
        assert_preferred_size_fits_renderer(
            "demo.idea",
            CanvasSize {
                width: 248.0,
                height: idea_card_required_height(),
            },
        );
        assert_preferred_size_fits_renderer(
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
        region_bottom(layout.chip_row)
            + PRODUCT_SECTION_GAP
            + PRODUCT_PORT_RAIL_HEIGHT
            + PRODUCT_CARD_PAD
    }

    fn shader_card_required_height() -> f32 {
        let layout = shader_card_layout(layout_probe_size(340.0), 3, 3);
        region_bottom(layout.output_rail) + PRODUCT_CARD_PAD
    }

    fn table_card_required_height() -> f32 {
        let layout = table_card_layout(layout_probe_size(396.0), 3, 3);
        region_bottom(layout.columns.region) + PRODUCT_CARD_PAD
    }

    fn topic_card_required_height() -> f32 {
        let layout = topic_card_layout(layout_probe_size(304.0));
        region_bottom(layout.summary_control) + PRODUCT_CARD_PAD
    }

    fn idea_card_required_height() -> f32 {
        let layout = idea_card_layout(layout_probe_size(248.0));
        region_bottom(layout.title_control) + PRODUCT_CARD_PAD
    }

    fn source_card_required_height() -> f32 {
        let layout = source_card_layout(layout_probe_size(312.0));
        region_bottom(layout.asset_control) + PRODUCT_CARD_PAD
    }

    fn assert_preferred_size_fits_renderer(kind: &str, required: CanvasSize) {
        let registry = NodeKitRegistry::builtin().node_registry();
        let descriptor = registry
            .view_descriptor(&NodeKindKey::new(kind))
            .expect("builtin product descriptor");
        let preset = OpenGpuiProductSurfacePreset::from_descriptor(&descriptor);
        let preferred = preset
            .preferred_size
            .expect("product renderer should publish preferred size");

        assert!(
            preferred.width >= required.width,
            "{kind} preferred width {} must fit renderer requirement {}",
            preferred.width,
            required.width
        );
        assert!(
            preferred.height >= required.height,
            "{kind} preferred height {} must fit renderer requirement {}",
            preferred.height,
            required.height
        );
    }

    #[test]
    fn table_repeatable_limit_accounts_for_overflow_indicator_budget() {
        let constrained = table_card_layout(
            CanvasSize {
                width: 396.0,
                height: 258.0,
            },
            5,
            4,
        );
        assert_eq!(constrained.columns.visible_items, 1);
        assert_eq!(constrained.columns.hidden_items, 4);

        let full = table_card_layout(
            CanvasSize {
                width: 396.0,
                height: 360.0,
            },
            4,
            4,
        );
        assert_eq!(full.columns.visible_items, 4);
        assert_eq!(full.columns.hidden_items, 0);
    }

    #[test]
    fn shader_repeatable_plan_uses_region_height_not_width_fit() {
        let narrow = shader_card_layout(
            CanvasSize {
                width: 220.0,
                height: 252.0,
            },
            4,
            4,
        );
        let wide = shader_card_layout(
            CanvasSize {
                width: 420.0,
                height: 252.0,
            },
            4,
            4,
        );
        assert_eq!(narrow.input_rows.visible_items, 2);
        assert_eq!(narrow.input_rows.hidden_items, 2);
        assert_eq!(
            wide.input_rows.visible_items,
            narrow.input_rows.visible_items
        );
        assert_eq!(wide.input_rows.hidden_items, narrow.input_rows.hidden_items);

        let shell = shader_card_layout(
            CanvasSize {
                width: 420.0,
                height: 146.0,
            },
            4,
            4,
        );
        assert_eq!(shell.input_rows.visible_items, 0);
        assert_eq!(shell.input_rows.hidden_items, 4);
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
            decision_size.height - PRODUCT_CARD_PAD - PRODUCT_PORT_RAIL_HEIGHT,
        );
        let shader_size = CanvasSize {
            width: 340.0,
            height: 168.0,
        };
        assert_layout_stays_inside(
            shader_card_layout(shader_size, 3, 3),
            shader_size.height - PRODUCT_CARD_PAD,
        );
        let table_size = CanvasSize {
            width: 396.0,
            height: 184.0,
        };
        assert_layout_stays_inside(
            table_card_layout(table_size, 3, 3),
            table_size.height - PRODUCT_CARD_PAD,
        );
        let topic_size = CanvasSize {
            width: 304.0,
            height: 132.0,
        };
        assert_layout_stays_inside(
            topic_card_layout(topic_size),
            topic_size.height - PRODUCT_CARD_PAD,
        );
        let idea_size = CanvasSize {
            width: 248.0,
            height: 112.0,
        };
        assert_layout_stays_inside(
            idea_card_layout(idea_size),
            idea_size.height - PRODUCT_CARD_PAD,
        );
        let source_size = CanvasSize {
            width: 312.0,
            height: 144.0,
        };
        assert_layout_stays_inside(
            source_card_layout(source_size),
            source_size.height - PRODUCT_CARD_PAD,
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
                self.input_rows.region,
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

    impl ProductLayoutRegions for IdeaCardLayout {
        fn regions(&self) -> Vec<ProductLayoutRegion> {
            vec![self.title, self.title_control]
        }
    }

    impl ProductLayoutRegions for TableCardLayout {
        fn regions(&self) -> Vec<ProductLayoutRegion> {
            vec![
                self.title,
                self.primary_control,
                self.chip_row,
                self.columns.region,
            ]
        }
    }

    impl ProductLayoutRegions for SourceCardLayout {
        fn regions(&self) -> Vec<ProductLayoutRegion> {
            vec![self.preview, self.title_control, self.asset_control]
        }
    }

    fn assert_layout_stays_inside(layout: impl ProductLayoutRegions, bottom_y: f32) {
        for region in layout.regions() {
            assert!(region.top.as_f32() >= PRODUCT_BODY_TOP);
            assert!(region.height.as_f32() >= 0.0);
            assert!(region_bottom(region).is_finite());
            assert!(region_bottom(region) <= bottom_y + 0.01);
        }
    }
}
