use crate::{
    DockNodeId, SplitAxis,
    geometry::{bounds_from_ui_rect, ui_rect_from_bounds},
};
use open_gpui::{Bounds, Pixels};
use open_gpui_ui_core::{
    Orientation, Size as SplitterSize, SplitterHandlePlacement, SplitterLayoutScene,
    SplitterMetrics, SplitterPanelDescriptor, SplitterState,
    resolve_split_fractions_with_fill_child, ui_px,
};

#[derive(Debug, PartialEq)]
pub(crate) struct DockResolvedSplitLayout<'a> {
    shares: Vec<f32>,
    children: &'a [DockNodeId],
    axis: SplitAxis,
    extent: Pixels,
    scene: SplitterLayoutScene,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockResolvedSplitPanel {
    pub(crate) child: DockNodeId,
    pub(crate) bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockResolvedSplitHandle {
    pub(crate) index: usize,
    pub(crate) before: DockNodeId,
    pub(crate) after: DockNodeId,
    pub(crate) axis: SplitAxis,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) extent: Pixels,
}

impl<'a> DockResolvedSplitLayout<'a> {
    pub(crate) fn shares(&self) -> &[f32] {
        &self.shares
    }

    pub(crate) fn panels(&self) -> impl Iterator<Item = DockResolvedSplitPanel> + '_ {
        self.children
            .iter()
            .copied()
            .zip(self.scene.panels())
            .map(|(child, panel)| DockResolvedSplitPanel {
                child,
                bounds: bounds_from_ui_rect(panel.bounds()),
            })
    }

    pub(crate) fn handles(&self) -> impl Iterator<Item = DockResolvedSplitHandle> + '_ {
        self.scene.handles().iter().filter_map(|handle| {
            let index = handle.index();
            Some(DockResolvedSplitHandle {
                index,
                before: *self.children.get(index)?,
                after: *self.children.get(index + 1)?,
                axis: self.axis,
                bounds: bounds_from_ui_rect(handle.bounds()),
                extent: self.extent,
            })
        })
    }
}

pub(crate) fn resolve_dock_split_shares(
    child_count: usize,
    fractions: &[f32],
    central_child_index: Option<usize>,
) -> Vec<f32> {
    resolve_split_fractions_with_fill_child(child_count, fractions, central_child_index)
}

pub(crate) fn dock_split_handle_center_shares(shares: &[f32]) -> impl Iterator<Item = f32> + '_ {
    shares
        .iter()
        .take(shares.len().saturating_sub(1))
        .scan(0.0_f32, |center_share, share| {
            *center_share += *share;
            Some(*center_share)
        })
}

pub(crate) fn resolve_dock_split_layout<'a>(
    split: DockNodeId,
    axis: SplitAxis,
    children: &'a [DockNodeId],
    fractions: &[f32],
    central_child_index: Option<usize>,
    bounds: Bounds<Pixels>,
    handle_size: Pixels,
) -> DockResolvedSplitLayout<'a> {
    let shares = resolve_dock_split_shares(children.len(), fractions, central_child_index);
    let scene = split_layout_scene(split, axis, children, &shares, bounds, handle_size);
    let extent = split_extent(axis, bounds);

    DockResolvedSplitLayout {
        shares,
        children,
        axis,
        extent,
        scene,
    }
}

fn split_layout_scene(
    split: DockNodeId,
    axis: SplitAxis,
    children: &[DockNodeId],
    shares: &[f32],
    bounds: Bounds<Pixels>,
    handle_size: Pixels,
) -> SplitterLayoutScene {
    let orientation = match axis {
        SplitAxis::Horizontal => Orientation::Horizontal,
        SplitAxis::Vertical => Orientation::Vertical,
    };
    let state = SplitterState::resolve(
        format!("dock-split-{}", split.as_u64()),
        orientation,
        SplitterSize::Medium,
        false,
        children.iter().enumerate().map(|(index, child)| {
            SplitterPanelDescriptor::new(
                format!("dock-node-{}", child.as_u64()),
                shares.get(index).copied().unwrap_or(0.0),
            )
            .min_fraction(0.0)
        }),
    );
    let handle_size = ui_px(f32::from(handle_size).max(0.0));
    let metrics = SplitterMetrics::new(handle_size, handle_size, ui_px(0.0))
        .with_handle_placement(SplitterHandlePlacement::OverlayBoundary);
    SplitterLayoutScene::from_state_with_metrics(&state, ui_rect_from_bounds(bounds), metrics)
}

fn split_extent(axis: SplitAxis, bounds: Bounds<Pixels>) -> Pixels {
    match axis {
        SplitAxis::Horizontal => bounds.size.width,
        SplitAxis::Vertical => bounds.size.height,
    }
}
