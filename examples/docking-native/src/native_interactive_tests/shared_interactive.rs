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
use open_gpui::{
    App, AppContext as _, Application, AsyncApp, PlatformWindowPhysicalGeometry, QuitMode,
    WindowFrameSubmissionOutcome, WindowProvisionalPlacementOutcome,
    WindowProvisionalPlacementPurpose,
};
use open_gpui_docking::{dockspace::geometry::PhysicalRect, native::NativeReport};
use open_gpui_windows::{
    NativeTestPointerAction, NativeWindowTestEventKind, NativeWindowTestMessage,
    NativeWindowTestObservation, WindowsPlatform, begin_native_window_test_observation,
    native_test_acquire_foreground_window, native_test_non_shell_root_window_at,
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
    NativeWorkerSurfaceCensus, finish_worker_report,
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
    NativeNoInputOverlay, NativeOpaqueBarrier, NativeReleasePointAuthority,
    NativeScenarioRegistration, NativeWorkerCompletion, capture_worker_app_census_without_surface,
    close_no_input_overlay, native_generation_label, native_system_mouse_message_observed,
    native_test_display_at, native_test_displays, open_desktop_release_point,
    open_desktop_release_point_away_from, open_desktop_release_point_away_from_all,
    open_desktop_release_point_on_display, open_no_input_overlay, panic_message,
    panic_payload_message, point_is_open_desktop, process_harness, raise_native_window, raw_hwnd,
    rect_contains, screen_point_lparam, shutdown_native_worker_application_without_surface,
    unique_matching_debug_bounds, wait_until, window_has_non_empty_frame, window_scale_factor,
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
    let process_worker = Rc::new(RefCell::new(None::<SharedScenarioWorker>));
    let process_worker_for_application = process_worker.clone();
    let mut process_application = None;
    process_harness::worker_phase("platform.initializing");
    match WindowsPlatform::new_returning_for_test(false) {
        Ok(platform) => {
            process_harness::worker_phase("platform.initialized");
            let mut application =
                Application::with_platform(Rc::new(platform)).with_quit_mode(quit_mode);
            application.run_returning_for_test(move |cx| {
                process_harness::worker_phase("application.launched");
                let completion = completion_for_application.clone();
                let observation = worker_observation.clone();
                cx.spawn(async move |cx| {
                    process_harness::worker_phase("scenario.running");
                    let mut worker = SharedScenarioWorker::new(observation);
                    let result = AssertUnwindSafe(worker.run(behavior, worker_subcase, cx))
                        .catch_unwind()
                        .await
                        .map_err(panic_message)
                        .and_then(|result| result.map_err(|error| format!("{error:#}")));
                    process_harness::worker_phase("scenario.finished");
                    let mut failures = Vec::new();
                    if let Err(error) = result {
                        failures.push(error);
                    }
                    if worker.should_skip_app_shutdown() {
                        let outcome = if failures.is_empty() {
                            NativeWorkerOutcome::Passed
                        } else {
                            NativeWorkerOutcome::Failed(failures.join("; "))
                        };
                        *completion.borrow_mut() = Some(NativeWorkerCompletion {
                            outcome,
                            milestones: std::mem::take(&mut worker.milestones),
                            exit_path: worker.exit_path,
                            app_census: NativeWorkerAppCensus::default(),
                        });
                        // Keep both sessions and their probes alive through the natural loop return.
                        process_worker_for_application.replace(Some(worker));
                        return;
                    }
                    let cleanup = AssertUnwindSafe(worker.shutdown(cx))
                        .catch_unwind()
                        .await
                        .map_err(panic_message)
                        .and_then(|result| result.map_err(|error| format!("{error:#}")));
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
                            .and_then(|result| result.map_err(|error| format!("{error:#}")))
                    };
                    if let Err(error) = app_shutdown {
                        failures.push(format!("shared interactive app shutdown failed: {error}"));
                    }
                    let app_census = worker.census(cx);
                    process_harness::worker_phase("cleanup.census-ready");
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
                        cx.update(|app| app.shutdown_and_quit_for_native_exit_test());
                    }
                })
                .detach();
            });
            if behavior == NativeDockBehavior::ProcessConvergence {
                process_application = Some(application);
            }
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
    process_harness::worker_phase("application.returned");
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
    let mut failures = match &completion.outcome {
        NativeWorkerOutcome::Failed(error) => vec![error.clone()],
        NativeWorkerOutcome::Passed => Vec::new(),
    };
    if behavior == NativeDockBehavior::ProcessConvergence
        && !matches!(completion.exit_path, NativeWorkerExitPath::ExplicitCleanup)
    {
        let validation = process_worker
            .borrow_mut()
            .as_mut()
            .context("shared process convergence did not retain its sessions")
            .and_then(|worker| {
                process_application
                    .as_mut()
                    .context("shared process convergence did not retain its application")
                    .and_then(|application| {
                        application.update_for_test(|app| {
                            completion.app_census = worker.capture_process_census(app)?;
                            worker.validate_process_return(&completion.app_census)
                        })
                    })
            });
        match validation {
            Ok(()) if failures.is_empty() => {
                completion.milestones.push(match completion.exit_path {
                    NativeWorkerExitPath::LastWindowClosed => {
                        "process.last-window-policy-returned".to_owned()
                    }
                    _ => "process.app-shutdown-returned".to_owned(),
                });
            }
            Ok(()) => {}
            Err(error) => failures.push(format!(
                "shared process convergence did not converge after loop return: {error:#}"
            )),
        }
    }
    let observed_panics = panics
        .lock()
        .map(|panics| panics.clone())
        .unwrap_or_else(|_| {
            vec!["shared interactive panic observation lock was poisoned".to_owned()]
        });
    if !observed_panics.is_empty() {
        failures.push(format!(
            "shared interactive worker observed panics: {observed_panics:?}"
        ));
    }
    if !failures.is_empty() {
        completion.outcome = NativeWorkerOutcome::Failed(failures.join("; "));
    } else if !matches!(completion.exit_path, NativeWorkerExitPath::ExplicitCleanup) {
        completion.milestones.push("scenario.completed".to_owned());
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
        matches!(
            self.exit_path,
            NativeWorkerExitPath::LastWindowClosed | NativeWorkerExitPath::AppShutdown
        )
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
        self.open_session_on_display(cx, shape, bounds, None).await
    }

    async fn open_session_on_display(
        &mut self,
        cx: &mut AsyncApp,
        shape: SharedFixtureShape,
        bounds: Vec<open_gpui::Bounds<open_gpui::Pixels>>,
        display: Option<open_gpui_windows::NativeTestDisplay>,
    ) -> Result<&mut SharedSession> {
        let observation = self.observation.clone();
        process_harness::worker_phase("roots.opening");
        let session = cx.update(|app| match display {
            Some(display) => SharedSession::build_on_display(
                app,
                observation,
                shape,
                &bounds,
                display.display_id(),
            ),
            None => SharedSession::build(app, observation, shape, &bounds),
        })?;
        process_harness::worker_phase("roots.opened");
        self.session = Some(session);
        let session = self
            .session
            .as_mut()
            .expect("shared session just installed");
        session.wait_ready(cx).await?;
        if let Some(display) = display {
            cx.update(|app| session.assert_roots_on_display(app, display, &bounds))?;
        }
        process_harness::worker_phase("roots.ready");
        Ok(session)
    }

    async fn run_source_capture(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let session = self
            .open_session(cx, SharedFixtureShape::TwoWindows)
            .await?;
        let observation = session.observation.clone();
        let source = session.source_window()?;
        let target = session.target_window()?;
        super::shared_recovery::seed_source_view(cx, session).await?;
        raise_native_window(target.hwnd)?;
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
        let hit = native_test_non_shell_root_window_at(delivered);
        ensure!(
            hit == Some(target.hwnd) && unsafe { GetCapture() } == source.hwnd,
            "shared release did not retain an unobstructed target and source capture: point={delivered:?}, hit={hit:?}, target={:?}",
            target.hwnd
        );
        session.release_captured_pointer(delivered)?;
        session.wait_drag_released(cx).await?;
        wait_shared_transfer(cx, session, source.item, target).await?;
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
        super::shared_recovery::seed_source_view(cx, session).await?;
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
            raise_native_window(target.hwnd)?;
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
            .await
            .with_context(|| {
                format!(
                    "workflow step {index}, original generation={:?}, app_drag={}, capture={:?}, target_hit={:?}, target={:?}, dock={:?}, recent_drag_events={:?}",
                    drag.generation,
                    cx.update(|app| app.has_active_drag()),
                    unsafe { GetCapture() },
                    native_test_non_shell_root_window_at(delivered),
                    target.hwnd,
                    session.dock.inspection(),
                    session.drag_events.borrow().iter().rev().take(4).collect::<Vec<_>>()
                )
            })?;
            let hit = native_test_non_shell_root_window_at(delivered);
            ensure!(
                hit == Some(target.hwnd) && unsafe { GetCapture() } == source.hwnd,
                "workflow step {index} did not retain an unobstructed target and source capture: point={delivered:?}, hit={hit:?}, target={:?}",
                target.hwnd
            );
            session.release_captured_pointer(delivered)?;
            session.wait_drag_released(cx).await?;
            wait_shared_transfer(cx, session, item, target)
                .await
                .with_context(|| format!("workflow step {index} transfer"))?;
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
            .open_session_on_display(
                cx,
                SharedFixtureShape::TwoWindows,
                bounds,
                mixed_dpi_target.map(|selected| selected.source),
            )
            .await?;
        let observation = session.observation.clone();
        let source = session.source_window()?;
        let target = session.target_window()?;
        super::shared_recovery::seed_source_view(cx, session).await?;
        let roots = cx.update(|app| session.snapshot_roots(app))?;
        let drag = session.begin_captured_drag(cx, source, source.item).await?;
        let reveal_point = if let Some(selected) = mixed_dpi_target {
            open_desktop_release_point_on_display(
                source.hwnd,
                target.hwnd,
                selected.source,
                None,
                (selected.source.display_id() == selected.negative_display.display_id())
                    .then_some(selected.negative_point),
            )?
        } else {
            open_desktop_release_point(source.hwnd, target.hwnd)?
        };
        let delivered_reveal = session
            .move_captured_pointer(cx, reveal_point, "shared provisional reveal")
            .await?;
        if let Some(selected) = mixed_dpi_target {
            ensure!(
                native_test_display_at(delivered_reveal)?.display_id()
                    == selected.source.display_id(),
                "shared mixed-DPI reveal missed the selected source display: selected={selected:?}, delivered={delivered_reveal:?}"
            );
            wait_until(cx, "shared mixed-DPI source WndProc reveal", |cx| {
                session.pump(cx);
                session.check_health()?;
                Ok(native_system_mouse_message_observed(
                    &observation,
                    source.window,
                    NativeWindowTestMessage::MouseMove,
                    delivered_reveal,
                ))
            })
            .await?;
        }
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
                (selected.target.display_id() == selected.negative_display.display_id())
                    .then_some(selected.negative_point),
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
            wait_until(cx, "shared live route source WndProc move", |cx| {
                session.pump(cx);
                session.check_health()?;
                Ok(native_system_mouse_message_observed(
                    &observation,
                    source.window,
                    NativeWindowTestMessage::MouseMove,
                    delivered,
                ))
            })
            .await?;
            wait_shared_live_placement(cx, session, provisional, source, delivered)
                .await
                .with_context(|| {
                    format!(
                        "shared live route point={delivered:?}, inspection={:?}, recent_drag={:?}",
                        session.dock.inspection(),
                        session
                            .drag_events
                            .borrow()
                            .iter()
                            .rev()
                            .take(4)
                            .collect::<Vec<_>>(),
                    )
                })?;
        }
        let delivered_release = session
            .move_captured_pointer(cx, release_point, "shared provisional release")
            .await?;
        if let Some(selected) = mixed_dpi_target {
            ensure!(
                native_test_display_at(delivered_release)?.display_id()
                    == selected.target.display_id(),
                "shared mixed-DPI release missed the selected target display: selected={selected:?}, delivered={delivered_release:?}"
            );
            let negative_route_point =
                if selected.source.display_id() == selected.negative_display.display_id() {
                    delivered_reveal
                } else {
                    delivered_release
                };
            ensure!(
                (negative_route_point.x < 0 || negative_route_point.y < 0)
                    && native_test_display_at(negative_route_point)?.display_id()
                        == selected.negative_display.display_id(),
                "shared mixed-DPI routing lost its actual negative-coordinate route: selected={selected:?}, reveal={delivered_reveal:?}, release={delivered_release:?}"
            );
            wait_until(cx, "shared mixed-DPI source WndProc target move", |cx| {
                session.pump(cx);
                session.check_health()?;
                Ok(native_system_mouse_message_observed(
                    &observation,
                    source.window,
                    NativeWindowTestMessage::MouseMove,
                    delivered_release,
                ))
            })
            .await?;
            ensure!(
                cx.update(|app| raw_hwnd(app, provisional.window))? == provisional.hwnd
                    && unsafe { IsWindow(Some(provisional.hwnd)).as_bool() }
                    && unsafe { IsWindowVisible(provisional.hwnd).as_bool() }
                    && unsafe { GetCapture() } == source.hwnd,
                "shared mixed-DPI route replaced or hid its provisional HWND or lost source capture before release"
            );
            ensure!(
                point_is_open_desktop(delivered_release),
                "shared mixed-DPI release point is no longer open desktop before release"
            );
            wait_shared_live_placement(cx, session, provisional, source, delivered_release).await?;
        }
        let release_display = native_test_display_at(delivered_release)?;
        let divergence = if behavior == NativeDockBehavior::LiveRouteAndReleaseLock {
            let mut used_points = vec![reveal_point];
            used_points.extend(route_points.iter().copied());
            Some(open_desktop_release_point_away_from_all(
                source.hwnd,
                target.hwnd,
                &used_points,
            )?)
        } else {
            None
        };
        let release_authority = barrier
            .as_ref()
            .map_or(NativeReleasePointAuthority::OpenDesktop, |barrier| {
                NativeReleasePointAuthority::OpaqueBarrier(barrier.hwnd())
            });
        if barrier.is_some() {
            release_authority.validate_current(delivered_release)?;
        }
        if let Some(divergence) = divergence {
            session
                .pointer
                .as_mut()
                .context("shared pointer guard is unavailable")?
                .inject_sequence(&[
                    (delivered_release, NativeTestPointerAction::PrimaryUp),
                    (divergence, NativeTestPointerAction::Move),
                ])?;
        } else {
            session.release_captured_pointer(delivered_release)?;
        }
        session.wait_drag_released(cx).await?;
        if let Some(divergence) = divergence {
            super::wait_for_cursor(cx, divergence, "shared post-release pointer divergence")
                .await?;
        }
        let release = session
            .drag_events
            .borrow()
            .iter()
            .find(|event| {
                event.source_window() == source.window.window_id()
                    && event.generation() == drag.generation
                    && event.phase() == open_gpui::NativeCapturedDragPhase::Released
            })
            .cloned()
            .context("shared promotion lost its original captured release")?;
        ensure!(
            release.global_position()
                == Some(open_gpui::point(
                    open_gpui::DevicePixels(delivered_release.x),
                    open_gpui::DevicePixels(delivered_release.y),
                ))
                && release
                    .window_hit_stack()
                    .observation()
                    .is_some_and(|observation| {
                        release_authority.matches_hit_terminus(observation.terminus())
                    }),
            "shared promotion lost the original release position or typed hit terminus: {release:?}"
        );
        let mut placement = None;
        wait_until(cx, "shared provisional same-HWND promotion", |cx| {
            session.pump(cx);
            session.check_health()?;
            let Some(callbacks) = session.dock.native_callbacks_for_window(provisional.window.window_id()) else {
                return Ok(false);
            };
            let Some(receipt) = callbacks.provisional_placement() else { return Ok(false) };
            let Some(create) = callbacks.create_request() else { return Ok(false) };
            ensure!(receipt.window_id() == provisional.window.window_id()
                && receipt.purpose() == WindowProvisionalPlacementPurpose::FinalRelease
                && receipt.anchor_point() == open_gpui::point(
                    open_gpui::DevicePixels(delivered_release.x), open_gpui::DevicePixels(delivered_release.y))
                && receipt.target_display().display_id() == release_display.display_id()
                && (receipt.target_display().scale_factor() - release_display.scale_factor()).abs() <= 0.001,
                "shared promotion lost its original release/target-display placement request: {receipt:?}");
            let binding = callbacks.binding();
            ensure!(create.binding.window() == binding,
                "shared promotion replaced its original create binding");
            let admitted = session.reports.iter().any(|report| matches!(report,
                NativeReport::Admitted(placement) if *placement == create.binding));
            let submitted = session.reports.iter().any(|report| matches!(report,
                NativeReport::Submission { binding: owner, snapshot }
                    if *owner == binding && snapshot.window_id() == provisional.window.window_id()
                        && snapshot.outcome() == WindowFrameSubmissionOutcome::Submitted));
            placement = Some(receipt);
            Ok(receipt.outcome() == WindowProvisionalPlacementOutcome::Settled
                && receipt.native_facts().is_some_and(|facts| facts.accepts_placement())
                && admitted && submitted
                && cx.update(|app| app.windows().len())
                    == session.preexisting_window_count + session.windows.len() + 1
                && !shared_surface_contains_item(session, source.item, source.surface)
                && unsafe { IsWindow(Some(provisional.hwnd)).as_bool() })
        })
        .await
        .with_context(|| {
            format!(
                "shared promotion inspection={:?}, original_hwnd_live={}, placement={placement:?}, reports={:?}, recent_drag={:?}",
                session.dock.inspection(),
                unsafe { IsWindow(Some(provisional.hwnd)).as_bool() },
                session.reports,
                session.drag_events.borrow().iter().rev().take(4).collect::<Vec<_>>(),
            )
        })?;
        let promoted = cx.update(|app| raw_hwnd(app, provisional.window))?;
        ensure!(
            promoted == provisional.hwnd,
            "shared provisional did not preserve its HWND"
        );
        let placement =
            placement.context("shared promotion lost its original placement receipt")?;
        let create = session
            .dock
            .native_callbacks_for_window(provisional.window.window_id())
            .and_then(|callbacks| callbacks.create_request())
            .context("shared promotion lost its original core outer intent")?;
        let facts = cx.update(|app| super::window_platform_facts(app, provisional.window))?;
        let geometry = facts
            .physical_geometry
            .context("shared promotion has no physical client geometry")?;
        let client = super::native_client_screen_bounds(promoted)?;
        ensure!(
            client == placement.client_bounds()
                && geometry.client_bounds() == placement.client_bounds()
                && geometry.display_observation() == Some(placement.target_display())
                && facts.display_id == Some(release_display.display_id())
                && (facts.scale_factor - release_display.scale_factor()).abs() <= 0.001
                && (geometry.scale_factor() - release_display.scale_factor()).abs() <= 0.001
                && native_outer_matches_intent(promoted, geometry, create.outer_bounds)?,
            "shared promotion diverged from its original target-DPI request: placement={placement:?}, facts={facts:?}, native_client={client:?}"
        );
        ensure!(
            Some(super::unique_native_identity_for_window(
                &observation.events(),
                provisional.window.window_id(),
                "promoted provisional",
            )?) == provisional.identity,
            "shared promotion replaced its native generation"
        );
        let destination = super::shared_recovery::wait_admitted_destination(cx, session).await?;
        ensure!(
            destination.window == provisional.window
                && destination.hwnd == provisional.hwnd
                && destination.identity == provisional.identity
                && destination.surface == create.binding.surface(),
            "shared promotion presented the retained View in a different destination"
        );
        cx.update(|app| session.assert_roots_unchanged(app, &roots))?;
        let final_hit_point = POINT {
            x: placement.client_bounds().center().x.0,
            y: placement.client_bounds().center().y.0,
        };
        let promoted_hit = unsafe {
            SendMessageW(
                promoted,
                WM_NCHITTEST,
                Some(WPARAM::default()),
                Some(screen_point_lparam(final_hit_point)),
            )
        };
        ensure!(
            promoted_hit.0 != HTTRANSPARENT as isize,
            "shared promotion retained the provisional hit gate"
        );
        session.assert_release_terminal(source, drag.generation)?;
        if let Some(barrier) = barrier {
            barrier.assert_point_scoped_band(promoted, delivered_release)?;
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
        wait_until(cx, "shared no-input overlay policies to commit", |cx| {
            cx.update(|app| {
                for overlay in &self.overlays {
                    let accepts_input = app.update_window(overlay.window, |_, window, _| {
                        window.accepts_pointer_input()
                    })?;
                    if accepts_input || !unsafe { IsWindowVisible(overlay.hwnd).as_bool() } {
                        return Ok(false);
                    }
                }
                Ok(true)
            })
        })
        .await?;
        native_test_acquire_foreground_window(target.hwnd.0 as isize)?;
        raise_native_window(first_hwnd)?;
        raise_native_window(second_hwnd)?;
        ensure!(
            super::native_window_is_above(second_hwnd, first_hwnd)
                && super::native_window_is_above(first_hwnd, target.hwnd),
            "shared no-input overlays did not form the required native prefix"
        );
        let point = target_point(cx, target, target.item).await?;
        for overlay in &self.overlays {
            let bounds = native_test_window_rect(overlay.hwnd)?;
            ensure!(
                rect_contains(bounds, point),
                "shared no-input overlay does not cover its target: overlay={:?}, bounds={bounds:?}, point={point:?}",
                overlay.hwnd
            );
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
        let hit = native_test_non_shell_root_window_at(point);
        ensure!(
            hit == Some(target.hwnd),
            "shared no-input prefix did not pass through to target: point={point:?}, expected={:?}, observed={hit:?}",
            target.hwnd
        );
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
        process_harness::worker_phase("process.primary-opening");
        let primary = cx.update(|app| {
            SharedSession::build(
                app,
                observation.clone(),
                SharedFixtureShape::TwoWindows,
                &bounds[..2],
            )
        })?;
        process_harness::worker_phase("process.primary-opened");
        let survivor = cx.update(|app| {
            SharedSession::build(
                app,
                observation,
                SharedFixtureShape::SurvivorWindow,
                &bounds[2..],
            )
        })?;
        process_harness::worker_phase("process.survivor-opened");
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
        process_harness::worker_phase("process.sessions-ready");
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
        process_harness::worker_phase("process.capture-active");
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
        process_harness::worker_phase("process.primary-retired");
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
        // Exit can return the platform loop before this future is polled again.
        if worker_subcase.uses_last_window_policy() {
            process_harness::worker_phase("process.last-window-requested");
            unsafe {
                PostMessageW(
                    Some(survivor_window.hwnd),
                    WM_CLOSE,
                    WPARAM::default(),
                    LPARAM::default(),
                )
            }
            .context("shared process convergence could not close survivor")?;
            self.exit_path = NativeWorkerExitPath::LastWindowClosed;
        } else {
            self.exit_path = NativeWorkerExitPath::AppShutdown;
            process_harness::worker_phase("process.app-shutdown-requested");
            cx.update(|app| app.shutdown_and_quit_for_native_exit_test());
        }
        Ok(())
    }

    fn capture_process_census(&mut self, app: &mut App) -> Result<NativeWorkerAppCensus> {
        let primary = self
            .session
            .as_mut()
            .context("shared primary session disappeared before loop return")?;
        let survivor = self
            .survivor
            .as_mut()
            .context("shared survivor session disappeared before loop return")?;
        let mut session_closed = true;
        let mut runtime_empty = true;
        let mut pending_terminal_ticket_count = 0;
        let mut failed_terminal_ticket_count = 0;
        for session in [primary, survivor] {
            session.reports.extend(session.dock.take_native_reports());
            let inspection = session.dock.inspection()?;
            let native = inspection
                .native
                .context("shared process session lost its managed native authority")?;
            session_closed &= native.admission_frozen && native.quiescent;
            runtime_empty &= native.quiescent
                && inspection.mounts.is_empty()
                && inspection.native_window_binding_count == 0
                && !inspection.update_pending
                && !inspection.active_gesture
                && !inspection.pending_presentation
                && !inspection.pending_presentation_reports
                && !inspection.pending_native_input
                && !inspection.pending_native_create
                && inspection.queued_input_count == 0
                && inspection.queued_native_effect_count == 0;
            pending_terminal_ticket_count += native.pending_root_terminal_count;
            failed_terminal_ticket_count += session
                .reports
                .iter()
                .filter(|report| {
                    matches!(report,
                        NativeReport::OpenFailed { retirement_ticket: Some(ticket), .. }
                            if ticket.snapshot().protocol_violation())
                })
                .count();
        }
        Ok(NativeWorkerAppCensus {
            window_registry_count: app.windows().len(),
            active_drag: app.has_active_drag()
                || app.active_native_captured_drag_source_window().is_some(),
            native_exit_authority_settled: app.native_exit_authority_is_settled_for_test(),
            surface: NativeWorkerSurfaceCensus::constructed(
                session_closed,
                Some(runtime_empty),
                pending_terminal_ticket_count,
                failed_terminal_ticket_count,
            ),
        })
    }

    fn validate_process_return(&self, census: &NativeWorkerAppCensus) -> Result<()> {
        ensure!(
            census.window_registry_count == 0
                && !census.active_drag
                && census.native_exit_authority_settled
                && census.surface.is_clean(),
            "shared process return retained logical authority: {census:?}"
        );
        ensure!(
            unsafe { GetCapture() } == HWND::default(),
            "shared process return retained native pointer capture"
        );
        ensure!(
            super::native_observation_is_terminal(&self.observation),
            "shared process return retained an unterminated native window generation"
        );
        let events = self.observation.events();
        for session in [&self.session, &self.survivor] {
            let session = session
                .as_ref()
                .context("shared process return lost a retained session")?;
            session.check_health()?;
            for window in &session.windows {
                ensure!(
                    !unsafe { IsWindow(Some(window.hwnd)).as_bool() },
                    "shared process return retained a session HWND: {window:?}"
                );
                let identity = window
                    .identity
                    .context("shared process return lost a native window identity")?;
                let lifecycle = super::native_shutdown_ordinals(&events, identity)?;
                super::assert_native_shutdown_order("shared process window", &lifecycle)?;
            }
        }
        Ok(())
    }

    async fn run_committed_loss(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let session = self
            .open_session(cx, SharedFixtureShape::TwoWindows)
            .await?;
        super::shared_recovery::seed_source_view(cx, session).await?;
        let source = session.source_window()?;
        let target = session.target_window()?;
        let drag = session.begin_captured_drag(cx, source, source.item).await?;
        let release = open_desktop_release_point(source.hwnd, target.hwnd)?;
        session
            .move_captured_pointer(cx, release, "shared recovery tear-off")
            .await?;
        session.release_captured_pointer(release)?;
        session.wait_drag_released(cx).await?;
        let destination = super::shared_recovery::wait_admitted_destination(cx, session).await?;
        super::shared_recovery::lose_and_recover_destination(cx, session, destination).await?;
        session.assert_release_terminal(source, drag.generation)?;
        session
            .milestones
            .push("shared.committed-loss-physical-release".to_owned());
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

async fn wait_shared_transfer(
    cx: &mut AsyncApp,
    session: &mut SharedSession,
    item: open_gpui_docking::dockspace::model::ItemId,
    target: SharedWindow,
) -> Result<()> {
    let selector = format!("dockspace:tab:{}", item.get());
    wait_until(cx, "shared item transfer and target presentation", |cx| {
        session.pump(cx);
        session.check_health()?;
        if !shared_surface_contains_item(session, item, target.surface)
            || !shared_item_selected(session, item)
        {
            return Ok(false);
        }
        let inspection = session.dock.inspection()?;
        let binding = session
            .dock
            .native_binding_for_window(target.window.window_id());
        let presented = inspection
            .mounts
            .iter()
            .find(|mount| mount.window_id == target.window.window_id().as_u64())
            .and_then(|mount| {
                mount
                    .regions
                    .iter()
                    .find(|region| region.surface == target.surface)
            })
            .is_some_and(|region| {
                region.current_presentation
                    && region.packet_deferred == Some(false)
                    && region.input_binding_current == Some(true)
                    && region.input_native_binding.is_some_and(|placement| {
                        placement.surface() == target.surface && Some(placement.window()) == binding
                    })
            });
        let painted = cx.update(|app| {
            app.update_window(target.window, |_, window, _| {
                window
                    .committed_debug_bounds_for_test()
                    .iter()
                    .any(|(candidate, bounds)| {
                        candidate == &selector
                            && bounds.size.width > open_gpui::px(0.0)
                            && bounds.size.height > open_gpui::px(0.0)
                    })
            })
        })?;
        let original_view_presented = item != SHARED_SOURCE_ITEM
            || cx.update(|app| super::shared_recovery::view_presented(app, session, target, 41))?;
        Ok(original_view_presented
            && presented
            && painted
            && cx.update(|app| window_has_non_empty_frame(app, target.window))?)
    })
    .await
    .with_context(|| {
        format!(
            "item {item:?} into {:?}: {:?}",
            target.surface,
            session.dock.inspection()
        )
    })
}

fn native_outer_matches_intent(
    hwnd: HWND,
    geometry: PlatformWindowPhysicalGeometry,
    intent: PhysicalRect,
) -> Result<bool> {
    let native = native_test_window_rect(hwnd)?;
    let outer = geometry
        .outer_bounds()
        .context("native placement has no outer frame observation")?;
    Ok((native.left, native.top, native.right, native.bottom)
        == (
            outer.origin.x.0,
            outer.origin.y.0,
            outer.right().0,
            outer.bottom().0,
        )
        && [
            f64::from(native.left),
            f64::from(native.top),
            f64::from(native.right) - f64::from(native.left),
            f64::from(native.bottom) - f64::from(native.top),
        ] == [intent.x(), intent.y(), intent.width(), intent.height()].map(f64::round))
}

async fn wait_shared_live_placement(
    cx: &mut AsyncApp,
    session: &mut SharedSession,
    provisional: SharedWindow,
    source: SharedWindow,
    delivered: POINT,
) -> Result<()> {
    let display = native_test_display_at(delivered)?;
    let anchor = open_gpui::point(
        open_gpui::DevicePixels(delivered.x),
        open_gpui::DevicePixels(delivered.y),
    );
    wait_until(
        cx,
        "shared pre-release live placement on the target DPI",
        |cx| {
            session.pump(cx);
            session.check_health()?;
            let events = session.observation.events();
            ensure!(
                cx.update(|app| raw_hwnd(app, provisional.window))? == provisional.hwnd
                    && Some(super::unique_native_identity_for_window(
                        &events,
                        provisional.window.window_id(),
                        "live provisional"
                    )?) == provisional.identity
                    && !events
                        .iter()
                        .any(|event| Some(event.window()) == provisional.identity
                            && matches!(
                                event.kind(),
                                NativeWindowTestEventKind::NativeTerminal { .. }
                            ))
                    && unsafe { IsWindowVisible(provisional.hwnd).as_bool() }
                    && unsafe { GetCapture() } == source.hwnd,
                "shared live placement replaced its native generation or lost source capture"
            );
            let Some((outer, receipt)) = session
                .dock
                .native_provisional_placement_for_test(provisional.window.window_id())
            else {
                return Ok(false);
            };
            if receipt.window_id() != provisional.window.window_id()
                || receipt.purpose() != WindowProvisionalPlacementPurpose::LiveRoute
                || receipt.anchor_point() != anchor
                || receipt.target_display().display_id() != display.display_id()
                || (receipt.target_display().scale_factor() - display.scale_factor()).abs() > 0.001
                || receipt.outcome() != WindowProvisionalPlacementOutcome::Settled
                || !receipt
                    .native_facts()
                    .is_some_and(|facts| facts.accepts_placement())
            {
                return Ok(false);
            }
            let facts = cx.update(|app| super::window_platform_facts(app, provisional.window))?;
            let Some(geometry) = facts.physical_geometry else {
                return Ok(false);
            };
            let client = super::native_client_screen_bounds(provisional.hwnd)?;
            let hit = unsafe {
                SendMessageW(
                    provisional.hwnd,
                    WM_NCHITTEST,
                    Some(WPARAM::default()),
                    Some(screen_point_lparam(delivered)),
                )
            };
            Ok(client == receipt.client_bounds()
                && geometry.client_bounds() == receipt.client_bounds()
                && geometry.display_observation() == Some(receipt.target_display())
                && facts.display_id == Some(display.display_id())
                && (facts.scale_factor - display.scale_factor()).abs() <= 0.001
                && native_outer_matches_intent(provisional.hwnd, geometry, outer)?
                && hit.0 == HTTRANSPARENT as isize)
        },
    )
    .await?;
    session.milestones.push(format!(
        "shared.live-placement.{}-{}.settled",
        delivered.x, delivered.y
    ));
    Ok(())
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
    .await
    .with_context(|| {
        format!(
            "shared provisional inspection={:?}, windows={:?}, reports={:?}, drag={:?}",
            session.dock.inspection(),
            cx.update(|app| app.windows()),
            session.reports,
            session.drag_events.borrow().last(),
        )
    })?;
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
