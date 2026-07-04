use super::*;
use std::collections::BTreeSet;

use jellyflow::runtime::runtime::measurement::{MeasuredSurfaceAnchor, MeasuredSurfaceSlot};
use jellyflow_open_gpui::{
    OpenGpuiMeasurementCoverage, OpenGpuiSizeEvidence,
    testing::{
        OpenGpuiHostRendererSource, OpenGpuiHostVisualInteractionReport,
        OpenGpuiHostVisualSurfaceRow, OpenGpuiMeasuredInternalsEvidence,
        OpenGpuiMeasuredInternalsSource, product_fixture_catalog,
    },
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
        let measurement_projection =
            measurement_bridge::measurement_store_with_explicit_projection_fallback(
                &store,
                &semantic_registry,
            );
        let measured_store = measurement_projection.store();

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
                measurement_projection
                    .evidence()
                    .node_measurement_source(node_id),
                None,
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
    measurement_source: measurement_bridge::ProjectionFallbackMeasurementSource,
    measurement_coverage: Option<&OpenGpuiMeasurementCoverage>,
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
    let actual_size = OpenGpuiSizeEvidence::from_canvas_size(JellySize {
        width: canvas_node.size.width.as_f32(),
        height: canvas_node.size.height.as_f32(),
    });
    let measured_content = measured_content_evidence(surface, canvas_node, measurement_coverage);
    let content_readable = within_node_bounds
        && measured_content.text_overflow_count == 0
        && measured_content.clipped_control_count == 0;
    let stale_regions = if measured_store.node_measurement_status(node_id).is_fresh() {
        0
    } else {
        1
    };
    let repeatable_rows = surface.repeatable_items.len();
    let hidden_repeatable_overflow = hidden_repeatable_overflow_count(surface);
    let repeatable_overflow_indicators =
        repeatable_overflow_indicator_count(surface, hidden_repeatable_overflow);
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
    .with_readability_budget(actual_size, None)
    .with_text_overflow_count(measured_content.text_overflow_count)
    .with_control_clipping_count(measured_content.clipped_control_count)
    .with_handle_overlap_count(handle_overlap_count(canvas_node))
    .with_stale_measured_regions(stale_regions)
    .with_repeatable_anchor_coverage(repeatable_rows, repeatable_rows_with_anchors)
    .with_repeatable_overflow(hidden_repeatable_overflow, repeatable_overflow_indicators)
    .with_measured_internals_evidence(measured_internals_evidence(
        surface,
        canvas_node,
        measured_store,
        node_id,
        source,
        measurement_source,
        measured_content.readable_region_count,
        measured_content.control_region_count,
        measured_content.drag_exclusion_region_count,
        measured_content.overflow_region_count,
        measurement_coverage,
        hidden_repeatable_overflow,
        repeatable_overflow_indicators,
    ))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct MeasuredContentEvidence {
    text_overflow_count: usize,
    clipped_control_count: usize,
    readable_region_count: usize,
    control_region_count: usize,
    drag_exclusion_region_count: usize,
    overflow_region_count: usize,
}

fn measured_content_evidence(
    surface: &NodeSurfaceSummary,
    canvas_node: &CanvasNode,
    measurement_coverage: Option<&OpenGpuiMeasurementCoverage>,
) -> MeasuredContentEvidence {
    let Some(measurement) = surface.measurement.as_ref() else {
        return measured_region_kind_evidence(measurement_coverage);
    };
    let control_keys = expected_control_keys(surface);
    measured_content_evidence_from_slots(
        measurement.slots.iter(),
        &control_keys,
        measurement_coverage,
        JellySize {
            width: canvas_node.size.width.as_f32(),
            height: canvas_node.size.height.as_f32(),
        },
    )
}

fn measured_content_evidence_from_slots<'a>(
    slots: impl IntoIterator<Item = &'a MeasuredSurfaceSlot>,
    control_keys: &BTreeSet<String>,
    measurement_coverage: Option<&OpenGpuiMeasurementCoverage>,
    node_size: JellySize,
) -> MeasuredContentEvidence {
    let mut evidence = measured_region_kind_evidence(measurement_coverage);
    let has_measured_controls = evidence.control_region_count > 0;

    for slot in slots.into_iter().filter(|slot| slot.is_visible()) {
        if has_measured_controls && control_keys.contains(slot.key.as_str()) {
            if !rect_inside_size(slot.rect, node_size) {
                evidence.clipped_control_count += 1;
            }
        } else if !rect_inside_size(slot.rect, node_size) {
            evidence.text_overflow_count += 1;
        }
    }

    evidence
}

fn measured_region_kind_evidence(
    measurement_coverage: Option<&OpenGpuiMeasurementCoverage>,
) -> MeasuredContentEvidence {
    measurement_coverage
        .map(|coverage| MeasuredContentEvidence {
            text_overflow_count: 0,
            clipped_control_count: 0,
            readable_region_count: coverage.readable_regions,
            control_region_count: coverage.control_regions,
            drag_exclusion_region_count: coverage.drag_exclusion_regions,
            overflow_region_count: coverage.overflow_regions,
        })
        .unwrap_or_default()
}

fn expected_control_keys(surface: &NodeSurfaceSummary) -> BTreeSet<String> {
    surface
        .slot_descriptors
        .iter()
        .flat_map(|slot| project_slot_controls(&surface.node_data, slot))
        .map(|control| control.key)
        .collect()
}

fn rect_inside_size(rect: JellyRect, size: JellySize) -> bool {
    rect.origin.x >= 0.0
        && rect.origin.y >= 0.0
        && rect.origin.x + rect.size.width <= size.width
        && rect.origin.y + rect.size.height <= size.height
        && rect.size.width > 0.0
        && rect.size.height > 0.0
}

