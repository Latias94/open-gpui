//! Windows journeys backed by the shared managed docking session.

use std::{
    cell::RefCell,
    collections::BTreeSet,
    panic::{AssertUnwindSafe, set_hook, take_hook},
    rc::Rc,
    sync::{Arc, Mutex},
};

use anyhow::{Context as _, Result, bail, ensure};
use futures::FutureExt as _;
use open_gpui::{AppContext as _, Application, AsyncApp, QuitMode};
use open_gpui_windows::{
    NativeWindowTestEventKind, NativeWindowTestMessage, NativeWindowTestObservation,
    WindowsPlatform, begin_native_window_test_observation, native_test_non_shell_root_window_at,
    native_test_window_rect,
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT, WPARAM},
    UI::{
        Input::KeyboardAndMouse::GetCapture,
        WindowsAndMessaging::{
            GetCursorPos, HTTRANSPARENT, IsWindow, IsWindowVisible, PostMessageW, SendMessageW,
            WM_CLOSE, WM_NCHITTEST,
        },
    },
};

use super::process_harness::{
    NativeWorkerAppCensus, NativeWorkerExitPath, NativeWorkerOutcome, NativeWorkerSubcase,
    finish_worker_report,
};
use super::shared_session::{
    SHARED_MIDDLE_ITEM, SHARED_MIDDLE_SECONDARY_ITEM, SHARED_MIDDLE_SURFACE, SHARED_SOURCE_ITEM,
    SHARED_SOURCE_SECONDARY_ITEM, SHARED_SOURCE_SURFACE, SHARED_TARGET_ITEM,
    SHARED_TARGET_SECONDARY_ITEM, SHARED_TARGET_SURFACE, SharedFixtureShape, SharedSession,
    SharedWindow, shared_item_selected, shared_surface_contains_item, three_window_bounds,
    two_window_bounds,
};
use super::{
    INPUT_CANARY, NATIVE_SCENARIO_ENV, NATIVE_TEST_INPUT_CANARY, NativeDockBehavior,
    NativeNoInputOverlay, NativeOpaqueBarrier, NativeScenarioRegistration, NativeWorkerCompletion,
    capture_worker_app_census_without_surface, close_no_input_overlay, native_generation_label,
    native_system_mouse_message_observed, native_test_display_at, native_test_displays,
    open_desktop_release_point, open_desktop_release_point_away_from,
    open_desktop_release_point_away_from_all, open_desktop_release_point_on_display,
    open_no_input_overlay, panic_message, panic_payload_message, point_is_open_desktop,
    process_harness, raise_native_window, raw_hwnd, rect_contains, screen_point_lparam,
    shutdown_native_worker_application_without_surface, unique_matching_debug_bounds, wait_until,
    window_has_non_empty_frame, window_scale_factor,
};

const SHARED_SCENARIO_ENV: &str = "OPEN_GPUI_NATIVE_SHARED_SCENARIO";

