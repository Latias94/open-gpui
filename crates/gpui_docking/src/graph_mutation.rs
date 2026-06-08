use crate::{DockItemId, DockNodeId, DockSpaceId};
use open_gpui::{Bounds, Pixels};

use super::{
    DockFloatingContainer, DockGraph, DockNode, DropZone, EdgeDockDecision, SplitAxis,
    normalize_shares,
};

impl DockGraph {
    /// Selects an active tab by index.
    pub fn set_active_tab(&mut self, tabs: DockNodeId, active: usize) -> bool {
        let Some(DockNode::Tabs {
            items,
            active: current,
        }) = self.nodes.get_mut(tabs)
        else {
            return false;
        };

        let next = if items.is_empty() {
            0
        } else {
            active.min(items.len().saturating_sub(1))
        };
        if *current == next {
            return false;
        }
        *current = next;
        true
    }

    /// Updates a two-child split by setting the first child's fraction.
    pub fn update_split_two(&mut self, split: DockNodeId, first_fraction: f32) -> bool {
        let Some(DockNode::Split {
            children,
            fractions,
            ..
        }) = self.nodes.get_mut(split)
        else {
            return false;
        };
        if children.len() != 2 || fractions.len() != 2 {
            return false;
        }

        let first = first_fraction.clamp(0.0, 1.0);
        let next = [first, 1.0 - first];
        if fractions
            .iter()
            .zip(next.iter())
            .all(|(current, next)| (*current - *next).abs() <= 0.0001)
        {
            return false;
        }
        fractions[0] = first;
        fractions[1] = 1.0 - first;
        true
    }

    /// Replaces a split's fractions after sanitizing and normalizing them.
    pub fn update_split_fractions(&mut self, split: DockNodeId, mut next: Vec<f32>) -> bool {
        let Some(DockNode::Split {
            children,
            fractions,
            ..
        }) = self.nodes.get_mut(split)
        else {
            return false;
        };
        if children.len() < 2 || next.len() != children.len() {
            return false;
        }

        normalize_shares(&mut next);
        if fractions.len() == next.len()
            && fractions
                .iter()
                .zip(next.iter())
                .all(|(current, next)| (*current - *next).abs() <= 0.0001)
        {
            return false;
        }
        *fractions = next;
        true
    }

    pub(in crate::graph) fn close_item(&mut self, space: &DockSpaceId, item: DockItemId) -> bool {
        let Some((tabs, index)) = self.find_item_in_space(space, &item) else {
            return false;
        };
        if !self.remove_item_from_tabs(tabs, index) {
            return false;
        }
        self.simplify_space(space);
        true
    }

    pub(in crate::graph) fn open_item(
        &mut self,
        space: &DockSpaceId,
        target_tabs: Option<DockNodeId>,
        item: DockItemId,
        insert_index: Option<usize>,
    ) -> bool {
        if self.contains_item(&item) {
            return false;
        }

        if let Some(target_tabs) = target_tabs {
            if self.root_for_node_in_space(space, target_tabs).is_none() {
                return false;
            }
            return self.insert_item_into_tabs_at(target_tabs, item, insert_index);
        }

        if self.root(space).is_some() || !self.floating_containers(space).is_empty() {
            return false;
        }

        let tabs = self.insert_node(DockNode::Tabs {
            items: vec![item],
            active: 0,
        });
        self.set_root(space.clone(), tabs);
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::graph) fn move_item_between_spaces(
        &mut self,
        source_space: &DockSpaceId,
        item: DockItemId,
        target_space: &DockSpaceId,
        target_tabs: DockNodeId,
        zone: DropZone,
        insert_index: Option<usize>,
    ) -> bool {
        let Some((source_tabs, source_index)) = self.find_item_in_space(source_space, &item) else {
            return false;
        };

        if zone == DropZone::Center
            && source_space == target_space
            && source_tabs == target_tabs
            && insert_index.is_none()
        {
            return false;
        }
        if self
            .root_for_node_in_space(target_space, target_tabs)
            .is_none()
        {
            return false;
        }
        if zone == DropZone::Center
            && !matches!(self.nodes.get(target_tabs), Some(DockNode::Tabs { .. }))
        {
            return false;
        }

        if !self.remove_item_from_tabs(source_tabs, source_index) {
            return false;
        }

        if zone == DropZone::Center {
            let mut index = insert_index;
            if source_space == target_space
                && source_tabs == target_tabs
                && let Some(i) = index.as_mut()
                && *i > source_index
            {
                *i = i.saturating_sub(1);
            }

            let ok = self.insert_item_into_tabs_at(target_tabs, item, index);
            self.simplify_space(source_space);
            if source_space != target_space {
                self.simplify_space(target_space);
            }
            return ok;
        }

        let Some(axis) = zone.axis() else {
            return false;
        };
        let new_tabs = self.insert_node(DockNode::Tabs {
            items: vec![item],
            active: 0,
        });

        if self.insert_edge_child_prefer_same_axis_split(
            target_space,
            target_tabs,
            axis,
            zone,
            new_tabs,
        ) {
            self.simplify_space(source_space);
            if source_space != target_space {
                self.simplify_space(target_space);
            }
            return true;
        }

        let (first, second) = ordered_edge_children(zone, new_tabs, target_tabs);
        let split = self.insert_node(DockNode::Split {
            axis,
            children: vec![first, second],
            fractions: vec![0.5, 0.5],
        });
        self.replace_node_in_space_tree(target_space, target_tabs, split);
        self.simplify_space(source_space);
        if source_space != target_space {
            self.simplify_space(target_space);
        }
        true
    }