fn hidden_repeatable_overflow_count(surface: &NodeSurfaceSummary) -> usize {
    let visible_items = surface
        .renderer_context
        .surface_preset
        .repeatable_visible_items_or(usize::MAX);
    surface.repeatable_items.len().saturating_sub(visible_items)
}

fn repeatable_overflow_indicator_count(surface: &NodeSurfaceSummary, hidden_count: usize) -> usize {
    usize::from(
        hidden_count > 0
            && surface
                .renderer_context
                .surface_preset
                .overflow_indicator
                .is_some(),
    )
}

fn measured_internals_evidence(
    surface: &NodeSurfaceSummary,
    canvas_node: &CanvasNode,
    measured_store: &NodeGraphStore,
    node_id: JellyNodeId,
    source: OpenGpuiHostRendererSource,
    measurement_source: measurement_bridge::ProjectionFallbackMeasurementSource,
    measured_readable_regions: usize,
    measured_control_regions: usize,
    measured_drag_exclusion_regions: usize,
    measured_overflow_regions: usize,
    measurement_coverage: Option<&OpenGpuiMeasurementCoverage>,
    hidden_repeatable_overflow: usize,
    repeatable_overflow_indicators: usize,
) -> OpenGpuiMeasuredInternalsEvidence {
    let node_bounds_present =
        canvas_node.size.width.as_f32() > 0.0 && canvas_node.size.height.as_f32() > 0.0;
    let measured_handle_count = canvas_node.handles.len();
    let stale_region_count = measurement_coverage
        .map(|coverage| coverage.stale_regions)
        .unwrap_or_else(|| {
            usize::from(!measured_store.node_measurement_status(node_id).is_fresh())
        });
    let missing_required_overflow_count =
        usize::from(hidden_repeatable_overflow > 0 && repeatable_overflow_indicators == 0);
    let node_bounds_source = match measurement_source {
        measurement_bridge::ProjectionFallbackMeasurementSource::FreshLayoutPass
            if node_bounds_present
                && measurement_coverage.is_some_and(|coverage| coverage.is_full_layout_pass()) =>
        {
            OpenGpuiMeasuredInternalsSource::LayoutPass
        }
        measurement_bridge::ProjectionFallbackMeasurementSource::ProjectionFallback => {
            OpenGpuiMeasuredInternalsSource::ProjectionFallback
        }
        measurement_bridge::ProjectionFallbackMeasurementSource::Missing
        | measurement_bridge::ProjectionFallbackMeasurementSource::FreshLayoutPass => {
            OpenGpuiMeasuredInternalsSource::Missing
        }
    };
    let readable_region_count = if source == OpenGpuiHostRendererSource::ProductRenderer {
        measured_readable_regions
    } else {
        semantic_readable_region_count(surface)
    };

    OpenGpuiMeasuredInternalsEvidence {
        node_bounds_source,
        node_bounds_present,
        handle_bounds_present: measured_handle_count > 0,
        measured_handle_count,
        projected_handle_count: 0,
        readable_region_count,
        control_region_count: measured_control_regions,
        drag_exclusion_region_count: measured_drag_exclusion_regions,
        overflow_region_count: measured_overflow_regions,
        stale_region_count,
        component_declared_overflow_count: repeatable_overflow_indicators,
        missing_required_overflow_count,
    }
}

fn semantic_readable_region_count(surface: &NodeSurfaceSummary) -> usize {
    usize::from(!surface.title.trim().is_empty())
        + usize::from(!surface.summary.trim().is_empty())
        + surface.slots.len()
        + surface.repeatable_items.len()
        + surface.chrome.len()
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
    let Some(completion_handle) = node.handle(Some(&open_gpui_canvas::HandleId::from(
        canvas_port_id(&completion),
    ))) else {
        return false;
    };
    let measured_completion_point = node.position + completion_handle.position;
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
        && completion_handle.position == point(canvas_node.size.width, px(150.0))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn full_region_coverage() -> OpenGpuiMeasurementCoverage {
        OpenGpuiMeasurementCoverage {
            layout_pass_regions: 4,
            projection_fallback_regions: 0,
            missing_regions: 0,
            stale_regions: 0,
            partial_regions: 0,
            duplicate_regions: 0,
            measured_slots: 1,
            measured_anchors: 1,
            readable_regions: 1,
            control_regions: 1,
            drag_exclusion_regions: 1,
            overflow_regions: 1,
        }
    }

    #[test]
    fn measured_content_evidence_uses_coverage_region_kinds() {
        let prompt_slot = MeasuredSurfaceSlot::new(
            "prompt",
            JellyRect {
                origin: JellyPoint { x: 8.0, y: 12.0 },
                size: JellySize {
                    width: 120.0,
                    height: 24.0,
                },
            },
        );
        let control_keys = BTreeSet::from(["prompt".to_owned()]);
        let node_size = JellySize {
            width: 220.0,
            height: 160.0,
        };

        let fallback_evidence =
            measured_content_evidence_from_slots([&prompt_slot], &control_keys, None, node_size);
        assert_eq!(fallback_evidence.control_region_count, 0);
        assert_eq!(fallback_evidence.drag_exclusion_region_count, 0);

        let layout_pass_evidence = measured_content_evidence_from_slots(
            [&prompt_slot],
            &control_keys,
            Some(&full_region_coverage()),
            node_size,
        );
        assert_eq!(layout_pass_evidence.readable_region_count, 1);
        assert_eq!(layout_pass_evidence.control_region_count, 1);
        assert_eq!(layout_pass_evidence.drag_exclusion_region_count, 1);
        assert_eq!(layout_pass_evidence.overflow_region_count, 1);
    }
}
