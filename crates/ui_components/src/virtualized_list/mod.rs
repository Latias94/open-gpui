//! Renderer-neutral state for virtualized list surfaces.

use crate::a11y::UiA11yElementExt;
use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use crate::roving_focus::paged_navigation_target;
use crate::scroll_area::ScrollArea;
use crate::scroll_surface::{
    ScrollSurfaceRevealStrategy, ScrollSurfaceRuntime, fixed_row_scroll_target,
    scroll_surface_handle, set_vertical_scroll_offset, vertical_scroll_offset,
    vertical_viewport_extent,
};
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ClickEvent, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, RenderOnce, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, rgb,
};
#[cfg(test)]
use open_gpui_ui_core::ui_px;
use open_gpui_ui_core::{
    Role, RowWindow, Sizable, Size, UiPx, VirtualizerItemKey, VirtualizerItemMeasurement,
    VirtualizerResolvedState, VirtualizerSnapshot, VirtualizerState,
};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

type VirtualizedListActivationHandler =
    Rc<dyn Fn(VirtualizedListActivation, &mut Window, &mut App)>;
type VirtualizedListSelectionChangeHandler =
    Rc<dyn Fn(VirtualizedListSelectionChange, &mut Window, &mut App)>;
type VirtualizedListRowRenderer =
    Rc<dyn Fn(VirtualizedListRowRenderContext, &mut Window, &mut App) -> AnyElement>;

/// Scroll alignment requested when a virtualized row should be revealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VirtualizedListScrollStrategy {
    /// Keep the row visible with the smallest scroll movement.
    #[default]
    Nearest,
    /// Align the row to the top edge of the viewport.
    Top,
    /// Align the row to the viewport center.
    Center,
    /// Align the row to the bottom edge of the viewport.
    Bottom,
}

impl VirtualizedListScrollStrategy {
    /// Returns the stable scroll strategy label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::Top => "top",
            Self::Center => "center",
            Self::Bottom => "bottom",
        }
    }
}

/// Selection behavior for a virtualized list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VirtualizedListSelectionMode {
    /// A single row may be selected. Click, Enter, and Space select and activate.
    #[default]
    Single,
    /// Multiple rows may be selected. Click and Space toggle selection; Enter activates.
    Multiple,
}

impl VirtualizedListSelectionMode {
    /// Returns the stable selection mode label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Multiple => "multiple",
        }
    }
}

/// Anatomy of one virtualized-list row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VirtualizedListRowKind {
    /// Selectable item row.
    #[default]
    Item,
    /// Non-selectable section heading that groups following item rows.
    Section,
    /// Non-selectable visual separator.
    Separator,
    /// Non-selectable loading status row.
    Loading,
    /// Non-selectable empty status row.
    Empty,
    /// Non-selectable error status row.
    Error,
}

impl VirtualizedListRowKind {
    /// Returns the stable row kind label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Section => "section",
            Self::Separator => "separator",
            Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Error => "error",
        }
    }

    /// Returns whether the row participates in active selection and activation.
    pub const fn selectable(self) -> bool {
        matches!(self, Self::Item)
    }

    /// Returns the row accessibility role.
    pub const fn role(self) -> Role {
        match self {
            Self::Item => Role::ListBoxOption,
            Self::Section => Role::Group,
            Self::Separator => Role::Separator,
            Self::Loading => Role::ProgressIndicator,
            Self::Empty => Role::Section,
            Self::Error => Role::AlertDialog,
        }
    }
}

/// Body row height ownership for virtualized-list rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VirtualizedListRowMeasureMode {
    /// Rows keep the shared fixed-height contract.
    #[default]
    Fixed,
    /// Rows may grow to fit rendered content and feed measurements back into the virtualizer.
    Measured,
}

impl VirtualizedListRowMeasureMode {
    /// Returns whether row heights should be measured from rendered content.
    pub const fn measured(self) -> bool {
        matches!(self, Self::Measured)
    }

    /// Returns the stable row measurement mode label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Measured => "measured",
        }
    }
}

/// Pure descriptor for one virtualized list item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizedListItemDescriptor {
    key: String,
    label: String,
    kind: VirtualizedListRowKind,
    disabled: bool,
    disabled_reason: Option<String>,
    secondary_text: Option<String>,
    text_value: Option<String>,
    leading_metadata: Option<String>,
    trailing_metadata: Option<String>,
    badge: Option<String>,
    status: Option<String>,
}

impl VirtualizedListItemDescriptor {
    /// Creates a new item descriptor.
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            kind: VirtualizedListRowKind::Item,
            disabled: false,
            disabled_reason: None,
            secondary_text: None,
            text_value: None,
            leading_metadata: None,
            trailing_metadata: None,
            badge: None,
            status: None,
        }
    }

    /// Creates a selectable item descriptor.
    pub fn item(key: impl Into<String>, primary_text: impl Into<String>) -> Self {
        Self::new(key, primary_text)
    }

    /// Creates a non-selectable section row.
    pub fn section(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(key, label).with_kind(VirtualizedListRowKind::Section)
    }

    /// Creates a non-selectable separator row.
    pub fn separator(key: impl Into<String>) -> Self {
        Self::new(key, "").with_kind(VirtualizedListRowKind::Separator)
    }

    /// Creates a non-selectable loading status row.
    pub fn loading(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(key, message).with_kind(VirtualizedListRowKind::Loading)
    }

    /// Creates a non-selectable empty status row.
    pub fn empty(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(key, message).with_kind(VirtualizedListRowKind::Empty)
    }

    /// Creates a non-selectable error status row.
    pub fn error(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(key, message).with_kind(VirtualizedListRowKind::Error)
    }

    /// Marks the item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the item as disabled and records the reason exposed in snapshots.
    pub fn disabled_reason(mut self, reason: impl Into<String>) -> Self {
        self.disabled = true;
        self.disabled_reason = Some(reason.into());
        self
    }

    /// Applies secondary row text.
    pub fn secondary_text(mut self, secondary_text: impl Into<String>) -> Self {
        self.secondary_text = Some(secondary_text.into());
        self
    }

    /// Applies explicit text used by typeahead and activation payloads.
    pub fn with_text_value(mut self, text_value: impl Into<String>) -> Self {
        self.text_value = Some(text_value.into());
        self
    }

    /// Applies leading metadata text.
    pub fn leading_metadata(mut self, metadata: impl Into<String>) -> Self {
        self.leading_metadata = Some(metadata.into());
        self
    }

    /// Applies trailing metadata text.
    pub fn trailing_metadata(mut self, metadata: impl Into<String>) -> Self {
        self.trailing_metadata = Some(metadata.into());
        self
    }

    /// Applies compact badge text.
    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Applies status text.
    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// Returns the stable item key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the visible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the primary row text.
    pub fn primary_text(&self) -> &str {
        &self.label
    }

    /// Returns the secondary row text.
    pub fn secondary_text_ref(&self) -> Option<&str> {
        self.secondary_text.as_deref()
    }

    /// Returns the text value used by typeahead and accessibility.
    pub fn text_value(&self) -> &str {
        self.text_value.as_deref().unwrap_or(&self.label)
    }

    /// Returns the row kind.
    pub const fn kind(&self) -> VirtualizedListRowKind {
        self.kind
    }

    /// Returns whether the item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    /// Returns the disabled reason.
    pub fn disabled_reason_ref(&self) -> Option<&str> {
        self.disabled_reason.as_deref()
    }

    /// Returns leading metadata text.
    pub fn leading_metadata_ref(&self) -> Option<&str> {
        self.leading_metadata.as_deref()
    }

    /// Returns trailing metadata text.
    pub fn trailing_metadata_ref(&self) -> Option<&str> {
        self.trailing_metadata.as_deref()
    }

    /// Returns badge text.
    pub fn badge_ref(&self) -> Option<&str> {
        self.badge.as_deref()
    }

    /// Returns status text.
    pub fn status_ref(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Returns whether the row participates in active selection and activation.
    pub const fn selectable(&self) -> bool {
        self.kind.selectable() && !self.disabled
    }

    /// Returns the renderer-neutral state item for this descriptor.
    pub fn state_item(&self) -> VirtualizedListStateItem {
        VirtualizedListStateItem::new(self.key(), self.text_value())
            .row_kind(self.kind)
            .disabled(self.disabled)
    }

    fn with_kind(mut self, kind: VirtualizedListRowKind) -> Self {
        self.kind = kind;
        self.disabled = !kind.selectable();
        self
    }
}

/// Renderer-neutral item metadata used by virtualized-list state resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizedListStateItem {
    key: String,
    disabled: bool,
    text_value: String,
    kind: VirtualizedListRowKind,
}

impl VirtualizedListStateItem {
    /// Creates a state item.
    pub fn new(key: impl Into<String>, text_value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            disabled: false,
            text_value: text_value.into(),
            kind: VirtualizedListRowKind::Item,
        }
    }

    /// Marks the item as disabled for focus, selection, and activation.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies row anatomy.
    pub fn row_kind(mut self, kind: VirtualizedListRowKind) -> Self {
        self.kind = kind;
        self
    }

    /// Returns the stable semantic key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns whether this item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
    }

    /// Returns the text value used by typeahead and accessibility.
    pub fn text_value(&self) -> &str {
        &self.text_value
    }

    /// Returns row anatomy.
    pub const fn kind(&self) -> VirtualizedListRowKind {
        self.kind
    }

    /// Returns whether the row participates in active selection and activation.
    pub const fn selectable(&self) -> bool {
        self.kind.selectable() && !self.disabled
    }
}

impl From<VirtualizedListItemDescriptor> for VirtualizedListStateItem {
    fn from(item: VirtualizedListItemDescriptor) -> Self {
        Self::new(
            item.key,
            item.text_value.unwrap_or_else(|| item.label.clone()),
        )
        .row_kind(item.kind)
        .disabled(item.disabled)
    }
}

impl From<&VirtualizedListItemDescriptor> for VirtualizedListStateItem {
    fn from(item: &VirtualizedListItemDescriptor) -> Self {
        item.state_item()
    }
}

/// Resolved virtualized-list metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualizedListMetrics {
    row_height: UiPx,
    overscan_count: usize,
}

impl VirtualizedListMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            row_height: size.list_row_h(),
            overscan_count: match size {
                Size::XSmall | Size::Small => 4,
                Size::Medium => 5,
                Size::Large => 6,
            },
        }
    }

    /// Returns the default fixed row height.
    pub const fn row_height(self) -> UiPx {
        self.row_height
    }

    /// Returns the number of rows the adapter should keep beyond the viewport.
    pub const fn overscan_count(self) -> usize {
        self.overscan_count
    }

    /// Returns the same metrics with a different row height.
    pub fn with_row_height(mut self, row_height: UiPx) -> Self {
        self.row_height = nonnegative_px(row_height);
        self
    }

    /// Returns the same metrics with a different overscan budget.
    pub const fn with_overscan_count(mut self, overscan_count: usize) -> Self {
        self.overscan_count = overscan_count;
        self
    }
}

/// Resolved item target for key-first virtualized-list actions.
#[derive(Debug, Clone, PartialEq, Eq)]
struct VirtualizedListItemTarget {
    key: String,
    index: usize,
    disabled: bool,
    text_value: String,
}

impl VirtualizedListItemTarget {
    /// Creates an item target.
    fn new(
        key: impl Into<String>,
        index: usize,
        disabled: bool,
        text_value: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            index,
            disabled,
            text_value: text_value.into(),
        }
    }

    /// Returns the stable item key.
    fn key(&self) -> &str {
        &self.key
    }

    /// Returns the resolved item index.
    /// Returns whether the target is disabled.
    const fn disabled(&self) -> bool {
        self.disabled
    }
}

/// Resolved activation payload for a virtualized row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizedListActivation {
    key: String,
    index: usize,
    disabled: bool,
    selected: bool,
    text_value: String,
}

impl VirtualizedListActivation {
    /// Creates an activation payload for a visible item.
    pub fn new(index: usize, key: impl Into<String>, text_value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            index,
            disabled: false,
            selected: false,
            text_value: text_value.into(),
        }
    }

    fn from_target(target: VirtualizedListItemTarget, selected: bool) -> Self {
        Self {
            key: target.key,
            index: target.index,
            disabled: target.disabled,
            selected,
            text_value: target.text_value,
        }
    }

    /// Returns the activated item key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the activated item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns whether the activated item was disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the activated item was selected before activation.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns the activated item text value.
    pub fn text_value(&self) -> &str {
        &self.text_value
    }
}

