use crate::{
    DockPolicy, DockViewportAdapter, DockViewportDropRouteRequest, DockViewportDropRouteResolution,
    DockViewportResolvedDropRoute, DockViewportRuntimeUpdate, DockViewportWindowEffects,
};

pub(crate) struct DockViewportBackendRouteRequest {
    pub(crate) request: DockViewportDropRouteRequest,
    pub(crate) changed: bool,
}

pub(crate) struct DockViewportDropRouteSnapshotRefresh {
    pub(crate) snapshot: DockViewportDropRouteSnapshot,
    pub(crate) changed: bool,
    pub(crate) window_effects: DockViewportWindowEffects,
}

#[derive(Debug, Clone)]
pub(crate) struct DockViewportResolvedDropRouteOutcome {
    resolution: DockViewportResolvedDropRoute,
    changed: bool,
}

pub(crate) struct DockViewportResolvedDropRouteRefresh {
    pub(crate) outcome: DockViewportResolvedDropRouteOutcome,
    pub(crate) window_effects: DockViewportWindowEffects,
}

impl DockViewportResolvedDropRouteRefresh {
    pub(crate) fn window_effects(&self) -> DockViewportWindowEffects {
        self.window_effects.clone()
    }
}

impl DockViewportResolvedDropRouteOutcome {
    pub(crate) fn new(resolution: DockViewportResolvedDropRoute, changed: bool) -> Self {
        Self {
            resolution,
            changed,
        }
    }

    pub(crate) fn changed(&self) -> bool {
        self.changed
    }

    pub(crate) fn resolution(&self) -> &DockViewportResolvedDropRoute {
        &self.resolution
    }

    pub(crate) fn into_resolution(self) -> DockViewportResolvedDropRoute {
        self.resolution
    }
}

#[derive(Debug)]
pub(crate) struct DockViewportDropRouteSnapshot {
    request: DockViewportDropRouteRequest,
    route_resolution: DockViewportDropRouteResolution,
}

pub(crate) struct DockViewportDropRouteSnapshotSelection {
    pub(crate) request: DockViewportDropRouteRequest,
    pub(crate) route_resolution: DockViewportDropRouteResolution,
}

impl DockViewportDropRouteSnapshot {
    pub(crate) fn resolve(
        adapter: &DockViewportAdapter,
        request: DockViewportDropRouteRequest,
        policy: &DockPolicy,
    ) -> Self {
        let route_resolution = adapter.resolve_payload_drop_route_resolution(&request, policy);
        Self {
            request,
            route_resolution,
        }
    }

    pub(crate) fn into_route_selection(self) -> DockViewportDropRouteSnapshotSelection {
        DockViewportDropRouteSnapshotSelection {
            request: self.request,
            route_resolution: self.route_resolution,
        }
    }
}

pub(crate) fn resolved_drop_route_outcome(
    resolution: DockViewportResolvedDropRoute,
    update: DockViewportRuntimeUpdate,
) -> DockViewportResolvedDropRouteRefresh {
    let changed = update.changed();
    let window_effects = DockViewportWindowEffects::refresh_only(update.into_windows());
    DockViewportResolvedDropRouteRefresh {
        outcome: DockViewportResolvedDropRouteOutcome::new(resolution, changed),
        window_effects,
    }
}
