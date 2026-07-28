use crate::{
    DockSpaceId, DockViewportDropRoute, DockViewportHostGeometry, DockViewportRouteProof,
    DockViewportRouteSelectionSource,
};
use open_gpui::{AnyWindowHandle, Bounds, Pixels, Point};

/// Hovered-window-local drop facts selected by a trusted hovered-window signal.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DockTrustedHoveredWindowLocalDropTarget {
    pub(super) target_space: DockSpaceId,
    pub(super) target_window: AnyWindowHandle,
    pub(super) host_position: Point<Pixels>,
    pub(super) route_proof: DockViewportRouteProof,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum DockEventReceiverLocalSceneRouteContextMode {
    HitTestedScene,
    ReceiverSceneProof,
}

pub(super) struct DockEventReceiverLocalSceneRouteContext {
    pub(super) route_proof: DockViewportRouteProof,
    pub(super) host_geometry: DockViewportHostGeometry,
    pub(super) global_screen_bounds: Option<Bounds<Pixels>>,
}

impl DockEventReceiverLocalSceneRouteContext {
    pub(super) fn host_position_from_window_position(
        &self,
        window_position: Point<Pixels>,
    ) -> Option<Point<Pixels>> {
        self.host_geometry.window_to_host(window_position)
    }

    pub(super) fn local_route(&self, host_position: Point<Pixels>) -> DockViewportDropRoute {
        DockViewportDropRoute::Local {
            host_position,
            route_proof: self.route_proof.clone(),
            source: DockViewportRouteSelectionSource::EventReceiverLocalScene,
        }
    }
}