/// Selection-change payload for controlled virtualized-list selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizedListSelectionChange {
    changed_key: String,
    selected_keys: Vec<String>,
}

impl VirtualizedListSelectionChange {
    /// Creates a selection-change payload.
    pub fn new<K>(
        changed_key: impl Into<String>,
        selected_keys: impl IntoIterator<Item = K>,
    ) -> Self
    where
        K: Into<String>,
    {
        Self {
            changed_key: changed_key.into(),
            selected_keys: selected_keys.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns the key whose selection changed.
    pub fn changed_key(&self) -> &str {
        &self.changed_key
    }

    /// Returns all selected keys after the change.
    pub fn selected_keys(&self) -> Vec<&str> {
        self.selected_keys.iter().map(String::as_str).collect()
    }

    fn selected_key_set(&self) -> BTreeSet<String> {
        self.selected_keys.iter().cloned().collect()
    }
}

/// Resolved key-based scroll target for a virtualized list.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListRevealTarget {
    key: String,
    index: usize,
    scroll_offset: UiPx,
    estimated: bool,
}

impl VirtualizedListRevealTarget {
    /// Creates a reveal target.
    pub fn new(key: impl Into<String>, index: usize, scroll_offset: UiPx, estimated: bool) -> Self {
        Self {
            key: key.into(),
            index,
            scroll_offset,
            estimated,
        }
    }

    /// Returns the target key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the target index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the resolved scroll offset.
    pub const fn scroll_offset(&self) -> UiPx {
        self.scroll_offset
    }

    /// Returns whether the target used estimated geometry.
    pub const fn estimated(&self) -> bool {
        self.estimated
    }
}

/// Result of resolving a key-based reveal request.
#[derive(Debug, Clone, PartialEq)]
pub enum VirtualizedListRevealResult {
    /// The row can be revealed with exact fixed-row geometry.
    Revealed(VirtualizedListRevealTarget),
    /// The row can be revealed with estimated geometry.
    Estimated(VirtualizedListRevealTarget),
    /// The key is not present in the current collection.
    NotFound(String),
    /// The key is present but belongs to a non-selectable structural row.
    NotSelectable(String),
}

/// Resolved virtualized-list state used by tests, adapters, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListState {
    size: Size,
    disabled: bool,
    items: Arc<[VirtualizedListStateItem]>,
    duplicate_keys: BTreeSet<String>,
    active_key: Option<String>,
    active_index: Option<usize>,
    selected_keys: BTreeSet<String>,
    selected_indices: Vec<usize>,
    selection_mode: VirtualizedListSelectionMode,
    viewport_item_count: usize,
    metrics: VirtualizedListMetrics,
}

impl VirtualizedListState {
    /// Resolves public state for a virtualized list.
    pub fn resolve<I, T, S, K>(
        size: Size,
        disabled: bool,
        items: I,
        active_key: Option<&str>,
        selected_keys: S,
        selection_mode: VirtualizedListSelectionMode,
        viewport_item_count: Option<usize>,
    ) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<VirtualizedListStateItem>,
        S: IntoIterator<Item = K>,
        K: AsRef<str>,
    {
        let items: Arc<[VirtualizedListStateItem]> = Arc::from(
            items
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let duplicate_keys = duplicate_state_item_keys(items.as_ref());
        let requested_selected_keys = selected_keys
            .into_iter()
            .map(|key| key.as_ref().to_owned())
            .collect::<BTreeSet<_>>();

        let mut selected_keys = BTreeSet::new();
        let mut selected_indices = Vec::new();
        if !disabled {
            for (index, item) in items.iter().enumerate() {
                if !is_selectable_state_item(item, &duplicate_keys) {
                    continue;
                }

                if requested_selected_keys.contains(item.key()) {
                    selected_keys.insert(item.key().to_owned());
                    selected_indices.push(index);
                    if selection_mode == VirtualizedListSelectionMode::Single {
                        break;
                    }
                }
            }
        }

        let active_index = if disabled {
            None
        } else {
            active_key
                .and_then(|key| {
                    state_item_index_by_unique_key(items.as_ref(), &duplicate_keys, key)
                })
                .or_else(|| selected_indices.first().copied())
                .or_else(|| first_selectable_state_item_index(items.as_ref(), &duplicate_keys))
        };
        let active_key = active_index.map(|index| items[index].key().to_owned());

        Self {
            size,
            disabled,
            items,
            duplicate_keys,
            active_key,
            active_index,
            selected_keys,
            selected_indices,
            selection_mode,
            viewport_item_count: viewport_item_count.filter(|count| *count > 0).unwrap_or(1),
            metrics: VirtualizedListMetrics::from_size(size),
        }
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns whether the list should ignore navigation and activation.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the total item count.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns the resolved state items.
    pub fn items(&self) -> &[VirtualizedListStateItem] {
        &self.items
    }

    /// Returns the active descendant key.
    pub fn active_key(&self) -> Option<&str> {
        self.active_key.as_deref()
    }

    /// Returns the active descendant index.
    pub const fn active_index(&self) -> Option<usize> {
        self.active_index
    }

    /// Returns the first selected row index in item order.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_indices.first().copied()
    }

    /// Returns selected row indices in item order.
    pub fn selected_indices(&self) -> &[usize] {
        &self.selected_indices
    }

    /// Returns selected keys in sorted key order.
    pub fn selected_keys(&self) -> Vec<&str> {
        self.selected_keys.iter().map(String::as_str).collect()
    }

    /// Returns selected keys as a set.
    pub const fn selected_key_set(&self) -> &BTreeSet<String> {
        &self.selected_keys
    }

    /// Returns the selection behavior.
    pub const fn selection_mode(&self) -> VirtualizedListSelectionMode {
        self.selection_mode
    }

    /// Returns the estimated number of rows visible in the viewport.
    pub const fn viewport_item_count(&self) -> usize {
        self.viewport_item_count
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> VirtualizedListMetrics {
        self.metrics
    }

    /// Returns the same state with a different resolved metric bundle.
    pub const fn with_metrics(mut self, metrics: VirtualizedListMetrics) -> Self {
        self.metrics = metrics;
        self
    }

    /// Returns the default viewport extent implied by the resolved metrics and viewport item count.
    pub fn viewport_extent(&self) -> UiPx {
        self.metrics.row_height() * self.viewport_item_count as f32
    }

    /// Returns whether the list has no items to render.
    pub fn visible_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the target index for an APG-style navigation key.
    pub fn navigation_target(&self, key: &str) -> Option<usize> {
        if self.disabled {
            return None;
        }

        let active_index = self.active_index?;
        let selectable_indices = self.selectable_indices();
        let active_position = selectable_indices
            .iter()
            .position(|index| *index == active_index)?;
        let target_position = virtualized_list_navigation_target(
            key,
            active_position,
            selectable_indices.len(),
            self.viewport_item_count,
        )?;

        selectable_indices.get(target_position).copied()
    }

    /// Returns activation payload for Enter or Space.
    pub fn activation_for_key(&self, key: &str) -> Option<VirtualizedListActivation> {
        if self.disabled {
            return None;
        }

        match (key, self.selection_mode) {
            ("enter", _) | ("space", VirtualizedListSelectionMode::Single) => {
                let target = self.active_target()?;
                Some(VirtualizedListActivation::from_target(
                    target,
                    self.active_key
                        .as_deref()
                        .is_some_and(|key| self.selected_keys.contains(key)),
                ))
            }
            _ => None,
        }
    }

    /// Returns selection change payload for selection keyboard commands.
    pub fn selection_change_for_key(&self, key: &str) -> Option<VirtualizedListSelectionChange> {
        if key != "space" {
            return None;
        }

        let target = self.active_target()?;
        self.selection_change_for_target(&target)
    }

    /// Returns a key-based reveal target for fixed-height rows.
    pub fn scroll_target_for_key(
        &self,
        key: &str,
        strategy: VirtualizedListScrollStrategy,
        viewport_extent: UiPx,
        current_scroll_offset: UiPx,
    ) -> VirtualizedListRevealResult {
        let matches = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| (item.key() == key).then_some(index))
            .collect::<Vec<_>>();

        let index = match matches.as_slice() {
            [] => return VirtualizedListRevealResult::NotFound(key.to_owned()),
            [index] => *index,
            _ => return VirtualizedListRevealResult::NotSelectable(key.to_owned()),
        };
        if !self.items[index].kind().selectable() {
            return VirtualizedListRevealResult::NotSelectable(key.to_owned());
        }
        let scroll_offset = virtualized_list_scroll_target(
            strategy,
            index,
            self.item_count(),
            self.metrics.row_height(),
            viewport_extent,
            current_scroll_offset,
        );

        VirtualizedListRevealResult::Revealed(VirtualizedListRevealTarget::new(
            key,
            index,
            scroll_offset,
            false,
        ))
    }

    /// Returns a key-based reveal target using keyed measured row sizes when available.
    pub fn scroll_target_for_key_with_snapshot(
        &self,
        key: &str,
        strategy: VirtualizedListScrollStrategy,
        viewport_extent: UiPx,
        current_scroll_offset: UiPx,
        snapshot: &VirtualizerSnapshot,
    ) -> VirtualizedListRevealResult {
        let matches = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| (item.key() == key).then_some(index))
            .collect::<Vec<_>>();

        let index = match matches.as_slice() {
            [] => return VirtualizedListRevealResult::NotFound(key.to_owned()),
            [index] => *index,
            _ => return VirtualizedListRevealResult::NotSelectable(key.to_owned()),
        };
        if !self.items[index].kind().selectable() {
            return VirtualizedListRevealResult::NotSelectable(key.to_owned());
        }

        let (scroll_offset, estimated) = virtualized_list_measured_scroll_target(
            strategy,
            index,
            self.items.as_ref(),
            self.metrics.row_height(),
            viewport_extent,
            current_scroll_offset,
            snapshot,
        );
        let target = VirtualizedListRevealTarget::new(key, index, scroll_offset, estimated);

        if estimated {
            VirtualizedListRevealResult::Estimated(target)
        } else {
            VirtualizedListRevealResult::Revealed(target)
        }
    }

    /// Clamps a requested item index into the list range.
    pub fn clamped_index(&self, index: usize) -> Option<usize> {
        valid_index(index, self.item_count()).or_else(|| self.item_count().checked_sub(1))
    }

    fn target_at_index(&self, index: usize) -> Option<VirtualizedListItemTarget> {
        let item = self.items.get(index)?;
        is_selectable_state_item(item, &self.duplicate_keys).then(|| {
            VirtualizedListItemTarget::new(
                item.key().to_owned(),
                index,
                self.disabled || item.disabled_state(),
                item.text_value().to_owned(),
            )
        })
    }

    fn active_target(&self) -> Option<VirtualizedListItemTarget> {
        self.active_index
            .and_then(|index| self.target_at_index(index))
    }

    fn selection_change_for_target(
        &self,
        target: &VirtualizedListItemTarget,
    ) -> Option<VirtualizedListSelectionChange> {
        if self.disabled || target.disabled() {
            return None;
        }

        let mut selected_keys = self.selected_keys.clone();
        match self.selection_mode {
            VirtualizedListSelectionMode::Single => {
                if selected_keys.len() == 1 && selected_keys.contains(target.key()) {
                    return None;
                }
                selected_keys.clear();
                selected_keys.insert(target.key().to_owned());
            }
            VirtualizedListSelectionMode::Multiple => {
                if !selected_keys.insert(target.key().to_owned()) {
                    selected_keys.remove(target.key());
                }
            }
        }

        Some(VirtualizedListSelectionChange::new(
            target.key().to_owned(),
            selected_keys,
        ))
    }

    fn selectable_indices(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                is_selectable_state_item(item, &self.duplicate_keys).then_some(index)
            })
            .collect()
    }
}

