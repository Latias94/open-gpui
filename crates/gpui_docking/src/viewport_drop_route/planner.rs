use crate::{
    DockViewportAdapter, DockViewportRouteSelectionSource, DockViewportTargetContext,
    DockViewportTargetHit, DockViewportWindowHit,
    interaction::DockPayloadDropReleaseOrigin,
    viewport_registry::DockViewportInputMask,
    viewport_target_resolver::{DockViewportRouteSelection, resolve_viewport_route_selection},
};

use super::{
    event_receiver::{
        DockEventReceiverLocalSceneRouteContext, DockEventReceiverLocalSceneRouteContextMode,
        DockTrustedHoveredWindowLocalDropTarget,
    },
    model::{
        DockViewportDropRoute, DockViewportDropRoutePlan, DockViewportDropRouteUnavailableReason,
        unavailable_route_selection_reason,
    },
    request::{DockViewportDropRouteRequest, DockViewportPointerCoordinateSpace},
};

impl DockViewportAdapter {
    pub(super) fn normalize_target_context(
        &self,
        target_context: DockViewportTargetContext,
    ) -> DockViewportTargetContext {
        let target_context =
            if target_context
                .trusted_hovered_window()
                .is_some_and(|hovered_window| {
                    self.window_input_mask(hovered_window)
                        == Some(DockViewportInputMask::NoInputPassThrough)
                })
            {
                target_context.without_trusted_hovered_window()
            } else {
                target_context
            };

        if target_context
            .drag_last_hovered_window()
            .is_some_and(|last_hovered_window| {
                self.window_input_mask(last_hovered_window)
                    == Some(DockViewportInputMask::NoInputPassThrough)
            })
        {
            return target_context.without_drag_last_hovered_window();
        }

        target_context
    }

