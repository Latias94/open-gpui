use crate::{
    DockEdgeDockSizing, DockNodeId, DockPolicyError, DockViewportDropRoute, DropZone,
    drag::DockDragPayload,
    drop_runtime::resolution_target,
    drop_target::{DockDropResolution, DockResolvedDropTarget, DockResolvedDropTargetKind},
    geometry::{DockDropBox, DockDropBoxKind},
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
    Rejected { reason: Option<DockPolicyError> },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPreviewLayer {
    pub(crate) kind: DockPreviewLayerKind,
    pub(crate) active_zones: DockPreviewActiveZones,
    pub(crate) active_split: Option<DockPreviewSplit>,
    pub(crate) drop_boxes: Vec<DockPreviewDropBox>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DockPreviewLayerKind {
    Inner,
    Outer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DockPreviewActiveZones {
    pub(crate) center: bool,
    pub(crate) side: bool,
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
    pub(crate) tabs: Vec<DockPreviewPayloadTab>,
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

impl DockDropPreview {
    pub(crate) fn from_resolution(resolution: &DockDropResolution) -> Option<Self> {
        let target = resolution_target(resolution)?;
        let decision = match resolution {
            DockDropResolution::Valid(_) => DockPreviewDecision::allowed(),
            DockDropResolution::Rejected(rejection) => {
                DockPreviewDecision::rejected(Some(rejection.reason.clone()))
            }
        };
        Self::from_target(target, decision)
    }

    pub(crate) fn from_resolved_target(target: &DockResolvedDropTarget) -> Option<Self> {
        Self::from_target(target, DockPreviewDecision::allowed())
    }

    pub(crate) fn from_rejected_target(target: &DockResolvedDropTarget) -> Option<Self> {
        Self::from_target(target, DockPreviewDecision::rejected(None))
    }

    fn from_target(target: &DockResolvedDropTarget, decision: DockPreviewDecision) -> Option<Self> {
        let bounds = match &target.kind {
            DockResolvedDropTargetKind::TabBar { .. }
            | DockResolvedDropTargetKind::LeafCenter { .. }
            | DockResolvedDropTargetKind::InnerEdge { .. }
            | DockResolvedDropTargetKind::RootEdge { .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { .. }
            | DockResolvedDropTargetKind::EmptyDockSpace { .. } => target.preview_bounds?,
        };
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
        let scene =
            DockPreviewScene::from_target(target, bounds, target_tabs, insert_index, decision);

        Some(Self { scene })
    }

    pub(crate) fn populate_payload_tabs(&mut self, payload: &DockDragPayload) {
        let Some(payload_tabs) = self.scene.payload_tabs.as_mut() else {
            return;
        };
        *payload_tabs = DockPreviewPayloadTabs::from_payload(
            payload_tabs.target_tabs,
            payload_tabs.insert_index,
            payload,
        );
    }
}

impl DockPreviewScene {
    pub(crate) fn from_target(
        target: &DockResolvedDropTarget,
        bounds: Bounds<Pixels>,
        target_tabs: Option<DockNodeId>,
        insert_index: Option<usize>,
        decision: DockPreviewDecision,
    ) -> Self {
        let payload_tabs = payload_tabs_for_target(target, target_tabs, insert_index, &decision);
        let layer = preview_layer_for_target(target);
        Self {
            decision,
            layers: vec![layer],
            body: DockPreviewBody {
                future_bounds: bounds,
                body_bounds: bounds,
            },
            payload_tabs,
        }
    }

    pub(crate) fn active_split(&self) -> Option<&DockPreviewSplit> {
        self.layers
            .iter()
            .find_map(|layer| layer.active_split.as_ref())
    }

    pub(crate) fn active_drop_box(&self) -> Option<&DockPreviewDropBox> {
        self.layers
            .iter()
            .flat_map(|layer| layer.drop_boxes.iter())
            .find(|drop_box| drop_box.active)
    }
}

impl DockPreviewDecision {
    pub(crate) fn allowed() -> Self {
        Self::Allowed
    }

    pub(crate) fn rejected(reason: Option<DockPolicyError>) -> Self {
        Self::Rejected { reason }
    }

    pub(crate) fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    #[cfg(test)]
    pub(crate) fn rejection_reason(&self) -> Option<&DockPolicyError> {
        match self {
            Self::Allowed => None,
            Self::Rejected { reason } => reason.as_ref(),
        }
    }
}

impl DockPreviewPayloadTabs {
    pub(crate) fn from_payload(
        target_tabs: Option<DockNodeId>,
        insert_index: Option<usize>,
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
            tabs,
        }
    }
}

fn payload_tabs_for_target(
    target: &DockResolvedDropTarget,
    target_tabs: Option<DockNodeId>,
    insert_index: Option<usize>,
    decision: &DockPreviewDecision,
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
            tabs: Vec::new(),
        }),
        DockResolvedDropTargetKind::InnerEdge { .. }
        | DockResolvedDropTargetKind::RootEdge { .. } => None,
    }
}

fn preview_layer_for_target(target: &DockResolvedDropTarget) -> DockPreviewLayer {
    let kind = match target.kind {
        DockResolvedDropTargetKind::RootEdge { .. } => DockPreviewLayerKind::Outer,
        DockResolvedDropTargetKind::TabBar { .. }
        | DockResolvedDropTargetKind::LeafCenter { .. }
        | DockResolvedDropTargetKind::InnerEdge { .. }
        | DockResolvedDropTargetKind::FloatingTitleBar { .. }
        | DockResolvedDropTargetKind::EmptyDockSpace { .. } => DockPreviewLayerKind::Inner,
    };
    let zone = target.zone();
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
        active_zones: DockPreviewActiveZones {
            center: matches!(zone, Some(DropZone::Center) | None),
            side: active_split.is_some(),
        },
        active_split,
        drop_boxes: target
            .drop_box
            .map(|drop_box| vec![preview_drop_box(drop_box, kind, true)])
            .unwrap_or_default(),
    }
}

