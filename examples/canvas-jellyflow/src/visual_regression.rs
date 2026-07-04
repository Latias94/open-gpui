use super::*;
use std::collections::BTreeSet;

use jellyflow::runtime::runtime::measurement::{MeasuredSurfaceAnchor, MeasuredSurfaceSlot};
use jellyflow_open_gpui::{
    OpenGpuiMeasuredContentEvidence, OpenGpuiMeasurementCoverage,
    OpenGpuiProjectionMeasurementSource, OpenGpuiSizeEvidence,
    open_gpui_measured_content_evidence_from_slots, open_gpui_measured_internals_evidence,
    open_gpui_measured_region_kind_evidence,
    testing::{
        OpenGpuiHostCapabilityGap, OpenGpuiHostRendererSource, OpenGpuiHostSurfaceReport,
        OpenGpuiHostSurfaceReportRow, OpenGpuiHostVisualInteractionReport,
        OpenGpuiHostVisualSurfaceRow, OpenGpuiMeasuredInternalsEvidence,
        OpenGpuiMeasuredInternalsEvidenceInput, product_fixture_catalog,
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

pub(super) fn canvas_host_surface_report() -> OpenGpuiHostSurfaceReport {
    let catalog = product_fixture_catalog();
    let renderer_registry = demo_node_renderer_registry();
    let renderers = demo_custom_node_renderers();
    let node_kit_registry = NodeKitRegistry::builtin();
    let semantic_registry = node_kit_registry.node_registry();
    let mut report = OpenGpuiHostSurfaceReport::default();

    for fixture in catalog {
        let (store, document, _projection) =
            project_kit_fixture(&fixture.kit_key, &fixture.fixture_key)
                .expect("product fixture projects into canvas document");
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

            report.push(host_surface_report_row(
                &fixture,
                &renderer_registry,
                &renderers,
                &surface,
                measurement_projection
                    .evidence()
                    .node_uses_projection_fallback(node_id),
            ));
        }
    }

    report
}

fn host_surface_report_row(
    fixture: &jellyflow_open_gpui::testing::OpenGpuiProductFixtureCase,
    registry: &OpenGpuiNodeRendererRegistry,
    renderers: &GpuiNodeRendererTable,
    surface: &NodeSurfaceSummary,
    node_uses_projection_fallback: bool,
) -> OpenGpuiHostSurfaceReportRow {
    let measurement = surface.measurement.as_ref();
    let mut row = OpenGpuiHostSurfaceReportRow::new(
        fixture,
        surface.node_kind.clone(),
        surface.renderer_key.clone(),
        host_renderer_source(registry, renderers, &surface.renderer_context),
    )
    .with_measurement(
        measurement
            .map(|measurement| measurement.slots.len())
            .unwrap_or(0),
        measurement
            .map(|measurement| measurement.anchors.len())
            .unwrap_or(0),
    )
    .with_style_budget(surface.renderer_context.surface_preset.style.evidence());

    for slot in &surface.slot_descriptors {
        for control in project_slot_controls(&surface.node_data, slot) {
            if control.is_partial_stub() {
                row.capability_gaps
                    .insert(OpenGpuiHostCapabilityGap::AdvancedControlStub);
            }
        }
    }
    if surface
        .repeatable_items
        .iter()
        .any(|item| item.dynamic_port_policy == OpenGpuiDynamicPortPolicy::MissingGraphPort)
    {
        row.capability_gaps
            .insert(OpenGpuiHostCapabilityGap::MissingDynamicPort);
    }
    if surface.slots.iter().any(|slot| !slot.visible)
        || surface.projection.slot_limit < surface.slots.len()
    {
        row.capability_gaps
            .insert(OpenGpuiHostCapabilityGap::PartialOrHiddenRegion);
    }
    if node_uses_projection_fallback {
        row.capability_gaps
            .insert(OpenGpuiHostCapabilityGap::MissingMeasuredRegion);
    }

    row
}

fn visual_surface_report_row(
    fixture: &jellyflow_open_gpui::testing::OpenGpuiProductFixtureCase,
    registry: &OpenGpuiNodeRendererRegistry,
    renderers: &GpuiNodeRendererTable,
    canvas_node: &CanvasNode,
    measured_store: &NodeGraphStore,
    node_id: JellyNodeId,
    surface: &NodeSurfaceSummary,
    measurement_source: OpenGpuiProjectionMeasurementSource,
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

fn measured_content_evidence(
    surface: &NodeSurfaceSummary,
    canvas_node: &CanvasNode,
    measurement_coverage: Option<&OpenGpuiMeasurementCoverage>,
) -> OpenGpuiMeasuredContentEvidence {
    let Some(measurement) = surface.measurement.as_ref() else {
        return open_gpui_measured_region_kind_evidence(measurement_coverage);
    };
    let control_keys = expected_control_keys(surface);
    open_gpui_measured_content_evidence_from_slots(
        measurement.slots.iter(),
        &control_keys,
        measurement_coverage,
        JellySize {
            width: canvas_node.size.width.as_f32(),
            height: canvas_node.size.height.as_f32(),
        },
    )
}

fn expected_control_keys(surface: &NodeSurfaceSummary) -> BTreeSet<String> {
    surface
        .slot_descriptors
        .iter()
        .flat_map(|slot| project_slot_controls(&surface.node_data, slot))
        .map(|control| control.key)
        .collect()
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
    measurement_source: OpenGpuiProjectionMeasurementSource,
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
    open_gpui_measured_internals_evidence(OpenGpuiMeasuredInternalsEvidenceInput {
        renderer_source: source,
        measurement_source,
        measurement_coverage,
        node_bounds_present,
        measured_handle_count,
        measured_readable_regions,
        measured_control_regions,
        measured_drag_exclusion_regions,
        measured_overflow_regions,
        semantic_readable_region_count: semantic_readable_region_count(surface),
        fallback_stale_region_count: usize::from(
            !measured_store.node_measurement_status(node_id).is_fresh(),
        ),
        hidden_repeatable_overflow_count: hidden_repeatable_overflow,
        repeatable_overflow_indicator_count: repeatable_overflow_indicators,
    })
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

pub(super) fn repeatable_edits_update_anchor_identity() -> bool {
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

        let fallback_evidence = open_gpui_measured_content_evidence_from_slots(
            [&prompt_slot],
            &control_keys,
            None,
            node_size,
        );
        assert_eq!(fallback_evidence.control_region_count, 0);
        assert_eq!(fallback_evidence.drag_exclusion_region_count, 0);

        let layout_pass_evidence = open_gpui_measured_content_evidence_from_slots(
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
