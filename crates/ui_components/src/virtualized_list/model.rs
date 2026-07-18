use crate::choice::{normalize_query, normalized_text_starts_with};
use crate::roving_focus::paged_navigation_target;
use crate::scroll_surface::{
    ScrollSurfaceRevealStrategy, fixed_row_scroll_target, row_geometry_scroll_target,
};
use open_gpui_ui_core::{Size, UiPx, VirtualizerItemGeometry, VirtualizerSnapshot};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::descriptor::{VirtualizedListItemDescriptor, VirtualizedListRowKind};
use super::style::{VirtualizedListMetrics, nonnegative_px};

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
        Self::new(item.key().to_owned(), item.text_value().to_owned())
            .row_kind(item.kind())
            .disabled(item.disabled_state())
    }
}

impl From<&VirtualizedListItemDescriptor> for VirtualizedListStateItem {
    fn from(item: &VirtualizedListItemDescriptor) -> Self {
        item.state_item()
    }
}

/// Resolved item target for key-first virtualized-list actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VirtualizedListItemTarget {
    key: String,
    index: usize,
    disabled: bool,
    text_value: String,
}

impl VirtualizedListItemTarget {
    /// Creates an item target.
    pub(super) fn new(
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
    pub(super) fn key(&self) -> &str {
        &self.key
    }

    /// Returns the resolved item index.
    /// Returns whether the target is disabled.
    pub(super) const fn disabled(&self) -> bool {
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

    pub(super) fn from_target(target: VirtualizedListItemTarget, selected: bool) -> Self {
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

    pub(super) fn selected_key_set(&self) -> BTreeSet<String> {
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
    /// The key is present more than once and is not a stable reveal target.
    DuplicateKey(String),
    /// The key is present but belongs to a disabled item row.
    Disabled(String),
    /// The key is present but belongs to a non-selectable status row.
    StatusRow(String),
    /// The key is present but belongs to a non-selectable structural row.
    StructuralRow(String),
    /// The key is present but cannot participate in reveal.
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
        self.navigation_target_from_key(key, self.active_key())
    }

    pub(crate) fn navigation_target_from_key(
        &self,
        key: &str,
        current_key: Option<&str>,
    ) -> Option<usize> {
        if self.disabled {
            return None;
        }

        let active_index = current_key
            .and_then(|key| self.selectable_index_for_key(key))
            .or(self.active_index)?;
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

    /// Returns the next selectable item whose text value starts with `query`.
    ///
    /// The scan starts after the current active row and wraps once. Disabled,
    /// structural, and duplicate-key rows do not participate in typeahead.
    pub fn typeahead_target(&self, query: &str) -> Option<&VirtualizedListStateItem> {
        let index = self.typeahead_target_index(query, self.active_key(), true)?;
        self.items.get(index)
    }

    pub(crate) fn typeahead_target_from_key(
        &self,
        query: &str,
        current_key: Option<&str>,
        search_after_current: bool,
    ) -> Option<&VirtualizedListStateItem> {
        let index = self.typeahead_target_index(query, current_key, search_after_current)?;
        self.items.get(index)
    }

    /// Returns activation payload for Enter or Space.
    pub fn activation_for_key(&self, key: &str) -> Option<VirtualizedListActivation> {
        self.activation_for_key_from_state(key, self.active_key(), &self.selected_keys)
    }

    pub(crate) fn activation_for_key_from_state(
        &self,
        key: &str,
        current_key: Option<&str>,
        selected_keys: &BTreeSet<String>,
    ) -> Option<VirtualizedListActivation> {
        if self.disabled {
            return None;
        }

        match (key, self.selection_mode) {
            ("enter", _) | ("space", VirtualizedListSelectionMode::Single) => {
                let target = self.active_target_from_key(current_key)?;
                Some(VirtualizedListActivation::from_target(
                    target,
                    current_key.is_some_and(|key| selected_keys.contains(key)),
                ))
            }
            _ => None,
        }
    }

    /// Returns selection change payload for selection keyboard commands.
    pub fn selection_change_for_key(&self, key: &str) -> Option<VirtualizedListSelectionChange> {
        self.selection_change_for_key_from_state(key, self.active_key(), &self.selected_keys)
    }

    pub(crate) fn selection_change_for_key_from_state(
        &self,
        key: &str,
        current_key: Option<&str>,
        selected_keys: &BTreeSet<String>,
    ) -> Option<VirtualizedListSelectionChange> {
        if key != "space" {
            return None;
        }

        let target = self.active_target_from_key(current_key)?;
        self.selection_change_for_target_from_selected(&target, selected_keys)
    }

    /// Returns replacement-style range selection for multi-select lists.
    pub fn range_selection_change(
        &self,
        anchor_key: Option<&str>,
        target_key: &str,
    ) -> Option<VirtualizedListSelectionChange> {
        self.range_selection_change_from_selected(anchor_key, target_key, &self.selected_keys)
    }

    pub(crate) fn range_selection_change_from_selected(
        &self,
        anchor_key: Option<&str>,
        target_key: &str,
        current_selected_keys: &BTreeSet<String>,
    ) -> Option<VirtualizedListSelectionChange> {
        if self.disabled || self.selection_mode != VirtualizedListSelectionMode::Multiple {
            return None;
        }

        let target_index = self.selectable_index_for_key(target_key)?;
        let anchor_index = self.range_anchor_index(anchor_key, target_index)?;
        let start = anchor_index.min(target_index);
        let end = anchor_index.max(target_index);
        let selected_keys = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                (index >= start
                    && index <= end
                    && is_selectable_state_item(item, &self.duplicate_keys))
                .then(|| item.key().to_owned())
            })
            .collect::<Vec<_>>();
        let selected_key_set = selected_keys.iter().cloned().collect::<BTreeSet<_>>();
        if selected_key_set == *current_selected_keys {
            return None;
        }

        Some(VirtualizedListSelectionChange::new(
            target_key.to_owned(),
            selected_keys,
        ))
    }

    /// Returns a key-based reveal target for fixed-height rows.
    pub fn scroll_target_for_key(
        &self,
        key: &str,
        strategy: VirtualizedListScrollStrategy,
        viewport_extent: UiPx,
        current_scroll_offset: UiPx,
    ) -> VirtualizedListRevealResult {
        let index = match self.reveal_index_for_key(key) {
            Ok(index) => index,
            Err(result) => return result,
        };
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
        let index = match self.reveal_index_for_key(key) {
            Ok(index) => index,
            Err(result) => return result,
        };

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

    fn typeahead_target_index(
        &self,
        query: &str,
        current_key: Option<&str>,
        search_after_current: bool,
    ) -> Option<usize> {
        if self.disabled {
            return None;
        }

        let query = normalize_query(query);
        if query.is_empty() || self.items.is_empty() {
            return None;
        }

        let selectable_indices = self.selectable_indices();
        if selectable_indices.is_empty() {
            return None;
        }

        let active_position = current_key
            .and_then(|key| self.selectable_index_for_key(key))
            .and_then(|active_index| {
                selectable_indices
                    .iter()
                    .position(|index| *index == active_index)
            });
        let start_position = active_position.map_or(0, |position| {
            if search_after_current {
                (position + 1) % selectable_indices.len()
            } else {
                position
            }
        });

        (0..selectable_indices.len())
            .map(|step| selectable_indices[(start_position + step) % selectable_indices.len()])
            .find(|index| {
                self.items.get(*index).is_some_and(|item| {
                    normalized_text_starts_with(item.text_value(), query.as_str())
                })
            })
    }

    pub(super) fn target_at_index(&self, index: usize) -> Option<VirtualizedListItemTarget> {
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

    fn active_target_from_key(
        &self,
        current_key: Option<&str>,
    ) -> Option<VirtualizedListItemTarget> {
        current_key
            .and_then(|key| self.selectable_index_for_key(key))
            .or(self.active_index)
            .and_then(|index| self.target_at_index(index))
    }

    pub(super) fn selectable_index_for_key(&self, key: &str) -> Option<usize> {
        state_item_index_by_unique_key(self.items.as_ref(), &self.duplicate_keys, key)
    }

    fn reveal_index_for_key(&self, key: &str) -> Result<usize, VirtualizedListRevealResult> {
        let matches = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| (item.key() == key).then_some(index))
            .collect::<Vec<_>>();

        let index = match matches.as_slice() {
            [] => return Err(VirtualizedListRevealResult::NotFound(key.to_owned())),
            [index] => *index,
            _ => return Err(VirtualizedListRevealResult::DuplicateKey(key.to_owned())),
        };

        let item = &self.items[index];
        match item.kind() {
            VirtualizedListRowKind::Item => {
                if item.disabled_state() {
                    Err(VirtualizedListRevealResult::Disabled(key.to_owned()))
                } else {
                    Ok(index)
                }
            }
            VirtualizedListRowKind::Loading
            | VirtualizedListRowKind::Empty
            | VirtualizedListRowKind::Error => {
                Err(VirtualizedListRevealResult::StatusRow(key.to_owned()))
            }
            VirtualizedListRowKind::Section | VirtualizedListRowKind::Separator => {
                Err(VirtualizedListRevealResult::StructuralRow(key.to_owned()))
            }
        }
    }

    pub(super) fn range_anchor_key(
        &self,
        anchor_key: Option<&str>,
        target_key: &str,
    ) -> Option<&str> {
        let target_index = self.selectable_index_for_key(target_key)?;
        let anchor_index = self.range_anchor_index(anchor_key, target_index)?;
        Some(self.items[anchor_index].key())
    }

    fn range_anchor_index(&self, anchor_key: Option<&str>, target_index: usize) -> Option<usize> {
        anchor_key
            .and_then(|key| self.selectable_index_for_key(key))
            .or_else(|| {
                self.active_index
                    .filter(|index| self.target_at_index(*index).is_some())
            })
            .or(Some(target_index))
    }

    pub(super) fn selection_change_for_target(
        &self,
        target: &VirtualizedListItemTarget,
    ) -> Option<VirtualizedListSelectionChange> {
        self.selection_change_for_target_from_selected(target, &self.selected_keys)
    }

    pub(super) fn selection_change_for_target_from_selected(
        &self,
        target: &VirtualizedListItemTarget,
        current_selected_keys: &BTreeSet<String>,
    ) -> Option<VirtualizedListSelectionChange> {
        if self.disabled || target.disabled() {
            return None;
        }

        let mut selected_keys = current_selected_keys.clone();
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

/// Resolves virtualized-list navigation for APG-style key names.
pub(super) fn virtualized_list_navigation_target(
    key: &str,
    current: usize,
    item_count: usize,
    viewport_item_count: usize,
) -> Option<usize> {
    paged_navigation_target(key, current, item_count, viewport_item_count)
}

/// Resolves a fixed-height scroll target for a virtualized list.
pub(super) fn virtualized_list_scroll_target(
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
    if items.is_empty() {
        return (UiPx::ZERO, true);
    }

    let target_index = target_index.min(items.len() - 1);
    let measurements_by_key = snapshot
        .measurements()
        .iter()
        .map(|item| (item.key().as_str(), nonnegative_px(item.size())))
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

    let target = row_geometry_scroll_target(
        scroll_surface_reveal_strategy(strategy),
        VirtualizerItemGeometry::new(target_start, target_size),
        cursor,
        viewport_extent,
        current_scroll_offset,
    );

    (target, estimated)
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

pub(super) fn virtualized_list_state_items(
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
