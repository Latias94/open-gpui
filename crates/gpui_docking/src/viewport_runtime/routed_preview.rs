use super::DockViewportRuntime;
use crate::{
    DockCapturedNativeDropRoute, DockSpaceId, DockViewportDropRoute, DockViewportResolvedDropRoute,
    DockViewportRoutePreview, DockViewportRouteProof, DockViewportRoutedDropPreview,
    DockViewportRoutedDropPreviewReplacement, DockViewportRoutedPreviewOwner,
    DockViewportRuntimeUpdate, drag::DockDragPayload, interaction::DockRuntimeDragSession,
};
use open_gpui::{AnyWindowHandle, Pixels, Point, WindowId};

impl DockViewportRuntime {
    #[cfg(test)]
    pub(crate) fn routed_drop_target_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<crate::drop_target::DockResolvedDropTarget> {
        let registration = self
            .adapter
            .registration_key(space)
            .filter(|registration| registration.window_id() == window_id)?;
        self.routed_drop_preview
            .resolved_target_for_registration(&registration)
    }

    pub(crate) fn routed_drop_preview_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockViewportRoutedDropPreview> {
        let registration = self
            .adapter
            .registration_key(space)
            .filter(|registration| registration.window_id() == window_id)?;
        self.routed_drop_preview
            .preview_for_registration(&registration)
    }

    pub(crate) fn routed_drop_route_preview_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<crate::drop_preview::DockDropRoutePreview> {
        let registration = self
            .adapter
            .registration_key(space)
            .filter(|registration| registration.window_id() == window_id)?;
        self.routed_drop_preview
            .route_preview_for_registration(&registration)
    }

    #[cfg(test)]
    pub(crate) fn has_routed_drop_preview_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> bool {
        self.routed_drop_preview
            .has_preview_for_drag_session(session)
    }

    pub(crate) fn update_routed_drop_preview(
        &mut self,
        resolution: &DockViewportResolvedDropRoute,
        payload: &DockDragPayload,
    ) -> DockViewportRuntimeUpdate {
        self.update_routed_drop_preview_inner(resolution, payload, None, None, None)
    }

    pub(crate) fn update_host_routed_drop_preview(
        &mut self,
        resolution: &DockViewportResolvedDropRoute,
        payload: &DockDragPayload,
        host_space: DockSpaceId,
        host_window_id: WindowId,
        host_position: Point<Pixels>,
    ) -> DockViewportRuntimeUpdate {
        self.update_routed_drop_preview_inner(
            resolution,
            payload,
            Some(host_space),
            Some(host_window_id),
            Some(host_position),
        )
    }

    fn update_routed_drop_preview_inner(
        &mut self,
        resolution: &DockViewportResolvedDropRoute,
        payload: &DockDragPayload,
        host_space: Option<DockSpaceId>,
        host_window_id: Option<WindowId>,
        host_position: Option<Point<Pixels>>,
    ) -> DockViewportRuntimeUpdate {
        if !self.resolution_registration_is_current(resolution) {
            return DockViewportRuntimeUpdate::default();
        }
        if let Some(session) = resolution.drag_session() {
            if !self.payload_drag.matches_session(Some(session))
                || !session.accepts_payload(payload)
            {
                return DockViewportRuntimeUpdate::default();
            }
        } else if resolution.routed_preview_target_snapshot().is_some() {
            return DockViewportRuntimeUpdate::default();
        }
        let owner = resolution
            .drag_session()
            .cloned()
            .map(DockViewportRoutedPreviewOwner::Local);
        if let Some(active_drag_session) = self.payload_drag.active_session()
            && let Some(identity) = crate::last_routed_viewport_identity_from_resolution(
                resolution,
                Some(active_drag_session),
            )
        {
            self.payload_drag
                .record_last_routed_viewport_identity(Some(identity));
        }
        let next = match resolution.route() {
            DockViewportDropRoute::Local { .. } | DockViewportDropRoute::KnownViewport { .. } => {
                resolution
                    .routed_preview_target_snapshot()
                    .and_then(|target| {
                        crate::routed_drop_preview_from_target(target, owner.clone(), payload)
                    })
            }
            DockViewportDropRoute::Rejected(_) => resolution
                .routed_preview_target_snapshot()
                .and_then(|target| {
                    crate::routed_rejected_drop_preview_from_target(target, owner.clone(), payload)
                }),
            DockViewportDropRoute::TearOff => None,
            DockViewportDropRoute::Unavailable => None,
        };
        let next_route_preview = match (host_space, host_window_id, host_position) {
            (Some(space), Some(window_id), Some(position)) => self
                .current_route_proof(&space, window_id)
                .and_then(|route_proof| {
                    crate::routed_drop_route_preview_for_host(
                        resolution,
                        route_proof,
                        position,
                        owner.clone(),
                    )
                }),
            _ => None,
        };
        let next_resolution = match resolution.route() {
            DockViewportDropRoute::Unavailable => None,
            _ => Some(resolution.clone()),
        };
        self.replace_routed_drop_preview(next, next_route_preview, next_resolution)
    }

