use crate::{DockItemId, DockNodeId, DockSpaceId};

use super::DockGraph;

impl DockGraph {
    pub(in crate::graph) fn target_space_is_empty_for_open(&self, space: &DockSpaceId) -> bool {
        self.root(space).is_none() && self.floating_containers(space).is_empty()
    }

    pub(in crate::graph) fn target_space_is_empty_for_item_move(
        &self,
        source_space: &DockSpaceId,
        item: &DockItemId,
        target_space: &DockSpaceId,
    ) -> bool {
        if self.root(target_space).is_some() {
            return false;
        }
        if source_space != target_space {
            return self.floating_containers(target_space).is_empty();
        }

        let target_items = self.collect_items_in_space(target_space);
        if target_items.is_empty() {
            return true;
        }
        matches!(target_items.as_slice(), [target_item] if target_item == item)
    }

    pub(in crate::graph) fn target_space_is_empty_for_tabs_move(
        &self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
    ) -> bool {
        if self.root(target_space).is_some() {
            return false;
        }
        if source_space != target_space {
            return self.floating_containers(target_space).is_empty();
        }

        let target_items = self.collect_items_in_space(target_space);
        if target_items.is_empty() {
            return true;
        }
        let source_items = self.collect_items_in_subtree(source_tabs);
        !source_items.is_empty() && target_items == source_items
    }
}