/// Public behavior snapshot for one virtualized-list row.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListRowBehaviorSnapshot {
    item: VirtualizedListItemDescriptor,
    render_key: String,
    index: usize,
    position_in_set: Option<usize>,
    size_of_set: usize,
    virtual_start: UiPx,
    virtual_size: UiPx,
    measured: bool,
    active: bool,
    selected: bool,
    disabled: bool,
    role: Role,
}

impl VirtualizedListRowBehaviorSnapshot {
    fn from_render_plan(row: &VirtualizedListRowRenderPlan) -> Self {
        Self {
            item: row.item().clone(),
            render_key: row.render_key().to_owned(),
            index: row.index(),
            position_in_set: row.position_in_set(),
            size_of_set: row.size_of_set(),
            virtual_start: row.virtual_start(),
            virtual_size: row.virtual_size(),
            measured: row.measured(),
            active: row.active(),
            selected: row.selected(),
            disabled: row.disabled(),
            role: row.role(),
        }
    }

    /// Returns the source descriptor.
    pub const fn item(&self) -> &VirtualizedListItemDescriptor {
        &self.item
    }

    /// Returns the stable source item key.
    pub fn key(&self) -> &str {
        self.item.key()
    }

    /// Returns the visible item label.
    pub fn label(&self) -> &str {
        self.item.label()
    }

    /// Returns the row kind.
    pub const fn kind(&self) -> VirtualizedListRowKind {
        self.item.kind()
    }

    /// Returns secondary row text.
    pub fn secondary_text(&self) -> Option<&str> {
        self.item.secondary_text_ref()
    }

    /// Returns the text value used by typeahead and activation.
    pub fn text_value(&self) -> &str {
        self.item.text_value()
    }

    /// Returns disabled reason text.
    pub fn disabled_reason(&self) -> Option<&str> {
        self.item.disabled_reason_ref()
    }

    /// Returns leading metadata text.
    pub fn leading_metadata(&self) -> Option<&str> {
        self.item.leading_metadata_ref()
    }

    /// Returns trailing metadata text.
    pub fn trailing_metadata(&self) -> Option<&str> {
        self.item.trailing_metadata_ref()
    }

    /// Returns badge text.
    pub fn badge(&self) -> Option<&str> {
        self.item.badge_ref()
    }

    /// Returns status text.
    pub fn status(&self) -> Option<&str> {
        self.item.status_ref()
    }

    /// Returns the stable render key.
    pub fn render_key(&self) -> &str {
        &self.render_key
    }

    /// Returns the zero-based item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the one-based position within the selectable option set.
    pub const fn position_in_set(&self) -> Option<usize> {
        self.position_in_set
    }

    /// Returns the total selectable option set size.
    pub const fn size_of_set(&self) -> usize {
        self.size_of_set
    }

    /// Returns the virtual row start offset.
    pub const fn virtual_start(&self) -> UiPx {
        self.virtual_start
    }

    /// Returns the virtual row size.
    pub const fn virtual_size(&self) -> UiPx {
        self.virtual_size
    }

    /// Returns whether the virtual row size came from measured content.
    pub const fn measured(&self) -> bool {
        self.measured
    }

    /// Returns whether this row is active.
    pub const fn active(&self) -> bool {
        self.active
    }

    /// Returns whether this row is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether this row is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the accessibility role for this row.
    pub const fn role(&self) -> Role {
        self.role
    }
}

/// Read-only context passed to a custom `VirtualizedList` row renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListRowRenderContext {
    item: VirtualizedListItemDescriptor,
    render_key: String,
    index: usize,
    position_in_set: Option<usize>,
    size_of_set: usize,
    virtual_start: UiPx,
    virtual_size: UiPx,
    measured: bool,
    row_measure_mode: VirtualizedListRowMeasureMode,
    active: bool,
    selected: bool,
    disabled: bool,
    role: Role,
}

impl VirtualizedListRowRenderContext {
    fn from_render_plan(
        row: &VirtualizedListRowRenderPlan,
        row_measure_mode: VirtualizedListRowMeasureMode,
    ) -> Self {
        Self {
            item: row.item().clone(),
            render_key: row.render_key().to_owned(),
            index: row.index(),
            position_in_set: row.position_in_set(),
            size_of_set: row.size_of_set(),
            virtual_start: row.virtual_start(),
            virtual_size: row.virtual_size(),
            measured: row.measured(),
            row_measure_mode,
            active: row.active(),
            selected: row.selected(),
            disabled: row.disabled(),
            role: row.role(),
        }
    }

    /// Returns the source descriptor.
    pub const fn item(&self) -> &VirtualizedListItemDescriptor {
        &self.item
    }

    /// Returns the stable source item key.
    pub fn key(&self) -> &str {
        self.item.key()
    }

    /// Returns the visible item label.
    pub fn label(&self) -> &str {
        self.item.label()
    }

    /// Returns the row kind.
    pub const fn kind(&self) -> VirtualizedListRowKind {
        self.item.kind()
    }

    /// Returns whether this row participates in active selection and activation.
    pub const fn selectable(&self) -> bool {
        self.item.kind().selectable() && !self.disabled
    }

    /// Returns secondary row text.
    pub fn secondary_text(&self) -> Option<&str> {
        self.item.secondary_text_ref()
    }

    /// Returns the text value used by typeahead and activation.
    pub fn text_value(&self) -> &str {
        self.item.text_value()
    }

    /// Returns disabled reason text.
    pub fn disabled_reason(&self) -> Option<&str> {
        self.item.disabled_reason_ref()
    }

    /// Returns leading metadata text.
    pub fn leading_metadata(&self) -> Option<&str> {
        self.item.leading_metadata_ref()
    }

    /// Returns trailing metadata text.
    pub fn trailing_metadata(&self) -> Option<&str> {
        self.item.trailing_metadata_ref()
    }

    /// Returns badge text.
    pub fn badge(&self) -> Option<&str> {
        self.item.badge_ref()
    }

    /// Returns status text.
    pub fn status(&self) -> Option<&str> {
        self.item.status_ref()
    }

    /// Returns the stable render key used for element identity.
    pub fn render_key(&self) -> &str {
        &self.render_key
    }

    /// Returns the zero-based source row index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the one-based position within the selectable option set.
    pub const fn position_in_set(&self) -> Option<usize> {
        self.position_in_set
    }

    /// Returns the total selectable option set size.
    pub const fn size_of_set(&self) -> usize {
        self.size_of_set
    }

    /// Returns the virtual row start offset.
    pub const fn virtual_start(&self) -> UiPx {
        self.virtual_start
    }

    /// Returns the virtual row size enforced by the outer row.
    pub const fn virtual_size(&self) -> UiPx {
        self.virtual_size
    }

    /// Returns whether the virtual row size came from measured content.
    pub const fn measured(&self) -> bool {
        self.measured
    }

    /// Returns the row measurement mode for this render pass.
    pub const fn row_measure_mode(&self) -> VirtualizedListRowMeasureMode {
        self.row_measure_mode
    }

    /// Returns whether this row is active.
    pub const fn active(&self) -> bool {
        self.active
    }

    /// Returns whether this row is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether this row is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the accessibility role owned by the outer row element.
    pub const fn role(&self) -> Role {
        self.role
    }
}

/// Public behavior snapshot for a concrete virtualized list.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListBehaviorSnapshot {
    list_id: String,
    label: String,
    state: VirtualizedListState,
    metrics: VirtualizedListMetrics,
    row_measure_mode: VirtualizedListRowMeasureMode,
    total_size: UiPx,
    viewport_extent: UiPx,
    scroll_offset: UiPx,
    virtualizer_snapshot: VirtualizerSnapshot,
    visible_range: open_gpui_ui_core::VirtualizerRange,
    overscan_range: open_gpui_ui_core::VirtualizerRange,
    rows: Vec<VirtualizedListRowBehaviorSnapshot>,
    visible_row_count: usize,
    overscan_count: usize,
    role: Role,
    row_role: Role,
}

impl VirtualizedListBehaviorSnapshot {
    fn from_render_plan(plan: &VirtualizedListRenderPlan) -> Self {
        Self {
            list_id: plan.list_id().to_owned(),
            label: plan.label().to_owned(),
            state: plan.state().clone(),
            metrics: plan.metrics(),
            row_measure_mode: plan.row_measure_mode(),
            total_size: plan.virtualizer().total_size(),
            viewport_extent: plan.virtualizer().viewport_extent(),
            scroll_offset: plan.virtualizer().scroll_offset(),
            virtualizer_snapshot: plan.virtualizer().snapshot().clone(),
            visible_range: plan.virtualizer().visible_range().clone(),
            overscan_range: plan.virtualizer().overscan_range().clone(),
            rows: plan
                .rows()
                .iter()
                .map(VirtualizedListRowBehaviorSnapshot::from_render_plan)
                .collect(),
            visible_row_count: plan.visible_row_count(),
            overscan_count: plan.overscan_count(),
            role: plan.role(),
            row_role: plan.row_role(),
        }
    }

    /// Returns the stable list id.
    pub fn list_id(&self) -> &str {
        &self.list_id
    }

    /// Returns the accessible list label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the resolved renderer-neutral state.
    pub const fn state(&self) -> &VirtualizedListState {
        &self.state
    }

    /// Returns the resolved metrics.
    pub const fn metrics(&self) -> VirtualizedListMetrics {
        self.metrics
    }

    /// Returns the row measurement mode used by the snapshot.
    pub const fn row_measure_mode(&self) -> VirtualizedListRowMeasureMode {
        self.row_measure_mode
    }

    /// Returns the virtualized total size.
    pub const fn total_size(&self) -> UiPx {
        self.total_size
    }

    /// Returns the viewport extent used to resolve the snapshot.
    pub const fn viewport_extent(&self) -> UiPx {
        self.viewport_extent
    }

    /// Returns the scroll offset used to resolve the snapshot.
    pub const fn scroll_offset(&self) -> UiPx {
        self.scroll_offset
    }

    /// Returns the virtualizer snapshot emitted by this resolution.
    pub const fn virtualizer_snapshot(&self) -> &VirtualizerSnapshot {
        &self.virtualizer_snapshot
    }

    /// Returns the viewport-visible source row range.
    pub const fn visible_range(&self) -> &open_gpui_ui_core::VirtualizerRange {
        &self.visible_range
    }

    /// Returns the rendered source row range after overscan.
    pub const fn overscan_range(&self) -> &open_gpui_ui_core::VirtualizerRange {
        &self.overscan_range
    }

    /// Returns rows in render order.
    pub fn rows(&self) -> &[VirtualizedListRowBehaviorSnapshot] {
        &self.rows
    }

    /// Returns the accessibility role for the root list container.
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Returns the accessibility role for row containers.
    pub const fn row_role(&self) -> Role {
        self.row_role
    }

    /// Returns the number of rows visible before overscan.
    pub const fn visible_row_count(&self) -> usize {
        self.visible_row_count
    }

    /// Returns the number of rows rendered after overscan.
    pub fn rendered_row_count(&self) -> usize {
        self.rows.len()
    }

    /// Returns the overscan budget.
    pub const fn overscan_count(&self) -> usize {
        self.overscan_count
    }

    /// Returns the active row, when it is inside the rendered window.
    pub fn active_row(&self) -> Option<&VirtualizedListRowBehaviorSnapshot> {
        self.rows.iter().find(|row| row.active())
    }

    /// Returns the selected row, when it is inside the rendered window.
    pub fn selected_row(&self) -> Option<&VirtualizedListRowBehaviorSnapshot> {
        self.rows.iter().find(|row| row.selected())
    }
}

/// One resolved virtualized row in render order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VirtualizedListRowRenderPlan {
    item: VirtualizedListItemDescriptor,
    render_key: String,
    index: usize,
    position_in_set: Option<usize>,
    size_of_set: usize,
    measurement: VirtualizerItemMeasurement,
    active: bool,
    selected: bool,
    disabled: bool,
    role: Role,
}

impl VirtualizedListRowRenderPlan {
    fn new(
        item: VirtualizedListItemDescriptor,
        render_key: String,
        index: usize,
        measurement: VirtualizerItemMeasurement,
        position_in_set: Option<usize>,
        size_of_set: usize,
        state: &VirtualizedListState,
    ) -> Self {
        let active = item.selectable() && state.active_key() == Some(item.key());
        let selected = item.kind().selectable() && state.selected_key_set().contains(item.key());
        let disabled = state.disabled() || item.disabled_state();
        let role = item.kind().role();

        Self {
            item,
            render_key,
            index,
            position_in_set,
            size_of_set,
            measurement,
            active,
            selected,
            disabled,
            role,
        }
    }

