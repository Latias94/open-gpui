use super::{TreeItemState, TreeMetrics, TreeState, nonnegative_px};
use open_gpui_ui_core::{
    Role, UiPx, VirtualizerItemKey, VirtualizerItemMeasurement, VirtualizerResolvedState,
    VirtualizerState,
};

/// One row in a resolved virtualized tree render window.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeRowRenderPlan {
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

    /// Returns the virtualizer measurement for this row.
    pub const fn measurement(&self) -> &VirtualizerItemMeasurement {
        &self.measurement
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
        self.measurement.start()
    }

    /// Returns the virtual row size.
    pub const fn virtual_size(&self) -> UiPx {
        self.measurement.size()
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

/// Fully resolved render contract for a virtualized tree.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeRenderPlan {
    tree_id: String,
    label: String,
    state: TreeState,
    metrics: TreeMetrics,
    virtualizer: VirtualizerResolvedState,
    rows: Vec<TreeRowRenderPlan>,
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
                    .map(|item| tree_row_render_key(item, index))
                    .unwrap_or_else(|| index.to_string());

                VirtualizerItemKey::new(key)
            });
        let rows = virtualizer
            .items()
            .iter()
            .filter_map(|measurement| {
                state.items().get(measurement.index()).cloned().map(|item| {
                    TreeRowRenderPlan::new(
                        item.clone(),
                        tree_row_render_key(&item, measurement.index()),
                        measurement.clone(),
                    )
                })
            })
            .collect();

        Self {
            tree_id: tree_id.into(),
            label: label.into(),
            state,
            metrics,
            virtualizer,
            rows,
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

    /// Returns the number of rows rendered after overscan.
    pub fn rendered_row_count(&self) -> usize {
        self.rows.len()
    }

    /// Returns the number of rows visible before overscan.
    pub fn visible_row_count(&self) -> usize {
        self.virtualizer.visible_items().len()
    }

    /// Returns the overscan item budget.
    pub const fn overscan_count(&self) -> usize {
        self.virtualizer.overscan()
    }

    /// Returns the focused row when it is inside the rendered window.
    pub fn focused_row(&self) -> Option<&TreeRowRenderPlan> {
        self.rows.iter().find(|row| row.focused())
    }

    /// Returns the selected row when it is inside the rendered window.
    pub fn selected_row(&self) -> Option<&TreeRowRenderPlan> {
        self.rows.iter().find(|row| row.selected())
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

fn tree_row_render_key(item: &TreeItemState, index: usize) -> String {
    format!("{index}:{}", item.value())
}
