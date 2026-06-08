use crate::{DockItemId, DockNodeId, DockOp, DockOpApplyError, DockSpaceId, SplitFractionsUpdate};
use open_gpui::{Bounds, Pixels, Point, Size, point, px, size};
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;

/// Axis used by split dock nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitAxis {
    /// Children are laid out left to right.
    Horizontal,
    /// Children are laid out top to bottom.
    Vertical,
}

/// Drop zone used when docking into an existing node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DropZone {
    /// Merge into the target tabs node.
    Center,
    /// Split to the target's left side.
    Left,
    /// Split to the target's right side.
    Right,
    /// Split to the target's top side.
    Top,
    /// Split to the target's bottom side.
    Bottom,
}

/// Runtime node in a dock graph.
#[derive(Debug, Clone, PartialEq)]
pub enum DockNode {
    /// N-ary split container.
    Split {
        /// Split axis.
        axis: SplitAxis,
        /// Child nodes.
        children: Vec<DockNodeId>,
        /// Normalized child fractions.
        fractions: Vec<f32>,
    },
    /// Tab stack containing dock item ids.
    Tabs {
        /// Items in tab order.
        items: Vec<DockItemId>,
        /// Active item index.
        active: usize,
    },
    /// In-window floating container.
    Floating {
        /// Child root rendered inside the floating container.
        child: DockNodeId,
    },
}

/// A pure decision describing how an edge dock will mutate the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDockDecision {
    /// Insert a new child into an existing same-axis split.
    InsertIntoSplit {
        /// The split container receiving the new child.
        split: DockNodeId,
        /// Existing child whose share will be split.
        anchor_index: usize,
        /// Position where the new child will be inserted.
        insert_index: usize,
    },
    /// Wrap the target in a new split.
    WrapNewSplit,
}

/// In-window floating container metadata.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockFloatingContainer {
    /// Floating node id.
    pub node: DockNodeId,
    /// Container bounds relative to the dock host.
    pub bounds: Bounds<Pixels>,
}

/// Retained docking graph for one or more logical dock spaces.
#[derive(Debug, Default)]
pub struct DockGraph {
    nodes: SlotMap<DockNodeId, DockNode>,
    roots: HashMap<DockSpaceId, DockNodeId>,
    floatings: HashMap<DockSpaceId, Vec<DockFloatingContainer>>,
}

impl DockGraph {
    /// Creates an empty dock graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a node and returns its runtime id.
    pub fn insert_node(&mut self, node: DockNode) -> DockNodeId {
        self.nodes.insert(node)
    }

    /// Returns a node by id.
    pub fn node(&self, id: DockNodeId) -> Option<&DockNode> {
        self.nodes.get(id)
    }

    /// Sets the root node for a dock space.
    pub fn set_root(&mut self, space: DockSpaceId, root: DockNodeId) {
        self.roots.insert(space, root);
    }

    /// Returns the root node for a dock space.
    pub fn root(&self, space: &DockSpaceId) -> Option<DockNodeId> {
        self.roots.get(space).copied()
    }

    /// Removes and returns the root node for a dock space.
    pub fn remove_root(&mut self, space: &DockSpaceId) -> Option<DockNodeId> {
        self.roots.remove(space)
    }

    /// Returns all logical dock spaces known to the graph.
    pub fn spaces(&self) -> Vec<DockSpaceId> {
        let mut spaces: Vec<DockSpaceId> = self
            .roots
            .keys()
            .chain(self.floatings.keys())
            .cloned()
            .collect();
        spaces.sort();
        spaces.dedup();
        spaces
    }

