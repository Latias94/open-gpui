use super::*;
use crate::DockViewportHostGeometry;

fn admits_scene_registration(
    runtime: &DockViewportRuntime,
    work_context: DockViewportRuntimeWorkContext,
    current_registration: Option<&DockViewportRegistrationKey>,
    expected_registration: Option<&DockViewportRegistrationKey>,
) -> bool {
    match expected_registration {
        Some(expected) => {
            current_registration == Some(expected)
                && runtime.admits_registration_in_context(work_context, expected)
        }
        None => runtime.admits_work_context(work_context) && current_registration.is_none(),
    }
}

fn sample_viewport_frame_request<C: open_gpui::AppContext>(
    request: DockViewportFrameSampleRequest,
    cx: &mut C,
) -> DockViewportFrameSample {
    let observation = request
        .window()
        .update(cx, |_, window, _| {
            if window.is_minimized() {
                DockViewportFrameObservation::InputMask(
                    crate::viewport_registry::DockViewportInputMask::Minimized,
                )
            } else if !window.accepts_pointer_input() {
                DockViewportFrameObservation::InputMask(
                    crate::viewport_registry::DockViewportInputMask::NoInputPassThrough,
                )
            } else {
                DockViewportFrameObservation::InputMask(
                    crate::viewport_registry::DockViewportInputMask::ReceivesInput,
                )
            }
        })
        .unwrap_or(DockViewportFrameObservation::Unavailable);
    DockViewportFrameSample::new(request, observation)
}

impl DockViewportRuntimeHandle {
    #[cfg(test)]
    pub(crate) fn begin_viewport_host_scene(
        &self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_geometry: impl Into<DockViewportHostGeometry>,
        host_position: Point<Pixels>,
    ) -> bool {
        self.runtime.borrow_mut().begin_viewport_host_scene(
            space,
            window_id,
            window_facts,
            host_geometry,
            host_position,
        )
    }

    #[cfg(test)]
    pub(crate) fn unregister_host_for_space_with_app(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
        cx: &mut App,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .unregister_host_for_space_with_cleanup(space, window_id);
        apply_runtime_update(self, update, cx)
    }

    pub(crate) fn begin_viewport_host_scene_frame(
        &self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_geometry: impl Into<DockViewportHostGeometry>,
        host_position: Point<Pixels>,
        drop_guide_metrics: crate::DockDropGuideMetrics,
    ) -> Option<DockViewportHostSceneRegistration> {
        self.begin_viewport_host_scene_frame_with_facts(
            space,
            window_id,
            window_facts,
            host_geometry,
            host_position,
            drop_guide_metrics,
            Vec::new(),
        )
    }

    pub(crate) fn begin_viewport_host_scene_frame_with_facts(
        &self,
        space: impl Into<DockSpaceId>,
        window_id: WindowId,
        window_facts: DockViewportWindowFacts,
        host_geometry: impl Into<DockViewportHostGeometry>,
        host_position: Point<Pixels>,
        drop_guide_metrics: crate::DockDropGuideMetrics,
        initial_facts: Vec<DockHostDropSceneFact>,
    ) -> Option<DockViewportHostSceneRegistration> {
        self.runtime
            .borrow_mut()
            .begin_viewport_host_scene_frame_with_facts(
                space,
                window_id,
                window_facts,
                host_geometry,
                host_position,
                drop_guide_metrics,
                initial_facts,
            )
    }

