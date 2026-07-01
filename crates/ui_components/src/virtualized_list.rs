//! Renderer-neutral state for virtualized list surfaces.

use crate::a11y::UiA11yElementExt;
use crate::geometry::{gpui_px_from_ui, ui_px_from_gpui};
use crate::roving_focus::paged_navigation_target;
use crate::row_window::RowWindow;
use crate::scroll_area::ScrollArea;
use open_gpui::prelude::*;
use open_gpui::{
    App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement, Styled,
    Window, div, point, px, rgb,
};
#[cfg(test)]
use open_gpui_ui_core::ui_px;
use open_gpui_ui_core::{
    Role, Sizable, Size, UiPx, VirtualizerItemKey, VirtualizerItemMeasurement,
    VirtualizerResolvedState, VirtualizerState,
};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

type VirtualizedListActivationHandler =
    Rc<dyn Fn(VirtualizedListActivation, &mut Window, &mut App)>;

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

/// Public behavior snapshot for one virtualized-list row.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListRowBehaviorSnapshot {
    item: VirtualizedListItemDescriptor,
    render_key: String,
    index: usize,
    position_in_set: usize,
    size_of_set: usize,
    virtual_start: UiPx,
    virtual_size: UiPx,
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

    /// Returns the virtual row start offset.
    pub const fn virtual_start(&self) -> UiPx {
        self.virtual_start
    }

    /// Returns the virtual row size.
    pub const fn virtual_size(&self) -> UiPx {
        self.virtual_size
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

/// Public behavior snapshot for a concrete virtualized list.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualizedListBehaviorSnapshot {
    list_id: String,
    label: String,
    state: VirtualizedListState,
    metrics: VirtualizedListMetrics,
    total_size: UiPx,
    viewport_extent: UiPx,
    scroll_offset: UiPx,
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
            total_size: plan.virtualizer().total_size(),
            viewport_extent: plan.virtualizer().viewport_extent(),
            scroll_offset: plan.virtualizer().scroll_offset(),
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
pub(crate) struct VirtualizedListRenderPlan {
    list_id: String,
    label: String,
    state: VirtualizedListState,
    metrics: VirtualizedListMetrics,
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
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> Self {
        let metrics = state.metrics();
        let state = VirtualizedListState::resolve(
            state.size(),
            state.disabled(),
            items.len(),
            state.active_index(),
            state.selected_index(),
            Some(state.viewport_item_count()),
        )
        .with_metrics(metrics);
        let metrics = state.metrics();
        let viewport_extent = resolve_viewport_extent(&state, viewport_extent);
        let duplicate_keys = duplicate_item_keys(items);
        let virtualizer = VirtualizerState::new(items.len(), metrics.row_height())
            .with_viewport_extent(viewport_extent)
            .with_overscan(metrics.overscan_count())
            .with_scroll_offset(nonnegative_px(scroll_offset))
            .resolve_fixed_window(|index| {
                let item = &items[index];
                VirtualizerItemKey::new(virtualized_list_render_key(item, index, &duplicate_keys))
            });
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
                    state.item_count(),
                    &state,
                )
            })
            .collect();

        Self {
            list_id: list_id.into(),
            label: label.into(),
            state,
            metrics,
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
    scroll_handle: ScrollHandle,
    focus_handle: FocusHandle,
    active_index: Option<usize>,
    selected_index: Option<usize>,
    pending_scroll_to_active: Option<usize>,
}

