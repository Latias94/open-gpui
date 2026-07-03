use crate::{
    DockItemId, DockNodeId, DropZone,
    drop_preview::DockDropRoutePreviewKind,
    visual_affordance_scene::{
        DockVisualAffordanceId, DockVisualAffordanceLayer, DockVisualAffordanceScene,
        DockVisualAffordanceState,
    },
};
#[cfg(test)]
use std::collections::HashMap;

/// Debug-test region emitted by a dock host render pass.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum DockDebugRegion {
    /// The whole dock host.
    Host,
    /// The empty dock-space placeholder.
    EmptySpace,
    /// A split container.
    Split {
        /// Runtime split node id.
        node: DockNodeId,
    },
    /// A child wrapper inside a split container.
    SplitChild {
        /// Runtime split node id.
        split: DockNodeId,
        /// Child index within the split.
        index: usize,
    },
    /// A splitter handle between adjacent split children.
    SplitterHandle {
        /// Runtime split node id.
        split: DockNodeId,
        /// Handle index between child `index` and child `index + 1`.
        index: usize,
    },
    /// A tabs container.
    Tabs {
        /// Runtime tabs node id.
        node: DockNodeId,
    },
    /// The tab-strip chrome inside a tabs container.
    TabBar {
        /// Runtime tabs node id.
        node: DockNodeId,
    },
    /// The active drag/drop preview overlay for the host.
    DropPreview,
    /// Visible guide for one local dock drop zone during a drag.
    DropGuide {
        /// Runtime node id that owns the guide, or None for host-level empty/root guides.
        node: Option<DockNodeId>,
        /// Zone advertised by the guide.
        zone: DropZone,
    },
    /// One payload tab label rendered inside a center/tab drop preview.
    DropPayloadTabPreview {
        /// Payload tab preview index in drag payload order.
        index: usize,
    },
    /// The insertion slot rendered before payload tab previews during center/tab docking.
    DropTabInsertionPreview,
    /// The body rectangle rendered below a center/tab drop preview tab label.
    DropPreviewBody,
    /// A viewport route or tear-off preview before host-local target resolution.
    DropRoutePreview {
        /// Preview route category.
        kind: DockDropRoutePreviewKind,
    },
    /// Root visual layer rendered from a sampled transition frame.
    TransitionLayer,
    /// A sampled pane reveal/occlusion rectangle.
    TransitionPaneClip {
        /// Runtime tabs or pane node id.
        node: DockNodeId,
    },
    /// Full-size occlusion mask behind a sampled pane reveal.
    TransitionPaneOcclusion {
        /// Runtime tabs or pane node id.
        node: DockNodeId,
    },
    /// Full-size pane content mounted inside a sampled transition clip.
    TransitionPaneContent {
        /// Runtime tabs or pane node id.
        node: DockNodeId,
    },
    /// A sampled divider rectangle.
    TransitionDivider {
        /// Runtime split node id.
        split: DockNodeId,
        /// Handle index between child `index` and child `index + 1`.
        index: usize,
    },
    /// A sampled overlay rectangle.
    TransitionOverlay {
        /// Overlay sample index in the sampled transition.
        index: usize,
    },
    /// A tab label for one dock item.
    Tab {
        /// Runtime tabs node id containing the item.
        tabs: DockNodeId,
        /// Dock item id.
        item: DockItemId,
    },
    /// Close control for one tab label.
    TabClose {
        /// Runtime tabs node id containing the item.
        tabs: DockNodeId,
        /// Dock item id.
        item: DockItemId,
    },
    /// The selected panel body for one dock item.
    Panel {
        /// Dock item id.
        item: DockItemId,
    },
    /// The missing-panel placeholder for one dock item.
    MissingPanel {
        /// Dock item id.
        item: DockItemId,
    },
    /// An in-window floating container frame.
    Floating {
        /// Runtime floating node id.
        node: DockNodeId,
    },
    /// The drag handle for an in-window floating container.
    FloatingHandle {
        /// Runtime floating node id.
        node: DockNodeId,
    },
    /// A placeholder for a graph node that cannot be found.
    MissingNode {
        /// Runtime node id referenced by the graph.
        node: DockNodeId,
    },
}

/// Compact debug view of the current docking visual affordance scene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockVisualAffordanceDebugSummary {
    /// Dock space that produced the scene, if known.
    pub space: Option<String>,
    /// Render-frame generation attached to the scene, if known.
    pub frame_generation: Option<u64>,
    /// Total number of visual affordance layers in the scene.
    pub layer_count: usize,
    /// Number of non-idle and non-passive layers.
    pub active_count: usize,
    /// First active layer, useful for compact inspectors.
    pub active: Option<DockVisualAffordanceDebugLayer>,
    /// Current overlay motion executor state, if an overlay transition is active.
    pub motion_state: Option<String>,
    /// Stable signature used to spot retarget churn without logging every frame.
    pub churn_signature: String,
}

/// Compact debug view of one visual affordance layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockVisualAffordanceDebugLayer {
    /// Stable affordance identifier rendered as a public debug string.
    pub id: String,
    /// Affordance kind rendered as a public debug string.
    pub kind: String,
    /// Layer scope rendered as a public debug string.
    pub scope: String,
    /// Layer state rendered as a public debug string.
    pub state: String,
    /// Target dock node id, if the layer is tied to a node.
    pub target_node: Option<u64>,
    /// Drop zone advertised by the layer.
    pub zone: Option<DropZone>,
    /// Drag payload index, if the layer represents payload feedback.
    pub payload_index: Option<usize>,
    /// Human-readable label associated with the layer.
    pub label: Option<String>,
}

