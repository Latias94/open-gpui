use super::{TreeItemState, TreeMetrics, TreeState, nonnegative_px};
use open_gpui_ui_core::{
    Role, RowWindow, UiPx, VirtualizerItemKey, VirtualizerItemMeasurement, VirtualizerRange,
    VirtualizerResolvedState, VirtualizerState,
};

/// Public behavior snapshot for one virtualized tree row.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeRowBehaviorSnapshot {
    item: TreeItemState,
    render_key: String,
    virtual_start: UiPx,
    virtual_size: UiPx,
}

impl TreeRowBehaviorSnapshot {
    fn from_render_plan(row: &TreeRowRenderPlan) -> Self {
        Self {
            item: row.item().clone(),
            render_key: row.render_key().to_owned(),
            virtual_start: row.virtual_start(),
            virtual_size: row.virtual_size(),
        }
    }

    /// Returns the resolved tree item for this row.
    pub const fn item(&self) -> &TreeItemState {
        &self.item
    }

    /// Returns the renderer-stable row key.
    pub fn render_key(&self) -> &str {
        &self.render_key
    }

    /// Returns the zero-based visible item index.
    pub const fn index(&self) -> usize {
        self.item.index()
    }

    /// Returns the stable item value.
    pub fn value(&self) -> &str {
        self.item.value()
    }

    /// Returns the visible item label.
    pub fn label(&self) -> &str {
        self.item.label()
    }

    /// Returns the zero-based hierarchy depth.
    pub const fn depth(&self) -> usize {
        self.item.depth()
    }

    /// Returns the virtual row start offset.
    pub const fn virtual_start(&self) -> UiPx {
        self.virtual_start
    }

    /// Returns the virtual row size.
    pub const fn virtual_size(&self) -> UiPx {
        self.virtual_size
    }

    /// Returns whether this row is selected.
    pub const fn selected(&self) -> bool {
        self.item.selected()
    }

    /// Returns whether this row currently has roving focus.
    pub const fn focused(&self) -> bool {
        self.item.focused()
    }
}

/// Public behavior snapshot for a concrete tree viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeBehaviorSnapshot {
    tree_id: String,
    label: String,
    state: TreeState,
    metrics: TreeMetrics,
    total_size: UiPx,
    viewport_extent: UiPx,
    scroll_offset: UiPx,
    visible_range: VirtualizerRange,
    overscan_range: VirtualizerRange,
    rows: Vec<TreeRowBehaviorSnapshot>,
    visible_row_count: usize,
    overscan_count: usize,
    role: Role,
    row_role: Role,
}

impl TreeBehaviorSnapshot {
    pub(super) fn from_render_plan(plan: &TreeRenderPlan) -> Self {
        Self {
            tree_id: plan.tree_id().to_owned(),
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
                .map(TreeRowBehaviorSnapshot::from_render_plan)
                .collect(),
            visible_row_count: plan.visible_row_count(),
            overscan_count: plan.overscan_count(),
            role: plan.role(),
            row_role: plan.row_role(),
        }
    }

    /// Returns the stable tree id.
    pub fn tree_id(&self) -> &str {
        &self.tree_id
    }

    /// Returns the accessible tree label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the resolved renderer-neutral tree state.
    pub const fn state(&self) -> &TreeState {
        &self.state
    }

    /// Returns the resolved metrics.
    pub const fn metrics(&self) -> TreeMetrics {
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
    pub const fn visible_range(&self) -> &VirtualizerRange {
        &self.visible_range
    }

    /// Returns the rendered source row range after overscan.
    pub const fn overscan_range(&self) -> &VirtualizerRange {
        &self.overscan_range
    }

    /// Returns rows in render order after overscan.
    pub fn rows(&self) -> &[TreeRowBehaviorSnapshot] {
        &self.rows
    }

    /// Returns the accessibility role for the root tree container.
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
    pub const fn visible_row_count(&self) -> usize {
        self.visible_row_count
    }

    /// Returns the overscan item budget.
    pub const fn overscan_count(&self) -> usize {
        self.overscan_count
    }

    /// Returns the focused row when it is inside the rendered window.
    pub fn focused_row(&self) -> Option<&TreeRowBehaviorSnapshot> {
        self.rows.iter().find(|row| row.focused())
    }

    /// Returns the selected row when it is inside the rendered window.
    pub fn selected_row(&self) -> Option<&TreeRowBehaviorSnapshot> {
        self.rows.iter().find(|row| row.selected())
    }
}

/// One row in a resolved virtualized tree render window.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TreeRowRenderPlan {
    item: TreeItemState,
    render_key: String,
    measurement: VirtualizerItemMeasurement,
}