fn preview_drop_box(
    drop_box: DockDropBox,
    layer: DockPreviewLayerKind,
    active: bool,
) -> DockPreviewDropBox {
    DockPreviewDropBox {
        kind: drop_box.kind,
        zone: drop_box.kind.zone(),
        layer,
        hit_bounds: drop_box.hit_bounds,
        draw_bounds: drop_box.draw_bounds,
        preview_bounds: drop_box.preview_bounds,
        active,
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
        drop_target::{DockDropRejection, DockDropResolveSource, DockResolvedDropTargetKind},
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
        let preview = DockDropPreview::from_resolved_target(&resolved_target(
            DockResolvedDropTargetKind::LeafCenter {
                root: tabs,
                target_tabs: tabs,
            },
            Some(drop_box(DockDropBoxKind::Center)),
        ))
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
        assert!(preview.scene.layers[0].active_zones.center);
        assert!(!preview.scene.layers[0].active_zones.side);
    }

    #[test]
    fn edge_target_builds_scene_with_explicit_split_and_no_payload_tabs() {
        let tabs = DockNodeId::null();
        let preview = DockDropPreview::from_resolved_target(&resolved_target(
            DockResolvedDropTargetKind::InnerEdge {
                root: tabs,
                target_tabs: tabs,
                zone: DropZone::Left,
            },
            Some(drop_box(DockDropBoxKind::InnerEdge(DropZone::Left))),
        ))
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
        assert!(preview.scene.layers[0].active_zones.side);
        assert_eq!(
            preview
                .scene
                .active_drop_box()
                .map(|drop_box| drop_box.draw_bounds),
            Some(bounds(22.0, 32.0, 40.0, 32.0))
        );
    }

    #[test]
    fn root_edge_target_uses_outer_preview_layer() {
        let root = DockNodeId::null();
        let preview = DockDropPreview::from_resolved_target(&resolved_target(
            DockResolvedDropTargetKind::RootEdge {
                root,
                leaf_tabs: None,
                zone: DropZone::Right,
            },
            Some(drop_box(DockDropBoxKind::OuterEdge(DropZone::Right))),
        ))
        .expect("root edge target should produce preview");

        assert_eq!(preview.scene.layers[0].kind, DockPreviewLayerKind::Outer);
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
        let preview =
            DockDropPreview::from_resolution(&DockDropResolution::Rejected(DockDropRejection {
                target,
                reason: DockPolicyError::CenterMergeDisabled,
            }))
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
