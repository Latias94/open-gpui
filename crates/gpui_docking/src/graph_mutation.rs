use crate::{DockItemId, DockNodeId, DockSpaceId};
use open_gpui::{Bounds, Pixels};
#[cfg(test)]
use std::collections::HashSet;

use super::{
    DockFloatingContainer, DockGraph, DockNode, DropZone, EdgeDockDecision, SplitAxis,
    normalize_shares,
};

impl DockGraph {
    /// Simplifies every tree in one dock space into canonical form.
    pub fn simplify_space(&mut self, space: &DockSpaceId) {
        if let Some(root) = self.root(space) {
            match self.simplify_subtree(root) {
                Some(root) => self.set_root(space.clone(), root),
                None => {
                    self.remove_root(space);
                }
            }
        }

        let Some(mut floatings) = self.floatings.remove(space) else {
            return;
        };

        floatings.retain_mut(|floating| match self.simplify_subtree(floating.node) {
            Some(node) => {
                floating.node = node;
                true
            }
            None => false,
        });

        if !floatings.is_empty() {
            self.floatings.insert(space.clone(), floatings);
        }
    }

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
            .root_for_node_in_space(source_space, source_tabs)
            .is_none()
            || self
                .root_for_node_in_space(target_space, target_tabs)
                .is_none()
        {
            return false;
        }

        let (items, active) = match self.nodes.get(source_tabs) {
            Some(DockNode::Tabs { items, active }) if !items.is_empty() => {
                (items.clone(), (*active).min(items.len().saturating_sub(1)))
            }
            _ => return false,
        };

        if zone == DropZone::Center
            && !matches!(self.nodes.get(target_tabs), Some(DockNode::Tabs { .. }))
        {
            return false;
        }

        if let Some(DockNode::Tabs { items, active }) = self.nodes.get_mut(source_tabs) {
            items.clear();
            *active = 0;
        }
        if self.root(source_space) == Some(source_tabs) {
            self.remove_root(source_space);
        }
        self.simplify_space(source_space);

        if zone == DropZone::Center {
            let ok = self.insert_items_into_tabs_at(target_tabs, &items, insert_index, active);
            self.simplify_space(target_space);
            return ok;
        }

        let Some(axis) = zone.axis() else {
            return false;
        };
        let new_tabs = self.insert_node(DockNode::Tabs { items, active });

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
        if self
            .root_for_node_in_space(source_space, source_tabs)
            .is_none()
        {
            return false;
        }

        let (items, active) = match self.nodes.get(source_tabs) {
            Some(DockNode::Tabs { items, active }) if !items.is_empty() => {
                (items.clone(), (*active).min(items.len().saturating_sub(1)))
            }
            _ => return false,
        };

        if let Some(DockNode::Tabs { items, active }) = self.nodes.get_mut(source_tabs) {
            items.clear();
            *active = 0;
        }
        if self.root(source_space) == Some(source_tabs) {
            self.remove_root(source_space);
        }
        let tabs = self.insert_node(DockNode::Tabs { items, active });
        self.set_root(target_space.clone(), tabs);
        self.simplify_space(source_space);
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
        if self
            .root_for_node_in_space(source_space, source_tabs)
            .is_none()
        {
            return false;
        }

        let (items, active) = match self.nodes.get(source_tabs) {
            Some(DockNode::Tabs { items, active }) if !items.is_empty() => {
                (items.clone(), (*active).min(items.len().saturating_sub(1)))
            }
            _ => return false,
        };

        if let Some(DockNode::Tabs { items, active }) = self.nodes.get_mut(source_tabs) {
            items.clear();
            *active = 0;
        }
        if self.root(source_space) == Some(source_tabs) {
            self.remove_root(source_space);
        }
        self.simplify_space(source_space);

        let tabs = self.insert_node(DockNode::Tabs { items, active });
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

    fn simplify_subtree(&mut self, node: DockNodeId) -> Option<DockNodeId> {
        let node_value = self.nodes.get(node)?.clone();
        match node_value {
            DockNode::Tabs { items, mut active } => {
                if items.is_empty() {
                    return None;
                }
                if active >= items.len() {
                    active = items.len().saturating_sub(1);
                }
                if let Some(DockNode::Tabs {
                    items: current_items,
                    active: current_active,
                }) = self.nodes.get_mut(node)
                {
                    *current_items = items;
                    *current_active = active;
                }
                Some(node)
            }
            DockNode::Floating { child } => {
                let child = self.simplify_subtree(child)?;
                if let Some(DockNode::Floating {
                    child: current_child,
                }) = self.nodes.get_mut(node)
                {
                    *current_child = child;
                }
                Some(node)
            }
            DockNode::Split {
                axis,
                children,
                fractions,
            } => {
                let mut next_children = Vec::new();
                let mut next_fractions = Vec::new();
                for (index, child) in children.into_iter().enumerate() {
                    let Some(child) = self.simplify_subtree(child) else {
                        continue;
                    };
                    next_children.push(child);
                    next_fractions.push(fractions.get(index).copied().unwrap_or(1.0));
                }

                if next_children.is_empty() {
                    return None;
                }
                if next_children.len() == 1 {
                    return Some(next_children[0]);
                }

                self.flatten_same_axis_splits(axis, &mut next_children, &mut next_fractions);

                if next_children.is_empty() {
                    return None;
                }
                if next_children.len() == 1 {
                    return Some(next_children[0]);
                }

                normalize_shares(&mut next_fractions);

                if let Some(DockNode::Split {
                    children: current_children,
                    fractions: current_fractions,
                    ..
                }) = self.nodes.get_mut(node)
                {
                    *current_children = next_children;
                    *current_fractions = next_fractions;
                }
                Some(node)
            }
        }
    }