impl TreeRowRenderPlan {
    fn new(
        item: TreeItemState,
        render_key: String,
        measurement: VirtualizerItemMeasurement,
    ) -> Self {
        Self {
            item,
            render_key,
            measurement,
        }
    }

    /// Returns the resolved tree item for this row.
    pub const fn item(&self) -> &TreeItemState {
        &self.item
    }

    /// Returns the renderer-stable row key.
    pub fn render_key(&self) -> &str {
        &self.render_key
    }

    /// Returns the zero-based visible item index.
    pub const fn index(&self) -> usize {
        self.item.index()
    }

    /// Returns the virtual row start offset.
    pub const fn virtual_start(&self) -> UiPx {
        self.measurement.start()
    }

    /// Returns the virtual row size.
    pub const fn virtual_size(&self) -> UiPx {
        self.measurement.size()
    }
}

/// Fully resolved render contract for a virtualized tree.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TreeRenderPlan {
    tree_id: String,
    label: String,
    state: TreeState,
    metrics: TreeMetrics,
    virtualizer: VirtualizerResolvedState,
    rows: Vec<TreeRowRenderPlan>,
    visible_row_count: usize,
    overscan_count: usize,
    role: Role,
    row_role: Role,
}

impl TreeRenderPlan {
    /// Resolves a fixed-row virtualized tree render plan.
    pub fn resolve(
        tree_id: impl Into<String>,
        label: impl Into<String>,
        state: TreeState,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
        viewport_item_count: usize,
        overscan_count: usize,
    ) -> Self {
        let metrics = state.metrics();
        let viewport_extent =
            resolve_tree_viewport_extent(metrics, viewport_extent, viewport_item_count);
        let virtualizer = VirtualizerState::new(state.items().len(), metrics.row_height())
            .with_viewport_extent(viewport_extent)
            .with_overscan(overscan_count)
            .with_scroll_offset(nonnegative_px(scroll_offset))
            .resolve_fixed_window(|index| {
                let key = state
                    .items()
                    .get(index)
                    .map(|item| item.render_identity().to_owned())
                    .unwrap_or_else(|| index.to_string());

                VirtualizerItemKey::new(key)
            });
        let row_window =
            RowWindow::project(&virtualizer, |index| state.items().get(index).cloned());
        let visible_row_count = row_window.visible_row_count();
        let overscan_count = row_window.overscan_count();
        let rows = row_window
            .into_rows()
            .into_iter()
            .map(|projected| {
                let (_, render_key, measurement, item) = projected.into_parts();
                TreeRowRenderPlan::new(item, render_key, measurement)
            })
            .collect();

        Self {
            tree_id: tree_id.into(),
            label: label.into(),
            state,
            metrics,
            virtualizer,
            rows,
            visible_row_count,
            overscan_count,
            role: Role::Tree,
            row_role: Role::TreeItem,
        }
    }

    /// Returns the stable tree id.
    pub fn tree_id(&self) -> &str {
        &self.tree_id
    }

    /// Returns the accessible tree label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the resolved renderer-neutral tree state.
    pub const fn state(&self) -> &TreeState {
        &self.state
    }

    /// Returns the resolved metrics.
    pub const fn metrics(&self) -> TreeMetrics {
        self.metrics
    }

    /// Returns the resolved virtualizer state.
    pub const fn virtualizer(&self) -> &VirtualizerResolvedState {
        &self.virtualizer
    }

    /// Returns rows in render order after overscan.
    pub fn rows(&self) -> &[TreeRowRenderPlan] {
        &self.rows
    }

    /// Returns the accessibility role for the root tree container.
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

    /// Returns the overscan item budget.
    pub const fn overscan_count(&self) -> usize {
        self.overscan_count
    }
}

fn resolve_tree_viewport_extent(
    metrics: TreeMetrics,
    viewport_extent: UiPx,
    viewport_item_count: usize,
) -> UiPx {
    let viewport_extent = nonnegative_px(viewport_extent);
    if viewport_extent.as_f32() > 0.0 {
        viewport_extent
    } else {
        metrics.row_height() * viewport_item_count.max(1) as f32
    }
}
