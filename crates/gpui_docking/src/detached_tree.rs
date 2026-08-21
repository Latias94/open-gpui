#![allow(dead_code)]

use crate::graph_assembly::{CanonicalDockGraph, CanonicalGraphError, CanonicalSizeRef};
use crate::payload_ref::{DockPayloadRef, DockPayloadRefError};
use crate::{
    CanonicalNodeId, DockFloatingId, DockGroupId, DockItemId, DockLayoutRect, DockSpaceId,
    SplitAxis,
};
use thiserror::Error;

/// A linear proof that a payload may be detached from one graph version.
///
/// The value is intentionally not cloneable. U3 consumes it while extracting the sole detached
/// owner inside the infallible commit interval.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PreparedDetach {
    pub(crate) payload: DockPayloadRef,
    pub(crate) source_space: DockSpaceId,
    pub(crate) source_node: CanonicalNodeId,
    pub(crate) expected_central_node: Option<CanonicalNodeId>,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum PreparedDetachError {
    #[error(transparent)]
    Payload(#[from] DockPayloadRefError),
    #[error(transparent)]
    Graph(#[from] CanonicalGraphError),
    #[error("payload node {node:?} is not present in source space {space}")]
    MissingSourceNode {
        node: CanonicalNodeId,
        space: DockSpaceId,
    },
}

impl PreparedDetach {
    pub(crate) fn prepare(
        graph: &CanonicalDockGraph,
        payload: DockPayloadRef,
    ) -> Result<Self, PreparedDetachError> {
        graph.validate_payload_ref(&payload)?;
        let space = graph.space(&payload.source_space).ok_or_else(|| {
            PreparedDetachError::MissingSourceNode {
                node: payload.source_node,
                space: payload.source_space.clone(),
            }
        })?;
        if graph.index().space_for_node(payload.source_node) != Some(&payload.source_space) {
            return Err(PreparedDetachError::MissingSourceNode {
                node: payload.source_node,
                space: payload.source_space.clone(),
            });
        }
        Ok(Self {
            source_space: payload.source_space.clone(),
            source_node: payload.source_node,
            expected_central_node: space.central.node,
            payload,
        })
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum DetachedDockTree {
    Item {
        item: DockItemId,
    },
    Group {
        id: DockGroupId,
        items: Vec<DockItemId>,
        selected: DockItemId,
        mru: Vec<DockItemId>,
    },
    Subtree {
        root: DetachedNode,
    },
    FloatingRoot {
        id: DockFloatingId,
        root: DetachedNode,
        bounds: DockLayoutRect,
        z_order: i32,
    },
}

#[derive(Debug, PartialEq)]
pub(crate) enum DetachedNode {
    Branch {
        axis: SplitAxis,
        first: Box<DetachedNode>,
        second: Box<DetachedNode>,
        ratio: f32,
        size_ref: Option<CanonicalSizeRef>,
    },
    Group {
        id: DockGroupId,
        items: Vec<DockItemId>,
        selected: DockItemId,
        mru: Vec<DockItemId>,
        size_ref: Option<CanonicalSizeRef>,
    },
}
