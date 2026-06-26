//! Tree component and renderer-neutral state for hierarchical tree surfaces.

use crate::a11y::UiA11yElementExt;
use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use crate::scroll_area::ScrollArea;
use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, Context, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement, Styled,
    Window, div, point, px, rgb,
};
use open_gpui_ui_core::{Role, Sizable, Size, UiPx, ui_px};
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::roving_focus::{first_enabled, last_enabled, next_enabled};

type TreeSelectHandler = Rc<dyn Fn(TreeSelection, &mut Window, &mut App)>;
type TreeToggleHandler = Rc<dyn Fn(TreeToggle, &mut Window, &mut App)>;

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

    const fn marks_branch(&self) -> bool {
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

#[derive(Debug, Clone, Default)]
struct TreeRuntime {
    scroll_handle: ScrollHandle,
    selected_value: Option<String>,
    focused_value: Option<String>,
    expanded_values: BTreeMap<String, bool>,
    focus_handles: BTreeMap<String, FocusHandle>,
}

impl TreeRuntime {
    fn sync(&mut self, state: &TreeState, cx: &mut Context<Self>) {
        self.focus_handles
            .retain(|value, _| state.items().iter().any(|item| item.value() == value));

        for item in state.items().iter().filter(|item| item.focusable()) {
            self.focus_handles
                .entry(item.value().to_owned())
                .or_insert_with(|| cx.focus_handle());
        }

        self.selected_value = state.selected_value().map(str::to_owned);
        self.focused_value = state.focused_value().map(str::to_owned);
    }

    fn set_focused(&mut self, value: &str, cx: &mut Context<Self>) -> Option<FocusHandle> {
        let value = value.to_owned();
        let changed = self.focused_value.as_deref() != Some(value.as_str());
        self.focused_value = Some(value.clone());
        if changed {
            cx.notify();
        }
        self.focus_handles.get(&value).cloned()
    }

    fn set_selected(&mut self, value: &str, cx: &mut Context<Self>) {
        let changed = self.selected_value.as_deref() != Some(value);
        self.selected_value = Some(value.to_owned());
        if changed {
            cx.notify();
        }
    }

    fn set_expanded(&mut self, value: &str, expanded: bool, cx: &mut Context<Self>) {
        let changed = self.expanded_values.get(value).copied() != Some(expanded);
        self.expanded_values.insert(value.to_owned(), expanded);
        if changed {
            cx.notify();
        }
    }
}

/// A concrete GPUI tree renderer backed by [`TreeState`].
#[derive(IntoElement)]
pub struct Tree {
    id: String,
    label: SharedString,
    items: Vec<TreeItemDescriptor>,
    size: Size,
    selected_value: Option<String>,
    focused_value: Option<String>,
    on_select: Option<TreeSelectHandler>,
    on_toggle: Option<TreeToggleHandler>,
}

impl Tree {
    /// Creates a new tree renderer.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        items: impl IntoIterator<Item = TreeItemDescriptor>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            items: items.into_iter().collect(),
            size: Size::Medium,
            selected_value: None,
            focused_value: None,
            on_select: None,
            on_toggle: None,
        }
    }

    /// Adds one root item descriptor.
    pub fn item(mut self, item: TreeItemDescriptor) -> Self {
        self.items.push(item);
        self
    }

    /// Applies the default selected item value for adapter-owned runtime state.
    pub fn default_selected(mut self, value: impl Into<SharedString>) -> Self {
        self.selected_value = Some(value.into().to_string());
        self
    }

    /// Applies the default focused item value for adapter-owned runtime state.
    pub fn default_focused(mut self, value: impl Into<SharedString>) -> Self {
        self.focused_value = Some(value.into().to_string());
        self
    }

    /// Registers a tree selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(TreeSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Registers a tree expansion toggle handler.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(TreeToggle, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Returns root item descriptors.
    pub fn items(&self) -> &[TreeItemDescriptor] {
        &self.items
    }

    /// Returns resolved tree state from the builder seed.
    pub fn state(&self) -> TreeState {
        self.resolve_state(
            self.items.clone(),
            self.selected_value.as_deref(),
            self.focused_value.as_deref(),
        )
    }

    fn resolve_state(
        &self,
        items: Vec<TreeItemDescriptor>,
        selected_value: Option<&str>,
        focused_value: Option<&str>,
    ) -> TreeState {
        TreeState::resolve(
            self.size,
            self.label.to_string(),
            selected_value,
            focused_value,
            items,
        )
    }
}