    /// Returns the source descriptor.
    pub fn item(&self) -> &VirtualizedListItemDescriptor {
        &self.item
    }

    /// Returns the stable source item key.
    pub fn key(&self) -> &str {
        self.item.key()
    }

    /// Returns the visible item label.
    pub fn label(&self) -> &str {
        self.item.label()
    }

    fn target(&self) -> VirtualizedListItemTarget {
        VirtualizedListItemTarget::new(
            self.key().to_owned(),
            self.index,
            self.disabled,
            self.item.text_value().to_owned(),
        )
    }

    /// Returns the stable render key.
    pub fn render_key(&self) -> &str {
        &self.render_key
    }

    /// Returns the zero-based item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the one-based position within the selectable option set.
    pub const fn position_in_set(&self) -> Option<usize> {
        self.position_in_set
    }

    /// Returns the total selectable option set size.
    pub const fn size_of_set(&self) -> usize {
        self.size_of_set
    }

    /// Returns whether this row is active.
    pub const fn active(&self) -> bool {
        self.active
    }

    /// Returns whether this row is selected.
    pub const fn selected(&self) -> bool {
        self.selected
    }

    /// Returns whether this row is disabled.
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    /// Returns the virtual row start offset.
    pub const fn virtual_start(&self) -> UiPx {
        self.measurement.start()
    }

    /// Returns the virtual row size.
    pub const fn virtual_size(&self) -> UiPx {
        self.measurement.size()
    }

    /// Returns whether this row size came from a measurement cache.
    pub const fn measured(&self) -> bool {
        self.measurement.measured()
    }

    /// Returns the accessibility role for this row.
    pub const fn role(&self) -> Role {
        self.role
    }

    fn render_context(
        &self,
        row_measure_mode: VirtualizedListRowMeasureMode,
    ) -> VirtualizedListRowRenderContext {
        VirtualizedListRowRenderContext::from_render_plan(self, row_measure_mode)
    }
}

/// Fully resolved render contract for a concrete virtualized list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VirtualizedListRenderPlan {
    list_id: String,
    label: String,
    state: VirtualizedListState,
    metrics: VirtualizedListMetrics,
    row_measure_mode: VirtualizedListRowMeasureMode,
    virtualizer: VirtualizerResolvedState,
    rows: Vec<VirtualizedListRowRenderPlan>,
    visible_row_count: usize,
    overscan_count: usize,
    role: Role,
    row_role: Role,
}

impl VirtualizedListRenderPlan {
    /// Resolves a render plan from renderer-neutral state and item descriptors.
    pub fn resolve(
        list_id: impl Into<String>,
        label: impl Into<String>,
        state: VirtualizedListState,
        items: &[VirtualizedListItemDescriptor],
        row_measure_mode: VirtualizedListRowMeasureMode,
        row_measurements: &BTreeMap<String, UiPx>,
        snapshot: Option<&VirtualizerSnapshot>,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> Self {
        let metrics = state.metrics();
        let state_items = virtualized_list_state_items(items);
        let selected_keys = state
            .selected_keys()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let state = VirtualizedListState::resolve(
            state.size(),
            state.disabled(),
            state_items,
            state.active_key(),
            selected_keys,
            state.selection_mode(),
            Some(state.viewport_item_count()),
        )
        .with_metrics(metrics);
        let metrics = state.metrics();
        let viewport_extent = resolve_viewport_extent(&state, viewport_extent);
        let duplicate_keys = duplicate_item_keys(items);
        let row_positions = virtualized_list_row_positions(items);
        let option_count = row_positions
            .iter()
            .filter(|position| position.is_some())
            .count();
        let virtualizer = resolve_virtualized_list_virtualizer(
            items,
            metrics,
            row_measure_mode,
            row_measurements,
            snapshot,
            nonnegative_px(scroll_offset),
            viewport_extent,
            &duplicate_keys,
        );
        let row_window = RowWindow::project(&virtualizer, |index| items.get(index).cloned());
        let visible_row_count = row_window.visible_row_count();
        let overscan_count = row_window.overscan_count();
        let rows = row_window
            .into_rows()
            .into_iter()
            .map(|projected| {
                let (index, render_key, measurement, item) = projected.into_parts();
                VirtualizedListRowRenderPlan::new(
                    item,
                    render_key,
                    index,
                    measurement,
                    row_positions.get(index).copied().flatten(),
                    option_count,
                    &state,
                )
            })
            .collect();

        Self {
            list_id: list_id.into(),
            label: label.into(),
            state,
            metrics,
            row_measure_mode,
            virtualizer,
            rows,
            visible_row_count,
            overscan_count,
            role: Role::ListBox,
            row_role: Role::ListBoxOption,
        }
    }

    /// Returns the stable list id.
    pub fn list_id(&self) -> &str {
        &self.list_id
    }

    /// Returns the accessible list label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the resolved renderer-neutral state.
    pub fn state(&self) -> &VirtualizedListState {
        &self.state
    }

    /// Returns the resolved metrics.
    pub const fn metrics(&self) -> VirtualizedListMetrics {
        self.metrics
    }

    /// Returns the row measurement mode used by the plan.
    pub const fn row_measure_mode(&self) -> VirtualizedListRowMeasureMode {
        self.row_measure_mode
    }

    /// Returns the resolved virtualizer state.
    pub const fn virtualizer(&self) -> &VirtualizerResolvedState {
        &self.virtualizer
    }

    /// Returns rows in render order.
    pub fn rows(&self) -> &[VirtualizedListRowRenderPlan] {
        &self.rows
    }

    /// Returns custom-renderer contexts in render order.
    #[cfg(test)]
    pub fn row_contexts(&self) -> Vec<VirtualizedListRowRenderContext> {
        self.rows
            .iter()
            .map(|row| row.render_context(self.row_measure_mode))
            .collect()
    }

    /// Returns the accessibility role for the root list container.
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Returns the accessibility role for row containers.
    pub const fn row_role(&self) -> Role {
        self.row_role
    }

    /// Returns the number of rows visible before overscan.
    pub const fn visible_row_count(&self) -> usize {
        self.visible_row_count
    }

    /// Returns the overscan budget.
    pub const fn overscan_count(&self) -> usize {
        self.overscan_count
    }
}

#[derive(Debug, Clone)]
struct VirtualizedListRuntime {
    scroll_surface: ScrollSurfaceRuntime,
    focus_handle: FocusHandle,
    active_key: Option<String>,
    selected_keys: BTreeSet<String>,
    row_measurements: BTreeMap<String, UiPx>,
    pending_scroll_to_active: Option<String>,
}

impl VirtualizedListRuntime {
    fn set_row_measurement(&mut self, render_key: String, height: UiPx, cx: &mut Context<Self>) {
        let height = nonnegative_px(height);
        if self.row_measurements.get(&render_key).copied() != Some(height) {
            self.row_measurements.insert(render_key, height);
            cx.notify();
        }
    }
}

/// A concrete GPUI virtualized list renderer.
#[derive(IntoElement)]
pub struct VirtualizedList {
    id: String,
    label: SharedString,
    items: Arc<[VirtualizedListItemDescriptor]>,
    size: Size,
    disabled: bool,
    active_key: Option<String>,
    selected_keys: BTreeSet<String>,
    selection_mode: VirtualizedListSelectionMode,
    viewport_item_count: usize,
    metrics: VirtualizedListMetrics,
    row_measure_mode: VirtualizedListRowMeasureMode,
    snapshot: Option<VirtualizerSnapshot>,
    row_renderer: Option<VirtualizedListRowRenderer>,
    on_activate: Option<VirtualizedListActivationHandler>,
    on_selection_change: Option<VirtualizedListSelectionChangeHandler>,
}

impl VirtualizedList {
    /// Creates a new virtualized list renderer.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        items: impl IntoIterator<Item = VirtualizedListItemDescriptor>,
    ) -> Self {
        Self::from_shared_items(
            id,
            label,
            Arc::from(items.into_iter().collect::<Vec<_>>().into_boxed_slice()),
        )
    }

    /// Creates a new virtualized list renderer from shared item storage.
    pub fn from_shared_items(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        items: Arc<[VirtualizedListItemDescriptor]>,
    ) -> Self {
        let size = Size::Medium;

        Self {
            id: id.into(),
            label: label.into(),
            items,
            size,
            disabled: false,
            active_key: None,
            selected_keys: BTreeSet::new(),
            selection_mode: VirtualizedListSelectionMode::Single,
            viewport_item_count: DEFAULT_VIRTUALIZED_LIST_VIEWPORT_ITEM_COUNT,
            metrics: VirtualizedListMetrics::from_size(size),
            row_measure_mode: VirtualizedListRowMeasureMode::default(),
            snapshot: None,
            row_renderer: None,
            on_activate: None,
            on_selection_change: None,
        }
    }

    /// Marks the list as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies the default active item key for adapter-owned runtime state.
    pub fn default_active_key(mut self, key: impl Into<String>) -> Self {
        self.active_key = Some(key.into());
        self
    }

    /// Applies the default selected item key for adapter-owned runtime state.
    pub fn default_selected_key(mut self, key: impl Into<String>) -> Self {
        self.selected_keys.clear();
        self.selected_keys.insert(key.into());
        self
    }

    /// Applies the default selected item keys for adapter-owned runtime state.
    pub fn default_selected_keys<I, K>(mut self, keys: I) -> Self
    where
        I: IntoIterator<Item = K>,
        K: Into<String>,
    {
        self.selected_keys = keys.into_iter().map(Into::into).collect();
        self
    }

    /// Applies the list selection behavior.
    pub fn selection_mode(mut self, selection_mode: VirtualizedListSelectionMode) -> Self {
        self.selection_mode = selection_mode;
        self
    }

    /// Applies the estimated viewport item count used for keyboard page navigation.
    pub fn viewport_item_count(mut self, count: usize) -> Self {
        self.viewport_item_count = count.max(1);
        self
    }

    /// Applies a fixed row height.
    pub fn row_height(mut self, row_height: UiPx) -> Self {
        self.metrics = self.metrics.with_row_height(row_height);
        self
    }

    /// Applies the body row measurement mode.
    pub fn row_measure_mode(mut self, row_measure_mode: VirtualizedListRowMeasureMode) -> Self {
        self.row_measure_mode = row_measure_mode;
        self
    }

    /// Seeds measured-row virtualizer measurements from a snapshot.
    pub fn virtualizer_snapshot(mut self, snapshot: VirtualizerSnapshot) -> Self {
        self.snapshot = Some(snapshot);
        self
    }

    /// Registers a custom row-content renderer.
    ///
    /// The outer row keeps ownership of virtual layout, accessibility, focus, hit testing, and
    /// selection behavior. The renderer replaces only the row content.
    pub fn render_row<E>(
        mut self,
        renderer: impl Fn(VirtualizedListRowRenderContext, &mut Window, &mut App) -> E + 'static,
    ) -> Self
    where
        E: IntoElement + 'static,
    {
        self.row_renderer = Some(Rc::new(move |context, window, cx| {
            renderer(context, window, cx).into_any_element()
        }));
        self
    }

    /// Applies the overscan row budget.
    pub fn overscan(mut self, overscan: usize) -> Self {
        self.metrics = self.metrics.with_overscan_count(overscan);
        self
    }

