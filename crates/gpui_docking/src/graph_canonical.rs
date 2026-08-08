use crate::{DockNodeId, DockSpaceId};
use open_gpui_ui_core::normalize_split_fractions;
use std::collections::HashSet;

use super::{DockGraph, DockNode, SplitAxis};

impl DockGraph {
    /// Simplifies every tree in one dock space into canonical form.
    ///
    /// This operation preserves every stored node identity. In particular, nodes detached by the
    /// simplification and nodes inserted for a later [`DockGraph::set_root`] call remain available
    /// to the caller. Use [`Self::canonicalize`] only at an explicit complete-graph commit boundary
    /// when all unattached nodes may be discarded.
    pub fn simplify_space(&mut self, space: &DockSpaceId) {
        self.simplify_space_structure(space);
    }

    /// Simplifies one space after a committed graph mutation and reclaims only nodes that the
    /// mutation detached from that space.
    ///
    /// Unattached staging roots and their transitive dependencies remain authoritative. This keeps
    /// the public insert-then-attach construction window intact while preventing ordinary runtime
    /// mutations from accumulating the nodes they replace.
    pub(crate) fn simplify_space_after_mutation(&mut self, space: &DockSpaceId) {
        let globally_reachable_before = self.reachable_node_ids();
        let staged_roots = self
            .nodes
            .keys()
            .filter(|node| !globally_reachable_before.contains(node))
            .collect::<Vec<_>>();
        let staged_dependencies = self.reachable_node_ids_from(staged_roots);
        let previously_reachable = self.reachable_node_ids_for_space(space);

        self.simplify_space_structure(space);
        self.remove_nodes_detached_from(previously_reachable, &staged_dependencies);
    }

    /// Canonicalizes a fully assembled graph and removes every unattached node.
    ///
    /// Call this only at builder, import, or equivalent commit boundaries where no staged node is
    /// expected to remain unattached. Runtime mutations should use [`Self::simplify_space`].
    pub fn canonicalize(&mut self) {
        for space in self.spaces() {
            self.simplify_space_structure(&space);
        }
        self.prune_unreachable_nodes();
    }

    fn simplify_space_structure(&mut self, space: &DockSpaceId) {
        let previous_root = self.root(space);
        let simplified_root = previous_root.and_then(|root| self.simplify_subtree(root));
        match simplified_root {
            Some(root) => self.set_root(space.clone(), root),
            None => {
                self.remove_root(space);
            }
        }
        if self.central_regions.contains_key(space) {
            let mut central_node = self
                .central_regions
                .get(space)
                .and_then(|central| central.node);
            if let (Some(previous_root), Some(simplified_root)) = (previous_root, simplified_root)
                && central_node == Some(previous_root)
                && previous_root != simplified_root
            {
                central_node = Some(simplified_root);
            }
            if central_node.is_some_and(|node| !self.root_subtree_contains(space, node)) {
                central_node = None;
            }
            if let Some(central) = self.central_regions.get_mut(space) {
                central.node = central_node;
            }
        }

        if let Some(mut floatings) = self.floatings.remove(space) {
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
    }

    fn prune_unreachable_nodes(&mut self) {
        let reachable = self.reachable_node_ids();
        let unreachable = self
            .nodes
            .keys()
            .filter(|node| !reachable.contains(node))
            .collect::<Vec<_>>();

        for node in unreachable {
            self.nodes.remove(node);
            self.tab_selection_history.remove(&node);
        }
        self.tab_selection_history
            .retain(|node, _| self.nodes.contains_key(*node));
    }

    fn remove_nodes_detached_from(
        &mut self,
        candidates: HashSet<DockNodeId>,
        staged_dependencies: &HashSet<DockNodeId>,
    ) {
        let still_reachable = self.reachable_node_ids();
        for node in candidates
            .difference(&still_reachable)
            .filter(|node| !staged_dependencies.contains(node))
            .copied()
        {
            self.nodes.remove(node);
            self.tab_selection_history.remove(&node);
        }
    }

    fn reachable_node_ids_for_space(&self, space: &DockSpaceId) -> HashSet<DockNodeId> {
        self.reachable_node_ids_from(
            self.root(space)
                .into_iter()
                .chain(
                    self.floating_containers(space)
                        .iter()
                        .map(|floating| floating.node),
                )
                .collect(),
        )
    }

    fn reachable_node_ids(&self) -> HashSet<DockNodeId> {
        self.reachable_node_ids_from(
            self.roots
                .values()
                .copied()
                .chain(
                    self.floatings
                        .values()
                        .flatten()
                        .map(|floating| floating.node),
                )
                .collect(),
        )
    }

    fn reachable_node_ids_from(&self, mut pending: Vec<DockNodeId>) -> HashSet<DockNodeId> {
        let mut reachable = HashSet::new();
        while let Some(node) = pending.pop() {
            if !reachable.insert(node) {
                continue;
            }
            match self.nodes.get(node) {
                Some(DockNode::Split { children, .. }) => pending.extend(children.iter().copied()),
                Some(DockNode::Floating { child }) => pending.push(*child),
                Some(DockNode::Tabs { .. }) | None => {}
            }
        }

        reachable
    }

    #[cfg(test)]
    pub(crate) fn stored_node_count(&self) -> usize {
        self.nodes.len()
    }

    #[cfg(test)]
    pub(crate) fn reachable_node_count(&self) -> usize {
        self.reachable_node_ids()
            .into_iter()
            .filter(|node| self.nodes.contains_key(*node))
            .count()
    }

    fn simplify_subtree(&mut self, node: DockNodeId) -> Option<DockNodeId> {
        let node_value = self.nodes.get(node)?.clone();
        match node_value {
            DockNode::Tabs { items, .. } => {
                if items.is_empty() {
                    return None;
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

                normalize_split_fractions(&mut next_fractions);

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
                normalize_split_fractions(&mut shares);
                for (&grand_child, &share) in grand_children.iter().zip(shares.iter()) {
                    out_children.push(grand_child);
                    out_fractions.push(parent_share * share);
                }
            }

            *children = out_children;
            *fractions = out_fractions;
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
            DockNode::Tabs { items, selected } => {
                assert!(!items.is_empty(), "tabs nodes must be non-empty");
                assert!(
                    selected.as_ref().is_some_and(|item| items.contains(item)),
                    "selected tab item must be present"
                );
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