impl Sizable for Tree {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Tree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Tree {
            id,
            label,
            items,
            size,
            selected_value,
            focused_value,
            on_select,
            on_toggle,
        } = self;

        window.with_id(id.clone(), |window| {
            let debug_id = id.clone();
            let runtime = window.use_keyed_state("runtime", cx, |_, _| TreeRuntime {
                scroll_handle: ScrollHandle::new(),
                selected_value: selected_value.clone(),
                focused_value: focused_value.clone(),
                expanded_values: BTreeMap::new(),
                focus_handles: BTreeMap::new(),
            });
            let runtime_snapshot = runtime.read(cx).clone();
            let resolved_items =
                apply_tree_expanded_overrides(&items, &runtime_snapshot.expanded_values);
            let state = TreeState::resolve(
                size,
                label.to_string(),
                runtime_snapshot
                    .selected_value
                    .as_deref()
                    .or(selected_value.as_deref()),
                runtime_snapshot
                    .focused_value
                    .as_deref()
                    .or(focused_value.as_deref()),
                resolved_items,
            );
            runtime.update(cx, |runtime, cx| runtime.sync(&state, cx));

            let focus_handles = {
                let runtime = runtime.read(cx);
                state
                    .items()
                    .iter()
                    .map(|item| runtime.focus_handles.get(item.value()).cloned())
                    .collect::<Vec<_>>()
            };
            let root_focus_handle = state
                .focused_index()
                .and_then(|index| focus_handles.get(index).cloned().flatten());
            let scroll_handle = runtime.read(cx).scroll_handle.clone();
            let metrics = state.metrics();
            let rows = state.items().to_vec();
            let content = div()
                .debug_selector({
                    let debug_id = debug_id.clone();
                    move || format!("tree:{debug_id}:content")
                })
                .flex()
                .flex_col()
                .gap_1()
                .p(gpui_px_from_ui(ui_px(6.0)))
                .children(rows.into_iter().enumerate().map(|(index, item)| {
                    render_tree_item(
                        debug_id.clone(),
                        item,
                        focus_handles.get(index).cloned().flatten(),
                        metrics,
                        runtime.clone(),
                        scroll_handle.clone(),
                        state.clone(),
                        on_select.clone(),
                        on_toggle.clone(),
                    )
                }));

            div()
                .id(id.clone())
                .debug_selector({
                    let debug_id = debug_id.clone();
                    move || format!("tree:{debug_id}:root")
                })
                .size_full()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(6.0))
                .border_1()
                .border_color(rgb(0xd6d8ce))
                .bg(rgb(0xffffff))
                .text_size(gpui_px_from_ui(metrics.text_size()))
                .text_color(rgb(0x2f3845))
                .ui_role(state.role())
                .aria_label(label.to_string())
                .on_click(move |_, window, cx| {
                    if let Some(focus_handle) = root_focus_handle.as_ref() {
                        focus_handle.focus(window, cx);
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                })
                .on_scroll_wheel(|_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .child(
                    div().flex_1().min_h(px(0.0)).child(
                        ScrollArea::new(format!("tree:{id}:scroll"), content)
                            .vertical()
                            .with_size(size)
                            .scroll_handle(&scroll_handle),
                    ),
                )
        })
    }
}

