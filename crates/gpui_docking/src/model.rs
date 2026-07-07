//! Explicit low-level docking model APIs.
//!
//! Normal applications should prefer [`crate::DockSurface`]. This module is for tools and tests
//! that need direct graph, layout, workspace, or action access.

pub use crate::{
    DockLayoutRect, action::DockAction, action::DockActionApplyError, action::DockActionOutcome,
    action::DockSplitResize, builder::DockLayoutBuilder, builder::EditorDockLayoutSpec,
    graph::DockCentralRegion, graph::DockEdgeDockPlan, graph::DockEdgeDockSizing,
    graph::DockEdgeDockSizingScope, graph::DockFloatingContainer, graph::DockGraph,
    graph::DockGraphValidationError, graph::DockNode, graph::DropZone, graph::SplitAxis,
    graph::dock_bounds, ids::DockNodeId, layout::DockLayoutCentralRegion,
    layout::DockLayoutFloatingContainer, layout::DockLayoutNode, layout::DockLayoutSpace,
    op::DockGraphMutationError, spatial_navigation::DockSpatialDirection, workspace::DockWorkspace,
};