pub(super) fn run_worker(registration: NativeScenarioRegistration) -> ! {
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let before_application = process_harness::capture_process_window_census()
        .expect("shared interactive worker must capture its pre-application HWND census");
    let panics = Arc::new(Mutex::new(Vec::<String>::new()));
    let panics_for_hook = Arc::clone(&panics);
    let previous_hook = take_hook();
    set_hook(Box::new(move |panic| {
        if let Ok(mut panics) = panics_for_hook.lock() {
            panics.push(panic_payload_message(panic.payload()));
        }
        previous_hook(panic);
    }));
    let completion = Rc::new(RefCell::new(None::<NativeWorkerCompletion>));
    let behavior = registration.behavior;
    let scenario_id = registration.id.clone();
    let worker_subcase = process_harness::worker_subcase(&registration.id, behavior);
    let quit_mode = if behavior == NativeDockBehavior::ProcessConvergence
        && worker_subcase.uses_last_window_policy()
    {
        QuitMode::LastWindowClosed
    } else {
        QuitMode::Explicit
    };
    let worker_observation = observation.clone();
    let completion_for_application = completion.clone();
    match WindowsPlatform::new_returning_for_test(false) {
        Ok(platform) => {
            let mut application =
                Application::with_platform(Rc::new(platform)).with_quit_mode(quit_mode);
            application.run_returning_for_test(move |cx| {
                let completion = completion_for_application.clone();
                let scenario_id = scenario_id.clone();
                let observation = worker_observation.clone();
                cx.spawn(async move |cx| {
                    let mut worker = SharedScenarioWorker::new(observation);
                    let result = AssertUnwindSafe(worker.run(behavior, worker_subcase, cx))
                        .catch_unwind()
                        .await
                        .map_err(panic_message)
                        .and_then(|result| result.map_err(|error| error.to_string()));
                    let mut failures = Vec::new();
                    if let Err(error) = result {
                        failures.push(error);
                    }
                    let cleanup = AssertUnwindSafe(worker.shutdown(cx))
                        .catch_unwind()
                        .await
                        .map_err(panic_message)
                        .and_then(|result| result.map_err(|error| error.to_string()));
                    if let Err(error) = cleanup {
                        failures.push(format!("shared interactive cleanup failed: {error}"));
                    }
                    let app_shutdown = if worker.should_skip_app_shutdown() {
                        Ok(())
                    } else {
                        AssertUnwindSafe(shutdown_native_worker_application_without_surface(cx))
                            .catch_unwind()
                            .await
                            .map_err(panic_message)
                            .and_then(|result| result.map_err(|error| error.to_string()))
                    };
                    if let Err(error) = app_shutdown {
                        failures.push(format!("shared interactive app shutdown failed: {error}"));
                    }
                    let app_census = worker.census(cx);
                    if app_census.window_registry_count != 0
                        || app_census.active_drag
                        || !app_census.native_exit_authority_settled
                    {
                        failures.push(format!(
                            "shared interactive cleanup did not converge: {app_census:?}"
                        ));
                    }
                    let outcome = if failures.is_empty() {
                        worker.milestones.push("scenario.completed".to_owned());
                        NativeWorkerOutcome::Passed
                    } else {
                        NativeWorkerOutcome::Failed(failures.join("; "))
                    };
                    let skip_app_shutdown = worker.should_skip_app_shutdown();
                    let exit_path = worker.exit_path;
                    *completion.borrow_mut() = Some(NativeWorkerCompletion {
                        outcome,
                        milestones: worker.milestones,
                        exit_path,
                        app_census,
                    });
                    if !skip_app_shutdown {
                        cx.update(|app| app.quit());
                    }
                })
                .detach();
            });
        }
        Err(error) => {
            *completion.borrow_mut() = Some(NativeWorkerCompletion {
                outcome: NativeWorkerOutcome::Failed(format!(
                    "shared interactive worker Windows platform initialization failed: {error:#}"
                )),
                milestones: Vec::new(),
                exit_path: NativeWorkerExitPath::ExplicitCleanup,
                app_census: NativeWorkerAppCensus::default(),
            });
        }
    }
    let mut completion = completion
        .borrow_mut()
        .take()
        .unwrap_or_else(|| NativeWorkerCompletion {
            outcome: NativeWorkerOutcome::Failed(
                "shared interactive worker returned without completion".into(),
            ),
            milestones: Vec::new(),
            exit_path: NativeWorkerExitPath::ExplicitCleanup,
            app_census: NativeWorkerAppCensus::default(),
        });
    let observed_panics = panics
        .lock()
        .map(|panics| panics.clone())
        .unwrap_or_else(|_| {
            vec!["shared interactive panic observation lock was poisoned".to_owned()]
        });
    if !observed_panics.is_empty() {
        completion.outcome = NativeWorkerOutcome::Failed(format!(
            "shared interactive worker observed panics: {observed_panics:?}"
        ));
    }
    let events = observation.events();
    let observed = events
        .iter()
        .map(|event| native_generation_label(event.window()))
        .collect::<BTreeSet<_>>();
    let terminal = events
        .iter()
        .filter(|event| {
            matches!(
                event.kind(),
                NativeWindowTestEventKind::NativeTerminal { .. }
            )
        })
        .map(|event| native_generation_label(event.window()))
        .collect::<BTreeSet<_>>();
    let unterminated = observed.difference(&terminal).cloned().collect();
    let mixed_dpi_pair = process_harness::worker_mixed_dpi_pair_plan(registration.behavior)
        .unwrap_or_else(|error| {
            panic!("shared worker received an invalid mixed-DPI plan: {error:#}")
        });
    let report = finish_worker_report(
        &registration.id,
        process_harness::worker_subcase(&registration.id, registration.behavior),
        completion.outcome,
        completion.milestones,
        completion.exit_path,
        before_application,
        completion.app_census,
        observed.len(),
        terminal.len(),
        unterminated,
        mixed_dpi_pair,
    )
    .unwrap_or_else(|error| {
        eprintln!("shared interactive worker could not publish its census: {error:#}");
        std::process::exit(2);
    });
    process_harness::publish_worker_report_and_wait_for_release(&report);
    std::process::exit(0);
}

