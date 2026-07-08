use super::DockViewportRuntime;
use crate::{
    DockViewportBackendRouteRequest, DockViewportDropRoute, DockViewportDropRouteRequest,
    DockViewportDropRouteSnapshot, DockViewportDropRouteSnapshotRefresh,
    DockViewportFocusStampFallbackPermit, DockViewportResolvedDropRoute,
    DockViewportResolvedDropRouteOutcome, DockViewportResolvedDropRouteRefresh,
    DockViewportRuntimeUpdate, DockViewportWindowEffects, resolved_drop_route_outcome,
};
#[cfg(test)]
use open_gpui::{App, AppContext as _};

impl DockViewportRuntime {
    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_delivery_for_request<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        self.resolve_payload_drop_delivery_for_request_with_outcome(request, cx)
            .outcome
            .into_resolution()
    }

    pub(crate) fn resolve_payload_drop_delivery_for_request_with_outcome<
        C: open_gpui::AppContext,
    >(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRouteRefresh {
        let refresh = self.resolve_payload_drop_delivery_with_outcome(request, cx);
        let DockViewportResolvedDropRouteRefresh {
            outcome,
            window_effects,
        } = refresh;
        let changed = outcome.changed();
        let resolution = outcome.into_resolution();
        let outcome = DockViewportResolvedDropRouteOutcome::new(resolution, changed);
        DockViewportResolvedDropRouteRefresh {
            outcome,
            window_effects,
        }
    }

    /// Resolves a rendered payload release into route and delivery facts from one snapshot.
    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_delivery<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        self.resolve_payload_drop_delivery_with_outcome(request, cx)
            .outcome
            .into_resolution()
    }

    pub(crate) fn resolve_payload_drop_delivery_with_outcome<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportResolvedDropRouteRefresh {
        let mut update = DockViewportRuntimeUpdate::default();
        let policy = cx.read_entity(&self.controller, |controller, _| {
            controller.workspace().policy().to_owned()
        });
        let initial_route_request =
            self.backend_route_request_without_target_context_resample(request, cx);
        update.mark_changed(initial_route_request.changed);

        let DockViewportDropRouteSnapshotRefresh {
            snapshot: resampled_snapshot,
            changed: resampled_changed,
            window_effects: resampled_effects,
        } = self.resampled_drop_route_snapshot(request, &policy, cx);
        update.mark_changed(resampled_changed);
        update.extend_windows(resampled_effects.refresh().iter().cloned());
        let selection = resampled_snapshot.into_route_selection();
        let unavailable_reason = selection.route_resolution.unavailable_reason();
        let route = selection.route_resolution.into_route();
        let resolution =
            self.resolve_payload_drop_delivery_resolution(&selection.request, route, cx);
        self.status
            .record_route(&selection.request, resolution.route(), unavailable_reason);
        resolved_drop_route_outcome(resolution, update)
    }

    fn resampled_drop_route_snapshot<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        policy: &crate::DockPolicy,
        cx: &mut C,
    ) -> DockViewportDropRouteSnapshotRefresh {
        let frame_update =
            self.reconcile_viewport_frame_except_window(request.event_receiver_window(), cx);
        let route_request = self.resampled_backend_route_request(request, cx);
        let frame_changed = frame_update.changed();
        DockViewportDropRouteSnapshotRefresh {
            snapshot: DockViewportDropRouteSnapshot::resolve(
                &self.adapter,
                route_request.request,
                policy,
            ),
            changed: frame_changed || route_request.changed,
            window_effects: DockViewportWindowEffects::refresh_only(frame_update.into_windows()),
        }
    }

    fn backend_route_request_without_target_context_resample<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportBackendRouteRequest {
        let backend_focus = cx.read_entity(&self.controller, |_, app| app.focused_window());
        let changed = self.record_confirmed_backend_focus_signal(backend_focus);
        let request = request.clone().with_focus_stamp_fallback_permit(
            DockViewportFocusStampFallbackPermit::from_backend_focus(backend_focus),
        );
        DockViewportBackendRouteRequest {
            request: self.with_runtime_fallback_route_context(request),
            changed,
        }
    }

    fn resampled_backend_route_request<C: open_gpui::AppContext>(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut C,
    ) -> DockViewportBackendRouteRequest {
        let (request, backend_focus) = cx.read_entity(&self.controller, |_, app| {
            (
                request
                    .clone()
                    .with_resampled_platform_target_context_from_app(app),
                app.focused_window(),
            )
        });
        let changed = self.record_confirmed_backend_focus_signal(backend_focus);
        let request = request.with_focus_stamp_fallback_permit(
            DockViewportFocusStampFallbackPermit::from_backend_focus(backend_focus),
        );
        DockViewportBackendRouteRequest {
            request: self.with_runtime_fallback_route_context(request),
            changed,
        }
    }

    fn with_runtime_fallback_route_context(
        &self,
        request: DockViewportDropRouteRequest,
    ) -> DockViewportDropRouteRequest {
        let request = self.with_drag_last_hovered_viewport_context(request);
        self.with_focus_stamp_fallback_context(request)
    }

    fn with_drag_last_hovered_viewport_context(
        &self,
        request: DockViewportDropRouteRequest,
    ) -> DockViewportDropRouteRequest {
        if request.release_origin() != crate::interaction::DockPayloadDropReleaseOrigin::HoveredHost
        {
            return request;
        }
        let Some(drag_session) = request.drag_session() else {
            return request;
        };
        if !self.payload_drag.matches_session(Some(drag_session)) {
            return request;
        }
        let Some(identity) = self
            .payload_drag
            .last_hovered_viewport_identity(Some(drag_session))
        else {
            return request;
        };
        let Some(window) = self.adapter.window_for_space(identity.space()) else {
            return request;
        };
        if window.window_id() != identity.window_id() {
            return request;
        }
        if self.adapter.window_route_ready(identity.window_id()) != Some(true) {
            return request;
        }
        request.with_drag_last_hovered_viewport_window(identity.window_id())
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

    fn resolve_payload_drop_delivery_resolution<C: open_gpui::AppContext>(
        &self,
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
        cx: &mut C,
    ) -> DockViewportResolvedDropRoute {
        cx.read_entity(&self.controller, |controller, _| {
            let workspace = controller.workspace();
            let payload_classes = workspace.payload_dock_classes_for_viewport_payload(
                request.payload(),
                request.source_node(),
            );
            self.resolve_payload_drop_delivery_resolution_with_workspace(
                request,
                route,
                workspace,
                &payload_classes,
            )
        })
    }

    fn resolve_payload_drop_delivery_resolution_with_workspace(
        &self,
        request: &DockViewportDropRouteRequest,
        route: DockViewportDropRoute,
        workspace: &crate::DockWorkspace,
        payload_classes: &crate::workspace_move_validation::DockPayloadDockClasses,
    ) -> DockViewportResolvedDropRoute {
        let workspace_target = crate::resolve_workspace_target_for_route(
            &self.adapter,
            self.frame_coordinator.host_scenes(),
            &route,
            request,
            workspace,
            payload_classes,
        );
        DockViewportResolvedDropRoute::from_workspace_route_target(request, route, workspace_target)
    }

    #[cfg(test)]
    pub(crate) fn resolve_payload_drop_route_for_test(
        &mut self,
        request: &DockViewportDropRouteRequest,
        cx: &mut App,
    ) -> DockViewportDropRoute {
        self.reconcile_viewport_frame(cx);
        let policy = cx.read_entity(&self.controller, |controller, _| {
            controller.workspace().policy().to_owned()
        });
        self.adapter
            .resolve_payload_drop_route_resolution(request, &policy)
            .into_route()
    }
}
