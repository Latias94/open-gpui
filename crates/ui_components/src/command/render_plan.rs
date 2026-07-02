use std::collections::{BTreeMap, BTreeSet};

use open_gpui_ui_core::{
    Role, UiPx, VirtualizerItemKey, VirtualizerItemMeasurement, VirtualizerResolvedState,
    VirtualizerState,
};

use super::{
    CommandItemState, CommandMetrics, CommandState, DEFAULT_COMMAND_VIEWPORT_ITEM_COUNT,
    nonnegative_px,
};

/// Public behavior snapshot for one virtualized command row.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandRowBehaviorSnapshot {
    item: CommandItemState,
    render_key: String,
    group_label: Option<String>,
    virtual_start: UiPx,
    virtual_size: UiPx,
}

impl CommandRowBehaviorSnapshot {
    fn from_render_plan(row: &CommandRowRenderPlan) -> Self {
        Self {
            item: row.item().clone(),
            render_key: row.render_key().to_owned(),
            group_label: row.group_label().map(str::to_owned),
            virtual_start: row.virtual_start(),
            virtual_size: row.virtual_size(),
        }
    }

    /// Returns the resolved command item state.
    pub const fn item(&self) -> &CommandItemState {
        &self.item
    }

    /// Returns the stable item value.
    pub fn value(&self) -> &str {
        self.item.value()
    }

    /// Returns the visible item label.
    pub fn label(&self) -> &str {
        self.item.label()
    }

    /// Returns optional shortcut label.
    pub fn shortcut(&self) -> Option<&str> {
        self.item.shortcut()
    }

    /// Returns caller-owned availability metadata.
    pub fn when_ref(&self) -> Option<&str> {
        self.item.when_ref()
    }

    /// Returns the render key used by element ids and virtualizer measurements.
    pub fn render_key(&self) -> &str {
        &self.render_key
    }

    /// Returns flattened command item index.
    pub const fn index(&self) -> usize {
        self.item.index()
    }

    /// Returns the group label when this row starts or belongs to a visible group.
    pub fn group_label(&self) -> Option<&str> {
        self.group_label.as_deref()
    }

    /// Returns whether this row is selected.
    pub const fn selected(&self) -> bool {
        self.item.selected()
    }

    /// Returns whether this row is active.
    pub const fn active(&self) -> bool {
        self.item.active()
    }

    /// Returns whether this row is disabled.
    pub const fn disabled(&self) -> bool {
        self.item.disabled()
    }

    /// Returns the virtual row start offset.
    pub const fn virtual_start(&self) -> UiPx {
        self.virtual_start
    }

    /// Returns the virtual row size.
    pub const fn virtual_size(&self) -> UiPx {
        self.virtual_size
    }

    /// Returns the row accessibility role.
    pub const fn role(&self) -> Role {
        self.item.role()
    }
}

/// Public behavior snapshot for command results.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandBehaviorSnapshot {
    command_id: String,
    listbox_id: String,
    label: String,
    state: CommandState,
    metrics: CommandMetrics,
    total_size: UiPx,
    viewport_extent: UiPx,
    scroll_offset: UiPx,
    visible_range: open_gpui_ui_core::VirtualizerRange,
    overscan_range: open_gpui_ui_core::VirtualizerRange,
    rows: Vec<CommandRowBehaviorSnapshot>,
    role: Role,
    row_role: Role,
}

impl CommandBehaviorSnapshot {
    pub(super) fn from_render_plan(plan: &CommandRenderPlan) -> Self {
        Self {
            command_id: plan.command_id().to_owned(),
            listbox_id: plan.listbox_id().to_owned(),
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
                .map(CommandRowBehaviorSnapshot::from_render_plan)
                .collect(),
            role: plan.role(),
            row_role: plan.row_role(),
        }
    }

    /// Returns stable command id.
    pub fn command_id(&self) -> &str {
        &self.command_id
    }

    /// Returns stable nested listbox id.
    pub fn listbox_id(&self) -> &str {
        &self.listbox_id
    }

