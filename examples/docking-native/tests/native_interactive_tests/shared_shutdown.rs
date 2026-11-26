//! Real Windows capture and dependent-first shutdown for the shared docking adapter.

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
    AnyWindowHandle, App, AppContext as _, Application, AsyncApp, Empty, NativeCapturedDragEvent,
    NativeCapturedDragGeneration, NativeCapturedDragPhase, NativeCapturedDragReleaseTerminal,
    PointerCancelReason, QuitMode, Subscription, WindowFrameSubmissionOutcome, WindowId,
    WindowOptions, point, px,
};
use open_gpui_docking::{
    DockPane, DockSurface, GpuiDockspaceConfig,
    dockspace::{
        geometry::PhysicalRect,
        model::{
            DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, ItemId,
            NativeWindowPlacement, RootId, SurfaceId,
        },
        policy::DockPolicy,
        runtime::{DockspaceSession, NativeWindowBinding},
    },
    native::NativeReport,
};
use open_gpui_platform::windows::{
    NativeTestPointerAction, NativeTestSystemPointerGuard, NativeWindowTestCaptureOwner,
    NativeWindowTestEventKind, NativeWindowTestIdentity, NativeWindowTestMessage,
    NativeWindowTestMessageDisposition, NativeWindowTestObservation,
    NativeWindowTestObservationCursor, WindowsPlatform, begin_native_window_test_observation,
    native_test_acquire_foreground_window, native_test_logical_client_point_to_screen,
    native_test_non_shell_root_window_at, native_test_release_primary_button_best_effort,
    native_test_window_rect,
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, GetCapture, VK_LBUTTON},
        WindowsAndMessaging::{GetCursorPos, IsWindow, PostMessageW, WM_CLOSE},
    },
};

use super::{
    INPUT_CANARY, NativeWorkerCompletion, assert_native_shutdown_order, native_generation_label,
    native_point, native_shutdown_ordinals, native_system_mouse_message_observed, panic_message,
    panic_payload_message, process_harness, raw_hwnd,
    shutdown_native_worker_application_without_surface, unique_matching_debug_bounds,
    unique_native_identity_for_window, wait_for_cursor, wait_until, window_has_non_empty_frame,
    window_scale_factor,
};
use process_harness::{
    NativeWorkerAppCensus, NativeWorkerExitPath, NativeWorkerOutcome, NativeWorkerSubcase,
    NativeWorkerSurfaceCensus,
};

const ANCHOR: SurfaceId = SurfaceId::new(1);
const CHILD: SurfaceId = SurfaceId::new(2);
const ANCHOR_ITEM: ItemId = ItemId::new(1);
const CHILD_ITEM: ItemId = ItemId::new(2);

