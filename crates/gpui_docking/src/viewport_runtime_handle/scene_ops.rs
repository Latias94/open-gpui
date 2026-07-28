use super::*;
use crate::DockViewportHostGeometry;

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
        window: &mut Window,
        cx: &mut App,
    ) -> Option<DockViewportRenderedHostScenePreparation> {
        let space = draft.space.clone();
        let window_id = window.window_handle().window_id();
        if draft.window_id != window_id
            || self
                .registration_key_for_space_window(&space, window_id)
                .as_ref()
                != expected_registration
        {
            return None;
        }

        let backend_focus_changed = self.reconcile_backend_window_focus(cx);
        let viewport_frame_changed = self.reconcile_viewport_frame_except_window(window_id, cx);
        let window_facts = DockViewportWindowFacts::from_window(window, cx);
        Some(DockViewportRenderedHostScenePreparation {
            changed: backend_focus_changed || viewport_frame_changed,
            draft,
            expected_registration: expected_registration.cloned(),
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
            window,
            window_facts,
        } = preparation;
        let space = draft.space.clone();
        let window_id = draft.window_id;
        let mut runtime = self.runtime.borrow_mut();
        if runtime
            .registration_key_for_space_window(&space, window_id)
            .as_ref()
            != expected_registration.as_ref()
        {
            return DockViewportRenderedHostSceneCommit {
                changed,
                frame: None,
                registration_update: DockViewportRuntimeUpdate::default(),
                route_preview_update: DockViewportRuntimeUpdate::default(),
            };
        }

        let registration_update =
            runtime.register_rendered_host_viewport_with_cleanup(space.clone(), window);
        let registration = runtime
            .registration_key_for_space_window(&space, window_id)
            .and_then(|registration_key| draft.bind(registration_key))
            .and_then(|snapshot| {
                runtime.commit_viewport_host_scene_snapshot(snapshot, window_facts)
            });
        let route_preview_update = runtime.clear_preview_for_unready_window_route(window_id);
        let (host_scene_changed, frame) = registration
            .map(|registration| (registration.changed, Some(registration.frame)))
            .unwrap_or((false, None));
        DockViewportRenderedHostSceneCommit {
            changed: changed || host_scene_changed,
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
        let current_window = Some(window.window_handle().window_id());
        self.publish_surface_commit(&commit.registration_update, cx);
        let registration_changed =
            refresh_runtime_update_excluding(commit.registration_update, current_window, cx);
        let route_preview_changed =
            refresh_runtime_update_excluding(commit.route_preview_update, current_window, cx);
        commit.changed || registration_changed || route_preview_changed
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

    pub(crate) fn sync_rendered_viewport_pointer_input(
        &self,
        registration: &DockViewportRegistrationKey,
        passthrough_pointer_input: bool,
        window: &mut Window,
    ) -> bool {
        let window_id = window.window_handle().window_id();
        if registration.window_id() != window_id
            || !self
                .runtime
                .borrow()
                .adapter()
                .is_current_registration(registration)
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
                | WindowMutationRequest::FocusOnAppearing(_)
                | WindowMutationRequest::FocusOnClick(_)
                | WindowMutationRequest::Alpha(_)
                | WindowMutationRequest::Topmost(_)
                | WindowMutationRequest::TaskbarVisibility(_) => None,
            });
        let pointer_input_resolution = {
            let mut runtime = self.runtime.borrow_mut();
            if !runtime.adapter().is_current_registration(registration) {
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
        if !self
            .runtime
            .borrow()
            .adapter()
            .is_current_registration(registration)
        {
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

    pub(crate) fn discard_rendered_viewport_host_scene_frame_from_window(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
        expected_registration: Option<&DockViewportRegistrationKey>,
        window: &Window,
        cx: &mut App,
    ) -> bool {
        let update = self.runtime.borrow_mut().discard_viewport_host_scene_frame(
            space,
            window_id,
            expected_registration,
        );
        refresh_runtime_update_excluding(update, Some(window.window_handle().window_id()), cx)
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
    use crate::{drop_target::DockEmptySpaceDropTarget, viewport_test_support::bounds};
    use open_gpui::{TestAppContext, point, px};

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