    /// Returns accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns complete command state.
    pub const fn state(&self) -> &CommandState {
        &self.state
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> CommandMetrics {
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

    /// Returns virtualized rows in render order.
    pub fn rows(&self) -> &[CommandRowBehaviorSnapshot] {
        &self.rows
    }

    /// Returns row lookup keyed by flattened command item index.
    pub fn row_by_index(&self, index: usize) -> Option<&CommandRowBehaviorSnapshot> {
        self.rows.iter().find(|row| row.index() == index)
    }

    /// Returns list accessibility role.
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Returns row accessibility role.
    pub const fn row_role(&self) -> Role {
        self.row_role
    }

    /// Returns number of rows visible before overscan.
    pub fn visible_row_count(&self) -> usize {
        self.visible_range.len()
    }

    /// Returns number of rendered rows after overscan.
    pub fn rendered_row_count(&self) -> usize {
        self.rows.len()
    }

    /// Returns the active row if it is inside the behavior window.
    pub fn active_row(&self) -> Option<&CommandRowBehaviorSnapshot> {
        self.rows.iter().find(|row| row.active())
    }

    /// Returns selected rows inside the behavior window.
    pub fn selected_rows(&self) -> impl Iterator<Item = &CommandRowBehaviorSnapshot> + '_ {
        self.rows.iter().filter(|row| row.selected())
    }
}

/// One virtualized command item row in render order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CommandRowRenderPlan {
    item: CommandItemState,
    render_key: String,
    group_label: Option<String>,
    measurement: VirtualizerItemMeasurement,
}

impl CommandRowRenderPlan {
    pub(super) fn new(
        item: CommandItemState,
        render_key: String,
        group_label: Option<String>,
        measurement: VirtualizerItemMeasurement,
    ) -> Self {
        Self {
            item,
            render_key,
            group_label,
            measurement,
        }
    }

    /// Returns the resolved command item state.
    pub const fn item(&self) -> &CommandItemState {
        &self.item
    }

    /// Returns the stable item value.
    pub fn value(&self) -> &str {
        self.item.value()
    }

    /// Returns the visible item label.
    pub fn label(&self) -> &str {
        self.item.label()
    }

    /// Returns optional shortcut label.
    pub fn shortcut(&self) -> Option<&str> {
        self.item.shortcut()
    }

    /// Returns the render key used by element ids and virtualizer measurements.
    pub fn render_key(&self) -> &str {
        &self.render_key
    }

    /// Returns the group label when this row starts or belongs to a visible group.
    pub fn group_label(&self) -> Option<&str> {
        self.group_label.as_deref()
    }

    /// Returns whether this row is selected.
    pub const fn selected(&self) -> bool {
        self.item.selected()
    }

    /// Returns whether this row is active.
    pub const fn active(&self) -> bool {
        self.item.active()
    }

    /// Returns whether this row is disabled.
    pub const fn disabled(&self) -> bool {
        self.item.disabled()
    }

    /// Returns the virtual row start offset.
    pub const fn virtual_start(&self) -> UiPx {
        self.measurement.start()
    }

    /// Returns the virtual row size.
    pub const fn virtual_size(&self) -> UiPx {
        self.measurement.size()
    }

    /// Returns the row accessibility role.
    pub const fn role(&self) -> Role {
        self.item.role()
    }
}

/// Renderer-neutral virtualized render contract for command results.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CommandRenderPlan {
    command_id: String,
    listbox_id: String,
    label: String,
    state: CommandState,
    metrics: CommandMetrics,
    virtualizer: VirtualizerResolvedState,
    rows: Vec<CommandRowRenderPlan>,
    role: Role,
    row_role: Role,
}

