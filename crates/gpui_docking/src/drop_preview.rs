use crate::{
    DockEdgeDockSizing, DockNodeId, DockPolicyError, DockViewportDropRoute, DropZone,
    drag::DockDragPayload,
    drop_runtime::resolution_target,
    drop_target::{DockDropResolution, DockResolvedDropTarget, DockResolvedDropTargetKind},
    geometry::{self, DockDropBox, DockDropBoxKind, DockDropBoxSet, DockDropGuideStyle},
};
use open_gpui::{Bounds, Pixels, Point, point, px, size};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DockDropRoutePreviewKind {
    KnownViewport,
    TearOff,
    Rejected,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropPreview {
    pub(crate) scene: DockPreviewScene,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPreviewScene {
    pub(crate) decision: DockPreviewDecision,
    pub(crate) layers: Vec<DockPreviewLayer>,
    pub(crate) body: DockPreviewBody,
    pub(crate) payload_tabs: Option<DockPreviewPayloadTabs>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockPreviewDecision {
    Allowed,
    GuideOnly,
    Rejected { reason: Option<DockPolicyError> },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPreviewLayer {
    pub(crate) kind: DockPreviewLayerKind,
    pub(crate) availability: DockPreviewAvailability,
    pub(crate) active_split: Option<DockPreviewSplit>,
    pub(crate) drop_boxes: Vec<DockPreviewDropBox>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DockPreviewLayerKind {
    Inner,
    Outer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockPreviewAvailability {
    pub(crate) center: bool,
    pub(crate) sides: bool,
}

impl DockPreviewAvailability {
    fn filtered_by_decision(mut self, decision: &DockPreviewDecision) -> Self {
        let DockPreviewDecision::Rejected { reason } = decision else {
            return self;
        };
        match reason {
            Some(
                DockPolicyError::CenterMergeDisabled
                | DockPolicyError::SameStackCenterDropDisabled
                | DockPolicyError::SplitPayloadCenterMergeRejected
                | DockPolicyError::CentralRegionDockOverDisabled,
            ) => {
                self.center = false;
            }
            Some(DockPolicyError::EdgeSplitDisabled) => {
                self.sides = false;
            }
            Some(DockPolicyError::DockClassRejected { .. }) => {
                self.center = false;
                self.sides = false;
            }
            Some(
                DockPolicyError::FloatingDisabled
                | DockPolicyError::PlatformViewportsDisabled
                | DockPolicyError::SplitterResizeDisabled,
            )
            | None => {}
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPreviewSplit {
    pub(crate) zone: DropZone,
    pub(crate) explicit: bool,
    pub(crate) ratio: Option<f32>,
    pub(crate) sizing: Option<DockEdgeDockSizing>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockPreviewDropBox {
    pub(crate) kind: DockDropBoxKind,
    pub(crate) zone: DropZone,
    pub(crate) layer: DockPreviewLayerKind,
    pub(crate) debug_node: Option<DockNodeId>,
    pub(crate) hit_bounds: Bounds<Pixels>,
    pub(crate) draw_bounds: Bounds<Pixels>,
    pub(crate) preview_bounds: Bounds<Pixels>,
    pub(crate) active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockPreviewBody {
    pub(crate) future_bounds: Bounds<Pixels>,
    pub(crate) body_bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPreviewPayloadTabs {
    pub(crate) target_tabs: Option<DockNodeId>,
    pub(crate) insert_index: Option<usize>,
    pub(crate) insertion: Option<DockPreviewTabInsertion>,
    pub(crate) tabs: Vec<DockPreviewPayloadTab>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPreviewTabInsertion {
    pub(crate) target_tabs: Option<DockNodeId>,
    pub(crate) index: DockPreviewTabInsertionIndex,
    pub(crate) slot_bounds: Option<Bounds<Pixels>>,
    pub(crate) clipping_bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockPreviewTabInsertionIndex {
    At(usize),
    Append,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockPreviewPayloadTab {
    pub(crate) title: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockDropRoutePreview {
    pub(crate) kind: DockDropRoutePreviewKind,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) rejected: bool,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPreviewVisualDescriptor {
    pub(crate) decision: DockPreviewVisualDecision,
    pub(crate) active_layer: Option<DockPreviewLayerKind>,
    pub(crate) active_zone: Option<DropZone>,
    pub(crate) tab_insertion: Option<DockPreviewTabInsertionVisualDescriptor>,
    pub(crate) payload_tabs: Vec<DockPreviewPayloadTabVisualDescriptor>,
    pub(crate) has_body: bool,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DockPreviewVisualDecision {
    Allowed,
    GuideOnly,
    Rejected,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockPreviewPayloadTabVisualDescriptor {
    pub(crate) index: usize,
    pub(crate) title: String,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPreviewTabInsertionVisualDescriptor {
    pub(crate) target_tabs: Option<DockNodeId>,
    pub(crate) index: DockPreviewTabInsertionIndex,
    pub(crate) has_slot_bounds: bool,
    pub(crate) slot_bounds: Option<Bounds<Pixels>>,
    pub(crate) clipping_bounds: Bounds<Pixels>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockRoutePreviewVisualDescriptor {
    pub(crate) kind: DockDropRoutePreviewKind,
    pub(crate) rejected: bool,
}

impl DockDropPreview {
    pub(crate) fn from_resolution(
        resolution: &DockDropResolution,
        style: DockDropGuideStyle,
    ) -> Option<Self> {
        let target = resolution_target(resolution)?;
        let decision = match resolution {
            DockDropResolution::Valid(_) => DockPreviewDecision::allowed(),
            DockDropResolution::Rejected(rejection) => {
                DockPreviewDecision::rejected(Some(rejection.reason.clone()))
            }
        };
        Self::from_target(target, decision, style)
    }

    pub(crate) fn from_resolved_target(
        target: &DockResolvedDropTarget,
        style: DockDropGuideStyle,
    ) -> Option<Self> {
        Self::from_target(target, DockPreviewDecision::allowed(), style)
    }

    pub(crate) fn from_guide_target(
        target: &DockResolvedDropTarget,
        style: DockDropGuideStyle,
    ) -> Option<Self> {
        Self::from_target(target, DockPreviewDecision::guide_only(), style)
    }

    pub(crate) fn from_rejected_target(
        target: &DockResolvedDropTarget,
        style: DockDropGuideStyle,
    ) -> Option<Self> {
        Self::from_target(target, DockPreviewDecision::rejected(None), style)
    }

    fn from_target(
        target: &DockResolvedDropTarget,
        decision: DockPreviewDecision,
        style: DockDropGuideStyle,
    ) -> Option<Self> {
        let preview_bounds = match &target.kind {
            DockResolvedDropTargetKind::TabBar { .. }
            | DockResolvedDropTargetKind::LeafCenter { .. }
            | DockResolvedDropTargetKind::InnerEdge { .. }
            | DockResolvedDropTargetKind::RootEdge { .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { .. }
            | DockResolvedDropTargetKind::EmptyDockSpace { .. } => target.preview_bounds?,
        };
        let target_bounds = target.target_bounds.unwrap_or(preview_bounds);
        let (target_tabs, insert_index) = match target.kind {
            DockResolvedDropTargetKind::TabBar {
                target_tabs,
                insert_index,
            } => (Some(target_tabs), Some(insert_index)),
            DockResolvedDropTargetKind::LeafCenter { target_tabs, .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { target_tabs, .. } => {
                (Some(target_tabs), None)
            }
            DockResolvedDropTargetKind::InnerEdge { .. }
            | DockResolvedDropTargetKind::RootEdge { .. }
            | DockResolvedDropTargetKind::EmptyDockSpace { .. } => (None, None),
        };
        let scene = DockPreviewScene::from_target(
            target,
            target_bounds,
            preview_bounds,
            target_tabs,
            insert_index,
            decision,
            style,
        );

        Some(Self { scene })
    }

    pub(crate) fn populate_payload_tabs(&mut self, payload: &DockDragPayload) {
        let Some(payload_tabs) = self.scene.payload_tabs.as_mut() else {
            return;
        };
        *payload_tabs = DockPreviewPayloadTabs::from_payload(
            payload_tabs.target_tabs,
            payload_tabs.insert_index,
            payload_tabs.insertion.clone(),
            payload,
        );
    }

    #[cfg(test)]
    pub(crate) fn visual_descriptor(&self) -> DockPreviewVisualDescriptor {
        self.scene.visual_descriptor()
    }
}

impl DockPreviewScene {
    pub(crate) fn from_target(
        target: &DockResolvedDropTarget,
        target_bounds: Bounds<Pixels>,
        body_bounds: Bounds<Pixels>,
        target_tabs: Option<DockNodeId>,
        insert_index: Option<usize>,
        decision: DockPreviewDecision,
        style: DockDropGuideStyle,
    ) -> Self {
        let body_bounds = if decision.is_guide_only() {
            Bounds::new(body_bounds.origin, size(px(0.0), px(0.0)))
        } else {
            body_bounds
        };
        let payload_tabs =
            payload_tabs_for_target(target, target_tabs, insert_index, &decision, body_bounds);
        let layers = preview_layers_for_target(target, target_bounds, &decision, style);
        Self {
            decision,
            layers,
            body: DockPreviewBody {
                future_bounds: target_bounds,
                body_bounds,
            },
            payload_tabs,
        }
    }

    pub(crate) fn active_split(&self) -> Option<&DockPreviewSplit> {
        self.layers
            .iter()
            .find_map(|layer| layer.active_split.as_ref())
    }

    #[cfg(test)]
    pub(crate) fn visual_descriptor(&self) -> DockPreviewVisualDescriptor {
        let active_drop_box = self
            .layers
            .iter()
            .flat_map(|layer| layer.drop_boxes.iter())
            .find(|drop_box| drop_box.active);
        DockPreviewVisualDescriptor {
            decision: DockPreviewVisualDecision::from(&self.decision),
            active_layer: active_drop_box.map(|drop_box| drop_box.layer),
            active_zone: active_drop_box.map(|drop_box| drop_box.zone),
            tab_insertion: self
                .payload_tabs
                .as_ref()
                .and_then(|payload_tabs| payload_tabs.insertion.as_ref())
                .map(DockPreviewTabInsertionVisualDescriptor::from),
            payload_tabs: self
                .payload_tabs
                .as_ref()
                .map(|payload_tabs| payload_tabs.visual_descriptors())
                .unwrap_or_default(),
            has_body: self.body.body_bounds.size.width > px(0.0)
                && self.body.body_bounds.size.height > px(0.0),
        }
    }

    #[cfg(test)]
    pub(crate) fn active_drop_box(&self) -> Option<&DockPreviewDropBox> {
        self.layers
            .iter()
            .flat_map(|layer| layer.drop_boxes.iter())
            .find(|drop_box| drop_box.active)
    }
}

#[cfg(test)]
impl From<&DockPreviewDecision> for DockPreviewVisualDecision {
    fn from(decision: &DockPreviewDecision) -> Self {
        match decision {
            DockPreviewDecision::Allowed => Self::Allowed,
            DockPreviewDecision::GuideOnly => Self::GuideOnly,
            DockPreviewDecision::Rejected { .. } => Self::Rejected,
        }
    }
}

impl DockPreviewDecision {
    pub(crate) fn allowed() -> Self {
        Self::Allowed
    }

    pub(crate) fn rejected(reason: Option<DockPolicyError>) -> Self {
        Self::Rejected { reason }
    }

    pub(crate) fn guide_only() -> Self {
        Self::GuideOnly
    }

    pub(crate) fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    pub(crate) fn is_guide_only(&self) -> bool {
        matches!(self, Self::GuideOnly)
    }

    #[cfg(test)]
    pub(crate) fn rejection_reason(&self) -> Option<&DockPolicyError> {
        match self {
            Self::Allowed => None,
            Self::GuideOnly => None,
            Self::Rejected { reason } => reason.as_ref(),
        }
    }
}

impl DockPreviewPayloadTabs {
    pub(crate) fn from_payload(
        target_tabs: Option<DockNodeId>,
        insert_index: Option<usize>,
        insertion: Option<DockPreviewTabInsertion>,
        payload: &DockDragPayload,
    ) -> Self {
        let tabs = payload
            .preview_tabs()
            .into_iter()
            .map(|title| DockPreviewPayloadTab {
                title: title.to_string(),
            })
            .collect();
        Self {
            target_tabs,
            insert_index,
            insertion,
            tabs,
        }
    }

    #[cfg(test)]
    fn visual_descriptors(&self) -> Vec<DockPreviewPayloadTabVisualDescriptor> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| DockPreviewPayloadTabVisualDescriptor {
                index,
                title: tab.title.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
impl From<&DockPreviewTabInsertion> for DockPreviewTabInsertionVisualDescriptor {
    fn from(insertion: &DockPreviewTabInsertion) -> Self {
        Self {
            target_tabs: insertion.target_tabs,
            index: insertion.index,
            has_slot_bounds: insertion.slot_bounds.is_some(),
            slot_bounds: insertion.slot_bounds,
            clipping_bounds: insertion.clipping_bounds,
        }
    }
}

fn payload_tabs_for_target(
    target: &DockResolvedDropTarget,
    target_tabs: Option<DockNodeId>,
    insert_index: Option<usize>,
    decision: &DockPreviewDecision,
    clipping_bounds: Bounds<Pixels>,
) -> Option<DockPreviewPayloadTabs> {
    if !decision.is_allowed() {
        return None;
    }

    match target.kind {
        DockResolvedDropTargetKind::TabBar { .. }
        | DockResolvedDropTargetKind::LeafCenter { .. }
        | DockResolvedDropTargetKind::FloatingTitleBar { .. }
        | DockResolvedDropTargetKind::EmptyDockSpace { .. } => Some(DockPreviewPayloadTabs {
            target_tabs,
            insert_index,
            insertion: Some(DockPreviewTabInsertion {
                target_tabs,
                index: insert_index
                    .map(DockPreviewTabInsertionIndex::At)
                    .unwrap_or(DockPreviewTabInsertionIndex::Append),
                slot_bounds: None,
                clipping_bounds,
            }),
            tabs: Vec::new(),
        }),
        DockResolvedDropTargetKind::InnerEdge { .. }
        | DockResolvedDropTargetKind::RootEdge { .. } => None,
    }
}

fn preview_layers_for_target(
    target: &DockResolvedDropTarget,
    bounds: Bounds<Pixels>,
    decision: &DockPreviewDecision,
    style: DockDropGuideStyle,
) -> Vec<DockPreviewLayer> {
    let mut layers = Vec::new();
    if let Some(inner_bounds) = inner_layer_bounds_for_target(target, bounds) {
        layers.push(preview_layer_for_target(
            target,
            inner_bounds,
            DockPreviewLayerKind::Inner,
            decision,
            style,
        ));
    }
    if outer_layer_available_for_target(target) {
        layers.push(preview_layer_for_target(
            target,
            bounds,
            DockPreviewLayerKind::Outer,
            decision,
            style,
        ));
    }
    layers
}

fn preview_layer_for_target(
    target: &DockResolvedDropTarget,
    bounds: Bounds<Pixels>,
    kind: DockPreviewLayerKind,
    decision: &DockPreviewDecision,
    style: DockDropGuideStyle,
) -> DockPreviewLayer {
    let active = active_layer_for_target(target) == Some(kind) && !decision.is_guide_only();
    let zone = active
        .then(|| target.zone())
        .flatten()
        .filter(|zone| drop_zone_allowed_for_target(target, kind, *zone, decision));
    let active_split = match zone {
        Some(zone @ (DropZone::Left | DropZone::Right | DropZone::Top | DropZone::Bottom)) => {
            Some(DockPreviewSplit {
                zone,
                explicit: true,
                ratio: target.edge_sizing.map(DockEdgeDockSizing::new_child_share),
                sizing: target.edge_sizing,
            })
        }
        Some(DropZone::Center) | None => None,
    };
    DockPreviewLayer {
        kind,
        availability: availability_for_target(target, kind, decision),
        active_split,
        drop_boxes: preview_drop_boxes_for_layer(target, bounds, kind, decision, style),
    }
}

fn active_layer_for_target(target: &DockResolvedDropTarget) -> Option<DockPreviewLayerKind> {
    match target.kind {
        DockResolvedDropTargetKind::RootEdge { .. } => Some(DockPreviewLayerKind::Outer),
        DockResolvedDropTargetKind::TabBar { .. }
        | DockResolvedDropTargetKind::LeafCenter { .. }
        | DockResolvedDropTargetKind::InnerEdge { .. }
        | DockResolvedDropTargetKind::FloatingTitleBar { .. }
        | DockResolvedDropTargetKind::EmptyDockSpace { .. } => Some(DockPreviewLayerKind::Inner),
    }
}

fn inner_layer_bounds_for_target(
    target: &DockResolvedDropTarget,
    fallback_bounds: Bounds<Pixels>,
) -> Option<Bounds<Pixels>> {
    match target.kind {
        DockResolvedDropTargetKind::RootEdge { .. } => target.inner_target_bounds,
        DockResolvedDropTargetKind::TabBar { .. }
        | DockResolvedDropTargetKind::LeafCenter { .. }
        | DockResolvedDropTargetKind::InnerEdge { .. }
        | DockResolvedDropTargetKind::FloatingTitleBar { .. }
        | DockResolvedDropTargetKind::EmptyDockSpace { .. } => Some(fallback_bounds),
    }
}

fn outer_layer_available_for_target(target: &DockResolvedDropTarget) -> bool {
    matches!(target.kind, DockResolvedDropTargetKind::RootEdge { .. })
}

fn availability_for_target(
    target: &DockResolvedDropTarget,
    layer: DockPreviewLayerKind,
    decision: &DockPreviewDecision,
) -> DockPreviewAvailability {
    let availability = match (&target.kind, layer) {
        (DockResolvedDropTargetKind::RootEdge { .. }, DockPreviewLayerKind::Inner) => {
            DockPreviewAvailability {
                center: false,
                sides: false,
            }
        }
        (DockResolvedDropTargetKind::RootEdge { .. }, DockPreviewLayerKind::Outer) => {
            DockPreviewAvailability {
                center: false,
                sides: target.availability.sides,
            }
        }
        (DockResolvedDropTargetKind::InnerEdge { .. }, DockPreviewLayerKind::Inner) => {
            DockPreviewAvailability {
                center: target.availability.center,
                sides: target.availability.sides,
            }
        }
        (
            DockResolvedDropTargetKind::TabBar { .. }
            | DockResolvedDropTargetKind::LeafCenter { .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { .. },
            DockPreviewLayerKind::Inner,
        ) => DockPreviewAvailability {
            center: target.availability.center,
            sides: target.availability.sides,
        },
        (DockResolvedDropTargetKind::EmptyDockSpace { .. }, DockPreviewLayerKind::Inner) => {
            DockPreviewAvailability {
                center: target.availability.center,
                sides: false,
            }
        }
        _ => DockPreviewAvailability {
            center: false,
            sides: false,
        },
    };
    availability.filtered_by_decision(decision)
}

fn preview_drop_boxes_for_layer(
    target: &DockResolvedDropTarget,
    bounds: Bounds<Pixels>,
    layer: DockPreviewLayerKind,
    decision: &DockPreviewDecision,
    style: DockDropGuideStyle,
) -> Vec<DockPreviewDropBox> {
    let set = match layer {
        DockPreviewLayerKind::Inner => DockDropBoxSet::Inner,
        DockPreviewLayerKind::Outer => DockDropBoxSet::Outer,
    };
    let active_drop_box = target
        .drop_box
        .filter(|drop_box| drop_box_allowed_for_target(target, layer, drop_box.kind, decision));
    let active_kind = active_drop_box.map(|drop_box| drop_box.kind);
    let debug_node = debug_node_for_target(target, layer);
    geometry::drop_boxes_with_style(bounds, set, style)
        .into_iter()
        .filter(|drop_box| drop_box_allowed_for_target(target, layer, drop_box.kind, decision))
        .map(|drop_box| {
            let active = Some(drop_box.kind) == active_kind;
            let drop_box = if active {
                active_drop_box.unwrap_or(drop_box)
            } else {
                drop_box
            };
            preview_drop_box(drop_box, layer, debug_node, active)
        })
        .collect()
}

fn drop_box_allowed_for_target(
    target: &DockResolvedDropTarget,
    layer: DockPreviewLayerKind,
    kind: DockDropBoxKind,
    decision: &DockPreviewDecision,
) -> bool {
    let availability = availability_for_target(target, layer, decision);
    match kind {
        DockDropBoxKind::Center => availability.center,
        DockDropBoxKind::InnerEdge(_) | DockDropBoxKind::OuterEdge(_) => availability.sides,
    }
}

fn drop_zone_allowed_for_target(
    target: &DockResolvedDropTarget,
    layer: DockPreviewLayerKind,
    zone: DropZone,
    decision: &DockPreviewDecision,
) -> bool {
    let availability = availability_for_target(target, layer, decision);
    match zone {
        DropZone::Center => availability.center,
        DropZone::Left | DropZone::Right | DropZone::Top | DropZone::Bottom => availability.sides,
    }
}

fn preview_drop_box(
    drop_box: DockDropBox,
    layer: DockPreviewLayerKind,
    debug_node: Option<DockNodeId>,
    active: bool,
) -> DockPreviewDropBox {
    DockPreviewDropBox {
        kind: drop_box.kind,
        zone: drop_box.kind.zone(),
        layer,
        debug_node,
        hit_bounds: drop_box.hit_bounds,
        draw_bounds: drop_box.draw_bounds,
        preview_bounds: drop_box.preview_bounds,
        active,
    }
}

fn debug_node_for_target(
    target: &DockResolvedDropTarget,
    layer: DockPreviewLayerKind,
) -> Option<DockNodeId> {
    if layer != DockPreviewLayerKind::Inner {
        return None;
    }

    match target.kind {
        DockResolvedDropTargetKind::TabBar { target_tabs, .. }
        | DockResolvedDropTargetKind::LeafCenter { target_tabs, .. }
        | DockResolvedDropTargetKind::InnerEdge { target_tabs, .. }
        | DockResolvedDropTargetKind::FloatingTitleBar { target_tabs, .. } => Some(target_tabs),
        DockResolvedDropTargetKind::RootEdge { .. }
        | DockResolvedDropTargetKind::EmptyDockSpace { .. } => None,
    }
}

impl DockDropRoutePreview {
    pub(crate) fn from_route(
        route: &DockViewportDropRoute,
        host_position: Point<Pixels>,
    ) -> Option<Self> {
        let (kind, rejected) = match route {
            DockViewportDropRoute::Local { .. } => return None,
            DockViewportDropRoute::KnownViewport { .. } => {
                (DockDropRoutePreviewKind::KnownViewport, false)
            }
            DockViewportDropRoute::TearOff => (DockDropRoutePreviewKind::TearOff, false),
            DockViewportDropRoute::Unavailable => return None,
            DockViewportDropRoute::Rejected(_) => (DockDropRoutePreviewKind::Rejected, true),
        };

        Some(Self {
            kind,
            bounds: route_bounds(host_position),
            rejected,
        })
    }

    #[cfg(test)]
    pub(crate) fn visual_descriptor(&self) -> DockRoutePreviewVisualDescriptor {
        DockRoutePreviewVisualDescriptor {
            kind: self.kind,
            rejected: self.rejected,
        }
    }
}

fn route_bounds(anchor: Point<Pixels>) -> Bounds<Pixels> {
    let marker = size(px(56.0), px(40.0));
    Bounds::new(
        point(
            anchor.x - marker.width / 2.0,
            anchor.y - marker.height / 2.0,
        ),
        marker,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockEdgeDockSizing, DockPolicyError, DockViewportRouteSelectionSource,
        DockViewportTargetHit,
        drop_target::{
            DockDropRejection, DockDropResolveSource, DockResolvedDropTargetAvailability,
            DockResolvedDropTargetKind,
        },
        geometry::{DockDropBox, DockDropBoxKind},
        viewport_test_support::{handle, space},
    };
    use open_gpui::{point, px};
    use slotmap::Key;

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    fn drop_box(kind: DockDropBoxKind) -> DockDropBox {
        let hit_bounds = bounds(20.0, 30.0, 44.0, 36.0);
        let draw_bounds = bounds(22.0, 32.0, 40.0, 32.0);
        let preview_bounds = bounds(0.0, 0.0, 120.0, 80.0);
        DockDropBox {
            kind,
            hit_bounds,
            draw_bounds,
            preview_bounds,
        }
    }

    fn resolved_target(
        kind: DockResolvedDropTargetKind,
        drop_box: Option<DockDropBox>,
    ) -> DockResolvedDropTarget {
        let edge_sizing = match kind {
            DockResolvedDropTargetKind::InnerEdge { .. }
            | DockResolvedDropTargetKind::RootEdge { .. } => Some(DockEdgeDockSizing::fallback()),
            DockResolvedDropTargetKind::TabBar { .. }
            | DockResolvedDropTargetKind::LeafCenter { .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { .. }
            | DockResolvedDropTargetKind::EmptyDockSpace { .. } => None,
        };
        DockResolvedDropTarget {
            kind,
            source: DockDropResolveSource::LeafBody,
            target_bounds: Some(bounds(0.0, 0.0, 320.0, 200.0)),
            inner_target_bounds: Some(bounds(0.0, 0.0, 320.0, 200.0)),
            availability: DockResolvedDropTargetAvailability::all(),
            drop_box,
            preview_bounds: Some(bounds(0.0, 0.0, 320.0, 200.0)),
            edge_sizing,
            edge_plan: None,
            is_central_region: false,
        }
    }

    #[test]
    fn center_target_builds_scene_with_payload_tab_capability() {
        let tabs = DockNodeId::null();
        let preview = DockDropPreview::from_resolved_target(
            &resolved_target(
                DockResolvedDropTargetKind::LeafCenter {
                    root: tabs,
                    target_tabs: tabs,
                },
                Some(drop_box(DockDropBoxKind::Center)),
            ),
            DockDropGuideStyle::default(),
        )
        .expect("center target should produce preview");

        assert!(preview.scene.decision.is_allowed());
        assert!(preview.scene.payload_tabs.is_some());
        assert_eq!(preview.scene.active_split(), None);
        assert_eq!(
            preview
                .scene
                .active_drop_box()
                .map(|drop_box| drop_box.kind),
            Some(DockDropBoxKind::Center)
        );
        assert_eq!(preview.scene.layers[0].kind, DockPreviewLayerKind::Inner);
        assert!(preview.scene.layers[0].availability.center);
        assert!(preview.scene.layers[0].availability.sides);
        assert_eq!(preview.scene.layers[0].drop_boxes.len(), 5);
        assert_eq!(
            preview
                .scene
                .layers
                .iter()
                .flat_map(|layer| layer.drop_boxes.iter())
                .filter(|drop_box| drop_box.active)
                .count(),
            1
        );
    }

    #[test]
    fn guide_only_target_builds_drop_boxes_without_delivery_body() {
        let tabs = DockNodeId::null();
        let preview = DockDropPreview::from_guide_target(
            &resolved_target(
                DockResolvedDropTargetKind::LeafCenter {
                    root: tabs,
                    target_tabs: tabs,
                },
                None,
            ),
            DockDropGuideStyle::default(),
        )
        .expect("guide target should produce preview");
        let visual = preview.visual_descriptor();

        assert_eq!(visual.decision, DockPreviewVisualDecision::GuideOnly);
        assert_eq!(visual.active_layer, None);
        assert_eq!(visual.active_zone, None);
        assert_eq!(visual.payload_tabs, Vec::new());
        assert!(!visual.has_body);
        assert_eq!(preview.scene.layers[0].kind, DockPreviewLayerKind::Inner);
        assert_eq!(preview.scene.layers[0].drop_boxes.len(), 5);
        assert!(
            preview
                .scene
                .layers
                .iter()
                .flat_map(|layer| layer.drop_boxes.iter())
                .all(|drop_box| !drop_box.active),
            "guide-only preview should render passive drop boxes without selecting a drop target"
        );
    }

    #[test]
    fn guide_only_root_target_does_not_activate_split_preview() {
        let root = DockNodeId::null();
        let preview = DockDropPreview::from_guide_target(
            &resolved_target(
                DockResolvedDropTargetKind::RootEdge {
                    root,
                    leaf_tabs: None,
                    zone: DropZone::Left,
                },
                None,
            ),
            DockDropGuideStyle::default(),
        )
        .expect("root guide target should produce preview");
        let visual = preview.visual_descriptor();

        assert_eq!(visual.decision, DockPreviewVisualDecision::GuideOnly);
        assert_eq!(visual.active_layer, None);
        assert_eq!(visual.active_zone, None);
        assert_eq!(preview.scene.active_split(), None);
        assert!(!visual.has_body);
        assert_eq!(preview.scene.layers.len(), 2);
        assert!(
            preview
                .scene
                .layers
                .iter()
                .flat_map(|layer| layer.drop_boxes.iter())
                .all(|drop_box| !drop_box.active)
        );
    }

    #[test]
    fn edge_target_builds_scene_with_explicit_split_and_no_payload_tabs() {
        let tabs = DockNodeId::null();
        let preview = DockDropPreview::from_resolved_target(
            &resolved_target(
                DockResolvedDropTargetKind::InnerEdge {
                    root: tabs,
                    target_tabs: tabs,
                    zone: DropZone::Left,
                },
                Some(drop_box(DockDropBoxKind::InnerEdge(DropZone::Left))),
            ),
            DockDropGuideStyle::default(),
        )
        .expect("edge target should produce preview");

        let split = preview
            .scene
            .active_split()
            .expect("edge target should expose active split");
        assert_eq!(split.zone, DropZone::Left);
        assert!(split.explicit);
        assert_eq!(
            split.ratio,
            Some(DockEdgeDockSizing::fallback().new_child_share())
        );
        assert!(preview.scene.payload_tabs.is_none());
        assert_eq!(preview.scene.layers[0].kind, DockPreviewLayerKind::Inner);
        assert!(preview.scene.layers[0].availability.sides);
        assert_eq!(
            preview
                .scene
                .active_drop_box()
                .map(|drop_box| drop_box.draw_bounds),
            Some(bounds(22.0, 32.0, 40.0, 32.0))
        );
        assert_eq!(
            preview
                .scene
                .active_drop_box()
                .map(|drop_box| drop_box.preview_bounds),
            Some(bounds(0.0, 0.0, 120.0, 80.0))
        );
    }

    #[test]
    fn root_edge_target_preserves_inner_layer_but_activates_outer_layer() {
        let root = DockNodeId::null();
        let preview = DockDropPreview::from_resolved_target(
            &resolved_target(
                DockResolvedDropTargetKind::RootEdge {
                    root,
                    leaf_tabs: None,
                    zone: DropZone::Right,
                },
                Some(drop_box(DockDropBoxKind::OuterEdge(DropZone::Right))),
            ),
            DockDropGuideStyle::default(),
        )
        .expect("root edge target should produce preview");

        assert_eq!(preview.scene.layers.len(), 2);
        assert_eq!(preview.scene.layers[0].kind, DockPreviewLayerKind::Inner);
        assert_eq!(preview.scene.layers[1].kind, DockPreviewLayerKind::Outer);
        assert!(!preview.scene.layers[0].availability.center);
        assert!(!preview.scene.layers[0].availability.sides);
        assert!(preview.scene.layers[0].drop_boxes.is_empty());
        assert!(!preview.scene.layers[1].availability.center);
        assert!(preview.scene.layers[1].availability.sides);
        assert_eq!(
            preview
                .scene
                .active_drop_box()
                .map(|drop_box| drop_box.layer),
            Some(DockPreviewLayerKind::Outer)
        );
    }

    #[test]
    fn rejected_resolution_builds_rejected_scene_without_payload_tabs() {
        let tabs = DockNodeId::null();
        let target = resolved_target(
            DockResolvedDropTargetKind::LeafCenter {
                root: tabs,
                target_tabs: tabs,
            },
            Some(drop_box(DockDropBoxKind::Center)),
        );
        let preview = DockDropPreview::from_resolution(
            &DockDropResolution::Rejected(DockDropRejection {
                target,
                reason: DockPolicyError::CenterMergeDisabled,
            }),
            DockDropGuideStyle::default(),
        )
        .expect("rejected target should still produce preview");

        assert!(!preview.scene.decision.is_allowed());
        assert_eq!(
            preview.scene.decision.rejection_reason(),
            Some(&DockPolicyError::CenterMergeDisabled)
        );
        assert!(preview.scene.payload_tabs.is_none());
    }

    #[test]
    fn known_viewport_route_preview_uses_host_pointer_anchor() {
        let preview = DockDropRoutePreview::from_route(
            &DockViewportDropRoute::KnownViewport {
                target: DockViewportTargetHit::new(
                    space("target"),
                    handle(7),
                    point(px(300.0), px(20.0)),
                ),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            },
            point(px(40.0), px(50.0)),
        )
        .expect("known viewport route should produce a preview");

        assert_eq!(preview.kind, DockDropRoutePreviewKind::KnownViewport);
        assert!(!preview.rejected);
        assert!(preview.bounds.contains(&point(px(40.0), px(50.0))));
    }

    #[test]
    fn tear_off_route_preview_is_visible_without_receiver_bounds() {
        let preview = DockDropRoutePreview::from_route(
            &DockViewportDropRoute::TearOff,
            point(px(100.0), px(120.0)),
        )
        .expect("tear-off route should produce a preview");

        assert_eq!(preview.kind, DockDropRoutePreviewKind::TearOff);
        assert!(!preview.rejected);
        assert!(preview.bounds.contains(&point(px(100.0), px(120.0))));
    }

    #[test]
    fn rejected_route_preview_is_marked_rejected() {
        let preview = DockDropRoutePreview::from_route(
            &DockViewportDropRoute::Rejected(DockPolicyError::PlatformViewportsDisabled),
            point(px(12.0), px(34.0)),
        )
        .expect("rejected route should produce a preview");

        assert_eq!(preview.kind, DockDropRoutePreviewKind::Rejected);
        assert!(preview.rejected);
        assert!(preview.bounds.contains(&point(px(12.0), px(34.0))));
    }

    #[test]
    fn unavailable_route_preview_is_hidden() {
        assert_eq!(
            DockDropRoutePreview::from_route(
                &DockViewportDropRoute::Unavailable,
                point(px(12.0), px(34.0)),
            ),
            None
        );
    }
}
