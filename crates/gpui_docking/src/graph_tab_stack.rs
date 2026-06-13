use crate::{DockItemId, DockNodeId, DockSpaceId};

use super::{DockGraph, DockNode};

pub(in crate::graph) struct DetachedTabs {
    pub(in crate::graph) items: Vec<DockItemId>,
    pub(in crate::graph) selected: Option<DockItemId>,
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

        let (items, selected) = match self.nodes.get(tabs) {
            Some(DockNode::Tabs { items, selected }) if !items.is_empty() => {
                (items.clone(), sanitize_selected_item(items, selected))
            }
            _ => return None,
        };

        if let Some(DockNode::Tabs { items, selected }) = self.nodes.get_mut(tabs) {
            items.clear();
            *selected = None;
        }
        if self.root(space) == Some(tabs) {
            self.remove_root(space);
        }
        if simplify {
            self.simplify_space(space);
        }
        Some(DetachedTabs { items, selected })
    }

    pub(in crate::graph) fn insert_detached_tabs(&mut self, detached: DetachedTabs) -> DockNodeId {
        self.insert_node(DockNode::Tabs {
            items: detached.items,
            selected: detached.selected,
        })
    }

    pub(in crate::graph) fn insert_item_into_tabs_at(
        &mut self,
        tabs: DockNodeId,
        item: DockItemId,
        index: Option<usize>,
    ) -> bool {
        let Some(DockNode::Tabs { items, selected }) = self.nodes.get_mut(tabs) else {
            return false;
        };
        if items.contains(&item) {
            *selected = Some(item);
            return true;
        }

        let selected_item = item.clone();
        match index {
            Some(index) => {
                let index = index.min(items.len());
                items.insert(index, item);
            }
            None => {
                items.push(item);
            }
        }
        *selected = Some(selected_item);
        true
    }

    pub(in crate::graph) fn insert_items_into_tabs_at(
        &mut self,
        tabs: DockNodeId,
        next_items: &[DockItemId],
        index: Option<usize>,
        selected_in_group: Option<&DockItemId>,
    ) -> bool {
        let Some(DockNode::Tabs { items, selected }) = self.nodes.get_mut(tabs) else {
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
        if let Some(selected_item) = selected_in_group {
            *selected = Some(selected_item.clone());
        }
        if items.is_empty() {
            *selected = None;
        } else if selected
            .as_ref()
            .is_none_or(|candidate| !items.contains(candidate))
        {
            *selected = items.first().cloned();
        }
        true
    }

    pub(in crate::graph) fn remove_item_from_tabs(
        &mut self,
        tabs: DockNodeId,
        index: usize,
    ) -> bool {
        let Some(DockNode::Tabs { items, selected }) = self.nodes.get_mut(tabs) else {
            return false;
        };
        if index >= items.len() {
            return false;
        }

        let removed = items.remove(index);
        if items.is_empty() {
            *selected = None;
        } else if selected.as_ref() == Some(&removed)
            || selected
                .as_ref()
                .is_none_or(|candidate| !items.contains(candidate))
        {
            let next_index = index.min(items.len().saturating_sub(1));
            *selected = items.get(next_index).cloned();
        }
        true
    }
}

pub(in crate::graph) fn sanitize_selected_item(
    items: &[DockItemId],
    selected: &Option<DockItemId>,
) -> Option<DockItemId> {
    selected
        .as_ref()
        .filter(|candidate| items.contains(candidate))
        .cloned()
        .or_else(|| items.first().cloned())
}
