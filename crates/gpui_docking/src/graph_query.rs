use crate::{DockItemId, DockNodeId, DockSpaceId};

use super::{DockGraph, DockNode};

impl DockGraph {
    /// Returns all dock items reachable from a dock space.
    pub fn collect_items_in_space(&self, space: &DockSpaceId) -> Vec<DockItemId> {
        let mut out = Vec::new();
        if let Some(root) = self.root(space) {
            self.collect_items_in_subtree_into(root, &mut out);
        }
        if let Some(floatings) = self.floatings.get(space) {
            for floating in floatings {
                self.collect_items_in_subtree_into(floating.node, &mut out);
            }
        }
        out
    }

    /// Returns all dock items reachable from a subtree.
    pub fn collect_items_in_subtree(&self, root: DockNodeId) -> Vec<DockItemId> {
        let mut out = Vec::new();
        self.collect_items_in_subtree_into(root, &mut out);
        out
    }

    /// Returns all tabs nodes reachable from a dock space in stable tree order.
    pub fn tabs_in_space(&self, space: &DockSpaceId) -> Vec<DockNodeId> {
        let mut out = Vec::new();
        if let Some(root) = self.root(space) {
            self.collect_tabs_in_subtree_into(root, &mut out);
        }
        if let Some(floatings) = self.floatings.get(space) {
            for floating in floatings {
                self.collect_tabs_in_subtree_into(floating.node, &mut out);
            }
        }
        out
    }

    /// Returns the first tabs node reachable from a dock space.
    pub fn first_tabs_in_space(&self, space: &DockSpaceId) -> Option<DockNodeId> {
        self.tabs_in_space(space).into_iter().next()
    }

    /// Returns true when an item is reachable from any dock space.
    pub fn contains_item(&self, item: &DockItemId) -> bool {
        self.spaces()
            .iter()
            .any(|space| self.find_item_in_space(space, item).is_some())
    }

    /// Finds an item in a dock space and returns its tabs node and tab index.
    pub fn find_item_in_space(
        &self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> Option<(DockNodeId, usize)> {
        if let Some(root) = self.root(space)
            && let Some(found) = self.find_item_in_subtree(root, item)
        {
            return Some(found);
        }

        self.floatings.get(space).and_then(|floatings| {
            floatings
                .iter()
                .find_map(|floating| self.find_item_in_subtree(floating.node, item))
        })
    }

    /// Returns the root that contains a node within a dock space forest.
    pub fn root_for_node_in_space(
        &self,
        space: &DockSpaceId,
        target: DockNodeId,
    ) -> Option<DockNodeId> {
        if let Some(root) = self.root(space)
            && self.subtree_contains(root, target)
        {
            return Some(root);
        }

        self.floatings.get(space).and_then(|floatings| {
            floatings.iter().find_map(|floating| {
                self.subtree_contains(floating.node, target)
                    .then_some(floating.node)
            })
        })
    }

    /// Returns the active item in a tabs node, clamping stale indexes defensively.
    pub fn active_item_in_tabs(&self, tabs: DockNodeId) -> Option<DockItemId> {
        let DockNode::Tabs { items, active } = self.nodes.get(tabs)? else {
            return None;
        };
        items
            .get((*active).min(items.len().checked_sub(1)?))
            .cloned()
    }

    pub(in crate::graph) fn root_subtree_contains(
        &self,
        space: &DockSpaceId,
        target: DockNodeId,
    ) -> bool {
        self.root(space)
            .is_some_and(|root| self.subtree_contains(root, target))
    }

    fn find_item_in_subtree(
        &self,
        root: DockNodeId,
        item: &DockItemId,
    ) -> Option<(DockNodeId, usize)> {
        match self.nodes.get(root)? {
            DockNode::Tabs { items, .. } => items
                .iter()
                .position(|candidate| candidate == item)
                .map(|index| (root, index)),
            DockNode::Floating { child } => self.find_item_in_subtree(*child, item),
            DockNode::Split { children, .. } => children
                .iter()
                .copied()
                .find_map(|child| self.find_item_in_subtree(child, item)),
        }
    }

    fn collect_items_in_subtree_into(&self, root: DockNodeId, out: &mut Vec<DockItemId>) {
        let Some(node) = self.nodes.get(root) else {
            return;
        };
        match node {
            DockNode::Tabs { items, .. } => out.extend(items.iter().cloned()),
            DockNode::Floating { child } => self.collect_items_in_subtree_into(*child, out),
            DockNode::Split { children, .. } => {
                for child in children {
                    self.collect_items_in_subtree_into(*child, out);
                }
            }
        }
    }

    fn collect_tabs_in_subtree_into(&self, root: DockNodeId, out: &mut Vec<DockNodeId>) {
        let Some(node) = self.nodes.get(root) else {
            return;
        };
        match node {
            DockNode::Tabs { .. } => out.push(root),
            DockNode::Floating { child } => self.collect_tabs_in_subtree_into(*child, out),
            DockNode::Split { children, .. } => {
                for child in children {
                    self.collect_tabs_in_subtree_into(*child, out);
                }
            }
        }
    }

    pub(in crate::graph) fn subtree_contains(&self, root: DockNodeId, target: DockNodeId) -> bool {
        if root == target {
            return true;
        }
        let Some(node) = self.nodes.get(root) else {
            return false;
        };
        match node {
            DockNode::Tabs { .. } => false,
            DockNode::Floating { child } => self.subtree_contains(*child, target),
            DockNode::Split { children, .. } => children
                .iter()
                .copied()
                .any(|child| self.subtree_contains(child, target)),
        }
    }
}
