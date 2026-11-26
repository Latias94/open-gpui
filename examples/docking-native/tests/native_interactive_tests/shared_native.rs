//! Returning native application harness for shared-session tests without system input.

use std::{cell::RefCell, panic::AssertUnwindSafe, rc::Rc};

use anyhow::Result;
use futures::{FutureExt as _, future::LocalBoxFuture};
use open_gpui::{Application, AsyncApp, QuitMode};
use open_gpui_docking::native::NativeReport;
use open_gpui_platform::windows::{
    NativeTestDisplay, WindowsPlatform, begin_native_window_test_observation,
};

use super::shared_session::{
    SharedFixtureShape, SharedSession, two_window_bounds, two_window_bounds_on_display,
};
use super::{
    native_observation_is_terminal, panic_message, process_harness,
    shutdown_native_worker_application_without_surface,
};

pub(super) fn run_shared_native_test(
    source_display: Option<NativeTestDisplay>,
    run: impl for<'a> FnOnce(&'a mut AsyncApp, &'a mut SharedSession) -> LocalBoxFuture<'a, Result<()>>
    + 'static,
) {
    let (_guard, observation) = begin_native_window_test_observation();
    let before = process_harness::capture_process_window_census().unwrap();
    let outcome = Rc::new(RefCell::new(None));
    let completed = outcome.clone();
    let observed = observation.clone();
    {
        let platform = WindowsPlatform::new_returning_for_test(false).unwrap();
        let mut application =
            Application::with_platform(Rc::new(platform)).with_quit_mode(QuitMode::Explicit);
        application.run_returning_for_test(move |cx| {
            cx.spawn(async move |cx| {
                let mut session = None;
                let result = AssertUnwindSafe(async {
                    let bounds = cx.update(|app| match source_display {
                        Some(display) => two_window_bounds_on_display(app, display),
                        None => two_window_bounds(app),
                    })?;
                    session = Some(cx.update(|app| match source_display {
                        Some(display) => SharedSession::build_on_display(
                            app,
                            observed,
                            SharedFixtureShape::TwoWindows,
                            &bounds,
                            display.display_id(),
                        ),
                        None => SharedSession::build(
                            app,
                            observed,
                            SharedFixtureShape::TwoWindows,
                            &bounds,
                        ),
                    })?);
                    let session = session.as_mut().unwrap();
                    session.wait_ready(cx).await?;
                    if let Some(display) = source_display {
                        cx.update(|app| session.assert_roots_on_display(app, display, &bounds))?;
                    }
                    run(cx, session).await
                })
                .catch_unwind()
                .await
                .map_err(panic_message)
                .and_then(|result| result.map_err(|error| format!("{error:#}")));
                let session_cleanup = AssertUnwindSafe(async {
                    if let Some(session) = session.as_mut() {
                        session.shutdown(cx).await?;
                    }
                    Ok::<_, anyhow::Error>(())
                })
                .catch_unwind()
                .await
                .map_err(panic_message)
                .and_then(|result| result.map_err(|error| format!("{error:#}")));
                if session_cleanup.is_err()
                    && let Some(session) = &session
                {
                    let (observed, terminal) = session.native_generation_sets();
                    eprintln!(
                        "shared native state before App cleanup: inspection={:?}; census={:?}; generations={}/{}; lifetime reports={:?}",
                        session.dock.inspection(), session.capture_census(cx), terminal.len(), observed.len(),
                        session.reports.iter().filter(|report| matches!(report,
                            NativeReport::NativeTerminal(_)
                                | NativeReport::LogicalClosed(_)
                                | NativeReport::RetirePlacement(_)
                        )).rev().take(12).collect::<Vec<_>>()
                    );
                }
                let app_cleanup = AssertUnwindSafe(
                    shutdown_native_worker_application_without_surface(cx),
                )
                .catch_unwind()
                .await
                .map_err(panic_message)
                .and_then(|result| result.map_err(|error| format!("{error:#}")));
                if let Some(session) = &session {
                    let (observed, terminal) = session.native_generation_sets();
                    eprintln!(
                        "shared native checkpoints={:?}; native generations: observed={}, terminal={}",
                        session.milestones, observed.len(), terminal.len()
                    );
                }
                *completed.borrow_mut() = Some((result, session_cleanup, app_cleanup));
                cx.update(|app| app.shutdown_and_quit_for_native_exit_test());
            })
            .detach();
        });
    }
    let (result, session_cleanup, app_cleanup) = outcome
        .borrow_mut()
        .take()
        .expect("shared native test must finish before loop return");
    assert!(
        session_cleanup.is_ok() && app_cleanup.is_ok(),
        "shared native cleanup: session={session_cleanup:?}, app={app_cleanup:?}; scenario: {result:?}"
    );
    assert!(
        native_observation_is_terminal(&observation),
        "shared native test retained a window generation"
    );
    let after = process_harness::capture_process_window_census().unwrap();
    process_harness::validate_process_window_census(&before, &after)
        .expect("shared native test must restore the process HWND census");
    assert!(result.is_ok(), "shared native test: {result:?}");
}
