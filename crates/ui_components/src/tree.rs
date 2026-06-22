//! Renderer-neutral state for hierarchical tree surfaces.

use open_gpui_ui_core::{Size, UiPx, ui_px};

use crate::roving_focus::{first_enabled, last_enabled, next_enabled};

/// Pure descriptor for one tree item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeItemDescriptor {
    value: String,
    label: String,
    children: Vec<TreeItemDescriptor>,
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

    /// Returns whether this item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    /// Returns whether this item is expanded.
    pub const fn expanded_state(&self) -> bool {
        self.expanded
    }
}

/// Resolved tree metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeMetrics {
    row_height: UiPx,
    indent_width: UiPx,
    row_padding_x: UiPx,
    row_padding_y: UiPx,
    text_size: UiPx,
}

impl TreeMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            row_height: size.list_row_h(),
            indent_width: match size {
                Size::XSmall | Size::Small => ui_px(14.0),
                Size::Medium | Size::Large => ui_px(16.0),
            },
            row_padding_x: size.list_px(),
            row_padding_y: size.list_py(),
            text_size: size.control_text_px(),
        }
    }

    /// Returns the row height.
    pub const fn row_height(self) -> UiPx {
        self.row_height
    }

    /// Returns the indentation applied per depth level.
    pub const fn indent_width(self) -> UiPx {
        self.indent_width
    }

    /// Returns row horizontal padding.
    pub const fn row_padding_x(self) -> UiPx {
        self.row_padding_x
    }

    /// Returns row vertical padding.
    pub const fn row_padding_y(self) -> UiPx {
        self.row_padding_y
    }

    /// Returns row text size.
    pub const fn text_size(self) -> UiPx {
        self.text_size
    }
}

/// Resolved tree item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeItemState {
    index: usize,
    value: String,
    label: String,
    depth: usize,
    parent_value: Option<String>,
    has_children: bool,
    expanded: bool,
    disabled: bool,
    selected: bool,
    focused: bool,
    position_in_set: Option<usize>,
    size_of_set: usize,
}

impl TreeItemState {
    /// Returns the zero-based visible item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the stable item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the visible item label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the zero-based depth.
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the visible parent value, when present.
    pub fn parent_value(&self) -> Option<&str> {
        self.parent_value.as_deref()
    }

    /// Returns whether the item has children.
    pub const fn has_children(&self) -> bool {
        self.has_children
    }

    /// Returns whether the item is expanded.
    pub const fn expanded(&self) -> bool {
        self.expanded
    }

    /// Returns whether the item is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the item participates in focus and activation.
    pub const fn focusable(&self) -> bool {
        !self.disabled
    }

    /// Returns whether the item is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether the item currently has roving focus.
    pub const fn focused(&self) -> bool {
        self.focused
    }

    /// Returns the one-based position among focusable visible items.
    pub const fn position_in_set(&self) -> Option<usize> {
        self.position_in_set
    }

    /// Returns the total count of focusable visible items.
    pub const fn size_of_set(&self) -> usize {
        self.size_of_set
    }
}

/// Resolved tree selection payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSelection {
    index: usize,
    value: String,
    label: String,
}

impl TreeSelection {
    /// Creates a selection payload from a tree item.
    pub fn from_item(item: &TreeItemState) -> Option<Self> {
        item.focusable().then(|| Self {
            index: item.index,
            value: item.value.clone(),
            label: item.label.clone(),
        })
    }

    /// Returns the selected visible item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the selected item value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the selected item label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Resolved tree expansion toggle payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeToggle {
    index: usize,
    value: String,
    expanded: bool,
}

impl TreeToggle {
    /// Creates a toggle payload from a tree item.
    pub fn from_item(item: &TreeItemState) -> Option<Self> {
        (item.focusable() && item.has_children()).then(|| Self {
            index: item.index,
            value: item.value.clone(),
            expanded: !item.expanded,
        })
    }

    /// Returns the visible item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the item value being toggled.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the desired expanded state after the toggle.
    pub const fn expanded(&self) -> bool {
        self.expanded
    }
}

/// A focus movement requested by tree keyboard handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeFocusTarget {
    index: usize,
    value: String,
}

impl TreeFocusTarget {
    /// Creates a focus target.
    pub fn new(index: usize, value: impl Into<String>) -> Self {
        Self {
            index,
            value: value.into(),
        }
    }