    pub(crate) fn update_captured_native_foreign_surface_preview(
        &mut self,
        request: &crate::DockViewportDropRouteRequest,
        owner: &DockViewportRoutedPreviewOwner,
    ) -> (bool, DockViewportRuntimeUpdate) {
        let Some((_, generation, sequence, session)) = owner.captured_native_parts() else {
            return (false, DockViewportRuntimeUpdate::default());
        };
        let valid_owner = owner.is_current()
            && request.captured_native_generation() == Some(generation)
            && request.captured_native_sequence() == Some(sequence)
            && request.drag_session() == Some(session);
        let Some(DockCapturedNativeDropRoute::ForbiddenTarget(target)) =
            request.captured_native_route()
        else {
            let update = self.clear_routed_drop_preview_for_owner(owner);
            return (false, update);
        };
        let planned_foreign_rejection = valid_owner
            && matches!(
                self.adapter
                    .resolve_payload_drop_route_resolution(request, &crate::DockPolicy::default())
                    .into_route(),
                DockViewportDropRoute::Rejected(
                    crate::DockViewportDropRouteRejectionReason::ForeignSurface
                )
            );
        let route_proof = planned_foreign_rejection
            .then(|| {
                self.adapter
                    .resolve_captured_native_forbidden_route_proof(request)
            })
            .flatten();
        let current_route_proof = route_proof.as_ref().and_then(|route_proof| {
            self.current_route_proof(target.target_space(), target.target_window().window_id())
                .filter(|current| current == route_proof)
        });
        let current_scene = self
            .frame_coordinator
            .host_scenes()
            .is_current_frame(target.scene_frame());
        let Some(route_proof) = current_route_proof.filter(|_| current_scene && owner.is_current())
        else {
            let update = self.clear_routed_drop_preview_for_owner(owner);
            return (false, update);
        };

        let resolution = DockViewportResolvedDropRoute::foreign_surface_rejection(request);
        let route_preview = crate::routed_drop_route_preview_for_host(
            &resolution,
            route_proof,
            target.host_position(),
            Some(owner.clone()),
        );
        if !owner.is_current() {
            let update = self.clear_routed_drop_preview_for_owner(owner);
            return (false, update);
        }
        let update = self.replace_routed_drop_preview(None, route_preview, Some(resolution));
        (true, update)
    }

    fn captured_native_source_is_current(
        &self,
        request: &crate::DockViewportDropRouteRequest,
        owner: &DockViewportRoutedPreviewOwner,
        payload: &DockDragPayload,
    ) -> bool {
        let Some((_, generation, sequence, session)) = owner.captured_native_parts() else {
            return false;
        };
        owner.is_current()
            && request.captured_native_generation() == Some(generation)
            && request.captured_native_sequence() == Some(sequence)
            && request.drag_session() == Some(session)
            && self.payload_drag.matches_session(Some(session))
            && session.accepts_payload(payload)
            && request.source_space() == &payload.source_space
    }

    fn captured_native_source_foreign_surface_is_current(
        &self,
        request: &crate::DockViewportDropRouteRequest,
        owner: &DockViewportRoutedPreviewOwner,
        payload: &DockDragPayload,
    ) -> bool {
        self.captured_native_source_is_current(request, owner, payload)
            && matches!(
                request.captured_native_route(),
                Some(DockCapturedNativeDropRoute::ForbiddenTarget(_))
            )
    }

