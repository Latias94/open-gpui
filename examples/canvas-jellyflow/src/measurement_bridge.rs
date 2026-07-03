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
    OpenGpuiViewPoint, layout_pass_measurement_from_regions, measured_surface_anchors,
    project_node_measurement, projected_node_surface_graph_layout,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ProjectionFallbackStoreSummary {
    pub(crate) fresh_live_measurements: usize,
    pub(crate) projection_fallback_measurements: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectionFallbackMeasurementSource {
    FreshLayoutPass,
    ProjectionFallback,
    Missing,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProjectionFallbackStoreEvidence {
    pub(crate) summary: ProjectionFallbackStoreSummary,
    pub(crate) fresh_nodes: Vec<JellyNodeId>,
    pub(crate) fallback_nodes: Vec<JellyNodeId>,
}

impl ProjectionFallbackStoreEvidence {
    pub(crate) fn node_uses_projection_fallback(&self, node_id: JellyNodeId) -> bool {
        self.fallback_nodes.contains(&node_id)
    }

    pub(crate) fn node_measurement_source(
        &self,
        node_id: JellyNodeId,
    ) -> ProjectionFallbackMeasurementSource {
        if self.fresh_nodes.contains(&node_id) {
            ProjectionFallbackMeasurementSource::FreshLayoutPass
        } else if self.fallback_nodes.contains(&node_id) {
            ProjectionFallbackMeasurementSource::ProjectionFallback
        } else {
            ProjectionFallbackMeasurementSource::Missing
        }
    }
}

pub(crate) struct ProjectionFallbackStore {
    store: NodeGraphStore,
    evidence: ProjectionFallbackStoreEvidence,
}

impl ProjectionFallbackStore {
    pub(crate) fn store(&self) -> &NodeGraphStore {
        &self.store
    }

    pub(crate) fn evidence(&self) -> &ProjectionFallbackStoreEvidence {
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

pub(crate) fn measurement_store_with_explicit_projection_fallback(
    store: &NodeGraphStore,
    semantic_registry: &NodeRegistry,
) -> ProjectionFallbackStore {
    let mut measured_store = NodeGraphStore::new(
        store.graph().clone(),
        store.view_state().clone(),
        NodeGraphEditorConfig::default(),
    );
    let mut evidence = ProjectionFallbackStoreEvidence::default();

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
        evidence.fresh_nodes.push(measurement.node);
        measured_store
            .report_node_measurement(measurement)
            .expect("live GPUI measurement should match the graph");
        evidence.summary.fresh_live_measurements += 1;
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

    evidence.summary.projection_fallback_measurements = measurements.len();
    for measurement in measurements {
        evidence.fallback_nodes.push(measurement.node);
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
    use jellyflow_open_gpui::{OpenGpuiMeasurementId, OpenGpuiViewBounds, OpenGpuiViewSize};

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
            ProjectionFallbackMeasurementSource::FreshLayoutPass
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
            ProjectionFallbackMeasurementSource::ProjectionFallback
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
}