struct SharedScenarioWorker {
    observation: NativeWindowTestObservation,
    session: Option<SharedSession>,
    survivor: Option<SharedSession>,
    overlays: Vec<NativeNoInputOverlay>,
    milestones: Vec<String>,
    exit_path: NativeWorkerExitPath,
}

impl SharedScenarioWorker {
    fn new(observation: NativeWindowTestObservation) -> Self {
        Self {
            observation,
            session: None,
            survivor: None,
            overlays: Vec::new(),
            milestones: Vec::new(),
            exit_path: NativeWorkerExitPath::ExplicitCleanup,
        }
    }

    fn should_skip_app_shutdown(&self) -> bool {
        !matches!(self.exit_path, NativeWorkerExitPath::ExplicitCleanup)
    }

    async fn run(
        &mut self,
        behavior: NativeDockBehavior,
        worker_subcase: NativeWorkerSubcase,
        cx: &mut AsyncApp,
    ) -> Result<()> {
        match behavior {
            NativeDockBehavior::SourceCapture => self.run_source_capture(cx).await,
            NativeDockBehavior::ComplexDockspaceWorkflow => self.run_complex_workflow(cx).await,
            NativeDockBehavior::OpaqueOcclusion
            | NativeDockBehavior::ProvisionalSameHwndPromotion
            | NativeDockBehavior::LiveRouteAndReleaseLock
            | NativeDockBehavior::MixedDpiPlacement => {
                self.run_provisional(cx, behavior, worker_subcase).await
            }
            NativeDockBehavior::NoInputPassThrough => self.run_no_input(cx).await,
            NativeDockBehavior::ProcessConvergence => {
                self.run_process_convergence(cx, worker_subcase).await
            }
            NativeDockBehavior::CommittedLossRecovery => self.run_committed_loss(cx).await,
            NativeDockBehavior::SurfaceShutdown | NativeDockBehavior::TabDragLatencyBenchmark => {
                bail!("shared interactive worker received unsupported behavior {behavior:?}")
            }
        }
    }

    async fn open_session(
        &mut self,
        cx: &mut AsyncApp,
        shape: SharedFixtureShape,
    ) -> Result<&mut SharedSession> {
        let bounds = cx.update(|app| match shape {
            SharedFixtureShape::TwoWindows => two_window_bounds(app),
            SharedFixtureShape::ThreeWindows => three_window_bounds(app),
            SharedFixtureShape::SurvivorWindow => {
                bail!("survivor fixture requires an explicit placement bound")
            }
        })?;
        self.open_session_with_bounds(cx, shape, bounds).await
    }

    async fn open_session_with_bounds(
        &mut self,
        cx: &mut AsyncApp,
        shape: SharedFixtureShape,
        bounds: Vec<open_gpui::Bounds<open_gpui::Pixels>>,
    ) -> Result<&mut SharedSession> {
        let observation = self.observation.clone();
        let session = cx.update(|app| SharedSession::build(app, observation, shape, &bounds))?;
        self.session = Some(session);
        let session = self
            .session
            .as_mut()
            .expect("shared session just installed");
        session.wait_ready(cx).await?;
        Ok(session)
    }

