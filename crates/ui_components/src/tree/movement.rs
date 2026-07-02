use super::{TreeItemDescriptor, TreeItemState};

/// Relative drop position for a Tree move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeDropPosition {
    /// Insert before the target row in the target row's parent.
    Before,
    /// Insert after the target row in the target row's parent.
    After,
    /// Insert as the last loaded child of the target row.
    Inside,
}

impl TreeDropPosition {
    /// Returns a stable position label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
            Self::Inside => "inside",
        }
    }
}

/// Resolved Tree move target metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeMoveTarget {
    target_index: usize,
    target_value: String,
    target_label: String,
    position: TreeDropPosition,
    target_parent_value: Option<String>,
    sibling_anchor_value: Option<String>,
}

impl TreeMoveTarget {
    pub(super) fn from_target(target: &TreeItemState, position: TreeDropPosition) -> Self {
        let target_parent_value = match position {
            TreeDropPosition::Inside => Some(target.value().to_owned()),
            TreeDropPosition::Before | TreeDropPosition::After => {
                target.parent_value().map(str::to_owned)
            }
        };
        let sibling_anchor_value = match position {
            TreeDropPosition::Inside => None,
            TreeDropPosition::Before | TreeDropPosition::After => Some(target.value().to_owned()),
        };

        Self {
            target_index: target.index(),
            target_value: target.value().to_owned(),
            target_label: target.label().to_owned(),
            position,
            target_parent_value,
            sibling_anchor_value,
        }
    }

    /// Returns the visible target row index.
    pub const fn target_index(&self) -> usize {
        self.target_index
    }

    /// Returns the target row value.
    pub fn target_value(&self) -> &str {
        &self.target_value
    }

    /// Returns the target row label.
    pub fn target_label(&self) -> &str {
        &self.target_label
    }

    /// Returns the relative drop position.
    pub const fn position(&self) -> TreeDropPosition {
        self.position
    }

    /// Returns the destination parent value.
    pub fn target_parent_value(&self) -> Option<&str> {
        self.target_parent_value.as_deref()
    }

    /// Returns the sibling anchor for before/after drops.
    pub fn sibling_anchor_value(&self) -> Option<&str> {
        self.sibling_anchor_value.as_deref()
    }
}

/// Controlled Tree move payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeMove {
    source_index: usize,
    value: String,
    label: String,
    source_parent_value: Option<String>,
    target: TreeMoveTarget,
}

impl TreeMove {
    pub(super) fn from_items(source: &TreeItemState, target: TreeMoveTarget) -> Self {
        Self {
            source_index: source.index(),
            value: source.value().to_owned(),
            label: source.label().to_owned(),
            source_parent_value: source.parent_value().map(str::to_owned),
            target,
        }
    }

    /// Returns the source visible row index.
    pub const fn source_index(&self) -> usize {
        self.source_index
    }

    /// Returns the moved item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the moved item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the source parent value.
    pub fn source_parent_value(&self) -> Option<&str> {
        self.source_parent_value.as_deref()
    }

    /// Returns the resolved move target.
    pub const fn target(&self) -> &TreeMoveTarget {
        &self.target
    }

    /// Returns the destination parent value.
    pub fn target_parent_value(&self) -> Option<&str> {
        self.target.target_parent_value()
    }

    /// Returns the target sibling anchor for before/after drops.
    pub fn sibling_anchor_value(&self) -> Option<&str> {
        self.target.sibling_anchor_value()
    }

    /// Returns the relative drop position.
    pub const fn position(&self) -> TreeDropPosition {
        self.target.position()
    }
}

/// Applies a controlled Tree move to descriptor data.
pub fn apply_tree_move(
    items: impl IntoIterator<Item = TreeItemDescriptor>,
    tree_move: &TreeMove,
) -> Option<Vec<TreeItemDescriptor>> {
    let mut items = items.into_iter().collect::<Vec<_>>();
    let moved = remove_tree_descriptor(&mut items, tree_move.value())?;

    match tree_move.position() {
        TreeDropPosition::Inside => {
            let parent_value = tree_move.target_parent_value()?;
            let parent = find_tree_descriptor_mut(&mut items, parent_value)?;
            parent.child_descriptors_mut().push(moved);
        }
        TreeDropPosition::Before | TreeDropPosition::After => {
            let parent_value = tree_move.target_parent_value();
            let anchor_value = tree_move.sibling_anchor_value()?;
            let siblings = tree_descriptor_children_mut(&mut items, parent_value)?;
            let anchor_index = siblings
                .iter()
                .position(|item| item.value() == anchor_value)?;
            let insert_index = match tree_move.position() {
                TreeDropPosition::Before => anchor_index,
                TreeDropPosition::After => anchor_index + 1,
                TreeDropPosition::Inside => unreachable!("inside handled above"),
            };
            siblings.insert(insert_index, moved);
        }
    }

    Some(items)
}

fn remove_tree_descriptor(
    items: &mut Vec<TreeItemDescriptor>,
    value: &str,
) -> Option<TreeItemDescriptor> {
    let mut index = 0usize;
    while index < items.len() {
        if items[index].value() == value {
            return Some(items.remove(index));
        }
        if let Some(removed) = remove_tree_descriptor(items[index].child_descriptors_mut(), value) {
            return Some(removed);
        }
        index += 1;
    }

    None
}

fn tree_descriptor_children_mut<'a>(
    items: &'a mut Vec<TreeItemDescriptor>,
    parent_value: Option<&str>,
) -> Option<&'a mut Vec<TreeItemDescriptor>> {
    match parent_value {
        Some(parent_value) => find_tree_descriptor_mut(items, parent_value)
            .map(TreeItemDescriptor::child_descriptors_mut),
        None => Some(items),
    }
}

fn find_tree_descriptor_mut<'a>(
    items: &'a mut [TreeItemDescriptor],
    value: &str,
) -> Option<&'a mut TreeItemDescriptor> {
    for item in items {
        if item.value() == value {
            return Some(item);
        }
        if let Some(found) = find_tree_descriptor_mut(item.child_descriptors_mut(), value) {
            return Some(found);
        }
    }

    None
}