/// A concrete GPUI virtualized list renderer.
#[derive(IntoElement)]
pub struct VirtualizedList {
    id: String,
    label: SharedString,
    items: Arc<[VirtualizedListItemDescriptor]>,
    size: Size,
    disabled: bool,
    active_index: Option<usize>,
    selected_index: Option<usize>,
    viewport_item_count: usize,
    metrics: VirtualizedListMetrics,
    on_activate: Option<VirtualizedListActivationHandler>,
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
            active_index: None,
            selected_index: None,
            viewport_item_count: DEFAULT_VIRTUALIZED_LIST_VIEWPORT_ITEM_COUNT,
            metrics: VirtualizedListMetrics::from_size(size),
            on_activate: None,
        }
    }

    /// Marks the list as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies the default active item index for adapter-owned runtime state.
    pub fn default_active_index(mut self, index: usize) -> Self {
        self.active_index = Some(index);
        self
    }

    /// Applies the default selected item index for adapter-owned runtime state.
    pub fn default_selected_index(mut self, index: usize) -> Self {
        self.selected_index = Some(index);
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

    /// Returns resolved renderer-neutral list state from the builder seed.
    pub fn state(&self) -> VirtualizedListState {
        self.resolved_state(
            self.active_index,
            self.selected_index,
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
            self.active_index,
            self.selected_index,
            self.viewport_item_count,
        );
        VirtualizedListRenderPlan::resolve(
            self.id.clone(),
            self.label.to_string(),
            state,
            self.items.as_ref(),
            scroll_offset,
            viewport_extent,
        )
    }

    fn resolved_state(
        &self,
        active_index: Option<usize>,
        selected_index: Option<usize>,
        viewport_item_count: usize,
    ) -> VirtualizedListState {
        VirtualizedListState::resolve(
            self.size,
            self.disabled,
            self.items.len(),
            active_index,
            selected_index,
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
            scroll_handle: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
            active_index: self.active_index,
            selected_index: self.selected_index,
            pending_scroll_to_active: None,
        });
        let runtime_state = runtime.read(cx).clone();
        let scroll_handle = runtime_state.scroll_handle.clone();
        let focus_handle = runtime_state.focus_handle.clone();
        let viewport_extent = ui_px_from_gpui(scroll_handle.bounds().size.height);
        let viewport_item_count = resolve_viewport_item_count(
            self.metrics.row_height(),
            viewport_extent,
            self.viewport_item_count,
        );
        let active_index = runtime_state.active_index.or(self.active_index);
        let selected_index = runtime_state.selected_index.or(self.selected_index);
        let state = self.resolved_state(active_index, selected_index, viewport_item_count);
        if let Some(pending_scroll_to_active) = runtime_state.pending_scroll_to_active {
            scroll_active_index(&scroll_handle, &state, pending_scroll_to_active);
            runtime.update(cx, |runtime, _| {
                runtime.pending_scroll_to_active = None;
            });
        }
        let scroll_offset =
            UiPx::new((-ui_px_from_gpui(scroll_handle.offset().y).as_f32()).max(0.0));
        let plan = VirtualizedListRenderPlan::resolve(
            self.id.clone(),
            self.label.to_string(),
            state.clone(),
            self.items.as_ref(),
            scroll_offset,
            viewport_extent,
        );
        let on_activate = self.on_activate.clone();
        let list_state = plan.state().clone();
        let row_role = plan.row_role();
        let rows = plan.rows().to_vec();
        let list_id = plan.list_id().to_owned();
        let scroll_viewport_id = format!("virtualized-list:{}:viewport", plan.list_id());
        let root_click_state = list_state.clone();

        runtime.update(cx, |runtime, _| {
            if runtime.active_index != list_state.active_index() {
                runtime.active_index = list_state.active_index();
                runtime.pending_scroll_to_active = list_state.active_index();
            }
            if runtime.selected_index != list_state.selected_index() {
                runtime.selected_index = list_state.selected_index();
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
                let items = self.items.clone();
                let plan_state = list_state.clone();
                move |event: &KeyDownEvent, window, cx| {
                    handle_virtualized_list_key_down(
                        &plan_state,
                        items.as_ref(),
                        runtime.clone(),
                        scroll_handle.clone(),
                        on_activate.clone(),
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
                            row_role,
                            runtime.clone(),
                            focus_handle,
                            on_activate,
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
    row_role: Role,
    runtime: Entity<VirtualizedListRuntime>,
    focus_handle: FocusHandle,
    on_activate: Option<VirtualizedListActivationHandler>,
) -> impl IntoElement {
    let rows = rows.to_vec();
    let list_id = list_id.to_owned();
    let body_id = format!("virtualized-list:{list_id}:body");

    div()
        .id(body_id.clone())
        .debug_selector({
            let body_id = body_id.clone();
            move || body_id.clone()
        })
        .relative()
        .w_full()
        .h(gpui_px_from_ui(total_size))
        .children(rows.into_iter().map(move |row| {
            render_virtualized_list_row(
                list_id.clone(),
                row,
                row_role,
                runtime.clone(),
                focus_handle.clone(),
                on_activate.clone(),
            )
        }))
}

fn render_virtualized_list_row(
    list_id: String,
    row: VirtualizedListRowRenderPlan,
    row_role: Role,
    runtime: Entity<VirtualizedListRuntime>,
    focus_handle: FocusHandle,
    on_activate: Option<VirtualizedListActivationHandler>,
) -> impl IntoElement {
    let render_key = row.render_key().to_owned();
    let row_index = row.index();
    let activation = VirtualizedListActivation::new(row_index);
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

    div()
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
        .h(gpui_px_from_ui(row.virtual_size()))
        .min_w(px(0.0))
        .flex()
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(rgb(0xe2e4dc))
        .bg(row_background)
        .text_color(text_color)
        .ui_role(row_role)
        .aria_selected(row.selected())
        .aria_disabled(row.disabled())
        .aria_position_in_set(row.position_in_set())
        .when(!row.disabled(), |this| {
            this.cursor_pointer().hover(|style| style.bg(rgb(0xeef2f7)))
        })
        .when(!row.disabled(), |this| {
            let runtime = runtime.clone();
            let focus_handle = focus_handle.clone();
            let on_activate = on_activate.clone();
            this.on_click(move |_event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                window.prevent_default();
                runtime.update(cx, |runtime, _| {
                    runtime.active_index = Some(row_index);
                    runtime.selected_index = Some(row_index);
                    runtime.pending_scroll_to_active = None;
                });
                focus_handle.focus(window, cx);
                if let Some(on_activate) = on_activate.as_ref() {
                    on_activate(activation, window, cx);
                }
            })
        })
        .child(row.label().to_owned())
}

fn handle_virtualized_list_key_down(
    state: &VirtualizedListState,
    items: &[VirtualizedListItemDescriptor],
    runtime: Entity<VirtualizedListRuntime>,
    scroll_handle: ScrollHandle,
    on_activate: Option<VirtualizedListActivationHandler>,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if state.disabled() || state.visible_empty() {
        return;
    }

    let key = event.keystroke.key.as_str();
    if let Some(target) = state.navigation_target(key) {
        cx.stop_propagation();
        window.prevent_default();
        runtime.update(cx, |runtime, _| {
            runtime.active_index = Some(target);
            runtime.pending_scroll_to_active = Some(target);
        });
        scroll_active_index(&scroll_handle, state, target);
        return;
    }

    if let Some(activation) = state.activation_for_key(key) {
        let Some(item) = items.get(activation.index()) else {
            return;
        };
        if item.disabled_state() {
            return;
        }

        cx.stop_propagation();
        window.prevent_default();
        runtime.update(cx, |runtime, _| {
            runtime.active_index = Some(activation.index());
            runtime.selected_index = Some(activation.index());
            runtime.pending_scroll_to_active = Some(activation.index());
        });
        scroll_active_index(&scroll_handle, state, activation.index());
        if let Some(on_activate) = on_activate.as_ref() {
            on_activate(activation, window, cx);
        }
    }
}

fn scroll_active_index(scroll_handle: &ScrollHandle, state: &VirtualizedListState, index: usize) {
    let viewport_extent =
        resolve_viewport_extent(state, ui_px_from_gpui(scroll_handle.bounds().size.height));
    let current_scroll_offset =
        UiPx::new((-ui_px_from_gpui(scroll_handle.offset().y).as_f32()).max(0.0));
    let target = virtualized_list_scroll_target(
        VirtualizedListScrollStrategy::Nearest,
        index,
        state.item_count(),
        state.metrics().row_height(),
        viewport_extent,
        current_scroll_offset,
    );
    scroll_handle.set_offset(point(px(0.0), -gpui_px_from_ui(target)));
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
    fn virtualized_list_behavior_snapshot_preserves_roles_metadata_and_keys() {
        let items = vec![
            VirtualizedListItemDescriptor::new("root", "Root"),
            VirtualizedListItemDescriptor::new("duplicate", "First"),
            VirtualizedListItemDescriptor::new("duplicate", "Second").disabled(true),
            VirtualizedListItemDescriptor::new("tail", "Tail"),
        ];
        let snapshot = VirtualizedList::new("virtualized-list", "Virtualized list", items)
            .with_size(Size::Small)
            .default_active_index(2)
            .default_selected_index(1)
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
        assert!(snapshot.rows()[1].selected());
        assert!(snapshot.rows()[2].disabled());
        assert!(snapshot.rows()[2].active());
        assert_eq!(snapshot.rows()[2].position_in_set(), 3);
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