    async fn run_source_capture(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let session = self
            .open_session(cx, SharedFixtureShape::TwoWindows)
            .await?;
        let observation = session.observation.clone();
        let source = session.source_window()?;
        let target = session.target_window()?;
        let drag = session.begin_captured_drag(cx, source, source.item).await?;
        let target_point = tab_screen_point(cx, target, target.item).await?;
        let delivered = session
            .move_captured_pointer(cx, target_point, "shared target route")
            .await?;
        wait_until(cx, "shared target gesture route", |cx| {
            Ok(cx.update(|app| app.has_active_drag())
                && cx.update(|app| session.dock.has_active_gesture())
                && unsafe { GetCapture() } == source.hwnd
                && native_system_mouse_message_observed(
                    &observation,
                    source.window,
                    NativeWindowTestMessage::MouseMove,
                    delivered,
                ))
        })
        .await?;
        session.release_captured_pointer(target_point)?;
        session.wait_drag_released(cx).await?;
        ensure!(
            shared_surface_contains_item(session, source.item, target.surface),
            "shared source item did not commit into the target surface"
        );
        ensure!(
            shared_item_selected(session, source.item),
            "shared target route did not select the moved item"
        );
        session.assert_release_terminal(source, drag.generation)?;
        session
            .milestones
            .push("shared.source-capture-committed".to_owned());
        Ok(())
    }

    async fn run_complex_workflow(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let session = self
            .open_session(cx, SharedFixtureShape::ThreeWindows)
            .await?;
        let observation = session.observation.clone();
        let steps = [
            (
                SHARED_SOURCE_SURFACE,
                SHARED_SOURCE_ITEM,
                SHARED_TARGET_SURFACE,
                SHARED_TARGET_ITEM,
            ),
            (
                SHARED_MIDDLE_SURFACE,
                SHARED_MIDDLE_ITEM,
                SHARED_TARGET_SURFACE,
                SHARED_TARGET_SECONDARY_ITEM,
            ),
            (
                SHARED_TARGET_SURFACE,
                SHARED_SOURCE_ITEM,
                SHARED_MIDDLE_SURFACE,
                SHARED_MIDDLE_SECONDARY_ITEM,
            ),
            (
                SHARED_TARGET_SURFACE,
                SHARED_MIDDLE_ITEM,
                SHARED_SOURCE_SURFACE,
                SHARED_SOURCE_SECONDARY_ITEM,
            ),
        ];
        for (index, (source_surface, item, target_surface, target_item)) in
            steps.into_iter().enumerate()
        {
            let source = session.window(source_surface)?;
            let target = session.window(target_surface)?;
            ensure!(
                source.hwnd != target.hwnd,
                "workflow step {index} crossed no HWND boundary"
            );
            let source_point = tab_screen_point(cx, source, item).await?;
            let drag = session.begin_captured_drag(cx, source, item).await?;
            let target_point = tab_screen_point(cx, target, target_item).await?;
            let delivered = session
                .move_captured_pointer(cx, target_point, "shared workflow target route")
                .await?;
            wait_until(cx, "shared workflow active target route", |cx| {
                Ok(cx.update(|app| app.has_active_drag())
                    && cx.update(|app| session.dock.has_active_gesture())
                    && native_system_mouse_message_observed(
                        &observation,
                        source.window,
                        NativeWindowTestMessage::MouseMove,
                        delivered,
                    ))
            })
            .await?;
            session.release_captured_pointer(target_point)?;
            session.wait_drag_released(cx).await?;
            ensure!(
                shared_surface_contains_item(session, item, target_surface),
                "workflow step {index} did not move item {item:?} into {target_surface:?}"
            );
            ensure!(
                shared_item_selected(session, item),
                "workflow step {index} did not select moved item"
            );
            session.assert_release_terminal(source, drag.generation)?;
            let _ = source_point;
            session
                .milestones
                .push(format!("shared.workflow.step-{index}-committed"));
        }
        ensure!(
            session
                .windows
                .iter()
                .all(|window| unsafe { IsWindow(Some(window.hwnd)).as_bool() }),
            "shared workflow lost one of its original HWNDs"
        );
        Ok(())
    }