    pub(crate) fn record_captured_native_source_foreign_surface_feedback(
        &mut self,
        request: &crate::DockViewportDropRouteRequest,
        owner: &DockViewportRoutedPreviewOwner,
        payload: &DockDragPayload,
    ) -> bool {
        if !self.captured_native_source_foreign_surface_is_current(request, owner, payload) {
            return false;
        }
        let resolution = DockViewportResolvedDropRoute::foreign_surface_rejection(request);
        self.status.record_route(request, resolution.route(), None);
        true
    }

    pub(crate) fn update_captured_native_source_foreign_surface_preview(
        &mut self,
        request: &crate::DockViewportDropRouteRequest,
        owner: &DockViewportRoutedPreviewOwner,
        payload: &DockDragPayload,
        source_window: WindowId,
        source_frame: &crate::viewport_drop_scene::DockViewportHostSceneFrame,
        host_position: Point<Pixels>,
    ) -> (bool, DockViewportRuntimeUpdate) {
        if !self.captured_native_source_foreign_surface_is_current(request, owner, payload)
            || !source_frame.matches_viewport(&payload.source_space, source_window)
            || !self
                .frame_coordinator
                .host_scenes()
                .is_current_frame(source_frame)
        {
            let update = self.clear_routed_drop_preview_for_owner(owner);
            return (false, update);
        }

        let Some(route_proof) = self
            .current_route_proof(&payload.source_space, source_window)
            .filter(|proof| proof.registration_key() == source_frame.registration_key())
        else {
            let update = self.clear_routed_drop_preview_for_owner(owner);
            return (false, update);
        };
        let resolution = DockViewportResolvedDropRoute::foreign_surface_rejection(request);
        let route_preview = crate::routed_drop_route_preview_for_host(
            &resolution,
            route_proof,
            host_position,
            Some(owner.clone()),
        );
        if !owner.is_current() {
            let update = self.clear_routed_drop_preview_for_owner(owner);
            return (false, update);
        }
        let update = self.replace_routed_drop_preview(None, route_preview, Some(resolution));
        (true, update)
    }

    pub(crate) fn record_captured_native_foreign_surface_terminal(
        &mut self,
        request: &crate::DockViewportDropRouteRequest,
        owner: &DockViewportRoutedPreviewOwner,
        payload: &DockDragPayload,
    ) -> bool {
        if !self.captured_native_source_foreign_surface_is_current(request, owner, payload) {
            return false;
        }
        let resolution = DockViewportResolvedDropRoute::foreign_surface_rejection(request);
        self.status.record_route(request, resolution.route(), None);
        self.status
            .record_drop_result(&Err(crate::DockActionApplyError::DropTargetUnavailable));
        true
    }

    pub(crate) fn record_captured_native_unavailable_terminal(
        &mut self,
        request: &crate::DockViewportDropRouteRequest,
        owner: &DockViewportRoutedPreviewOwner,
        payload: &DockDragPayload,
    ) -> bool {
        if !self.captured_native_source_is_current(request, owner, payload)
            || !matches!(
                request.captured_native_route(),
                Some(DockCapturedNativeDropRoute::Unavailable)
            )
        {
            return false;
        }
        self.status.record_route(
            request,
            &DockViewportDropRoute::Unavailable,
            Some(crate::DockViewportDropRouteUnavailableReason::NoViewportRouteSelection),
        );
        self.status
            .record_drop_result(&Err(crate::DockActionApplyError::DropTargetUnavailable));
        true
    }

