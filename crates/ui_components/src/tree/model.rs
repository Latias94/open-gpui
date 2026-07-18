use open_gpui_ui_core::{Orientation, Role, Size, UiPx};

use crate::choice::{ChoiceCollection, ChoiceInteractionPolicy, ChoiceItemProjection};

use super::{
    TreeChildrenLoadState, TreeDropPosition, TreeItemDescriptor, TreeMetrics, TreeMove,
    TreeMoveTarget,
};
/// Resolved tree item state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeItemState {
    index: usize,
    value: String,
    label: String,
    depth: usize,
    parent_value: Option<String>,
    has_children: bool,
    loaded_child_count: usize,
    children_load_state: TreeChildrenLoadState,
    expanded: bool,
    disabled: bool,
    selected: bool,
    focused: bool,
    position_in_set: Option<usize>,
    size_of_set: usize,
}

impl TreeItemState {
    /// Returns the accessibility role for the tree item.
    pub const fn role(&self) -> Role {
        Role::TreeItem
    }

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

    /// Returns how many child descriptors are currently loaded.
    pub const fn loaded_child_count(&self) -> usize {
        self.loaded_child_count
    }

    /// Returns caller-owned child loading metadata.
    pub const fn children_load_state(&self) -> &TreeChildrenLoadState {
        &self.children_load_state
    }

    /// Returns whether descriptor children are fully loaded.
    pub const fn children_loaded(&self) -> bool {
        self.children_load_state.is_loaded()
    }

    /// Returns whether descriptor children are not loaded yet.
    pub const fn children_unloaded(&self) -> bool {
        self.children_load_state.is_unloaded()
    }

    /// Returns whether children are currently loading.
    pub const fn children_loading(&self) -> bool {
        self.children_load_state.is_loading()
    }

