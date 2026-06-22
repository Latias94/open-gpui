//! Renderer-neutral state for virtualized list surfaces.

#[cfg(test)]
use open_gpui_ui_core::ui_px;
use open_gpui_ui_core::{
    Role, Size, UiPx, VirtualizerItemKey, VirtualizerItemMeasurement, VirtualizerResolvedState,
    VirtualizerState,
};
use std::collections::{BTreeMap, BTreeSet};

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

/// Pure descriptor for one virtualized list item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizedListItemDescriptor {
    key: String,
    label: String,
    disabled: bool,
}

impl VirtualizedListItemDescriptor {
    /// Creates a new item descriptor.
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Marks the item as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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

    /// Returns whether the item is disabled.
    pub const fn disabled_state(&self) -> bool {
        self.disabled
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
}

/// Resolved activation payload for a virtualized row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualizedListActivation {
    index: usize,
}

impl VirtualizedListActivation {
    /// Creates an activation payload for a visible item index.
    pub const fn new(index: usize) -> Self {
        Self { index }
    }

    /// Returns the activated item index.
    pub const fn index(self) -> usize {
        self.index
    }
}

/// Resolved virtualized-list state used by tests, adapters, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListState {
    size: Size,
    disabled: bool,
    item_count: usize,
    active_index: Option<usize>,
    selected_index: Option<usize>,
    viewport_item_count: usize,
    metrics: VirtualizedListMetrics,
}

impl VirtualizedListState {
    /// Resolves public state for a virtualized list.
    pub fn resolve(
        size: Size,
        disabled: bool,
        item_count: usize,
        active_index: Option<usize>,
        selected_index: Option<usize>,
        viewport_item_count: Option<usize>,
    ) -> Self {
        let selected_index = selected_index.and_then(|index| valid_index(index, item_count));
        let active_index = if disabled || item_count == 0 {
            None
        } else {
            active_index
                .and_then(|index| valid_index(index, item_count))
                .or(selected_index)
                .or(Some(0))
        };
        let selected_index = if disabled { None } else { selected_index };

        Self {
            size,
            disabled,
            item_count,
            active_index,
            selected_index,
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
    pub const fn item_count(&self) -> usize {
        self.item_count
    }

    /// Returns the active descendant index.
    pub const fn active_index(&self) -> Option<usize> {
        self.active_index
    }

    /// Returns the selected row index.
    pub const fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Returns the estimated number of rows visible in the viewport.
    pub const fn viewport_item_count(&self) -> usize {
        self.viewport_item_count
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> VirtualizedListMetrics {
        self.metrics
    }

    /// Returns the default viewport extent implied by the resolved metrics and viewport item count.
    pub fn viewport_extent(&self) -> UiPx {
        self.metrics.row_height() * self.viewport_item_count as f32
    }

    /// Returns whether the list has no items to render.
    pub const fn visible_empty(&self) -> bool {
        self.item_count == 0
    }

    /// Returns the target index for an APG-style navigation key.
    pub fn navigation_target(&self, key: &str) -> Option<usize> {
        if self.disabled {
            return None;
        }

        virtualized_list_navigation_target(
            key,
            self.active_index?,
            self.item_count,
            self.viewport_item_count,
        )
    }

    /// Returns activation payload for Enter or Space.
    pub fn activation_for_key(&self, key: &str) -> Option<VirtualizedListActivation> {
        if self.disabled || !matches!(key, "enter" | "space") {
            return None;
        }

        self.active_index.map(VirtualizedListActivation::new)
    }

    /// Clamps a requested item index into the list range.
    pub fn clamped_index(&self, index: usize) -> Option<usize> {
        valid_index(index, self.item_count).or_else(|| self.item_count.checked_sub(1))
    }
}

/// One resolved virtualized row in render order.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListRowRenderPlan {
    item: VirtualizedListItemDescriptor,
    render_key: String,
    index: usize,
    position_in_set: usize,
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
        size_of_set: usize,
        state: &VirtualizedListState,
    ) -> Self {
        let active = state.active_index() == Some(index);
        let selected = state.selected_index() == Some(index);
        let disabled = state.disabled() || item.disabled_state();

        Self {
            item,
            render_key,
            index,
            position_in_set: index + 1,
            size_of_set,
            measurement,
            active,
            selected,
            disabled,
            role: Role::ListBoxOption,
        }
    }

    /// Returns the source descriptor.
    pub fn item(&self) -> &VirtualizedListItemDescriptor {
        &self.item
    }

    /// Returns the stable item key.
    pub fn key(&self) -> &str {
        self.item.key()
    }

    /// Returns the visible item label.
    pub fn label(&self) -> &str {
        self.item.label()
    }

    /// Returns the stable render key.
    pub fn render_key(&self) -> &str {
        &self.render_key
    }

    /// Returns the zero-based item index.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the one-based position within the rendered set.
    pub const fn position_in_set(&self) -> usize {
        self.position_in_set
    }

    /// Returns the total set size.
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

    /// Returns the accessibility role for this row.
    pub const fn role(&self) -> Role {
        self.role
    }
}

/// Fully resolved render contract for a concrete virtualized list.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListRenderPlan {
    list_id: String,
    label: String,
    state: VirtualizedListState,
    metrics: VirtualizedListMetrics,
    virtualizer: VirtualizerResolvedState,
    rows: Vec<VirtualizedListRowRenderPlan>,
    role: Role,
    row_role: Role,
}

