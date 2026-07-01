use super::*;
use jellyflow::runtime::runtime::measurement::MeasuredSurfaceAnchor;
use jellyflow_open_gpui::testing::{
    OpenGpuiHostRendererSource, OpenGpuiHostVisualInteractionReport, OpenGpuiHostVisualSurfaceRow,
    product_fixture_catalog,
};
use open_gpui_canvas::{
    CanvasConnectionEndpointRole, CanvasGeometryFacts, CanvasRuntime, connection_hit_options,
};

pub(super) fn canvas_host_visual_interaction_report() -> OpenGpuiHostVisualInteractionReport {
    let catalog = product_fixture_catalog();
    let renderer_registry = demo_node_renderer_registry();
    let renderers = demo_custom_node_renderers();
    let node_kit_registry = NodeKitRegistry::builtin();
    let semantic_registry = node_kit_registry.node_registry();
    let mut report = OpenGpuiHostVisualInteractionReport::default();

    for fixture in catalog {
        let (store, document, _projection) =
            project_kit_fixture(&fixture.kit_key, &fixture.fixture_key)
                .expect("product fixture projects into canvas document");
        report.add_node_bounds_overlap_count(node_bounds_overlap_count(&document));
        let measured_store = measurement_store_with_projection_fallback(&store, &semantic_registry);

        for canvas_node in document.nodes() {
            let Some(node_id) = jelly_node_id_from_node(canvas_node) else {
                continue;
            };
            let Some(graph_node) = store.graph().nodes().get(&node_id) else {
                continue;
            };
            let Some(surface) = node_surface_summary_for_node(
                canvas_node,
                node_id,
                graph_node,
                store.graph(),
                1.0,
                false,
                &semantic_registry,
                &node_kit_registry,
                measured_store.node_measurement(node_id),
            ) else {
                continue;
            };

            report.push(visual_surface_report_row(
                &fixture,
                &renderer_registry,
                &renderers,
                canvas_node,
                &measured_store,
                node_id,
                &surface,
            ));
        }
    }

    report.mark_invalid_hover_bounds_checked(invalid_hover_feedback_stays_inside_bounds());
    report.mark_dropped_wire_menu_bounds_checked(dropped_wire_menu_stays_inside_canvas_bounds());
    report.mark_repeatable_edit_updates_anchors(repeatable_edits_update_anchor_identity());
    report.mark_edge_endpoints_follow_measured_handles(edge_endpoints_follow_measured_handles());
    report
}

fn visual_surface_report_row(
    fixture: &jellyflow_open_gpui::testing::OpenGpuiProductFixtureCase,
    registry: &OpenGpuiNodeRendererRegistry,
    renderers: &GpuiNodeRendererTable,
    canvas_node: &CanvasNode,
    measured_store: &NodeGraphStore,
    node_id: JellyNodeId,
    surface: &NodeSurfaceSummary,
) -> OpenGpuiHostVisualSurfaceRow {
    let source = host_renderer_source(registry, renderers, &surface.renderer_context);
    let content_visible = !surface.title.is_empty()
        || !surface.summary.is_empty()
        || !surface.slots.is_empty()
        || !surface.repeatable_items.is_empty()
        || !surface.chrome.is_empty();
    let within_node_bounds = surface.document_bounds.size.width <= canvas_node.size.width.as_f32()
        && surface.document_bounds.size.height <= canvas_node.size.height.as_f32()
        && surface.document_bounds.size.width > 0.0
        && surface.document_bounds.size.height > 0.0;
    let content_readable = renderer_min_readable_size(&surface.node_kind, &surface.renderer_key)
        .is_none_or(|(min_width, min_height)| {
            canvas_node.size.width.as_f32() >= min_width
                && canvas_node.size.height.as_f32() >= min_height
        });
    let stale_regions = if measured_store.node_measurement_status(node_id).is_fresh() {
        0
    } else {
        1
    };
    let repeatable_rows = surface.repeatable_items.len();
    let repeatable_rows_with_anchors = surface
        .repeatable_items
        .iter()
        .filter(|item| !item.anchor.is_empty())
        .count();

    OpenGpuiHostVisualSurfaceRow::new(
        fixture,
        surface.node_kind.clone(),
        surface.renderer_key.clone(),
        source,
    )
    .with_selection(surface.selected)
    .with_content_bounds(content_visible, content_readable, within_node_bounds)
    .with_handle_overlap_count(handle_overlap_count(canvas_node))
    .with_stale_measured_regions(stale_regions)
    .with_repeatable_anchor_coverage(repeatable_rows, repeatable_rows_with_anchors)
}

