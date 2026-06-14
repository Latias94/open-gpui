use crate::{
    DockGraphDropTarget, DockItemId, DockNodeId, DockSpaceId, split_fraction::normalize_shares,
};

use super::{DockGraph, DockNode};

impl DockGraph {
    /// Selects a tab by item identity.
    pub fn select_tab(&mut self, tabs: DockNodeId, item: DockItemId) -> bool {
        let Some(DockNode::Tabs {
            items,
            selected: current,
        }) = self.nodes.get_mut(tabs)
        else {
            return false;
        };
        let next = items.contains(&item).then_some(item);
        if current == &next {
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
            items: vec![item.clone()],
            selected: Some(item),
        });
        self.set_root_for_empty_space(space, tabs);
        true
    }

    pub(in crate::graph) fn move_item_between_spaces(
        &mut self,
        source_space: &DockSpaceId,
        item: DockItemId,
        target_space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> bool {
        let Some((source_tabs, source_index)) = self.find_item_in_space(source_space, &item) else {
            return false;
        };

        match target {
            DockGraphDropTarget::Center { tabs } => {
                if source_space == target_space && source_tabs == tabs {
                    return false;
                }
                if self.root_for_node_in_space(target_space, tabs).is_none() {
                    return false;
                }
                if !matches!(self.nodes.get(tabs), Some(DockNode::Tabs { .. })) {
                    return false;
                }
                if !self.remove_item_from_tabs(source_tabs, source_index) {
                    return false;
                }

                let ok = self.insert_item_into_tabs_at(tabs, item, None);
                self.simplify_space(source_space);
                if source_space != target_space {
                    self.simplify_space(target_space);
                }
                ok
            }
            DockGraphDropTarget::TabBar { tabs, insert_index } => {
                if self.root_for_node_in_space(target_space, tabs).is_none() {
                    return false;
                }
                if !matches!(self.nodes.get(tabs), Some(DockNode::Tabs { .. })) {
                    return false;
                }
                if source_space == target_space && source_tabs == tabs {
                    let same_position = insert_index == source_index
                        || insert_index == source_index.saturating_add(1);
                    if same_position {
                        return false;
                    }
                }
                if !self.remove_item_from_tabs(source_tabs, source_index) {
                    return false;
                }

                let mut index = Some(insert_index);
                if source_space == target_space
                    && source_tabs == tabs
                    && insert_index > source_index
                {
                    index = Some(insert_index.saturating_sub(1));
                }

                let ok = self.insert_item_into_tabs_at(tabs, item, index);
                self.simplify_space(source_space);
                if source_space != target_space {
                    self.simplify_space(target_space);
                }
                ok
            }
            DockGraphDropTarget::Edge { anchor, zone } => {
                let target_node = anchor.node();
                let Some(edge_plan) = self.edge_dock_plan(target_space, target_node, zone) else {
                    return false;
                };
                if !self.remove_item_from_tabs(source_tabs, source_index) {
                    return false;
                }
                let new_tabs = self.insert_node(DockNode::Tabs {
                    items: vec![item.clone()],
                    selected: Some(item),
                });

                if !self.apply_edge_dock_plan(target_space, edge_plan, new_tabs) {
                    return false;
                }
                self.simplify_space(source_space);
                if source_space != target_space {
                    self.simplify_space(target_space);
                }
                true
            }
            DockGraphDropTarget::EmptySpace => {
                if !self.target_space_is_empty_for_item_move(source_space, &item, target_space) {
                    return false;
                }
                self.move_item_to_empty_space(source_space, item, target_space)
            }
        }
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
            items: vec![item.clone()],
            selected: Some(item),
        });
        self.set_root_for_empty_space(target_space, tabs);
        self.simplify_space(source_space);
        self.simplify_space(target_space);
        true
    }

    pub(in crate::graph) fn move_tabs_between_spaces(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> bool {
        match target {
            DockGraphDropTarget::Center { tabs } | DockGraphDropTarget::TabBar { tabs, .. } => {
                if source_space == target_space && source_tabs == tabs {
                    return false;
                }
                if self.root_for_node_in_space(target_space, tabs).is_none() {
                    return false;
                }
                if !matches!(self.nodes.get(tabs), Some(DockNode::Tabs { .. })) {
                    return false;
                }
                let Some(detached) =
                    self.take_tabs_from_space_without_simplify(source_space, source_tabs)
                else {
                    return false;
                };
                let ok = self.insert_items_into_tabs_at(
                    tabs,
                    &detached.items,
                    match target {
                        DockGraphDropTarget::Center { .. } => None,
                        DockGraphDropTarget::TabBar { insert_index, .. } => Some(insert_index),
                        DockGraphDropTarget::Edge { .. } | DockGraphDropTarget::EmptySpace => {
                            unreachable!()
                        }
                    },
                    detached.selected.as_ref(),
                );
                self.simplify_space(source_space);
                if source_space != target_space {
                    self.simplify_space(target_space);
                }
                ok
            }
            DockGraphDropTarget::Edge { anchor, zone } => {
                let target_node = anchor.node();
                let Some(edge_plan) = self.edge_dock_plan(target_space, target_node, zone) else {
                    return false;
                };
                let Some(detached) =
                    self.take_tabs_from_space_without_simplify(source_space, source_tabs)
                else {
                    return false;
                };
                let new_tabs = self.insert_detached_tabs(detached);

                if !self.apply_edge_dock_plan(target_space, edge_plan, new_tabs) {
                    return false;
                }
                self.simplify_space(source_space);
                if source_space != target_space {
                    self.simplify_space(target_space);
                }
                true
            }
            DockGraphDropTarget::EmptySpace => {
                if !self.target_space_is_empty_for_tabs_move(
                    source_space,
                    source_tabs,
                    target_space,
                ) {
                    return false;
                }
                self.move_tabs_to_empty_space(source_space, source_tabs, target_space)
            }
        }
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
        self.set_root_for_empty_space(target_space, tabs);
        self.simplify_space(target_space);
        true
    }

    pub(crate) fn move_root_to_empty_space(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> bool {
        if source_space == target_space
            || self.root(target_space).is_some()
            || !self.floating_containers(target_space).is_empty()
        {
            return false;
        }

        let Some(source_root) = self.remove_root(source_space) else {
            return false;
        };

        self.set_root_for_empty_space(target_space, source_root);
        self.simplify_space(source_space);
        self.simplify_space(target_space);
        true
    }

    pub(crate) fn move_root_to_non_empty_space(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> bool {
        if source_space == target_space {
            return false;
        }

        let Some(source_root) = self.root(source_space) else {
            return false;
        };
        let Some(target_root) = self.root(target_space) else {
            return false;
        };

        match self.nodes.get(source_root).cloned() {
            Some(DockNode::Tabs { .. }) => self.move_root_tabs_to_non_empty_space(
                source_space,
                source_root,
                target_space,
                target_root,
            ),
            Some(DockNode::Split { .. }) => false,
            Some(DockNode::Floating { .. }) => false,
            None => false,
        }
    }

    fn move_root_tabs_to_non_empty_space(
        &mut self,
        source_space: &DockSpaceId,
        source_root: DockNodeId,
        target_space: &DockSpaceId,
        target_root: DockNodeId,
    ) -> bool {
        let Some(detached) = self.take_tabs_from_space_without_simplify(source_space, source_root)
        else {
            return false;
        };
        let target_tabs = self
            .selected_tabs_in_subtree(target_root)
            .unwrap_or(target_root);
        if !self.insert_items_into_tabs_at(
            target_tabs,
            &detached.items,
            None,
            detached.selected.as_ref(),
        ) {
            return false;
        }
        self.remove_subtree(source_root);
        self.simplify_space(source_space);
        self.simplify_space(target_space);
        true
    }
}
