use std::collections::BTreeMap;
/// Caller-owned child loading metadata for a tree item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeChildrenLoadState {
    /// Children are fully represented by the current descriptor list.
    Loaded,
    /// Children may exist, but none are currently loaded into descriptors.
    Unloaded,
    /// Children are being loaded by the caller.
    Loading {
        /// Loading status text supplied by the caller.
        message: String,
    },
    /// Child loading failed.
    Failed {
        /// Failure status text supplied by the caller.
        message: String,
    },
}

impl TreeChildrenLoadState {
    /// Creates loaded child metadata.
    pub const fn loaded() -> Self {
        Self::Loaded
    }

    /// Creates unloaded child metadata.
    pub const fn unloaded() -> Self {
        Self::Unloaded
    }

    /// Creates loading child metadata.
    pub fn loading(message: impl Into<String>) -> Self {
        Self::Loading {
            message: message.into(),
        }
    }

    /// Creates failed child metadata.
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }

    /// Returns whether the descriptor children are fully loaded.
    pub const fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded)
    }

    /// Returns whether children are not loaded yet.
    pub const fn is_unloaded(&self) -> bool {
        matches!(self, Self::Unloaded)
    }

    /// Returns whether children are currently loading.
    pub const fn is_loading(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }

    /// Returns whether child loading failed.
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Returns a stable loading-state label.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Loaded => "loaded",
            Self::Unloaded => "unloaded",
            Self::Loading { .. } => "loading",
            Self::Failed { .. } => "failed",
        }
    }

    /// Returns the loading or failure message, when present.
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Loaded | Self::Unloaded => None,
            Self::Loading { message } | Self::Failed { message } => Some(message.as_str()),
        }
    }

    pub(crate) const fn marks_branch(&self) -> bool {
        !matches!(self, Self::Loaded)
    }
}

impl Default for TreeChildrenLoadState {
    fn default() -> Self {
        Self::Loaded
    }
}

/// Pure descriptor for one tree item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeItemDescriptor {
    value: String,
    label: String,
    children: Vec<TreeItemDescriptor>,
    children_load_state: TreeChildrenLoadState,
    disabled: bool,
    expanded: bool,
}

impl TreeItemDescriptor {
    /// Creates a tree item descriptor.
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            children: Vec::new(),
            children_load_state: TreeChildrenLoadState::Loaded,
            disabled: false,
            expanded: false,
        }
    }

    /// Adds one child item.
    pub fn child(mut self, child: TreeItemDescriptor) -> Self {
        self.children.push(child);
        self
    }

    /// Adds many child items.
    pub fn children(mut self, children: impl IntoIterator<Item = TreeItemDescriptor>) -> Self {
        self.children.extend(children);
        self
    }

    /// Applies caller-owned child loading metadata.
    pub fn with_children_load_state(mut self, state: TreeChildrenLoadState) -> Self {
        self.children_load_state = state;
        self
    }

    /// Marks children as loadable but not loaded yet.
    pub fn with_children_unloaded(self) -> Self {
        self.with_children_load_state(TreeChildrenLoadState::unloaded())
    }

    /// Marks children as currently loading.
    pub fn with_children_loading(self, message: impl Into<String>) -> Self {
        self.with_children_load_state(TreeChildrenLoadState::loading(message))
    }

    /// Marks child loading as failed.
    pub fn with_children_load_failed(self, message: impl Into<String>) -> Self {
        self.with_children_load_state(TreeChildrenLoadState::failed(message))
    }

    /// Marks this item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks this item as expanded.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Returns the stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns child descriptors.
    pub fn child_descriptors(&self) -> &[TreeItemDescriptor] {
        &self.children
    }

    pub(crate) fn child_descriptors_mut(&mut self) -> &mut Vec<TreeItemDescriptor> {
        &mut self.children
    }

    /// Returns caller-owned child loading metadata.
    pub const fn children_load_state(&self) -> &TreeChildrenLoadState {
        &self.children_load_state
    }

    /// Returns whether this item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    /// Returns whether this item is expanded.
    pub const fn expanded_state(&self) -> bool {
        self.expanded
    }
}

pub(crate) fn apply_tree_expanded_overrides(
    items: &[TreeItemDescriptor],
    expanded_values: &BTreeMap<String, bool>,
) -> Vec<TreeItemDescriptor> {
    items
        .iter()
        .map(|item| apply_tree_expanded_override(item, expanded_values))
        .collect()
}

fn apply_tree_expanded_override(
    item: &TreeItemDescriptor,
    expanded_values: &BTreeMap<String, bool>,
) -> TreeItemDescriptor {
    let mut item = item.clone();
    if let Some(expanded) = expanded_values.get(item.value()) {
        item.expanded = *expanded;
    }
    item.children = item
        .children
        .iter()
        .map(|child| apply_tree_expanded_override(child, expanded_values))
        .collect();
    item
}