    async fn run_provisional(
        &mut self,
        cx: &mut AsyncApp,
        behavior: NativeDockBehavior,
        worker_subcase: NativeWorkerSubcase,
    ) -> Result<()> {
        let mixed_dpi_target = if behavior == NativeDockBehavior::MixedDpiPlacement {
            let plan = process_harness::worker_mixed_dpi_pair_plan(behavior)?
                .context("mixed-DPI worker lost its frozen display pair")?;
            Some(super::select_native_mixed_dpi_display_pair(
                worker_subcase,
                plan,
            )?)
        } else {
            None
        };
        let bounds = if let Some(selected) = mixed_dpi_target {
            cx.update(|app| {
                super::shared_session::two_window_bounds_on_display(app, selected.source)
            })?
        } else {
            cx.update(two_window_bounds)?
        };
        let session = self
            .open_session_with_bounds(cx, SharedFixtureShape::TwoWindows, bounds)
            .await?;
        let observation = session.observation.clone();
        let source = session.source_window()?;
        let target = session.target_window()?;
        let drag = session.begin_captured_drag(cx, source, source.item).await?;
        let reveal_point = open_desktop_release_point(source.hwnd, target.hwnd)?;
        session
            .move_captured_pointer(cx, reveal_point, "shared provisional reveal")
            .await?;
        let provisional = wait_for_new_shared_window(cx, session, source, target).await?;
        ensure!(
            unsafe { IsWindowVisible(provisional.hwnd).as_bool() },
            "shared provisional HWND is hidden"
        );
        ensure!(
            unsafe { GetCapture() } == source.hwnd,
            "shared provisional reveal lost source capture"
        );
        let provisional_rect = native_test_window_rect(provisional.hwnd)?;
        let hit_point = POINT {
            x: (provisional_rect.left + provisional_rect.right) / 2,
            y: (provisional_rect.top + provisional_rect.bottom) / 2,
        };
        let hit = unsafe {
            SendMessageW(
                provisional.hwnd,
                WM_NCHITTEST,
                Some(WPARAM::default()),
                Some(screen_point_lparam(hit_point)),
            )
        };
        ensure!(
            hit.0 == HTTRANSPARENT as isize,
            "shared provisional must remain hit-transparent before release"
        );
        let mut barrier = None;
        let mut route_points = Vec::new();
        let release_point = if let Some(selected) = mixed_dpi_target {
            open_desktop_release_point_on_display(
                source.hwnd,
                target.hwnd,
                selected.target,
                Some(reveal_point),
                None,
            )?
        } else if behavior == NativeDockBehavior::OpaqueOcclusion {
            let point =
                open_desktop_release_point_away_from(source.hwnd, target.hwnd, reveal_point)?;
            let opaque = NativeOpaqueBarrier::prepare_partial_cover(point)?;
            opaque.present()?;
            ensure!(
                native_test_non_shell_root_window_at(point) == Some(opaque.hwnd()),
                "opaque barrier is not the hit target"
            );
            barrier = Some(opaque);
            point
        } else if behavior == NativeDockBehavior::LiveRouteAndReleaseLock {
            let second =
                open_desktop_release_point_away_from(source.hwnd, target.hwnd, reveal_point)?;
            let third = open_desktop_release_point_away_from_all(
                source.hwnd,
                target.hwnd,
                &[reveal_point, second],
            )?;
            route_points.extend([second, third]);
            third
        } else {
            reveal_point
        };
        for route_point in route_points.iter().copied() {
            let delivered = session
                .move_captured_pointer(cx, route_point, "shared provisional live route")
                .await?;
            ensure!(
                native_system_mouse_message_observed(
                    &observation,
                    source.window,
                    NativeWindowTestMessage::MouseMove,
                    delivered,
                ),
                "shared live route point was not observed by the original source WndProc"
            );
        }
        let delivered_release = session
            .move_captured_pointer(cx, release_point, "shared provisional release")
            .await?;
        session.release_captured_pointer(delivered_release)?;
        session.wait_drag_released(cx).await?;
        wait_until(cx, "shared provisional same-HWND promotion", |cx| {
            Ok(
                cx.update(|app| app.windows().len()) == session.initial_window_count + 1
                    && shared_surface_contains_item(session, source.item, source.surface) == false
                    && unsafe { IsWindow(Some(provisional.hwnd)).as_bool() },
            )
        })
        .await?;
        let promoted = cx.update(|app| raw_hwnd(app, provisional.window))?;
        ensure!(
            promoted == provisional.hwnd,
            "shared provisional did not preserve its HWND"
        );
        let promoted_rect = native_test_window_rect(promoted)?;
        ensure!(
            promoted_rect != provisional_rect,
            "shared promotion did not update native placement"
        );
        let promoted_hit = unsafe {
            SendMessageW(
                promoted,
                WM_NCHITTEST,
                Some(WPARAM::default()),
                Some(screen_point_lparam(hit_point)),
            )
        };
        ensure!(
            promoted_hit.0 != HTTRANSPARENT as isize,
            "shared promotion retained the provisional hit gate"
        );
        session.assert_release_terminal(source, drag.generation)?;
        if let Some(barrier) = barrier {
            drop(barrier);
        }
        session
            .milestones
            .push(format!("shared.{}.committed", behavior.as_str()));
        Ok(())
    }