    pub(in crate::graph) fn move_item_to_empty_space(
        &mut self,
        source_space: &DockSpaceId,
        item: DockItemId,
        target_space: &DockSpaceId,
    ) -> bool {
        let Some((source_tabs, source_index)) = self.find_item_in_space(source_space, &item) else {
            return false;
        };
        if !self.remove_item_from_tabs(source_tabs, source_index) {
            return false;
        }
        let tabs = self.insert_node(DockNode::Tabs {
            items: vec![item],
            active: 0,
        });
        self.set_root(target_space.clone(), tabs);
        self.simplify_space(source_space);
        self.simplify_space(target_space);
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::graph) fn move_tabs_between_spaces(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
        target_tabs: DockNodeId,
        zone: DropZone,
        insert_index: Option<usize>,
    ) -> bool {
        if source_space == target_space && source_tabs == target_tabs {
            return false;
        }
        if self
            .root_for_node_in_space(target_space, target_tabs)
            .is_none()
        {
            return false;
        }

        if zone == DropZone::Center
            && !matches!(self.nodes.get(target_tabs), Some(DockNode::Tabs { .. }))
        {
            return false;
        }

        let Some(detached) = self.take_tabs_from_space(source_space, source_tabs) else {
            return false;
        };

        if zone == DropZone::Center {
            let ok = self.insert_items_into_tabs_at(
                target_tabs,
                &detached.items,
                insert_index,
                detached.active,
            );
            self.simplify_space(target_space);
            return ok;
        }

        let Some(axis) = zone.axis() else {
            return false;
        };
        let new_tabs = self.insert_detached_tabs(detached);

        if self.insert_edge_child_prefer_same_axis_split(
            target_space,
            target_tabs,
            axis,
            zone,
            new_tabs,
        ) {
            self.simplify_space(target_space);
            return true;
        }

        let (first, second) = ordered_edge_children(zone, new_tabs, target_tabs);
        let split = self.insert_node(DockNode::Split {
            axis,
            children: vec![first, second],
            fractions: vec![0.5, 0.5],
        });
        self.replace_node_in_space_tree(target_space, target_tabs, split);
        self.simplify_space(target_space);
        true
    }

    pub(in crate::graph) fn move_tabs_to_empty_space(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
    ) -> bool {
        let Some(detached) = self.take_tabs_from_space(source_space, source_tabs) else {
            return false;
        };
        let tabs = self.insert_detached_tabs(detached);
        self.set_root(target_space.clone(), tabs);
        self.simplify_space(target_space);
        true
    }

    pub(in crate::graph) fn float_item_in_window(
        &mut self,
        source_space: &DockSpaceId,
        item: DockItemId,
        target_space: &DockSpaceId,
        bounds: Bounds<Pixels>,
    ) -> bool {
        let Some((source_tabs, source_index)) = self.find_item_in_space(source_space, &item) else {
            return false;
        };
        if !self.remove_item_from_tabs(source_tabs, source_index) {
            return false;
        }

        let tabs = self.insert_node(DockNode::Tabs {
            items: vec![item],
            active: 0,
        });
        let floating = self.insert_node(DockNode::Floating { child: tabs });
        self.floating_containers_mut(target_space.clone())
            .push(DockFloatingContainer {
                node: floating,
                bounds,
            });
        self.simplify_space(source_space);
        self.simplify_space(target_space);
        true
    }

    pub(in crate::graph) fn float_tabs_in_window(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
        bounds: Bounds<Pixels>,
    ) -> bool {
        let Some(detached) = self.take_tabs_from_space(source_space, source_tabs) else {
            return false;
        };
        let tabs = self.insert_detached_tabs(detached);
        let floating = self.insert_node(DockNode::Floating { child: tabs });
        self.floating_containers_mut(target_space.clone())
            .push(DockFloatingContainer {
                node: floating,
                bounds,
            });
        self.simplify_space(target_space);
        true
    }

    pub(in crate::graph) fn set_floating_bounds(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
        bounds: Bounds<Pixels>,
    ) -> bool {
        let Some(floatings) = self.floatings.get_mut(space) else {
            return false;
        };
        let Some(container) = floatings.iter_mut().find(|entry| entry.node == floating) else {
            return false;
        };
        if container.bounds == bounds {
            return false;
        }
        container.bounds = bounds;
        true
    }

    pub(in crate::graph) fn raise_floating(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
    ) -> bool {
        let Some(floatings) = self.floatings.get_mut(space) else {
            return false;
        };
        let Some(index) = floatings.iter().position(|entry| entry.node == floating) else {
            return false;
        };
        if index + 1 == floatings.len() {
            return false;
        }
        let entry = floatings.remove(index);
        floatings.push(entry);
        true
    }

    pub(in crate::graph) fn merge_floating_into(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
        target_tabs: DockNodeId,
    ) -> bool {
        let Some(floatings) = self.floatings.get(space) else {
            return false;
        };
        if !floatings.iter().any(|entry| entry.node == floating) {
            return false;
        }
        if !matches!(self.nodes.get(target_tabs), Some(DockNode::Tabs { .. })) {
            return false;
        }
        let Some(target_root) = self.root_for_node_in_space(space, target_tabs) else {
            return false;
        };
        if target_root == floating {
            return false;
        }

        let items = self.collect_items_in_subtree(floating);
        for item in items {
            let _ = self.move_item_between_spaces(
                space,
                item,
                space,
                target_tabs,
                DropZone::Center,
                None,
            );
        }
        if let Some(floatings) = self.floatings.get_mut(space)
            && let Some(index) = floatings.iter().position(|entry| entry.node == floating)
        {
            floatings.remove(index);
        }
        self.simplify_space(space);
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