    /// Registers an activation handler for clicked or keyboard-activated rows.
    pub fn on_activate(
        mut self,
        handler: impl Fn(VirtualizedListActivation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Registers a selection-change handler for controlled selected keys.
    pub fn on_selection_change(
        mut self,
        handler: impl Fn(VirtualizedListSelectionChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved renderer-neutral list state from the builder seed.
    pub fn state(&self) -> VirtualizedListState {
        self.resolved_state(
            self.active_key.as_deref(),
            self.selected_keys.iter().map(String::as_str),
            self.viewport_item_count,
        )
    }

    /// Returns the public behavior snapshot at the default viewport origin.
    pub fn behavior_snapshot(&self) -> VirtualizedListBehaviorSnapshot {
        self.behavior_snapshot_with_viewport(
            UiPx::ZERO,
            self.metrics.row_height() * self.viewport_item_count as f32,
        )
    }

    /// Resolves the public behavior snapshot for a viewport.
    pub fn behavior_snapshot_with_viewport(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> VirtualizedListBehaviorSnapshot {
        let plan = self.render_plan(scroll_offset, viewport_extent);
        VirtualizedListBehaviorSnapshot::from_render_plan(&plan)
    }

    /// Resolves the renderer-neutral state and virtual window for the current list.
    fn render_plan(&self, scroll_offset: UiPx, viewport_extent: UiPx) -> VirtualizedListRenderPlan {
        let state = self.resolved_state(
            self.active_key.as_deref(),
            self.selected_keys.iter().map(String::as_str),
            self.viewport_item_count,
        );
        VirtualizedListRenderPlan::resolve(
            self.id.clone(),
            self.label.to_string(),
            state,
            self.items.as_ref(),
            self.row_measure_mode,
            &BTreeMap::new(),
            self.snapshot.as_ref(),
            scroll_offset,
            viewport_extent,
        )
    }

    fn resolved_state<'a, I>(
        &self,
        active_key: Option<&str>,
        selected_keys: I,
        viewport_item_count: usize,
    ) -> VirtualizedListState
    where
        I: IntoIterator<Item = &'a str>,
    {
        VirtualizedListState::resolve(
            self.size,
            self.disabled,
            virtualized_list_state_items(self.items.as_ref()),
            active_key,
            selected_keys,
            self.selection_mode,
            Some(viewport_item_count.max(1)),
        )
        .with_metrics(self.metrics)
    }
}

impl Sizable for VirtualizedList {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self.metrics = VirtualizedListMetrics::from_size(size);
        self
    }
}

impl RenderOnce for VirtualizedList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime_id = format!("virtualized-list:{}:runtime", self.id);
        let debug_id = self.id.to_string();
        let runtime = window.use_keyed_state(runtime_id, cx, |_, cx| VirtualizedListRuntime {
            scroll_surface: ScrollSurfaceRuntime::new(None),
            focus_handle: cx.focus_handle(),
            active_key: self.active_key.clone(),
            selected_keys: self.selected_keys.clone(),
            row_measurements: BTreeMap::new(),
            pending_scroll_to_active: None,
        });
        let runtime_state = runtime.read(cx).clone();
        let scroll_handle = scroll_surface_handle(&runtime_state.scroll_surface, None);
        let focus_handle = runtime_state.focus_handle.clone();
        let viewport_extent = vertical_viewport_extent(&scroll_handle);
        let viewport_item_count = resolve_viewport_item_count(
            self.metrics.row_height(),
            viewport_extent,
            self.viewport_item_count,
        );
        let state = self.resolved_state(
            runtime_state.active_key.as_deref(),
            runtime_state.selected_keys.iter().map(String::as_str),
            viewport_item_count,
        );
        let scroll_offset = vertical_scroll_offset(&scroll_handle);
        let plan = VirtualizedListRenderPlan::resolve(
            self.id.clone(),
            self.label.to_string(),
            state.clone(),
            self.items.as_ref(),
            self.row_measure_mode,
            &runtime_state.row_measurements,
            self.snapshot.as_ref(),
            scroll_offset,
            viewport_extent,
        );
        if let Some(pending_scroll_to_active) = runtime_state.pending_scroll_to_active.as_deref() {
            scroll_active_key(
                &scroll_handle,
                &state,
                pending_scroll_to_active,
                plan.row_measure_mode(),
                plan.virtualizer().snapshot(),
            );
            runtime.update(cx, |runtime, _| {
                runtime.pending_scroll_to_active = None;
            });
        }
        let on_activate = self.on_activate.clone();
        let on_selection_change = self.on_selection_change.clone();
        let list_state = plan.state().clone();
        let rows = plan.rows().to_vec();
        let row_measure_mode = plan.row_measure_mode();
        let estimated_row_height = plan.metrics().row_height();
        let virtualizer_snapshot = plan.virtualizer().snapshot().clone();
        let row_renderer = self.row_renderer.clone();
        let list_id = plan.list_id().to_owned();
        let scroll_viewport_id = format!("virtualized-list:{}:viewport", plan.list_id());
        let root_click_state = list_state.clone();

        runtime.update(cx, |runtime, _| {
            if runtime.active_key.as_deref() != list_state.active_key() {
                runtime.active_key = list_state.active_key().map(str::to_owned);
                runtime.pending_scroll_to_active = list_state.active_key().map(str::to_owned);
            }
            if &runtime.selected_keys != list_state.selected_key_set() {
                runtime.selected_keys = list_state.selected_key_set().clone();
            }
        });

        div()
            .id(self.id)
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("virtualized-list:{debug_id}:root")
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
            .text_size(gpui_px_from_ui(self.size.control_text_px()))
            .text_color(rgb(0x2f3845))
            .focusable()
            .tab_group()
            .tab_stop(!list_state.disabled() && !list_state.visible_empty())
            .track_focus(&focus_handle)
            .focus_visible(|style| style.border_color(rgb(0x2f80ed)))
            .ui_role(plan.role())
            .aria_label(plan.label().to_owned())
            .aria_disabled(list_state.disabled())
            .on_click({
                let focus_handle = focus_handle.clone();
                move |_, window, cx| {
                    if !root_click_state.disabled() && !root_click_state.visible_empty() {
                        focus_handle.focus(window, cx);
                    }
                }
            })
            .on_scroll_wheel(|_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_key_down({
                let runtime = runtime.clone();
                let scroll_handle = scroll_handle.clone();
                let on_activate = on_activate.clone();
                let on_selection_change = on_selection_change.clone();
                let plan_state = list_state.clone();
                let row_measure_mode = row_measure_mode;
                let virtualizer_snapshot = virtualizer_snapshot.clone();
                move |event: &KeyDownEvent, window, cx| {
                    handle_virtualized_list_key_down(
                        &plan_state,
                        runtime.clone(),
                        scroll_handle.clone(),
                        on_activate.clone(),
                        on_selection_change.clone(),
                        row_measure_mode,
                        &virtualizer_snapshot,
                        event,
                        window,
                        cx,
                    );
                }
            })
            .child(
                div().flex_1().min_h(px(0.0)).child(
                    ScrollArea::new(
                        scroll_viewport_id,
                        render_virtualized_list_body(
                            &list_id,
                            &rows,
                            plan.virtualizer().total_size(),
                            row_measure_mode,
                            estimated_row_height,
                            row_renderer,
                            list_state.clone(),
                            runtime.clone(),
                            focus_handle,
                            on_activate,
                            on_selection_change,
                            window,
                            cx,
                        ),
                    )
                    .vertical()
                    .scroll_handle(&scroll_handle)
                    .with_size(self.size),
                ),
            )
    }
}

fn render_virtualized_list_body(
    list_id: &str,
    rows: &[VirtualizedListRowRenderPlan],
    total_size: UiPx,
    row_measure_mode: VirtualizedListRowMeasureMode,
    estimated_row_height: UiPx,
    row_renderer: Option<VirtualizedListRowRenderer>,
    list_state: VirtualizedListState,
    runtime: Entity<VirtualizedListRuntime>,
    focus_handle: FocusHandle,
    on_activate: Option<VirtualizedListActivationHandler>,
    on_selection_change: Option<VirtualizedListSelectionChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let rows = rows.to_vec();
    let list_id = list_id.to_owned();
    let body_id = format!("virtualized-list:{list_id}:body");
    let mut row_elements = Vec::with_capacity(rows.len());
    for row in rows {
        row_elements.push(
            render_virtualized_list_row(
                list_id.clone(),
                row,
                row_measure_mode,
                estimated_row_height,
                row_renderer.clone(),
                list_state.clone(),
                runtime.clone(),
                focus_handle.clone(),
                on_activate.clone(),
                on_selection_change.clone(),
                window,
                cx,
            )
            .into_any_element(),
        );
    }

    div()
        .id(body_id.clone())
        .debug_selector({
            let body_id = body_id.clone();
            move || body_id.clone()
        })
        .relative()
        .w_full()
        .h(gpui_px_from_ui(total_size))
        .children(row_elements)
        .into_any_element()
}

fn render_virtualized_list_row(
    list_id: String,
    row: VirtualizedListRowRenderPlan,
    row_measure_mode: VirtualizedListRowMeasureMode,
    estimated_row_height: UiPx,
    row_renderer: Option<VirtualizedListRowRenderer>,
    list_state: VirtualizedListState,
    runtime: Entity<VirtualizedListRuntime>,
    focus_handle: FocusHandle,
    on_activate: Option<VirtualizedListActivationHandler>,
    on_selection_change: Option<VirtualizedListSelectionChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let render_key = row.render_key().to_owned();
    let target = row.target();
    let activation = VirtualizedListActivation::from_target(target.clone(), row.selected());
    let row_kind = row.item().kind();
    let primary_text = row.label().to_owned();
    let secondary_text = row.item().secondary_text_ref().map(str::to_owned);
    let leading_metadata = row.item().leading_metadata_ref().map(str::to_owned);
    let trailing_metadata = row.item().trailing_metadata_ref().map(str::to_owned);
    let badge = row.item().badge_ref().map(str::to_owned);
    let status = row.item().status_ref().map(str::to_owned);
    let row_background = if row.selected() {
        rgb(0xe7f0ff)
    } else if row.active() {
        rgb(0xeef2f7)
    } else if row.index().is_multiple_of(2) {
        rgb(0xffffff)
    } else {
        rgb(0xf8f9f3)
    };
    let text_color = if row.disabled() {
        rgb(0x8b93a1)
    } else {
        rgb(0x2f3845)
    };
    let row_content = if let Some(row_renderer) = row_renderer.as_ref() {
        row_renderer(row.render_context(row_measure_mode), window, cx)
    } else {
        render_default_virtualized_list_row_content(
            row_kind,
            primary_text,
            secondary_text,
            leading_metadata,
            trailing_metadata,
            badge,
            status,
        )
    };

    div()
        .on_children_prepainted({
            let runtime = runtime.clone();
            let render_key = render_key.clone();
            move |row_bounds, _window, cx| {
                if row_measure_mode.measured() {
                    let measured_height = row_bounds
                        .iter()
                        .map(|bounds| bounds.size.height)
                        .fold(Pixels::ZERO, Pixels::max);
                    let measured_height = measured_height.ceil();
                    runtime.update(cx, |runtime, cx| {
                        runtime.set_row_measurement(
                            render_key.clone(),
                            ui_px_from_gpui(measured_height),
                            cx,
                        );
                    });
                }
            }
        })
        .id(format!("virtualized-list:{list_id}:row:{render_key}"))
        .debug_selector({
            let list_id = list_id.clone();
            let render_key = render_key.clone();
            move || format!("virtualized-list:{list_id}:row:{render_key}")
        })
        .absolute()
        .top(gpui_px_from_ui(row.virtual_start()))
        .left(px(0.0))
        .right(px(0.0))
        .when(row_measure_mode.measured(), |this| {
            this.min_h(gpui_px_from_ui(estimated_row_height))
        })
        .when(!row_measure_mode.measured(), |this| {
            this.h(gpui_px_from_ui(row.virtual_size()))
        })
        .min_w(px(0.0))
        .flex()
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(rgb(0xe2e4dc))
        .bg(row_background)
        .text_color(text_color)
        .ui_role(row.role())
        .aria_selected(row.selected())
        .aria_disabled(row.disabled())
        .when_some(row.position_in_set(), |this, position| {
            this.aria_position_in_set(position)
        })
        .when(!row.disabled(), |this| {
            this.cursor_pointer().hover(|style| style.bg(rgb(0xeef2f7)))
        })
        .when(!row.disabled(), |this| {
            let runtime = runtime.clone();
            let focus_handle = focus_handle.clone();
            let on_activate = on_activate.clone();
            let on_selection_change = on_selection_change.clone();
            let list_state = list_state.clone();
            let target = target.clone();
            let activation = activation.clone();
            let activate_on_click =
                list_state.selection_mode() == VirtualizedListSelectionMode::Single;
            this.on_click(move |_event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                window.prevent_default();
                let selection_change = list_state.selection_change_for_target(&target);
                runtime.update(cx, |runtime, _| {
                    runtime.active_key = Some(target.key().to_owned());
                    if let Some(selection_change) = selection_change.as_ref() {
                        runtime.selected_keys = selection_change.selected_key_set();
                    }
                    runtime.pending_scroll_to_active = None;
                });
                focus_handle.focus(window, cx);
                if let (Some(on_selection_change), Some(selection_change)) =
                    (on_selection_change.as_ref(), selection_change)
                {
                    on_selection_change(selection_change, window, cx);
                }
                if activate_on_click && let Some(on_activate) = on_activate.as_ref() {
                    on_activate(activation.clone(), window, cx);
                }
            })
        })
        .child(row_content)
}

fn render_default_virtualized_list_row_content(
    row_kind: VirtualizedListRowKind,
    primary_text: String,
    secondary_text: Option<String>,
    leading_metadata: Option<String>,
    trailing_metadata: Option<String>,
    badge: Option<String>,
    status: Option<String>,
) -> AnyElement {
    if row_kind == VirtualizedListRowKind::Separator {
        return div()
            .mx(px(8.0))
            .h(px(1.0))
            .w_full()
            .bg(rgb(0xe2e4dc))
            .into_any_element();
    }

    div()
        .w_full()
        .min_w(px(0.0))
        .px(px(10.0))
        .flex()
        .items_center()
        .gap_2()
        .when_some(leading_metadata, |this, metadata| {
            this.child(
                div()
                    .text_color(rgb(0x667085))
                    .text_size(gpui_px_from_ui(Size::XSmall.control_text_px()))
                    .child(metadata),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .child(primary_text)
                .when_some(secondary_text, |this, secondary_text| {
                    this.child(
                        div()
                            .text_color(rgb(0x667085))
                            .text_size(gpui_px_from_ui(Size::XSmall.control_text_px()))
                            .child(secondary_text),
                    )
                }),
        )
        .when_some(badge, |this, badge| {
            this.child(
                div()
                    .rounded(px(4.0))
                    .bg(rgb(0xeef2f7))
                    .px_1()
                    .text_color(rgb(0x475467))
                    .text_size(gpui_px_from_ui(Size::XSmall.control_text_px()))
                    .child(badge),
            )
        })
        .when_some(status, |this, status| {
            this.child(
                div()
                    .text_color(rgb(0x475467))
                    .text_size(gpui_px_from_ui(Size::XSmall.control_text_px()))
                    .child(status),
            )
        })
        .when_some(trailing_metadata, |this, metadata| {
            this.child(
                div()
                    .text_color(rgb(0x667085))
                    .text_size(gpui_px_from_ui(Size::XSmall.control_text_px()))
                    .child(metadata),
            )
        })
        .into_any_element()
}

fn handle_virtualized_list_key_down(
    state: &VirtualizedListState,
    runtime: Entity<VirtualizedListRuntime>,
    scroll_handle: ScrollHandle,
    on_activate: Option<VirtualizedListActivationHandler>,
    on_selection_change: Option<VirtualizedListSelectionChangeHandler>,
    row_measure_mode: VirtualizedListRowMeasureMode,
    virtualizer_snapshot: &VirtualizerSnapshot,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if state.disabled() || state.visible_empty() {
        return;
    }

    let key = event.keystroke.key.as_str();
    if let Some(target) = state.navigation_target(key) {
        let Some(target) = state.target_at_index(target) else {
            return;
        };
        cx.stop_propagation();
        window.prevent_default();
        runtime.update(cx, |runtime, _| {
            runtime.active_key = Some(target.key().to_owned());
            runtime.pending_scroll_to_active = Some(target.key().to_owned());
        });
        scroll_active_key(
            &scroll_handle,
            state,
            target.key(),
            row_measure_mode,
            virtualizer_snapshot,
        );
        return;
    }

    if let Some(activation) = state.activation_for_key(key) {
        cx.stop_propagation();
        window.prevent_default();
        let selection_change = if state.selection_mode() == VirtualizedListSelectionMode::Single {
            state
                .target_at_index(activation.index())
                .and_then(|target| state.selection_change_for_target(&target))
        } else {
            None
        };
        runtime.update(cx, |runtime, _| {
            runtime.active_key = Some(activation.key().to_owned());
            if let Some(selection_change) = selection_change.as_ref() {
                runtime.selected_keys = selection_change.selected_key_set();
            }
            runtime.pending_scroll_to_active = Some(activation.key().to_owned());
        });
        scroll_active_key(
            &scroll_handle,
            state,
            activation.key(),
            row_measure_mode,
            virtualizer_snapshot,
        );
        if let (Some(on_selection_change), Some(selection_change)) =
            (on_selection_change.as_ref(), selection_change)
        {
            on_selection_change(selection_change, window, cx);
        }
        if let Some(on_activate) = on_activate.as_ref() {
            on_activate(activation, window, cx);
        }
        return;
    }

    if let Some(selection_change) = state.selection_change_for_key(key) {
        cx.stop_propagation();
        window.prevent_default();
        runtime.update(cx, |runtime, _| {
            runtime.selected_keys = selection_change.selected_key_set();
        });
        if let Some(on_selection_change) = on_selection_change.as_ref() {
            on_selection_change(selection_change, window, cx);
        }
    }
}

fn scroll_active_key(
    scroll_handle: &ScrollHandle,
    state: &VirtualizedListState,
    key: &str,
    row_measure_mode: VirtualizedListRowMeasureMode,
    virtualizer_snapshot: &VirtualizerSnapshot,
) {
    let viewport_extent = state.viewport_extent();
    let current_scroll_offset = vertical_scroll_offset(scroll_handle);
    let target = match if row_measure_mode.measured() {
        state.scroll_target_for_key_with_snapshot(
            key,
            VirtualizedListScrollStrategy::Nearest,
            viewport_extent,
            current_scroll_offset,
            virtualizer_snapshot,
        )
    } else {
        state.scroll_target_for_key(
            key,
            VirtualizedListScrollStrategy::Nearest,
            viewport_extent,
            current_scroll_offset,
        )
    } {
        VirtualizedListRevealResult::Revealed(target)
        | VirtualizedListRevealResult::Estimated(target) => target,
        VirtualizedListRevealResult::NotFound(_)
        | VirtualizedListRevealResult::NotSelectable(_) => {
            return;
        }
    };

    if target.scroll_offset() != current_scroll_offset {
        set_vertical_scroll_offset(scroll_handle, target.scroll_offset());
    }
}

fn resolve_viewport_item_count(row_height: UiPx, viewport_extent: UiPx, fallback: usize) -> usize {
    let row_height = nonnegative_px(row_height);
    let viewport_extent = nonnegative_px(viewport_extent);
    if viewport_extent.as_f32() > 0.0 && row_height.as_f32() > 0.0 {
        (viewport_extent.as_f32() / row_height.as_f32())
            .ceil()
            .max(1.0) as usize
    } else {
        fallback.max(1)
    }
}

const DEFAULT_VIRTUALIZED_LIST_VIEWPORT_ITEM_COUNT: usize = 8;

const fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        UiPx::ZERO
    } else {
        value
    }
}

/// Resolves virtualized-list navigation for APG-style key names.
pub fn virtualized_list_navigation_target(
    key: &str,
    current: usize,
    item_count: usize,
    viewport_item_count: usize,
) -> Option<usize> {
    paged_navigation_target(key, current, item_count, viewport_item_count)
}

/// Resolves a fixed-height scroll target for a virtualized list.
pub fn virtualized_list_scroll_target(
    strategy: VirtualizedListScrollStrategy,
    target_index: usize,
    item_count: usize,
    row_height: UiPx,
    viewport_extent: UiPx,
    current_scroll_offset: UiPx,
) -> UiPx {
    fixed_row_scroll_target(
        scroll_surface_reveal_strategy(strategy),
        target_index,
        item_count,
        row_height,
        viewport_extent,
        current_scroll_offset,
    )
}

fn virtualized_list_measured_scroll_target(
    strategy: VirtualizedListScrollStrategy,
    target_index: usize,
    items: &[VirtualizedListStateItem],
    estimated_row_height: UiPx,
    viewport_extent: UiPx,
    current_scroll_offset: UiPx,
    snapshot: &VirtualizerSnapshot,
) -> (UiPx, bool) {
    let estimated_row_height = nonnegative_px(estimated_row_height);
    let viewport_extent = nonnegative_px(viewport_extent);
    if items.is_empty() {
        return (UiPx::ZERO, true);
    }

    let target_index = target_index.min(items.len() - 1);
    let measurements_by_key = snapshot
        .measurements()
        .iter()
        .map(|item| (item.key().as_str().to_owned(), nonnegative_px(item.size())))
        .collect::<BTreeMap<_, _>>();
    let mut cursor = UiPx::ZERO;
    let mut estimated = false;
    let mut target_start = UiPx::ZERO;
    let mut target_size = estimated_row_height;

    for (index, item) in items.iter().enumerate() {
        let measured_size = measurements_by_key.get(item.key()).copied();
        let size = measured_size.unwrap_or_else(|| {
            estimated = true;
            estimated_row_height
        });

        if index == target_index {
            target_start = cursor;
            target_size = size;
        }

        cursor = cursor + size;
    }

    let total_size = cursor;
    let max_scroll_offset = nonnegative_px(total_size - viewport_extent);
    let current_scroll_offset = nonnegative_px(current_scroll_offset).min(max_scroll_offset);
    let target_end = target_start + target_size;
    let target = match scroll_surface_reveal_strategy(strategy) {
        ScrollSurfaceRevealStrategy::Nearest => {
            let viewport_start = current_scroll_offset;
            let viewport_end = viewport_start + viewport_extent;
            if target_start < viewport_start {
                target_start
            } else if target_end > viewport_end {
                target_end - viewport_extent
            } else {
                viewport_start
            }
        }
        ScrollSurfaceRevealStrategy::Top => target_start,
        ScrollSurfaceRevealStrategy::Center => {
            target_start + target_size.half() - viewport_extent.half()
        }
        ScrollSurfaceRevealStrategy::Bottom => target_end - viewport_extent,
    };

    (nonnegative_px(target).min(max_scroll_offset), estimated)
}

const fn scroll_surface_reveal_strategy(
    strategy: VirtualizedListScrollStrategy,
) -> ScrollSurfaceRevealStrategy {
    match strategy {
        VirtualizedListScrollStrategy::Nearest => ScrollSurfaceRevealStrategy::Nearest,
        VirtualizedListScrollStrategy::Top => ScrollSurfaceRevealStrategy::Top,
        VirtualizedListScrollStrategy::Center => ScrollSurfaceRevealStrategy::Center,
        VirtualizedListScrollStrategy::Bottom => ScrollSurfaceRevealStrategy::Bottom,
    }
}

const fn valid_index(index: usize, item_count: usize) -> Option<usize> {
    if index < item_count {
        Some(index)
    } else {
        None
    }
}

fn resolve_viewport_extent(state: &VirtualizedListState, viewport_extent: UiPx) -> UiPx {
    let viewport_extent = nonnegative_px(viewport_extent);
    if viewport_extent.as_f32() > 0.0 {
        viewport_extent
    } else {
        state.viewport_extent()
    }
}

fn duplicate_item_keys(items: &[VirtualizedListItemDescriptor]) -> BTreeSet<String> {
    let mut counts = BTreeMap::new();
    for item in items {
        *counts.entry(item.key().to_owned()).or_insert(0usize) += 1;
    }

    counts
        .into_iter()
        .filter_map(|(key, count)| (count > 1).then_some(key))
        .collect()
}

fn virtualized_list_row_positions(items: &[VirtualizedListItemDescriptor]) -> Vec<Option<usize>> {
    let mut option_position = 0usize;
    items
        .iter()
        .map(|item| {
            item.kind().selectable().then(|| {
                option_position += 1;
                option_position
            })
        })
        .collect()
}

fn virtualized_list_state_items(
    items: &[VirtualizedListItemDescriptor],
) -> Vec<VirtualizedListStateItem> {
    items.iter().map(VirtualizedListStateItem::from).collect()
}

fn duplicate_state_item_keys(items: &[VirtualizedListStateItem]) -> BTreeSet<String> {
    let mut counts = BTreeMap::new();
    for item in items {
        *counts.entry(item.key().to_owned()).or_insert(0usize) += 1;
    }

    counts
        .into_iter()
        .filter_map(|(key, count)| (count > 1).then_some(key))
        .collect()
}

fn is_selectable_state_item(
    item: &VirtualizedListStateItem,
    duplicate_keys: &BTreeSet<String>,
) -> bool {
    item.selectable() && !duplicate_keys.contains(item.key())
}

fn state_item_index_by_unique_key(
    items: &[VirtualizedListStateItem],
    duplicate_keys: &BTreeSet<String>,
    key: &str,
) -> Option<usize> {
    if duplicate_keys.contains(key) {
        return None;
    }

    items
        .iter()
        .position(|item| item.key() == key && item.selectable())
}

fn first_selectable_state_item_index(
    items: &[VirtualizedListStateItem],
    duplicate_keys: &BTreeSet<String>,
) -> Option<usize> {
    items
        .iter()
        .position(|item| is_selectable_state_item(item, duplicate_keys))
}

fn virtualized_list_render_key(
    item: &VirtualizedListItemDescriptor,
    index: usize,
    duplicate_keys: &BTreeSet<String>,
) -> String {
    if duplicate_keys.contains(item.key()) {
        format!("{index}:{}", item.key())
    } else {
        item.key().to_owned()
    }
}

fn resolve_virtualized_list_virtualizer(
    items: &[VirtualizedListItemDescriptor],
    metrics: VirtualizedListMetrics,
    row_measure_mode: VirtualizedListRowMeasureMode,
    row_measurements: &BTreeMap<String, UiPx>,
    snapshot: Option<&VirtualizerSnapshot>,
    scroll_offset: UiPx,
    viewport_extent: UiPx,
    duplicate_keys: &BTreeSet<String>,
) -> VirtualizerResolvedState {
    let mut state = VirtualizerState::new(items.len(), metrics.row_height())
        .with_viewport_extent(viewport_extent)
        .with_overscan(metrics.overscan_count())
        .with_scroll_offset(nonnegative_px(scroll_offset));

    if !row_measure_mode.measured() {
        return state.resolve_fixed_window(|index| {
            let item = &items[index];
            VirtualizerItemKey::new(virtualized_list_render_key(item, index, duplicate_keys))
        });
    }

    if let Some(snapshot) = snapshot.cloned() {
        state = state.with_snapshot(snapshot);
    }
    for (key, height) in row_measurements {
        state = state.with_measurement(key.clone(), *height);
    }
    state = state.with_scroll_offset(nonnegative_px(scroll_offset));

    state.resolve_measured_window(|index| {
        let item = &items[index];
        VirtualizerItemKey::new(virtualized_list_render_key(item, index, duplicate_keys))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtualized_list_state_resolves_active_from_keys_and_preserves_metrics() {
        let items = (0..10)
            .map(|index| VirtualizedListStateItem::new(format!("item-{index}"), index.to_string()))
            .collect::<Vec<_>>();

        let state = VirtualizedListState::resolve(
            Size::Small,
            false,
            items,
            Some("item-12"),
            ["item-4"],
            VirtualizedListSelectionMode::Single,
            Some(5),
        );

        assert_eq!(state.size(), Size::Small);
        assert_eq!(state.item_count(), 10);
        assert_eq!(state.active_key(), Some("item-4"));
        assert_eq!(state.active_index(), Some(4));
        assert_eq!(state.selected_index(), Some(4));
        assert_eq!(state.selected_keys(), ["item-4"]);
        assert_eq!(state.viewport_item_count(), 5);
        assert_eq!(state.metrics().row_height(), ui_px(28.0));
        assert!(!state.visible_empty());
    }

    #[test]
    fn virtualized_list_navigation_stays_inside_range() {
        let items = (0..12)
            .map(|index| VirtualizedListStateItem::new(format!("item-{index}"), index.to_string()))
            .collect::<Vec<_>>();
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            items,
            Some("item-6"),
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            Some(4),
        );

        assert_eq!(state.navigation_target("home"), Some(0));
        assert_eq!(state.navigation_target("end"), Some(11));
        assert_eq!(state.navigation_target("up"), Some(5));
        assert_eq!(state.navigation_target("down"), Some(7));
        assert_eq!(state.navigation_target("pageup"), Some(2));
        assert_eq!(state.navigation_target("pagedown"), Some(10));
    }

    #[test]
    fn virtualized_list_navigation_skips_disabled_rows() {
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("beta", "Beta").disabled(true),
                VirtualizedListStateItem::new("gamma", "Gamma"),
            ],
            Some("alpha"),
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            Some(2),
        );

        assert_eq!(state.navigation_target("down"), Some(2));
        assert_eq!(state.navigation_target("end"), Some(2));
    }