fn render_tree_item(
    tree_id: String,
    item: TreeItemState,
    focus_handle: Option<FocusHandle>,
    metrics: TreeMetrics,
    runtime: Entity<TreeRuntime>,
    scroll_handle: ScrollHandle,
    state: TreeState,
    on_select: Option<TreeSelectHandler>,
    on_toggle: Option<TreeToggleHandler>,
) -> impl IntoElement {
    let item_value = item.value().to_owned();
    let item_label = item.label().to_owned();
    let item_index = item.index();
    let disabled = item.disabled();
    let selected = item.selected();
    let focused = item.focused();
    let has_children = item.has_children();
    let children_load_state = item.children_load_state().clone();
    let expanded = item.expanded();
    let selection = TreeSelection::from_item(&item);
    let toggle = TreeToggle::from_item(&item);
    let row_background = if selected {
        rgb(0xe8f3ef)
    } else if focused {
        rgb(0xeef2f7)
    } else {
        rgb(0xffffff)
    };
    let text_color = if disabled {
        rgb(0x7a8492)
    } else {
        rgb(0x2f3845)
    };
    let indent = metrics.indent_width() * item.depth() as f32;
    let item_position = item.position_in_set();
    let item_size_of_set = item.size_of_set();

    div()
        .id(format!("tree:{tree_id}:item:{item_value}"))
        .debug_selector({
            let tree_id = tree_id.clone();
            let item_value = item_value.clone();
            move || format!("tree:{tree_id}:item:{item_value}")
        })
        .min_h(gpui_px_from_ui(metrics.row_height()))
        .w_full()
        .px(gpui_px_from_ui(metrics.row_padding_x()))
        .py(gpui_px_from_ui(metrics.row_padding_y()))
        .flex()
        .items_center()
        .gap_2()
        .rounded_sm()
        .bg(row_background)
        .text_color(text_color)
        .overflow_hidden()
        .ui_role(item.role())
        .aria_label(item.label().to_owned())
        .aria_selected(selected)
        .aria_disabled(disabled)
        .aria_level(item.depth() + 1)
        .when(has_children, |this| this.aria_expanded(expanded))
        .when_some(item_position, |this, position| {
            this.aria_position_in_set(position)
                .aria_size_of_set(item_size_of_set)
        })
        .focusable()
        .tab_stop(focused)
        .when_some(focus_handle.clone(), |this, focus_handle| {
            this.track_focus(&focus_handle)
        })
        .focus_visible(|style| style.border_color(rgb(0x2f80ed)))
        .when(!disabled, |this| {
            this.cursor_pointer().hover(|style| style.bg(rgb(0xf1f5ee)))
        })
        .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
        .when(!disabled, |this| {
            let runtime = runtime.clone();
            let on_select = on_select.clone();
            let selection = selection.clone();
            let focus_handle = focus_handle.clone();
            let scroll_handle = scroll_handle.clone();
            let state = state.clone();
            let item_value = item_value.clone();
            this.on_click(move |_event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                window.prevent_default();
                runtime.update(cx, |runtime, cx| {
                    runtime.set_focused(&item_value, cx);
                    runtime.set_selected(&item_value, cx);
                });
                if let Some(focus_handle) = focus_handle.as_ref() {
                    focus_handle.focus(window, cx);
                }
                scroll_tree_item_into_view(&scroll_handle, &state, item_index);
                if let Some(selection) = selection.clone() {
                    if let Some(on_select) = on_select.as_ref() {
                        on_select(selection, window, cx);
                    }
                }
            })
        })
        .on_key_down({
            let runtime = runtime.clone();
            let scroll_handle = scroll_handle.clone();
            let on_select = on_select.clone();
            let on_toggle = on_toggle.clone();
            move |event: &KeyDownEvent, window, cx| {
                handle_tree_key_down(
                    &state,
                    runtime.clone(),
                    scroll_handle.clone(),
                    on_select.clone(),
                    on_toggle.clone(),
                    event,
                    window,
                    cx,
                );
            }
        })
        .child(div().w(gpui_px_from_ui(indent)).flex_none())
        .child(tree_disclosure(
            tree_id.clone(),
            item_value.clone(),
            item_label.clone(),
            has_children,
            children_load_state.clone(),
            expanded,
            disabled,
            toggle,
            runtime,
            focus_handle,
            on_toggle,
        ))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .child(item_label),
        )
        .when_some(
            tree_children_load_hint(&children_load_state),
            move |this, hint| {
                this.child(
                    div()
                        .debug_selector({
                            let tree_id = tree_id.clone();
                            let item_value = item_value.clone();
                            move || format!("tree:{tree_id}:load-state:{item_value}")
                        })
                        .flex_none()
                        .text_xs()
                        .text_color(rgb(0x5a6472))
                        .child(hint),
                )
            },
        )
}