    fn flatten_same_axis_splits(
        &mut self,
        axis: SplitAxis,
        children: &mut Vec<DockNodeId>,
        fractions: &mut Vec<f32>,
    ) {
        let mut changed = true;
        while changed {
            changed = false;
            let mut out_children = Vec::with_capacity(children.len());
            let mut out_fractions = Vec::with_capacity(fractions.len());

            for (child, parent_share) in children.iter().copied().zip(fractions.iter().copied()) {
                let Some(DockNode::Split {
                    axis: child_axis,
                    children: grand_children,
                    fractions: grand_fractions,
                }) = self.nodes.get(child)
                else {
                    out_children.push(child);
                    out_fractions.push(parent_share);
                    continue;
                };

                if *child_axis != axis {
                    out_children.push(child);
                    out_fractions.push(parent_share);
                    continue;
                }

                changed = true;
                let mut shares = grand_fractions.clone();
                normalize_shares(&mut shares);
                for (&grand_child, &share) in grand_children.iter().zip(shares.iter()) {
                    out_children.push(grand_child);
                    out_fractions.push(parent_share * share);
                }
            }

            *children = out_children;
            *fractions = out_fractions;
        }
    }

    fn insert_item_into_tabs_at(
        &mut self,
        tabs: DockNodeId,
        item: DockItemId,
        index: Option<usize>,
    ) -> bool {
        let Some(DockNode::Tabs {
            items,
            active: current_active,
        }) = self.nodes.get_mut(tabs)
        else {
            return false;
        };
        if items.contains(&item) {
            return true;
        }

        match index {
            Some(index) => {
                let index = index.min(items.len());
                items.insert(index, item);
                *current_active = index;
            }
            None => {
                items.push(item);
                *current_active = items.len().saturating_sub(1);
            }
        }
        true
    }

    fn insert_items_into_tabs_at(
        &mut self,
        tabs: DockNodeId,
        next_items: &[DockItemId],
        index: Option<usize>,
        active_in_group: usize,
    ) -> bool {
        let Some(DockNode::Tabs { items, active }) = self.nodes.get_mut(tabs) else {
            return false;
        };
        if next_items.is_empty() {
            return true;
        }

        let mut insert_at = index.unwrap_or(items.len()).min(items.len());
        for item in next_items {
            if items.contains(item) {
                continue;
            }
            items.insert(insert_at, item.clone());
            insert_at = insert_at.saturating_add(1);
        }
        if let Some(active_item) = next_items.get(active_in_group)
            && let Some(index) = items.iter().position(|item| item == active_item)
        {
            *active = index;
        }
        if items.is_empty() {
            *active = 0;
        } else if *active >= items.len() {
            *active = items.len().saturating_sub(1);
        }
        true
    }

    fn remove_item_from_tabs(&mut self, tabs: DockNodeId, index: usize) -> bool {
        let Some(DockNode::Tabs { items, active }) = self.nodes.get_mut(tabs) else {
            return false;
        };
        if index >= items.len() {
            return false;
        }

        items.remove(index);
        if items.is_empty() {
            *active = 0;
        } else if *active >= items.len() {
            *active = items.len().saturating_sub(1);
        } else if index < *active {
            *active = active.saturating_sub(1);
        }
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

    #[cfg(test)]
    pub(crate) fn assert_canonical_space(&self, space: &DockSpaceId) {
        let mut reachable = HashSet::new();
        if let Some(root) = self.root(space) {
            self.assert_canonical_subtree(root, &mut reachable);
        }
        for floating in self.floating_containers(space) {
            assert!(
                matches!(self.node(floating.node), Some(DockNode::Floating { .. })),
                "floating container must point to a Floating node"
            );
            self.assert_canonical_subtree(floating.node, &mut reachable);
        }
    }

    #[cfg(test)]
    fn assert_canonical_subtree(&self, root: DockNodeId, reachable: &mut HashSet<DockNodeId>) {
        assert!(
            reachable.insert(root),
            "dock graph contains a cycle or shared node"
        );
        let Some(node) = self.node(root) else {
            panic!("dock graph references missing node");
        };
        match node {
            DockNode::Tabs { items, active } => {
                assert!(!items.is_empty(), "tabs nodes must be non-empty");
                assert!(*active < items.len(), "active tab index must be in bounds");
            }
            DockNode::Floating { child } => self.assert_canonical_subtree(*child, reachable),
            DockNode::Split {
                axis,
                children,
                fractions,
            } => {
                assert!(
                    children.len() >= 2,
                    "split nodes must have at least two children"
                );
                assert_eq!(children.len(), fractions.len());
                let sum: f32 = fractions.iter().sum();
                assert!((sum - 1.0).abs() <= 1e-3, "fractions must be normalized");
                for fraction in fractions {
                    assert!(fraction.is_finite());
                    assert!(*fraction >= 0.0);
                }
                for child in children {
                    if let Some(DockNode::Split {
                        axis: child_axis, ..
                    }) = self.node(*child)
                    {
                        assert_ne!(axis, child_axis, "same-axis splits must be flattened");
                    }
                    self.assert_canonical_subtree(*child, reachable);
                }
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