    pub(crate) fn prepare_rendered_viewport_host_scene_draft(
        &self,
        draft: DockViewportHostSceneDraft,
        expected_registration: Option<&DockViewportRegistrationKey>,
        work_context: DockViewportRuntimeWorkContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<DockViewportRenderedHostScenePreparation> {
        let space = draft.space.clone();
        let window_id = window.window_handle().window_id();
        let registration_is_admitted = {
            let runtime = self.runtime.borrow();
            let current_registration = runtime.registration_key_for_space_window(&space, window_id);
            admits_scene_registration(
                &runtime,
                work_context,
                current_registration.as_ref(),
                expected_registration,
            )
        };
        if draft.window_id != window_id || !registration_is_admitted {
            return None;
        }

        let backend_focus_changed = self.reconcile_backend_window_focus(cx);
        let viewport_frame_changed = self.reconcile_viewport_frame_except_window(window_id, cx);
        let window_facts = DockViewportWindowFacts::from_window(window, cx);
        Some(DockViewportRenderedHostScenePreparation {
            changed: backend_focus_changed || viewport_frame_changed,
            draft,
            expected_registration: expected_registration.cloned(),
            update_generation: cx.current_update_generation(),
            work_context,
            window: window.window_handle(),
            window_facts,
        })
    }

    pub(crate) fn finalize_rendered_viewport_host_scene_draft(
        &self,
        preparation: DockViewportRenderedHostScenePreparation,
    ) -> DockViewportRenderedHostSceneCommit {
        let DockViewportRenderedHostScenePreparation {
            changed,
            draft,
            expected_registration,
            update_generation,
            work_context,
            window,
            window_facts,
        } = preparation;
        let space = draft.space.clone();
        let window_id = draft.window_id;
        let mut runtime = self.runtime.borrow_mut();
        let current_registration = runtime.registration_key_for_space_window(&space, window_id);
        if !admits_scene_registration(
            &runtime,
            work_context,
            current_registration.as_ref(),
            expected_registration.as_ref(),
        ) {
            return DockViewportRenderedHostSceneCommit {
                changed,
                work_context,
                frame: None,
                registration_update: DockViewportRuntimeUpdate::default(),
                route_preview_update: DockViewportRuntimeUpdate::default(),
            };
        }

        let mut registration_update = runtime.register_rendered_host_viewport_with_cleanup(
            space.clone(),
            window,
            work_context,
        );
        let registration = runtime
            .registration_key_for_space_window(&space, window_id)
            .and_then(|registration_key| draft.bind(registration_key))
            .and_then(|snapshot| {
                runtime.commit_viewport_host_scene_snapshot_at_update(
                    snapshot,
                    window_facts,
                    Some(update_generation),
                )
            });
        let mut route_preview_update = runtime.clear_preview_for_unready_window_route(window_id);
        route_preview_update.bind_work_context(work_context);
        let (host_scene_changed, placement_changed, frame) = registration
            .map(|registration| {
                (
                    registration.changed,
                    registration.placement_changed,
                    Some(registration.frame),
                )
            })
            .unwrap_or((false, false, None));
        registration_update.mark_observed_viewport_placement(placement_changed, work_context);
        DockViewportRenderedHostSceneCommit {
            changed: changed || host_scene_changed,
            work_context,
            frame,
            registration_update,
            route_preview_update,
        }
    }

    #[cfg(test)]
    pub(crate) fn publish_rendered_viewport_host_scene_commit(
        &self,
        commit: DockViewportRenderedHostSceneCommit,
        cx: &mut App,
    ) -> bool {
        if !self.admits_rendered_viewport_host_scene_commit(&commit) {
            return false;
        }
        let registration_changed =
            refresh_runtime_update_with_commit(self, commit.registration_update, cx);
        let route_preview_changed = refresh_runtime_update(commit.route_preview_update, cx);
        commit.changed || registration_changed || route_preview_changed
    }

    pub(crate) fn publish_rendered_viewport_host_scene_commit_from_window(
        &self,
        commit: DockViewportRenderedHostSceneCommit,
        window: &Window,
        cx: &mut App,
    ) -> bool {
        if !self.admits_rendered_viewport_host_scene_commit(&commit) {
            return false;
        }
        let current_window = Some(window.window_handle().window_id());
        self.publish_surface_commit(&commit.registration_update, cx);
        let registration_changed =
            refresh_runtime_update_excluding(commit.registration_update, current_window, cx);
        let route_preview_changed =
            refresh_runtime_update_excluding(commit.route_preview_update, current_window, cx);
        commit.changed || registration_changed || route_preview_changed
    }

    fn admits_rendered_viewport_host_scene_commit(
        &self,
        commit: &DockViewportRenderedHostSceneCommit,
    ) -> bool {
        let runtime = self.runtime.borrow();
        runtime.admits_work_context(commit.work_context)
            && commit.frame.as_ref().is_none_or(|frame| {
                runtime
                    .admits_registration_in_context(commit.work_context, frame.registration_key())
            })
    }

    pub(crate) fn rollback_rendered_viewport_host_scene_frame(
        &self,
        frame: &DockViewportHostSceneFrame,
        commit: &mut DockViewportRenderedHostSceneCommit,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .discard_viewport_host_scene_frame_exact(frame);
        let changed = update.changed();
        commit.route_preview_update.merge(update);
        changed
    }

    pub(crate) fn is_current_viewport_host_scene_frame(
        &self,
        frame: &DockViewportHostSceneFrame,
    ) -> bool {
        self.runtime
            .borrow()
            .is_current_viewport_host_scene_frame(frame)
    }

    pub(crate) fn discard_rendered_viewport_host_scene_frame_exact_from_window(
        &self,
        frame: &DockViewportHostSceneFrame,
        window: &Window,
        cx: &mut App,
    ) -> bool {
        let update = self
            .runtime
            .borrow_mut()
            .discard_viewport_host_scene_frame_exact(frame);
        refresh_runtime_update_excluding(update, Some(window.window_handle().window_id()), cx)
    }

    pub(crate) fn sync_rendered_viewport_pointer_input(
        &self,
        registration: &DockViewportRegistrationKey,
        passthrough_pointer_input: bool,
        window: &mut Window,
    ) -> bool {
        let window_id = window.window_handle().window_id();
        if registration.window_id() != window_id
            || !self.runtime.borrow().admits_registration(registration)
        {
            return false;
        }

        let accepts_pointer_input = window.platform_facts().accepts_pointer_input;
        let pending_pointer_input = self
            .pending_platform_mutation_request(
                window_id,
                WindowMutationDomain::PointerInput,
                Some(registration),
            )
            .and_then(|request| match request {
                WindowMutationRequest::PointerInput(accepts_pointer_input) => {
                    Some(accepts_pointer_input)
                }
                WindowMutationRequest::Placement(_)
                | WindowMutationRequest::ActivationPolicy(_)
                | WindowMutationRequest::Alpha(_)
                | WindowMutationRequest::Topmost(_)
                | WindowMutationRequest::TaskbarVisibility(_) => None,
            });
        let pointer_input_resolution = {
            let mut runtime = self.runtime.borrow_mut();
            if !runtime.admits_registration(registration) {
                return false;
            }
            resolve_render_passthrough_pointer_input_request(
                &mut runtime,
                window_id,
                accepts_pointer_input,
                pending_pointer_input,
                passthrough_pointer_input,
            )
        };
        let retry_blocked = pointer_input_resolution.target.is_some_and(|target| {
            self.platform_mutation_retry_is_blocked(
                window_id,
                WindowMutationRequest::PointerInput(target),
                window.platform_facts(),
                Some(registration),
            )
        });
        let pointer_input_request = pointer_input_resolution.request.filter(|_| !retry_blocked);
        if !self.runtime.borrow().admits_registration(registration) {
            return false;
        }
        let Some(pointer_sync) = pointer_input_request.and_then(|accepts_pointer_input| {
            (pending_pointer_input != Some(accepts_pointer_input))
                .then(|| sync_pointer_input_window(window, accepts_pointer_input))
        }) else {
            return false;
        };
        let queued = pointer_sync.record().dispatches.iter().any(|dispatch| {
            matches!(
                dispatch,
                crate::DockViewportPlatformSyncDispatch::Queued { .. }
            )
        });
        let recorded = self.record_platform_dispatch_result(
            pointer_sync,
            window.platform_facts(),
            Some(registration),
        );
        if !recorded {
            window.refresh();
        }
        queued
    }

    #[cfg(test)]
    #[cfg(test)]
    pub(crate) fn discard_rendered_viewport_host_scene_frame(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
        expected_registration: Option<&DockViewportRegistrationKey>,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .discard_viewport_host_scene_frame(space, window_id, expected_registration)
            .changed()
    }

    pub(crate) fn reconcile_viewport_frame<C: open_gpui::AppContext>(&self, cx: &mut C) -> bool {
        self.reconcile_viewport_frame_skipping(None, cx)
    }

    pub(crate) fn reconcile_viewport_frame_except_window<C: open_gpui::AppContext>(
        &self,
        skip_window_id: WindowId,
        cx: &mut C,
    ) -> bool {
        self.reconcile_viewport_frame_skipping(Some(skip_window_id), cx)
    }

    pub(super) fn reconcile_viewport_frame_skipping<C: open_gpui::AppContext>(
        &self,
        skip_window_id: Option<WindowId>,
        cx: &mut C,
    ) -> bool {
        self.reconcile_viewport_frame_with(skip_window_id, cx, sample_viewport_frame_request)
    }

    fn reconcile_viewport_frame_with<C, S>(
        &self,
        skip_window_id: Option<WindowId>,
        cx: &mut C,
        mut sample: S,
    ) -> bool
    where
        C: open_gpui::AppContext,
        S: FnMut(DockViewportFrameSampleRequest, &mut C) -> DockViewportFrameSample,
    {
        let requests = self
            .runtime
            .borrow()
            .prepare_viewport_frame_reconciliation(skip_window_id);
        let samples = requests
            .into_iter()
            .map(|request| sample(request, cx))
            .collect();
        let update = self
            .runtime
            .borrow_mut()
            .finalize_viewport_frame_reconciliation(samples);
        refresh_runtime_update(update, cx)
    }

    #[cfg(test)]
    pub(crate) fn push_viewport_host_scene_fact(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
        fact: DockHostDropSceneFact,
    ) -> bool {
        self.runtime
            .borrow_mut()
            .push_viewport_host_scene_fact(space, window_id, fact)
    }

    pub(crate) fn push_viewport_host_scene_frame_fact(
        &self,
        frame: &DockViewportHostSceneFrame,
        fact: DockHostDropSceneFact,
    ) -> Option<DockViewportHostSceneFrame> {
        self.runtime
            .borrow_mut()
            .push_viewport_host_scene_frame_fact(frame, fact)
    }

    pub(crate) fn rendered_leaf_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.runtime
            .borrow()
            .rendered_leaf_bounds_for_tabs(space, window_id, tabs)
    }

