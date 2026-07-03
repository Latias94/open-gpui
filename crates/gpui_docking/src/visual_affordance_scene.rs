#![cfg_attr(not(test), allow(dead_code))]

use crate::{
    DockNodeId, DockSpaceId, DropZone,
    divider_hit_map::{
        DockDividerAffordanceState, DockDividerHandleHitTarget, DockDividerHitMap,
        DockDividerHitTarget,
    },
    drop_preview::{DockDropRoutePreview, DockPreviewScene},
    overlay_scene::{DockOverlayLayer, DockOverlayLayerKind, DockOverlayScene},
    presentation_scene::{DockPresentationFocusRegion, DockPresentationScene},
    zoom_state::DockZoomScene,
};
use open_gpui::{Bounds, Pixels, Point};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockVisualAffordanceScene {
    pub(crate) space: Option<DockSpaceId>,
    pub(crate) frame_generation: Option<u64>,
    pub(crate) layers: Vec<DockVisualAffordanceLayer>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockVisualAffordanceLayer {
    pub(crate) id: DockVisualAffordanceId,
    pub(crate) kind: DockVisualAffordanceKind,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) draw_bounds: Bounds<Pixels>,
    pub(crate) hit_bounds: Bounds<Pixels>,
    pub(crate) target_node: Option<DockNodeId>,
    pub(crate) zone: Option<DropZone>,
    pub(crate) layer_scope: DockVisualLayerScope,
    pub(crate) state: DockVisualAffordanceState,
    pub(crate) payload_index: Option<usize>,
    pub(crate) payload_title: Option<String>,
    pub(crate) motion_key: DockVisualAffordanceId,
    pub(crate) accessibility_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DockVisualAffordanceId {
    pub(crate) kind: DockVisualAffordanceKind,
    pub(crate) target_node: Option<DockNodeId>,
    pub(crate) zone: Option<DropZone>,
    pub(crate) layer_scope: DockVisualLayerScope,
    pub(crate) payload_index: Option<usize>,
    pub(crate) serial: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DockVisualAffordanceKind {
    DropTargetBody,
    GuideBox,
    TabInsertionSlot,
    PayloadTab,
    PayloadGhost,
    RouteMarker,
    RejectedTarget,
    DividerHandle,
    DividerCorner,
    FocusRing,
    ZoomEgress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DockVisualLayerScope {
    Local,
    Inner,
    Outer,
    RouteSource,
    Focus,
    Divider,
    Zoom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DockVisualAffordanceState {
    Idle,
    Passive,
    Hover,
    Active,
    Rejected,
    Disabled,
    CommittedPreview,
}

impl DockVisualAffordanceScene {
    pub(crate) fn empty(space: Option<DockSpaceId>) -> Self {
        Self {
            space,
            frame_generation: None,
            layers: Vec::new(),
        }
    }

    pub(crate) fn from_preview(preview: &DockPreviewScene) -> Self {
        Self::from_overlay_scene(&DockOverlayScene::from_preview(preview))
    }

    pub(crate) fn from_route_preview(preview: &DockDropRoutePreview) -> Self {
        Self {
            space: None,
            frame_generation: None,
            layers: vec![DockVisualAffordanceLayer::new(
                DockVisualAffordanceKind::RouteMarker,
                preview.bounds,
                preview.bounds,
                preview.bounds,
                None,
                None,
                DockVisualLayerScope::RouteSource,
                if preview.rejected {
                    DockVisualAffordanceState::Passive
                } else {
                    DockVisualAffordanceState::Active
                },
                None,
                None,
                Some(0),
                Some("Route to another viewport".to_string()),
            )],
        }
    }

    pub(crate) fn from_overlay_scene(overlay: &DockOverlayScene) -> Self {
        let layers = overlay
            .layers
            .iter()
            .enumerate()
            .map(|(serial, layer)| visual_layer_from_overlay_layer(serial, layer))
            .collect();
        Self {
            space: None,
            frame_generation: None,
            layers,
        }
    }

    pub(crate) fn from_focus_scene(scene: &DockPresentationScene) -> Self {
        let mut affordances = Self::empty(Some(scene.space.clone()));
        affordances.extend_focus_regions(&scene.focus_regions);
        affordances
    }

    pub(crate) fn from_divider_hit_map(
        scene: &DockPresentationScene,
        hit_map: &DockDividerHitMap,
        hover_position: Option<Point<Pixels>>,
        dragging: bool,
        enabled: bool,
    ) -> Self {
        let mut affordances = Self::empty(Some(scene.space.clone()));
        affordances.extend_divider_hit_map(hit_map, hover_position, dragging, enabled);
        affordances
    }

    pub(crate) fn from_zoom_scene(zoom: &DockZoomScene) -> Self {
        let mut affordances = Self::empty(Some(zoom.scene.space.clone()));
        for (serial, egress) in zoom.egress.iter().enumerate() {
            affordances.layers.push(DockVisualAffordanceLayer::new(
                DockVisualAffordanceKind::ZoomEgress,
                egress.from,
                egress.from,
                egress.from,
                Some(egress.node),
                None,
                DockVisualLayerScope::Zoom,
                if zoom.immediate {
                    DockVisualAffordanceState::CommittedPreview
                } else {
                    DockVisualAffordanceState::Active
                },
                None,
                None,
                Some(serial),
                Some(format!("Zoom egress {:?}", egress.edge)),
            ));
        }
        if let Some(focus) = zoom.focus.as_ref() {
            affordances.push_focus_region(focus);
        }
        affordances
    }

    pub(crate) fn extend_divider_hit_map(
        &mut self,
        hit_map: &DockDividerHitMap,
        hover_position: Option<Point<Pixels>>,
        dragging: bool,
        enabled: bool,
    ) {
        for (serial, target) in hit_map.targets().iter().enumerate() {
            match target {
                DockDividerHitTarget::Single(handle) => self.push_divider_handle(
                    *handle,
                    divider_handle_state(*handle, hover_position, dragging, enabled),
                    serial,
                ),
                DockDividerHitTarget::Corner(corner) => {
                    self.push_divider_handle(
                        corner.horizontal,
                        divider_handle_state(corner.horizontal, hover_position, dragging, enabled),
                        serial,
                    );
                    self.push_divider_handle(
                        corner.vertical,
                        divider_handle_state(corner.vertical, hover_position, dragging, enabled),
                        serial,
                    );
                    self.layers.push(DockVisualAffordanceLayer::new(
                        DockVisualAffordanceKind::DividerCorner,
                        corner.bounds,
                        corner.bounds,
                        corner.bounds,
                        Some(corner.horizontal.key.split),
                        None,
                        DockVisualLayerScope::Divider,
                        DockVisualAffordanceState::from_divider_state(
                            hit_map
                                .corner_affordances(hover_position, dragging, enabled)
                                .iter()
                                .find(|affordance| affordance.corner == *corner)
                                .map(|affordance| affordance.state)
                                .unwrap_or(DockDividerAffordanceState::Idle),
                        ),
                        None,
                        None,
                        Some(serial),
                        Some("Resize split corner".to_string()),
                    ));
                }
            }
        }
    }

    pub(crate) fn extend_focus_regions(&mut self, focus_regions: &[DockPresentationFocusRegion]) {
        for focus in focus_regions {
            self.push_focus_region(focus);
        }
    }

    fn push_divider_handle(
        &mut self,
        handle: DockDividerHandleHitTarget,
        state: DockVisualAffordanceState,
        serial: usize,
    ) {
        self.layers.push(DockVisualAffordanceLayer::new(
            DockVisualAffordanceKind::DividerHandle,
            handle.bounds,
            handle.bounds,
            handle.bounds,
            Some(handle.key.split),
            None,
            DockVisualLayerScope::Divider,
            state,
            None,
            None,
            Some(serial),
            Some(format!("Resize split {}", handle.key.index)),
        ));
    }

    fn push_focus_region(&mut self, focus: &DockPresentationFocusRegion) {
        self.layers.push(DockVisualAffordanceLayer::new(
            DockVisualAffordanceKind::FocusRing,
            focus.bounds,
            focus.bounds,
            focus.bounds,
            Some(focus.tabs),
            None,
            DockVisualLayerScope::Focus,
            DockVisualAffordanceState::Active,
            None,
            None,
            None,
            Some("Focused pane".to_string()),
        ));
    }
}

impl DockVisualAffordanceLayer {
    fn new(
        kind: DockVisualAffordanceKind,
        bounds: Bounds<Pixels>,
        draw_bounds: Bounds<Pixels>,
        hit_bounds: Bounds<Pixels>,
        target_node: Option<DockNodeId>,
        zone: Option<DropZone>,
        layer_scope: DockVisualLayerScope,
        state: DockVisualAffordanceState,
        payload_index: Option<usize>,
        payload_title: Option<String>,
        serial: Option<usize>,
        accessibility_label: Option<String>,
    ) -> Self {
        let id = DockVisualAffordanceId {
            kind,
            target_node,
            zone,
            layer_scope,
            payload_index,
            serial,
        };
        Self {
            id: id.clone(),
            kind,
            bounds,
            draw_bounds,
            hit_bounds,
            target_node,
            zone,
            layer_scope,
            state,
            payload_index,
            payload_title,
            motion_key: id,
            accessibility_label,
        }
    }
}

impl DockVisualAffordanceState {
    fn from_active(active: bool) -> Self {
        if active { Self::Active } else { Self::Passive }
    }

    fn from_divider_state(state: DockDividerAffordanceState) -> Self {
        match state {
            DockDividerAffordanceState::Idle => Self::Idle,
            DockDividerAffordanceState::Hover => Self::Hover,
            DockDividerAffordanceState::Active => Self::Active,
            DockDividerAffordanceState::Disabled => Self::Disabled,
        }
    }
}

fn visual_layer_from_overlay_layer(
    serial: usize,
    layer: &DockOverlayLayer,
) -> DockVisualAffordanceLayer {
    let (kind, scope, state) = match layer.kind {
        DockOverlayLayerKind::TargetBody => (
            DockVisualAffordanceKind::DropTargetBody,
            DockVisualLayerScope::Local,
            DockVisualAffordanceState::from_active(layer.active),
        ),
        DockOverlayLayerKind::GuideBox => (
            DockVisualAffordanceKind::GuideBox,
            preview_scope(layer),
            DockVisualAffordanceState::from_active(layer.active),
        ),
        DockOverlayLayerKind::TabInsertion => (
            DockVisualAffordanceKind::TabInsertionSlot,
            DockVisualLayerScope::Local,
            DockVisualAffordanceState::from_active(layer.active),
        ),
        DockOverlayLayerKind::PayloadTab => (
            DockVisualAffordanceKind::PayloadTab,
            DockVisualLayerScope::Local,
            DockVisualAffordanceState::from_active(layer.active),
        ),
        DockOverlayLayerKind::PayloadGhost => (
            DockVisualAffordanceKind::PayloadGhost,
            DockVisualLayerScope::Local,
            DockVisualAffordanceState::from_active(layer.active),
        ),
        DockOverlayLayerKind::RejectedState => (
            DockVisualAffordanceKind::RejectedTarget,
            DockVisualLayerScope::Local,
            DockVisualAffordanceState::Rejected,
        ),
    };
    let hit_bounds = layer
        .drop_box
        .as_ref()
        .map(|drop_box| drop_box.hit_bounds)
        .unwrap_or(layer.bounds);
    DockVisualAffordanceLayer::new(
        kind,
        layer.bounds,
        layer.bounds,
        hit_bounds,
        layer.target_node,
        layer.zone,
        scope,
        state,
        layer.payload_index,
        layer.payload_title.clone(),
        Some(serial),
        accessibility_label_for_overlay_layer(layer),
    )
}

fn preview_scope(layer: &DockOverlayLayer) -> DockVisualLayerScope {
    match layer.preview_layer {
        Some(crate::drop_preview::DockPreviewLayerKind::Inner) => DockVisualLayerScope::Inner,
        Some(crate::drop_preview::DockPreviewLayerKind::Outer) => DockVisualLayerScope::Outer,
        None => DockVisualLayerScope::Local,
    }
}

fn accessibility_label_for_overlay_layer(layer: &DockOverlayLayer) -> Option<String> {
    match layer.kind {
        DockOverlayLayerKind::TargetBody => Some("Dock target".to_string()),
        DockOverlayLayerKind::GuideBox => layer.zone.map(|zone| format!("Dock {zone:?}")),
        DockOverlayLayerKind::TabInsertion => Some("Insert tab".to_string()),
        DockOverlayLayerKind::PayloadTab => layer.payload_title.clone(),
        DockOverlayLayerKind::PayloadGhost => layer
            .payload_title
            .as_ref()
            .map(|title| format!("Preview {title}")),
        DockOverlayLayerKind::RejectedState => Some("Drop target unavailable".to_string()),
    }
}

fn divider_handle_state(
    handle: DockDividerHandleHitTarget,
    hover_position: Option<Point<Pixels>>,
    dragging: bool,
    enabled: bool,
) -> DockVisualAffordanceState {
    if !enabled {
        return DockVisualAffordanceState::Disabled;
    }
    if dragging {
        return DockVisualAffordanceState::Active;
    }
    if hover_position.is_some_and(|position| handle.bounds.contains(&position)) {
        return DockVisualAffordanceState::Hover;
    }
    DockVisualAffordanceState::Idle
}
