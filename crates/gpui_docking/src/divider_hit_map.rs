#![allow(dead_code)]

use crate::{
    DockNodeId, SplitAxis,
    presentation_scene::{DockPresentationScene, DockPresentationSplitter},
};
use open_gpui::{Bounds, Pixels, Point, point, px, size};

const CORNER_HIT_SIZE: f32 = 12.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDividerHitMap {
    targets: Vec<DockDividerHitTarget>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockDividerHitTarget {
    Single(DockDividerHandleHitTarget),
    Corner(DockDividerCornerHitTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockDividerHandleKey {
    pub(crate) split: DockNodeId,
    pub(crate) index: usize,
    pub(crate) axis: SplitAxis,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockDividerHandleHitTarget {
    pub(crate) key: DockDividerHandleKey,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) extent: Pixels,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockDividerCornerHitTarget {
    pub(crate) horizontal: DockDividerHandleHitTarget,
    pub(crate) vertical: DockDividerHandleHitTarget,
    pub(crate) bounds: Bounds<Pixels>,
}

impl DockDividerHitMap {
    pub(crate) fn from_scene(scene: &DockPresentationScene) -> Self {
        let handles = scene
            .splitters
            .iter()
            .map(handle_target)
            .collect::<Vec<_>>();
        let mut targets = Vec::new();

        for (index, first) in handles.iter().copied().enumerate() {
            for second in handles.iter().copied().skip(index + 1) {
                if first.key.axis == second.key.axis {
                    continue;
                }
                let (horizontal, vertical) = match (first.key.axis, second.key.axis) {
                    (SplitAxis::Horizontal, SplitAxis::Vertical) => (first, second),
                    (SplitAxis::Vertical, SplitAxis::Horizontal) => (second, first),
                    _ => continue,
                };
                if let Some(bounds) = corner_bounds(horizontal.bounds, vertical.bounds) {
                    targets.push(DockDividerHitTarget::Corner(DockDividerCornerHitTarget {
                        horizontal,
                        vertical,
                        bounds,
                    }));
                }
            }
        }

        targets.extend(handles.into_iter().map(DockDividerHitTarget::Single));
        Self { targets }
    }

    pub(crate) fn hit(&self, position: Point<Pixels>) -> Option<&DockDividerHitTarget> {
        self.targets.iter().find(|target| match target {
            DockDividerHitTarget::Corner(corner) => corner.bounds.contains(&position),
            DockDividerHitTarget::Single(handle) => handle.bounds.contains(&position),
        })
    }

    pub(crate) fn targets(&self) -> &[DockDividerHitTarget] {
        &self.targets
    }
}

fn handle_target(splitter: &DockPresentationSplitter) -> DockDividerHandleHitTarget {
    DockDividerHandleHitTarget {
        key: DockDividerHandleKey {
            split: splitter.split,
            index: splitter.index,
            axis: splitter.axis,
        },
        bounds: splitter.bounds,
        extent: splitter.extent,
    }
}

fn corner_bounds(horizontal: Bounds<Pixels>, vertical: Bounds<Pixels>) -> Option<Bounds<Pixels>> {
    let center = point(horizontal.center().x, vertical.center().y);
    if !near_bounds(horizontal, center) || !near_bounds(vertical, center) {
        return None;
    }
    let hit_size = px(CORNER_HIT_SIZE);
    Some(Bounds::new(
        point(center.x - hit_size / 2.0, center.y - hit_size / 2.0),
        size(hit_size, hit_size),
    ))
}

fn near_bounds(bounds: Bounds<Pixels>, position: Point<Pixels>) -> bool {
    bounds.dilate(px(CORNER_HIT_SIZE / 2.0)).contains(&position)
}