    pub(super) fn resolve_payload_drop_route_plan(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> DockViewportDropRoutePlan {
        if self.trusted_hovered_window_is_known_but_unusable(target_context) {
            return DockViewportDropRoutePlan::unavailable(
                DockViewportDropRouteUnavailableReason::BlockedByViewportWindow,
            );
        }
        match request.coordinate_space() {
            DockViewportPointerCoordinateSpace::GlobalScreen => {
                if let Some(plan) =
                    self.resolve_global_screen_payload_drop_route_plan(request, target_context)
                {
                    return plan;
                }
            }
            DockViewportPointerCoordinateSpace::TrustedHoveredWindowLocal => {
                return DockViewportDropRoutePlan::route(
                    self.resolve_trusted_hovered_window_local_payload_drop_route(
                        request,
                        target_context,
                    ),
                );
            }
            DockViewportPointerCoordinateSpace::EventReceiverLocal => {
                if let Some(route) = self
                    .resolve_event_receiver_local_scene_payload_drop_route(request, target_context)
                {
                    return DockViewportDropRoutePlan::route(route);
                }
                return DockViewportDropRoutePlan::unavailable(unavailable_route_selection_reason(
                    target_context,
                ));
            }
            DockViewportPointerCoordinateSpace::SourceLocalOnly => {
                let local_route =
                    self.resolve_source_local_payload_drop_route(request, target_context);
                if !matches!(local_route, DockViewportDropRoute::Unavailable) {
                    return DockViewportDropRoutePlan::route(local_route);
                }
            }
        }

        DockViewportDropRoutePlan::OutsideRegisteredViewport
    }

    fn trusted_hovered_window_is_known_but_unusable(
        &self,
        target_context: &DockViewportTargetContext,
    ) -> bool {
        target_context
            .trusted_hovered_window()
            .is_some_and(|hovered| self.window_can_route_hover_hit(hovered) == Some(false))
    }

    fn resolve_global_screen_payload_drop_route_plan(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> Option<DockViewportDropRoutePlan> {
        let window_hits = self.global_screen_viewport_window_hits(request.release_position());
        let has_any_hits = !window_hits.is_empty();
        let has_blocking_window_hit = window_hits
            .iter()
            .any(DockViewportWindowHit::blocks_host_target);
        let host_hits = window_hits
            .iter()
            .filter_map(DockViewportWindowHit::target_hit)
            .collect::<Vec<_>>();
        let event_receiver_target =
            self.event_receiver_local_scene_target_from_hits(request, target_context, &host_hits);
        let resolution = resolve_viewport_route_selection(window_hits, target_context);
        let Some(resolution) = resolution.or(event_receiver_target) else {
            if let Some(route) =
                self.resolve_event_receiver_global_scene_payload_drop_route(request, target_context)
            {
                return Some(DockViewportDropRoutePlan::route(route));
            }
            if has_blocking_window_hit {
                return Some(DockViewportDropRoutePlan::unavailable(
                    DockViewportDropRouteUnavailableReason::BlockedByViewportWindow,
                ));
            }
            return has_any_hits.then(|| {
                DockViewportDropRoutePlan::unavailable(unavailable_route_selection_reason(
                    target_context,
                ))
            });
        };
        Some(self.route_plan_from_viewport_route_selection(request, resolution))
    }

    fn route_plan_from_viewport_route_selection(
        &self,
        request: &DockViewportDropRouteRequest,
        resolution: DockViewportRouteSelection,
    ) -> DockViewportDropRoutePlan {
        let route_selection_source = resolution.source();
        let Some(target) = resolution.into_target().into_target_hit() else {
            return DockViewportDropRoutePlan::unavailable(
                DockViewportDropRouteUnavailableReason::BlockedByViewportWindow,
            );
        };
        let source_only_cross_viewport_without_trusted_hover = request.release_origin()
            == DockPayloadDropReleaseOrigin::SourceOnly
            && target.space() != request.source_space()
            && route_selection_source != DockViewportRouteSelectionSource::TrustedHoveredWindow;
        if source_only_cross_viewport_without_trusted_hover {
            return DockViewportDropRoutePlan::unavailable(
                DockViewportDropRouteUnavailableReason::NoViewportRouteSelection,
            );
        }
        if target.space() == request.source_space() {
            return DockViewportDropRoutePlan::route(DockViewportDropRoute::Local {
                host_position: target.host_position(),
                route_proof: target.route_proof().clone(),
                source: route_selection_source,
            });
        }
        DockViewportDropRoutePlan::route(DockViewportDropRoute::KnownViewport {
            target,
            source: route_selection_source,
        })
    }

    fn event_receiver_local_scene_target_from_hits(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
        hits: &[DockViewportTargetHit],
    ) -> Option<DockViewportRouteSelection> {
        let route_context = self.event_receiver_local_scene_route_context(
            request,
            target_context,
            DockEventReceiverLocalSceneRouteContextMode::HitTestedScene,
        )?;
        let receiver_hit = hits.iter().find(|hit| {
            hit.window_id() == route_context.route_proof.window_id()
                && hit.space() == request.source_space()
        })?;
        Some(DockViewportRouteSelection::event_receiver_local_scene(
            receiver_hit.clone(),
        ))
    }

    fn resolve_event_receiver_local_scene_payload_drop_route(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> Option<DockViewportDropRoute> {
        let route_context = self.event_receiver_local_scene_route_context(
            request,
            target_context,
            DockEventReceiverLocalSceneRouteContextMode::ReceiverSceneProof,
        )?;
        let Some(host_position) =
            route_context.host_position_from_window_position(request.release_position())
        else {
            return None;
        };
        Some(route_context.local_route(host_position))
    }

    fn resolve_event_receiver_global_scene_payload_drop_route(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> Option<DockViewportDropRoute> {
        let route_context = self.event_receiver_local_scene_route_context(
            request,
            target_context,
            DockEventReceiverLocalSceneRouteContextMode::ReceiverSceneProof,
        )?;
        let screen_bounds = route_context.global_screen_bounds?;
        if !screen_bounds.contains(&request.release_position()) {
            return None;
        }
        let window_position = open_gpui::point(
            request.release_position().x - screen_bounds.origin.x,
            request.release_position().y - screen_bounds.origin.y,
        );
        let Some(host_position) = route_context.host_position_from_window_position(window_position)
        else {
            return None;
        };
        Some(route_context.local_route(host_position))
    }

    fn event_receiver_local_scene_route_context(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
        mode: DockEventReceiverLocalSceneRouteContextMode,
    ) -> Option<DockEventReceiverLocalSceneRouteContext> {
        let proof_required = match (mode, target_context.trusted_hovered_signal()) {
            (
                DockEventReceiverLocalSceneRouteContextMode::HitTestedScene,
                crate::DockViewportTrustedHoveredSignal::Unavailable,
            ) => false,
            (
                DockEventReceiverLocalSceneRouteContextMode::HitTestedScene,
                crate::DockViewportTrustedHoveredSignal::TrustedNone,
            )
            | (
                DockEventReceiverLocalSceneRouteContextMode::ReceiverSceneProof,
                crate::DockViewportTrustedHoveredSignal::Unavailable
                | crate::DockViewportTrustedHoveredSignal::TrustedNone,
            ) => true,
            _ => return None,
        };
        let proof = request.event_receiver_local_scene_proof();
        if proof_required && proof.is_none() {
            return None;
        }
        let Some(receiver_window) = request.event_receiver_window() else {
            return None;
        };
        let Some(receiver_space) = self.space_for_window_id(receiver_window) else {
            return None;
        };
        if receiver_space != request.source_space() {
            return None;
        }
        let Some(snapshot) = self.snapshot(request.source_space()) else {
            return None;
        };
        if snapshot.window.window_id() != receiver_window {
            return None;
        }
        if let Some(proof) = proof {
            if !proof.matches_viewport(request.source_space(), receiver_window)
                || proof.registration_key() != &snapshot.registration_key(request.source_space())
            {
                return None;
            }
        }
        let facts_generation = match self
            .snapshot_facts_generation(request.source_space(), receiver_window)
            .or_else(|| {
                let _proof = proof?;
                if request.coordinate_space()
                    != DockViewportPointerCoordinateSpace::EventReceiverLocal
                    || snapshot.is_platform_close_requested()
                    || snapshot.input_mask == DockViewportInputMask::Minimized
                {
                    return None;
                }
                Some(snapshot.facts_generation())
            }) {
            Some(facts_generation) => facts_generation,
            None => {
                return None;
            }
        };
        let Some(host_geometry) = snapshot.host_geometry.as_ref() else {
            return None;
        };
        if request.coordinate_space() != DockViewportPointerCoordinateSpace::EventReceiverLocal
            && snapshot.global_screen_bounds().is_none()
        {
            return None;
        }
        Some(DockEventReceiverLocalSceneRouteContext {
            route_proof: crate::DockViewportRouteProof::new(
                snapshot.registration_key(request.source_space()),
                facts_generation,
            ),
            host_geometry: host_geometry.clone(),
            global_screen_bounds: snapshot.global_screen_bounds(),
        })
    }

    fn resolve_trusted_hovered_window_local_payload_drop_route(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> DockViewportDropRoute {
        if let Some(target) = self.trusted_hovered_window_local_drop_target(request, target_context)
        {
            if &target.target_space == request.source_space() {
                return DockViewportDropRoute::Local {
                    host_position: target.host_position,
                    route_proof: target.route_proof,
                    source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
                };
            }

            return DockViewportDropRoute::KnownViewport {
                target: crate::DockViewportTargetHit::with_route_proof(
                    target.target_window,
                    target.host_position,
                    target.route_proof,
                ),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            };
        }

        DockViewportDropRoute::Unavailable
    }

    fn trusted_hovered_window_local_drop_target(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> Option<DockTrustedHoveredWindowLocalDropTarget> {
        let receiver_window = request.event_receiver_window()?;
        if target_context.trusted_hovered_window() != Some(receiver_window) {
            return None;
        }
        let target_space = self.space_for_window_id(receiver_window).cloned()?;
        let target_window = self.window_for_space(&target_space)?;
        let host_position = self.window_to_host(&target_space, request.release_position())?;
        let facts_generation = self.snapshot_facts_generation(&target_space, receiver_window)?;
        let registration_key = self.registration_key(&target_space)?;
        Some(DockTrustedHoveredWindowLocalDropTarget {
            target_space,
            target_window,
            host_position,
            route_proof: crate::DockViewportRouteProof::new(registration_key, facts_generation),
        })
    }

    fn resolve_source_local_payload_drop_route(
        &self,
        request: &DockViewportDropRouteRequest,
        target_context: &DockViewportTargetContext,
    ) -> DockViewportDropRoute {
        if let Some(hovered_window) = target_context.trusted_hovered_window()
            && self
                .window_for_space(request.source_space())
                .is_some_and(|window| window.window_id() == hovered_window)
            && let Some(host_position) =
                self.window_to_host(request.source_space(), request.release_position())
        {
            let Some(facts_generation) =
                self.snapshot_facts_generation(request.source_space(), hovered_window)
            else {
                return DockViewportDropRoute::Unavailable;
            };
            let Some(registration_key) = self.registration_key(request.source_space()) else {
                return DockViewportDropRoute::Unavailable;
            };
            return DockViewportDropRoute::Local {
                host_position,
                route_proof: crate::DockViewportRouteProof::new(registration_key, facts_generation),
                source: DockViewportRouteSelectionSource::TrustedHoveredWindow,
            };
        }
        DockViewportDropRoute::Unavailable
    }
}
