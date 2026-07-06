use crate::{DockEdgeDockSizingScope, DockNodeId, DockSpaceId};
use open_gpui_ui_core::normalize_split_fractions;
use std::collections::HashMap;

use super::{DockEdgeDockPlan, DockEdgeDockSizing, DockGraph, DockNode, DropZone, SplitAxis};

impl DockGraph {
    /// Plans the n-ary topology change for an edge dock.
    pub fn edge_dock_plan(
        &self,
        space: &DockSpaceId,
        target: DockNodeId,
        zone: DropZone,
    ) -> Option<DockEdgeDockPlan> {
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
            let anchor_child = children[anchor_index];
            return Some(DockEdgeDockPlan::InsertIntoSplit {
                split: target,
                zone,
                anchor_child,
                anchor_index,
                insert_index,
                sizing: DockEdgeDockSizing::fallback(),
                sizing_scope: DockEdgeDockSizingScope::WholeSplit,
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

            // Inner edge docking is scoped to the hit node. Reusing an ancestor split is only
            // valid until the first real split boundary; crossing an opposing-axis split would
            // dock beside that subtree instead of inside the hit leaf.
            if *split_axis != axis || children.is_empty() || children.len() != fractions.len() {
                break;
            }

            let Some(anchor_index) = index.split_child_index.get(&cur).copied() else {
                break;
            };
            let insert_index = match zone {
                DropZone::Left | DropZone::Top => anchor_index,
                DropZone::Right | DropZone::Bottom => anchor_index.saturating_add(1),
                DropZone::Center => unreachable!(),
            };
            let anchor_child = children[anchor_index];
            return Some(DockEdgeDockPlan::InsertIntoSplit {
                split: parent,
                zone,
                anchor_child,
                anchor_index,
                insert_index,
                sizing: DockEdgeDockSizing::fallback(),
                sizing_scope: DockEdgeDockSizingScope::AnchorChild,
            });
        }

        Some(DockEdgeDockPlan::WrapTarget {
            target,
            axis,
            zone,
            sizing: DockEdgeDockSizing::fallback(),
        })
    }

    /// Plans the n-ary topology and initial split sizing for an edge dock.
    pub(crate) fn edge_dock_plan_with_sizing(
        &self,
        space: &DockSpaceId,
        target: DockNodeId,
        zone: DropZone,
        sizing: DockEdgeDockSizing,
    ) -> Option<DockEdgeDockPlan> {
        let mut plan = self.edge_dock_plan(space, target, zone)?;
        plan.set_sizing(sizing);
        Some(plan)
    }

    pub(in crate::graph) fn apply_edge_dock_plan(
        &mut self,
        space: &DockSpaceId,
        plan: DockEdgeDockPlan,
        new_child: DockNodeId,
    ) -> bool {
        match plan {
            DockEdgeDockPlan::InsertIntoSplit {
                split,
                zone: _,
                anchor_child: _,
                anchor_index,
                insert_index,
                sizing,
                sizing_scope,
            } => {
                let Some(DockNode::Split {
                    children,
                    fractions,
                    ..
                }) = self.nodes.get_mut(split)
                else {
                    return false;
                };
                split_share_and_insert(
                    children,
                    fractions,
                    anchor_index,
                    insert_index,
                    new_child,
                    sizing,
                    sizing_scope,
                )
            }
            DockEdgeDockPlan::WrapTarget {
                target,
                axis,
                zone,
                sizing,
            } => {
                let ordered_children = ordered_edge_children(zone, new_child, target);
                let ordered_fractions = ordered_edge_fractions(zone, sizing.new_child_share());
                let split = self.insert_node(DockNode::Split {
                    axis,
                    children: vec![ordered_children.leading, ordered_children.trailing],
                    fractions: vec![ordered_fractions.leading, ordered_fractions.trailing],
                });
                self.replace_node_in_space_tree(space, target, split)
            }
        }
    }

    pub(crate) fn edge_dock_plan_is_current(
        &self,
        space: &DockSpaceId,
        plan: DockEdgeDockPlan,
    ) -> bool {
        match plan {
            DockEdgeDockPlan::InsertIntoSplit {
                split,
                zone,
                anchor_child,
                anchor_index,
                insert_index,
                sizing,
                sizing_scope: _,
            } => {
                if zone == DropZone::Center || self.root_for_node_in_space(space, split).is_none() {
                    return false;
                }
                let Some(DockNode::Split {
                    axis,
                    children,
                    fractions,
                }) = self.nodes.get(split)
                else {
                    return false;
                };
                if zone.axis() != Some(*axis) {
                    return false;
                }
                !children.is_empty()
                    && children.len() == fractions.len()
                    && anchor_index < fractions.len()
                    && children.get(anchor_index) == Some(&anchor_child)
                    && insert_index <= fractions.len()
                    && edge_insert_index_matches_zone(zone, anchor_index, insert_index)
                    && sizing.is_valid()
            }
            DockEdgeDockPlan::WrapTarget {
                target,
                axis,
                zone,
                sizing,
            } => {
                zone.axis() == Some(axis)
                    && sizing.is_valid()
                    && self.root_for_node_in_space(space, target).is_some()
            }
        }
    }

    fn replace_node_in_space_tree(
        &mut self,
        space: &DockSpaceId,
        old: DockNodeId,
        new: DockNodeId,
    ) -> bool {
        if self.root(space) == Some(old) {
            self.set_root(space.clone(), new);
            return true;
        }
        if let Some(floatings) = self.floatings.get_mut(space) {
            for floating in floatings {
                if floating.node == old {
                    floating.node = new;
                    return true;
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
                return self.replace_child_in_node(parent, old, new);
            }
        }
        false
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

fn edge_insert_index_matches_zone(
    zone: DropZone,
    anchor_index: usize,
    insert_index: usize,
) -> bool {
    match zone {
        DropZone::Left | DropZone::Top => insert_index == anchor_index,
        DropZone::Right | DropZone::Bottom => insert_index == anchor_index.saturating_add(1),
        DropZone::Center => false,
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

struct DockOrderedEdgeChildren {
    leading: DockNodeId,
    trailing: DockNodeId,
}

struct DockOrderedEdgeFractions {
    leading: f32,
    trailing: f32,
}

fn ordered_edge_children(
    zone: DropZone,
    new_child: DockNodeId,
    target: DockNodeId,
) -> DockOrderedEdgeChildren {
    match zone {
        DropZone::Left | DropZone::Top => DockOrderedEdgeChildren {
            leading: new_child,
            trailing: target,
        },
        DropZone::Right | DropZone::Bottom => DockOrderedEdgeChildren {
            leading: target,
            trailing: new_child,
        },
        DropZone::Center => unreachable!(),
    }
}

fn ordered_edge_fractions(zone: DropZone, new_child_share: f32) -> DockOrderedEdgeFractions {
    let existing_share = 1.0 - new_child_share;
    match zone {
        DropZone::Left | DropZone::Top => DockOrderedEdgeFractions {
            leading: new_child_share,
            trailing: existing_share,
        },
        DropZone::Right | DropZone::Bottom => DockOrderedEdgeFractions {
            leading: existing_share,
            trailing: new_child_share,
        },
        DropZone::Center => unreachable!(),
    }
}

fn split_share_and_insert(
    children: &mut Vec<DockNodeId>,
    fractions: &mut Vec<f32>,
    anchor_index: usize,
    insert_index: usize,
    new_child: DockNodeId,
    sizing: DockEdgeDockSizing,
    sizing_scope: DockEdgeDockSizingScope,
) -> bool {
    if children.is_empty()
        || children.len() != fractions.len()
        || anchor_index >= fractions.len()
        || insert_index > fractions.len()
    {
        return false;
    }

    let take = match sizing_scope {
        DockEdgeDockSizingScope::WholeSplit => sizing.new_child_share(),
        DockEdgeDockSizingScope::AnchorChild => fractions[anchor_index] * sizing.new_child_share(),
    };
    let scale = 1.0 - take;
    match sizing_scope {
        DockEdgeDockSizingScope::WholeSplit => {
            for fraction in fractions.iter_mut() {
                *fraction *= scale;
            }
        }
        DockEdgeDockSizingScope::AnchorChild => {
            fractions[anchor_index] -= take;
        }
    }
    children.insert(insert_index, new_child);
    fractions.insert(insert_index, take);
    normalize_split_fractions(fractions);
    true
}