    pub(crate) fn rendered_leaf_displayed_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.runtime
            .borrow()
            .rendered_leaf_displayed_bounds_for_tabs(space, window_id, tabs)
    }

    pub(crate) fn rendered_tab_bar_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
    ) -> Option<Bounds<Pixels>> {
        self.runtime
            .borrow()
            .rendered_tab_bar_bounds_for_tabs(space, window_id, tabs)
    }

    pub(crate) fn rendered_tab_label_bounds_for_tabs(
        &self,
        space: &DockSpaceId,
        window_id: Option<WindowId>,
        tabs: DockNodeId,
        target_index: usize,
    ) -> Option<Bounds<Pixels>> {
        self.runtime.borrow().rendered_tab_label_bounds_for_tabs(
            space,
            window_id,
            tabs,
            target_index,
        )
    }

    pub(crate) fn window_id_for_space(&self, space: &DockSpaceId) -> Option<WindowId> {
        self.runtime
            .borrow()
            .adapter()
            .window_for_space(space)
            .map(|window| window.window_id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_viewport_runtime_test_support::DockViewportRuntimeFixture;
    use crate::surface::window_session::{
        DockSurfaceWindowSession, DockSurfaceWindowSessionLease,
        DockSurfaceWindowSessionShutdownConvergenceOutcome, DockSurfaceWindowSessionShutdownReason,
        DockSurfaceWindowSessionTerminalDisposition,
    };
    use crate::{drop_target::DockEmptySpaceDropTarget, viewport_test_support::bounds};
    use open_gpui::{Empty, EntityId, TestAppContext, WindowId, point, px, size};

    fn active_surface_lease(
        session: &mut DockSurfaceWindowSession,
        anchor: WindowId,
    ) -> DockSurfaceWindowSessionLease {
        let opening = session
            .reserve_opening()
            .expect("the surface session should reserve an opening generation");
        session
            .commit_opening(opening, anchor)
            .expect("the reserved surface generation should activate")
    }

    fn reopen_surface_runtime(
        runtime: &DockViewportRuntimeHandle,
        session: &mut DockSurfaceWindowSession,
        lease: DockSurfaceWindowSessionLease,
        next_anchor: WindowId,
        cx: &mut App,
    ) -> DockSurfaceWindowSessionLease {
        session.begin_shutdown(
            lease,
            DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
            std::iter::empty(),
        );
        let reservation = runtime
            .freeze_surface_shutdown(lease)
            .expect("the active generation should begin runtime shutdown");
        assert!(
            reservation.windows().is_empty(),
            "the scene-only generation should not own platform windows"
        );
        assert!(runtime.commit_surface_shutdown(reservation, cx).is_empty());
        session.mark_runtime_empty(lease);
        session.settle_terminal(
            lease,
            lease.anchor(),
            DockSurfaceWindowSessionTerminalDisposition::ObservedClosed,
        );
        assert_eq!(
            session.complete_shutdown(lease),
            DockSurfaceWindowSessionShutdownConvergenceOutcome::Closed
        );
        let next = active_surface_lease(session, next_anchor);
        assert_eq!(
            runtime.activate_surface_lineage(next),
            DockViewportRuntimeLineageActivationOutcome::Activated
        );
        next
    }

    #[open_gpui::test]
    fn reconcile_viewport_frame_releases_runtime_borrow_before_external_sample(
        cx: &mut TestAppContext,
    ) {
        let space = DockSpaceId::from("main");
        let fixture = DockViewportRuntimeFixture::builder(space.clone())
            .space(space.clone(), ["a"])
            .allow_platform_viewports(true)
            .build(cx);
        let opened = fixture.open_unfocused_viewport(cx, &space);
        let expected_window_id = opened.window().window_id();
        let runtime = fixture.runtime.clone();
        let sampled = Cell::new(false);

        cx.update(|app| {
            runtime.reconcile_viewport_frame_with(None, app, |request, _| {
                assert_eq!(
                    runtime.window_id_for_space(&space),
                    Some(expected_window_id),
                    "sampling must run after the runtime prepare borrow is released"
                );
                sampled.set(true);
                DockViewportFrameSample::new(
                    request,
                    DockViewportFrameObservation::InputMask(
                        crate::viewport_registry::DockViewportInputMask::ReceivesInput,
                    ),
                )
            });
        });

        assert!(sampled.get(), "the registered viewport should be sampled");
    }

    #[open_gpui::test]
    fn unregistered_scene_preparation_cannot_finalize_after_surface_generation_switch(
        cx: &mut TestAppContext,
    ) {
        let space = DockSpaceId::from("main");
        let controller = DockViewportRuntimeFixture::builder(space.clone())
            .space(space.clone(), ["a"])
            .build_controller(cx)
            .controller;
        let authority = EntityId::from(101);
        let runtime = DockViewportRuntimeHandle::for_surface(
            controller,
            authority,
            DockViewportClosePolicy::default(),
            None,
        );
        let mut session = DockSurfaceWindowSession::new(authority);
        let g1 = active_surface_lease(&mut session, WindowId::from(1001));
        assert_eq!(
            runtime.activate_surface_lineage(g1),
            DockViewportRuntimeLineageActivationOutcome::Activated
        );
        let window: AnyWindowHandle = cx
            .open_window(size(px(480.0), px(320.0)), |_, _| Empty)
            .into();
        let window_id = window.window_id();
        let host_bounds = bounds(0.0, 0.0, 480.0, 320.0);
        let preparation = window
            .update(cx, |_, window, app| {
                runtime
                    .prepare_rendered_viewport_host_scene_draft(
                        DockViewportHostSceneDraft::new(
                            space.clone(),
                            window_id,
                            DockViewportWindowFacts::from_window(window, app).current_bounds,
                            host_bounds,
                            point(px(12.0), px(12.0)),
                            crate::DockDropGuideMetrics::default(),
                        ),
                        None,
                        DockViewportRuntimeWorkContext::new(
                            DockViewportRuntimeLineage::Surface(g1),
                            None,
                        ),
                        window,
                        app,
                    )
                    .expect("G1 should prepare while its exact session is active")
            })
            .expect("the raw window should remain live");

        let g2 = cx.update(|app| {
            reopen_surface_runtime(&runtime, &mut session, g1, WindowId::from(1002), app)
        });
        let commit = runtime.finalize_rendered_viewport_host_scene_draft(preparation);

        assert!(commit.frame.is_none(), "G1 work must not finalize under G2");
        assert_eq!(
            runtime.registration_key_for_space_window(&space, window_id),
            None,
            "an absent expected registration must not become a wildcard across generations"
        );
        assert!(runtime.windows_for_surface(g1).is_empty());
        assert!(runtime.windows_for_surface(g2).is_empty());
    }

    #[open_gpui::test]
    fn rendered_scene_registration_is_minted_only_by_unmanaged_runtime(cx: &mut TestAppContext) {
        let space = DockSpaceId::from("main");
        let controller = DockViewportRuntimeFixture::builder(space.clone())
            .space(space.clone(), ["a"])
            .build_controller(cx)
            .controller;
        let authority = EntityId::from(102);
        let surface_runtime = DockViewportRuntimeHandle::for_surface(
            controller.clone(),
            authority,
            DockViewportClosePolicy::default(),
            None,
        );
        let mut session = DockSurfaceWindowSession::new(authority);
        let lease = active_surface_lease(&mut session, WindowId::from(2001));
        assert_eq!(
            surface_runtime.activate_surface_lineage(lease),
            DockViewportRuntimeLineageActivationOutcome::Activated
        );
        let surface_window: AnyWindowHandle = cx
            .open_window(size(px(480.0), px(320.0)), |_, _| Empty)
            .into();
        let surface_window_id = surface_window.window_id();
        let host_bounds = bounds(0.0, 0.0, 480.0, 320.0);
        let surface_preparation = surface_window
            .update(cx, |_, window, app| {
                surface_runtime
                    .prepare_rendered_viewport_host_scene_draft(
                        DockViewportHostSceneDraft::new(
                            space.clone(),
                            surface_window_id,
                            DockViewportWindowFacts::from_window(window, app).current_bounds,
                            host_bounds,
                            point(px(12.0), px(12.0)),
                            crate::DockDropGuideMetrics::default(),
                        ),
                        None,
                        DockViewportRuntimeWorkContext::new(
                            DockViewportRuntimeLineage::Surface(lease),
                            None,
                        ),
                        window,
                        app,
                    )
                    .expect("the active surface context may sample an unregistered scene")
            })
            .expect("the surface test window should remain live");
        let surface_commit =
            surface_runtime.finalize_rendered_viewport_host_scene_draft(surface_preparation);
        assert!(
            surface_commit.frame.is_none(),
            "surface rendering must not mint ownership for an arbitrary window"
        );
        assert_eq!(
            surface_runtime.registration_key_for_space_window(&space, surface_window_id),
            None
        );
        assert!(surface_runtime.windows_for_surface(lease).is_empty());

        let unmanaged_runtime = DockViewportRuntimeHandle::new(controller);
        let unmanaged_window: AnyWindowHandle = cx
            .open_window(size(px(480.0), px(320.0)), |_, _| Empty)
            .into();
        let unmanaged_window_id = unmanaged_window.window_id();
        let unmanaged_preparation = unmanaged_window
            .update(cx, |_, window, app| {
                unmanaged_runtime
                    .prepare_rendered_viewport_host_scene_draft(
                        DockViewportHostSceneDraft::new(
                            space.clone(),
                            unmanaged_window_id,
                            DockViewportWindowFacts::from_window(window, app).current_bounds,
                            host_bounds,
                            point(px(24.0), px(24.0)),
                            crate::DockDropGuideMetrics::default(),
                        ),
                        None,
                        DockViewportRuntimeWorkContext::new(
                            DockViewportRuntimeLineage::Unmanaged,
                            None,
                        ),
                        window,
                        app,
                    )
                    .expect("an unmanaged runtime should prepare its first rendered scene")
            })
            .expect("the unmanaged test window should remain live");
        let unmanaged_commit =
            unmanaged_runtime.finalize_rendered_viewport_host_scene_draft(unmanaged_preparation);
        assert!(
            unmanaged_commit.frame.is_some(),
            "the explicitly unmanaged low-level runtime keeps first-render registration"
        );
        assert!(
            unmanaged_runtime
                .registration_key_for_space_window(&space, unmanaged_window_id)
                .is_some()
        );
    }

    #[open_gpui::test]
    fn finalized_scene_commit_cannot_publish_after_registration_generation_switch(
        cx: &mut TestAppContext,
    ) {
        let space = DockSpaceId::from("main");
        let fixture = DockViewportRuntimeFixture::builder(space.clone())
            .space(space.clone(), ["a"])
            .allow_platform_viewports(true)
            .build(cx);
        let opened = fixture.open_unfocused_viewport(cx, &space);
        let window = opened.window();
        let window_id = window.window_id();
        let runtime = fixture.runtime.clone();
        let host_bounds = bounds(0.0, 0.0, 480.0, 320.0);
        let expected_registration = runtime
            .registration_key_for_space_window(&space, window_id)
            .expect("the opened viewport should have a registration");
        let preparation = window
            .update(cx, |_, window, app| {
                runtime
                    .prepare_rendered_viewport_host_scene_draft(
                        DockViewportHostSceneDraft::new(
                            space.clone(),
                            window_id,
                            DockViewportWindowFacts::from_window(window, app).current_bounds,
                            host_bounds,
                            point(px(12.0), px(12.0)),
                            crate::DockDropGuideMetrics::default(),
                        ),
                        Some(&expected_registration),
                        DockViewportRuntimeWorkContext::new(
                            DockViewportRuntimeLineage::Unmanaged,
                            None,
                        ),
                        window,
                        app,
                    )
                    .expect("the current registration should prepare")
            })
            .expect("the viewport window should remain live");
        let commit = runtime.finalize_rendered_viewport_host_scene_draft(preparation);
        assert!(commit.frame.is_some(), "the current scene should finalize");

        let replacement = runtime
            .runtime
            .borrow_mut()
            .replace_adapter_registration_for_test(space.clone(), window);
        assert_ne!(expected_registration, replacement);
        let published =
            cx.update(|app| runtime.publish_rendered_viewport_host_scene_commit(commit, app));

        assert!(
            !published,
            "a finalized G1 scene must not publish after the adapter advances to G2"
        );
        assert_eq!(
            runtime.registration_key_for_space_window(&space, window_id),
            Some(replacement)
        );
    }

    #[open_gpui::test]
    fn stale_scene_preparation_cannot_cross_same_window_registration_generation(
        cx: &mut TestAppContext,
    ) {
        let space = DockSpaceId::from("main");
        let fixture = DockViewportRuntimeFixture::builder(space.clone())
            .space(space.clone(), ["a"])
            .allow_platform_viewports(true)
            .build(cx);
        let opened = fixture.open_unfocused_viewport(cx, &space);
        let window = opened.window();
        let window_id = window.window_id();
        let runtime = fixture.runtime.clone();
        let host_bounds = bounds(0.0, 0.0, 480.0, 320.0);
        let window_facts = window
            .update(cx, |_, window, app| {
                DockViewportWindowFacts::from_window(window, app)
            })
            .expect("the viewport window must remain live");
        let old_frame = runtime
            .runtime
            .borrow_mut()
            .begin_viewport_host_scene_frame(
                space.clone(),
                window_id,
                window_facts,
                host_bounds,
                point(px(12.0), px(12.0)),
                crate::DockDropGuideMetrics::default(),
            )
            .expect("the first registration must publish a scene")
            .frame;
        let old_registration = old_frame.registration_key().clone();
        let preparation = window
            .update(cx, |_, window, app| {
                runtime
                    .prepare_rendered_viewport_host_scene_draft(
                        DockViewportHostSceneDraft::new(
                            space.clone(),
                            window_id,
                            DockViewportWindowFacts::from_window(window, app).current_bounds,
                            host_bounds,
                            point(px(24.0), px(24.0)),
                            crate::DockDropGuideMetrics::default(),
                        ),
                        Some(&old_registration),
                        DockViewportRuntimeWorkContext::new(
                            DockViewportRuntimeLineage::Unmanaged,
                            None,
                        ),
                        window,
                        app,
                    )
                    .expect("the current registration must prepare")
            })
            .expect("the viewport window must remain live");

        let replacement_registration = runtime
            .runtime
            .borrow_mut()
            .replace_adapter_registration_for_test(space.clone(), window);
        assert_ne!(old_registration, replacement_registration);
        let replacement_frame = runtime
            .runtime
            .borrow_mut()
            .begin_viewport_host_scene_frame(
                space.clone(),
                window_id,
                window_facts,
                host_bounds,
                point(px(36.0), px(36.0)),
                crate::DockDropGuideMetrics::default(),
            )
            .expect("the replacement registration must publish its own scene")
            .frame;

        let stale_commit = runtime.finalize_rendered_viewport_host_scene_draft(preparation);
        assert!(
            stale_commit.frame.is_none(),
            "a G1 preparation must not be rebound to the current G2 registration"
        );
        assert!(
            !runtime.discard_rendered_viewport_host_scene_frame(
                &space,
                window_id,
                Some(&old_registration),
            ),
            "a G1 discard must not remove the G2 scene"
        );
        let mut stale_rollback_commit = DockViewportRenderedHostSceneCommit {
            changed: false,
            work_context: DockViewportRuntimeWorkContext::new(
                DockViewportRuntimeLineage::Unmanaged,
                None,
            ),
            frame: None,
            registration_update: DockViewportRuntimeUpdate::default(),
            route_preview_update: DockViewportRuntimeUpdate::default(),
        };
        assert!(
            !runtime.rollback_rendered_viewport_host_scene_frame(
                &old_frame,
                &mut stale_rollback_commit,
            ),
            "an exact G1 rollback must not remove the G2 frame"
        );
        let replacement_frame = runtime
            .push_viewport_host_scene_frame_fact(
                &replacement_frame,
                DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
                    space: space.clone(),
                    bounds: host_bounds,
                    is_central: false,
                }),
            )
            .expect("the G2 frame must remain current after every stale G1 operation");

        let mut current_rollback_commit = DockViewportRenderedHostSceneCommit {
            changed: false,
            work_context: DockViewportRuntimeWorkContext::new(
                DockViewportRuntimeLineage::Unmanaged,
                None,
            ),
            frame: None,
            registration_update: DockViewportRuntimeUpdate::default(),
            route_preview_update: DockViewportRuntimeUpdate::default(),
        };
        assert!(runtime.rollback_rendered_viewport_host_scene_frame(
            &replacement_frame,
            &mut current_rollback_commit,
        ));
        assert!(
            !runtime.viewport_route_ready(&space),
            "rolling back the current frame must demote its route facts"
        );
        assert!(
            runtime
                .push_viewport_host_scene_frame_fact(
                    &replacement_frame,
                    DockHostDropSceneFact::EmptySpace(DockEmptySpaceDropTarget {
                        space,
                        bounds: host_bounds,
                        is_central: false,
                    }),
                )
                .is_none(),
            "the exact rolled-back frame must no longer own a scene"
        );
    }
}