    async fn run_no_input(&mut self, cx: &mut AsyncApp) -> Result<()> {
        self.open_session(cx, SharedFixtureShape::TwoWindows)
            .await?;
        let target = self
            .session
            .as_ref()
            .context("shared no-input session disappeared")?
            .target_window()?;
        let bounds =
            cx.update(|app| app.update_window(target.window, |_, window, _| window.bounds()))?;
        let first =
            cx.update(|app| open_no_input_overlay(app, bounds, "shared no-input prefix 1"))?;
        let second =
            cx.update(|app| open_no_input_overlay(app, bounds, "shared no-input prefix 2"))?;
        let first_hwnd = first.hwnd;
        let second_hwnd = second.hwnd;
        self.overlays = vec![first, second];
        raise_native_window(first_hwnd)?;
        raise_native_window(second_hwnd)?;
        ensure!(
            native_test_non_shell_root_window_at(target_point(cx, target, target.item).await?)
                == Some(target.hwnd),
            "shared no-input prefix did not pass through to target"
        );
        for overlay in &self.overlays {
            let point = target_point(cx, target, target.item).await?;
            let hit = unsafe {
                SendMessageW(
                    overlay.hwnd,
                    WM_NCHITTEST,
                    Some(WPARAM::default()),
                    Some(screen_point_lparam(point)),
                )
            };
            ensure!(
                hit.0 == HTTRANSPARENT as isize,
                "shared no-input overlay returned a non-transparent hit"
            );
        }
        let session = self
            .session
            .as_mut()
            .context("shared no-input session disappeared")?;
        let source = session.source_window()?;
        let drag = session.begin_captured_drag(cx, source, source.item).await?;
        let pass_through = target_point(cx, target, target.item).await?;
        let delivered = session
            .move_captured_pointer(cx, pass_through, "shared no-input pass-through")
            .await?;
        ensure!(
            native_test_non_shell_root_window_at(delivered) == Some(target.hwnd),
            "shared pass-through target changed"
        );
        session.release_captured_pointer(delivered)?;
        session.wait_drag_released(cx).await?;
        session.assert_release_terminal(source, drag.generation)?;
        for overlay in self.overlays.drain(..) {
            cx.update(|app| close_no_input_overlay(app, overlay))?;
        }
        Ok(())
    }

