use crate::{DockItemId, DockNodeId, DockSpaceId};

use super::{DockGraph, DockNode};

pub(in crate::graph) struct DetachedTabs {
    pub(in crate::graph) items: Vec<DockItemId>,
    pub(in crate::graph) active: usize,
}

impl DockGraph {
    pub(in crate::graph) fn take_tabs_from_space(
        &mut self,
        space: &DockSpaceId,
        tabs: DockNodeId,
    ) -> Option<DetachedTabs> {
        self.take_tabs_from_space_with_simplify(space, tabs, true)
    }

    pub(in crate::graph) fn take_tabs_from_space_without_simplify(
        &mut self,
        space: &DockSpaceId,
        tabs: DockNodeId,
    ) -> Option<DetachedTabs> {
        self.take_tabs_from_space_with_simplify(space, tabs, false)
    }

    fn take_tabs_from_space_with_simplify(
        &mut self,
        space: &DockSpaceId,
        tabs: DockNodeId,
        simplify: bool,
    ) -> Option<DetachedTabs> {
        self.root_for_node_in_space(space, tabs)?;

        let (items, active) = match self.nodes.get(tabs) {
            Some(DockNode::Tabs { items, active }) if !items.is_empty() => {
                (items.clone(), (*active).min(items.len().saturating_sub(1)))
            }
            _ => return None,
        };

        if let Some(DockNode::Tabs { items, active }) = self.nodes.get_mut(tabs) {
            items.clear();
            *active = 0;
        }
        if self.root(space) == Some(tabs) {
            self.remove_root(space);
        }
        if simplify {
            self.simplify_space(space);
        }
        Some(DetachedTabs { items, active })
    }

    pub(in crate::graph) fn insert_detached_tabs(&mut self, detached: DetachedTabs) -> DockNodeId {
        self.insert_node(DockNode::Tabs {
            items: detached.items,
            active: detached.active,
        })
    }

    pub(in crate::graph) fn insert_item_into_tabs_at(
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

    pub(in crate::graph) fn insert_items_into_tabs_at(
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

    pub(in crate::graph) fn remove_item_from_tabs(
        &mut self,
        tabs: DockNodeId,
        index: usize,
    ) -> bool {
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
}
