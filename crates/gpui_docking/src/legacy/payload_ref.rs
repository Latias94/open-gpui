#![allow(dead_code)]

use crate::legacy::graph_assembly::{CanonicalDockGraph, CanonicalGraphError};
use crate::legacy::{
    CanonicalGraphVersion, CanonicalNodeId, DockFloatingId, DockGroupId, DockItemId, DockSpaceId,
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockPayloadKind {
    Item,
    WholeGroup,
    RetainedSubtree,
    WholeFloating,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockPayloadRef {
    pub(crate) graph_version: CanonicalGraphVersion,
    pub(crate) source_space: DockSpaceId,
    pub(crate) anchor_item: DockItemId,
    pub(crate) kind: DockPayloadKind,
    pub(crate) source_node: CanonicalNodeId,
    pub(crate) source_group: Option<DockGroupId>,
    pub(crate) source_floating: Option<DockFloatingId>,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum DockPayloadRefError {
    #[error("payload item {item} is not present in source space {space}")]
    MissingItem {
        space: DockSpaceId,
        item: DockItemId,
    },
    #[error("payload floating root for item {item} is not present in source space {space}")]
    MissingFloating {
        space: DockSpaceId,
        item: DockItemId,
    },
    #[error("payload graph version {expected:?} is stale; current version is {actual:?}")]
    StaleVersion {
        expected: CanonicalGraphVersion,
        actual: CanonicalGraphVersion,
    },
    #[error("payload item {item} moved out of source space {space}")]
    SourceMembershipChanged {
        space: DockSpaceId,
        item: DockItemId,
    },
    #[error("payload group identity for item {item} changed")]
    GroupIdentityChanged { item: DockItemId },
    #[error("payload floating ownership for item {item} changed")]
    FloatingOwnershipChanged { item: DockItemId },
    #[error("payload anchor item {item} is not inside subtree {node:?}")]
    AnchorOutsideSubtree {
        node: CanonicalNodeId,
        item: DockItemId,
    },
    #[error(transparent)]
    Graph(#[from] CanonicalGraphError),
}

impl CanonicalDockGraph {
    pub(crate) fn payload_ref(
        &self,
        source_space: impl Into<DockSpaceId>,
        anchor_item: impl Into<DockItemId>,
        kind: DockPayloadKind,
    ) -> Result<DockPayloadRef, DockPayloadRefError> {
        let source_space = source_space.into();
        let anchor_item = anchor_item.into();
        let anchor_node = self
            .index()
            .node_for_item(&anchor_item)
            .filter(|node| self.index().space_for_node(*node) == Some(&source_space))
            .ok_or_else(|| DockPayloadRefError::MissingItem {
                space: source_space.clone(),
                item: anchor_item.clone(),
            })?;
        let source_group = self.index().group_for_item(&anchor_item).cloned();
        let source_floating = self.index().floating_for_item(&anchor_item).cloned();
        let source_node = if matches!(kind, DockPayloadKind::WholeFloating) {
            let floating = source_floating
                .as_ref()
                .and_then(|floating| self.floating_roots().get(floating))
                .filter(|floating| floating.space == source_space)
                .ok_or_else(|| DockPayloadRefError::MissingFloating {
                    space: source_space.clone(),
                    item: anchor_item.clone(),
                })?;
            floating.root
        } else {
            anchor_node
        };
        Ok(DockPayloadRef {
            graph_version: self.version(),
            source_space,
            anchor_item,
            kind,
            source_node,
            source_group,
            source_floating,
        })
    }

    pub(crate) fn subtree_payload_ref(
        &self,
        source_space: impl Into<DockSpaceId>,
        source_node: CanonicalNodeId,
        anchor_item: impl Into<DockItemId>,
    ) -> Result<DockPayloadRef, DockPayloadRefError> {
        let source_space = source_space.into();
        let anchor_item = anchor_item.into();
        if self.index().space_for_node(source_node) != Some(&source_space) {
            return Err(DockPayloadRefError::SourceMembershipChanged {
                space: source_space,
                item: anchor_item,
            });
        }
        if !self.subtree_contains_item(source_node, &anchor_item) {
            return Err(DockPayloadRefError::AnchorOutsideSubtree {
                node: source_node,
                item: anchor_item,
            });
        }
        Ok(DockPayloadRef {
            graph_version: self.version(),
            source_space,
            source_group: self.index().group_for_item(&anchor_item).cloned(),
            source_floating: self.index().floating_for_item(&anchor_item).cloned(),
            anchor_item,
            kind: DockPayloadKind::RetainedSubtree,
            source_node,
        })
    }

    pub(crate) fn validate_payload_ref(
        &self,
        payload: &DockPayloadRef,
    ) -> Result<(), DockPayloadRefError> {
        if self.version() != payload.graph_version {
            return Err(DockPayloadRefError::StaleVersion {
                expected: payload.graph_version,
                actual: self.version(),
            });
        }
        let current_node = self
            .index()
            .node_for_item(&payload.anchor_item)
            .filter(|node| self.index().space_for_node(*node) == Some(&payload.source_space))
            .ok_or_else(|| DockPayloadRefError::SourceMembershipChanged {
                space: payload.source_space.clone(),
                item: payload.anchor_item.clone(),
            })?;
        if self.index().group_for_item(&payload.anchor_item) != payload.source_group.as_ref() {
            return Err(DockPayloadRefError::GroupIdentityChanged {
                item: payload.anchor_item.clone(),
            });
        }
        if self.index().floating_for_item(&payload.anchor_item) != payload.source_floating.as_ref()
        {
            return Err(DockPayloadRefError::FloatingOwnershipChanged {
                item: payload.anchor_item.clone(),
            });
        }
        match payload.kind {
            DockPayloadKind::Item | DockPayloadKind::WholeGroup => {
                if current_node != payload.source_node {
                    return Err(DockPayloadRefError::SourceMembershipChanged {
                        space: payload.source_space.clone(),
                        item: payload.anchor_item.clone(),
                    });
                }
            }
            DockPayloadKind::RetainedSubtree => {
                if self.index().space_for_node(payload.source_node) != Some(&payload.source_space)
                    || !self.subtree_contains_item(payload.source_node, &payload.anchor_item)
                {
                    return Err(DockPayloadRefError::AnchorOutsideSubtree {
                        node: payload.source_node,
                        item: payload.anchor_item.clone(),
                    });
                }
            }
            DockPayloadKind::WholeFloating => {
                let root = payload
                    .source_floating
                    .as_ref()
                    .and_then(|floating| self.floating_roots().get(floating))
                    .filter(|floating| floating.space == payload.source_space)
                    .map(|floating| floating.root);
                if root != Some(payload.source_node)
                    || !self.subtree_contains_item(payload.source_node, &payload.anchor_item)
                {
                    return Err(DockPayloadRefError::FloatingOwnershipChanged {
                        item: payload.anchor_item.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn subtree_contains_item(&self, root: CanonicalNodeId, item: &DockItemId) -> bool {
        let Some(mut node) = self.index().node_for_item(item) else {
            return false;
        };
        loop {
            if node == root {
                return true;
            }
            match self.index().parent_of(node) {
                Some(Some(parent)) => node = parent,
                Some(None) | None => return false,
            }
        }
    }
}