    fn current_route_proof(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockViewportRouteProof> {
        let registration_key = self
            .adapter
            .registration_key(space)
            .filter(|key| key.window_id() == window_id)?;
        let facts_generation = self.adapter.snapshot_facts_generation(space, window_id)?;
        Some(DockViewportRouteProof::new(
            registration_key,
            facts_generation,
        ))
    }

    fn resolution_registration_is_current(
        &self,
        resolution: &DockViewportResolvedDropRoute,
    ) -> bool {
        let route_proof = resolution.route().route_proof();
        match resolution.route() {
            DockViewportDropRoute::Local {
                route_proof,
                source,
                ..
            } => {
                if !self
                    .adapter
                    .is_current_registration(route_proof.registration_key())
                {
                    return false;
                }
                if source.requires_current_route_facts()
                    && self
                        .adapter
                        .snapshot_facts_generation(route_proof.space(), route_proof.window_id())
                        != Some(route_proof.facts_generation())
                {
                    return false;
                }
            }
            DockViewportDropRoute::KnownViewport { target, .. } => {
                let proof = target.route_proof();
                if !self
                    .adapter
                    .is_current_registration(proof.registration_key())
                    || self
                        .adapter
                        .snapshot_facts_generation(proof.space(), proof.window_id())
                        != Some(proof.facts_generation())
                {
                    return false;
                }
            }
            DockViewportDropRoute::TearOff
            | DockViewportDropRoute::Unavailable
            | DockViewportDropRoute::Rejected(_) => {}
        }

        let Some(target) = resolution.routed_preview_target_snapshot() else {
            return true;
        };
        if !self
            .frame_coordinator
            .host_scenes()
            .is_current_frame(target.frame())
        {
            return false;
        }
        let frame_registration = target.frame().registration_key();
        if !self.adapter.is_current_registration(frame_registration) {
            return false;
        }
        if target.facts_generation().is_some_and(|facts_generation| {
            self.adapter.snapshot_facts_generation(
                target.route_proof().space(),
                target.route_proof().window_id(),
            ) != Some(facts_generation)
        }) {
            return false;
        }
        route_proof.is_none_or(|proof| proof == target.route_proof())
    }

    fn replace_routed_drop_preview(
        &mut self,
        next: Option<DockViewportRoutedDropPreview>,
        next_route_preview: Option<DockViewportRoutePreview>,
        next_resolution: Option<DockViewportResolvedDropRoute>,
    ) -> DockViewportRuntimeUpdate {
        let replacement =
            self.routed_drop_preview
                .replace(next, next_route_preview, next_resolution);
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(replacement.has_changed());
        update.extend_windows(self.windows_for_routed_preview_replacement(&replacement));
        update
    }

    fn windows_for_routed_preview_replacement(
        &self,
        replacement: &DockViewportRoutedDropPreviewReplacement,
    ) -> Vec<AnyWindowHandle> {
        let mut windows = Vec::new();
        for space in replacement.affected_spaces() {
            crate::push_unique_window(&mut windows, self.adapter.window_for_space(space));
        }
        windows
    }

    pub(crate) fn clear_routed_drop_preview(&mut self) -> DockViewportRuntimeUpdate {
        self.replace_routed_drop_preview(None, None, None)
    }

    pub(super) fn clear_routed_drop_preview_if_window_matches(
        &mut self,
        window_id: WindowId,
    ) -> DockViewportRuntimeUpdate {
        self.payload_drag
            .clear_last_routed_viewport_identity_if_window_matches(window_id);
        if self.routed_drop_preview.targets_window(window_id) {
            self.replace_routed_drop_preview(None, None, None)
        } else {
            DockViewportRuntimeUpdate::default()
        }
    }

    pub(crate) fn clear_routed_drop_preview_for_drag_session(
        &mut self,
        session: Option<&DockRuntimeDragSession>,
    ) -> DockViewportRuntimeUpdate {
        let Some(session) = session else {
            return DockViewportRuntimeUpdate::default();
        };
        self.payload_drag
            .clear_last_routed_viewport_identity_for_session(session);
        let replacement = self
            .routed_drop_preview
            .clear_for_drag_session(Some(session));
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(replacement.has_changed());
        update.extend_windows(self.windows_for_routed_preview_replacement(&replacement));
        update
    }

    pub(crate) fn clear_routed_drop_preview_for_owner(
        &mut self,
        owner: &DockViewportRoutedPreviewOwner,
    ) -> DockViewportRuntimeUpdate {
        let replacement = self.routed_drop_preview.clear_for_owner(owner);
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(replacement.has_changed());
        update.extend_windows(self.windows_for_routed_preview_replacement(&replacement));
        update
    }

    pub(crate) fn clear_routed_drop_preview_for_target_scene_frame(
        &mut self,
        frame: &crate::viewport_drop_scene::DockViewportHostSceneFrame,
    ) -> DockViewportRuntimeUpdate {
        let replacement = self.routed_drop_preview.clear_for_target_scene_frame(frame);
        let mut update = DockViewportRuntimeUpdate::default();
        update.mark_changed(replacement.has_changed());
        update.extend_windows(self.windows_for_routed_preview_replacement(&replacement));
        update
    }
}
