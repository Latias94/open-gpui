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
    floating_surfaces: Vec<DockDividerFloatingSurface>,
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
    pub(crate) surface: DockDividerSurface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockDividerSurface {
    Root,
    Floating(DockNodeId),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockDividerFloatingSurface {
    surface: DockDividerSurface,
    bounds: Bounds<Pixels>,
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
        let floating_surfaces = scene
            .floating_containers
            .iter()
            .map(|floating| DockDividerFloatingSurface {
                surface: DockDividerSurface::Floating(floating.node),
                bounds: floating.bounds,
            })
            .collect::<Vec<_>>();
        let mut surfaces = Vec::with_capacity(floating_surfaces.len() + 1);
        surfaces.push(DockDividerSurface::Root);
        surfaces.extend(floating_surfaces.iter().map(|floating| floating.surface));

        let mut targets = Vec::new();
        for surface in surfaces {
            let handles = scene
                .splitters
                .iter()
                .filter(|splitter| surface_for_splitter(splitter) == surface)
                .map(handle_target)
                .collect::<Vec<_>>();
            targets.extend(targets_for_surface(&handles));
        }

        Self {
            targets,
            floating_surfaces,
        }
    }

    pub(crate) fn hit(&self, position: Point<Pixels>) -> Option<&DockDividerHitTarget> {
        let surface = self.surface_at(position);
        self.targets.iter().find(|target| {
            target_surface(target) == surface && target_bounds(target).contains(&position)
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
                DockDividerHitTarget::Corner(corner)
                    if self.is_unoccluded(corner.horizontal.surface, corner.bounds) =>
                {
                    Some(DockDividerCornerAffordance {
                        corner: *corner,
                        state: corner_affordance_state(*corner, hover_position, dragging, enabled),
                    })
                }
                DockDividerHitTarget::Corner(_) => None,
                DockDividerHitTarget::Single(_) => None,
            })
            .collect()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn targets(&self) -> &[DockDividerHitTarget] {
        &self.targets
    }

    fn surface_at(&self, position: Point<Pixels>) -> DockDividerSurface {
        self.floating_surfaces
            .iter()
            .rev()
            .find(|floating| floating.bounds.contains(&position))
            .map(|floating| floating.surface)
            .unwrap_or(DockDividerSurface::Root)
    }

    fn is_unoccluded(&self, surface: DockDividerSurface, bounds: Bounds<Pixels>) -> bool {
        let first_occluding_surface = match surface {
            DockDividerSurface::Root => 0,
            DockDividerSurface::Floating(node) => self
                .floating_surfaces
                .iter()
                .position(|floating| floating.surface == DockDividerSurface::Floating(node))
                .map(|index| index + 1)
                .unwrap_or(0),
        };
        !self.floating_surfaces[first_occluding_surface..]
            .iter()
            .any(|floating| floating.bounds.intersects(&bounds))
    }
}

fn targets_for_surface(handles: &[DockDividerHandleHitTarget]) -> Vec<DockDividerHitTarget> {
    let core_handles = handles
        .iter()
        .copied()
        .map(core_handle_target_for_dock_target)
        .collect::<Vec<_>>();
    let core_hit_map =
        SplitterHitMap::from_handles(core_handles.iter().map(|handle| handle.core.clone()));
    core_hit_map
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
        .collect()
}

fn surface_for_splitter(splitter: &DockPresentationSplitter) -> DockDividerSurface {
    splitter
        .floating
        .map(DockDividerSurface::Floating)
        .unwrap_or(DockDividerSurface::Root)
}

fn target_surface(target: &DockDividerHitTarget) -> DockDividerSurface {
    match target {
        DockDividerHitTarget::Single(handle) => handle.surface,
        DockDividerHitTarget::Corner(corner) => corner.horizontal.surface,
    }
}

fn target_bounds(target: &DockDividerHitTarget) -> Bounds<Pixels> {
    match target {
        DockDividerHitTarget::Single(handle) => handle.bounds,
        DockDividerHitTarget::Corner(corner) => corner.bounds,
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
        surface: surface_for_splitter(splitter),
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
