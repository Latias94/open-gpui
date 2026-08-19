use super::DockViewportRuntime;
#[cfg(test)]
use crate::DockViewportDropRoute;
use crate::{
    DockController, DockViewportBackendRouteRequest, DockViewportDropRouteRequest,
    DockViewportDropRouteSnapshot, DockViewportFocusStampFallbackPermit,
    DockViewportResolvedDropRoute, DockViewportResolvedDropRouteRefresh, DockViewportRuntimeUpdate,
    DockViewportWorkspaceRouteFacts, resolved_drop_route_outcome,
};
use open_gpui::{Entity, PlatformFocusedWindow};

pub(crate) struct DockViewportPreparedDropRouteResolution {
    controller: Entity<DockController>,
    request: DockViewportDropRouteRequest,
    frame_changed: bool,
}

pub(crate) struct DockViewportSampledDropRouteResolution {
    request: DockViewportDropRouteRequest,
    backend_focus: PlatformFocusedWindow,
    workspace_facts: DockViewportWorkspaceRouteFacts,
    frame_changed: bool,
}

impl DockViewportPreparedDropRouteResolution {
    pub(crate) fn sample<C: open_gpui::AppContext>(
        self,
        cx: &mut C,
    ) -> DockViewportSampledDropRouteResolution {
        let Self {
            controller,
            request,
            frame_changed,
        } = self;
        let (request, backend_focus, workspace_facts) =
            cx.read_entity(&controller, |controller, app| {
                let request = request
                    .clone()
                    .with_resampled_platform_target_context_from_app(app);
                let workspace_facts =
                    DockViewportWorkspaceRouteFacts::capture(controller.workspace(), &request);
                (request, app.focused_window(), workspace_facts)
            });
        DockViewportSampledDropRouteResolution {
            request,
            backend_focus,
            workspace_facts,
            frame_changed,
        }
    }
}

impl DockViewportRuntime {
    pub(crate) fn prepare_payload_drop_route_resolution(
        &self,
        request: &DockViewportDropRouteRequest,
        frame_changed: bool,
    ) -> DockViewportPreparedDropRouteResolution {
        DockViewportPreparedDropRouteResolution {
            controller: self.controller.clone(),
            request: request.clone(),
            frame_changed,
        }
    }

    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_delivery_for_request<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        let sampled = self
            .prepare_payload_drop_route_resolution(request, false)
            .sample(cx);
        self.finalize_payload_drop_route_resolution(sampled)
            .outcome
            .into_resolution()
    }

    /// Resolves a rendered payload release into route and delivery facts from one snapshot.
    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_delivery<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        let sampled = self
            .prepare_payload_drop_route_resolution(request, false)
            .sample(cx);
        self.finalize_payload_drop_route_resolution(sampled)
            .outcome
            .into_resolution()
    }

    pub(crate) fn finalize_payload_drop_route_resolution(
        &mut self,
        sampled: DockViewportSampledDropRouteResolution,
    ) -> DockViewportResolvedDropRouteRefresh {
        #[cfg(test)]
        {
            self.payload_drop_route_resolution_count += 1;
        }
        let DockViewportSampledDropRouteResolution {
            request,
            backend_focus,
            workspace_facts,
            frame_changed,
        } = sampled;
        let mut update = DockViewportRuntimeUpdate::default();
        let route_request = self.resampled_backend_route_request(request, backend_focus);
        update.mark_changed(frame_changed || route_request.changed);
        let selection = DockViewportDropRouteSnapshot::resolve(
            &self.adapter,
            route_request.request,
            workspace_facts.policy(),
        )
        .into_route_selection();
        let unavailable_reason = selection.route_resolution.unavailable_reason();
        let route = selection.route_resolution.into_route();
        let reorder_hold = self
            .routed_drop_preview
            .tab_reorder_hold_for_session(selection.request.drag_session());
        let workspace_target =
            crate::resolve_workspace_target_for_route_with_facts_and_reorder_hold(
                &self.adapter,
                self.frame_coordinator.host_scenes(),
                &route,
                &selection.request,
                &workspace_facts,
                reorder_hold.as_ref(),
            );
        let resolution = DockViewportResolvedDropRoute::from_workspace_route_target(
            &selection.request,
            route,
            workspace_target,
        );
        self.status
            .record_route(&selection.request, resolution.route(), unavailable_reason);
        resolved_drop_route_outcome(resolution, update)
    }

    fn resampled_backend_route_request(
        &mut self,
        request: DockViewportDropRouteRequest,
        backend_focus: PlatformFocusedWindow,
    ) -> DockViewportBackendRouteRequest {
        let changed = self.record_confirmed_backend_focus_signal(backend_focus);
        let request = request.with_focus_stamp_fallback_permit(
            DockViewportFocusStampFallbackPermit::from_backend_focus(backend_focus),
        );
        DockViewportBackendRouteRequest {
            request: self.with_focus_stamp_fallback_context(request),
            changed,
        }
    }

    fn with_focus_stamp_fallback_context(
        &self,
        request: DockViewportDropRouteRequest,
    ) -> DockViewportDropRouteRequest {
        if !request.allows_focus_stamp_fallback()
            || request.release_origin()
                == crate::interaction::DockPayloadDropReleaseOrigin::SourceOnly
            || !matches!(
                request.target_context().trusted_hovered_signal(),
                crate::DockViewportTrustedHoveredSignal::Unavailable
            )
            || request.target_context().has_hover_fallback_window_stack()
        {
            return request;
        }
        let focused_windows = self
            .backend_focus
            .front_to_back_z_order_windows(|window_id| {
                self.adapter.window_can_route_hover_hit(window_id) == Some(true)
            });
        if focused_windows.is_empty() {
            return request;
        }
        request.with_focus_stamp_window_stack(focused_windows)
    }

    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_route_for_test(
        &self,
        request: &DockViewportDropRouteRequest,
        policy: &crate::DockPolicy,
    ) -> DockViewportDropRoute {
        self.adapter
            .resolve_payload_drop_route_resolution(request, policy)
            .into_route()
    }
}
