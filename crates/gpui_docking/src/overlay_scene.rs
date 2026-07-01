use crate::{
    DockNodeId, DropZone,
    drop_preview::{
        DockDropRoutePreview, DockPreviewDecision, DockPreviewDropBox, DockPreviewLayerKind,
        DockPreviewScene, DockPreviewTabInsertion,
    },
};
use open_gpui::{Bounds, Pixels};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockOverlayScene {
    pub(crate) layers: Vec<DockOverlayLayer>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockOverlayLayer {
    pub(crate) kind: DockOverlayLayerKind,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) target_node: Option<DockNodeId>,
    pub(crate) zone: Option<DropZone>,
    pub(crate) preview_layer: Option<DockPreviewLayerKind>,
    pub(crate) active: bool,
    pub(crate) payload_index: Option<usize>,
    pub(crate) payload_title: Option<String>,
    pub(crate) drop_box: Option<DockPreviewDropBox>,
    pub(crate) tab_insertion: Option<DockPreviewTabInsertion>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockOverlayPayloadTabLayout {
    pub(crate) body_bounds: Bounds<Pixels>,
    pub(crate) insertion_bounds: Bounds<Pixels>,
    pub(crate) payload_tabs: Vec<DockOverlayPayloadTabPlacement>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DockOverlayPayloadTabPlacement {
    pub(crate) payload_index: usize,
    pub(crate) bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum DockOverlayLayerKind {
    RouteMarker,
    TargetBody,
    GuideBox,
    TabInsertion,
    PayloadTab,
    PayloadGhost,
    #[allow(dead_code)]
    FocusRing,
    RejectedState,
}

impl DockOverlayScene {
    pub(crate) fn from_preview(preview: &DockPreviewScene) -> Self {
        let mut scene = Self { layers: Vec::new() };

        if preview.body.body_bounds.size.width > open_gpui::px(0.0)
            && preview.body.body_bounds.size.height > open_gpui::px(0.0)
        {
            scene.layers.push(DockOverlayLayer {
                kind: DockOverlayLayerKind::TargetBody,
                bounds: preview.body.body_bounds,
                target_node: preview
                    .payload_tabs
                    .as_ref()
                    .and_then(|tabs| tabs.target_tabs),
                zone: None,
                preview_layer: None,
                active: preview.decision.is_allowed(),
                payload_index: None,
                payload_title: None,
                drop_box: None,
                tab_insertion: None,
            });
        }

        for layer in &preview.layers {
            for drop_box in &layer.drop_boxes {
                scene.layers.push(DockOverlayLayer {
                    kind: DockOverlayLayerKind::GuideBox,
                    bounds: drop_box.draw_bounds,
                    target_node: drop_box.debug_node,
                    zone: Some(drop_box.zone),
                    preview_layer: Some(drop_box.layer),
                    active: drop_box.active,
                    payload_index: None,
                    payload_title: None,
                    drop_box: Some(*drop_box),
                    tab_insertion: None,
                });
            }
        }

        if preview.active_split().is_none()
            && let Some(payload_tabs) = preview.payload_tabs.as_ref()
        {
            if let Some(insertion) = payload_tabs.insertion.clone() {
                scene.layers.push(DockOverlayLayer {
                    kind: DockOverlayLayerKind::TabInsertion,
                    bounds: insertion.slot_bounds.unwrap_or(insertion.clipping_bounds),
                    target_node: insertion.target_tabs,
                    zone: Some(DropZone::Center),
                    preview_layer: None,
                    active: preview.decision.is_allowed(),
                    payload_index: None,
                    payload_title: None,
                    drop_box: None,
                    tab_insertion: Some(insertion),
                });
            }

            for (index, tab) in payload_tabs.tabs.iter().enumerate() {
                let bounds = payload_tabs
                    .insertion
                    .as_ref()
                    .map(|insertion| insertion.clipping_bounds)
                    .unwrap_or(preview.body.body_bounds);
                scene.layers.push(DockOverlayLayer {
                    kind: DockOverlayLayerKind::PayloadTab,
                    bounds,
                    target_node: payload_tabs.target_tabs,
                    zone: Some(DropZone::Center),
                    preview_layer: None,
                    active: preview.decision.is_allowed(),
                    payload_index: Some(index),
                    payload_title: Some(tab.title.clone()),
                    drop_box: None,
                    tab_insertion: None,
                });
            }

            for (index, tab) in payload_tabs.tabs.iter().enumerate() {
                let bounds = payload_tabs
                    .insertion
                    .as_ref()
                    .map(|insertion| insertion.clipping_bounds)
                    .unwrap_or(preview.body.body_bounds);
                scene.layers.push(DockOverlayLayer {
                    kind: DockOverlayLayerKind::PayloadGhost,
                    bounds,
                    target_node: payload_tabs.target_tabs,
                    zone: Some(DropZone::Center),
                    preview_layer: None,
                    active: preview.decision.is_allowed(),
                    payload_index: Some(index),
                    payload_title: Some(tab.title.clone()),
                    drop_box: None,
                    tab_insertion: None,
                });
            }
        }

        if matches!(preview.decision, DockPreviewDecision::Rejected { .. }) {
            scene.layers.push(DockOverlayLayer {
                kind: DockOverlayLayerKind::RejectedState,
                bounds: preview.body.future_bounds,
                target_node: preview
                    .payload_tabs
                    .as_ref()
                    .and_then(|tabs| tabs.target_tabs),
                zone: None,
                preview_layer: None,
                active: true,
                payload_index: None,
                payload_title: None,
                drop_box: None,
                tab_insertion: None,
            });
        }

        scene
    }

    pub(crate) fn from_route_preview(preview: &DockDropRoutePreview) -> Self {
        Self {
            layers: vec![DockOverlayLayer {
                kind: DockOverlayLayerKind::RouteMarker,
                bounds: preview.bounds,
                target_node: None,
                zone: None,
                preview_layer: None,
                active: !preview.rejected,
                payload_index: None,
                payload_title: None,
                drop_box: None,
                tab_insertion: None,
            }],
        }
    }

    pub(crate) fn apply_payload_tab_layout(&mut self, layout: &DockOverlayPayloadTabLayout) {
        for layer in &mut self.layers {
            match layer.kind {
                DockOverlayLayerKind::TargetBody => {
                    layer.bounds = layout.body_bounds;
                }
                DockOverlayLayerKind::TabInsertion => {
                    layer.bounds = layout.insertion_bounds;
                    if let Some(insertion) = layer.tab_insertion.as_mut() {
                        insertion.slot_bounds = Some(layout.insertion_bounds);
                    }
                }
                DockOverlayLayerKind::PayloadTab | DockOverlayLayerKind::PayloadGhost => {
                    if let Some(bounds) = layer.payload_index.and_then(|index| {
                        layout
                            .payload_tabs
                            .iter()
                            .find(|placement| placement.payload_index == index)
                            .map(|placement| placement.bounds)
                    }) {
                        layer.bounds = bounds;
                    }
                }
                DockOverlayLayerKind::RouteMarker
                | DockOverlayLayerKind::GuideBox
                | DockOverlayLayerKind::FocusRing
                | DockOverlayLayerKind::RejectedState => {}
            }
        }
    }

    pub(crate) fn guide_drop_boxes(&self) -> impl Iterator<Item = DockPreviewDropBox> + '_ {
        self.layers
            .iter()
            .filter(|layer| layer.kind == DockOverlayLayerKind::GuideBox)
            .filter_map(|layer| layer.drop_box)
    }

    pub(crate) fn tab_insertion(&self) -> Option<&DockOverlayLayer> {
        self.layers
            .iter()
            .find(|layer| layer.kind == DockOverlayLayerKind::TabInsertion && layer.active)
    }

    pub(crate) fn payload_tabs(&self) -> impl Iterator<Item = &DockOverlayLayer> + '_ {
        self.layers
            .iter()
            .filter(|layer| layer.kind == DockOverlayLayerKind::PayloadTab && layer.active)
    }

    pub(crate) fn payload_ghosts(&self) -> impl Iterator<Item = &DockOverlayLayer> + '_ {
        self.layers
            .iter()
            .filter(|layer| layer.kind == DockOverlayLayerKind::PayloadGhost && layer.active)
    }

    pub(crate) fn has_payload_tab_preview(&self) -> bool {
        self.tab_insertion().is_some() && self.payload_tabs().next().is_some()
    }
}
