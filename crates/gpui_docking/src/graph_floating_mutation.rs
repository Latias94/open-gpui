use crate::{DockItemId, DockNodeId, DockSpaceId};
use open_gpui::{Bounds, Pixels};

use super::{DockFloatingContainer, DockGraph, DockNode, DropZone};

impl DockGraph {
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
        let Some(detached) = self.take_tabs_from_space(source_space, source_tabs) else {
            return false;
        };
        let tabs = self.insert_detached_tabs(detached);
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
}
