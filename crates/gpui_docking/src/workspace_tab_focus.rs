use crate::{DockGraph, DockItemId, DockNode, DockNodeId};
use std::collections::HashMap;

/// Workspace-local tab selection stamps.
///
/// ImGui keeps recent tab selection data in the tab bar, outside persisted dock-node settings.
/// Keep the same separation here: the graph stores the current selected item, while the workspace
/// uses this transient history to choose the next tab when the selected tab closes.
#[derive(Debug, Default)]
pub(crate) struct DockWorkspaceTabFocus {
    next_stamp: u64,
    selected_stamps_by_tabs: HashMap<DockNodeId, HashMap<DockItemId, u64>>,
}

impl DockWorkspaceTabFocus {
    pub(crate) fn refresh_selected(&mut self, tabs: DockNodeId, item: &DockItemId) {
        let stamp = self.next_stamp;
        self.next_stamp = self.next_stamp.saturating_add(1);
        self.selected_stamps_by_tabs
            .entry(tabs)
            .or_default()
            .insert(item.clone(), stamp);
    }

    pub(crate) fn preferred_after_close(
        &self,
        graph: &DockGraph,
        tabs: DockNodeId,
        closing_item: &DockItemId,
    ) -> Option<DockItemId> {
        let DockNode::Tabs { items, .. } = graph.node(tabs)? else {
            return None;
        };
        self.selected_stamps_by_tabs
            .get(&tabs)?
            .iter()
            .filter(|(candidate, _)| *candidate != closing_item && items.contains(candidate))
            .max_by_key(|(_, stamp)| *stamp)
            .map(|(item, _)| item.clone())
    }

    pub(crate) fn prune_to_graph(&mut self, graph: &DockGraph) {
        self.selected_stamps_by_tabs.retain(|tabs, stamps| {
            let Some(DockNode::Tabs { items, .. }) = graph.node(*tabs) else {
                return false;
            };
            stamps.retain(|candidate, _| items.contains(candidate));
            !stamps.is_empty()
        });
    }

    pub(crate) fn clear(&mut self) {
        self.next_stamp = 0;
        self.selected_stamps_by_tabs.clear();
    }
}
