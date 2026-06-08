use crate::{DockNodeId, DockSpaceId};

use super::{DockGraph, DockNode, DropZone, EdgeDockDecision, SplitAxis, normalize_shares};

impl DockGraph {
    pub(in crate::graph) fn insert_edge_docked_child(
        &mut self,
        space: &DockSpaceId,
        target: DockNodeId,
        zone: DropZone,
        new_child: DockNodeId,
    ) -> bool {
        let Some(axis) = zone.axis() else {
            return false;
        };

        if self.insert_edge_child_prefer_same_axis_split(space, target, axis, zone, new_child) {
            return true;
        }

        let (first, second) = ordered_edge_children(zone, new_child, target);
        let split = self.insert_node(DockNode::Split {
            axis,
            children: vec![first, second],
            fractions: vec![0.5, 0.5],
        });
        self.replace_node_in_space_tree(space, target, split);
        true
    }

    fn insert_edge_child_prefer_same_axis_split(
        &mut self,
        space: &DockSpaceId,
        target: DockNodeId,
        axis: SplitAxis,
        zone: DropZone,
        new_child: DockNodeId,
    ) -> bool {
        let Some(EdgeDockDecision::InsertIntoSplit {
            split,
            anchor_index,
            insert_index,
        }) = self.edge_dock_decision(space, target, zone)
        else {
            return false;
        };

        let Some(DockNode::Split {
            axis: split_axis,
            children,
            fractions,
        }) = self.nodes.get_mut(split)
        else {
            return false;
        };
        if *split_axis != axis || children.len() != fractions.len() || children.is_empty() {
            return false;
        }
        split_share_and_insert(children, fractions, anchor_index, insert_index, new_child);
        true
    }

    fn replace_node_in_space_tree(
        &mut self,
        space: &DockSpaceId,
        old: DockNodeId,
        new: DockNodeId,
    ) {
        if self.root(space) == Some(old) {
            self.set_root(space.clone(), new);
            return;
        }
        if let Some(floatings) = self.floatings.get_mut(space) {
            for floating in floatings {
                if floating.node == old {
                    floating.node = new;
                    return;
                }
            }
        }

        let roots: Vec<DockNodeId> = self
            .root(space)
            .into_iter()
            .chain(
                self.floatings
                    .get(space)
                    .into_iter()
                    .flatten()
                    .map(|floating| floating.node),
            )
            .collect();
        for root in roots {
            if let Some(parent) = self.find_parent_in_subtree(root, old) {
                self.replace_child_in_node(parent, old, new);
                return;
            }
        }
    }

    fn replace_child_in_node(
        &mut self,
        node: DockNodeId,
        old: DockNodeId,
        new: DockNodeId,
    ) -> bool {
        let Some(node) = self.nodes.get_mut(node) else {
            return false;
        };
        match node {
            DockNode::Split { children, .. } => {
                let Some(index) = children.iter().position(|child| *child == old) else {
                    return false;
                };
                children[index] = new;
                true
            }
            DockNode::Floating { child } => {
                if *child != old {
                    return false;
                }
                *child = new;
                true
            }
            DockNode::Tabs { .. } => false,
        }
    }

    fn find_parent_in_subtree(&self, root: DockNodeId, target: DockNodeId) -> Option<DockNodeId> {
        match self.nodes.get(root)? {
            DockNode::Tabs { .. } => None,
            DockNode::Floating { child } => {
                if *child == target {
                    Some(root)
                } else {
                    self.find_parent_in_subtree(*child, target)
                }
            }
            DockNode::Split { children, .. } => {
                if children.contains(&target) {
                    return Some(root);
                }
                children
                    .iter()
                    .copied()
                    .find_map(|child| self.find_parent_in_subtree(child, target))
            }
        }
    }
}

fn ordered_edge_children(
    zone: DropZone,
    new_child: DockNodeId,
    target: DockNodeId,
) -> (DockNodeId, DockNodeId) {
    match zone {
        DropZone::Left | DropZone::Top => (new_child, target),
        DropZone::Right | DropZone::Bottom => (target, new_child),
        DropZone::Center => unreachable!(),
    }
}

fn split_share_and_insert(
    children: &mut Vec<DockNodeId>,
    fractions: &mut Vec<f32>,
    anchor_index: usize,
    insert_index: usize,
    new_child: DockNodeId,
) {
    if children.is_empty()
        || children.len() != fractions.len()
        || anchor_index >= fractions.len()
        || insert_index > fractions.len()
    {
        return;
    }

    let old = fractions[anchor_index];
    let keep = old * 0.5;
    let take = old * 0.5;
    fractions[anchor_index] = keep;
    children.insert(insert_index, new_child);
    fractions.insert(insert_index, take);
    normalize_shares(fractions);
}
