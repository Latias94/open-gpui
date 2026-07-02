use crate::{DockNodeId, SplitAxis};
use open_gpui::{Bounds, Pixels, point, px, size};
use open_gpui_ui_core::{
    Orientation, Size as SplitterSize, SplitterHandlePlacement, SplitterLayoutScene,
    SplitterMetrics, SplitterPanelDescriptor, SplitterState, UiRect,
    resolve_split_fractions_with_fill_child, ui_point, ui_px, ui_rect, ui_size,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockResolvedSplitLayout {
    shares: Vec<f32>,
    panels: Vec<DockResolvedSplitPanel>,
    handles: Vec<DockResolvedSplitHandle>,
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

impl DockResolvedSplitLayout {
    pub(crate) fn shares(&self) -> &[f32] {
        &self.shares
    }

    pub(crate) fn panels(&self) -> &[DockResolvedSplitPanel] {
        &self.panels
    }

    pub(crate) fn handles(&self) -> &[DockResolvedSplitHandle] {
        &self.handles
    }
}

pub(crate) fn resolve_dock_split_shares(
    child_count: usize,
    fractions: &[f32],
    central_child_index: Option<usize>,
) -> Vec<f32> {
    resolve_split_fractions_with_fill_child(child_count, fractions, central_child_index)
}

pub(crate) fn dock_split_handle_center_shares(shares: &[f32]) -> Vec<f32> {
    shares
        .iter()
        .take(shares.len().saturating_sub(1))
        .scan(0.0_f32, |center_share, share| {
            *center_share += *share;
            Some(*center_share)
        })
        .collect()
}

pub(crate) fn resolve_dock_split_layout(
    split: DockNodeId,
    axis: SplitAxis,
    children: &[DockNodeId],
    fractions: &[f32],
    central_child_index: Option<usize>,
    bounds: Bounds<Pixels>,
    handle_size: Pixels,
) -> DockResolvedSplitLayout {
    let shares = resolve_dock_split_shares(children.len(), fractions, central_child_index);
    let scene = split_layout_scene(split, axis, children, &shares, bounds, handle_size);
    let extent = split_extent(axis, bounds);

    let panels = children
        .iter()
        .copied()
        .zip(scene.panels())
        .map(|(child, panel)| DockResolvedSplitPanel {
            child,
            bounds: bounds_from_ui_rect(panel.bounds()),
        })
        .collect();

    let handles = scene
        .handles()
        .iter()
        .filter_map(|handle| {
            let index = handle.index();
            Some(DockResolvedSplitHandle {
                index,
                before: *children.get(index)?,
                after: *children.get(index + 1)?,
                axis,
                bounds: bounds_from_ui_rect(handle.bounds()),
                extent,
            })
        })
        .collect();

    DockResolvedSplitLayout {
        shares,
        panels,
        handles,
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

fn ui_rect_from_bounds(bounds: Bounds<Pixels>) -> UiRect {
    ui_rect(
        ui_point(
            ui_px(f32::from(bounds.origin.x)),
            ui_px(f32::from(bounds.origin.y)),
        ),
        ui_size(
            ui_px(f32::from(bounds.size.width)),
            ui_px(f32::from(bounds.size.height)),
        ),
    )
}

fn bounds_from_ui_rect(rect: UiRect) -> Bounds<Pixels> {
    Bounds::new(
        point(px(rect.origin.x.as_f32()), px(rect.origin.y.as_f32())),
        size(px(rect.size.width.as_f32()), px(rect.size.height.as_f32())),
    )
}

fn split_extent(axis: SplitAxis, bounds: Bounds<Pixels>) -> Pixels {
    match axis {
        SplitAxis::Horizontal => bounds.size.width,
        SplitAxis::Vertical => bounds.size.height,
    }
}