impl DockVisualAffordanceDebugSummary {
    pub(crate) fn from_scene(
        scene: Option<&DockVisualAffordanceScene>,
        motion_state: Option<String>,
    ) -> Self {
        let Some(scene) = scene else {
            return Self {
                space: None,
                frame_generation: None,
                layer_count: 0,
                active_count: 0,
                active: None,
                motion_state,
                churn_signature: "empty".to_string(),
            };
        };
        let active_layers = scene
            .layers
            .iter()
            .filter(|layer| affordance_layer_is_active(layer.state))
            .collect::<Vec<_>>();
        let active = active_layers
            .first()
            .map(|layer| DockVisualAffordanceDebugLayer::from_layer(layer));
        let churn_signature = scene
            .layers
            .iter()
            .map(affordance_churn_signature)
            .collect::<Vec<_>>()
            .join("|");

        Self {
            space: scene.space.as_ref().map(|space| space.as_str().to_string()),
            frame_generation: scene.frame_generation,
            layer_count: scene.layers.len(),
            active_count: active_layers.len(),
            active,
            motion_state,
            churn_signature,
        }
    }
}

impl DockVisualAffordanceDebugLayer {
    fn from_layer(layer: &DockVisualAffordanceLayer) -> Self {
        Self {
            id: affordance_id_debug_string(&layer.id),
            kind: format!("{:?}", layer.kind),
            scope: format!("{:?}", layer.layer_scope),
            state: format!("{:?}", layer.state),
            target_node: layer.target_node.map(|node| node.as_u64()),
            zone: layer.zone,
            payload_index: layer.payload_index,
            label: layer.accessibility_label.clone(),
        }
    }
}

fn affordance_layer_is_active(state: DockVisualAffordanceState) -> bool {
    !matches!(
        state,
        DockVisualAffordanceState::Idle | DockVisualAffordanceState::Passive
    )
}

fn affordance_churn_signature(layer: &DockVisualAffordanceLayer) -> String {
    format!(
        "{}:{:?}:{:?}",
        affordance_id_debug_string(&layer.id),
        layer.layer_scope,
        layer.state
    )
}

fn affordance_id_debug_string(id: &DockVisualAffordanceId) -> String {
    format!(
        "{:?}:node-{}:zone-{}:scope-{:?}:payload-{}:serial-{}",
        id.kind,
        id.target_node
            .map(|node| node.as_u64().to_string())
            .unwrap_or_else(|| "none".to_string()),
        id.zone
            .map(|zone| format!("{zone:?}"))
            .unwrap_or_else(|| "none".to_string()),
        id.layer_scope,
        id.payload_index
            .map(|index| index.to_string())
            .unwrap_or_else(|| "none".to_string()),
        id.serial
            .map(|serial| serial.to_string())
            .unwrap_or_else(|| "none".to_string()),
    )
}

/// Selector instrumentation used by crate-local visual tests.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct DockDebugInstrumentation {
    selectors: HashMap<DockDebugRegion, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        overlay_scene::{DockOverlayLayer, DockOverlayLayerKind, DockOverlayScene},
        visual_affordance_scene::DockVisualAffordanceScene,
    };
    use open_gpui::{Bounds, point, px, size};
    use slotmap::Key;

    #[test]
    fn host_debug_affordance_summary_changes_on_retarget_but_not_steady_hover() {
        let tabs = DockNodeId::null();
        let scene_for_zone = |zone| {
            DockVisualAffordanceScene::from_overlay_scene(&DockOverlayScene {
                layers: vec![DockOverlayLayer {
                    kind: DockOverlayLayerKind::GuideBox,
                    bounds: Bounds::new(point(px(0.0), px(0.0)), size(px(80.0), px(40.0))),
                    target_node: Some(tabs),
                    zone: Some(zone),
                    preview_layer: None,
                    active: true,
                    payload_index: None,
                    payload_title: None,
                    drop_box: None,
                    tab_insertion: None,
                }],
            })
        };
        let left = scene_for_zone(DropZone::Left);
        let left_again = scene_for_zone(DropZone::Left);
        let right = scene_for_zone(DropZone::Right);

        let left_summary =
            DockVisualAffordanceDebugSummary::from_scene(Some(&left), Some("Scheduled".into()));
        let steady_summary = DockVisualAffordanceDebugSummary::from_scene(
            Some(&left_again),
            Some("Scheduled".into()),
        );
        let right_summary =
            DockVisualAffordanceDebugSummary::from_scene(Some(&right), Some("Scheduled".into()));

        assert_eq!(left_summary.churn_signature, steady_summary.churn_signature);
        assert_ne!(left_summary.churn_signature, right_summary.churn_signature);
        assert_eq!(left_summary.layer_count, 1);
        assert_eq!(left_summary.active_count, 1);
        assert_eq!(
            left_summary.active.as_ref().and_then(|layer| layer.zone),
            Some(DropZone::Left)
        );
    }
}

#[cfg(test)]
impl DockDebugInstrumentation {
    /// Clears selectors from the previous render pass.
    pub(crate) fn clear(&mut self) {
        self.selectors.clear();
    }

    /// Records a selector for a region and returns the selector for element wiring.
    pub(crate) fn record(&mut self, region: DockDebugRegion, selector: String) -> String {
        self.selectors.insert(region, selector.clone());
        selector
    }

    /// Returns the selector emitted for a region in the most recent render pass.
    #[cfg(test)]
    pub(crate) fn selector(&self, region: &DockDebugRegion) -> Option<&str> {
        self.selectors.get(region).map(String::as_str)
    }
}