impl VirtualizedListRenderPlan {
    /// Resolves a render plan from renderer-neutral state and item descriptors.
    pub fn resolve(
        list_id: impl Into<String>,
        label: impl Into<String>,
        state: VirtualizedListState,
        items: impl IntoIterator<Item = VirtualizedListItemDescriptor>,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> Self {
        let descriptors = items.into_iter().collect::<Vec<_>>();
        let state = VirtualizedListState::resolve(
            state.size(),
            state.disabled(),
            descriptors.len(),
            state.active_index(),
            state.selected_index(),
            Some(state.viewport_item_count()),
        );
        let metrics = state.metrics();
        let viewport_extent = resolve_viewport_extent(&state, viewport_extent);
        let duplicate_keys = duplicate_item_keys(&descriptors);
        let virtualizer = VirtualizerState::new(descriptors.len(), metrics.row_height())
            .with_viewport_extent(viewport_extent)
            .with_overscan(metrics.overscan_count())
            .with_scroll_offset(nonnegative_px(scroll_offset))
            .resolve_fixed_window(|index| {
                let item = &descriptors[index];
                VirtualizerItemKey::new(virtualized_list_render_key(item, index, &duplicate_keys))
            });
        let rows = virtualizer
            .items()
            .iter()
            .filter_map(|measurement| {
                descriptors.get(measurement.index()).cloned().map(|item| {
                    let render_key =
                        virtualized_list_render_key(&item, measurement.index(), &duplicate_keys);
                    VirtualizedListRowRenderPlan::new(
                        item,
                        render_key,
                        measurement.index(),
                        measurement.clone(),
                        state.item_count(),
                        &state,
                    )
                })
            })
            .collect();

        Self {
            list_id: list_id.into(),
            label: label.into(),
            state,
            metrics,
            virtualizer,
            rows,
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

    /// Returns the resolved virtualizer state.
    pub const fn virtualizer(&self) -> &VirtualizerResolvedState {
        &self.virtualizer
    }

    /// Returns rows in render order.
    pub fn rows(&self) -> &[VirtualizedListRowRenderPlan] {
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

    /// Returns the number of rows rendered after overscan.
    pub fn rendered_row_count(&self) -> usize {
        self.rows.len()
    }

    /// Returns the number of rows visible before overscan.
    pub fn visible_row_count(&self) -> usize {
        self.virtualizer.visible_items().len()
    }

    /// Returns the overscan budget.
    pub const fn overscan_count(&self) -> usize {
        self.virtualizer.overscan()
    }

    /// Returns the active row, when one is resolved.
    pub fn active_row(&self) -> Option<&VirtualizedListRowRenderPlan> {
        self.rows.iter().find(|row| row.active())
    }

    /// Returns the selected row, when one is resolved.
    pub fn selected_row(&self) -> Option<&VirtualizedListRowRenderPlan> {
        self.rows.iter().find(|row| row.selected())
    }
}

/// Resolves virtualized-list navigation for APG-style key names.
pub fn virtualized_list_navigation_target(
    key: &str,
    current: usize,
    item_count: usize,
    viewport_item_count: usize,
) -> Option<usize> {
    if item_count == 0 || current >= item_count {
        return None;
    }

    match key {
        "home" => Some(0),
        "end" => item_count.checked_sub(1),
        "up" => Some(current.saturating_sub(1)),
        "down" => Some((current + 1).min(item_count - 1)),
        "pageup" => Some(current.saturating_sub(viewport_item_count.max(1))),
        "pagedown" => Some((current + viewport_item_count.max(1)).min(item_count - 1)),
        _ => None,
    }
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
    let row_height = nonnegative_px(row_height);
    let viewport_extent = nonnegative_px(viewport_extent);
    if item_count == 0 || row_height.as_f32() <= 0.0 {
        return UiPx::ZERO;
    }

    let target_index = target_index.min(item_count - 1);
    let total_size = row_height * item_count as f32;
    let max_scroll_offset = nonnegative_px(total_size - viewport_extent);
    let current_scroll_offset = nonnegative_px(current_scroll_offset).min(max_scroll_offset);
    let row_start = row_height * target_index as f32;
    let row_end = row_start + row_height;
    let target = match strategy {
        VirtualizedListScrollStrategy::Nearest => {
            let viewport_start = current_scroll_offset;
            let viewport_end = viewport_start + viewport_extent;
            if row_start < viewport_start {
                row_start
            } else if row_end > viewport_end {
                row_end - viewport_extent
            } else {
                viewport_start
            }
        }
        VirtualizedListScrollStrategy::Top => row_start,
        VirtualizedListScrollStrategy::Center => {
            let row_center = row_start + row_height.half();
            let viewport_center = viewport_extent.half();
            row_center - viewport_center
        }
        VirtualizedListScrollStrategy::Bottom => row_end - viewport_extent,
    };

    nonnegative_px(target).min(max_scroll_offset)
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

fn nonnegative_px(value: UiPx) -> UiPx {
    if value.as_f32() < 0.0 {
        UiPx::ZERO
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtualized_list_state_clamps_active_and_preserves_metrics() {
        let state =
            VirtualizedListState::resolve(Size::Small, false, 10, Some(12), Some(4), Some(5));

        assert_eq!(state.size(), Size::Small);
        assert_eq!(state.item_count(), 10);
        assert_eq!(state.active_index(), Some(4));
        assert_eq!(state.selected_index(), Some(4));
        assert_eq!(state.viewport_item_count(), 5);
        assert_eq!(state.metrics().row_height(), ui_px(28.0));
        assert!(!state.visible_empty());
    }

    #[test]
    fn virtualized_list_navigation_stays_inside_range() {
        let state = VirtualizedListState::resolve(Size::Medium, false, 12, Some(6), None, Some(4));

        assert_eq!(state.navigation_target("home"), Some(0));
        assert_eq!(state.navigation_target("end"), Some(11));
        assert_eq!(state.navigation_target("up"), Some(5));
        assert_eq!(state.navigation_target("down"), Some(7));
        assert_eq!(state.navigation_target("pageup"), Some(2));
        assert_eq!(state.navigation_target("pagedown"), Some(10));
    }

    #[test]
    fn virtualized_list_empty_or_disabled_state_has_no_targets() {
        let empty = VirtualizedListState::resolve(Size::Medium, false, 0, None, None, None);
        let disabled =
            VirtualizedListState::resolve(Size::Medium, true, 10, Some(2), Some(2), None);

        assert!(empty.visible_empty());
        assert_eq!(empty.active_index(), None);
        assert_eq!(empty.navigation_target("down"), None);
        assert_eq!(disabled.active_index(), None);
        assert_eq!(disabled.selected_index(), None);
        assert_eq!(disabled.activation_for_key("enter"), None);
    }

    #[test]
    fn virtualized_list_scroll_strategy_labels_are_stable() {
        assert_eq!(VirtualizedListScrollStrategy::Nearest.as_str(), "nearest");
        assert_eq!(VirtualizedListScrollStrategy::Top.as_str(), "top");
        assert_eq!(VirtualizedListScrollStrategy::Center.as_str(), "center");
        assert_eq!(VirtualizedListScrollStrategy::Bottom.as_str(), "bottom");
    }

    #[test]
    fn virtualized_list_render_plan_preserves_roles_metadata_and_keys() {
        let items = vec![
            VirtualizedListItemDescriptor::new("root", "Root"),
            VirtualizedListItemDescriptor::new("duplicate", "First"),
            VirtualizedListItemDescriptor::new("duplicate", "Second").disabled(true),
            VirtualizedListItemDescriptor::new("tail", "Tail"),
        ];
        let state = VirtualizedListState::resolve(Size::Small, false, 4, Some(2), Some(1), Some(2));
        let plan = VirtualizedListRenderPlan::resolve(
            "virtualized-list",
            "Virtualized list",
            state,
            items,
            ui_px(56.0),
            ui_px(56.0),
        );

        assert_eq!(plan.role(), Role::ListBox);
        assert_eq!(plan.row_role(), Role::ListBoxOption);
        assert_eq!(plan.list_id(), "virtualized-list");
        assert_eq!(plan.label(), "Virtualized list");
        assert_eq!(plan.visible_row_count(), 2);
        assert_eq!(plan.overscan_count(), 4);
        assert_eq!(plan.rendered_row_count(), 4);
        assert_eq!(plan.rows()[0].key(), "root");
        assert_eq!(plan.rows()[1].render_key(), "1:duplicate");
        assert_eq!(plan.rows()[2].render_key(), "2:duplicate");
        assert!(plan.rows()[1].selected());
        assert!(plan.rows()[2].disabled());
        assert!(plan.rows()[2].active());
        assert_eq!(plan.rows()[2].position_in_set(), 3);
        assert_eq!(plan.rows()[2].size_of_set(), 4);
        assert_eq!(plan.rows()[2].virtual_start(), ui_px(56.0));
        assert_eq!(plan.rows()[2].virtual_size(), ui_px(28.0));
        assert!(plan.active_row().is_some());
        assert!(plan.selected_row().is_some());
        assert_eq!(plan.virtualizer().items().len(), 4);
        assert_eq!(
            plan.virtualizer()
                .items()
                .iter()
                .map(|measurement| measurement.key().as_str())
                .collect::<Vec<_>>(),
            ["root", "1:duplicate", "2:duplicate", "tail"]
        );
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