    async fn run_process_convergence(
        &mut self,
        cx: &mut AsyncApp,
        worker_subcase: NativeWorkerSubcase,
    ) -> Result<()> {
        let bounds = cx.update(three_window_bounds)?;
        let observation = self.observation.clone();
        let primary = cx.update(|app| {
            SharedSession::build(
                app,
                observation.clone(),
                SharedFixtureShape::TwoWindows,
                &bounds[..2],
            )
        })?;
        let survivor = cx.update(|app| {
            SharedSession::build(
                app,
                observation,
                SharedFixtureShape::SurvivorWindow,
                &bounds[2..],
            )
        })?;
        self.session = Some(primary);
        self.survivor = Some(survivor);
        self.session
            .as_mut()
            .context("shared primary session disappeared")?
            .wait_ready(cx)
            .await?;
        self.survivor
            .as_mut()
            .context("shared survivor session disappeared")?
            .wait_ready(cx)
            .await?;
        let primary = self
            .session
            .as_mut()
            .context("shared primary session disappeared")?;
        let survivor = self
            .survivor
            .as_mut()
            .context("shared survivor session disappeared")?;
        let source = primary.source_window()?;
        let target = primary.target_window()?;
        let survivor_window = survivor.windows[0];
        let drag = primary.begin_captured_drag(cx, source, source.item).await?;
        unsafe {
            PostMessageW(
                Some(target.hwnd),
                WM_CLOSE,
                WPARAM::default(),
                LPARAM::default(),
            )
        }
        .context("shared process convergence could not post WM_CLOSE")?;
        primary.wait_drag_released(cx).await?;
        wait_until(
            cx,
            "shared primary surface to close while survivor remains live",
            |cx| {
                primary.pump(cx);
                survivor.pump(cx);
                primary.check_health()?;
                survivor.check_health()?;
                Ok(primary.dock.native_is_quiescent()
                    && cx.update(|app| app.windows().len() == 1)
                    && unsafe { IsWindow(Some(survivor_window.hwnd)).as_bool() }
                    && cx.update(|app| window_has_non_empty_frame(app, survivor_window.window))?)
            },
        )
        .await?;
        primary.assert_release_terminal(source, drag.generation)?;
        if let Some(mut pointer) = primary.pointer.take() {
            ensure!(
                pointer.release_primary_button_best_effort(),
                "shared process convergence could not restore the physical primary button"
            );
        }
        if let (Some(source_identity), Some(anchor_identity)) = (source.identity, target.identity) {
            let events = self.observation.events();
            let source_lifecycle = super::native_shutdown_ordinals(&events, source_identity)?;
            let anchor_lifecycle = super::native_shutdown_ordinals(&events, anchor_identity)?;
            super::assert_native_shutdown_order("shared process dependent", &source_lifecycle)?;
            super::assert_native_shutdown_order("shared process anchor", &anchor_lifecycle)?;
            ensure!(
                source_lifecycle.non_client_destroy < anchor_lifecycle.destroy_entered,
                "shared process anchor destruction preceded its dependent"
            );
        }
        if worker_subcase.uses_last_window_policy() {
            unsafe {
                PostMessageW(
                    Some(survivor_window.hwnd),
                    WM_CLOSE,
                    WPARAM::default(),
                    LPARAM::default(),
                )
            }
            .context("shared process convergence could not close survivor")?;
            wait_until(cx, "shared last-window policy to return", |cx| {
                survivor.pump(cx);
                Ok(cx.update(|app| app.windows().is_empty()))
            })
            .await?;
            self.exit_path = NativeWorkerExitPath::LastWindowClosed;
        } else {
            cx.update(|app| app.shutdown_and_quit_for_native_exit_test());
            wait_until(cx, "shared App shutdown fanout", |cx| {
                primary.pump(cx);
                survivor.pump(cx);
                Ok(primary.dock.native_is_quiescent()
                    && survivor.dock.native_is_quiescent()
                    && cx.update(|app| app.windows().is_empty()))
            })
            .await?;
            self.exit_path = NativeWorkerExitPath::AppShutdown;
        }
        Ok(())
    }