    /// Returns whether child loading failed.
    pub const fn children_load_failed(&self) -> bool {
        self.children_load_state.is_failed()
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
    loaded_child_count: usize,
    children_load_state: TreeChildrenLoadState,
}

impl TreeToggle {
    /// Creates a toggle payload from a tree item.
    pub fn from_item(item: &TreeItemState) -> Option<Self> {
        (item.focusable() && item.has_children() && !item.children_loading()).then(|| Self {
            index: item.index,
            value: item.value.clone(),
            expanded: !item.expanded,
            loaded_child_count: item.loaded_child_count,
            children_load_state: item.children_load_state.clone(),
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

    /// Returns how many child descriptors are currently loaded.
    pub const fn loaded_child_count(&self) -> usize {
        self.loaded_child_count
    }

    /// Returns caller-owned child loading metadata captured at toggle time.
    pub const fn children_load_state(&self) -> &TreeChildrenLoadState {
        &self.children_load_state
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
        let collection = ChoiceCollection::resolve_unique(
            false,
            tree_choice_items(&flattened),
            selected_value,
            focused_value,
            tree_choice_policy(),
        );
        for item in collection
            .items()
            .iter()
            .filter(|item| item.ambiguous_value())
        {
            flattened[*item.item()].disabled = true;
        }
        let selected_index = collection.selected_item().map(|item| *item.item());
        let focused_index = collection.active_item().map(|item| *item.item());
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

    /// Returns the tree accessibility role.
    pub const fn role(&self) -> Role {
        Role::Tree
    }

    /// Returns the accessibility role for visible tree item rows.
    pub const fn item_role(&self) -> Role {
        Role::TreeItem
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

    /// Returns selected visible item value.
    pub fn selected_value(&self) -> Option<&str> {
        self.selected_index
            .and_then(|index| self.items.get(index))
            .map(TreeItemState::value)
    }

    /// Returns selected visible item.
    pub fn selected_item(&self) -> Option<&TreeItemState> {
        self.selected_index.and_then(|index| self.items.get(index))
    }

    /// Returns focused visible item index.
    pub const fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Returns focused visible item value.
    pub fn focused_value(&self) -> Option<&str> {
        self.focused_index
            .and_then(|index| self.items.get(index))
            .map(TreeItemState::value)
    }

    /// Returns focused visible item.
    pub fn focused_item(&self) -> Option<&TreeItemState> {
        self.focused_index.and_then(|index| self.items.get(index))
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
        self.navigation_target_from_value(key, self.focused_value())
    }

    fn navigation_target_from_value(
        &self,
        key: &str,
        current_value: Option<&str>,
    ) -> Option<&TreeItemState> {
        let collection = self.choice_collection();
        collection
            .navigation_target_from_value(key, current_value)
            .and_then(|target| self.items.get(*target.item()))
    }

    /// Resolves a typeahead target for a caller-owned text buffer.
    pub fn typeahead_target(&self, query: &str) -> Option<&TreeItemState> {
        let collection = self.choice_collection();
        collection
            .typeahead_target(query)
            .and_then(|target| self.items.get(*target.item()))
    }

    pub(crate) fn typeahead_target_from_value(
        &self,
        query: &str,
        current_value: Option<&str>,
        search_after_current: bool,
    ) -> Option<&TreeItemState> {
        let collection = self.choice_collection();
        collection
            .typeahead_target_from_value(query, current_value, search_after_current)
            .and_then(|target| self.items.get(*target.item()))
    }

    /// Resolves a keyboard action from the current focused item.
    pub fn keyboard_action_for_key(&self, key: &str) -> Option<TreeKeyboardAction> {
        self.keyboard_action_for_key_from_value(key, self.focused_value())
    }

    pub(crate) fn keyboard_action_for_key_from_value(
        &self,
        key: &str,
        current_value: Option<&str>,
    ) -> Option<TreeKeyboardAction> {
        if let Some(target) = self.navigation_target_from_value(key, current_value) {
            return Some(TreeKeyboardAction::Focus(TreeFocusTarget::new(
                target.index(),
                target.value(),
            )));
        }

        let current = current_value
            .and_then(|value| self.item_by_value(value))
            .or_else(|| self.focused_item())?;
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
        let mut matches = self.items.iter().filter(|item| item.value() == value);
        let item = matches.next()?;
        matches.next().is_none().then_some(item)
    }

    /// Resolves a legal controlled move payload for a visible Tree drop.
    pub fn move_for_drop(
        &self,
        source_value: &str,
        target_value: &str,
        position: TreeDropPosition,
    ) -> Option<TreeMove> {
        let source = self.item_by_value(source_value)?;
        let target = self.item_by_value(target_value)?;

        if !source.focusable()
            || !target.focusable()
            || source.value() == target.value()
            || self.item_is_descendant_of(target.value(), source.value())
        {
            return None;
        }

        if position == TreeDropPosition::Inside
            && (!target.has_children()
                || !target.expanded()
                || !target.children_loaded()
                || target.children_load_failed())
        {
            return None;
        }

        Some(TreeMove::from_items(
            source,
            TreeMoveTarget::from_target(target, position),
        ))
    }

    fn item_is_descendant_of(&self, value: &str, ancestor_value: &str) -> bool {
        let mut parent = self
            .item_by_value(value)
            .and_then(TreeItemState::parent_value);
        while let Some(parent_value) = parent {
            if parent_value == ancestor_value {
                return true;
            }
            parent = self
                .item_by_value(parent_value)
                .and_then(TreeItemState::parent_value);
        }

        false
    }

    fn choice_collection(&self) -> ChoiceCollection<usize> {
        ChoiceCollection::resolve_unique(
            false,
            tree_choice_items(&self.items),
            self.selected_value(),
            self.focused_value(),
            tree_choice_policy(),
        )
    }
}

/// Resolves tree navigation for APG-style key names.
pub fn tree_navigation_target(key: &str, current: usize, disabled: &[bool]) -> Option<usize> {
    ChoiceInteractionPolicy::roving(Orientation::Vertical)
        .navigation_target_index(key, current, disabled)
}

pub(crate) const fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        UiPx::ZERO
    } else {
        value
    }
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
            value: item.value().to_owned(),
            label: item.label().to_owned(),
            depth,
            parent_value: parent_value.map(str::to_owned),
            has_children: !item.child_descriptors().is_empty()
                || item.children_load_state().marks_branch(),
            loaded_child_count: item.child_descriptors().len(),
            children_load_state: item.children_load_state().clone(),
            expanded: item.expanded_state(),
            disabled: item.disabled_state(),
            selected: false,
            focused: false,
            position_in_set: None,
            size_of_set: 0,
        });

        if item.expanded_state() {
            flatten_tree_items(
                item.child_descriptors(),
                Some(item.value()),
                depth + 1,
                flattened,
            );
        }
    }
}

fn tree_choice_policy() -> ChoiceInteractionPolicy {
    ChoiceInteractionPolicy::single_optional(Orientation::Vertical).with_typeahead(true)
}

fn tree_choice_items(items: &[TreeItemState]) -> Vec<ChoiceItemProjection<usize>> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            ChoiceItemProjection::new(
                index,
                None,
                item.value(),
                item.label(),
                !item.focusable(),
                index,
            )
        })
        .collect()
}