impl CommandRenderPlan {
    /// Resolves a render plan from complete command state and a viewport snapshot.
    pub fn resolve(
        command_id: impl Into<String>,
        listbox_id: impl Into<String>,
        state: CommandState,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> Self {
        let metrics = state.metrics();
        let viewport_extent = resolve_command_viewport_extent(metrics, viewport_extent);
        let duplicate_values = duplicate_command_values(state.items());
        let virtualizer = VirtualizerState::new(state.items().len(), metrics.row_height())
            .with_viewport_extent(viewport_extent)
            .with_overscan(metrics.overscan_count())
            .with_scroll_offset(command_clamped_scroll_offset(
                scroll_offset,
                state.items().len(),
                metrics.row_height(),
                viewport_extent,
            ))
            .resolve_fixed_window(|index| {
                let item = &state.items()[index];
                VirtualizerItemKey::new(command_row_render_key(item, &duplicate_values))
            });
        let rows = virtualizer
            .items()
            .iter()
            .filter_map(|measurement| {
                state.items().get(measurement.index()).cloned().map(|item| {
                    let render_key = command_row_render_key(&item, &duplicate_values);
                    let group_label = item
                        .group_index()
                        .filter(|group_index| {
                            state
                                .items()
                                .iter()
                                .filter(|candidate| candidate.group_index() == Some(*group_index))
                                .map(CommandItemState::index)
                                .min()
                                == Some(item.index())
                        })
                        .and_then(|group_index| {
                            state
                                .groups()
                                .get(group_index)
                                .map(|group| group.label().to_owned())
                        });
                    CommandRowRenderPlan::new(item, render_key, group_label, measurement.clone())
                })
            })
            .collect();

        Self {
            command_id: command_id.into(),
            listbox_id: listbox_id.into(),
            label: state.label().to_owned(),
            state,
            metrics,
            virtualizer,
            rows,
            role: Role::ListBox,
            row_role: Role::ListBoxOption,
        }
    }

    /// Returns stable command id.
    pub fn command_id(&self) -> &str {
        &self.command_id
    }

    /// Returns stable nested listbox id.
    pub fn listbox_id(&self) -> &str {
        &self.listbox_id
    }

    /// Returns accessible label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns complete command state.
    pub const fn state(&self) -> &CommandState {
        &self.state
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> CommandMetrics {
        self.metrics
    }

    /// Returns resolved virtualizer state.
    pub const fn virtualizer(&self) -> &VirtualizerResolvedState {
        &self.virtualizer
    }

    /// Returns virtualized rows in render order.
    pub fn rows(&self) -> &[CommandRowRenderPlan] {
        &self.rows
    }

    /// Returns list accessibility role.
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Returns row accessibility role.
    pub const fn row_role(&self) -> Role {
        self.row_role
    }
}

pub(super) fn resolve_command_viewport_extent(
    metrics: CommandMetrics,
    viewport_extent: UiPx,
) -> UiPx {
    let viewport_extent = nonnegative_px(viewport_extent);
    if viewport_extent.as_f32() > 0.0 {
        viewport_extent
    } else {
        let row_height = nonnegative_px(metrics.row_height());
        if row_height.as_f32() > 0.0 {
            row_height * DEFAULT_COMMAND_VIEWPORT_ITEM_COUNT as f32
        } else {
            UiPx::ZERO
        }
    }
}

fn duplicate_command_values(items: &[CommandItemState]) -> BTreeSet<String> {
    let mut counts = BTreeMap::new();
    for item in items {
        *counts.entry(item.value().to_owned()).or_insert(0usize) += 1;
    }

    counts
        .into_iter()
        .filter_map(|(value, count)| (count > 1).then_some(value))
        .collect()
}

fn command_row_render_key(item: &CommandItemState, duplicate_values: &BTreeSet<String>) -> String {
    if duplicate_values.contains(item.value()) {
        format!("{}:{}", item.index(), item.value())
    } else {
        item.value().to_owned()
    }
}

fn command_clamped_scroll_offset(
    scroll_offset: UiPx,
    item_count: usize,
    row_height: UiPx,
    viewport_extent: UiPx,
) -> UiPx {
    let scroll_offset = nonnegative_px(scroll_offset);
    let row_height = nonnegative_px(row_height);
    let viewport_extent = nonnegative_px(viewport_extent);
    if item_count == 0 || row_height.as_f32() <= 0.0 {
        return UiPx::ZERO;
    }

    let total_size = row_height * item_count as f32;
    scroll_offset.min(nonnegative_px(total_size - viewport_extent))
}
