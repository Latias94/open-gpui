use crate::{
    DockEdgeDockPlan, DockEdgeDockSizing, DockNodeId, DockPolicy, DockPolicyError, DockSpaceId,
    DropZone,
    geometry::{DockDropBox, DockDropBoxKind, DockDropGuideStyle},
};
use open_gpui::{Bounds, Pixels, Point, Size};

pub(crate) type DockDropTargetValidator<'a> =
    dyn Fn(&DockResolvedDropTarget) -> Result<(), DockPolicyError> + 'a;
pub(crate) type DockEdgePlanResolver<'a> =
    dyn Fn(DockNodeId, DropZone, DockEdgeDockSizing) -> Option<DockEdgeDockPlan> + 'a;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockResolvedDropTarget {
    pub(crate) kind: DockResolvedDropTargetKind,
    pub(crate) source: DockDropResolveSource,
    pub(crate) target_bounds: Option<Bounds<Pixels>>,
    pub(crate) inner_target_bounds: Option<Bounds<Pixels>>,
    pub(crate) availability: DockResolvedDropTargetAvailability,
    pub(crate) drop_box: Option<DockDropBox>,
    pub(crate) hit_bounds: Option<Bounds<Pixels>>,
    pub(crate) preview_bounds: Option<Bounds<Pixels>>,
    pub(crate) tab_insertion_bounds: Option<Bounds<Pixels>>,
    pub(crate) edge_sizing: Option<DockEdgeDockSizing>,
    pub(crate) edge_plan: Option<DockEdgeDockPlan>,
    pub(crate) is_central_region: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockResolvedDropTargetAvailability {
    pub(crate) center: bool,
    pub(crate) sides: bool,
}

impl DockResolvedDropTargetAvailability {
    pub(crate) fn all() -> Self {
        Self {
            center: true,
            sides: true,
        }
    }

    pub(super) fn any(self) -> bool {
        self.center || self.sides
    }
}

impl DockResolvedDropTarget {
    pub(crate) fn target_key(&self) -> DockDropTargetKey {
        DockDropTargetKey {
            kind: self.kind.clone(),
            source: self.source,
            drop_box_kind: self.drop_box.map(|drop_box| drop_box.kind),
            edge_sizing: self.edge_sizing,
            edge_plan: self.edge_plan,
        }
    }

