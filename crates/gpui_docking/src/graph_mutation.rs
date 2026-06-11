use crate::{DockItemId, DockNodeId, DockSpaceId, split_fraction::normalize_shares};

use super::{DockGraph, DockNode, DropZone};

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
        self.set_root_for_empty_space(space, tabs);
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

        let new_tabs = self.insert_node(DockNode::Tabs {
            items: vec![item],
            active: 0,
        });

        if !self.insert_edge_docked_child(target_space, target_tabs, zone, new_tabs) {
            return false;
        }
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
        self.set_root_for_empty_space(target_space, tabs);
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

        let new_tabs = self.insert_detached_tabs(detached);

        if !self.insert_edge_docked_child(target_space, target_tabs, zone, new_tabs) {
            return false;
        }
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
        self.set_root_for_empty_space(target_space, tabs);
        self.simplify_space(target_space);
        true
    }
}