pub(crate) fn tree_children_load_hint(state: &TreeChildrenLoadState) -> Option<String> {
    match state {
        TreeChildrenLoadState::Loaded | TreeChildrenLoadState::Unloaded => None,
        TreeChildrenLoadState::Loading { message } => Some(message.clone()),
        TreeChildrenLoadState::Failed { message } => Some(format!("Failed: {message}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{Tree, apply_tree_move};
    use open_gpui_ui_core::Sizable;

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
    fn tree_typeahead_targets_visible_focusable_items_from_current_focus() {
        let state = TreeState::resolve(
            Size::Medium,
            "Document outline",
            None,
            Some("intro"),
            sample_tree(),
        );

        assert_eq!(
            state.typeahead_target(" fi").map(TreeItemState::value),
            Some("figures")
        );
        assert_eq!(
            state.typeahead_target("P").map(TreeItemState::value),
            Some("paper")
        );
        assert_eq!(
            state.typeahead_target("dis").map(TreeItemState::value),
            None
        );
        assert_eq!(state.typeahead_target("").map(TreeItemState::value), None);
        assert_eq!(
            state.typeahead_target("figure 1").map(TreeItemState::value),
            None,
            "collapsed descendants should not participate in visible Tree typeahead"
        );
    }

    #[test]
    fn tree_typeahead_uses_event_time_stable_value_and_refinement_mode() {
        let state = TreeState::resolve(
            Size::Medium,
            "Typeahead tree",
            None,
            Some("alpha"),
            [
                TreeItemDescriptor::new("alpha", "Alpha"),
                TreeItemDescriptor::new("alpine", "Alpine"),
                TreeItemDescriptor::new("amber", "Amber"),
            ],
        );

        assert_eq!(
            state
                .typeahead_target_from_value("a", Some("alpine"), true)
                .map(TreeItemState::value),
            Some("amber")
        );
        assert_eq!(
            state
                .typeahead_target_from_value("al", Some("alpha"), false)
                .map(TreeItemState::value),
            Some("alpha")
        );
        assert_eq!(
            state
                .typeahead_target_from_value("a", Some("removed"), true)
                .map(TreeItemState::value),
            Some("alpha")
        );
    }

    #[test]
    fn tree_duplicate_values_remain_visible_but_fail_closed() {
        let state = TreeState::resolve(
            Size::Medium,
            "Duplicate identity tree",
            Some("duplicate"),
            Some("duplicate"),
            [
                TreeItemDescriptor::new("duplicate", "Duplicate first"),
                TreeItemDescriptor::new("duplicate", "Duplicate second"),
                TreeItemDescriptor::new("unique", "Unique"),
            ],
        );

        assert_eq!(state.items().len(), 3);
        assert!(state.items()[0].disabled());
        assert!(state.items()[1].disabled());
        assert_eq!(state.selected_value(), None);
        assert_eq!(state.focused_value(), Some("unique"));
        assert_eq!(state.item_by_value("duplicate"), None);
        assert_eq!(state.typeahead_target("du"), None);
    }

    #[test]
    fn tree_move_payload_resolves_sibling_and_inside_targets() {
        let state = TreeState::resolve(
            Size::Medium,
            "Document outline",
            None,
            Some("intro"),
            sample_tree(),
        );

        let sibling_move = state
            .move_for_drop("intro", "figures", TreeDropPosition::After)
            .expect("visible siblings should resolve a move payload");
        assert_eq!(sibling_move.value(), "intro");
        assert_eq!(sibling_move.source_parent_value(), Some("paper"));
        assert_eq!(sibling_move.target_parent_value(), Some("paper"));
        assert_eq!(sibling_move.sibling_anchor_value(), Some("figures"));
        assert_eq!(sibling_move.position(), TreeDropPosition::After);
        assert_eq!(sibling_move.target().target_index(), 2);
        assert_eq!(sibling_move.target().target_label(), "Figures");

        let inside_move = state
            .move_for_drop("notes", "paper", TreeDropPosition::Inside)
            .expect("expanded loaded branch should accept inside drops");
        assert_eq!(inside_move.source_parent_value(), None);
        assert_eq!(inside_move.target_parent_value(), Some("paper"));
        assert_eq!(inside_move.sibling_anchor_value(), None);
        assert_eq!(inside_move.position().as_str(), "inside");
    }

    #[test]
    fn tree_move_payload_rejects_illegal_targets() {
        let state = TreeState::resolve(
            Size::Medium,
            "Document outline",
            None,
            Some("paper"),
            sample_tree(),
        );

        assert_eq!(
            state.move_for_drop("paper", "paper", TreeDropPosition::Before),
            None
        );
        assert_eq!(
            state.move_for_drop("paper", "intro", TreeDropPosition::Before),
            None,
            "dropping a branch near its descendant would create an invalid tree"
        );
        assert_eq!(
            state.move_for_drop("notes", "disabled", TreeDropPosition::Before),
            None
        );
        assert_eq!(
            state.move_for_drop("notes", "figures", TreeDropPosition::Inside),
            None,
            "collapsed branches are not inside-drop targets in the first slice"
        );

        let remote = TreeState::resolve(
            Size::Medium,
            "Remote tree",
            None,
            Some("leaf"),
            [
                TreeItemDescriptor::new("unloaded", "Unloaded")
                    .expanded(true)
                    .with_children_unloaded(),
                TreeItemDescriptor::new("loading", "Loading")
                    .expanded(true)
                    .with_children_loading("Loading children"),
                TreeItemDescriptor::new("failed", "Failed")
                    .expanded(true)
                    .with_children_load_failed("Network unavailable"),
                TreeItemDescriptor::new("leaf", "Leaf"),
            ],
        );

        for target in ["unloaded", "loading", "failed"] {
            assert_eq!(
                remote.move_for_drop("leaf", target, TreeDropPosition::Inside),
                None,
                "{target} should not accept inside drops"
            );
        }
    }

    #[test]
    fn apply_tree_move_reorders_and_reparents_descriptor_subtrees() {
        let items = sample_tree();
        let state = TreeState::resolve(
            Size::Medium,
            "Document outline",
            None,
            Some("intro"),
            items.clone(),
        );
        let sibling_move = state
            .move_for_drop("intro", "figures", TreeDropPosition::After)
            .unwrap();
        let reordered = apply_tree_move(items.clone(), &sibling_move).unwrap();
        let paper = reordered
            .iter()
            .find(|item| item.value() == "paper")
            .expect("paper branch should remain");
        let child_values = paper
            .child_descriptors()
            .iter()
            .map(TreeItemDescriptor::value)
            .collect::<Vec<_>>();

        assert_eq!(child_values, ["figures", "intro"]);

        let state = TreeState::resolve(
            Size::Medium,
            "Document outline",
            None,
            Some("notes"),
            reordered.clone(),
        );
        let inside_move = state
            .move_for_drop("notes", "paper", TreeDropPosition::Inside)
            .unwrap();
        let reparented = apply_tree_move(reordered, &inside_move).unwrap();
        let state = TreeState::resolve(
            Size::Medium,
            "Document outline",
            Some("notes"),
            Some("notes"),
            reparented,
        );
        let notes = state.item_by_value("notes").unwrap();

        assert_eq!(notes.parent_value(), Some("paper"));
        assert_eq!(notes.depth(), 1);
        assert!(notes.selected());
    }

    #[test]
    fn tree_behavior_snapshot_virtualizes_visible_rows_with_stable_metadata() {
        let items = (0..100)
            .map(|index| {
                TreeItemDescriptor::new(format!("node-{index:04}"), format!("Node {index:04}"))
            })
            .collect::<Vec<_>>();
        let tree = Tree::new("large-tree", "Large tree", items)
            .with_size(Size::Small)
            .default_selected("node-0012")
            .default_focused("node-0012")
            .virtualized(true)
            .viewport_item_count(5)
            .overscan_count(4);
        let row_height = TreeMetrics::from_size(Size::Small).row_height();
        let snapshot = tree.behavior_snapshot(row_height * 10.0, row_height * 5.0);

        assert_eq!(snapshot.tree_id(), "large-tree");
        assert_eq!(snapshot.label(), "Large tree");
        assert_eq!(snapshot.role(), Role::Tree);
        assert_eq!(snapshot.row_role(), Role::TreeItem);
        assert_eq!(snapshot.state().items().len(), 100);
        assert_eq!(
            *snapshot.visible_range(),
            open_gpui_ui_core::VirtualizerRange::new(10, 15)
        );
        assert_eq!(
            *snapshot.overscan_range(),
            open_gpui_ui_core::VirtualizerRange::new(8, 17)
        );
        assert_eq!(snapshot.visible_row_count(), 5);
        assert_eq!(snapshot.rendered_row_count(), 9);
        assert_eq!(snapshot.rows().len(), 9);
        assert_eq!(snapshot.rows()[0].index(), 8);
        assert_eq!(snapshot.rows()[0].value(), "node-0008");
        assert_eq!(snapshot.rows()[0].render_key(), "8:node-0008");
        assert_eq!(snapshot.rows()[0].virtual_start(), row_height * 8.0);
        assert_eq!(snapshot.rows()[0].virtual_size(), row_height);
        assert_eq!(
            snapshot.focused_row().map(|row| row.value()),
            Some("node-0012")
        );
        assert_eq!(
            snapshot.selected_row().map(|row| row.value()),
            Some("node-0012")
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
                loaded_child_count: 1,
                children_load_state: TreeChildrenLoadState::Loaded,
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
                loaded_child_count: 2,
                children_load_state: TreeChildrenLoadState::Loaded,
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

    #[test]
    fn tree_state_resolves_lazy_branch_load_metadata_without_synthetic_children() {
        let state = TreeState::resolve(
            Size::Medium,
            "Remote tree",
            None,
            Some("unloaded"),
            [
                TreeItemDescriptor::new("unloaded", "Unloaded")
                    .expanded(true)
                    .with_children_unloaded(),
                TreeItemDescriptor::new("loading", "Loading")
                    .expanded(true)
                    .with_children_loading("Loading children"),
                TreeItemDescriptor::new("failed", "Failed")
                    .expanded(true)
                    .with_children_load_failed("Network unavailable"),
                TreeItemDescriptor::new("loaded", "Loaded")
                    .expanded(true)
                    .child(TreeItemDescriptor::new("loaded-child", "Loaded child")),
            ],
        );
        let values = state
            .items()
            .iter()
            .map(TreeItemState::value)
            .collect::<Vec<_>>();

        assert_eq!(
            values,
            ["unloaded", "loading", "failed", "loaded", "loaded-child"]
        );

        let unloaded = state.item_by_value("unloaded").unwrap();
        assert!(unloaded.has_children());
        assert_eq!(unloaded.loaded_child_count(), 0);
        assert!(unloaded.children_unloaded());
        assert!(unloaded.expanded());

        let loading = state.item_by_value("loading").unwrap();
        assert!(loading.has_children());
        assert_eq!(loading.loaded_child_count(), 0);
        assert!(loading.children_loading());
        assert_eq!(
            loading.children_load_state().message(),
            Some("Loading children")
        );

        let failed = state.item_by_value("failed").unwrap();
        assert!(failed.has_children());
        assert_eq!(failed.loaded_child_count(), 0);
        assert!(failed.children_load_failed());
        assert_eq!(
            failed.children_load_state().message(),
            Some("Network unavailable")
        );

        let loaded = state.item_by_value("loaded").unwrap();
        assert!(loaded.children_loaded());
        assert_eq!(loaded.loaded_child_count(), 1);
    }

    #[test]
    fn tree_toggle_payload_includes_child_load_state_and_blocks_loading() {
        let state = TreeState::resolve(
            Size::Medium,
            "Remote tree",
            None,
            Some("unloaded"),
            [
                TreeItemDescriptor::new("unloaded", "Unloaded").with_children_unloaded(),
                TreeItemDescriptor::new("loading", "Loading")
                    .with_children_loading("Loading children"),
                TreeItemDescriptor::new("failed", "Failed")
                    .with_children_load_failed("Network unavailable"),
                TreeItemDescriptor::new("leaf", "Leaf"),
            ],
        );

        let unloaded = state.item_by_value("unloaded").unwrap();
        let toggle = TreeToggle::from_item(unloaded).expect("unloaded branch should toggle");
        assert_eq!(toggle.value(), "unloaded");
        assert!(toggle.expanded());
        assert_eq!(toggle.loaded_child_count(), 0);
        assert_eq!(
            toggle.children_load_state(),
            &TreeChildrenLoadState::Unloaded
        );

        let failed = state.item_by_value("failed").unwrap();
        let toggle = TreeToggle::from_item(failed).expect("failed branch should allow retry");
        assert_eq!(toggle.children_load_state().as_str(), "failed");
        assert_eq!(
            toggle.children_load_state().message(),
            Some("Network unavailable")
        );

        let loading = state.item_by_value("loading").unwrap();
        assert_eq!(TreeToggle::from_item(loading), None);

        let leaf = state.item_by_value("leaf").unwrap();
        assert_eq!(TreeToggle::from_item(leaf), None);
    }
}
