use crate::{
    DockActionApplyError, DockEdgeDockPlan, DockGraph, DockGraphDropTarget, DockGraphMutationError,
    DockItemId, DockNode, DockNodeId, DockOp, DockSpaceId,
    workspace_drop_transaction::DockWorkspaceDropPayload,
};
use std::collections::HashSet;

/// Why a locked payload could not be projected forward from its current graph location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockLockedPayloadForwardProjectionError {
    /// At least one item from the locked payload is no longer reachable in the graph.
    PayloadMissing,
    /// At least one item from the locked payload has more than one reachable graph location.
    PayloadAmbiguous,
    /// Rehoming the uniquely located payload would violate a graph invariant.
    GraphMutation(DockGraphMutationError),
}

impl From<DockGraphMutationError> for DockLockedPayloadForwardProjectionError {
    fn from(error: DockGraphMutationError) -> Self {
        Self::GraphMutation(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DockLockedPayloadIdentity {
    Item {
        source_space: DockSpaceId,
        source_tabs: DockNodeId,
        item: DockItemId,
    },
    Tabs {
        source_space: DockSpaceId,
        source_tabs: DockNodeId,
        ordered_items: Vec<DockItemId>,
    },
    Floating {
        source_space: DockSpaceId,
        floating: DockNodeId,
        child_root: DockNodeId,
        ordered_items: Vec<DockItemId>,
    },
}

impl DockLockedPayloadIdentity {
    pub(crate) fn capture(
        graph: &DockGraph,
        source_space: &DockSpaceId,
        payload: DockWorkspaceDropPayload<'_>,
    ) -> Result<Self, DockActionApplyError> {
        let identity = match payload {
            DockWorkspaceDropPayload::Item { source_tabs, item } => Self::Item {
                source_space: source_space.clone(),
                source_tabs,
                item: item.clone(),
            },
            DockWorkspaceDropPayload::Tabs { source_tabs } => {
                let Some(DockNode::Tabs { items, .. }) = graph.node(source_tabs) else {
                    return Err(payload_mismatch(source_space, source_tabs));
                };
                if items.is_empty() {
                    return Err(payload_mismatch(source_space, source_tabs));
                }
                Self::Tabs {
                    source_space: source_space.clone(),
                    source_tabs,
                    ordered_items: items.clone(),
                }
            }
            DockWorkspaceDropPayload::Floating { floating } => {
                if graph
                    .floating_containers(source_space)
                    .iter()
                    .all(|container| container.node != floating)
                {
                    return Err(DockGraphMutationError::FloatingContainerNotFound {
                        space: source_space.clone(),
                        floating,
                    }
                    .into());
                }
                let Some(DockNode::Floating { child }) = graph.node(floating) else {
                    return Err(payload_mismatch(source_space, floating));
                };
                let ordered_items = graph.collect_items_in_subtree(*child);
                if ordered_items.is_empty() {
                    return Err(payload_mismatch(source_space, floating));
                }
                Self::Floating {
                    source_space: source_space.clone(),
                    floating,
                    child_root: *child,
                    ordered_items,
                }
            }
        };
        identity.validate(graph)?;
        Ok(identity)
    }

    pub(crate) fn validate(&self, graph: &DockGraph) -> Result<(), DockActionApplyError> {
        match self {
            Self::Item {
                source_space,
                source_tabs,
                item,
            } => {
                if graph
                    .find_item_in_space(source_space, item)
                    .is_some_and(|(tabs, _)| tabs == *source_tabs)
                {
                    Ok(())
                } else {
                    Err(DockActionApplyError::ItemNotInTabs {
                        tabs: *source_tabs,
                        item: item.clone(),
                    })
                }
            }
            Self::Tabs {
                source_space,
                source_tabs,
                ordered_items,
            } => {
                let Some(DockNode::Tabs { items, .. }) = graph.node(*source_tabs) else {
                    return Err(payload_mismatch(source_space, *source_tabs));
                };
                if graph
                    .root_for_node_in_space(source_space, *source_tabs)
                    .is_none()
                    || items != ordered_items
                {
                    return Err(payload_mismatch(source_space, *source_tabs));
                }
                Ok(())
            }
            Self::Floating {
                source_space,
                floating,
                child_root,
                ordered_items,
            } => {
                if graph
                    .floating_containers(source_space)
                    .iter()
                    .all(|container| container.node != *floating)
                {
                    return Err(DockGraphMutationError::FloatingContainerNotFound {
                        space: source_space.clone(),
                        floating: *floating,
                    }
                    .into());
                }
                let Some(DockNode::Floating { child }) = graph.node(*floating) else {
                    return Err(payload_mismatch(source_space, *floating));
                };
                if child != child_root
                    || graph.collect_items_in_subtree(*child_root) != *ordered_items
                {
                    return Err(payload_mismatch(source_space, *floating));
                }
                Ok(())
            }
        }
    }

    pub(crate) fn source_space(&self) -> &DockSpaceId {
        match self {
            Self::Item { source_space, .. }
            | Self::Tabs { source_space, .. }
            | Self::Floating { source_space, .. } => source_space,
        }
    }

    pub(crate) const fn source_node(&self) -> DockNodeId {
        match self {
            Self::Item { source_tabs, .. } | Self::Tabs { source_tabs, .. } => *source_tabs,
            Self::Floating { floating, .. } => *floating,
        }
    }

    pub(crate) fn as_workspace_payload(&self) -> DockWorkspaceDropPayload<'_> {
        match self {
            Self::Item {
                source_tabs, item, ..
            } => DockWorkspaceDropPayload::Item {
                source_tabs: *source_tabs,
                item,
            },
            Self::Tabs { source_tabs, .. } => DockWorkspaceDropPayload::Tabs {
                source_tabs: *source_tabs,
            },
            Self::Floating { floating, .. } => DockWorkspaceDropPayload::Floating {
                floating: *floating,
            },
        }
    }

    pub(crate) fn graph_op(
        &self,
        target_space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> DockOp {
        match self {
            Self::Item {
                source_space, item, ..
            } => DockOp::MoveItem {
                source_space: source_space.clone(),
                item: item.clone(),
                target_space: target_space.clone(),
                target,
            },
            Self::Tabs {
                source_space,
                source_tabs,
                ..
            } => DockOp::MoveTabs {
                source_space: source_space.clone(),
                source_tabs: *source_tabs,
                target_space: target_space.clone(),
                target,
            },
            Self::Floating {
                source_space,
                floating,
                ..
            } => DockOp::MoveFloating {
                source_space: source_space.clone(),
                floating: *floating,
                target_space: target_space.clone(),
                target,
            },
        }
    }

    /// Projects this payload from its unique current locations into one empty target space.
    ///
    /// Unlike the exact locked operation, this projection intentionally ignores the captured
    /// source node identity. It is reserved for forward-only settlement after another synchronous
    /// graph mutation moved the payload while the original promotion was in flight.
    pub(crate) fn project_forward_rebased_to_empty_space(
        &self,
        graph: &DockGraph,
        target_space: &DockSpaceId,
    ) -> Result<(DockGraph, bool), DockLockedPayloadForwardProjectionError> {
        let items = self.ordered_items();
        if items.is_empty() {
            return Err(DockLockedPayloadForwardProjectionError::PayloadMissing);
        }

        let mut distinct_items = HashSet::with_capacity(items.len());
        for item in items {
            if !distinct_items.insert(item) {
                return Err(DockLockedPayloadForwardProjectionError::PayloadAmbiguous);
            }
            unique_item_location(graph, item)?;
        }

        if graph.root(target_space).is_some() || !graph.floating_containers(target_space).is_empty()
        {
            return Err(DockGraphMutationError::TargetSpaceNotEmpty {
                space: target_space.clone(),
            }
            .into());
        }

        let mut projected = graph.clone();
        let mut target_tabs = None;
        let mut changed = false;
        for item in items {
            let (source_space, _) = unique_item_location(&projected, item)?;
            let target = match target_tabs {
                Some(tabs) => {
                    let Some(DockNode::Tabs { items, .. }) = projected.node(tabs) else {
                        return Err(DockGraphMutationError::NodeIsNotTabs { node: tabs }.into());
                    };
                    DockGraphDropTarget::tab_bar(tabs, items.len())
                }
                None => DockGraphDropTarget::empty_space(),
            };
            changed |= projected.apply_op_checked(&DockOp::MoveItem {
                source_space,
                item: item.clone(),
                target_space: target_space.clone(),
                target,
            })?;

            if target_tabs.is_none() {
                let (projected_space, tabs) = unique_item_location(&projected, item)?;
                if projected_space != *target_space {
                    return Err(DockLockedPayloadForwardProjectionError::PayloadMissing);
                }
                target_tabs = Some(tabs);
            }
        }

        Ok((projected, changed))
    }

    fn ordered_items(&self) -> &[DockItemId] {
        match self {
            Self::Item { item, .. } => std::slice::from_ref(item),
            Self::Tabs { ordered_items, .. } | Self::Floating { ordered_items, .. } => {
                ordered_items
            }
        }
    }
}

fn unique_item_location(
    graph: &DockGraph,
    item: &DockItemId,
) -> Result<(DockSpaceId, DockNodeId), DockLockedPayloadForwardProjectionError> {
    let mut location = None;
    for space in graph.spaces() {
        for tabs in graph.tabs_in_space(&space) {
            let Some(DockNode::Tabs { items, .. }) = graph.node(tabs) else {
                continue;
            };
            for candidate in items {
                if candidate != item {
                    continue;
                }
                if location.is_some() {
                    return Err(DockLockedPayloadForwardProjectionError::PayloadAmbiguous);
                }
                location = Some((space.clone(), tabs));
            }
        }
    }
    location.ok_or(DockLockedPayloadForwardProjectionError::PayloadMissing)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockLockedTargetIdentity {
    TabBar {
        target_space: DockSpaceId,
        tabs: DockNodeId,
        insert_index: usize,
        ordered_items: Vec<DockItemId>,
    },
    LeafCenter {
        target_space: DockSpaceId,
        root: DockNodeId,
        tabs: DockNodeId,
    },
    FloatingTitleBar {
        target_space: DockSpaceId,
        floating: DockNodeId,
        tabs: DockNodeId,
    },
    Edge {
        target_space: DockSpaceId,
        plan: DockEdgeDockPlan,
    },
    Empty {
        target_space: DockSpaceId,
    },
}

impl DockLockedTargetIdentity {
    pub(crate) fn tab_bar(
        graph: &DockGraph,
        target_space: DockSpaceId,
        tabs: DockNodeId,
        insert_index: usize,
    ) -> Result<Self, DockActionApplyError> {
        let Some(DockNode::Tabs { items, .. }) = graph.node(tabs) else {
            return Err(DockActionApplyError::DropTargetUnavailable);
        };
        let identity = Self::TabBar {
            target_space,
            tabs,
            insert_index,
            ordered_items: items.clone(),
        };
        identity.validate(graph)?;
        Ok(identity)
    }

    pub(crate) fn leaf_center(
        graph: &DockGraph,
        target_space: DockSpaceId,
        root: DockNodeId,
        tabs: DockNodeId,
    ) -> Result<Self, DockActionApplyError> {
        let identity = Self::LeafCenter {
            target_space,
            root,
            tabs,
        };
        identity.validate(graph)?;
        Ok(identity)
    }

    pub(crate) fn floating_title_bar(
        graph: &DockGraph,
        target_space: DockSpaceId,
        floating: DockNodeId,
        tabs: DockNodeId,
    ) -> Result<Self, DockActionApplyError> {
        let identity = Self::FloatingTitleBar {
            target_space,
            floating,
            tabs,
        };
        identity.validate(graph)?;
        Ok(identity)
    }

    pub(crate) fn edge(
        graph: &DockGraph,
        target_space: DockSpaceId,
        plan: DockEdgeDockPlan,
    ) -> Result<Self, DockActionApplyError> {
        let identity = Self::Edge { target_space, plan };
        identity.validate(graph)?;
        Ok(identity)
    }

    pub(crate) fn empty(target_space: DockSpaceId) -> Self {
        Self::Empty { target_space }
    }

    pub(crate) fn validate(&self, graph: &DockGraph) -> Result<(), DockActionApplyError> {
        let valid = match self {
            Self::TabBar {
                target_space,
                tabs,
                ordered_items,
                ..
            } => {
                graph.root_for_node_in_space(target_space, *tabs).is_some()
                    && matches!(
                        graph.node(*tabs),
                        Some(DockNode::Tabs { items, .. }) if items == ordered_items
                    )
            }
            Self::LeafCenter {
                target_space,
                root,
                tabs,
            } => {
                graph.root_for_node_in_space(target_space, *root) == Some(*root)
                    && graph.root_for_node_in_space(target_space, *tabs) == Some(*root)
            }
            Self::FloatingTitleBar {
                target_space,
                floating,
                tabs,
            } => {
                matches!(graph.node(*floating), Some(DockNode::Floating { .. }))
                    && graph
                        .floating_containers(target_space)
                        .iter()
                        .any(|container| container.node == *floating)
                    && graph.root_for_node_in_space(target_space, *tabs) == Some(*floating)
            }
            Self::Edge { target_space, plan } => {
                graph.edge_dock_plan_is_current(target_space, *plan)
            }
            Self::Empty { .. } => true,
        };
        if valid {
            Ok(())
        } else {
            Err(DockActionApplyError::DropTargetUnavailable)
        }
    }

    pub(crate) fn target_space(&self) -> &DockSpaceId {
        match self {
            Self::TabBar { target_space, .. }
            | Self::LeafCenter { target_space, .. }
            | Self::FloatingTitleBar { target_space, .. }
            | Self::Edge { target_space, .. }
            | Self::Empty { target_space } => target_space,
        }
    }

    pub(crate) fn graph_target(&self) -> DockGraphDropTarget {
        match self {
            Self::TabBar {
                tabs, insert_index, ..
            } => DockGraphDropTarget::tab_bar(*tabs, *insert_index),
            Self::LeafCenter { tabs, .. } | Self::FloatingTitleBar { tabs, .. } => {
                DockGraphDropTarget::center(*tabs)
            }
            Self::Edge { plan, .. } => DockGraphDropTarget::edge(*plan),
            Self::Empty { .. } => DockGraphDropTarget::empty_space(),
        }
    }
}

fn payload_mismatch(source_space: &DockSpaceId, source_node: DockNodeId) -> DockActionApplyError {
    DockActionApplyError::DropPayloadMismatch {
        space: source_space.clone(),
        tabs: source_node,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DockFloatingContainer, SplitAxis};
    use open_gpui::{Bounds, point, px, size};

    fn space() -> DockSpaceId {
        DockSpaceId::from("main")
    }

    fn item(id: &str) -> DockItemId {
        DockItemId::from(id)
    }

    fn bounds(origin: f32) -> Bounds<open_gpui::Pixels> {
        Bounds::new(point(px(origin), px(origin)), size(px(100.0), px(100.0)))
    }

    #[test]
    fn tabs_payload_allows_selection_change_but_rejects_item_change() {
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("b")],
            selected: Some(item("a")),
        });
        graph.set_root(space(), tabs);
        let identity = DockLockedPayloadIdentity::capture(
            &graph,
            &space(),
            DockWorkspaceDropPayload::Tabs { source_tabs: tabs },
        )
        .expect("the original tabs payload should lock");

        graph
            .apply_op_checked(&DockOp::SelectTab {
                tabs,
                item: item("b"),
            })
            .expect("selection change should commit");
        identity
            .validate(&graph)
            .expect("selection alone must not change payload identity");

        graph
            .apply_op_checked(&DockOp::OpenItem {
                space: space(),
                target_tabs: Some(tabs),
                item: item("x"),
                insert_index: None,
            })
            .expect("item change should commit");
        assert_eq!(
            identity.validate(&graph),
            Err(DockActionApplyError::DropPayloadMismatch {
                space: space(),
                tabs,
            })
        );
    }

    #[test]
    fn floating_payload_allows_bounds_change_but_rejects_item_change() {
        let mut graph = DockGraph::new();
        let root_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("root")],
            selected: Some(item("root")),
        });
        let floating_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let floating = graph.insert_node(DockNode::Floating {
            child: floating_tabs,
        });
        graph.set_root(space(), root_tabs);
        graph
            .floating_containers_mut(space())
            .push(DockFloatingContainer {
                node: floating,
                bounds: bounds(0.0),
            });
        let identity = DockLockedPayloadIdentity::capture(
            &graph,
            &space(),
            DockWorkspaceDropPayload::Floating { floating },
        )
        .expect("the original floating payload should lock");

        graph.floating_containers_mut(space())[0].bounds = bounds(20.0);
        identity
            .validate(&graph)
            .expect("bounds alone must not change floating payload identity");

        graph
            .apply_op_checked(&DockOp::OpenItem {
                space: space(),
                target_tabs: Some(floating_tabs),
                item: item("x"),
                insert_index: None,
            })
            .expect("floating item change should commit");
        assert_eq!(
            identity.validate(&graph),
            Err(DockActionApplyError::DropPayloadMismatch {
                space: space(),
                tabs: floating,
            })
        );
    }

    #[test]
    fn forward_projection_reports_a_missing_payload_without_guessing() {
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        graph.set_root(space(), source_tabs);
        let identity = DockLockedPayloadIdentity::Item {
            source_space: space(),
            source_tabs,
            item: item("a"),
        };

        assert!(matches!(
            identity.project_forward_rebased_to_empty_space(&graph, &DockSpaceId::from("detached")),
            Err(DockLockedPayloadForwardProjectionError::PayloadMissing)
        ));
    }

    #[test]
    fn forward_projection_reports_ambiguous_payload_locations_without_guessing() {
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let duplicate_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        graph.set_root(space(), source_tabs);
        graph.set_root(DockSpaceId::from("duplicate"), duplicate_tabs);
        let identity = DockLockedPayloadIdentity::Item {
            source_space: space(),
            source_tabs,
            item: item("a"),
        };

        assert!(matches!(
            identity.project_forward_rebased_to_empty_space(&graph, &DockSpaceId::from("detached")),
            Err(DockLockedPayloadForwardProjectionError::PayloadAmbiguous)
        ));
    }

    #[test]
    fn leaf_center_allows_target_items_change_but_rejects_root_owner_change() {
        let mut graph = DockGraph::new();
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let other_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("c")],
            selected: Some(item("c")),
        });
        let root = graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![target_tabs, other_tabs],
            fractions: vec![0.5, 0.5],
        });
        graph.set_root(space(), root);
        let identity = DockLockedTargetIdentity::leaf_center(&graph, space(), root, target_tabs)
            .expect("the original leaf-center owner should lock");

        graph
            .apply_op_checked(&DockOp::OpenItem {
                space: space(),
                target_tabs: Some(target_tabs),
                item: item("x"),
                insert_index: None,
            })
            .expect("target item change should commit");
        identity
            .validate(&graph)
            .expect("center target contents must not define its owner identity");

        graph.remove_root(&space());
        graph.set_root(space(), target_tabs);
        assert_eq!(
            identity.validate(&graph),
            Err(DockActionApplyError::DropTargetUnavailable)
        );
    }

    #[test]
    fn floating_title_bar_rejects_owner_removal_without_retargeting_tabs() {
        let mut graph = DockGraph::new();
        let root_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a")],
            selected: Some(item("a")),
        });
        let target_tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("b")],
            selected: Some(item("b")),
        });
        let floating = graph.insert_node(DockNode::Floating { child: target_tabs });
        graph.set_root(space(), root_tabs);
        graph
            .floating_containers_mut(space())
            .push(DockFloatingContainer {
                node: floating,
                bounds: bounds(0.0),
            });
        let identity =
            DockLockedTargetIdentity::floating_title_bar(&graph, space(), floating, target_tabs)
                .expect("the original floating title-bar owner should lock");

        graph.floating_containers_mut(space()).clear();
        graph.set_root(space(), target_tabs);
        assert_eq!(
            identity.validate(&graph),
            Err(DockActionApplyError::DropTargetUnavailable)
        );
    }

    #[test]
    fn tab_bar_rejects_ordered_item_sequence_change() {
        let mut graph = DockGraph::new();
        let tabs = graph.insert_node(DockNode::Tabs {
            items: vec![item("a"), item("b")],
            selected: Some(item("a")),
        });
        graph.set_root(space(), tabs);
        let identity = DockLockedTargetIdentity::tab_bar(&graph, space(), tabs, 1)
            .expect("the original tab gap should lock");

        graph
            .apply_op_checked(&DockOp::OpenItem {
                space: space(),
                target_tabs: Some(tabs),
                item: item("x"),
                insert_index: Some(0),
            })
            .expect("target sequence change should commit");
        assert_eq!(
            identity.validate(&graph),
            Err(DockActionApplyError::DropTargetUnavailable)
        );
    }
}
