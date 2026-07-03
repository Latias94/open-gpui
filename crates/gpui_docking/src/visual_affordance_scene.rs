#![cfg_attr(not(test), allow(dead_code))]

use crate::{
    DockNodeId, DockSpaceId, DropZone,
    divider_hit_map::{
        DockDividerAffordanceState, DockDividerHandleHitTarget, DockDividerHitMap,
        DockDividerHitTarget,
    },
    drop_preview::{
        DockDropRoutePreview, DockPreviewDecision, DockPreviewDropBox, DockPreviewLayerKind,
        DockPreviewScene, DockPreviewTabInsertion,
    },
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
    pub(crate) drop_box: Option<DockPreviewDropBox>,
    pub(crate) tab_insertion: Option<DockPreviewTabInsertion>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockPayloadTabPreviewLayout {
    pub(crate) body_bounds: Bounds<Pixels>,
    pub(crate) insertion_bounds: Bounds<Pixels>,
    pub(crate) payload_tabs: Vec<DockPayloadTabPreviewPlacement>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockPayloadTabPreviewPlacement {
    pub(crate) payload_index: usize,
    pub(crate) bounds: Bounds<Pixels>,
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
        let mut scene = Self::empty(None);

        if preview.body.body_bounds.size.width > open_gpui::px(0.0)
            && preview.body.body_bounds.size.height > open_gpui::px(0.0)
        {
            scene.layers.push(DockVisualAffordanceLayer::new(
                DockVisualAffordanceKind::DropTargetBody,
                preview.body.body_bounds,
                preview.body.body_bounds,
                preview.body.body_bounds,
                preview
                    .payload_tabs
                    .as_ref()
                    .and_then(|tabs| tabs.target_tabs),
                None,
                DockVisualLayerScope::Local,
                DockVisualAffordanceState::from_active(preview.decision.is_allowed()),
                None,
                None,
                Some(0),
                Some("Dock target".to_string()),
            ));
        }

        let mut serial = scene.layers.len();
        for layer in &preview.layers {
            for drop_box in &layer.drop_boxes {
                let scope = match drop_box.layer {
                    DockPreviewLayerKind::Inner => DockVisualLayerScope::Inner,
                    DockPreviewLayerKind::Outer => DockVisualLayerScope::Outer,
                };
                scene.layers.push(
                    DockVisualAffordanceLayer::new(
                        DockVisualAffordanceKind::GuideBox,
                        drop_box.draw_bounds,
                        drop_box.draw_bounds,
                        drop_box.hit_bounds,
                        drop_box.debug_node,
                        Some(drop_box.zone),
                        scope,
                        DockVisualAffordanceState::from_active(drop_box.active),
                        None,
                        None,
                        Some(serial),
                        Some(format!("Dock {:?}", drop_box.zone)),
                    )
                    .with_drop_box(*drop_box),
                );
                serial += 1;
            }
        }

        if preview.active_split().is_none()
            && let Some(payload_tabs) = preview.payload_tabs.as_ref()
        {
            if let Some(insertion) = payload_tabs.insertion.clone() {
                let bounds = insertion.slot_bounds.unwrap_or(insertion.clipping_bounds);
                scene.layers.push(
                    DockVisualAffordanceLayer::new(
                        DockVisualAffordanceKind::TabInsertionSlot,
                        bounds,
                        bounds,
                        bounds,
                        insertion.target_tabs,
                        Some(DropZone::Center),
                        DockVisualLayerScope::Local,
                        DockVisualAffordanceState::from_active(preview.decision.is_allowed()),
                        None,
                        None,
                        Some(serial),
                        Some("Insert tab".to_string()),
                    )
                    .with_tab_insertion(insertion),
                );
                serial += 1;
            }

            for (index, tab) in payload_tabs.tabs.iter().enumerate() {
                let bounds = payload_tabs
                    .insertion
                    .as_ref()
                    .map(|insertion| insertion.clipping_bounds)
                    .unwrap_or(preview.body.body_bounds);
                scene.layers.push(DockVisualAffordanceLayer::new(
                    DockVisualAffordanceKind::PayloadTab,
                    bounds,
                    bounds,
                    bounds,
                    payload_tabs.target_tabs,
                    Some(DropZone::Center),
                    DockVisualLayerScope::Local,
                    DockVisualAffordanceState::from_active(preview.decision.is_allowed()),
                    Some(index),
                    Some(tab.title.clone()),
                    Some(serial),
                    Some(tab.title.clone()),
                ));
                serial += 1;
            }

            for (index, tab) in payload_tabs.tabs.iter().enumerate() {
                let bounds = payload_tabs
                    .insertion
                    .as_ref()
                    .map(|insertion| insertion.clipping_bounds)
                    .unwrap_or(preview.body.body_bounds);
                scene.layers.push(DockVisualAffordanceLayer::new(
                    DockVisualAffordanceKind::PayloadGhost,
                    bounds,
                    bounds,
                    bounds,
                    payload_tabs.target_tabs,
                    Some(DropZone::Center),
                    DockVisualLayerScope::Local,
                    DockVisualAffordanceState::from_active(preview.decision.is_allowed()),
                    Some(index),
                    Some(tab.title.clone()),
                    Some(serial),
                    Some(format!("Preview {}", tab.title)),
                ));
                serial += 1;
            }
        }

        if matches!(preview.decision, DockPreviewDecision::Rejected { .. }) {
            scene.layers.push(DockVisualAffordanceLayer::new(
                DockVisualAffordanceKind::RejectedTarget,
                preview.body.future_bounds,
                preview.body.future_bounds,
                preview.body.future_bounds,
                preview
                    .payload_tabs
                    .as_ref()
                    .and_then(|tabs| tabs.target_tabs),
                None,
                DockVisualLayerScope::Local,
                DockVisualAffordanceState::Rejected,
                None,
                None,
                Some(serial),
                Some("Drop target unavailable".to_string()),
            ));
        }

        scene
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

    pub(crate) fn apply_payload_tab_layout(&mut self, layout: &DockPayloadTabPreviewLayout) {
        for layer in &mut self.layers {
            match layer.kind {
                DockVisualAffordanceKind::DropTargetBody => {
                    layer.bounds = layout.body_bounds;
                    layer.draw_bounds = layout.body_bounds;
                    layer.hit_bounds = layout.body_bounds;
                }
                DockVisualAffordanceKind::TabInsertionSlot => {
                    layer.bounds = layout.insertion_bounds;
                    layer.draw_bounds = layout.insertion_bounds;
                    layer.hit_bounds = layout.insertion_bounds;
                    if let Some(insertion) = layer.tab_insertion.as_mut() {
                        insertion.slot_bounds = Some(layout.insertion_bounds);
                    }
                }
                DockVisualAffordanceKind::PayloadTab | DockVisualAffordanceKind::PayloadGhost => {
                    if let Some(bounds) = layer.payload_index.and_then(|index| {
                        layout
                            .payload_tabs
                            .iter()
                            .find(|placement| placement.payload_index == index)
                            .map(|placement| placement.bounds)
                    }) {
                        layer.bounds = bounds;
                        layer.draw_bounds = bounds;
                        layer.hit_bounds = bounds;
                    }
                }
                DockVisualAffordanceKind::GuideBox
                | DockVisualAffordanceKind::RouteMarker
                | DockVisualAffordanceKind::RejectedTarget
                | DockVisualAffordanceKind::DividerHandle
                | DockVisualAffordanceKind::DividerCorner
                | DockVisualAffordanceKind::FocusRing
                | DockVisualAffordanceKind::ZoomEgress => {}
            }
        }
    }

    pub(crate) fn target_body(&self) -> Option<&DockVisualAffordanceLayer> {
        self.layers
            .iter()
            .find(|layer| layer.kind == DockVisualAffordanceKind::DropTargetBody)
    }

    pub(crate) fn guide_drop_boxes(&self) -> impl Iterator<Item = DockPreviewDropBox> + '_ {
        self.layers
            .iter()
            .filter(|layer| layer.kind == DockVisualAffordanceKind::GuideBox)
            .filter_map(|layer| layer.drop_box)
    }

    pub(crate) fn tab_insertion(&self) -> Option<&DockVisualAffordanceLayer> {
        self.layers.iter().find(|layer| {
            layer.kind == DockVisualAffordanceKind::TabInsertionSlot
                && layer.state == DockVisualAffordanceState::Active
        })
    }

    pub(crate) fn payload_tabs(&self) -> impl Iterator<Item = &DockVisualAffordanceLayer> + '_ {
        self.layers.iter().filter(|layer| {
            layer.kind == DockVisualAffordanceKind::PayloadTab
                && layer.state == DockVisualAffordanceState::Active
        })
    }

    #[cfg(test)]
    pub(crate) fn payload_ghosts(&self) -> impl Iterator<Item = &DockVisualAffordanceLayer> + '_ {
        self.layers.iter().filter(|layer| {
            layer.kind == DockVisualAffordanceKind::PayloadGhost
                && layer.state == DockVisualAffordanceState::Active
        })
    }

    pub(crate) fn has_payload_tab_preview(&self) -> bool {
        self.tab_insertion().is_some() && self.payload_tabs().next().is_some()
    }

    #[cfg(test)]
    pub(crate) fn from_test_layers(layers: Vec<DockVisualAffordanceLayer>) -> Self {
        Self {
            space: None,
            frame_generation: None,
            layers,
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
            drop_box: None,
            tab_insertion: None,
        }
    }

    fn with_drop_box(mut self, drop_box: DockPreviewDropBox) -> Self {
        self.drop_box = Some(drop_box);
        self
    }

    fn with_tab_insertion(mut self, insertion: DockPreviewTabInsertion) -> Self {
        self.tab_insertion = Some(insertion);
        self
    }

    #[cfg(test)]
    pub(crate) fn test_layer(
        kind: DockVisualAffordanceKind,
        bounds: Bounds<Pixels>,
        target_node: Option<DockNodeId>,
        zone: Option<DropZone>,
        layer_scope: DockVisualLayerScope,
        state: DockVisualAffordanceState,
        payload_index: Option<usize>,
        payload_title: Option<String>,
        serial: Option<usize>,
        accessibility_label: Option<String>,
    ) -> Self {
        Self::new(
            kind,
            bounds,
            bounds,
            bounds,
            target_node,
            zone,
            layer_scope,
            state,
            payload_index,
            payload_title,
            serial,
            accessibility_label,
        )
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
