use crate::{DockGraphDropTarget, DockItemId, DockNodeId, DockSpaceId};
use open_gpui::{Bounds, Pixels};

use super::{DockFloatingContainer, DockGraph, DockNode};

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
            items: vec![item.clone()],
            selected: Some(item),
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

    pub(in crate::graph) fn move_floating_between_spaces(
        &mut self,
        source_space: &DockSpaceId,
        floating: DockNodeId,
        target_space: &DockSpaceId,
        target: DockGraphDropTarget,
    ) -> bool {
        if self.floating_container(source_space, floating).is_none() {
            return false;
        }
        if let Some(target_node) = target.existing_node()
            && self
                .root_for_node_in_space(target_space, target_node)
                .is_none()
        {
            return false;
        }
        if let Some(target_node) = target.existing_node()
            && source_space == target_space
            && self.subtree_contains(floating, target_node)
        {
            return false;
        }

        match target {
            DockGraphDropTarget::Center { tabs } => {
                let Some(child) = self.nodes.get(floating).and_then(|node| match node {
                    DockNode::Floating { child } => Some(*child),
                    _ => None,
                }) else {
                    return false;
                };
                if self.is_visible_split_payload(child) {
                    return false;
                }
                self.merge_floating_subtree_into_tabs(
                    source_space,
                    floating,
                    target_space,
                    tabs,
                    None,
                )
            }
            DockGraphDropTarget::TabBar { tabs, insert_index } => {
                let Some(child) = self.nodes.get(floating).and_then(|node| match node {
                    DockNode::Floating { child } => Some(*child),
                    _ => None,
                }) else {
                    return false;
                };
                if !matches!(self.nodes.get(child), Some(DockNode::Tabs { .. })) {
                    return false;
                }
                self.merge_floating_subtree_into_tabs(
                    source_space,
                    floating,
                    target_space,
                    tabs,
                    Some(insert_index),
                )
            }
            DockGraphDropTarget::Edge { plan } => {
                let Some(child) = self.take_floating_child_from_space(source_space, floating)
                else {
                    return false;
                };
                if !self.apply_edge_dock_plan(target_space, plan, child) {
                    return false;
                }
                self.simplify_space(source_space);
                if source_space != target_space {
                    self.simplify_space(target_space);
                }
                true
            }
            DockGraphDropTarget::EmptySpace => {
                self.move_floating_to_empty_space(source_space, floating, target_space)
            }
        }
    }

    pub(in crate::graph) fn move_floating_to_empty_space(
        &mut self,
        source_space: &DockSpaceId,
        floating: DockNodeId,
        target_space: &DockSpaceId,
    ) -> bool {
        if !self.target_space_is_empty_for_floating_move(source_space, floating, target_space) {
            return false;
        }
        let Some(child) = self.take_floating_child_from_space(source_space, floating) else {
            return false;
        };
        self.set_root_for_empty_space(target_space, child);
        self.simplify_space(source_space);
        if source_space != target_space {
            self.simplify_space(target_space);
        }
        true
    }

    pub(crate) fn merge_space_floating_forest_into(
        &mut self,
        source_space: &DockSpaceId,
        target_space: &DockSpaceId,
    ) -> bool {
        if source_space == target_space {
            return false;
        }

        let Some(mut source_floatings) = self.floatings.remove(source_space) else {
            return false;
        };
        if source_floatings.is_empty() {
            return false;
        }

        self.floatings
            .entry(target_space.clone())
            .or_default()
            .append(&mut source_floatings);
        self.simplify_space(source_space);
        self.simplify_space(target_space);
        true
    }

    fn merge_floating_subtree_into_tabs(
        &mut self,
        source_space: &DockSpaceId,
        floating: DockNodeId,
        target_space: &DockSpaceId,
        target_tabs: DockNodeId,
        insert_index: Option<usize>,
    ) -> bool {
        if !matches!(self.nodes.get(target_tabs), Some(DockNode::Tabs { .. })) {
            return false;
        }
        let Some(child) = self.nodes.get(floating).and_then(|node| match node {
            DockNode::Floating { child } => Some(*child),
            _ => None,
        }) else {
            return false;
        };

        let source_items = self.collect_items_in_subtree(child);
        if source_items.is_empty() {
            return false;
        }
        let source_selected = self.selected_item_in_subtree(child);
        if self
            .take_floating_child_from_space(source_space, floating)
            .is_none()
        {
            return false;
        }

        if !self.insert_items_into_tabs_at(
            target_tabs,
            &source_items,
            insert_index,
            source_selected.as_ref(),
        ) {
            return false;
        }
        self.remove_subtree(floating);

        self.simplify_space(source_space);
        if source_space != target_space {
            self.simplify_space(target_space);
        }
        true
    }

    fn take_floating_child_from_space(
        &mut self,
        space: &DockSpaceId,
        floating: DockNodeId,
    ) -> Option<DockNodeId> {
        let child = match self.nodes.get(floating)? {
            DockNode::Floating { child } => *child,
            _ => return None,
        };
        let floatings = self.floatings.get_mut(space)?;
        let index = floatings.iter().position(|entry| entry.node == floating)?;
        floatings.remove(index);
        Some(child)
    }
}