pub(super) fn run_worker(registration: super::NativeScenarioRegistration) -> ! {
    let (_observation_guard, observation) = begin_native_window_test_observation();
    let before_application = process_harness::capture_process_window_census()
        .expect("shared shutdown worker must capture its pre-application HWND census");
    let panics = Arc::new(Mutex::new(Vec::new()));
    let panics_for_hook = panics.clone();
    let previous_hook = take_hook();
    set_hook(Box::new(move |panic| {
        if let Ok(mut panics) = panics_for_hook.lock() {
            panics.push(panic_payload_message(panic.payload()));
        }
        previous_hook(panic);
    }));
    let completion = Rc::new(RefCell::new(None::<NativeWorkerCompletion>));
    process_harness::worker_phase("platform.initializing");
    match WindowsPlatform::new_returning_for_test(false) {
        Ok(platform) => {
            process_harness::worker_phase("platform.initialized");
            let mut application =
                Application::with_platform(Rc::new(platform)).with_quit_mode(QuitMode::Explicit);
            let completion_for_application = completion.clone();
            let worker_observation = observation.clone();
            application.run_returning_for_test(move |cx| {
                process_harness::worker_phase("application.launched");
                cx.spawn(async move |cx| {
                    process_harness::worker_phase("scenario.running");
                    let mut worker = ShutdownWorker::default();
                    let mut failures = Vec::new();
                    let result = AssertUnwindSafe(worker.exercise(cx, &worker_observation))
                        .catch_unwind()
                        .await
                        .map_err(panic_message)
                        .and_then(|result| result.map_err(|error| format!("{error:#}")));
                    process_harness::worker_phase("scenario.finished");
                    if let Err(error) = result {
                        failures.push(error);
                    }
                    let cleanup = AssertUnwindSafe(worker.shutdown_surface(cx))
                        .catch_unwind()
                        .await
                        .map_err(panic_message)
                        .and_then(|result| result.map_err(|error| format!("{error:#}")));
                    if let Err(error) = cleanup {
                        failures.push(format!("shared native cleanup failed: {error}"));
                    }
                    let app_shutdown =
                        AssertUnwindSafe(shutdown_native_worker_application_without_surface(cx))
                            .catch_unwind()
                            .await
                            .map_err(panic_message)
                            .and_then(|result| result.map_err(|error| format!("{error:#}")));
                    if let Err(error) = app_shutdown {
                        failures.push(format!("terminal App shutdown failed: {error}"));
                    }
                    worker.pump(cx);
                    if let Err(error) = worker.check_health() {
                        failures.push(format!(
                            "shared cleanup reported a native failure: {error:#}"
                        ));
                    }
                    if let Some(mut pointer) = worker.pointer.take() {
                        if !pointer.release_primary_button_best_effort() {
                            failures.push("could not release the physical primary button".into());
                        }
                        drop(pointer);
                    }
                    let app_census = match cx.update(|app| worker.census(app)) {
                        Ok(census) => census,
                        Err(error) => {
                            failures.push(format!("shared application census failed: {error:#}"));
                            NativeWorkerAppCensus::default()
                        }
                    };
                    process_harness::worker_phase("cleanup.census-ready");
                    if app_census.window_registry_count != 0
                        || app_census.active_drag
                        || !app_census.native_exit_authority_settled
                        || (worker.dock.is_some() && !app_census.surface.is_clean())
                    {
                        failures.push(format!("shared cleanup did not converge: {app_census:?}"));
                    }
                    let outcome = if failures.is_empty() {
                        worker.milestones.push("scenario.completed".into());
                        NativeWorkerOutcome::Passed
                    } else {
                        NativeWorkerOutcome::Failed(failures.join("; "))
                    };
                    *completion_for_application.borrow_mut() = Some(NativeWorkerCompletion {
                        outcome,
                        milestones: worker.milestones,
                        exit_path: NativeWorkerExitPath::ExplicitCleanup,
                        app_census,
                    });
                    cx.update(|app| app.shutdown_and_quit_for_native_exit_test());
                })
                .detach();
            });
        }
        Err(error) => {
            *completion.borrow_mut() = Some(NativeWorkerCompletion {
                outcome: NativeWorkerOutcome::Failed(format!(
                    "shared worker Windows platform initialization failed: {error:#}"
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
                "shared worker returned without completion".into(),
            ),
            milestones: Vec::new(),
            exit_path: NativeWorkerExitPath::ExplicitCleanup,
            app_census: NativeWorkerAppCensus::default(),
        });
    let panics = panics
        .lock()
        .map(|panics| panics.clone())
        .unwrap_or_else(|_| vec!["shared worker panic observation lock was poisoned".into()]);
    if !panics.is_empty() {
        let message = format!("shared worker observed unexpected panics: {panics:?}");
        completion.outcome = match completion.outcome {
            NativeWorkerOutcome::Passed => NativeWorkerOutcome::Failed(message),
            NativeWorkerOutcome::Failed(previous) => {
                NativeWorkerOutcome::Failed(format!("{previous}; {message}"))
            }
        };
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
    let report = process_harness::finish_worker_report(
        &registration.id,
        NativeWorkerSubcase::Primary,
        completion.outcome,
        completion.milestones,
        NativeWorkerExitPath::ExplicitCleanup,
        before_application,
        completion.app_census,
        observed.len(),
        terminal.len(),
        unterminated,
        None,
    )
    .expect("shared shutdown worker must capture its post-application HWND census");
    process_harness::publish_worker_report_and_wait_for_release(&report);
    std::process::exit(0);
}

#[derive(Default)]
struct ShutdownWorker {
    dock: Option<DockSurface>,
    reports: Vec<NativeReport>,
    drag_events: Rc<RefCell<Vec<NativeCapturedDragEvent>>>,
    _drag_observer: Option<Subscription>,
    pointer: Option<NativeTestSystemPointerGuard>,
    milestones: Vec<String>,
}

struct ObservedWindow {
    window: AnyWindowHandle,
    hwnd: HWND,
    binding: NativeWindowBinding,
    identity: NativeWindowTestIdentity,
}

#[derive(Debug)]
struct LogicalCloseObservation {
    window: WindowId,
    capture_owner: HWND,
    active_drag: bool,
    native_source: Option<WindowId>,
    drag_terminals: Vec<NativeCapturedDragEvent>,
    source_capture_released: bool,
}

impl ObservedWindow {
    fn capture(
        app: &mut App,
        dock: &DockSurface,
        observation: &NativeWindowTestObservation,
        window: AnyWindowHandle,
    ) -> Result<Self> {
        Ok(Self {
            window,
            hwnd: raw_hwnd(app, window)?,
            binding: dock
                .native_binding_for_window(window.window_id())
                .context("shared window must retain its core-minted native binding")?,
            identity: unique_native_identity_for_window(
                &observation.events(),
                window.window_id(),
                "shared shutdown window",
            )?,
        })
    }
}

impl ShutdownWorker {
    fn pump(&mut self, cx: &mut AsyncApp) {
        if let Some(dock) = &self.dock {
            cx.update(|app| dock.pump_native(app));
            self.reports.extend(dock.take_native_reports());
        }
    }

    fn check_health(&self) -> Result<()> {
        if let Some(error) = self.dock.as_ref().and_then(DockSurface::error) {
            bail!("shared native adapter failed: {error}");
        }
        for report in &self.reports {
            match report {
                NativeReport::OpenFailed { stage, message, .. } => {
                    bail!("shared native create failed at {stage:?}: {message}")
                }
                // This worker independently proves OS capture at the original WndProc and GetCapture.
                NativeReport::Unsupported {
                    operation: "independent native OS capture ownership observation",
                } => {}
                NativeReport::Unsupported { operation } => {
                    bail!("shared native operation unsupported: {operation}")
                }
                NativeReport::CaptureTerminal {
                    terminal: NativeCapturedDragReleaseTerminal::Failed,
                    ..
                } => {
                    bail!("shared native capture release failed: {report:?}")
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn submitted(&self, expected: NativeWindowBinding) -> bool {
        self.reports.iter().any(|report| matches!(
            report,
            NativeReport::Submission { binding, snapshot }
                if *binding == expected
                    && snapshot.outcome() == WindowFrameSubmissionOutcome::Submitted
                    && snapshot.accepted_frame_generation().is_some()
                    && snapshot.accepted_frame_generation() == snapshot.submitted_frame_generation()
        ))
    }

    fn inject(&mut self, point: POINT, action: NativeTestPointerAction) -> Result<()> {
        self.pointer
            .as_mut()
            .context("physical pointer guard captured before shared setup")?
            .inject(point, action)
    }

    async fn exercise(
        &mut self,
        cx: &mut AsyncApp,
        observation: &NativeWindowTestObservation,
    ) -> Result<()> {
        ensure!(
            unsafe { GetCapture() } == HWND::default(),
            "shared worker started with unrelated native capture"
        );
        if unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) } as u16 & 0x8000 != 0 {
            let _ = native_test_release_primary_button_best_effort();
            bail!("shared worker started with the physical primary button pressed");
        }
        self.pointer = Some(NativeTestSystemPointerGuard::capture()?);
        let dock = cx.update(|app| -> Result<_> {
            let layout = DockspaceLayout::new([(ANCHOR, ANCHOR_ITEM), (CHILD, CHILD_ITEM)].map(
                |(surface, item)| {
                    DockspaceSurfaceLayout::new(
                        surface,
                        DockspaceRootLayout::new(
                            RootId::new(surface.get()),
                            DockspaceNode::central_tabs([item]),
                        ),
                    )
                },
            ))?;
            let mut policy = DockPolicy::default();
            policy.set_allow_native_surfaces(true);
            DockSurface::new_managed(
                DockspaceSession::from_layout(layout, policy)?,
                [
                    DockPane::new(ANCHOR_ITEM, "Shutdown anchor", app.new(|_| Empty)),
                    DockPane::new(CHILD_ITEM, "Shutdown dependent", app.new(|_| Empty)),
                ],
                GpuiDockspaceConfig::default(),
                app,
            )
        })?;
        self.dock = Some(dock.clone());
        let drag_events = self.drag_events.clone();
        self._drag_observer = Some(cx.update(|app| {
            app.observe_native_captured_drag(move |event, _| {
                drag_events.borrow_mut().push(event);
            })
        }));
        process_harness::worker_phase("anchor.opening");
        let anchor =
            cx.update(|app| dock.open_native_root(ANCHOR, true, WindowOptions::default(), app))?;
        process_harness::worker_phase("anchor.opened");
        wait_until(cx, "shared anchor's original Submitted frame", |cx| {
            self.pump(cx);
            self.check_health()?;
            Ok(dock
                .native_binding_for_window(anchor.window_id())
                .is_some_and(|binding| self.submitted(binding))
                && !dock.is_update_pending()
                && cx.update(|app| window_has_non_empty_frame(app, anchor))?)
        })
        .await?;
        process_harness::worker_phase("anchor.submitted");
        let anchor = cx.update(|app| ObservedWindow::capture(app, &dock, observation, anchor))?;
        let outer = native_test_window_rect(anchor.hwnd)?;
        cx.update(|app| {
            dock.update(
                |frame| {
                    frame.create_native_surface_current(
                        CHILD,
                        ANCHOR,
                        NativeWindowPlacement::new(PhysicalRect::new(
                            f64::from(outer.left) + 40.0,
                            f64::from(outer.top) + 40.0,
                            640.0,
                            480.0,
                        )?),
                    )?;
                    Ok(())
                },
                app,
            )
        })?;
        let mut source = None;
        wait_until(
            cx,
            "shared-created child's first-live admission and two Submitted HWNDs",
            |cx| {
                self.pump(cx);
                self.check_health()?;
                let admitted = self.reports.iter().find_map(|report| match report {
                    NativeReport::Admitted(binding) if binding.surface() == CHILD => Some(*binding),
                    _ => None,
                });
                let Some(admitted) = admitted else {
                    return Ok(false);
                };
                source = cx.update(|app| {
                    app.windows().into_iter().find(|window| {
                        dock.native_binding_for_window(window.window_id())
                            == Some(admitted.window())
                    })
                });
                let Some(source) = source else {
                    return Ok(false);
                };
                let inspection = dock.inspection()?;
                Ok(self.submitted(admitted.window())
                    && !inspection.update_pending
                    && inspection.mounts.len() == 2
                    && inspection.mounts.iter().all(|mount| {
                        mount.regions.iter().all(|region| {
                            region.current_presentation
                                && region.input_binding_current == Some(true)
                        })
                    })
                    && cx.update(|app| -> Result<bool> {
                        Ok(window_has_non_empty_frame(app, anchor.window)?
                            && window_has_non_empty_frame(app, source)?)
                    })?)
            },
        )
        .await?;
        let source = cx.update(|app| {
            ObservedWindow::capture(
                app,
                &dock,
                observation,
                source.context("admitted child must own a GPUI window")?,
            )
        })?;
        ensure!(
            source.window != anchor.window
                && source.hwnd != anchor.hwnd
                && source.binding != anchor.binding,
            "shared creation must retain two distinct physical lifetimes"
        );
        ensure!(
            cx.update(|app| app.windows().len()) == 2
                && dock.inspection()?.native_window_binding_count == 2,
            "shared shutdown requires exactly two actual core-bound windows"
        );
        self.milestones.push("shared.two-submitted-windows".into());

        let generation = self.begin_drag(cx, observation, &source).await?;
        let close_cursor = observation.cursor();
        let logical_closes = Rc::new(RefCell::new(Vec::new()));
        let logical_closes_for_observer = logical_closes.clone();
        let drag_events = self.drag_events.clone();
        let close_observation = observation.clone();
        let source_identity = source.identity;
        let _close_observer = cx.update(|app| {
            app.on_window_closed(move |app, window| {
                let closed = LogicalCloseObservation {
                    window,
                    capture_owner: unsafe { GetCapture() },
                    active_drag: app.has_active_drag(),
                    native_source: app.active_native_captured_drag_source_window(),
                    drag_terminals: drag_events
                        .borrow()
                        .iter()
                        .filter(|event| event.phase() != NativeCapturedDragPhase::Moved)
                        .cloned()
                        .collect(),
                    source_capture_released: close_observation.any_after(close_cursor, |event| {
                        event.window() == source_identity
                            && matches!(
                                event.kind(),
                                NativeWindowTestEventKind::WindowMessage {
                                    message: NativeWindowTestMessage::CaptureChanged,
                                    capture_after: NativeWindowTestCaptureOwner::None,
                                    disposition: NativeWindowTestMessageDisposition::Returned(_),
                                    ..
                                }
                            )
                    }),
                };
                logical_closes_for_observer.borrow_mut().push(closed);
            })
        });
        unsafe {
            PostMessageW(
                Some(anchor.hwnd),
                WM_CLOSE,
                WPARAM::default(),
                LPARAM::default(),
            )
        }
        .context("could not post real WM_CLOSE to the shared anchor")?;
        wait_until(
            cx,
            "shared capture cancellation and dependent-before-anchor shutdown",
            |cx| {
                self.pump(cx);
                self.check_health()?;
                Ok(dock.native_is_quiescent()
                    && cx.update(|app| {
                        app.windows().is_empty()
                            && !app.has_active_drag()
                            && app.active_native_captured_drag_source_window().is_none()
                    })
                    && unsafe { GetCapture() } == HWND::default()
                    && !unsafe { IsWindow(Some(source.hwnd)).as_bool() }
                    && !unsafe { IsWindow(Some(anchor.hwnd)).as_bool() })
            },
        )
        .await?;
        ensure!(
            observation.events_after(close_cursor).iter().any(|event| {
                event.window() == anchor.identity
                    && matches!(
                        event.kind(),
                        NativeWindowTestEventKind::WindowMessage {
                            message: NativeWindowTestMessage::Close,
                            disposition: NativeWindowTestMessageDisposition::Returned(_),
                            ..
                        }
                    )
            }),
            "anchor shutdown must originate in the real WM_CLOSE WndProc"
        );
        self.assert_shutdown(
            observation,
            close_cursor,
            &source,
            &anchor,
            generation,
            &logical_closes.borrow(),
        )?;
        self.milestones
            .push("shared.capture-clean-at-logical-close".into());
        self.milestones
            .push("shared.captured-anchor-shutdown".into());
        Ok(())
    }

    async fn begin_drag(
        &mut self,
        cx: &mut AsyncApp,
        observation: &NativeWindowTestObservation,
        source: &ObservedWindow,
    ) -> Result<NativeCapturedDragGeneration> {
        let bounds = cx.update(|app| {
            unique_matching_debug_bounds(
                app,
                source.window,
                "dockspace:tab:",
                &format!(":{}", CHILD_ITEM.get()),
            )
        })?;
        ensure!(
            bounds.size.width > px(50.0) && bounds.size.height > px(0.0),
            "the committed child tab must contain the real input path: {bounds:?}"
        );
        let scale = cx.update(|app| window_scale_factor(app, source.window))?;
        let press =
            native_test_logical_client_point_to_screen(source.hwnd, bounds.center(), scale)?;
        let mut ambient = POINT::default();
        unsafe { GetCursorPos(&mut ambient) }
            .context("could not sample ambient pointer position")?;
        let left = native_test_logical_client_point_to_screen(
            source.hwnd,
            point(bounds.center().x - px(24.0), bounds.center().y),
            scale,
        )?;
        let right = native_test_logical_client_point_to_screen(
            source.hwnd,
            point(bounds.center().x + px(24.0), bounds.center().y),
            scale,
        )?;
        let distance = |point: &POINT| {
            u64::from(point.x.abs_diff(ambient.x)) + u64::from(point.y.abs_diff(ambient.y))
        };
        let threshold = if distance(&left) > distance(&right) {
            left
        } else {
            right
        };
        native_test_acquire_foreground_window(source.hwnd.0 as isize)?;
        for target in [threshold, press] {
            ensure!(
                native_test_non_shell_root_window_at(target) == Some(source.hwnd),
                "the committed child tab input point is occluded: {target:?}"
            );
            self.inject(target, NativeTestPointerAction::Move)?;
            let delivered =
                wait_for_cursor(cx, target, "shared source pointer canary route").await?;
            wait_until(cx, "shared source WndProc canary move", |_| {
                Ok(native_system_mouse_message_observed(
                    observation,
                    source.window,
                    NativeWindowTestMessage::MouseMove,
                    delivered,
                ))
            })
            .await?;
        }
        let press = wait_for_cursor(cx, press, "shared committed tab press point").await?;
        self.inject(press, NativeTestPointerAction::PrimaryDown)?;
        wait_until(cx, "shared child WndProc primary-down native capture", |_| {
            Ok(unsafe { GetCapture() } == source.hwnd && observation.events().iter().any(|event| {
                event.window() == source.identity && matches!(event.kind(),
                    NativeWindowTestEventKind::WindowMessage {
                        message: NativeWindowTestMessage::PrimaryButtonDown, extra_info,
                        screen_point: Some(point), capture_before: NativeWindowTestCaptureOwner::None,
                        capture_after: NativeWindowTestCaptureOwner::Recipient,
                        disposition: NativeWindowTestMessageDisposition::Returned(_), ..
                    } if extra_info == INPUT_CANARY as isize && point == native_point(press))
            }))
        }).await?;
        self.inject(threshold, NativeTestPointerAction::Move)?;
        wait_until(cx, "shared native drag activation", |cx| {
            self.pump(cx);
            self.check_health()?;
            Ok(cx.update(|app| {
                app.has_active_drag()
                    && app.active_native_captured_drag_source_window()
                        == Some(source.window.window_id())
            }) && self
                .dock
                .as_ref()
                .context("shared managed surface")?
                .inspection()?
                .active_gesture)
        })
        .await?;
        let move_cursor = observation.cursor();
        self.inject(press, NativeTestPointerAction::Move)?;
        let captured_point = wait_for_cursor(cx, press, "shared captured move point").await?;
        wait_until(cx, "shared captured move with the original drag generation", |cx| {
            self.pump(cx);
            self.check_health()?;
            Ok(self.drag_events.borrow().iter().any(|event| event.source_window() == source.window.window_id()
                && event.phase() == NativeCapturedDragPhase::Moved
                && event.global_position().is_some_and(|point| point.x.0 == captured_point.x && point.y.0 == captured_point.y))
                && observation.events_after(move_cursor).iter().any(|event| event.window() == source.identity && matches!(event.kind(),
                    NativeWindowTestEventKind::WindowMessage {
                        message: NativeWindowTestMessage::MouseMove, extra_info,
                        screen_point: Some(point),
                        capture_before: NativeWindowTestCaptureOwner::Recipient,
                        capture_after: NativeWindowTestCaptureOwner::Recipient,
                        disposition: NativeWindowTestMessageDisposition::Returned(_), ..
                    } if extra_info == INPUT_CANARY as isize && point == native_point(captured_point))))
        }).await?;
        let events = self.drag_events.borrow();
        let generation = events
            .first()
            .context("original native captured drag event")?
            .generation();
        ensure!(
            events
                .iter()
                .all(|event| event.source_window() == source.window.window_id()
                    && event.generation() == generation
                    && event.phase() == NativeCapturedDragPhase::Moved),
            "shared drag must retain one live source generation before WM_CLOSE: {events:?}"
        );
        ensure!(
            unsafe { GetCapture() } == source.hwnd
                && unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) } as u16 & 0x8000 != 0,
            "the source must retain native capture and a pressed physical button before WM_CLOSE"
        );
        Ok(generation)
    }

    fn assert_shutdown(
        &self,
        observation: &NativeWindowTestObservation,
        close_cursor: NativeWindowTestObservationCursor,
        source: &ObservedWindow,
        anchor: &ObservedWindow,
        generation: NativeCapturedDragGeneration,
        logical_closes: &[LogicalCloseObservation],
    ) -> Result<()> {
        ensure!(
            logical_closes
                .iter()
                .map(|closed| closed.window)
                .eq([source.window.window_id(), anchor.window.window_id()]),
            "shared windows must each close logically once, dependent before anchor: {logical_closes:?}"
        );
        for closed in logical_closes {
            ensure!(
                closed.capture_owner == HWND::default()
                    && !closed.active_drag
                    && closed.native_source.is_none()
                    && closed.source_capture_released,
                "native capture and drag authority must be clear at the logical-close callback: {closed:?}"
            );
            ensure!(
                matches!(closed.drag_terminals.as_slice(), [event]
                    if event.source_window() == source.window.window_id()
                        && event.generation() == generation
                        && event.phase()
                            == NativeCapturedDragPhase::Cancelled(PointerCancelReason::CaptureRevoked)),
                "the original drag cancellation must already be observed at each logical-close callback: {closed:?}"
            );
        }
        let events = observation.events();
        let observed = events
            .iter()
            .map(|event| native_generation_label(event.window()))
            .collect::<BTreeSet<_>>();
        ensure!(
            observed
                == [
                    native_generation_label(source.identity),
                    native_generation_label(anchor.identity)
                ]
                .into_iter()
                .collect(),
            "shared shutdown observed an unexpected physical window generation: {observed:?}"
        );
        for window in [source, anchor] {
            ensure!(
                unique_native_identity_for_window(
                    &events,
                    window.window.window_id(),
                    "shared retired window"
                )? == window.identity,
                "shared shutdown crossed native window generations"
            );
            ensure!(
                events
                    .iter()
                    .filter(|event| event.window() == window.identity
                        && matches!(
                            event.kind(),
                            NativeWindowTestEventKind::NativeTerminal { .. }
                        ))
                    .count()
                    == 1,
                "each shared HWND must publish exactly one native terminal: {events:?}"
            );
            ensure!(
                self.reports
                    .iter()
                    .filter(|report| matches!(report,
                NativeReport::NativeTerminal(binding) if *binding == window.binding))
                    .count()
                    == 1,
                "each core-minted native binding must reach exactly one terminal: {:?}",
                self.reports
            );
            let ticket_generations = events
                .iter()
                .filter(|event| event.window() == window.identity)
                .filter_map(|event| match event.kind() {
                    NativeWindowTestEventKind::PresentationQuiesced { ticket_generation }
                    | NativeWindowTestEventKind::DestroyEntered { ticket_generation }
                    | NativeWindowTestEventKind::NativeTerminal { ticket_generation } => {
                        Some(ticket_generation)
                    }
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            ensure!(
                ticket_generations.len() == 1,
                "shared window crossed retirement-ticket generations: {ticket_generations:?}"
            );
        }
        let dependent = native_shutdown_ordinals(&events, source.identity)?;
        let anchor_lifecycle = native_shutdown_ordinals(&events, anchor.identity)?;
        assert_native_shutdown_order("shared dependent", &dependent)?;
        assert_native_shutdown_order("shared anchor", &anchor_lifecycle)?;
        ensure!(
            dependent.non_client_destroy < anchor_lifecycle.destroy_entered,
            "shared anchor destruction preceded dependent WM_NCDESTROY: dependent={dependent:?}, anchor={anchor_lifecycle:?}"
        );
        let capture_releases = self
            .reports
            .iter()
            .filter_map(|report| match report {
                NativeReport::CaptureTerminal {
                    binding,
                    barrier,
                    terminal,
                } => Some((*binding, *barrier, *terminal)),
                _ => None,
            })
            .collect::<Vec<_>>();
        ensure!(
            capture_releases.len() == 1
                && capture_releases[0].0 == source.binding
                && capture_releases[0].1.source_window() == source.window.window_id()
                && capture_releases[0].1.drag_generation() == generation
                && capture_releases[0].2 == NativeCapturedDragReleaseTerminal::Released,
            "shared shutdown requires one exact original capture-release barrier: {capture_releases:?}"
        );
        let drags = self.drag_events.borrow();
        let terminals = drags
            .iter()
            .filter(|event| event.phase() != NativeCapturedDragPhase::Moved)
            .collect::<Vec<_>>();
        ensure!(
            terminals.len() == 1
                && terminals[0].source_window() == source.window.window_id()
                && terminals[0].generation() == generation
                && terminals[0].phase()
                    == NativeCapturedDragPhase::Cancelled(PointerCancelReason::CaptureRevoked)
                && drags.iter().all(|event| event.generation() == generation),
            "shared anchor close must cancel exactly the original drag generation once: {drags:?}"
        );
        ensure!(
            observation
                .events_after(close_cursor)
                .iter()
                .any(|event| event.window() == source.identity
                    && event.ordinal() < dependent.destroy_entered
                    && matches!(
                        event.kind(),
                        NativeWindowTestEventKind::WindowMessage {
                            message: NativeWindowTestMessage::CaptureChanged,
                            capture_after: NativeWindowTestCaptureOwner::None,
                            disposition: NativeWindowTestMessageDisposition::Returned(_),
                            ..
                        }
                    )),
            "source WndProc must observe capture release before native destruction: {events:?}"
        );
        ensure!(
            self.reports
                .iter()
                .filter(|report| matches!(report, NativeReport::GroupShutdownRequested))
                .count()
                == 1,
            "anchor close must freeze exactly one shared provider generation: {:?}",
            self.reports
        );
        self.check_health()
    }

    async fn shutdown_surface(&mut self, cx: &mut AsyncApp) -> Result<()> {
        let Some(dock) = self.dock.clone() else {
            return Ok(());
        };
        let begin = cx.update(|app| dock.begin_native_shutdown(app));
        self.pump(cx);
        begin?;
        wait_until(
            cx,
            "shared cleanup through its original native shutdown owner",
            |cx| {
                self.pump(cx);
                Ok(dock.native_is_quiescent()
                    && cx.update(|app| app.windows().is_empty() && !app.has_active_drag())
                    && unsafe { GetCapture() } == HWND::default())
            },
        )
        .await
    }

    fn census(&self, app: &mut App) -> Result<NativeWorkerAppCensus> {
        let surface = match &self.dock {
            Some(dock) => {
                let inspection = dock.inspection()?;
                let native = inspection
                    .native
                    .context("shared managed native inspection")?;
                let failed_tickets = self
                    .reports
                    .iter()
                    .filter(|report| {
                        matches!(report,
                    NativeReport::OpenFailed { retirement_ticket: Some(ticket), .. }
                        if ticket.snapshot().protocol_violation())
                    })
                    .count();
                NativeWorkerSurfaceCensus::constructed(
                    native.admission_frozen && native.quiescent,
                    Some(
                        dock.native_is_quiescent()
                            && inspection.mounts.is_empty()
                            && inspection.native_window_binding_count == 0
                            && !inspection.active_gesture
                            && !inspection.pending_presentation
                            && !inspection.pending_presentation_reports
                            && !inspection.pending_native_input
                            && !inspection.pending_native_create
                            && inspection.queued_input_count == 0
                            && inspection.queued_native_effect_count == 0,
                    ),
                    native.pending_root_terminal_count,
                    failed_tickets,
                )
            }
            None => NativeWorkerSurfaceCensus::NotConstructed,
        };
        Ok(NativeWorkerAppCensus {
            window_registry_count: app.windows().len(),
            active_drag: app.has_active_drag(),
            native_exit_authority_settled: app.native_exit_authority_is_settled_for_test(),
            surface,
        })
    }
}
