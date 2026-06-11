use crate::{DockNodeId, DockSpaceId, split_fraction::normalize_shares};
use std::collections::HashMap;

use super::{DockGraph, DockNode, DropZone, EdgeDockDecision, SplitAxis};

impl DockGraph {
    /// Decides whether an edge dock will insert into an existing same-axis split.
    pub fn edge_dock_decision(
        &self,
        space: &DockSpaceId,
        target: DockNodeId,
        zone: DropZone,
    ) -> Option<EdgeDockDecision> {
        if zone == DropZone::Center {
            return None;
        }

        let axis = zone.axis()?;
        let index = self.build_parent_index(space);
        if !index.root_for.contains_key(&target) {
            return None;
        }

        if let Some(DockNode::Split {
            axis: split_axis,
            children,
            fractions,
        }) = self.nodes.get(target)
            && *split_axis == axis
            && !children.is_empty()
            && children.len() == fractions.len()
        {
            let len = children.len();
            let (anchor_index, insert_index) = match zone {
                DropZone::Left | DropZone::Top => (0, 0),
                DropZone::Right | DropZone::Bottom => {
                    let last = len.saturating_sub(1);
                    (last, last.saturating_add(1))
                }
                DropZone::Center => unreachable!(),
            };
            return Some(EdgeDockDecision::InsertIntoSplit {
                split: target,
                anchor_index,
                insert_index,
            });
        }

        let mut cur = target;
        while let Some(parent) = index.parent.get(&cur).copied() {
            let Some(DockNode::Split {
                axis: split_axis,
                children,
                fractions,
            }) = self.nodes.get(parent)
            else {
                cur = parent;
                continue;
            };

            if *split_axis == axis && !children.is_empty() && children.len() == fractions.len() {
                let Some(anchor_index) = index.split_child_index.get(&cur).copied() else {
                    break;
                };
                let insert_index = match zone {
                    DropZone::Left | DropZone::Top => anchor_index,
                    DropZone::Right | DropZone::Bottom => anchor_index.saturating_add(1),
                    DropZone::Center => unreachable!(),
                };
                return Some(EdgeDockDecision::InsertIntoSplit {
                    split: parent,
                    anchor_index,
                    insert_index,
                });
            }

            cur = parent;
        }

        Some(EdgeDockDecision::WrapNewSplit)
    }

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

    fn build_parent_index(&self, space: &DockSpaceId) -> DockParentIndex {
        let mut index = DockParentIndex::default();
        if let Some(root) = self.root(space) {
            self.index_subtree(root, root, &mut index);
        }
        if let Some(floatings) = self.floatings.get(space) {
            for floating in floatings {
                self.index_subtree(floating.node, floating.node, &mut index);
            }
        }
        index
    }

    fn index_subtree(&self, root: DockNodeId, node: DockNodeId, index: &mut DockParentIndex) {
        if index.root_for.contains_key(&node) {
            return;
        }
        index.root_for.insert(node, root);
        let Some(current) = self.nodes.get(node) else {
            return;
        };
        match current {
            DockNode::Tabs { .. } => {}
            DockNode::Floating { child } => {
                index.parent.insert(*child, node);
                self.index_subtree(root, *child, index);
            }
            DockNode::Split { children, .. } => {
                for (child_index, child) in children.iter().copied().enumerate() {
                    index.parent.insert(child, node);
                    index.split_child_index.insert(child, child_index);
                    self.index_subtree(root, child, index);
                }
            }
        }
    }
}

impl DropZone {
    fn axis(self) -> Option<SplitAxis> {
        match self {
            DropZone::Left | DropZone::Right => Some(SplitAxis::Horizontal),
            DropZone::Top | DropZone::Bottom => Some(SplitAxis::Vertical),
            DropZone::Center => None,
        }
    }
}

#[derive(Default)]
struct DockParentIndex {
    root_for: HashMap<DockNodeId, DockNodeId>,
    parent: HashMap<DockNodeId, DockNodeId>,
    split_child_index: HashMap<DockNodeId, usize>,
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