    /// Returns the target visible item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the target item value.
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Keyboard action resolved from tree state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeKeyboardAction {
    /// Move focus to another visible item.
    Focus(TreeFocusTarget),
    /// Toggle expansion for the current visible item.
    Toggle(TreeToggle),
    /// Activate the current visible item.
    Select(TreeSelection),
}

/// Resolved tree state used by tests, adapters, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeState {
    size: Size,
    label: String,
    items: Vec<TreeItemState>,
    selected_index: Option<usize>,
    focused_index: Option<usize>,
    metrics: TreeMetrics,
}

impl TreeState {
    /// Resolves public state for a tree.
    pub fn resolve(
        size: Size,
        label: impl Into<String>,
        selected_value: Option<&str>,
        focused_value: Option<&str>,
        items: impl IntoIterator<Item = TreeItemDescriptor>,
    ) -> Self {
        let descriptors = items.into_iter().collect::<Vec<_>>();
        let mut flattened = Vec::new();
        flatten_tree_items(&descriptors, None, 0, &mut flattened);
        let disabled = flattened
            .iter()
            .map(|item| item.disabled)
            .collect::<Vec<_>>();
        let selected_index = find_focusable_value(&flattened, selected_value);
        let focused_index = find_focusable_value(&flattened, focused_value)
            .or(selected_index)
            .or_else(|| first_enabled(&disabled));
        let focusable_count = flattened.iter().filter(|item| !item.disabled).count();
        let mut position = 0usize;
        let items = flattened
            .into_iter()
            .enumerate()
            .map(|(index, mut item)| {
                let position_in_set = if item.disabled {
                    None
                } else {
                    position += 1;
                    Some(position)
                };

                item.index = index;
                item.selected = selected_index == Some(index);
                item.focused = focused_index == Some(index);
                item.position_in_set = position_in_set;
                item.size_of_set = focusable_count;
                item
            })
            .collect();

        Self {
            size,
            label: label.into(),
            items,
            selected_index,
            focused_index,
            metrics: TreeMetrics::from_size(size),
        }
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the accessible tree label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns visible tree items.
    pub fn items(&self) -> &[TreeItemState] {
        &self.items
    }

    /// Returns selected visible item index.
    pub const fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Returns focused visible item index.
    pub const fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> TreeMetrics {
        self.metrics
    }

    /// Returns whether the tree has no visible items.
    pub fn empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the target item for Up, Down, Home, or End.
    pub fn navigation_target(&self, key: &str) -> Option<&TreeItemState> {
        let disabled = self
            .items
            .iter()
            .map(|item| !item.focusable())
            .collect::<Vec<_>>();
        let target = tree_navigation_target(key, self.focused_index?, &disabled)?;

        self.items.get(target)
    }

    /// Resolves a keyboard action from the current focused item.
    pub fn keyboard_action_for_key(&self, key: &str) -> Option<TreeKeyboardAction> {
        if let Some(target) = self.navigation_target(key) {
            return Some(TreeKeyboardAction::Focus(TreeFocusTarget::new(
                target.index(),
                target.value(),
            )));
        }

        let current = self.items.get(self.focused_index?)?;
        match key {
            "left" if current.has_children() && current.expanded() => {
                TreeToggle::from_item(current).map(TreeKeyboardAction::Toggle)
            }
            "left" => current.parent_value().and_then(|parent| {
                self.item_by_value(parent)
                    .map(|item| TreeFocusTarget::new(item.index(), item.value()))
                    .map(TreeKeyboardAction::Focus)
            }),
            "right" if current.has_children() && !current.expanded() => {
                TreeToggle::from_item(current).map(TreeKeyboardAction::Toggle)
            }
            "right" => self
                .items
                .get(current.index() + 1)
                .filter(|candidate| candidate.parent_value() == Some(current.value()))
                .map(|item| TreeFocusTarget::new(item.index(), item.value()))
                .map(TreeKeyboardAction::Focus),
            "enter" | "space" => TreeSelection::from_item(current).map(TreeKeyboardAction::Select),
            _ => None,
        }
    }

    /// Returns an item by stable value.
    pub fn item_by_value(&self, value: &str) -> Option<&TreeItemState> {
        self.items.iter().find(|item| item.value() == value)
    }
}

/// Resolves tree navigation for APG-style key names.
pub fn tree_navigation_target(key: &str, current: usize, disabled: &[bool]) -> Option<usize> {
    match key {
        "home" => first_enabled(disabled),
        "end" => last_enabled(disabled),
        "up" => next_enabled(disabled, current, false, true),
        "down" => next_enabled(disabled, current, true, true),
        _ => None,
    }
}

fn find_focusable_value(items: &[TreeItemState], value: Option<&str>) -> Option<usize> {
    value.and_then(|value| {
        items
            .iter()
            .position(|item| item.value() == value && item.focusable())
    })
}

fn flatten_tree_items(
    items: &[TreeItemDescriptor],
    parent_value: Option<&str>,
    depth: usize,
    flattened: &mut Vec<TreeItemState>,
) {
    for item in items {
        flattened.push(TreeItemState {
            index: flattened.len(),
            value: item.value.clone(),
            label: item.label.clone(),
            depth,
            parent_value: parent_value.map(str::to_owned),
            has_children: !item.children.is_empty(),
            expanded: item.expanded,
            disabled: item.disabled,
            selected: false,
            focused: false,
            position_in_set: None,
            size_of_set: 0,
        });

        if item.expanded {
            flatten_tree_items(&item.children, Some(item.value()), depth + 1, flattened);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> Vec<TreeItemDescriptor> {
        vec![
            TreeItemDescriptor::new("paper", "Paper")
                .expanded(true)
                .child(TreeItemDescriptor::new("intro", "Introduction"))
                .child(
                    TreeItemDescriptor::new("figures", "Figures")
                        .expanded(false)
                        .child(TreeItemDescriptor::new("figure-1", "Figure 1")),
                ),
            TreeItemDescriptor::new("disabled", "Disabled").disabled(true),
            TreeItemDescriptor::new("notes", "Notes"),
        ]
    }

    #[test]
    fn tree_state_flattens_only_visible_expanded_items() {
        let state = TreeState::resolve(
            Size::Medium,
            "Document outline",
            Some("intro"),
            None,
            sample_tree(),
        );
        let values = state
            .items()
            .iter()
            .map(TreeItemState::value)
            .collect::<Vec<_>>();

        assert_eq!(values, ["paper", "intro", "figures", "disabled", "notes"]);
        assert_eq!(state.selected_index(), Some(1));
        assert_eq!(state.focused_index(), Some(1));
        assert_eq!(state.items()[1].depth(), 1);
        assert_eq!(state.items()[1].parent_value(), Some("paper"));
        assert_eq!(state.items()[3].position_in_set(), None);
        assert_eq!(state.items()[4].position_in_set(), Some(4));
    }

    #[test]
    fn tree_navigation_skips_disabled_visible_items() {
        let state = TreeState::resolve(
            Size::Medium,
            "Document outline",
            None,
            Some("notes"),
            sample_tree(),
        );

        assert_eq!(
            state.navigation_target("down").map(TreeItemState::value),
            Some("paper")
        );
        assert_eq!(
            state.navigation_target("up").map(TreeItemState::value),
            Some("figures")
        );
        assert_eq!(
            state.navigation_target("home").map(TreeItemState::value),
            Some("paper")
        );
        assert_eq!(
            state.navigation_target("end").map(TreeItemState::value),
            Some("notes")
        );
    }

    #[test]
    fn tree_keyboard_action_handles_expand_collapse_and_parent_focus() {
        let state = TreeState::resolve(
            Size::Medium,
            "Document outline",
            None,
            Some("figures"),
            sample_tree(),
        );

        assert_eq!(
            state.keyboard_action_for_key("right"),
            Some(TreeKeyboardAction::Toggle(TreeToggle {
                index: 2,
                value: "figures".to_owned(),
                expanded: true,
            }))
        );
        assert_eq!(
            state.keyboard_action_for_key("left"),
            Some(TreeKeyboardAction::Focus(TreeFocusTarget::new(0, "paper")))
        );

        let state = TreeState::resolve(
            Size::Medium,
            "Document outline",
            None,
            Some("paper"),
            sample_tree(),
        );
        assert_eq!(
            state.keyboard_action_for_key("left"),
            Some(TreeKeyboardAction::Toggle(TreeToggle {
                index: 0,
                value: "paper".to_owned(),
                expanded: false,
            }))
        );
        assert_eq!(
            state.keyboard_action_for_key("right"),
            Some(TreeKeyboardAction::Focus(TreeFocusTarget::new(1, "intro")))
        );
    }

    #[test]
    fn tree_selection_and_toggle_ignore_disabled_or_leaf_items() {
        let state = TreeState::resolve(
            Size::Medium,
            "Document outline",
            None,
            Some("disabled"),
            sample_tree(),
        );
        let disabled = state
            .item_by_value("disabled")
            .expect("disabled item should be visible");
        let notes = state
            .item_by_value("notes")
            .expect("notes item should be visible");

        assert_eq!(TreeSelection::from_item(disabled), None);
        assert_eq!(TreeToggle::from_item(disabled), None);
        assert_eq!(TreeToggle::from_item(notes), None);
        assert_eq!(
            TreeSelection::from_item(notes).map(|selection| selection.value().to_owned()),
            Some("notes".to_owned())
        );
    }
}
