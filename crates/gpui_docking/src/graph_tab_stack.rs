use crate::{DockItemId, DockNodeId, DockSpaceId};

use super::{DockGraph, DockNode, DockTabSelectionState};

pub(in crate::graph) struct DetachedTabs {
    pub(in crate::graph) items: Vec<DockItemId>,
    pub(in crate::graph) selected: Option<DockItemId>,
    pub(in crate::graph) selection_history: DockTabSelectionState,
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
                (items.clone(), selected.clone())
            }
            _ => return None,
        };
        let selection_history = self.take_tab_selection_state(tabs);

        if let Some(DockNode::Tabs { items, selected }) = self.nodes.get_mut(tabs) {
            items.clear();
            *selected = None;
        }
        if self.root(space) == Some(tabs) {
            self.remove_root(space);
        }
        if simplify {
            self.simplify_space_after_mutation(space);
        }
        Some(DetachedTabs {
            items,
            selected,
            selection_history,
        })
    }

    pub(in crate::graph) fn insert_detached_tabs(&mut self, detached: DetachedTabs) -> DockNodeId {
        let tabs = self.insert_node(DockNode::Tabs {
            items: detached.items,
            selected: detached.selected,
        });
        self.restore_tab_selection_state(tabs, detached.selection_history);
        tabs
    }

    pub(in crate::graph) fn insert_item_into_tabs_at(
        &mut self,
        tabs: DockNodeId,
        item: DockItemId,
        index: Option<usize>,
        selected_in_group: Option<&DockItemId>,
    ) -> bool {
        let next_selected = {
            let Some(DockNode::Tabs { items, selected }) = self.nodes.get_mut(tabs) else {
                return false;
            };
            if items.contains(&item) {
                *selected = Some(item.clone());
            } else {
                match index {
                    Some(index) => {
                        let index = index.min(items.len());
                        items.insert(index, item.clone());
                    }
                    None => {
                        items.push(item.clone());
                    }
                }
                *selected = Some(item.clone());
            }
            selected_in_group.cloned().unwrap_or(item)
        };
        self.record_tab_selection(tabs, &next_selected);
        true
    }

    pub(in crate::graph) fn insert_items_into_tabs_at(
        &mut self,
        tabs: DockNodeId,
        next_items: &[DockItemId],
        index: Option<usize>,
        selected_in_group: Option<&DockItemId>,
    ) -> bool {
        if next_items.is_empty() {
            return true;
        }

        let next_selected = {
            let Some(DockNode::Tabs { items, selected }) = self.nodes.get_mut(tabs) else {
                return false;
            };

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
                Some(selected_item.clone())
            } else {
                None
            }
        };
        if let Some(selected_item) = next_selected {
            self.record_tab_selection(tabs, &selected_item);
        }
        true
    }

    pub(in crate::graph) fn remove_item_from_tabs(
        &mut self,
        tabs: DockNodeId,
        index: usize,
    ) -> bool {
        let removed = {
            let Some(DockNode::Tabs { items, selected }) = self.nodes.get_mut(tabs) else {
                return false;
            };
            if index >= items.len() {
                return false;
            }

            let removed = items.remove(index);
            if items.is_empty() {
                *selected = None;
            } else if selected.as_ref() == Some(&removed) {
                *selected = items.first().cloned();
            }
            removed
        };
        if let Some(state) = self.tab_selection_history.get_mut(&tabs) {
            state.selected_stamps_by_item.remove(&removed);
            if state.selected_stamps_by_item.is_empty() {
                self.tab_selection_history.remove(&tabs);
            }
        }
        true
    }
}

pub(in crate::graph) fn selected_item(
    items: &[DockItemId],
    selected: &Option<DockItemId>,
) -> Option<DockItemId> {
    selected
        .as_ref()
        .filter(|candidate| items.contains(candidate))
        .cloned()
}