fn renderer_min_readable_size(node_kind: &str, renderer_key: &str) -> Option<(f32, f32)> {
    match (node_kind, renderer_key) {
        ("demo.llm", "decision-card") => Some((292.0, 246.0)),
        (_, "decision-card") => Some((240.0, 150.0)),
        (_, "shader-card") => Some((324.0, 244.0)),
        (_, "table-card") => Some((372.0, 292.0)),
        (_, "topic-card") => Some((278.0, 190.0)),
        (_, "source-card") => Some((286.0, 190.0)),
        _ => None,
    }
}

fn host_renderer_source(
    registry: &OpenGpuiNodeRendererRegistry,
    renderers: &GpuiNodeRendererTable,
    context: &OpenGpuiNodeRendererContext,
) -> OpenGpuiHostRendererSource {
    match registry.resolve(context) {
        OpenGpuiNodeRendererResolution::Custom(registration) => {
            if renderers.contains_key(&registration.renderer_key) {
                OpenGpuiHostRendererSource::ProductRenderer
            } else {
                OpenGpuiHostRendererSource::MissingHostRenderer
            }
        }
        OpenGpuiNodeRendererResolution::Fallback(fallback) => match fallback.reason {
            jellyflow_open_gpui::OpenGpuiNodeRendererFallbackReason::MissingHostRenderer => {
                OpenGpuiHostRendererSource::MissingHostRenderer
            }
            jellyflow_open_gpui::OpenGpuiNodeRendererFallbackReason::UnregisteredRenderer => {
                OpenGpuiHostRendererSource::UnregisteredRenderer
            }
        },
    }
}

fn handle_overlap_count(node: &CanvasNode) -> usize {
    let width = node.size.width.as_f32();
    let height = node.size.height.as_f32();
    let rail_padding = 10.0;
    node.handles
        .iter()
        .filter(|handle| {
            let x = handle.position.x.as_f32();
            let y = handle.position.y.as_f32();
            x > rail_padding
                && x < width - rail_padding
                && y > rail_padding
                && y < height - rail_padding
        })
        .count()
}

fn node_bounds_overlap_count(document: &CanvasDocument) -> usize {
    let bounds = document
        .nodes()
        .filter(|node| !node.hidden)
        .map(CanvasNode::bounds)
        .collect::<Vec<_>>();
    let mut overlaps = 0;

    for (index, left) in bounds.iter().enumerate() {
        for right in bounds.iter().skip(index + 1) {
            if bounds_overlap(*left, *right) {
                overlaps += 1;
            }
        }
    }

    overlaps
}

fn bounds_overlap(left: Bounds<Pixels>, right: Bounds<Pixels>) -> bool {
    let left_min_x = left.origin.x.as_f32();
    let left_min_y = left.origin.y.as_f32();
    let left_max_x = left_min_x + left.size.width.as_f32();
    let left_max_y = left_min_y + left.size.height.as_f32();
    let right_min_x = right.origin.x.as_f32();
    let right_min_y = right.origin.y.as_f32();
    let right_max_x = right_min_x + right.size.width.as_f32();
    let right_max_y = right_min_y + right.size.height.as_f32();

    left_min_x < right_max_x
        && left_max_x > right_min_x
        && left_min_y < right_max_y
        && left_max_y > right_min_y
}

fn invalid_hover_feedback_stays_inside_bounds() -> bool {
    let (measured_store, transform, _, completion) = measured_transform_store();
    let Ok((document, _)) = project_store(&measured_store) else {
        return false;
    };
    let registry = jellyflow_kind_registry();
    let runtime = CanvasRuntime::rebuild_with_kind_registry(&document, &registry);
    let facts = CanvasGeometryFacts::with_kind_registry(&document, &registry);
    let Some(node) = document.node(&NodeId::from(canvas_node_id(&transform))) else {
        return false;
    };
    let measured_completion_point = node.position + point(px(268.0), px(150.0));
    let records = runtime
        .precise_hit_test_with_kind_registry(
            &document,
            &registry,
            measured_completion_point,
            connection_hit_options(),
        )
        .collect::<Vec<_>>();

    point_inside_or_on_bounds(node.bounds(), measured_completion_point)
        && records.iter().any(|record| {
            record.target
                == HitTarget::Handle {
                    node_id: NodeId::from(canvas_node_id(&transform)),
                    handle_id: open_gpui_canvas::HandleId::from(canvas_port_id(&completion)),
                }
        })
        && facts
            .connection_endpoint_at(
                records.iter().copied(),
                CanvasConnectionEndpointRole::Target,
            )
            .is_none()
}

fn point_inside_or_on_bounds(bounds: Bounds<Pixels>, point: open_gpui::Point<Pixels>) -> bool {
    let x = point.x.as_f32();
    let y = point.y.as_f32();
    let left = bounds.origin.x.as_f32();
    let top = bounds.origin.y.as_f32();
    let right = left + bounds.size.width.as_f32();
    let bottom = top + bounds.size.height.as_f32();
    x >= left && x <= right && y >= top && y <= bottom
}

