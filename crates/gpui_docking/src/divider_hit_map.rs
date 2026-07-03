use crate::{
    DockNodeId, SplitAxis,
    geometry::{bounds_from_ui_rect, ui_rect_from_bounds},
    presentation_scene::{DockPresentationScene, DockPresentationSplitter},
};
use open_gpui::{Bounds, Pixels, Point};
use open_gpui_ui_core::{Orientation, SplitterHandleLayout, SplitterHitMap, SplitterHitTarget};

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
    pub(crate) before: DockNodeId,
    pub(crate) after: DockNodeId,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) extent: Pixels,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockDividerCornerHitTarget {
    pub(crate) horizontal: DockDividerHandleHitTarget,
    pub(crate) vertical: DockDividerHandleHitTarget,
    pub(crate) bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockDividerAffordanceState {
    Idle,
    Hover,
    Active,
    Disabled,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDividerCornerAffordance {
    pub(crate) corner: DockDividerCornerHitTarget,
    pub(crate) state: DockDividerAffordanceState,
}

#[derive(Debug, Clone, PartialEq)]
struct DockCoreHandleTarget {
    core: SplitterHandleLayout,
    dock: DockDividerHandleHitTarget,
}

impl DockDividerHitMap {
    pub(crate) fn from_scene(scene: &DockPresentationScene) -> Self {
        let handles = scene
            .splitters
            .iter()
            .map(handle_target)
            .collect::<Vec<_>>();
        let core_handles = handles
            .iter()
            .copied()
            .map(core_handle_target_for_dock_target)
            .collect::<Vec<_>>();
        let core_hit_map =
            SplitterHitMap::from_handles(core_handles.iter().map(|handle| handle.core.clone()));
        let targets = core_hit_map
            .targets()
            .iter()
            .filter_map(|target| match target {
                SplitterHitTarget::Handle(handle) => {
                    handle_target_for_core(handle, &core_handles).map(DockDividerHitTarget::Single)
                }
                SplitterHitTarget::Junction(junction) => {
                    let horizontal = handle_target_for_core(junction.horizontal(), &core_handles)?;
                    let vertical = handle_target_for_core(junction.vertical(), &core_handles)?;
                    Some(DockDividerHitTarget::Corner(DockDividerCornerHitTarget {
                        horizontal,
                        vertical,
                        bounds: bounds_from_ui_rect(junction.bounds()),
                    }))
                }
            })
            .collect();
        Self { targets }
    }

    pub(crate) fn hit(&self, position: Point<Pixels>) -> Option<&DockDividerHitTarget> {
        self.targets.iter().find(|target| match target {
            DockDividerHitTarget::Corner(corner) => corner.bounds.contains(&position),
            DockDividerHitTarget::Single(handle) => handle.bounds.contains(&position),
        })
    }

    pub(crate) fn corner_affordances(
        &self,
        hover_position: Option<Point<Pixels>>,
        dragging: bool,
        enabled: bool,
    ) -> Vec<DockDividerCornerAffordance> {
        self.targets
            .iter()
            .filter_map(|target| match target {
                DockDividerHitTarget::Corner(corner) => Some(DockDividerCornerAffordance {
                    corner: *corner,
                    state: corner_affordance_state(*corner, hover_position, dragging, enabled),
                }),
                DockDividerHitTarget::Single(_) => None,
            })
            .collect()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn targets(&self) -> &[DockDividerHitTarget] {
        &self.targets
    }
}

fn core_handle_target_for_dock_target(handle: DockDividerHandleHitTarget) -> DockCoreHandleTarget {
    DockCoreHandleTarget {
        core: splitter_handle_layout_for_target(handle),
        dock: handle,
    }
}

fn handle_target(splitter: &DockPresentationSplitter) -> DockDividerHandleHitTarget {
    DockDividerHandleHitTarget {
        key: DockDividerHandleKey {
            split: splitter.split,
            index: splitter.index,
            axis: splitter.axis,
        },
        before: splitter.before,
        after: splitter.after,
        bounds: splitter.bounds,
        extent: splitter.extent,
    }
}

fn splitter_handle_layout_for_target(handle: DockDividerHandleHitTarget) -> SplitterHandleLayout {
    SplitterHandleLayout::new(
        format!("dock-split-{}", handle.key.split.as_u64()),
        orientation_for_axis(handle.key.axis),
        handle.key.index,
        format!("dock-node-{}", handle.before.as_u64()),
        format!("dock-node-{}", handle.after.as_u64()),
        false,
        ui_rect_from_bounds(handle.bounds),
        ui_rect_from_bounds(handle.bounds),
    )
}

fn handle_target_for_core(
    handle: &SplitterHandleLayout,
    targets: &[DockCoreHandleTarget],
) -> Option<DockDividerHandleHitTarget> {
    targets
        .iter()
        .find(|target| {
            target.core.group_id() == handle.group_id()
                && target.core.index() == handle.index()
                && target.core.orientation() == handle.orientation()
        })
        .map(|target| target.dock)
}

fn orientation_for_axis(axis: SplitAxis) -> Orientation {
    match axis {
        SplitAxis::Horizontal => Orientation::Horizontal,
        SplitAxis::Vertical => Orientation::Vertical,
    }
}

fn corner_affordance_state(
    corner: DockDividerCornerHitTarget,
    hover_position: Option<Point<Pixels>>,
    dragging: bool,
    enabled: bool,
) -> DockDividerAffordanceState {
    if !enabled {
        return DockDividerAffordanceState::Disabled;
    }
    if dragging {
        return DockDividerAffordanceState::Active;
    }
    if hover_position.is_some_and(|position| corner.bounds.contains(&position)) {
        return DockDividerAffordanceState::Hover;
    }
    DockDividerAffordanceState::Idle
}