    #[test]
    fn virtualized_list_empty_or_disabled_state_has_no_targets() {
        let empty = VirtualizedListState::resolve(
            Size::Medium,
            false,
            Vec::<VirtualizedListStateItem>::new(),
            None,
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            None,
        );
        let disabled = VirtualizedListState::resolve(
            Size::Medium,
            true,
            (0..10).map(|index| {
                VirtualizedListStateItem::new(format!("item-{index}"), index.to_string())
            }),
            Some("item-2"),
            ["item-2"],
            VirtualizedListSelectionMode::Single,
            None,
        );

        assert!(empty.visible_empty());
        assert_eq!(empty.active_index(), None);
        assert_eq!(empty.navigation_target("down"), None);
        assert_eq!(disabled.active_index(), None);
        assert_eq!(disabled.selected_index(), None);
        assert_eq!(disabled.activation_for_key("enter"), None);
    }

    #[test]
    fn virtualized_list_duplicate_keys_are_not_semantic_targets() {
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("duplicate", "First duplicate"),
                VirtualizedListStateItem::new("duplicate", "Second duplicate"),
                VirtualizedListStateItem::new("tail", "Tail"),
            ],
            Some("duplicate"),
            ["duplicate"],
            VirtualizedListSelectionMode::Single,
            Some(3),
        );

        assert_eq!(state.active_key(), Some("tail"));
        assert_eq!(state.active_index(), Some(2));
        assert!(state.selected_keys().is_empty());
        assert_eq!(
            state.scroll_target_for_key(
                "duplicate",
                VirtualizedListScrollStrategy::Nearest,
                ui_px(84.0),
                UiPx::ZERO,
            ),
            VirtualizedListRevealResult::NotSelectable("duplicate".to_owned())
        );
    }

    #[test]
    fn virtualized_list_state_resolves_selection_by_key_after_reorder() {
        let items = [
            VirtualizedListStateItem::new("alpha", "Alpha"),
            VirtualizedListStateItem::new("beta", "Beta"),
            VirtualizedListStateItem::new("gamma", "Gamma"),
        ];
        let reordered = [items[2].clone(), items[0].clone(), items[1].clone()];

        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            reordered,
            Some("gamma"),
            ["beta"],
            VirtualizedListSelectionMode::Multiple,
            Some(3),
        );

        assert_eq!(state.active_key(), Some("gamma"));
        assert_eq!(state.active_index(), Some(0));
        assert_eq!(state.selected_keys(), ["beta"]);
        assert!(state.selected_key_set().contains("beta"));
        assert_eq!(state.selected_indices(), [2]);
    }

    #[test]
    fn virtualized_list_multi_select_space_toggles_and_enter_activates() {
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("beta", "Beta"),
            ],
            Some("beta"),
            ["alpha"],
            VirtualizedListSelectionMode::Multiple,
            Some(2),
        );

        let change = state
            .selection_change_for_key("space")
            .expect("space should toggle selection in multi-select mode");
        assert_eq!(change.changed_key(), "beta");
        assert_eq!(change.selected_keys(), ["alpha", "beta"]);
        assert_eq!(state.activation_for_key("space"), None);

        let activation = state
            .activation_for_key("enter")
            .expect("enter should activate the active key");
        assert_eq!(activation.key(), "beta");
        assert_eq!(activation.index(), 1);
        assert_eq!(activation.text_value(), "Beta");
    }

    #[test]
    fn virtualized_list_scroll_to_key_reports_reveal_result() {
        let state = VirtualizedListState::resolve(
            Size::Small,
            false,
            [
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("beta", "Beta").disabled(true),
                VirtualizedListStateItem::new("gamma", "Gamma"),
            ],
            Some("alpha"),
            ["alpha"],
            VirtualizedListSelectionMode::Single,
            Some(2),
        )
        .with_metrics(VirtualizedListMetrics::from_size(Size::Small).with_row_height(ui_px(28.0)));

        assert_eq!(
            state.scroll_target_for_key(
                "beta",
                VirtualizedListScrollStrategy::Top,
                ui_px(56.0),
                UiPx::ZERO,
            ),
            VirtualizedListRevealResult::Revealed(VirtualizedListRevealTarget::new(
                "beta",
                1,
                ui_px(28.0),
                false
            ))
        );
        assert_eq!(
            state.scroll_target_for_key(
                "missing",
                VirtualizedListScrollStrategy::Top,
                ui_px(56.0),
                UiPx::ZERO,
            ),
            VirtualizedListRevealResult::NotFound("missing".to_owned())
        );
    }

    #[test]
    fn virtualized_list_scroll_strategy_labels_are_stable() {
        assert_eq!(VirtualizedListScrollStrategy::Nearest.as_str(), "nearest");
        assert_eq!(VirtualizedListScrollStrategy::Top.as_str(), "top");
        assert_eq!(VirtualizedListScrollStrategy::Center.as_str(), "center");
        assert_eq!(VirtualizedListScrollStrategy::Bottom.as_str(), "bottom");
    }

    #[test]
    fn virtualized_list_behavior_snapshot_preserves_roles_metadata_and_keys() {
        let items = vec![
            VirtualizedListItemDescriptor::new("root", "Root"),
            VirtualizedListItemDescriptor::new("duplicate", "First"),
            VirtualizedListItemDescriptor::new("duplicate", "Second").disabled(true),
            VirtualizedListItemDescriptor::new("tail", "Tail"),
        ];
        let snapshot = VirtualizedList::new("virtualized-list", "Virtualized list", items)
            .with_size(Size::Small)
            .default_active_key("tail")
            .default_selected_key("root")
            .viewport_item_count(2)
            .behavior_snapshot_with_viewport(ui_px(56.0), ui_px(56.0));

        assert_eq!(snapshot.role(), Role::ListBox);
        assert_eq!(snapshot.row_role(), Role::ListBoxOption);
        assert_eq!(snapshot.list_id(), "virtualized-list");
        assert_eq!(snapshot.label(), "Virtualized list");
        assert_eq!(snapshot.visible_row_count(), 2);
        assert_eq!(snapshot.overscan_count(), 4);
        assert_eq!(snapshot.rows().len(), 4);
        assert_eq!(snapshot.rows()[0].item().key(), "root");
        assert_eq!(snapshot.rows()[1].render_key(), "1:duplicate");
        assert_eq!(snapshot.rows()[2].render_key(), "2:duplicate");
        assert!(snapshot.rows()[0].selected());
        assert!(snapshot.rows()[2].disabled());
        assert!(snapshot.rows()[3].active());
        assert_eq!(snapshot.rows()[2].position_in_set(), Some(3));
        assert_eq!(snapshot.rows()[2].size_of_set(), 4);
        assert_eq!(snapshot.rows()[2].virtual_start(), ui_px(56.0));
        assert_eq!(snapshot.rows()[2].virtual_size(), ui_px(28.0));
        assert!(snapshot.active_row().is_some());
        assert!(snapshot.selected_row().is_some());
        assert_eq!(
            snapshot
                .rows()
                .iter()
                .map(|row| row.render_key())
                .collect::<Vec<_>>(),
            ["root", "1:duplicate", "2:duplicate", "tail"]
        );
    }

    #[test]
    fn virtualized_list_typed_item_snapshot_preserves_anatomy() {
        let snapshot = VirtualizedList::new(
            "typed-list",
            "Typed list",
            [
                VirtualizedListItemDescriptor::item("release-42", "Release 42")
                    .secondary_text("Platform / Ready")
                    .with_text_value("release forty two platform ready")
                    .leading_metadata("UI")
                    .trailing_metadata("12 files")
                    .badge("Ready")
                    .status("Verified"),
            ],
        )
        .default_active_key("release-42")
        .default_selected_key("release-42")
        .behavior_snapshot();
        let row = &snapshot.rows()[0];

        assert_eq!(row.kind(), VirtualizedListRowKind::Item);
        assert_eq!(row.label(), "Release 42");
        assert_eq!(row.secondary_text(), Some("Platform / Ready"));
        assert_eq!(row.text_value(), "release forty two platform ready");
        assert_eq!(row.leading_metadata(), Some("UI"));
        assert_eq!(row.trailing_metadata(), Some("12 files"));
        assert_eq!(row.badge(), Some("Ready"));
        assert_eq!(row.status(), Some("Verified"));
        assert_eq!(row.position_in_set(), Some(1));
        assert_eq!(row.size_of_set(), 1);
        assert_eq!(
            snapshot
                .state()
                .activation_for_key("enter")
                .map(|activation| activation.text_value().to_owned()),
            Some("release forty two platform ready".to_owned())
        );
    }

    #[test]
    fn virtualized_list_sections_and_separators_are_not_selectable_options() {
        let snapshot = VirtualizedList::new(
            "sectioned-list",
            "Sectioned list",
            [
                VirtualizedListItemDescriptor::section("recent", "Recent"),
                VirtualizedListItemDescriptor::new("alpha", "Alpha"),
                VirtualizedListItemDescriptor::separator("split"),
                VirtualizedListItemDescriptor::new("beta", "Beta").disabled_reason("Offline"),
            ],
        )
        .default_active_key("recent")
        .default_selected_keys(["recent", "beta"])
        .behavior_snapshot();

        assert_eq!(snapshot.state().active_key(), Some("alpha"));
        assert!(snapshot.state().selected_keys().is_empty());
        assert_eq!(snapshot.rows()[0].kind(), VirtualizedListRowKind::Section);
        assert_eq!(snapshot.rows()[0].role(), Role::Group);
        assert_eq!(snapshot.rows()[0].position_in_set(), None);
        assert_eq!(snapshot.rows()[0].size_of_set(), 2);
        assert_eq!(snapshot.rows()[1].position_in_set(), Some(1));
        assert_eq!(snapshot.rows()[2].kind(), VirtualizedListRowKind::Separator);
        assert_eq!(snapshot.rows()[2].role(), Role::Separator);
        assert_eq!(snapshot.rows()[2].position_in_set(), None);
        assert_eq!(snapshot.rows()[3].disabled_reason(), Some("Offline"));
        assert_eq!(snapshot.rows()[3].position_in_set(), Some(2));
        assert_eq!(
            snapshot.state().scroll_target_for_key(
                "recent",
                VirtualizedListScrollStrategy::Nearest,
                ui_px(84.0),
                UiPx::ZERO,
            ),
            VirtualizedListRevealResult::NotSelectable("recent".to_owned())
        );
    }

    #[test]
    fn virtualized_list_status_rows_suppress_activation_and_expose_roles() {
        let loading = VirtualizedList::new(
            "loading-list",
            "Loading list",
            [VirtualizedListItemDescriptor::loading(
                "loading",
                "Loading releases",
            )],
        )
        .default_active_key("loading")
        .behavior_snapshot();
        let empty = VirtualizedList::new(
            "empty-list",
            "Empty list",
            [VirtualizedListItemDescriptor::empty("empty", "No releases")],
        )
        .behavior_snapshot();
        let error = VirtualizedList::new(
            "error-list",
            "Error list",
            [VirtualizedListItemDescriptor::error(
                "error",
                "Failed to load",
            )],
        )
        .behavior_snapshot();

        assert!(!loading.state().visible_empty());
        assert_eq!(loading.state().active_key(), None);
        assert_eq!(loading.state().activation_for_key("enter"), None);
        assert_eq!(loading.rows()[0].role(), Role::ProgressIndicator);
        assert_eq!(loading.rows()[0].position_in_set(), None);
        assert_eq!(loading.rows()[0].size_of_set(), 0);
        assert_eq!(empty.rows()[0].role(), Role::Section);
        assert_eq!(error.rows()[0].role(), Role::AlertDialog);
    }

    #[test]
    fn virtualized_list_measured_mode_restores_snapshot_by_key() {
        let mut items = vec![
            VirtualizedListItemDescriptor::new("beta", "Beta"),
            VirtualizedListItemDescriptor::new("alpha", "Alpha"),
        ];
        items.extend((2..100).map(|index| {
            VirtualizedListItemDescriptor::new(format!("row-{index}"), format!("Row {index}"))
        }));
        let snapshot = VirtualizerSnapshot::new(
            ui_px(0.0),
            [
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("beta"),
                    ui_px(44.0),
                ),
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("removed"),
                    ui_px(96.0),
                ),
            ],
        );
        let behavior = VirtualizedList::new("measured-list", "Measured list", items)
            .row_height(ui_px(20.0))
            .overscan(2)
            .row_measure_mode(VirtualizedListRowMeasureMode::Measured)
            .virtualizer_snapshot(snapshot)
            .behavior_snapshot_with_viewport(ui_px(0.0), ui_px(48.0));

        assert_eq!(
            behavior.row_measure_mode(),
            VirtualizedListRowMeasureMode::Measured
        );
        assert_eq!(behavior.state().item_count(), 100);
        assert!(behavior.rendered_row_count() < behavior.state().item_count());
        assert_eq!(behavior.rows()[0].key(), "beta");
        assert_eq!(behavior.rows()[0].virtual_size(), ui_px(44.0));
        assert!(behavior.rows()[0].measured());
        assert_eq!(
            behavior
                .virtualizer_snapshot()
                .measurements()
                .iter()
                .map(|item| item.key().as_str())
                .collect::<Vec<_>>(),
            ["beta"]
        );
    }

    #[test]
    fn virtualized_list_measured_scroll_target_uses_snapshot_sizes() {
        let state = VirtualizedListState::resolve(
            Size::Medium,
            false,
            [
                VirtualizedListStateItem::new("alpha", "Alpha"),
                VirtualizedListStateItem::new("beta", "Beta"),
                VirtualizedListStateItem::new("gamma", "Gamma"),
            ],
            Some("alpha"),
            std::iter::empty::<&str>(),
            VirtualizedListSelectionMode::Single,
            Some(2),
        )
        .with_metrics(VirtualizedListMetrics::from_size(Size::Medium).with_row_height(ui_px(20.0)));
        let exact_snapshot = VirtualizerSnapshot::new(
            ui_px(0.0),
            [
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("alpha"),
                    ui_px(10.0),
                ),
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("beta"),
                    ui_px(50.0),
                ),
                open_gpui_ui_core::VirtualizerSnapshotItem::new(
                    VirtualizerItemKey::new("gamma"),
                    ui_px(30.0),
                ),
            ],
        );

        assert_eq!(
            state.scroll_target_for_key_with_snapshot(
                "beta",
                VirtualizedListScrollStrategy::Top,
                ui_px(30.0),
                UiPx::ZERO,
                &exact_snapshot,
            ),
            VirtualizedListRevealResult::Revealed(VirtualizedListRevealTarget::new(
                "beta",
                1,
                ui_px(10.0),
                false,
            ))
        );
        assert_eq!(
            state.scroll_target_for_key_with_snapshot(
                "beta",
                VirtualizedListScrollStrategy::Center,
                ui_px(30.0),
                UiPx::ZERO,
                &exact_snapshot,
            ),
            VirtualizedListRevealResult::Revealed(VirtualizedListRevealTarget::new(
                "beta",
                1,
                ui_px(20.0),
                false,
            ))
        );
        let estimated_snapshot = VirtualizerSnapshot::new(
            ui_px(0.0),
            [open_gpui_ui_core::VirtualizerSnapshotItem::new(
                VirtualizerItemKey::new("alpha"),
                ui_px(10.0),
            )],
        );
        assert_eq!(
            state.scroll_target_for_key_with_snapshot(
                "beta",
                VirtualizedListScrollStrategy::Top,
                ui_px(30.0),
                UiPx::ZERO,
                &estimated_snapshot,
            ),
            VirtualizedListRevealResult::Estimated(VirtualizedListRevealTarget::new(
                "beta",
                1,
                ui_px(10.0),
                true,
            ))
        );
    }

    #[test]
    fn virtualized_list_row_measure_mode_labels_are_stable() {
        assert_eq!(VirtualizedListRowMeasureMode::Fixed.as_str(), "fixed");
        assert_eq!(VirtualizedListRowMeasureMode::Measured.as_str(), "measured");
        assert!(!VirtualizedListRowMeasureMode::Fixed.measured());
        assert!(VirtualizedListRowMeasureMode::Measured.measured());
    }

    #[test]
    fn virtualized_list_row_context_carries_custom_renderer_invariants() {
        let items = [
            VirtualizedListItemDescriptor::section("recent", "Recent"),
            VirtualizedListItemDescriptor::new("alpha", "Alpha"),
            VirtualizedListItemDescriptor::new("beta", "Beta").disabled_reason("Offline"),
            VirtualizedListItemDescriptor::empty("empty", "No results"),
        ];
        let state = VirtualizedListState::resolve(
            Size::Small,
            false,
            items.iter().map(VirtualizedListStateItem::from),
            Some("alpha"),
            ["alpha"],
            VirtualizedListSelectionMode::Single,
            Some(4),
        )
        .with_metrics(VirtualizedListMetrics::from_size(Size::Small).with_row_height(ui_px(28.0)));
        let plan = VirtualizedListRenderPlan::resolve(
            "custom-list",
            "Custom list",
            state,
            &items,
            VirtualizedListRowMeasureMode::Fixed,
            &BTreeMap::new(),
            None,
            UiPx::ZERO,
            ui_px(112.0),
        );
        let contexts = plan.row_contexts();

        assert_eq!(contexts.len(), plan.rows().len());
        assert_eq!(contexts[0].key(), "recent");
        assert_eq!(contexts[0].kind(), VirtualizedListRowKind::Section);
        assert_eq!(contexts[0].role(), Role::Group);
        assert_eq!(contexts[0].position_in_set(), None);
        assert_eq!(
            contexts[0].row_measure_mode(),
            VirtualizedListRowMeasureMode::Fixed
        );
        assert_eq!(contexts[1].key(), "alpha");
        assert!(contexts[1].active());
        assert!(contexts[1].selected());
        assert_eq!(contexts[1].position_in_set(), Some(1));
        assert_eq!(contexts[1].size_of_set(), 2);
        assert_eq!(contexts[1].virtual_start(), ui_px(28.0));
        assert_eq!(contexts[1].virtual_size(), ui_px(28.0));
        assert_eq!(contexts[2].disabled_reason(), Some("Offline"));
        assert_eq!(contexts[3].kind(), VirtualizedListRowKind::Empty);
        assert!(!contexts[3].selectable());
    }

    #[test]
    fn virtualized_list_scroll_target_applies_alignment_strategies() {
        let row_height = ui_px(32.0);
        let viewport_extent = ui_px(96.0);
        let current = ui_px(320.0);

        assert_eq!(
            virtualized_list_scroll_target(
                VirtualizedListScrollStrategy::Top,
                10,
                100,
                row_height,
                viewport_extent,
                current,
            ),
            ui_px(320.0)
        );
        assert_eq!(
            virtualized_list_scroll_target(
                VirtualizedListScrollStrategy::Center,
                10,
                100,
                row_height,
                viewport_extent,
                current,
            ),
            ui_px(288.0)
        );
        assert_eq!(
            virtualized_list_scroll_target(
                VirtualizedListScrollStrategy::Bottom,
                10,
                100,
                row_height,
                viewport_extent,
                current,
            ),
            ui_px(256.0)
        );
        assert_eq!(
            virtualized_list_scroll_target(
                VirtualizedListScrollStrategy::Nearest,
                10,
                100,
                row_height,
                viewport_extent,
                current,
            ),
            ui_px(320.0)
        );
        assert_eq!(
            virtualized_list_scroll_target(
                VirtualizedListScrollStrategy::Nearest,
                10,
                100,
                row_height,
                viewport_extent,
                ui_px(0.0),
            ),
            ui_px(256.0)
        );
    }
}