fn dropped_wire_menu_stays_inside_canvas_bounds() -> bool {
    let store = make_demo_store();
    let registry = NodeKitRegistry::builtin().node_registry();
    let source_key = PortKey::new("completion");
    let Some(source) = dropped_wire_source_for_port_key(store.graph(), &source_key) else {
        return false;
    };
    let pointer = dropped_wire_insert_pointer(store.graph(), source);
    let menu = project_dropped_wire_menu(&registry, source, Some(&source_key), pointer);

    menu.surface == MenuSurface::DroppedWire
        && pointer.x.is_finite()
        && pointer.y.is_finite()
        && pointer.x >= 0.0
        && pointer.y >= 0.0
        && pointer.x <= CANVAS_WIDTH
        && pointer.y <= CANVAS_HEIGHT
        && menu
            .actions
            .iter()
            .any(|action| action.key == "action.insert.llm" && action.dispatchable())
}

fn repeatable_edits_update_anchor_identity() -> bool {
    let Ok((mut store, _document, _projection, node_id)) = project_schema_node("demo.shader.mix")
    else {
        return false;
    };
    let registry = NodeKitRegistry::builtin().node_registry();
    let Some(node) = store.graph().nodes().get(&node_id) else {
        return false;
    };
    let Some(descriptor) = registry.view_descriptor(&node.kind) else {
        return false;
    };
    let before_items = repeatable_item_projection(&descriptor, node, store.graph(), &node_id);
    let Some(factor_before) = before_items.iter().find(|item| item.item_id == "factor") else {
        return false;
    };
    let factor_anchor = factor_before.anchor.clone();

    let reorder = OpenGpuiAuthoringController.apply_repeatable_action_to_store(
        &mut store,
        &registry,
        node_id,
        OpenGpuiRepeatableActionPlan::Reorder {
            collection_key: "shader.inputs".to_owned(),
            item_id: "factor".to_owned(),
            to_index: 0,
        },
    );
    if !matches!(reorder, Ok(Some(_))) {
        return false;
    }
    let Some(node) = store.graph().nodes().get(&node_id) else {
        return false;
    };
    let after_reorder = repeatable_item_projection(&descriptor, node, store.graph(), &node_id);
    let Some(factor_after) = after_reorder.iter().find(|item| item.item_id == "factor") else {
        return false;
    };
    factor_after.item_index == 0 && factor_after.anchor == factor_anchor
}

fn edge_endpoints_follow_measured_handles() -> bool {
    let (measured_store, transform, prompt, completion) = measured_transform_store();
    let Ok((document, _)) = project_store(&measured_store) else {
        return false;
    };
    let Some(canvas_node) = document.node(&NodeId::from(canvas_node_id(&transform))) else {
        return false;
    };
    let Some(prompt_handle) = canvas_node
        .handles
        .iter()
        .find(|handle| handle.id.as_str() == canvas_port_id(&prompt))
    else {
        return false;
    };
    let Some(completion_handle) = canvas_node
        .handles
        .iter()
        .find(|handle| handle.id.as_str() == canvas_port_id(&completion))
    else {
        return false;
    };

    prompt_handle.position == point(px(0.0), px(51.0))
        && completion_handle.position == point(px(268.0), px(150.0))
}

fn measured_transform_store() -> (NodeGraphStore, JellyNodeId, JellyPortId, JellyPortId) {
    let store = make_demo_store();
    let transform = JellyNodeId::from_u128(3);
    let prompt = JellyPortId::from_u128(30);
    let completion = JellyPortId::from_u128(31);
    let mut measured_store = NodeGraphStore::new(
        store.graph().clone(),
        store.view_state().clone(),
        NodeGraphEditorConfig::default(),
    );
    measured_store
        .report_node_measurement(
            NodeMeasurement::new(transform)
                .with_revision(7)
                .with_size(Some(JellySize {
                    width: 268.0,
                    height: 228.0,
                }))
                .with_anchors([
                    MeasuredSurfaceAnchor::new(
                        "prompt.measured",
                        JellyRect {
                            origin: JellyPoint { x: 0.0, y: 42.0 },
                            size: JellySize {
                                width: 16.0,
                                height: 18.0,
                            },
                        },
                        HandlePosition::Left,
                    )
                    .with_port(prompt)
                    .with_port_key(PortKey::new("prompt")),
                    MeasuredSurfaceAnchor::new(
                        "completion.measured",
                        JellyRect {
                            origin: JellyPoint { x: 252.0, y: 140.0 },
                            size: JellySize {
                                width: 16.0,
                                height: 20.0,
                            },
                        },
                        HandlePosition::Right,
                    )
                    .with_port(completion)
                    .with_port_key(PortKey::new("completion")),
                ]),
        )
        .expect("measured transform node");
    (measured_store, transform, prompt, completion)
}