    pub(crate) fn zone(&self) -> Option<DropZone> {
        match self.kind {
            DockResolvedDropTargetKind::TabBar { .. }
            | DockResolvedDropTargetKind::LeafCenter { .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { .. } => Some(DropZone::Center),
            DockResolvedDropTargetKind::InnerEdge { zone, .. }
            | DockResolvedDropTargetKind::RootEdge { zone, .. } => Some(zone),
            DockResolvedDropTargetKind::EmptyDockSpace { .. } => None,
        }
    }

    pub(crate) fn target_space<'a>(&'a self, default_space: &'a DockSpaceId) -> &'a DockSpaceId {
        match &self.kind {
            DockResolvedDropTargetKind::EmptyDockSpace { space, .. } => space,
            DockResolvedDropTargetKind::TabBar { .. }
            | DockResolvedDropTargetKind::LeafCenter { .. }
            | DockResolvedDropTargetKind::InnerEdge { .. }
            | DockResolvedDropTargetKind::RootEdge { .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { .. } => default_space,
        }
    }

    pub(crate) fn center_target_tabs(&self) -> Option<DockNodeId> {
        match self.kind {
            DockResolvedDropTargetKind::TabBar { target_tabs, .. }
            | DockResolvedDropTargetKind::LeafCenter { target_tabs, .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { target_tabs, .. } => Some(target_tabs),
            DockResolvedDropTargetKind::InnerEdge { .. }
            | DockResolvedDropTargetKind::RootEdge { .. }
            | DockResolvedDropTargetKind::EmptyDockSpace { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropTargetKey {
    kind: DockResolvedDropTargetKind,
    source: DockDropResolveSource,
    drop_box_kind: Option<DockDropBoxKind>,
    edge_sizing: Option<DockEdgeDockSizing>,
    edge_plan: Option<DockEdgeDockPlan>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockResolvedDropTargetKind {
    TabBar {
        target_tabs: DockNodeId,
        insert_index: usize,
    },
    LeafCenter {
        root: DockNodeId,
        target_tabs: DockNodeId,
    },
    InnerEdge {
        root: DockNodeId,
        target_tabs: DockNodeId,
        zone: DropZone,
    },
    RootEdge {
        root: DockNodeId,
        leaf_tabs: Option<DockNodeId>,
        zone: DropZone,
    },
    FloatingTitleBar {
        floating: DockNodeId,
        target_tabs: DockNodeId,
    },
    EmptyDockSpace {
        space: DockSpaceId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockDropResolveSource {
    TabBar,
    LeafBody,
    InnerEdge,
    RootEdge,
    FloatingTitleBar,
    EmptyDockSpace,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockTabLabelDropTarget {
    pub(crate) target_tabs: DockNodeId,
    pub(crate) target_index: usize,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) is_central: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockTabBarDropTarget {
    pub(crate) target_tabs: DockNodeId,
    pub(crate) insert_index: usize,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) is_central: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockLeafDropTarget {
    pub(crate) root: DockNodeId,
    pub(crate) target_tabs: DockNodeId,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) is_central: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockRootDropTarget {
    pub(crate) root: DockNodeId,
    pub(crate) bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockFloatingTitleBarDropTarget {
    pub(crate) floating: DockNodeId,
    pub(crate) target_tabs: DockNodeId,
    pub(crate) title_bounds: Bounds<Pixels>,
    pub(crate) preview_bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockEmptySpaceDropTarget {
    pub(crate) space: DockSpaceId,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) is_central: bool,
}

pub(crate) struct DockDropResolverInput<'a> {
    pub(crate) position: Point<Pixels>,
    pub(crate) payload_size: Option<Size<Pixels>>,
    pub(crate) drop_guide_style: DockDropGuideStyle,
    pub(crate) policy: &'a DockPolicy,
    pub(crate) target_validator: Option<&'a DockDropTargetValidator<'a>>,
    pub(crate) edge_plan_resolver: Option<&'a DockEdgePlanResolver<'a>>,
    pub(crate) tab_labels: &'a [DockTabLabelDropTarget],
    pub(crate) tab_bars: &'a [DockTabBarDropTarget],
    pub(crate) leaves: &'a [DockLeafDropTarget],
    pub(crate) root: Option<DockRootDropTarget>,
    pub(crate) floating_title_bars: &'a [DockFloatingTitleBarDropTarget],
    pub(crate) empty_spaces: &'a [DockEmptySpaceDropTarget],
}

impl<'a> DockDropResolverInput<'a> {
    #[cfg(test)]
    pub(crate) fn new(position: Point<Pixels>, policy: &'a DockPolicy) -> Self {
        Self {
            position,
            payload_size: None,
            drop_guide_style: DockDropGuideStyle::default(),
            policy,
            target_validator: None,
            edge_plan_resolver: None,
            tab_labels: &[],
            tab_bars: &[],
            leaves: &[],
            root: None,
            floating_title_bars: &[],
            empty_spaces: &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockDropResolution {
    Valid(DockResolvedDropTarget),
    Rejected(DockDropRejection),
}

impl DockDropResolution {
    #[cfg(test)]
    pub(crate) fn target(self) -> Option<DockResolvedDropTarget> {
        match self {
            Self::Valid(target) => Some(target),
            Self::Rejected(_) => None,
        }
    }

    pub(super) fn is_valid(&self) -> bool {
        matches!(self, Self::Valid(_))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropRejection {
    pub(crate) target: DockResolvedDropTarget,
    pub(crate) reason: DockPolicyError,
}
