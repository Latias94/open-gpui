use crate::{
    DockEdgeDockSizing, DockNodeId, DockPolicyError, DockViewportDropRoute,
    DockViewportRouteSelectionSource, DockViewportTargetHit, DropZone,
    drag::DockDragPayload,
    drop_preview::{
        DockDropPreview, DockDropRoutePreview, DockDropRoutePreviewKind, DockPreviewLayerKind,
        DockPreviewPayloadTabVisualDescriptor, DockPreviewTabInsertionIndex,
        DockPreviewTabInsertionVisualDescriptor, DockPreviewVisualDecision,
        DockPreviewVisualDescriptor, DockRoutePreviewVisualDescriptor,
    },
    drop_target::{
        DockDropRejection, DockDropResolution, DockDropResolveSource, DockResolvedDropTarget,
        DockResolvedDropTargetAvailability, DockResolvedDropTargetKind,
    },
    geometry::{DockDropBox, DockDropBoxKind, DockDropGuideStyle},
    overlay_scene::{DockOverlayLayerKind, DockOverlayScene},
    viewport_test_support::{handle, space},
};
use open_gpui::{Bounds, Pixels, point, px, size};
use slotmap::Key;

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

fn drop_box(kind: DockDropBoxKind) -> DockDropBox {
    DockDropBox {
        kind,
        hit_bounds: bounds(20.0, 30.0, 44.0, 36.0),
        draw_bounds: bounds(22.0, 32.0, 40.0, 32.0),
        preview_bounds: bounds(0.0, 0.0, 120.0, 80.0),
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
fn center_preview_descriptor_reports_body_center_and_payload_tab_capability() {
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

    assert_eq!(
        preview.visual_descriptor(),
        DockPreviewVisualDescriptor {
            decision: DockPreviewVisualDecision::Allowed,
            active_layer: Some(DockPreviewLayerKind::Inner),
            active_zone: Some(DropZone::Center),
            tab_insertion: Some(DockPreviewTabInsertionVisualDescriptor {
                target_tabs: Some(tabs),
                index: DockPreviewTabInsertionIndex::Append,
                has_slot_bounds: false,
                clipping_bounds: bounds(0.0, 0.0, 320.0, 200.0),
            }),
            payload_tabs: Vec::new(),
            has_body: true,
        },
        "center hover should render a preview body and expose payload-tab capability"
    );
}

#[test]
fn center_preview_descriptor_includes_ordered_payload_tabs() {
    let tabs = DockNodeId::null();
    let mut preview = DockDropPreview::from_resolved_target(
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
    let payload = DockDragPayload::new_tabs(space("source"), tabs, "Preview / Diff".to_string())
        .with_preview_tabs(["Preview".to_string(), "Diff".to_string()]);

    preview.populate_payload_tabs(&payload);

    assert_eq!(
        preview.visual_descriptor().tab_insertion,
        Some(DockPreviewTabInsertionVisualDescriptor {
            target_tabs: Some(tabs),
            index: DockPreviewTabInsertionIndex::Append,
            has_slot_bounds: false,
            clipping_bounds: bounds(0.0, 0.0, 320.0, 200.0),
        }),
        "center tab hover should expose the target insertion slot separately from payload tabs"
    );
    assert_eq!(
        preview.visual_descriptor().payload_tabs,
        vec![
            DockPreviewPayloadTabVisualDescriptor {
                index: 0,
                title: "Preview".to_string(),
            },
            DockPreviewPayloadTabVisualDescriptor {
                index: 1,
                title: "Diff".to_string(),
            },
        ],
        "center tab hover should expose the ordered tab previews that rendering will draw"
    );
}

#[test]
fn edge_preview_descriptor_suppresses_payload_tabs_and_reports_active_zone() {
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

    let descriptor = preview.visual_descriptor();
    assert_eq!(descriptor.decision, DockPreviewVisualDecision::Allowed);
    assert_eq!(descriptor.active_layer, Some(DockPreviewLayerKind::Inner));
    assert_eq!(descriptor.active_zone, Some(DropZone::Left));
    assert_eq!(descriptor.tab_insertion, None);
    assert!(
        descriptor.payload_tabs.is_empty(),
        "edge preview must suppress payload tab previews"
    );
}

#[test]
fn rejected_preview_descriptor_suppresses_payload_tabs() {
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

    let descriptor = preview.visual_descriptor();
    assert_eq!(descriptor.decision, DockPreviewVisualDecision::Rejected);
    assert_eq!(descriptor.tab_insertion, None);
    assert!(
        descriptor.payload_tabs.is_empty(),
        "rejected target previews must suppress payload tab previews"
    );
}

#[test]
fn overlay_scene_orders_center_tab_insertion_after_guides_before_payload_tabs() {
    let tabs = DockNodeId::null();
    let mut preview = DockDropPreview::from_resolved_target(
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
    let payload = DockDragPayload::new_tabs(space("source"), tabs, "Preview / Diff".to_string())
        .with_preview_tabs(["Preview".to_string(), "Diff".to_string()]);
    preview.populate_payload_tabs(&payload);

    let overlay = DockOverlayScene::from_preview(&preview.scene);
    let kinds = overlay
        .layers
        .iter()
        .map(|layer| layer.kind)
        .collect::<Vec<_>>();

    assert_eq!(kinds[0], DockOverlayLayerKind::TargetBody);
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == DockOverlayLayerKind::GuideBox)
            .count(),
        5
    );
    assert_eq!(kinds[6], DockOverlayLayerKind::TabInsertion);
    assert_eq!(kinds[7], DockOverlayLayerKind::PayloadTab);
    assert_eq!(kinds[8], DockOverlayLayerKind::PayloadTab);
    assert_eq!(overlay.layers[6].target_node, Some(tabs));
    assert_eq!(overlay.layers[6].zone, Some(DropZone::Center));
}

#[test]
fn overlay_scene_suppresses_tab_insertion_for_edge_and_adds_rejected_state() {
    let tabs = DockNodeId::null();
    let edge_preview = DockDropPreview::from_resolved_target(
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
    let edge_overlay = DockOverlayScene::from_preview(&edge_preview.scene);

    assert!(
        edge_overlay
            .layers
            .iter()
            .all(|layer| layer.kind != DockOverlayLayerKind::TabInsertion
                && layer.kind != DockOverlayLayerKind::PayloadTab)
    );
    assert!(
        edge_overlay
            .layers
            .iter()
            .any(|layer| layer.kind == DockOverlayLayerKind::GuideBox
                && layer.active
                && layer.zone == Some(DropZone::Left))
    );

    let target = resolved_target(
        DockResolvedDropTargetKind::LeafCenter {
            root: tabs,
            target_tabs: tabs,
        },
        Some(drop_box(DockDropBoxKind::Center)),
    );
    let rejected = DockDropPreview::from_resolution(
        &DockDropResolution::Rejected(DockDropRejection {
            target,
            reason: DockPolicyError::CenterMergeDisabled,
        }),
        DockDropGuideStyle::default(),
    )
    .expect("rejected target should still produce preview");
    let rejected_overlay = DockOverlayScene::from_preview(&rejected.scene);

    assert!(
        rejected_overlay
            .layers
            .iter()
            .any(|layer| layer.kind == DockOverlayLayerKind::RejectedState)
    );
    assert!(
        rejected_overlay
            .layers
            .iter()
            .all(|layer| layer.kind != DockOverlayLayerKind::TabInsertion)
    );
}

#[test]
fn route_preview_descriptor_distinguishes_known_and_rejected_markers() {
    let known = DockDropRoutePreview::from_route(
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
    .expect("known viewport route should produce a marker");
    let rejected = DockDropRoutePreview::from_route(
        &DockViewportDropRoute::Rejected(DockPolicyError::PlatformViewportsDisabled),
        point(px(12.0), px(34.0)),
    )
    .expect("rejected route should produce a marker");

    assert_eq!(
        known.visual_descriptor(),
        DockRoutePreviewVisualDescriptor {
            kind: DockDropRoutePreviewKind::KnownViewport,
            rejected: false,
        }
    );
    assert_eq!(
        rejected.visual_descriptor(),
        DockRoutePreviewVisualDescriptor {
            kind: DockDropRoutePreviewKind::Rejected,
            rejected: true,
        }
    );
}