    async fn run_committed_loss(&mut self, cx: &mut AsyncApp) -> Result<()> {
        // Shared core exposes dock-back/source-unavailable as the recovery contract. The
        // interactive journey proves that the retained item remains recoverable after a child
        // window is destroyed, without importing the legacy Restore registry.
        let session = self
            .open_session(cx, SharedFixtureShape::TwoWindows)
            .await?;
        let source = session.source_window()?;
        let target = session.target_window()?;
        let drag = session.begin_captured_drag(cx, source, source.item).await?;
        let release = open_desktop_release_point(source.hwnd, target.hwnd)?;
        session
            .move_captured_pointer(cx, release, "shared recovery tear-off")
            .await?;
        session.release_captured_pointer(release)?;
        session.wait_drag_released(cx).await?;
        let provisional = wait_for_new_shared_window(cx, session, source, target).await?;
        unsafe {
            PostMessageW(
                Some(provisional.hwnd),
                WM_CLOSE,
                WPARAM::default(),
                LPARAM::default(),
            )
        }
        .context("shared recovery could not close the promoted HWND")?;
        wait_until(cx, "shared recovery child terminal", |cx| {
            session.pump(cx);
            session.check_health()?;
            Ok(!unsafe { IsWindow(Some(provisional.hwnd)).as_bool() })
        })
        .await?;
        let promoted_root = session
            .dock
            .with_view(|view| view.item(source.item).map(|item| item.root()))
            .context("shared recovery tear-off did not retain a promoted root")?;
        cx.update(|app| {
            session
                .dock
                .submit_native_dock_back_for_test(promoted_root, app)
        })?;
        wait_until(cx, "shared dock-back recovery commit", |cx| {
            session.pump(cx);
            session.check_health()?;
            Ok(
                shared_surface_contains_item(session, source.item, source.surface)
                    && shared_item_selected(session, source.item),
            )
        })
        .await?;
        ensure!(
            session
                .dock
                .with_view(|view| view.item(source.item).is_some()),
            "shared recovery lost the retained item after child loss"
        );
        session.assert_release_terminal(source, drag.generation)?;
        session
            .milestones
            .push("shared.committed-loss-dock-back".to_owned());
        Ok(())
    }

    async fn shutdown(&mut self, cx: &mut AsyncApp) -> Result<()> {
        for overlay in self.overlays.drain(..) {
            let _ = cx.update(|app| close_no_input_overlay(app, overlay));
        }
        if self.should_skip_app_shutdown() {
            return Ok(());
        }
        if let Some(session) = self.session.as_mut() {
            session.shutdown(cx).await?;
        }
        if let Some(survivor) = self.survivor.as_mut() {
            survivor.shutdown(cx).await?;
        }
        Ok(())
    }

    fn census(&self, cx: &mut AsyncApp) -> NativeWorkerAppCensus {
        if let Some(session) = &self.session {
            session.capture_census(cx)
        } else {
            capture_worker_app_census_without_surface(cx)
        }
    }
}

async fn tab_screen_point(
    cx: &mut AsyncApp,
    window: SharedWindow,
    item: open_gpui_docking::dockspace::model::ItemId,
) -> Result<POINT> {
    let bounds = cx.update(|app| {
        unique_matching_debug_bounds(
            app,
            window.window,
            "dockspace:tab:",
            &item.get().to_string(),
        )
    })?;
    let scale = cx.update(|app| window_scale_factor(app, window.window))?;
    open_gpui_windows::native_test_logical_client_point_to_screen(
        window.hwnd,
        bounds.center(),
        scale,
    )
}

async fn target_point(
    cx: &mut AsyncApp,
    window: SharedWindow,
    item: open_gpui_docking::dockspace::model::ItemId,
) -> Result<POINT> {
    tab_screen_point(cx, window, item).await
}

async fn wait_for_new_shared_window(
    cx: &mut AsyncApp,
    session: &mut SharedSession,
    source: SharedWindow,
    target: SharedWindow,
) -> Result<SharedWindow> {
    let mut candidate = None;
    wait_until(cx, "shared provisional native window", |cx| {
        let windows = cx.update(|app| app.windows());
        candidate = windows
            .into_iter()
            .find(|window| *window != source.window && *window != target.window);
        let Some(window) = candidate else {
            return Ok(false);
        };
        let hwnd = cx.update(|app| raw_hwnd(app, window))?;
        Ok(unsafe { IsWindow(Some(hwnd)).as_bool() }
            && unsafe { IsWindowVisible(hwnd).as_bool() }
            && cx.update(|app| window_has_non_empty_frame(app, window))?)
    })
    .await?;
    let window = candidate.context("shared provisional window disappeared")?;
    let hwnd = cx.update(|app| raw_hwnd(app, window))?;
    let identity = super::unique_native_identity_for_window(
        &session.observation.events(),
        window.window_id(),
        "shared provisional window",
    )?;
    Ok(SharedWindow {
        surface: SHARED_TARGET_SURFACE,
        item: SHARED_SOURCE_ITEM,
        window,
        hwnd,
        identity: Some(identity),
    })
}