    /// Returns floating containers for a dock space.
    pub fn floating_containers(&self, space: &DockSpaceId) -> &[DockFloatingContainer] {
        self.floatings
            .get(space)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Returns mutable floating containers for a dock space.
    pub fn floating_containers_mut(
        &mut self,
        space: DockSpaceId,
    ) -> &mut Vec<DockFloatingContainer> {
        self.floatings.entry(space).or_default()
    }

    /// Applies an operation with validation for the common error-prone cases.
    pub fn apply_op_checked(&mut self, op: &DockOp) -> Result<bool, DockOpApplyError> {
        match op {
            DockOp::SetActiveTab { tabs, active } => {
                let Some(node) = self.node(*tabs) else {
                    return Err(DockOpApplyError::TabsNodeNotFound { tabs: *tabs });
                };
                let DockNode::Tabs { items, .. } = node else {
                    return Err(DockOpApplyError::NodeIsNotTabs { node: *tabs });
                };
                if *active >= items.len() {
                    return Err(DockOpApplyError::ActiveOutOfBounds {
                        tabs: *tabs,
                        active: *active,
                        len: items.len(),
                    });
                }
                Ok(self.set_active_tab(*tabs, *active))
            }
            DockOp::CloseItem { space, item } => {
                if self.close_item(space, item.clone()) {
                    Ok(true)
                } else {
                    Err(DockOpApplyError::ItemNotFound {
                        space: space.clone(),
                        item: item.clone(),
                    })
                }
            }
            DockOp::MoveItem {
                source_space,
                item,
                target_space,
                target_tabs,
                zone,
                ..
            } => {
                self.validate_move_item(source_space, item, target_space, *target_tabs, *zone)?;
                Ok(self.apply_op(op))
            }
            DockOp::MoveItemToEmptyDockSpace {
                source_space,
                item,
                target_space,
            } => {
                if self.root(target_space).is_some() {
                    return Err(DockOpApplyError::TargetSpaceNotEmpty {
                        space: target_space.clone(),
                    });
                }
                if self.find_item_in_space(source_space, item).is_none() {
                    return Err(DockOpApplyError::ItemNotFound {
                        space: source_space.clone(),
                        item: item.clone(),
                    });
                }
                Ok(self.apply_op(op))
            }
            DockOp::MoveTabsToEmptyDockSpace {
                source_space,
                source_tabs,
                target_space,
            } => {
                if self.root(target_space).is_some() {
                    return Err(DockOpApplyError::TargetSpaceNotEmpty {
                        space: target_space.clone(),
                    });
                }
                let Some(node) = self.node(*source_tabs) else {
                    return Err(DockOpApplyError::TabsNodeNotFound { tabs: *source_tabs });
                };
                let DockNode::Tabs { items, .. } = node else {
                    return Err(DockOpApplyError::NodeIsNotTabs { node: *source_tabs });
                };
                if items.is_empty() {
                    return Err(DockOpApplyError::OperationFailed);
                }
                if self
                    .root_for_node_in_space(source_space, *source_tabs)
                    .is_none()
                {
                    return Err(DockOpApplyError::SourceNodeNotInSpace {
                        space: source_space.clone(),
                        node: *source_tabs,
                    });
                }
                Ok(self.apply_op(op))
            }
            DockOp::SetSplitFractions { split, fractions } => {
                self.validate_split_fractions(*split, fractions)?;
                Ok(self.apply_op(op))
            }
            DockOp::SetSplitFractionsMany { updates } => {
                for update in updates {
                    self.validate_split_fractions(update.split, &update.fractions)?;
                }
                Ok(self.apply_op(op))
            }
            DockOp::SetSplitFractionTwo {
                split,
                first_fraction,
            } => {
                self.validate_split_fractions(*split, &[*first_fraction, 1.0 - *first_fraction])?;
                Ok(self.apply_op(op))
            }
            _ => {
                let ok = self.apply_op(op);
                if ok {
                    Ok(true)
                } else {
                    Err(DockOpApplyError::OperationFailed)
                }
            }
        }
    }

    /// Applies an operation and returns whether it changed or preserved a valid graph state.
    pub(crate) fn apply_op(&mut self, op: &DockOp) -> bool {
        match op {
            DockOp::SetActiveTab { tabs, active } => self.set_active_tab(*tabs, *active),
            DockOp::CloseItem { space, item } => self.close_item(space, item.clone()),
            DockOp::MoveItem {
                source_space,
                item,
                target_space,
                target_tabs,
                zone,
                insert_index,
            } => self.move_item_between_spaces(
                source_space,
                item.clone(),
                target_space,
                *target_tabs,
                *zone,
                *insert_index,
            ),
            DockOp::MoveItemToEmptyDockSpace {
                source_space,
                item,
                target_space,
            } => {
                if self.root(target_space).is_some() {
                    return false;
                }
                self.move_item_to_empty_space(source_space, item.clone(), target_space)
            }
            DockOp::MoveTabs {
                source_space,
                source_tabs,
                target_space,
                target_tabs,
                zone,
                insert_index,
            } => self.move_tabs_between_spaces(
                source_space,
                *source_tabs,
                target_space,
                *target_tabs,
                *zone,
                *insert_index,
            ),
            DockOp::MoveTabsToEmptyDockSpace {
                source_space,
                source_tabs,
                target_space,
            } => {
                if self.root(target_space).is_some() {
                    return false;
                }
                self.move_tabs_to_empty_space(source_space, *source_tabs, target_space)
            }
            DockOp::FloatItemInWindow {
                source_space,
                item,
                target_space,
                bounds,
            } => self.float_item_in_window(source_space, item.clone(), target_space, *bounds),
            DockOp::FloatTabsInWindow {
                source_space,
                source_tabs,
                target_space,
                bounds,
            } => self.float_tabs_in_window(source_space, *source_tabs, target_space, *bounds),
            DockOp::SetFloatingBounds {
                space,
                floating,
                bounds,
            } => self.set_floating_bounds(space, *floating, *bounds),
            DockOp::RaiseFloating { space, floating } => self.raise_floating(space, *floating),
            DockOp::MergeFloatingInto {
                space,
                floating,
                target_tabs,
            } => self.merge_floating_into(space, *floating, *target_tabs),
            DockOp::SetSplitFractions { split, fractions } => {
                self.update_split_fractions(*split, fractions.clone())
            }
            DockOp::SetSplitFractionsMany { updates } => {
                let mut changed = false;
                for update in updates {
                    changed |= self.update_split_fractions(update.split, update.fractions.clone());
                }
                changed
            }
            DockOp::SetSplitFractionTwo {
                split,
                first_fraction,
            } => self.update_split_two(*split, *first_fraction),
        }
    }

    /// Computes layout bounds for a subtree into `out`.
    pub fn compute_layout(
        &self,
        root: DockNodeId,
        bounds: Bounds<Pixels>,
        out: &mut HashMap<DockNodeId, Bounds<Pixels>>,
    ) {
        let Some(node) = self.nodes.get(root) else {
            return;
        };

        out.insert(root, bounds);
        match node {
            DockNode::Tabs { .. } => {}
            DockNode::Floating { child } => {
                self.compute_layout(*child, bounds, out);
            }
            DockNode::Split {
                axis,
                children,
                fractions,
            } => {
                if children.is_empty() {
                    return;
                }

                let shares = cleaned_layout_shares(children.len(), fractions);
                let mut cursor = 0.0_f32;
                let width = f32::from(bounds.size.width);
                let height = f32::from(bounds.size.height);
                let x = f32::from(bounds.origin.x);
                let y = f32::from(bounds.origin.y);

                for (child, share) in children.iter().copied().zip(shares) {
                    let (child_bounds, next_cursor) = match axis {
                        SplitAxis::Horizontal => {
                            let child_width = width * share;
                            (
                                Bounds::new(
                                    point(px(x + cursor), bounds.origin.y),
                                    size(px(child_width), bounds.size.height),
                                ),
                                cursor + child_width,
                            )
                        }
                        SplitAxis::Vertical => {
                            let child_height = height * share;
                            (
                                Bounds::new(
                                    point(bounds.origin.x, px(y + cursor)),
                                    size(bounds.size.width, px(child_height)),
                                ),
                                cursor + child_height,
                            )
                        }
                    };

                    cursor = next_cursor;
                    self.compute_layout(child, child_bounds, out);
                }
            }
        }
    }

    /// Returns all dock items reachable from a dock space.
    pub fn collect_items_in_space(&self, space: &DockSpaceId) -> Vec<DockItemId> {
        let mut out = Vec::new();
        if let Some(root) = self.root(space) {
            self.collect_items_in_subtree_into(root, &mut out);
        }
        if let Some(floatings) = self.floatings.get(space) {
            for floating in floatings {
                self.collect_items_in_subtree_into(floating.node, &mut out);
            }
        }
        out
    }

    /// Returns all dock items reachable from a subtree.
    pub fn collect_items_in_subtree(&self, root: DockNodeId) -> Vec<DockItemId> {
        let mut out = Vec::new();
        self.collect_items_in_subtree_into(root, &mut out);
        out
    }

    /// Finds an item in a dock space and returns its tabs node and tab index.
    pub fn find_item_in_space(
        &self,
        space: &DockSpaceId,
        item: &DockItemId,
    ) -> Option<(DockNodeId, usize)> {
        if let Some(root) = self.root(space)
            && let Some(found) = self.find_item_in_subtree(root, item)
        {
            return Some(found);
        }

        self.floatings.get(space).and_then(|floatings| {
            floatings
                .iter()
                .find_map(|floating| self.find_item_in_subtree(floating.node, item))
        })
    }

    /// Returns the root that contains a node within a dock space forest.
    pub fn root_for_node_in_space(
        &self,
        space: &DockSpaceId,
        target: DockNodeId,
    ) -> Option<DockNodeId> {
        if let Some(root) = self.root(space)
            && self.subtree_contains(root, target)
        {
            return Some(root);
        }

        self.floatings.get(space).and_then(|floatings| {
            floatings.iter().find_map(|floating| {
                self.subtree_contains(floating.node, target)
                    .then_some(floating.node)
            })
        })
    }

    /// Decides whether an edge dock will insert into an existing same-axis split.
    pub fn edge_dock_decision(
        &self,
        space: &DockSpaceId,
        target: DockNodeId,
        zone: DropZone,
    ) -> Option<EdgeDockDecision> {
        if zone == DropZone::Center {
            return None;
        }

        let axis = zone.axis()?;
        let index = self.build_parent_index(space);
        if !index.root_for.contains_key(&target) {
            return None;
        }

        if let Some(DockNode::Split {
            axis: split_axis,
            children,
            fractions,
        }) = self.nodes.get(target)
            && *split_axis == axis
            && !children.is_empty()
            && children.len() == fractions.len()
        {
            let len = children.len();
            let (anchor_index, insert_index) = match zone {
                DropZone::Left | DropZone::Top => (0, 0),
                DropZone::Right | DropZone::Bottom => {
                    let last = len.saturating_sub(1);
                    (last, last.saturating_add(1))
                }
                DropZone::Center => unreachable!(),
            };
            return Some(EdgeDockDecision::InsertIntoSplit {
                split: target,
                anchor_index,
                insert_index,
            });
        }

        let mut cur = target;
        while let Some(parent) = index.parent.get(&cur).copied() {
            let Some(DockNode::Split {
                axis: split_axis,
                children,
                fractions,
            }) = self.nodes.get(parent)
            else {
                cur = parent;
                continue;
            };

            if *split_axis == axis && !children.is_empty() && children.len() == fractions.len() {
                let Some(anchor_index) = index.split_child_index.get(&cur).copied() else {
                    break;
                };
                let insert_index = match zone {
                    DropZone::Left | DropZone::Top => anchor_index,
                    DropZone::Right | DropZone::Bottom => anchor_index.saturating_add(1),
                    DropZone::Center => unreachable!(),
                };
                return Some(EdgeDockDecision::InsertIntoSplit {
                    split: parent,
                    anchor_index,
                    insert_index,
                });
            }

            cur = parent;
        }

        Some(EdgeDockDecision::WrapNewSplit)
    }

    fn validate_move_item(
        &self,
        source_space: &DockSpaceId,
        item: &DockItemId,
        target_space: &DockSpaceId,
        target_tabs: DockNodeId,
        zone: DropZone,
    ) -> Result<(), DockOpApplyError> {
        if self.find_item_in_space(source_space, item).is_none() {
            return Err(DockOpApplyError::ItemNotFound {
                space: source_space.clone(),
                item: item.clone(),
            });
        }
        if self
            .root_for_node_in_space(target_space, target_tabs)
            .is_none()
        {
            return Err(DockOpApplyError::TargetNodeNotInSpace {
                space: target_space.clone(),
                target: target_tabs,
            });
        }
        if zone == DropZone::Center {
            match self.node(target_tabs) {
                Some(DockNode::Tabs { .. }) => {}
                Some(_) => return Err(DockOpApplyError::NodeIsNotTabs { node: target_tabs }),
                None => return Err(DockOpApplyError::TabsNodeNotFound { tabs: target_tabs }),
            }
        }
        Ok(())
    }

    fn validate_split_fractions(
        &self,
        split: DockNodeId,
        fractions: &[f32],
    ) -> Result<(), DockOpApplyError> {
        let Some(node) = self.node(split) else {
            return Err(DockOpApplyError::SplitNodeNotFound { split });
        };
        let DockNode::Split { children, .. } = node else {
            return Err(DockOpApplyError::NodeIsNotSplit { node: split });
        };
        if children.len() < 2 {
            return Err(DockOpApplyError::SplitTooFewChildren {
                split,
                children_len: children.len(),
            });
        }
        if fractions.len() != children.len() {
            return Err(DockOpApplyError::SplitFractionsLenMismatch {
                split,
                children_len: children.len(),
                fractions_len: fractions.len(),
            });
        }
        for (index, fraction) in fractions.iter().copied().enumerate() {
            if !fraction.is_finite() || fraction < 0.0 {
                return Err(DockOpApplyError::SplitFractionInvalid { split, index });
            }
        }
        Ok(())
    }

    /// Simplifies every tree in one dock space into canonical form.
    pub fn simplify_space(&mut self, space: &DockSpaceId) {
        if let Some(root) = self.root(space) {
            match self.simplify_subtree(root) {
                Some(root) => self.set_root(space.clone(), root),
                None => {
                    self.remove_root(space);
                }
            }
        }

        let Some(mut floatings) = self.floatings.remove(space) else {
            return;
        };

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

    fn close_item(&mut self, space: &DockSpaceId, item: DockItemId) -> bool {
        let Some((tabs, index)) = self.find_item_in_space(space, &item) else {
            return false;
        };
        if !self.remove_item_from_tabs(tabs, index) {
            return false;
        }
        self.simplify_space(space);
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn move_item_between_spaces(
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
            return true;
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

        let Some(axis) = zone.axis() else {
            return false;
        };
        let new_tabs = self.insert_node(DockNode::Tabs {
            items: vec![item],
            active: 0,
        });

        if self.insert_edge_child_prefer_same_axis_split(
            target_space,
            target_tabs,
            axis,
            zone,
            new_tabs,
        ) {
            self.simplify_space(source_space);
            if source_space != target_space {
                self.simplify_space(target_space);
            }
            return true;
        }

        let (first, second) = ordered_edge_children(zone, new_tabs, target_tabs);
        let split = self.insert_node(DockNode::Split {
            axis,
            children: vec![first, second],
            fractions: vec![0.5, 0.5],
        });
        self.replace_node_in_space_tree(target_space, target_tabs, split);
        self.simplify_space(source_space);
        if source_space != target_space {
            self.simplify_space(target_space);
        }
        true
    }

    fn move_item_to_empty_space(
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
        self.set_root(target_space.clone(), tabs);
        self.simplify_space(source_space);
        self.simplify_space(target_space);
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn move_tabs_between_spaces(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
        target_tabs: DockNodeId,
        zone: DropZone,
        insert_index: Option<usize>,
    ) -> bool {
        if source_space == target_space && source_tabs == target_tabs {
            return zone == DropZone::Center;
        }
        if self
            .root_for_node_in_space(source_space, source_tabs)
            .is_none()
            || self
                .root_for_node_in_space(target_space, target_tabs)
                .is_none()
        {
            return false;
        }

        let (items, active) = match self.nodes.get(source_tabs) {
            Some(DockNode::Tabs { items, active }) if !items.is_empty() => {
                (items.clone(), (*active).min(items.len().saturating_sub(1)))
            }
            _ => return false,
        };

        if zone == DropZone::Center
            && !matches!(self.nodes.get(target_tabs), Some(DockNode::Tabs { .. }))
        {
            return false;
        }

        if let Some(DockNode::Tabs { items, active }) = self.nodes.get_mut(source_tabs) {
            items.clear();
            *active = 0;
        }
        if self.root(source_space) == Some(source_tabs) {
            self.remove_root(source_space);
        }
        self.simplify_space(source_space);

        if zone == DropZone::Center {
            let ok = self.insert_items_into_tabs_at(target_tabs, &items, insert_index, active);
            self.simplify_space(target_space);
            return ok;
        }

        let Some(axis) = zone.axis() else {
            return false;
        };
        let new_tabs = self.insert_node(DockNode::Tabs { items, active });

        if self.insert_edge_child_prefer_same_axis_split(
            target_space,
            target_tabs,
            axis,
            zone,
            new_tabs,
        ) {
            self.simplify_space(target_space);
            return true;
        }

        let (first, second) = ordered_edge_children(zone, new_tabs, target_tabs);
        let split = self.insert_node(DockNode::Split {
            axis,
            children: vec![first, second],
            fractions: vec![0.5, 0.5],
        });
        self.replace_node_in_space_tree(target_space, target_tabs, split);
        self.simplify_space(target_space);
        true
    }

    fn move_tabs_to_empty_space(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
    ) -> bool {
        if self
            .root_for_node_in_space(source_space, source_tabs)
            .is_none()
        {
            return false;
        }

        let (items, active) = match self.nodes.get(source_tabs) {
            Some(DockNode::Tabs { items, active }) if !items.is_empty() => {
                (items.clone(), (*active).min(items.len().saturating_sub(1)))
            }
            _ => return false,
        };

        if let Some(DockNode::Tabs { items, active }) = self.nodes.get_mut(source_tabs) {
            items.clear();
            *active = 0;
        }
        if self.root(source_space) == Some(source_tabs) {
            self.remove_root(source_space);
        }
        let tabs = self.insert_node(DockNode::Tabs { items, active });
        self.set_root(target_space.clone(), tabs);
        self.simplify_space(source_space);
        self.simplify_space(target_space);
        true
    }

    fn float_item_in_window(
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

    fn float_tabs_in_window(
        &mut self,
        source_space: &DockSpaceId,
        source_tabs: DockNodeId,
        target_space: &DockSpaceId,
        bounds: Bounds<Pixels>,
    ) -> bool {
        if self
            .root_for_node_in_space(source_space, source_tabs)
            .is_none()
        {
            return false;
        }

        let (items, active) = match self.nodes.get(source_tabs) {
            Some(DockNode::Tabs { items, active }) if !items.is_empty() => {
                (items.clone(), (*active).min(items.len().saturating_sub(1)))
            }
            _ => return false,
        };

        if let Some(DockNode::Tabs { items, active }) = self.nodes.get_mut(source_tabs) {
            items.clear();
            *active = 0;
        }
        if self.root(source_space) == Some(source_tabs) {
            self.remove_root(source_space);
        }
        self.simplify_space(source_space);

        let tabs = self.insert_node(DockNode::Tabs { items, active });
        let floating = self.insert_node(DockNode::Floating { child: tabs });
        self.floating_containers_mut(target_space.clone())
            .push(DockFloatingContainer {
                node: floating,
                bounds,
            });
        self.simplify_space(target_space);
        true
    }

    fn set_floating_bounds(
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
        container.bounds = bounds;
        true
    }

    fn raise_floating(&mut self, space: &DockSpaceId, floating: DockNodeId) -> bool {
        let Some(floatings) = self.floatings.get_mut(space) else {
            return false;
        };
        let Some(index) = floatings.iter().position(|entry| entry.node == floating) else {
            return false;
        };
        if index + 1 == floatings.len() {
            return true;
        }
        let entry = floatings.remove(index);
        floatings.push(entry);
        true
    }

    fn merge_floating_into(
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

    fn simplify_subtree(&mut self, node: DockNodeId) -> Option<DockNodeId> {
        let node_value = self.nodes.get(node)?.clone();
        match node_value {
            DockNode::Tabs { items, mut active } => {
                if items.is_empty() {
                    return None;
                }
                if active >= items.len() {
                    active = items.len().saturating_sub(1);
                }
                if let Some(DockNode::Tabs {
                    items: current_items,
                    active: current_active,
                }) = self.nodes.get_mut(node)
                {
                    *current_items = items;
                    *current_active = active;
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

                normalize_shares(&mut next_fractions);

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
                normalize_shares(&mut shares);
                for (&grand_child, &share) in grand_children.iter().zip(shares.iter()) {
                    out_children.push(grand_child);
                    out_fractions.push(parent_share * share);
                }
            }

            *children = out_children;
            *fractions = out_fractions;
        }
    }

    fn insert_item_into_tabs_at(
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

    fn insert_items_into_tabs_at(
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

    fn remove_item_from_tabs(&mut self, tabs: DockNodeId, index: usize) -> bool {
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

    fn insert_edge_child_prefer_same_axis_split(
        &mut self,
        space: &DockSpaceId,
        target: DockNodeId,
        axis: SplitAxis,
        zone: DropZone,
        new_child: DockNodeId,
    ) -> bool {
        let Some(EdgeDockDecision::InsertIntoSplit {
            split,
            anchor_index,
            insert_index,
        }) = self.edge_dock_decision(space, target, zone)
        else {
            return false;
        };

        let Some(DockNode::Split {
            axis: split_axis,
            children,
            fractions,
        }) = self.nodes.get_mut(split)
        else {
            return false;
        };
        if *split_axis != axis || children.len() != fractions.len() || children.is_empty() {
            return false;
        }
        split_share_and_insert(children, fractions, anchor_index, insert_index, new_child);
        true
    }

    fn replace_node_in_space_tree(
        &mut self,
        space: &DockSpaceId,
        old: DockNodeId,
        new: DockNodeId,
    ) {
        if self.root(space) == Some(old) {
            self.set_root(space.clone(), new);
            return;
        }
        if let Some(floatings) = self.floatings.get_mut(space) {
            for floating in floatings {
                if floating.node == old {
                    floating.node = new;
                    return;
                }
            }
        }

        let roots: Vec<DockNodeId> = self
            .root(space)
            .into_iter()
            .chain(
                self.floatings
                    .get(space)
                    .into_iter()
                    .flatten()
                    .map(|floating| floating.node),
            )
            .collect();
        for root in roots {
            if let Some(parent) = self.find_parent_in_subtree(root, old) {
                self.replace_child_in_node(parent, old, new);
                return;
            }
        }
    }

    fn replace_child_in_node(
        &mut self,
        node: DockNodeId,
        old: DockNodeId,
        new: DockNodeId,
    ) -> bool {
        let Some(node) = self.nodes.get_mut(node) else {
            return false;
        };
        match node {
            DockNode::Split { children, .. } => {
                let Some(index) = children.iter().position(|child| *child == old) else {
                    return false;
                };
                children[index] = new;
                true
            }
            DockNode::Floating { child } => {
                if *child != old {
                    return false;
                }
                *child = new;
                true
            }
            DockNode::Tabs { .. } => false,
        }
    }

    fn find_parent_in_subtree(&self, root: DockNodeId, target: DockNodeId) -> Option<DockNodeId> {
        match self.nodes.get(root)? {
            DockNode::Tabs { .. } => None,
            DockNode::Floating { child } => {
                if *child == target {
                    Some(root)
                } else {
                    self.find_parent_in_subtree(*child, target)
                }
            }
            DockNode::Split { children, .. } => {
                if children.contains(&target) {
                    return Some(root);
                }
                children
                    .iter()
                    .copied()
                    .find_map(|child| self.find_parent_in_subtree(child, target))
            }
        }
    }

    fn find_item_in_subtree(
        &self,
        root: DockNodeId,
        item: &DockItemId,
    ) -> Option<(DockNodeId, usize)> {
        match self.nodes.get(root)? {
            DockNode::Tabs { items, .. } => items
                .iter()
                .position(|candidate| candidate == item)
                .map(|index| (root, index)),
            DockNode::Floating { child } => self.find_item_in_subtree(*child, item),
            DockNode::Split { children, .. } => children
                .iter()
                .copied()
                .find_map(|child| self.find_item_in_subtree(child, item)),
        }
    }

    fn collect_items_in_subtree_into(&self, root: DockNodeId, out: &mut Vec<DockItemId>) {
        let Some(node) = self.nodes.get(root) else {
            return;
        };
        match node {
            DockNode::Tabs { items, .. } => out.extend(items.iter().cloned()),
            DockNode::Floating { child } => self.collect_items_in_subtree_into(*child, out),
            DockNode::Split { children, .. } => {
                for child in children {
                    self.collect_items_in_subtree_into(*child, out);
                }
            }
        }
    }

    fn subtree_contains(&self, root: DockNodeId, target: DockNodeId) -> bool {
        if root == target {
            return true;
        }
        let Some(node) = self.nodes.get(root) else {
            return false;
        };
        match node {
            DockNode::Tabs { .. } => false,
            DockNode::Floating { child } => self.subtree_contains(*child, target),
            DockNode::Split { children, .. } => children
                .iter()
                .copied()
                .any(|child| self.subtree_contains(child, target)),
        }
    }

    fn build_parent_index(&self, space: &DockSpaceId) -> DockParentIndex {
        let mut index = DockParentIndex::default();
        if let Some(root) = self.root(space) {
            self.index_subtree(root, root, &mut index);
        }
        if let Some(floatings) = self.floatings.get(space) {
            for floating in floatings {
                self.index_subtree(floating.node, floating.node, &mut index);
            }
        }
        index
    }

    fn index_subtree(&self, root: DockNodeId, node: DockNodeId, index: &mut DockParentIndex) {
        if index.root_for.contains_key(&node) {
            return;
        }
        index.root_for.insert(node, root);
        let Some(current) = self.nodes.get(node) else {
            return;
        };
        match current {
            DockNode::Tabs { .. } => {}
            DockNode::Floating { child } => {
                index.parent.insert(*child, node);
                self.index_subtree(root, *child, index);
            }
            DockNode::Split { children, .. } => {
                for (child_index, child) in children.iter().copied().enumerate() {
                    index.parent.insert(child, node);
                    index.split_child_index.insert(child, child_index);
                    self.index_subtree(root, child, index);
                }
            }
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
            DockNode::Tabs { items, active } => {
                assert!(!items.is_empty(), "tabs nodes must be non-empty");
                assert!(*active < items.len(), "active tab index must be in bounds");
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

impl DropZone {
    fn axis(self) -> Option<SplitAxis> {
        match self {
            DropZone::Left | DropZone::Right => Some(SplitAxis::Horizontal),
            DropZone::Top | DropZone::Bottom => Some(SplitAxis::Vertical),
            DropZone::Center => None,
        }
    }
}

#[derive(Default)]
struct DockParentIndex {
    root_for: HashMap<DockNodeId, DockNodeId>,
    parent: HashMap<DockNodeId, DockNodeId>,
    split_child_index: HashMap<DockNodeId, usize>,
}

fn ordered_edge_children(
    zone: DropZone,
    new_child: DockNodeId,
    target: DockNodeId,
) -> (DockNodeId, DockNodeId) {
    match zone {
        DropZone::Left | DropZone::Top => (new_child, target),
        DropZone::Right | DropZone::Bottom => (target, new_child),
        DropZone::Center => unreachable!(),
    }
}

fn normalize_shares(shares: &mut Vec<f32>) {
    for share in shares.iter_mut() {
        if !share.is_finite() || *share < 0.0 {
            *share = 0.0;
        }
    }

    let sum: f32 = shares.iter().sum();
    if !sum.is_finite() || sum <= f32::EPSILON {
        let len = shares.len().max(1);
        *shares = vec![1.0 / len as f32; len];
        return;
    }

    for share in shares.iter_mut() {
        *share /= sum;
    }

    if !shares.is_empty() {
        let rest: f32 = shares.iter().take(shares.len().saturating_sub(1)).sum();
        let last = shares.len().saturating_sub(1);
        shares[last] = (1.0 - rest).clamp(0.0, 1.0);
    }
}

fn cleaned_layout_shares(len: usize, fractions: &[f32]) -> Vec<f32> {
    let mut shares: Vec<f32> = (0..len)
        .map(|index| fractions.get(index).copied().unwrap_or(1.0))
        .collect();
    normalize_shares(&mut shares);
    shares
}

fn split_share_and_insert(
    children: &mut Vec<DockNodeId>,
    fractions: &mut Vec<f32>,
    anchor_index: usize,
    insert_index: usize,
    new_child: DockNodeId,
) {
    if children.is_empty()
        || children.len() != fractions.len()
        || anchor_index >= fractions.len()
        || insert_index > fractions.len()
    {
        return;
    }

    let old = fractions[anchor_index];
    let keep = old * 0.5;
    let take = old * 0.5;
    fractions[anchor_index] = keep;
    children.insert(insert_index, new_child);
    fractions.insert(insert_index, take);
    normalize_shares(fractions);
}

/// Convenience constructor for bounds in tests and examples.
pub fn dock_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(Point::new(px(x), px(y)), Size::new(px(width), px(height)))
}

impl From<SplitFractionsUpdate> for (DockNodeId, Vec<f32>) {
    fn from(update: SplitFractionsUpdate) -> Self {
        (update.split, update.fractions)
    }
}
