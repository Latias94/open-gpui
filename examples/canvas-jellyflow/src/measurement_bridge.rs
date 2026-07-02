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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProjectionFallbackStoreEvidence {
    pub(crate) summary: ProjectionFallbackStoreSummary,
    pub(crate) fallback_nodes: Vec<JellyNodeId>,
}

impl ProjectionFallbackStoreEvidence {
    pub(crate) fn node_uses_projection_fallback(&self, node_id: JellyNodeId) -> bool {
        self.fallback_nodes.contains(&node_id)
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
