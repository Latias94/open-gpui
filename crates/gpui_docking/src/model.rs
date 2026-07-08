//! Explicit low-level docking model APIs.
//!
//! Normal applications should prefer [`crate::DockSurface`]. This module is for tools and tests
//! that need direct graph, layout, workspace, or action access.

pub use crate::{
    DockLayout, DockLayoutRect, action::DockAction, action::DockActionApplyError,
    action::DockActionOutcome, action::DockSplitResize, builder::DockLayoutBuilder,
    builder::EditorDockLayoutSpec, controller::DockController, controller::DockControllerBuilder,
    graph::DockCentralRegion, graph::DockEdgeDockPlan, graph::DockEdgeDockSizing,
    graph::DockEdgeDockSizingScope, graph::DockFloatingContainer, graph::DockGraph,
    graph::DockGraphValidationError, graph::DockNode, graph::DropZone, graph::SplitAxis,
    graph::dock_bounds, ids::DockNodeId, layout::DockLayoutCentralRegion,
    layout::DockLayoutFloatingContainer, layout::DockLayoutNode, layout::DockLayoutSpace,
    op::DockGraphMutationError, spatial_navigation::DockSpatialDirection, workspace::DockWorkspace,
};

/// Creates a dock layout from raw serialized spaces and nodes.
///
/// This is a model-tier constructor for tooling, tests, and migrations that intentionally operate
/// on serialized graph anatomy. Normal applications should prefer [`DockSurface`](crate::DockSurface)
/// builders, panel placements, or deserializing a saved [`DockLayout`].
pub fn layout_from_raw_parts(
    spaces: Vec<DockLayoutSpace>,
    nodes: Vec<DockLayoutNode>,
) -> DockLayout {
    DockLayout::from_raw_parts(spaces, nodes)
}

/// Decomposes a dock layout into raw serialized spaces and nodes.
///
/// This is the explicit model-tier escape hatch for code that needs to patch serialized graph
/// anatomy. Common application code should keep [`DockLayout`] opaque and use facade snapshots
/// such as [`DockSurface::export_layout`](crate::DockSurface::export_layout).
pub fn layout_into_raw_parts(layout: DockLayout) -> (Vec<DockLayoutSpace>, Vec<DockLayoutNode>) {
    layout.into_raw_parts()
}
