use open_gpui_ui_core::{
    Role, RowWindow, UiPx, VirtualizerItemKey, VirtualizerItemMeasurement,
    VirtualizerResolvedState, VirtualizerSnapshot, VirtualizerState,
};
use std::collections::{BTreeMap, BTreeSet};

use super::descriptor::{
    VirtualizedListItemDescriptor, VirtualizedListRowKind, VirtualizedListStatusKind,
};
use super::model::{VirtualizedListItemTarget, VirtualizedListState, virtualized_list_state_items};
use super::style::{VirtualizedListMetrics, nonnegative_px};

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
    pub(super) fn from_render_plan(row: &VirtualizedListRowRenderPlan) -> Self {
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

    /// Returns async/infinite status semantics for status rows.
    pub fn status_kind(&self) -> Option<VirtualizedListStatusKind> {
        self.item.status_kind()
    }

    /// Returns the retry command label for retry status rows.
    pub fn retry_action_label(&self) -> Option<&str> {
        self.item.retry_action_label_ref()
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
    pub(super) fn from_render_plan(
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

    /// Returns async/infinite status semantics for status rows.
    pub fn status_kind(&self) -> Option<VirtualizedListStatusKind> {
        self.item.status_kind()
    }

    /// Returns the retry command label for retry status rows.
    pub fn retry_action_label(&self) -> Option<&str> {
        self.item.retry_action_label_ref()
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

/// Current sticky section metadata for grouped virtualized lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizedListStickySectionSnapshot {
    key: String,
    label: String,
    index: usize,
}

impl VirtualizedListStickySectionSnapshot {
    pub(super) fn new(index: usize, item: &VirtualizedListItemDescriptor) -> Self {
        Self {
            key: item.key().to_owned(),
            label: item.label().to_owned(),
            index,
        }
    }

    /// Returns the stable section key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the visible section label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the zero-based source row index of the section row.
    pub const fn index(&self) -> usize {
        self.index
    }
}

/// Presentation-only sticky section overlay contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualizedListStickyOverlaySnapshot {
    section: VirtualizedListStickySectionSnapshot,
    source_row_visible: bool,
}

impl VirtualizedListStickyOverlaySnapshot {
    pub(super) const fn new(
        section: VirtualizedListStickySectionSnapshot,
        source_row_visible: bool,
    ) -> Self {
        Self {
            section,
            source_row_visible,
        }
    }

    /// Returns the section represented by the overlay.
    pub const fn section(&self) -> &VirtualizedListStickySectionSnapshot {
        &self.section
    }

    /// Returns whether the semantic source row is currently rendered.
    pub const fn source_row_visible(&self) -> bool {
        self.source_row_visible
    }

    /// Returns whether the overlay owns an accessibility role.
    pub const fn role(&self) -> Option<Role> {
        None
    }

    /// Returns whether the overlay can receive focus.
    pub const fn focusable(&self) -> bool {
        false
    }

    /// Returns whether the overlay owns pointer or click behavior.
    pub const fn pointer_interactive(&self) -> bool {
        false
    }

    /// Returns whether custom renderers may inject interactive content into the overlay.
    pub const fn allows_interactive_content(&self) -> bool {
        false
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
    sticky_section: Option<VirtualizedListStickySectionSnapshot>,
    sticky_overlay: Option<VirtualizedListStickyOverlaySnapshot>,
    rows: Vec<VirtualizedListRowBehaviorSnapshot>,
    visible_row_count: usize,
    overscan_count: usize,
    role: Role,
    row_role: Role,
}

impl VirtualizedListBehaviorSnapshot {
    pub(super) fn from_render_plan(plan: &VirtualizedListRenderPlan) -> Self {
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
            sticky_section: plan.sticky_section().cloned(),
            sticky_overlay: plan.sticky_overlay().cloned(),
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

    /// Returns the section that owns the first visible selectable row.
    pub fn sticky_section(&self) -> Option<&VirtualizedListStickySectionSnapshot> {
        self.sticky_section.as_ref()
    }

    /// Returns presentation-only sticky overlay metadata.
    pub fn sticky_overlay(&self) -> Option<&VirtualizedListStickyOverlaySnapshot> {
        self.sticky_overlay.as_ref()
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

    pub(super) fn target(&self) -> VirtualizedListItemTarget {
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

    pub(super) fn render_context(
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
    sticky_section: Option<VirtualizedListStickySectionSnapshot>,
    sticky_overlay: Option<VirtualizedListStickyOverlaySnapshot>,
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
        let row_positions = virtualized_list_row_positions(items, &duplicate_keys);
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
        let sticky_section = resolve_virtualized_list_sticky_section(
            items,
            virtualizer.visible_range(),
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
            .collect::<Vec<_>>();
        let sticky_overlay = sticky_section.as_ref().map(|section| {
            let source_row_visible = rows.iter().any(|row| row.index() == section.index());
            VirtualizedListStickyOverlaySnapshot::new(section.clone(), source_row_visible)
        });

        Self {
            list_id: list_id.into(),
            label: label.into(),
            state,
            metrics,
            row_measure_mode,
            virtualizer,
            sticky_section,
            sticky_overlay,
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

    /// Returns the section that owns the first visible selectable row.
    pub fn sticky_section(&self) -> Option<&VirtualizedListStickySectionSnapshot> {
        self.sticky_section.as_ref()
    }

    /// Returns presentation-only sticky overlay metadata.
    pub fn sticky_overlay(&self) -> Option<&VirtualizedListStickyOverlaySnapshot> {
        self.sticky_overlay.as_ref()
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

fn virtualized_list_row_positions(
    items: &[VirtualizedListItemDescriptor],
    duplicate_keys: &BTreeSet<String>,
) -> Vec<Option<usize>> {
    let mut option_position = 0usize;
    items
        .iter()
        .map(|item| {
            (item.selectable() && !duplicate_keys.contains(item.key())).then(|| {
                option_position += 1;
                option_position
            })
        })
        .collect()
}

fn resolve_virtualized_list_sticky_section(
    items: &[VirtualizedListItemDescriptor],
    visible_range: &open_gpui_ui_core::VirtualizerRange,
    duplicate_keys: &BTreeSet<String>,
) -> Option<VirtualizedListStickySectionSnapshot> {
    let visible_selectable_index = visible_range
        .as_range()
        .take_while(|index| *index < items.len())
        .find(|index| {
            items
                .get(*index)
                .is_some_and(|item| item.selectable() && !duplicate_keys.contains(item.key()))
        })?;

    (0..=visible_selectable_index)
        .rev()
        .find_map(|index| match items.get(index) {
            Some(item) if item.kind() == VirtualizedListRowKind::Section => {
                Some(VirtualizedListStickySectionSnapshot::new(index, item))
            }
            _ => None,
        })
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