fn tree_disclosure(
    tree_id: String,
    item_value: String,
    item_label: String,
    has_children: bool,
    children_load_state: TreeChildrenLoadState,
    expanded: bool,
    disabled: bool,
    toggle: Option<TreeToggle>,
    runtime: Entity<TreeRuntime>,
    focus_handle: Option<FocusHandle>,
    on_toggle: Option<TreeToggleHandler>,
) -> impl IntoElement {
    let children_loading = children_load_state.is_loading();
    let glyph = if !has_children {
        ""
    } else if children_loading {
        "..."
    } else if expanded {
        "v"
    } else {
        ">"
    };
    let aria_label = if children_loading {
        format!("Loading {item_label}")
    } else if expanded {
        format!("Collapse {item_label}")
    } else {
        format!("Expand {item_label}")
    };
    div()
        .id(format!("tree:{tree_id}:toggle:{item_value}"))
        .debug_selector({
            let tree_id = tree_id.clone();
            let item_value = item_value.clone();
            move || format!("tree:{tree_id}:toggle:{item_value}")
        })
        .w(px(18.0))
        .h(px(18.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .text_xs()
        .ui_role(Role::Button)
        .aria_label(aria_label)
        .aria_expanded(expanded)
        .aria_disabled(disabled || !has_children || children_loading)
        .when(has_children && !disabled && !children_loading, |this| {
            this.cursor_pointer()
                .hover(|style| style.bg(rgb(0xe8ede6)))
                .on_click(move |_event: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    window.prevent_default();
                    let Some(toggle) = toggle.clone() else {
                        return;
                    };
                    runtime.update(cx, |runtime, cx| {
                        runtime.set_focused(toggle.value(), cx);
                        runtime.set_expanded(toggle.value(), toggle.expanded(), cx);
                    });
                    if let Some(focus_handle) = focus_handle.as_ref() {
                        focus_handle.focus(window, cx);
                    }
                    if let Some(on_toggle) = on_toggle.as_ref() {
                        on_toggle(toggle, window, cx);
                    }
                })
        })
        .child(glyph)
}

fn handle_tree_key_down(
    state: &TreeState,
    runtime: Entity<TreeRuntime>,
    scroll_handle: ScrollHandle,
    on_select: Option<TreeSelectHandler>,
    on_toggle: Option<TreeToggleHandler>,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if event.keystroke.modifiers.modified() {
        return;
    }

    let key = event.keystroke.key.as_str();
    let Some(action) = state.keyboard_action_for_key(key) else {
        return;
    };

    cx.stop_propagation();
    window.prevent_default();

    match action {
        TreeKeyboardAction::Focus(target) => {
            let target_index = target.index();
            let focus_handle =
                runtime.update(cx, |runtime, cx| runtime.set_focused(target.value(), cx));
            if let Some(focus_handle) = focus_handle {
                focus_handle.focus(window, cx);
            }
            scroll_tree_item_into_view(&scroll_handle, state, target_index);
        }
        TreeKeyboardAction::Toggle(toggle) => {
            runtime.update(cx, |runtime, cx| {
                runtime.set_focused(toggle.value(), cx);
                runtime.set_expanded(toggle.value(), toggle.expanded(), cx);
            });
            if let Some(on_toggle) = on_toggle.as_ref() {
                on_toggle(toggle, window, cx);
            }
        }
        TreeKeyboardAction::Select(selection) => {
            let selection_index = selection.index();
            runtime.update(cx, |runtime, cx| {
                runtime.set_focused(selection.value(), cx);
                runtime.set_selected(selection.value(), cx);
            });
            scroll_tree_item_into_view(&scroll_handle, state, selection_index);
            if let Some(on_select) = on_select.as_ref() {
                on_select(selection, window, cx);
            }
        }
    }
}

fn scroll_tree_item_into_view(scroll_handle: &ScrollHandle, state: &TreeState, index: usize) {
    let viewport_extent = ui_px_from_gpui(scroll_handle.bounds().size.height);
    let row_height = nonnegative_px(state.metrics().row_height());
    if viewport_extent.as_f32() <= 0.0 || row_height.as_f32() <= 0.0 {
        return;
    }

    let total_extent = row_height * state.items().len() as f32;
    let current_scroll_offset =
        UiPx::new((-ui_px_from_gpui(scroll_handle.offset().y).as_f32()).max(0.0));
    let row_start = row_height * index as f32;
    let row_end = row_start + row_height;
    let max_scroll = nonnegative_px(total_extent - viewport_extent);
    let target = if row_start < current_scroll_offset {
        row_start
    } else if row_end > current_scroll_offset + viewport_extent {
        row_end - viewport_extent
    } else {
        current_scroll_offset
    };
    let target = target.max(UiPx::ZERO).min(max_scroll);

    scroll_handle.set_offset(point(px(0.0), -gpui_px_from_ui(target)));
}

fn apply_tree_expanded_overrides(
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

const fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        UiPx::ZERO
    } else {
        value
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
            has_children: !item.children.is_empty() || item.children_load_state.marks_branch(),
            loaded_child_count: item.children.len(),
            children_load_state: item.children_load_state.clone(),
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

fn tree_children_load_hint(state: &TreeChildrenLoadState) -> Option<String> {
    match state {
        TreeChildrenLoadState::Loaded | TreeChildrenLoadState::Unloaded => None,
        TreeChildrenLoadState::Loading { message } => Some(message.clone()),
        TreeChildrenLoadState::Failed { message } => Some(format!("Failed: {message}")),
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
