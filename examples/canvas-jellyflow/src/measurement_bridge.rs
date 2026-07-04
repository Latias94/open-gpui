use jellyflow::{
    NodeGraphStore,
    core::{CanvasSize as JellySize, Graph, Node, NodeId as JellyNodeId},
    runtime::{
        io::NodeGraphEditorConfig,
        runtime::measurement::{NodeMeasurement, NodeMeasurementStatus},
        schema::{NodeKindViewDescriptor, NodeRegistry},
    },
};
use jellyflow_open_gpui::{
    OpenGpuiMeasuredRegion, OpenGpuiMeasurementContext, OpenGpuiMeasurementCoverage,
    OpenGpuiProjectionFallbackStoreEvidence, OpenGpuiViewPoint,
    assign_layout_pass_measurement_revision, layout_pass_measurement_from_regions,
    measured_surface_anchors, project_node_measurement, projected_node_surface_graph_layout,
};
use open_gpui_canvas::{CanvasDocument, CanvasViewport, NodeId};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayoutPassMeasurementConsume {
    NoRegions,
    Unchanged,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LayoutPassMeasurementNodeInput {
    pub(crate) node_id: JellyNodeId,
    pub(crate) node_size: JellySize,
    pub(crate) node_view_origin: OpenGpuiViewPoint,
    pub(crate) view_to_document_scale: f32,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct LayoutPassMeasurementConsumeResult {
    pub(crate) outcome: LayoutPassMeasurementConsume,
    pub(crate) coverage: BTreeMap<JellyNodeId, OpenGpuiMeasurementCoverage>,
}

impl Default for LayoutPassMeasurementConsume {
    fn default() -> Self {
        Self::NoRegions
    }
}

pub(crate) struct ProjectionFallbackStore {
    store: NodeGraphStore,
    evidence: OpenGpuiProjectionFallbackStoreEvidence,
}

impl ProjectionFallbackStore {
    pub(crate) fn store(&self) -> &NodeGraphStore {
        &self.store
    }

    pub(crate) fn evidence(&self) -> &OpenGpuiProjectionFallbackStoreEvidence {
        &self.evidence
    }

    pub(crate) fn into_store(self) -> NodeGraphStore {
        self.store
    }
}

pub(crate) fn layout_pass_measurement_for_node(
    node_id: JellyNodeId,
    node: &Node,
    graph: &Graph,
    descriptor: &NodeKindViewDescriptor,
    node_size: JellySize,
    node_view_origin: OpenGpuiViewPoint,
    view_to_document_scale: f32,
    node_regions: impl IntoIterator<Item = OpenGpuiMeasuredRegion>,
) -> (NodeMeasurement, OpenGpuiMeasurementCoverage) {
    let fallback_layout =
        projected_node_surface_graph_layout(descriptor, node, graph, &node_id, node_size);
    let fallback_anchors = measured_surface_anchors(descriptor, graph, &node_id, &fallback_layout);
    let context = OpenGpuiMeasurementContext::new(
        node_id,
        node_view_origin,
        view_to_document_scale,
        node_size,
    )
    .with_revision(0);
    layout_pass_measurement_from_regions(context, node_regions, fallback_anchors)
}

pub(crate) fn consume_layout_pass_measurements(
    store: &mut NodeGraphStore,
    semantic_registry: &NodeRegistry,
    regions: impl IntoIterator<Item = OpenGpuiMeasuredRegion>,
    node_inputs: impl IntoIterator<Item = LayoutPassMeasurementNodeInput>,
    next_revision: &mut u64,
) -> LayoutPassMeasurementConsumeResult {
    let regions = regions.into_iter().collect::<Vec<_>>();
    if regions.is_empty() {
        return LayoutPassMeasurementConsumeResult {
            outcome: LayoutPassMeasurementConsume::NoRegions,
            coverage: BTreeMap::new(),
        };
    }

    let mut changed = false;
    let mut coverage = BTreeMap::new();

    for input in node_inputs {
        let Some(node) = store.graph().nodes().get(&input.node_id).cloned() else {
            continue;
        };
        let node_regions = regions
            .iter()
            .filter(|region| region.node == Some(input.node_id))
            .cloned()
            .collect::<Vec<_>>();
        if node_regions.is_empty() {
            continue;
        }
        let Some(descriptor) = semantic_registry.view_descriptor(&node.kind) else {
            continue;
        };
        let (mut measurement, node_coverage) = layout_pass_measurement_for_node(
            input.node_id,
            &node,
            store.graph(),
            &descriptor,
            input.node_size,
            input.node_view_origin,
            input.view_to_document_scale,
            node_regions,
        );
        let existing = store.node_measurement(input.node_id);
        assign_layout_pass_measurement_revision(
            store.node_measurement_status(input.node_id),
            existing.as_ref(),
            &mut measurement,
            next_revision,
        );
        if let Ok(outcome) = store.report_node_measurement(measurement) {
            coverage.insert(input.node_id, node_coverage);
            changed |= outcome.changed();
        }
    }

    LayoutPassMeasurementConsumeResult {
        outcome: if changed {
            LayoutPassMeasurementConsume::Changed
        } else {
            LayoutPassMeasurementConsume::Unchanged
        },
        coverage,
    }
}

pub(crate) fn consume_layout_pass_measurements_from_document(
    store: &mut NodeGraphStore,
    semantic_registry: &NodeRegistry,
    regions: impl IntoIterator<Item = OpenGpuiMeasuredRegion>,
    document: &CanvasDocument,
    viewport: &CanvasViewport,
    next_revision: &mut u64,
) -> LayoutPassMeasurementConsumeResult {
    let node_inputs = layout_pass_measurement_node_inputs_from_document(store, document, viewport);
    consume_layout_pass_measurements(
        store,
        semantic_registry,
        regions,
        node_inputs,
        next_revision,
    )
}

pub(crate) fn layout_pass_measurement_node_inputs_from_document(
    store: &NodeGraphStore,
    document: &CanvasDocument,
    viewport: &CanvasViewport,
) -> Vec<LayoutPassMeasurementNodeInput> {
    store
        .graph()
        .nodes()
        .iter()
        .filter_map(|(node_id, node)| {
            let canvas_node = document.node(&canvas_node_id(node_id))?;
            let node_size = node.size.unwrap_or(JellySize {
                width: canvas_node.size.width.as_f32(),
                height: canvas_node.size.height.as_f32(),
            });
            let node_view_bounds = viewport.document_bounds_to_view(canvas_node.bounds());
            Some(LayoutPassMeasurementNodeInput {
                node_id: *node_id,
                node_size,
                node_view_origin: OpenGpuiViewPoint::new(
                    node_view_bounds.origin.x.as_f32(),
                    node_view_bounds.origin.y.as_f32(),
                ),
                view_to_document_scale: 1.0 / viewport.zoom.max(f32::EPSILON),
            })
        })
        .collect()
}

fn canvas_node_id(id: &JellyNodeId) -> NodeId {
    NodeId::from(id.0.to_string())
}

pub(crate) fn measurement_store_with_explicit_projection_fallback(
    store: &NodeGraphStore,
    semantic_registry: &NodeRegistry,
) -> ProjectionFallbackStore {
    let mut measured_store = NodeGraphStore::new(
        store.graph().clone(),
        store.view_state().clone(),
        NodeGraphEditorConfig::default(),
    );
    let mut evidence = OpenGpuiProjectionFallbackStoreEvidence::default();

    let existing_measurements = store
        .graph()
        .nodes()
        .keys()
        .filter_map(|id| match store.node_measurement_status(*id) {
            NodeMeasurementStatus::Fresh { .. } => store.node_measurement(*id),
            NodeMeasurementStatus::Missing | NodeMeasurementStatus::Dirty { .. } => None,
        })
        .collect::<Vec<_>>();

    for measurement in existing_measurements {
        evidence.record_fresh_live_measurement(measurement.node);
        measured_store
            .report_node_measurement(measurement)
            .expect("live GPUI measurement should match the graph");
    }

    let measurements = measured_store
        .graph()
        .nodes()
        .iter()
        .filter_map(|(id, node)| {
            if measured_store.node_measurement(*id).is_some() {
                return None;
            }
            let descriptor = semantic_registry.view_descriptor(&node.kind)?;
            Some(project_node_measurement(
                id,
                node,
                measured_store.graph(),
                &descriptor,
            ))
        })
        .collect::<Vec<_>>();

    for measurement in measurements {
        evidence.record_projection_fallback_measurement(measurement.node);
        measured_store
            .report_node_measurement(measurement)
            .expect("projected GPUI measurement should match the graph");
    }

    ProjectionFallbackStore {
        store: measured_store,
        evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jellyflow::{
        core::{CanvasPoint as JellyPoint, CanvasRect as JellyRect, PortDirection, PortKey},
        runtime::runtime::{
            connection::ConnectionHandleRef,
            geometry::HandlePosition,
            measurement::{
                MeasuredSurfaceAnchor, NodeHandleMeasurementSource, NodeInternalsInvalidation,
                NodeInternalsInvalidationReason,
            },
        },
        runtime::schema::NodeKitRegistry,
    };
    use jellyflow_open_gpui::{
        OpenGpuiMeasurementId, OpenGpuiProjectionMeasurementSource, OpenGpuiViewBounds,
        OpenGpuiViewSize,
    };

    fn demo_registry() -> NodeRegistry {
        NodeKitRegistry::builtin().node_registry()
    }

    fn report_fresh_transform_measurement(store: &mut NodeGraphStore) -> JellyNodeId {
        let transform = JellyNodeId::from_u128(3);
        let prompt = jellyflow::core::PortId::from_u128(30);
        store
            .report_node_measurement(
                NodeMeasurement::new(transform)
                    .with_revision(7)
                    .with_size(Some(JellySize {
                        width: 268.0,
                        height: 228.0,
                    }))
                    .with_anchors([MeasuredSurfaceAnchor::new(
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
                    .with_port_key(PortKey::new("prompt"))]),
            )
            .expect("fresh measurement should report");
        transform
    }

    #[test]
    fn explicit_projection_fallback_preserves_fresh_live_measurements() {
        let mut store = crate::make_demo_store();
        let transform = report_fresh_transform_measurement(&mut store);
        let evidence_store =
            measurement_store_with_explicit_projection_fallback(&store, &demo_registry());

        assert_eq!(evidence_store.evidence().summary.fresh_live_measurements, 1);
        assert_eq!(
            evidence_store.evidence().node_measurement_source(transform),
            OpenGpuiProjectionMeasurementSource::FreshLayoutPass
        );
        assert!(
            !evidence_store
                .evidence()
                .node_uses_projection_fallback(transform)
        );

        let prompt = jellyflow::core::PortId::from_u128(30);
        let resolution =
            evidence_store
                .store()
                .resolve_node_handle_measurement(ConnectionHandleRef::new(
                    transform,
                    prompt,
                    PortDirection::In,
                ));
        assert!(matches!(
            resolution.source,
            NodeHandleMeasurementSource::MeasuredHandle
                | NodeHandleMeasurementSource::MeasuredAnchor { .. }
        ));
        assert_eq!(
            resolution
                .bounds
                .expect("fresh prompt handle")
                .rect
                .origin
                .y,
            42.0
        );
    }

    #[test]
    fn explicit_projection_fallback_demotes_dirty_live_measurements() {
        let mut store = crate::make_demo_store();
        let transform = report_fresh_transform_measurement(&mut store);
        assert_eq!(
            store.invalidate_node_internals(NodeInternalsInvalidation::one(
                transform,
                NodeInternalsInvalidationReason::DataChanged,
            )),
            jellyflow::runtime::runtime::measurement::NodeMeasurementOutcome::Changed
        );

        let evidence_store =
            measurement_store_with_explicit_projection_fallback(&store, &demo_registry());

        assert_eq!(evidence_store.evidence().summary.fresh_live_measurements, 0);
        assert_eq!(
            evidence_store.evidence().node_measurement_source(transform),
            OpenGpuiProjectionMeasurementSource::ProjectionFallback
        );
        assert!(
            evidence_store
                .evidence()
                .node_uses_projection_fallback(transform)
        );
    }

    #[test]
    fn layout_pass_bridge_reports_partial_coverage_when_anchors_fall_back() {
        let store = crate::make_demo_store();
        let registry = demo_registry();
        let graph = store.graph();
        let transform = JellyNodeId::from_u128(3);
        let node = graph.nodes().get(&transform).expect("transform node");
        let descriptor = registry
            .view_descriptor(&node.kind)
            .expect("transform descriptor");
        let fallback_layout = projected_node_surface_graph_layout(
            &descriptor,
            node,
            graph,
            &transform,
            node.size.expect("node size"),
        );
        let slot = fallback_layout.slots.first().expect("projected slot");
        let (measurement, coverage) = layout_pass_measurement_for_node(
            transform,
            node,
            graph,
            &descriptor,
            node.size.expect("node size"),
            OpenGpuiViewPoint::new(node.pos.x, node.pos.y),
            1.0,
            [
                OpenGpuiMeasurementId::slot(transform, slot.slot.key.as_str()).into_region(
                    OpenGpuiViewBounds::new(
                        OpenGpuiViewPoint::new(node.pos.x + 12.0, node.pos.y + 20.0),
                        OpenGpuiViewSize::new(96.0, 24.0),
                    ),
                ),
            ],
        );

        assert_eq!(measurement.slots.len(), 1);
        assert_eq!(coverage.layout_pass_regions, 1);
        assert!(coverage.projection_fallback_regions > 0);
        assert!(!coverage.is_full_layout_pass());
    }

    #[test]
    fn layout_pass_consume_reports_changed_then_unchanged_with_cached_revision() {
        let mut store = crate::make_demo_store();
        let registry = demo_registry();
        let transform = JellyNodeId::from_u128(3);
        let node = store
            .graph()
            .nodes()
            .get(&transform)
            .expect("transform node");
        let node_pos = node.pos;
        let node_size = node.size.expect("node size");
        let slot_key = "prompt";
        let regions = || {
            vec![
                OpenGpuiMeasurementId::slot(transform, slot_key).into_region(
                    OpenGpuiViewBounds::new(
                        OpenGpuiViewPoint::new(node_pos.x + 12.0, node_pos.y + 20.0),
                        OpenGpuiViewSize::new(96.0, 24.0),
                    ),
                ),
            ]
        };
        let inputs = || {
            vec![LayoutPassMeasurementNodeInput {
                node_id: transform,
                node_size,
                node_view_origin: OpenGpuiViewPoint::new(node_pos.x, node_pos.y),
                view_to_document_scale: 1.0,
            }]
        };
        let mut next_revision = 0;

        let first = consume_layout_pass_measurements(
            &mut store,
            &registry,
            regions(),
            inputs(),
            &mut next_revision,
        );
        assert_eq!(first.outcome, LayoutPassMeasurementConsume::Changed);
        assert_eq!(first.coverage[&transform].measured_slots, 1);
        let first_revision = store
            .node_measurement(transform)
            .expect("first measurement")
            .revision;

        let second = consume_layout_pass_measurements(
            &mut store,
            &registry,
            regions(),
            inputs(),
            &mut next_revision,
        );
        assert_eq!(second.outcome, LayoutPassMeasurementConsume::Unchanged);
        assert_eq!(second.coverage[&transform].measured_slots, 1);
        assert_eq!(
            store
                .node_measurement(transform)
                .expect("second measurement")
                .revision,
            first_revision
        );
    }

    #[test]
    fn layout_pass_node_inputs_follow_canvas_viewport() {
        let store = crate::make_demo_store();
        let (document, _) = crate::project_store(&store).expect("demo graph projects");
        let viewport = crate::initial_viewport_for_document(&document);
        let inputs =
            layout_pass_measurement_node_inputs_from_document(&store, &document, &viewport);
        let transform = JellyNodeId::from_u128(3);
        let input = inputs
            .iter()
            .find(|input| input.node_id == transform)
            .expect("transform node input should be present");
        let canvas_node = document
            .node(&canvas_node_id(&transform))
            .expect("transform canvas node");
        let view_bounds = viewport.document_bounds_to_view(canvas_node.bounds());

        assert_eq!(
            input.node_size,
            store.graph().nodes()[&transform].size.unwrap()
        );
        assert_eq!(
            input.node_view_origin,
            OpenGpuiViewPoint::new(view_bounds.origin.x.as_f32(), view_bounds.origin.y.as_f32())
        );
        assert_eq!(
            input.view_to_document_scale,
            1.0 / viewport.zoom.max(f32::EPSILON)
        );
    }
}
