use super::*;

use anyhow::{Context as _, Result, anyhow, bail, ensure};
use futures::FutureExt as _;
use open_gpui::{
    AnyWindowHandle, Application, AsyncApp, DevicePixels, DisplayId, Empty,
    PlatformWindowPresentOutcome, QuitMode, Subscription, WindowMouseEvent, WindowMutationDispatch,
    WindowPlatformFacts,
};
use open_gpui_docking::{
    DockSurfaceChangeEvent, DockSurfaceTransition, DockSurfaceWindowSessionPhase, model::DockGraph,
    native_captured_release_placement_for_test,
};
use open_gpui_windows::{
    NativeTestDisplay, NativeTestOpaqueWindow, NativeWindowTestCaptureOwner, NativeWindowTestEvent,
    NativeWindowTestEventKind, NativeWindowTestMessage, NativeWindowTestMessageDisposition,
    NativeWindowTestObservation, NativeWindowTestPoint, WindowsPlatform,
    arm_native_no_input_generation_drift, begin_native_window_test_observation,
    native_test_acquire_foreground_window, native_test_displays,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
    mem::size_of,
    panic::{AssertUnwindSafe, set_hook, take_hook},
    time::{Duration, Instant},
};
use windows::Win32::UI::WindowsAndMessaging::HTTRANSPARENT;
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT, RECT, WPARAM},
    Graphics::Gdi::{ClientToScreen, MONITOR_DEFAULTTONULL, MonitorFromPoint},
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, GetCapture, INPUT, INPUT_0, INPUT_MOUSE, MOUSE_EVENT_FLAGS,
            MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
            MOUSEEVENTF_MOVE_NOCOALESCE, MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, ReleaseCapture,
            SendInput, VK_LBUTTON,
        },
        WindowsAndMessaging::{
            GA_ROOT, GW_HWNDNEXT, GetAncestor, GetClassNameW, GetClientRect, GetCursorPos,
            GetDesktopWindow, GetForegroundWindow, GetShellWindow, GetSystemMetrics, GetWindow,
            GetWindowRect, GetWindowThreadProcessId, HWND_TOP, IsWindow, IsWindowVisible,
            MA_NOACTIVATEANDEAT, PostMessageW, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
            SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            SendMessageW, SetCursorPos, SetForegroundWindow, SetWindowPos, WM_CLOSE,
            WM_MOUSEACTIVATE, WM_NCHITTEST, WindowFromPoint,
        },
    },
};

const NATIVE_DOCK_SUITE_ID: &str = "docking-native.windows.interactive";
const NATIVE_SCENARIO_ENV: &str = "OPEN_GPUI_NATIVE_SCENARIO_ID";
const INPUT_CANARY: usize = 0x4f47_5044;
const SCENARIO_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(5);
const NATIVE_SCENARIO_MANIFEST: &str =
    include_str!("../tests/native_windows_interactive.native-scenarios.toml");

mod process_harness;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum NativeDockBehavior {
    SourceCapture,
    OpaqueOcclusion,
    SurfaceShutdown,
    ProvisionalSameHwndPromotion,
    CommittedLossRecovery,
    ProcessConvergence,
    NoInputPassThrough,
    MixedDpiPlacement,
}

impl NativeDockBehavior {
    async fn run(
        self,
        scenario_id: &str,
        cx: &mut AsyncApp,
        scenario: &mut NativeDockScenario,
    ) -> Result<()> {
        match self {
            Self::SourceCapture => run_captured_host_drop_scenario(cx, scenario, scenario_id).await,
            Self::OpaqueOcclusion
            | Self::ProvisionalSameHwndPromotion
            | Self::CommittedLossRecovery
            | Self::MixedDpiPlacement => {
                run_provisional_same_hwnd_promotion_scenario(cx, scenario, self, scenario_id).await
            }
            Self::SurfaceShutdown | Self::ProcessConvergence => {
                run_captured_surface_shutdown_scenario(cx, scenario, self, scenario_id).await
            }
            Self::NoInputPassThrough => {
                run_no_input_pass_through_scenario(cx, scenario, scenario_id).await
            }
        }
    }

    const fn uses_opaque_underlay(self) -> bool {
        matches!(self, Self::OpaqueOcclusion)
    }

    const fn closes_committed_destination(self) -> bool {
        matches!(self, Self::CommittedLossRecovery)
    }

    const fn crosses_dpi_boundary(self) -> bool {
        matches!(self, Self::MixedDpiPlacement)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeScenarioManifest {
    schema: u16,
    suite: String,
    runner: String,
    scenario: Vec<NativeScenarioRegistration>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeScenarioRegistration {
    id: String,
    requirement_owner: String,
    test: String,
    observation_domains: BTreeSet<String>,
    behavior: NativeDockBehavior,
}

fn native_scenario_manifest() -> NativeScenarioManifest {
    toml::from_str(NATIVE_SCENARIO_MANIFEST)
        .expect("the native interactive scenario manifest must parse")
}

fn native_scenario_registration_from_environment() -> NativeScenarioRegistration {
    let scenario_id = std::env::var(NATIVE_SCENARIO_ENV).unwrap_or_else(|_| {
        panic!(
            "{NATIVE_SCENARIO_ENV} must identify the manifest row when a native interactive worker is run"
        )
    });
    native_scenario_manifest()
        .scenario
        .into_iter()
        .find(|scenario| scenario.id == scenario_id)
        .unwrap_or_else(|| {
            panic!("native interactive scenario `{scenario_id}` is not present in the manifest")
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeMouseKind {
    Move,
    Down,
    Up,
}

#[derive(Debug, Default)]
struct NativeMouseTrace {
    source: Vec<NativeMouseKind>,
    target: Vec<NativeMouseKind>,
}

struct NativeOpaqueBarrier {
    window: NativeTestOpaqueWindow,
    hwnd: HWND,
}

impl NativeOpaqueBarrier {
    fn prepare_partial_cover(center: POINT) -> Result<Self> {
        let width = 160;
        let height = 160;
        let window = NativeTestOpaqueWindow::create_hidden(Bounds::new(
            point(
                DevicePixels(center.x.saturating_sub(width / 2)),
                DevicePixels(center.y.saturating_sub(height / 2)),
            ),
            size(DevicePixels(width), DevicePixels(height)),
        ))?;
        let hwnd = HWND(window.native_handle() as *mut c_void);
        ensure!(
            hwnd != HWND::default(),
            "the native Dock scenario created an invalid opaque barrier HWND"
        );
        Ok(Self { window, hwnd })
    }

    fn present(&self) -> Result<()> {
        self.window.present()
    }

    fn assert_point_scoped_band(&self, provisional_hwnd: HWND, reveal_point: POINT) -> Result<()> {
        let barrier_rect = window_rect(self.hwnd)?;
        let provisional_rect = window_rect(provisional_hwnd)?;
        ensure!(
            rect_contains(barrier_rect, reveal_point)
                && rect_contains(provisional_rect, reveal_point),
            "the reveal point must be covered by both the opaque barrier and provisional HWND: point={reveal_point:?}, barrier={barrier_rect:?}, provisional={provisional_rect:?}"
        );
        ensure!(
            rects_overlap(barrier_rect, provisional_rect)
                && !rect_contains_rect(barrier_rect, provisional_rect),
            "the normal opaque barrier must cover only part of the provisional HWND: barrier={barrier_rect:?}, provisional={provisional_rect:?}"
        );
        ensure!(
            native_window_is_above(self.hwnd, provisional_hwnd),
            "the point-scoped reveal must keep the normal opaque barrier above the provisional HWND"
        );
        ensure!(
            root_window_from_point(reveal_point) == self.hwnd,
            "the opaque barrier must remain the exact native hit at the reveal point while the provisional is visible"
        );
        Ok(())
    }
}

fn native_point(point: POINT) -> NativeWindowTestPoint {
    NativeWindowTestPoint {
        x: point.x,
        y: point.y,
    }
}

fn is_native_mouse_message(message: NativeWindowTestMessage) -> bool {
    matches!(
        message,
        NativeWindowTestMessage::MouseMove
            | NativeWindowTestMessage::PrimaryButtonDown
            | NativeWindowTestMessage::PrimaryButtonUp
    )
}

struct NativeDockScenario {
    surface: DockSurface,
    source_window: AnyWindowHandle,
    target_window: AnyWindowHandle,
    source_hwnd: HWND,
    target_hwnd: HWND,
    target_bounds: Bounds<Pixels>,
    initial_revision: u64,
    initial_window_count: usize,
    initial_owned_window_count: usize,
    trace: Arc<Mutex<NativeMouseTrace>>,
    surface_changes: Rc<RefCell<Vec<DockSurfaceChangeEvent>>>,
    native_observation: NativeWindowTestObservation,
    _source_interceptor: Subscription,
    _target_interceptor: Subscription,
    _surface_change_subscription: Subscription,
}

struct NativeNoInputOverlay {
    window: AnyWindowHandle,
    hwnd: HWND,
}

struct SystemPointerGuard {
    original_position: POINT,
    original_foreground: HWND,
    capture_owner: HWND,
    primary_button_down: bool,
}

impl SystemPointerGuard {
    fn capture(capture_owner: HWND) -> Result<Self> {
        let mut original_position = POINT::default();
        unsafe { GetCursorPos(&mut original_position) }
            .context("the native Dock scenario could not read the system cursor")?;
        Ok(Self {
            original_position,
            original_foreground: unsafe { GetForegroundWindow() },
            capture_owner,
            primary_button_down: false,
        })
    }
}

impl Drop for SystemPointerGuard {
    fn drop(&mut self) {
        if self.primary_button_down {
            let _ = inject_primary_button_up_best_effort();
        }
        if unsafe { GetCapture() } == self.capture_owner {
            let _ = unsafe { ReleaseCapture() };
        }
        let _ = unsafe { SetCursorPos(self.original_position.x, self.original_position.y) };
        if self.original_foreground != HWND::default()
            && unsafe { IsWindow(Some(self.original_foreground)).as_bool() }
        {
            let _ = unsafe { SetForegroundWindow(self.original_foreground) };
        }
    }
}

#[test]
fn native_interactive_scenario_registry_matches_cases() {
    let manifest = native_scenario_manifest();
    assert_eq!(manifest.schema, 3);
    assert_eq!(manifest.suite, NATIVE_DOCK_SUITE_ID);
    assert_eq!(
        manifest.runner,
        "open-gpui-windows-native-interactive-ephemeral"
    );

    let mut registrations = BTreeMap::new();
    for registration in manifest.scenario {
        let scenario_id = registration.id.clone();
        assert!(
            registrations
                .insert(
                    scenario_id.clone(),
                    (
                        registration.requirement_owner,
                        registration.test,
                        registration.observation_domains,
                        registration.behavior,
                    ),
                )
                .is_none(),
            "native scenario `{}` is registered more than once",
            scenario_id
        );
    }
    assert!(
        registrations
            .values()
            .all(|(_, test, domains, _)| !test.is_empty() && !domains.is_empty()),
        "every manifest row must carry one exact test coordinate and typed observation evidence"
    );
}

#[test]
#[ignore = "requires the open-gpui-windows-native-interactive-ephemeral runner"]
fn native_interactive_two_hwnd_captured_drag_routes_preview_and_drop() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::SourceCapture);
}

#[test]
#[ignore = "requires the open-gpui-windows-native-interactive-ephemeral runner"]
fn native_interactive_opaque_occlusion_blocks_underlay_and_preserves_same_hwnd() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::OpaqueOcclusion);
}

#[test]
#[ignore = "requires the open-gpui-windows-native-interactive-ephemeral runner"]
fn native_interactive_anchor_close_releases_capture_and_retires_dependent_hwnds() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::SurfaceShutdown);
}

#[test]
#[ignore = "requires the open-gpui-windows-native-interactive-ephemeral runner"]
fn native_interactive_provisional_gate_presents_and_promotes_same_hwnd() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::ProvisionalSameHwndPromotion);
}

#[test]
#[ignore = "requires the open-gpui-windows-native-interactive-ephemeral runner"]
fn native_interactive_committed_destination_loss_retires_runtime_authority() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::CommittedLossRecovery);
}

#[test]
#[ignore = "requires the open-gpui-windows-native-interactive-ephemeral runner"]
fn native_interactive_process_converges_after_active_surface_shutdown() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::ProcessConvergence);
}

#[test]
#[ignore = "requires the open-gpui-windows-native-interactive-ephemeral runner"]
fn native_interactive_no_input_prefix_passes_through_and_fails_closed_on_generation_drift() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::NoInputPassThrough);
}

#[test]
#[ignore = "requires the open-gpui-windows-native-interactive-ephemeral runner"]
fn native_interactive_mixed_dpi_final_client_bounds_are_exact() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::MixedDpiPlacement);
}

fn run_native_interactive_manifest_scenario(expected_behavior: NativeDockBehavior) {
    let registration = native_scenario_registration_from_environment();
    assert_eq!(
        registration.behavior, expected_behavior,
        "the manifest scenario must execute through its behavior-specific ignored test wrapper"
    );
    ensure_native_interactive_runner(&registration.id);
    if process_harness::is_worker(&registration.id) {
        run_native_interactive_worker(registration);
    }
    process_harness::run_case_in_worker(&registration);
}

fn run_native_interactive_worker(registration: NativeScenarioRegistration) -> ! {
    process_harness::await_worker_start();
    let scenario_id = registration.id.clone();
    let behavior = registration.behavior;
    let (_native_observation_guard, native_observation) = begin_native_window_test_observation();
    let final_native_observation = native_observation.clone();
    let before_application = process_harness::capture_process_window_census().unwrap_or_else(
        |error| {
            panic!(
                "native Dock worker `{scenario_id}` could not capture its pre-application HWND census: {error:#}"
            )
        },
    );
    let observed_panics = Arc::new(Mutex::new(Vec::<String>::new()));
    let observed_panics_for_hook = Arc::clone(&observed_panics);
    let completion = Arc::new(Mutex::new(None::<NativeWorkerCompletion>));
    let completion_for_application = Arc::clone(&completion);
    let scenario_id_for_application = scenario_id.clone();
    let previous_panic_hook = take_hook();
    set_hook(Box::new(move |panic| {
        if let Ok(mut observed_panics) = observed_panics_for_hook.lock() {
            observed_panics.push(panic.to_string());
        }
        previous_panic_hook(panic);
    }));

    Application::with_platform(Rc::new(
        WindowsPlatform::new_returning_for_test(false)
            .expect("native Dock worker Windows platform should initialize"),
    ))
        .with_quit_mode(QuitMode::Explicit)
        .run(
            move |cx| match build_native_scenario(cx, native_observation) {
                Ok(mut scenario) => {
                    let completion = Arc::clone(&completion_for_application);
                    cx.spawn(async move |cx| {
                        let result = AssertUnwindSafe(behavior.run(
                            &scenario_id_for_application,
                            cx,
                            &mut scenario,
                        ))
                        .catch_unwind()
                        .await
                        .map_err(panic_message)
                        .and_then(|result| result.map_err(|error| error.to_string()));
                        let observed_panics = observed_panics
                            .lock()
                            .map(|panics| panics.clone())
                            .unwrap_or_else(|_| {
                                vec!["native panic observation lock was poisoned".to_owned()]
                            });
                        let shutdown_result =
                            shutdown_native_worker_application(cx, &scenario).await;
                        let app_census = shutdown_result
                            .as_ref()
                            .cloned()
                            .unwrap_or_else(|_| capture_worker_app_census(cx, &scenario));
                        let mut failures = Vec::new();
                        if let Err(error) = result {
                            failures.push(error);
                        }
                        if !observed_panics.is_empty() {
                            failures
                                .push(format!("observed inner panics: {observed_panics:?}"));
                        }
                        if let Err(error) = shutdown_result {
                            failures.push(format!(
                                "native worker application did not converge before returning: {error:#}"
                            ));
                        }
                        let outcome = if failures.is_empty() {
                            process_harness::NativeWorkerOutcome::Passed
                        } else {
                            process_harness::NativeWorkerOutcome::Failed(failures.join("; "))
                        };
                        *completion
                            .lock()
                            .expect("native worker completion lock must not be poisoned") =
                            Some(NativeWorkerCompletion {
                                outcome,
                                milestones: vec!["scenario.completed".to_owned()],
                                app_census,
                            });
                        cx.update(|cx| cx.quit());
                    })
                    .detach();
                }
                Err(error) => {
                    let completion = Arc::clone(&completion_for_application);
                    cx.spawn(async move |cx| {
                        let shutdown_result =
                            shutdown_native_worker_application_without_surface(cx).await;
                        let app_census = capture_worker_app_census_without_surface(cx);
                        let message = match shutdown_result {
                            Ok(()) => error.to_string(),
                            Err(shutdown_error) => format!(
                                "{error:#}; native worker application did not converge after scenario construction failed: {shutdown_error:#}"
                            ),
                        };
                        *completion
                            .lock()
                            .expect("native worker completion lock must not be poisoned") =
                            Some(NativeWorkerCompletion {
                                outcome: process_harness::NativeWorkerOutcome::Failed(message),
                                milestones: Vec::new(),
                                app_census,
                            });
                        cx.update(|cx| cx.quit());
                    })
                    .detach();
                }
            },
        );

    let completion = completion
        .lock()
        .expect("native worker completion lock must not be poisoned")
        .take()
        .unwrap_or_else(|| NativeWorkerCompletion {
            outcome: process_harness::NativeWorkerOutcome::Failed(format!(
                "native Dock worker `{scenario_id}` returned without publishing scenario completion"
            )),
            milestones: Vec::new(),
            app_census: process_harness::NativeWorkerAppCensus::default(),
        });
    let events = final_native_observation.events();
    let observed_native_generations = events
        .iter()
        .map(|event| native_generation_label(event.window()))
        .collect::<BTreeSet<_>>();
    let terminal_native_generations = events
        .iter()
        .filter(|event| {
            matches!(
                event.kind(),
                NativeWindowTestEventKind::NativeTerminal { .. }
            )
        })
        .map(|event| native_generation_label(event.window()))
        .collect::<BTreeSet<_>>();
    let unterminated_native_generations = observed_native_generations
        .difference(&terminal_native_generations)
        .cloned()
        .collect();
    let report = process_harness::finish_worker_report(
        &scenario_id,
        completion.outcome,
        completion.milestones,
        before_application,
        completion.app_census,
        observed_native_generations.len(),
        terminal_native_generations.len(),
        unterminated_native_generations,
    )
    .unwrap_or_else(|error| {
        eprintln!(
            "native Dock worker `{scenario_id}` could not establish its post-application HWND census: {error:#}"
        );
        std::process::exit(2);
    });
    process_harness::publish_worker_report_and_wait_for_release(&report);
    std::process::exit(0);
}

struct NativeWorkerCompletion {
    outcome: process_harness::NativeWorkerOutcome,
    milestones: Vec<String>,
    app_census: process_harness::NativeWorkerAppCensus,
}

fn capture_worker_app_census(
    cx: &mut AsyncApp,
    scenario: &NativeDockScenario,
) -> process_harness::NativeWorkerAppCensus {
    cx.update(|app| {
        let status = scenario.surface.window_session_status(app);
        process_harness::NativeWorkerAppCensus {
            window_registry_count: app.windows().len(),
            active_drag: app.has_active_drag(),
            native_exit_authority_settled: app.native_exit_authority_is_settled_for_test(),
            surface_session_closed: status.phase() == DockSurfaceWindowSessionPhase::Closed,
            surface_runtime_empty: status.runtime_empty(),
            pending_terminal_ticket_count: status.pending_terminal_ticket_count(),
            failed_terminal_ticket_count: status.failed_terminal_ticket_count(),
        }
    })
}

fn capture_worker_app_census_without_surface(
    cx: &mut AsyncApp,
) -> process_harness::NativeWorkerAppCensus {
    cx.update(|app| process_harness::NativeWorkerAppCensus {
        window_registry_count: app.windows().len(),
        active_drag: app.has_active_drag(),
        native_exit_authority_settled: app.native_exit_authority_is_settled_for_test(),
        ..Default::default()
    })
}

fn native_worker_application_is_converged(
    census: &process_harness::NativeWorkerAppCensus,
    scenario: &NativeDockScenario,
) -> bool {
    census.window_registry_count == 0
        && !census.active_drag
        && census.native_exit_authority_settled
        && census.surface_session_closed
        && census.surface_runtime_empty == Some(true)
        && census.pending_terminal_ticket_count == 0
        && census.failed_terminal_ticket_count == 0
        && unsafe { GetCapture() } == HWND::default()
        && !unsafe { IsWindow(Some(scenario.source_hwnd)).as_bool() }
        && !unsafe { IsWindow(Some(scenario.target_hwnd)).as_bool() }
        && native_observation_is_terminal(&scenario.native_observation)
}

fn native_observation_is_terminal(observation: &NativeWindowTestObservation) -> bool {
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
    observed.is_subset(&terminal)
}

async fn shutdown_native_worker_application(
    cx: &mut AsyncApp,
    scenario: &NativeDockScenario,
) -> Result<process_harness::NativeWorkerAppCensus> {
    cx.update(|app| app.shutdown_for_native_exit_test());
    wait_until(cx, "native worker application shutdown", |cx| {
        Ok(native_worker_application_is_converged(
            &capture_worker_app_census(cx, scenario),
            scenario,
        ))
    })
    .await?;
    Ok(capture_worker_app_census(cx, scenario))
}

async fn shutdown_native_worker_application_without_surface(cx: &mut AsyncApp) -> Result<()> {
    cx.update(|app| app.shutdown_for_native_exit_test());
    wait_until(
        cx,
        "native worker application shutdown after scenario construction failure",
        |cx| {
            let census = capture_worker_app_census_without_surface(cx);
            Ok(census.window_registry_count == 0
                && !census.active_drag
                && census.native_exit_authority_settled
                && unsafe { GetCapture() } == HWND::default())
        },
    )
    .await
}

fn native_generation_label(window: open_gpui_windows::NativeWindowTestIdentity) -> String {
    format!("{:?}:{}", window.window_id(), window.native_generation())
}

fn ensure_native_interactive_runner(scenario_id: &str) {
    assert!(
        std::env::var("OPEN_GPUI_NATIVE_INTERACTIVE")
            .ok()
            .is_some_and(|value| value == "1"),
        "scenario `{scenario_id}` requires OPEN_GPUI_NATIVE_INTERACTIVE=1 on the named ephemeral runner"
    );
}

fn build_native_scenario(
    cx: &mut App,
    native_observation: NativeWindowTestObservation,
) -> Result<NativeDockScenario> {
    let display = cx
        .primary_display()
        .context("the native Dock scenario requires a primary display")?;
    let display_bounds = display.bounds();
    let gap = 32.0;
    let window_width = ((display_bounds.size.width.as_f32() - gap - 64.0) / 2.0).min(640.0);
    let window_height = (display_bounds.size.height.as_f32() - 80.0).min(520.0);
    ensure!(
        window_width >= 420.0 && window_height >= 360.0,
        "the native Dock runner display is too small for two non-overlapping HWNDs: {display_bounds:?}"
    );
    let total_width = window_width * 2.0 + gap;
    let left =
        display_bounds.origin.x + px((display_bounds.size.width.as_f32() - total_width) / 2.0);
    let top =
        display_bounds.origin.y + px((display_bounds.size.height.as_f32() - window_height) / 2.0);
    let target_bounds = Bounds::new(point(left, top), size(px(window_width), px(window_height)));
    let source_bounds = Bounds::new(
        point(left + px(window_width + gap), top),
        size(px(window_width), px(window_height)),
    );
    let central_bounds = Bounds::new(point(left, top), size(px(window_width), px(window_height)));
    let placement = saved_viewport_placement(target_bounds, source_bounds, central_bounds);
    let surface_slot = Rc::new(RefCell::new(None));
    let surface = build_managed_surface(
        Rc::clone(&surface_slot),
        placement.clone(),
        target_bounds,
        source_bounds,
        central_bounds,
        cx,
    );
    surface_slot.replace(Some(surface.clone()));

    let target_window = match surface.open_primary_window(
        restored_viewport_options(&placement, SPACE, target_bounds),
        cx,
    ) {
        DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
        outcome => bail!("native Dock target HWND failed to open: {outcome:?}"),
    };
    let source_window = match surface.viewports().open(
        SECONDARY_SPACE,
        restored_viewport_options(&placement, SECONDARY_SPACE, source_bounds),
        cx,
    ) {
        DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
        outcome => bail!("native Dock source HWND failed to open: {outcome:?}"),
    };

    let target_hwnd = raw_hwnd(cx, target_window)?;
    let source_hwnd = raw_hwnd(cx, source_window)?;
    ensure!(
        source_hwnd != target_hwnd,
        "the native Dock scenario must own two distinct HWNDs"
    );

    let trace = Arc::new(Mutex::new(NativeMouseTrace::default()));
    let source_trace = Arc::clone(&trace);
    let source_interceptor = cx.update_window(source_window, move |_, window, _| {
        window.intercept_window_mouse_events(move |event, _, _| {
            let kind = match event {
                WindowMouseEvent::Move(_) => Some(NativeMouseKind::Move),
                WindowMouseEvent::Down(_) => Some(NativeMouseKind::Down),
                WindowMouseEvent::Up(_) => Some(NativeMouseKind::Up),
                _ => None,
            };
            if let Some(kind) = kind
                && let Ok(mut trace) = source_trace.lock()
            {
                trace.source.push(kind);
            }
        })
    })?;
    let target_trace = Arc::clone(&trace);
    let target_interceptor = cx.update_window(target_window, move |_, window, _| {
        window.intercept_window_mouse_events(move |event, _, _| {
            let kind = match event {
                WindowMouseEvent::Move(_) => Some(NativeMouseKind::Move),
                WindowMouseEvent::Down(_) => Some(NativeMouseKind::Down),
                WindowMouseEvent::Up(_) => Some(NativeMouseKind::Up),
                _ => None,
            };
            if let Some(kind) = kind
                && let Ok(mut trace) = target_trace.lock()
            {
                trace.target.push(kind);
            }
        })
    })?;
    let initial_revision = surface.export_snapshot(cx).revision();
    let initial_window_count = cx.windows().len();
    let initial_owned_window_count = surface
        .viewports()
        .runtime_status(cx)
        .window_ownership
        .owned_window_count;
    let surface_changes = Rc::new(RefCell::new(Vec::new()));
    let surface_changes_for_subscription = Rc::clone(&surface_changes);
    let surface_change_subscription = surface.subscribe_changes(cx, move |event, _| {
        surface_changes_for_subscription
            .borrow_mut()
            .push(event.clone());
    });

    Ok(NativeDockScenario {
        surface,
        source_window,
        target_window,
        source_hwnd,
        target_hwnd,
        target_bounds,
        initial_revision,
        initial_window_count,
        initial_owned_window_count,
        trace,
        surface_changes,
        native_observation,
        _source_interceptor: source_interceptor,
        _target_interceptor: target_interceptor,
        _surface_change_subscription: surface_change_subscription,
    })
}

async fn run_captured_host_drop_scenario(
    cx: &mut AsyncApp,
    scenario: &mut NativeDockScenario,
    scenario_id: &str,
) -> Result<()> {
    let mut pointer_guard = begin_native_captured_drag(cx, scenario).await?;
    let target_selector_prefix = format!("dock:{SPACE}:tabs:");
    let target_bounds = cx.update(|app| {
        unique_matching_debug_bounds(
            app,
            scenario.target_window,
            &target_selector_prefix,
            ":tab:editor",
        )
    })?;
    let target_scale = cx.update(|app| window_scale_factor(app, scenario.target_window))?;
    let target_point =
        logical_client_point_to_screen(scenario.target_hwnd, target_bounds.center(), target_scale)?;

    let source_moves_before_crossing = scenario
        .trace
        .lock()
        .map_err(|_| anyhow!("native mouse trace lock was poisoned"))?
        .source
        .iter()
        .filter(|kind| **kind == NativeMouseKind::Move)
        .count();
    inject_system_pointer(target_point, MOUSEEVENTF_MOVE)?;
    wait_for_cursor(cx, target_point, "captured move over the target HWND").await?;
    wait_until(cx, "target HWND committed Dock drop preview", |cx| {
        let preview = cx.update(|app| {
            exact_debug_bounds(
                app,
                scenario.target_window,
                &format!("dock:{SPACE}:drop-preview"),
            )
        })?;
        let source_moves = scenario
            .trace
            .lock()
            .map_err(|_| anyhow!("native mouse trace lock was poisoned"))?
            .source
            .iter()
            .filter(|kind| **kind == NativeMouseKind::Move)
            .count();
        Ok(preview
            .is_some_and(|bounds| bounds.size.width > px(0.0) && bounds.size.height > px(0.0))
            && source_moves > source_moves_before_crossing)
    })
    .await?;
    ensure!(
        unsafe { GetCapture() } == scenario.source_hwnd,
        "the real target preview must be routed while the source HWND still owns capture"
    );

    inject_system_pointer(target_point, MOUSEEVENTF_LEFTUP)?;
    pointer_guard.primary_button_down = false;
    wait_until(
        cx,
        "captured release and durable cross-HWND Dock drop",
        |cx| {
            let committed = cx.update(|app| {
                let graph =
                    DockGraph::import_layout(scenario.surface.export_snapshot(app).layout())?;
                Ok::<_, anyhow::Error>(
                    graph
                        .find_item_in_space(
                            &DockSpaceId::from(SECONDARY_SPACE),
                            &DockItemId::from("preview"),
                        )
                        .is_none()
                        && graph
                            .find_item_in_space(
                                &DockSpaceId::from(SPACE),
                                &DockItemId::from("preview"),
                            )
                            .is_some(),
                )
            })?;
            Ok(committed && unsafe { GetCapture() } == HWND::default())
        },
    )
    .await?;

    let snapshot = cx.update(|app| scenario.surface.export_snapshot(app));
    ensure!(
        snapshot.revision() > scenario.initial_revision,
        "the cross-HWND drop must publish a newer durable surface revision"
    );
    let trace = scenario
        .trace
        .lock()
        .map_err(|_| anyhow!("native mouse trace lock was poisoned"))?;
    ensure!(
        trace.source.contains(&NativeMouseKind::Down)
            && trace.source.contains(&NativeMouseKind::Up)
            && trace
                .source
                .iter()
                .filter(|kind| **kind == NativeMouseKind::Move)
                .count()
                >= 2,
        "the source HWND did not receive the complete captured system-input sequence: {trace:?}"
    );
    assert_source_only_native_capture_trace(scenario, target_point)?;
    ensure!(
        !trace.target.contains(&NativeMouseKind::Down)
            && !trace.target.contains(&NativeMouseKind::Up),
        "captured button input must not be replayed into the target GPUI window: {trace:?}"
    );
    // GPUI may synthesize target hover moves after topology or layout changes. The typed native
    // observation above is the authority for proving that WndProc kept raw captured input on the
    // source HWND through release.
    ensure!(
        unsafe { IsWindow(Some(scenario.source_hwnd)).as_bool() }
            && unsafe { IsWindow(Some(scenario.target_hwnd)).as_bool() },
        "both exact HWNDs must remain live through durable drop completion"
    );
    log::info!(
        "scenario={} source_hwnd={:?} target_hwnd={:?} revision={} completed",
        scenario_id,
        scenario.source_hwnd,
        scenario.target_hwnd,
        snapshot.revision()
    );
    Ok(())
}

async fn run_no_input_pass_through_scenario(
    cx: &mut AsyncApp,
    scenario: &mut NativeDockScenario,
    scenario_id: &str,
) -> Result<()> {
    let first_overlay =
        cx.update(|app| open_no_input_overlay(app, scenario.target_bounds, "no-input prefix 1"))?;
    let second_overlay =
        cx.update(|app| open_no_input_overlay(app, scenario.target_bounds, "no-input prefix 2"))?;
    let first_overlay_hwnd = first_overlay.hwnd;
    let second_overlay_hwnd = second_overlay.hwnd;
    wait_until(cx, "two no-input GPUI HWNDs to become transparent", |cx| {
        cx.update(|app| {
            for overlay in [&first_overlay, &second_overlay] {
                let accepts_pointer_input = app.update_window(overlay.window, |_, window, _| {
                    window.accepts_pointer_input()
                })?;
                if accepts_pointer_input || !unsafe { IsWindowVisible(overlay.hwnd).as_bool() } {
                    return Ok(false);
                }
            }
            Ok(true)
        })
    })
    .await?;
    native_test_acquire_foreground_window(scenario.target_hwnd.0 as isize)?;
    raise_native_window(first_overlay.hwnd)?;
    raise_native_window(second_overlay.hwnd)?;
    ensure!(
        native_window_is_above(second_overlay.hwnd, first_overlay.hwnd)
            && native_window_is_above(first_overlay.hwnd, scenario.target_hwnd),
        "the owning-platform stack must contain two consecutive no-input GPUI HWNDs above the lower GPUI target"
    );

    let target_point = cx.update(|app| target_editor_screen_point(app, scenario))?;
    let drift_point = POINT {
        x: target_point.x.saturating_sub(12),
        y: target_point.y,
    };
    let pass_through_point = target_point;
    let opaque_point = POINT {
        x: target_point.x.saturating_add(12),
        y: target_point.y,
    };
    let opaque_barrier = NativeOpaqueBarrier::prepare_partial_cover(opaque_point)?;
    for overlay in [&first_overlay, &second_overlay] {
        let rect = window_rect(overlay.hwnd)?;
        ensure!(
            [drift_point, pass_through_point, opaque_point]
                .into_iter()
                .all(|point| rect_contains(rect, point)),
            "no-input overlay must cover every owning-platform probe: hwnd={:?}, rect={rect:?}",
            overlay.hwnd
        );
        let native_hit = unsafe {
            SendMessageW(
                overlay.hwnd,
                WM_NCHITTEST,
                Some(WPARAM::default()),
                Some(screen_point_lparam(pass_through_point)),
            )
        };
        ensure!(
            native_hit.0 == HTTRANSPARENT as isize,
            "no-input GPUI HWND must publish HTTRANSPARENT: hwnd={:?}, result={native_hit:?}",
            overlay.hwnd
        );
    }
    ensure!(
        root_window_from_point(pass_through_point) == scenario.target_hwnd,
        "two consecutive no-input GPUI HWNDs must expose the lower GPUI target HWND"
    );

    let source_point = cx.update(|app| source_preview_tab_screen_point(app, scenario))?;
    let mut pointer_guard = begin_native_captured_drag(cx, scenario).await?;
    let generation_drift = arm_native_no_input_generation_drift(second_overlay.window)?;
    inject_system_pointer(drift_point, MOUSEEVENTF_MOVE)?;
    wait_for_cursor(cx, drift_point, "no-input generation-drift probe").await?;
    wait_until(
        cx,
        "captured source WndProc to observe the generation-drift probe",
        |_| Ok(source_native_captured_move_observed(scenario, drift_point)),
    )
    .await?;
    cx.background_executor()
        .timer(Duration::from_millis(20))
        .await;
    ensure!(
        cx.update(|app| target_drop_preview_bounds(app, scenario))?
            .is_none(),
        "a no-input observation generation change must fail closed without publishing a Dock target preview"
    );
    generation_drift.finish()?;

    inject_system_pointer(pass_through_point, MOUSEEVENTF_MOVE)?;
    wait_for_cursor(
        cx,
        pass_through_point,
        "two-window no-input pass-through probe",
    )
    .await?;
    wait_until(cx, "lower GPUI HWND to receive Dock routing", |cx| {
        Ok(cx
            .update(|app| target_drop_preview_bounds(app, scenario))?
            .is_some())
    })
    .await?;

    inject_system_pointer(source_point, MOUSEEVENTF_MOVE)?;
    wait_until(
        cx,
        "Dock preview to clear before opaque-terminal probe",
        |cx| {
            Ok(cx
                .update(|app| target_drop_preview_bounds(app, scenario))?
                .is_none())
        },
    )
    .await?;
    opaque_barrier.present()?;
    raise_native_window(first_overlay.hwnd)?;
    raise_native_window(second_overlay.hwnd)?;
    ensure!(
        unsafe { GetCapture() } == scenario.source_hwnd && cx.update(|app| app.has_active_drag()),
        "presenting the opaque terminal must not terminate the captured Dock drag"
    );
    ensure!(
        native_window_is_above(second_overlay.hwnd, first_overlay.hwnd)
            && native_window_is_above(first_overlay.hwnd, opaque_barrier.hwnd)
            && native_window_is_above(opaque_barrier.hwnd, scenario.target_hwnd),
        "the owning-platform stack must be no-input/no-input/opaque/GPUI at the terminal probe"
    );
    ensure!(
        root_window_from_point(opaque_point) == opaque_barrier.hwnd,
        "two no-input GPUI HWNDs must pass through to the lower external opaque HWND"
    );
    inject_system_pointer(opaque_point, MOUSEEVENTF_MOVE)?;
    wait_for_cursor(cx, opaque_point, "opaque terminal below no-input prefix").await?;
    wait_until(
        cx,
        "captured source WndProc to observe the opaque-terminal probe",
        |_| Ok(source_native_captured_move_observed(scenario, opaque_point)),
    )
    .await
    .with_context(|| {
        let active_drag = cx.update(|app| app.has_active_drag());
        format!(
            "capture={:?} foreground={:?} source={:?} active_drag={active_drag} native_events={:?}",
            unsafe { GetCapture() },
            unsafe { GetForegroundWindow() },
            scenario.source_hwnd,
            scenario.native_observation.events(),
        )
    })?;
    cx.background_executor()
        .timer(Duration::from_millis(20))
        .await;
    ensure!(
        cx.update(|app| target_drop_preview_bounds(app, scenario))?
            .is_none(),
        "an external opaque terminal below a no-input prefix must block the lower GPUI target"
    );
    drop(opaque_barrier);

    inject_system_pointer(source_point, MOUSEEVENTF_MOVE)?;
    inject_system_pointer(source_point, MOUSEEVENTF_LEFTUP)?;
    pointer_guard.primary_button_down = false;
    wait_until(cx, "no-input scenario drag and capture to settle", |cx| {
        Ok(!cx.update(|app| app.has_active_drag()) && unsafe { GetCapture() } == HWND::default())
    })
    .await?;
    cx.update(|app| close_no_input_overlay(app, first_overlay))?;
    cx.update(|app| close_no_input_overlay(app, second_overlay))?;
    wait_until(cx, "no-input overlay HWNDs to retire", |cx| {
        Ok(
            cx.update(|app| app.windows().len()) == scenario.initial_window_count
                && !unsafe { IsWindow(Some(first_overlay_hwnd)).as_bool() }
                && !unsafe { IsWindow(Some(second_overlay_hwnd)).as_bool() },
        )
    })
    .await?;

    log::info!(
        "scenario={} source_hwnd={:?} target_hwnd={:?} completed",
        scenario_id,
        scenario.source_hwnd,
        scenario.target_hwnd
    );
    Ok(())
}

fn open_no_input_overlay(
    cx: &mut App,
    bounds: Bounds<Pixels>,
    title: &str,
) -> Result<NativeNoInputOverlay> {
    let mut options = viewport_window_options(bounds);
    options.focus_on_appearing = false;
    if let Some(titlebar) = options.titlebar.as_mut() {
        titlebar.title = Some(title.to_owned().into());
    }
    let window: AnyWindowHandle = cx.open_window(options, |_, cx| cx.new(|_| Empty))?.into();
    let dispatch = cx.update_window(window, |_, window, _| {
        window.set_accepts_pointer_input(false)
    })?;
    ensure!(
        matches!(
            dispatch,
            WindowMutationDispatch::Queued(_) | WindowMutationDispatch::Unchanged
        ),
        "no-input overlay pointer policy could not be committed: {dispatch:?}"
    );
    let hwnd = raw_hwnd(cx, window)?;
    Ok(NativeNoInputOverlay { window, hwnd })
}

fn close_no_input_overlay(cx: &mut App, overlay: NativeNoInputOverlay) -> Result<()> {
    cx.update_window(overlay.window, |_, window, cx| window.remove_window(cx))?;
    Ok(())
}

fn raise_native_window(hwnd: HWND) -> Result<()> {
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        )
    }
    .context("failed to raise native test window in the normal Z-order band")
}

fn target_editor_screen_point(cx: &mut App, scenario: &NativeDockScenario) -> Result<POINT> {
    let bounds = unique_matching_debug_bounds(
        cx,
        scenario.target_window,
        &format!("dock:{SPACE}:tabs:"),
        ":tab:editor",
    )?;
    let scale = window_scale_factor(cx, scenario.target_window)?;
    logical_client_point_to_screen(scenario.target_hwnd, bounds.center(), scale)
}

fn source_preview_tab_screen_point(cx: &mut App, scenario: &NativeDockScenario) -> Result<POINT> {
    let bounds = unique_matching_debug_bounds(
        cx,
        scenario.source_window,
        &format!("dock:{SECONDARY_SPACE}:tabs:"),
        ":tab:preview",
    )?;
    let scale = window_scale_factor(cx, scenario.source_window)?;
    logical_client_point_to_screen(scenario.source_hwnd, bounds.center(), scale)
}

fn target_drop_preview_bounds(
    cx: &mut App,
    scenario: &NativeDockScenario,
) -> Result<Option<Bounds<Pixels>>> {
    exact_debug_bounds(
        cx,
        scenario.target_window,
        &format!("dock:{SPACE}:drop-preview"),
    )
}

async fn begin_native_captured_drag(
    cx: &mut AsyncApp,
    scenario: &mut NativeDockScenario,
) -> Result<SystemPointerGuard> {
    ensure!(
        unsafe { GetCapture() } == HWND::default(),
        "the native Dock runner started with an unrelated capture owner"
    );
    if unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) } as u16 & 0x8000 != 0 {
        let _ = inject_primary_button_up_best_effort();
        bail!("the native Dock runner started with the primary pointer button pressed");
    }
    let mut pointer_guard = SystemPointerGuard::capture(scenario.source_hwnd)?;

    wait_until(cx, "both Dock HWNDs to present non-empty frames", |cx| {
        cx.update(|app| {
            Ok(window_has_non_empty_frame(app, scenario.source_window)?
                && window_has_non_empty_frame(app, scenario.target_window)?)
        })
    })
    .await
    .with_context(|| {
        cx.update(|app| {
            let source = app.update_window(scenario.source_window, |_, window, _| {
                (window.presentation_facts(), window.platform_facts().clone())
            });
            let target = app.update_window(scenario.target_window, |_, window, _| {
                (window.presentation_facts(), window.platform_facts().clone())
            });
            format!("source={source:?} target={target:?}")
        })
    })?;
    ensure!(
        unsafe { IsWindowVisible(scenario.source_hwnd).as_bool() }
            && unsafe { IsWindowVisible(scenario.target_hwnd).as_bool() },
        "both Dock HWNDs must be natively visible before input injection"
    );
    let source_selector_prefix = format!("dock:{SECONDARY_SPACE}:tabs:");
    let source_bounds = cx.update(|app| {
        unique_matching_debug_bounds(
            app,
            scenario.source_window,
            &source_selector_prefix,
            ":tab:preview",
        )
    })?;
    let source_scale = cx.update(|app| window_scale_factor(app, scenario.source_window))?;
    let source_point =
        logical_client_point_to_screen(scenario.source_hwnd, source_bounds.center(), source_scale)?;
    let threshold_point = logical_client_point_to_screen(
        scenario.source_hwnd,
        point(
            source_bounds.center().x + px(24.0),
            source_bounds.center().y,
        ),
        source_scale,
    )?;

    *scenario
        .trace
        .lock()
        .map_err(|_| anyhow!("native mouse trace lock was poisoned"))? =
        NativeMouseTrace::default();
    scenario.native_observation.clear();

    native_test_acquire_foreground_window(scenario.source_hwnd.0 as isize)?;
    ensure!(
        root_window_from_point(source_point) == scenario.source_hwnd,
        "the prepared source input point is occluded by a different native HWND"
    );
    inject_system_pointer(source_point, MOUSEEVENTF_MOVE)?;
    wait_for_cursor(cx, source_point, "source tab pointer move").await?;
    wait_until(cx, "source WndProc system-pointer move admission", |_| {
        Ok(native_system_mouse_message_observed(
            &scenario.native_observation,
            scenario.source_window,
            NativeWindowTestMessage::MouseMove,
            source_point,
        ))
    })
    .await?;
    pointer_guard.primary_button_down = true;
    inject_system_pointer(source_point, MOUSEEVENTF_LEFTDOWN)?;
    wait_until(cx, "source HWND native pointer-capture transition", |_| {
        let captured = unsafe { GetCapture() } == scenario.source_hwnd;
        let observed_down = scenario
            .trace
            .lock()
            .map_err(|_| anyhow!("native mouse trace lock was poisoned"))?
            .source
            .contains(&NativeMouseKind::Down);
        Ok(captured
            && observed_down
            && source_native_primary_button_down_captured_observed(scenario, source_point))
    })
    .await
    .with_context(|| {
        let trace = scenario
            .trace
            .lock()
            .map(|trace| format!("{trace:?}"))
            .unwrap_or_else(|_| "<poisoned>".to_owned());
        format!(
            "capture={:?} foreground={:?} source={:?} trace={trace} native_events={:?}",
            unsafe { GetCapture() },
            unsafe { GetForegroundWindow() },
            scenario.source_hwnd,
            scenario.native_observation.events(),
        )
    })?;

    inject_system_pointer(threshold_point, MOUSEEVENTF_MOVE)?;
    wait_until(cx, "GPUI Dock drag activation from native movement", |cx| {
        Ok(cx.update(|app| app.has_active_drag()))
    })
    .await?;
    ensure!(
        unsafe { GetCapture() } == scenario.source_hwnd,
        "the source HWND must retain native capture after Dock drag activation"
    );
    Ok(pointer_guard)
}

async fn run_provisional_same_hwnd_promotion_scenario(
    cx: &mut AsyncApp,
    scenario: &mut NativeDockScenario,
    behavior: NativeDockBehavior,
    scenario_id: &str,
) -> Result<()> {
    ensure!(
        matches!(
            behavior,
            NativeDockBehavior::OpaqueOcclusion
                | NativeDockBehavior::ProvisionalSameHwndPromotion
                | NativeDockBehavior::CommittedLossRecovery
                | NativeDockBehavior::MixedDpiPlacement
        ),
        "scenario `{scenario_id}` is not a provisional-promotion behavior"
    );
    let mixed_dpi_target = if behavior.crosses_dpi_boundary() {
        let source_facts = cx.update(|app| window_platform_facts(app, scenario.source_window))?;
        let source_display_id = source_facts
            .display_id
            .context("mixed-DPI live-undock requires a source display identity")?;
        let displays = native_test_displays();
        let source_display = displays
            .iter()
            .copied()
            .find(|display| display.display_id() == source_display_id)
            .with_context(|| {
                format!(
                    "mixed-DPI live-undock could not resolve source display {source_display_id:?}: {displays:?}"
                )
            })?;
        let target_display = displays
            .iter()
            .copied()
            .find(|display| {
                display.display_id() != source_display_id
                    && (display.scale_factor() - source_display.scale_factor()).abs() > 0.001
            })
            .with_context(|| {
                format!(
                    "mixed-DPI live-undock requires two real displays with distinct effective DPI: {displays:?}"
                )
            })?;
        Some((source_display, target_display))
    } else {
        None
    };
    let reveal_point = if let Some((source_display, _)) = mixed_dpi_target {
        open_desktop_release_point_on_display(
            scenario.source_hwnd,
            scenario.target_hwnd,
            source_display,
            None,
        )?
    } else {
        open_desktop_release_point(scenario.source_hwnd, scenario.target_hwnd)?
    };
    let release_point = if let Some((_, target_display)) = mixed_dpi_target {
        open_desktop_release_point_on_display(
            scenario.source_hwnd,
            scenario.target_hwnd,
            target_display,
            Some(reveal_point),
        )?
    } else if behavior.uses_opaque_underlay() {
        open_desktop_release_point_away_from(
            scenario.source_hwnd,
            scenario.target_hwnd,
            reveal_point,
        )?
    } else {
        reveal_point
    };
    let mut mixed_dpi_exact_placement: Option<(Bounds<DevicePixels>, DisplayId, f32)> = None;
    let opaque_barrier = behavior
        .uses_opaque_underlay()
        .then(|| NativeOpaqueBarrier::prepare_partial_cover(release_point))
        .transpose()?;
    let mut pointer_guard = begin_native_captured_drag(cx, scenario).await?;
    let pre_tear_off_snapshot = cx.update(|app| scenario.surface.export_snapshot(app));
    let pre_tear_off_revision = pre_tear_off_snapshot.revision();
    let pre_tear_off_runtime = cx.update(|app| scenario.surface.viewports().runtime_status(app));
    let pre_tear_off_source_facts =
        cx.update(|app| window_platform_facts(app, scenario.source_window))?;
    scenario.surface_changes.borrow_mut().clear();
    let registered_before_release =
        cx.update(|app| scenario.surface.registered_viewport_spaces(app));

    inject_system_pointer(reveal_point, MOUSEEVENTF_MOVE)?;
    wait_for_cursor(
        cx,
        reveal_point,
        "captured move onto the open-desktop provisional reveal point",
    )
    .await?;

    let provisional_trace = Arc::new(Mutex::new(Vec::new()));
    let mut provisional_interceptor = None;
    let mut provisional_window = None;
    let last_provisional_observation = Rc::new(RefCell::new(String::new()));
    let last_observation_for_wait = Rc::clone(&last_provisional_observation);
    wait_until(
        cx,
        "a visible provisional HWND with a non-empty presented frame before release",
        |cx| {
            let candidate = cx.update(|app| {
                app.windows().into_iter().find(|window| {
                    *window != scenario.source_window && *window != scenario.target_window
                })
            });
            let Some(candidate) = candidate else {
                let mut cursor = POINT::default();
                let _ = unsafe { GetCursorPos(&mut cursor) };
                let trace = scenario
                    .trace
                    .lock()
                    .map(|trace| format!("{trace:?}"))
                    .unwrap_or_else(|_| "<poisoned>".to_string());
                *last_observation_for_wait.borrow_mut() = format!(
                    "no third window; reveal_point={reveal_point:?} release_point={release_point:?} cursor={cursor:?} capture={:?} foreground={:?} trace={trace} runtime={:?}",
                    unsafe { GetCapture() },
                    unsafe { GetForegroundWindow() },
                    cx.update(|app| scenario.surface.viewports().runtime_status(app))
                );
                return Ok(false);
            };
            let runtime_status = cx.update(|app| scenario.surface.viewports().runtime_status(app));
            let (presentation, facts, visible) = cx.update(|app| {
                app.update_window(candidate, |_, window, _| {
                    (
                        window.presentation_facts(),
                        window.platform_facts().clone(),
                        window.presentation_facts().native_visible,
                    )
                })
            })?;
            *last_observation_for_wait.borrow_mut() = format!(
                "candidate={:?} visible={visible} presentation={presentation:?} facts={facts:?} runtime={runtime_status:?}",
                candidate.window_id(),
            );
            if runtime_status.window_ownership.owned_window_count
                != scenario.initial_owned_window_count + 1
                || runtime_status.window_ownership.opening_window_count == 0
            {
                return Ok(false);
            }
            if provisional_interceptor.is_none() {
                let trace = Arc::clone(&provisional_trace);
                provisional_interceptor = Some(cx.update(|app| {
                    app.update_window(candidate, move |_, window, _| {
                        window.intercept_window_mouse_events(move |event, _, _| {
                            let kind = match event {
                                WindowMouseEvent::Move(_) => Some(NativeMouseKind::Move),
                                WindowMouseEvent::Down(_) => Some(NativeMouseKind::Down),
                                WindowMouseEvent::Up(_) => Some(NativeMouseKind::Up),
                                _ => None,
                            };
                            if let Some(kind) = kind
                                && let Ok(mut trace) = trace.lock()
                            {
                                trace.push(kind);
                            }
                        })
                    })
                })?);
            }
            if !cx.update(|app| window_has_non_empty_frame(app, candidate))? {
                return Ok(false);
            }
            provisional_window = Some(candidate);
            Ok(true)
        },
    )
    .await
    .with_context(|| {
        format!(
            "last provisional observation: {}",
            last_provisional_observation.borrow()
        )
    })?;
    let provisional_window = provisional_window
        .context("the provisional presentation completed without retaining its exact window")?;
    let provisional_hwnd = cx.update(|app| raw_hwnd(app, provisional_window))?;
    ensure!(
        provisional_hwnd != scenario.source_hwnd && provisional_hwnd != scenario.target_hwnd,
        "the provisional viewport must own a third exact HWND before release"
    );
    ensure!(
        unsafe { IsWindowVisible(provisional_hwnd).as_bool() },
        "the provisional HWND must be natively visible before release"
    );
    ensure!(
        point_is_open_desktop(reveal_point),
        "the initial provisional reveal must be scoped to verified open desktop"
    );
    ensure!(
        unsafe { GetCapture() } == scenario.source_hwnd,
        "the source HWND must retain capture while the provisional is visibly presented"
    );
    let pre_release_window_count = cx.update(|app| app.windows().len());
    ensure!(
        pre_release_window_count == scenario.initial_window_count + 1,
        "pre-release provisional creation must add exactly one GPUI window: initial={}, current={}",
        scenario.initial_window_count,
        pre_release_window_count
    );

    let provisional_facts = cx.update(|app| window_platform_facts(app, provisional_window))?;
    ensure!(
        provisional_facts.accepts_pointer_input
            && provisional_facts.accepts_activation
            && provisional_facts.focus_on_click
            && !provisional_facts.is_active,
        "the provisional must retain its final peer-window native profile without becoming active: {provisional_facts:?}"
    );
    let provisional_rect = window_rect(provisional_hwnd)?;
    let provisional_hit_point = POINT {
        x: midpoint(provisional_rect.left, provisional_rect.right),
        y: midpoint(provisional_rect.top, provisional_rect.bottom),
    };
    let gated_hit = unsafe {
        SendMessageW(
            provisional_hwnd,
            WM_NCHITTEST,
            Some(WPARAM::default()),
            Some(screen_point_lparam(provisional_hit_point)),
        )
    };
    ensure!(
        gated_hit.0 == HTTRANSPARENT as isize,
        "the visible provisional HWND must remain natively hit-transparent before release: result={gated_hit:?}"
    );
    let gated_mouse_activation = unsafe {
        SendMessageW(
            provisional_hwnd,
            WM_MOUSEACTIVATE,
            Some(WPARAM::default()),
            Some(LPARAM::default()),
        )
    };
    ensure!(
        gated_mouse_activation.0 == MA_NOACTIVATEANDEAT as isize,
        "the visible provisional HWND must reject native mouse activation before release: result={gated_mouse_activation:?}"
    );
    let provisional_tab = cx.update(|app| {
        unique_matching_debug_bounds(app, provisional_window, "dock:", ":tab:preview")
    })?;
    ensure!(
        provisional_tab.size.width > px(0.0) && provisional_tab.size.height > px(0.0),
        "the provisional's non-empty presented frame must contain the real payload tab"
    );

    let pre_release_snapshot = cx.update(|app| scenario.surface.export_snapshot(app));
    let pre_release_changes = scenario.surface_changes.borrow().clone();
    ensure!(
        pre_release_snapshot.revision() == pre_tear_off_revision,
        "opening and presenting a provisional must not publish durable surface topology after drag activation: baseline={}, current={}, changes={pre_release_changes:?}",
        pre_tear_off_revision,
        pre_release_snapshot.revision()
    );
    let pre_release_graph = DockGraph::import_layout(pre_release_snapshot.layout())?;
    ensure!(
        pre_release_graph
            .find_item_in_space(
                &DockSpaceId::from(SECONDARY_SPACE),
                &DockItemId::from("preview"),
            )
            .is_some(),
        "the durable graph must retain payload ownership in the source before release"
    );
    let registered_after_presentation =
        cx.update(|app| scenario.surface.registered_viewport_spaces(app));
    ensure!(
        registered_after_presentation == registered_before_release,
        "a provisional must not publish a durable viewport registration before release: before={registered_before_release:?}, after={registered_after_presentation:?}"
    );

    let _provisional_interceptor = provisional_interceptor
        .context("the provisional HWND disappeared before its input gate could be observed")?;
    ensure!(
        provisional_trace
            .lock()
            .map_err(|_| anyhow!("provisional native mouse trace lock was poisoned"))?
            .is_empty(),
        "bootstrapping a visible provisional must not synthesize raw pointer input"
    );

    if behavior.uses_opaque_underlay() {
        let barrier = opaque_barrier
            .as_ref()
            .context("the opaque-underlay behavior lost its prepared native barrier")?;
        barrier.present()?;
        ensure!(
            unsafe { GetCapture() } == scenario.source_hwnd
                && cx.update(|app| app.has_active_drag()),
            "presenting the opaque underlay must not terminate the captured Dock drag"
        );
        ensure!(
            root_window_from_point(release_point) == barrier.hwnd,
            "the second captured point must resolve to the exact ordinary opaque barrier before release"
        );
        inject_system_pointer(release_point, MOUSEEVENTF_MOVE)?;
        wait_for_cursor(
            cx,
            release_point,
            "captured move from the reveal point onto the final opaque-barrier release point",
        )
        .await?;
        wait_until(
            cx,
            "the source WndProc observed the final captured move before release",
            |_| {
                Ok(source_native_captured_move_observed(
                    scenario,
                    release_point,
                ))
            },
        )
        .await?;
    } else if mixed_dpi_target.is_some() {
        ensure!(
            cx.update(|app| {
                native_captured_release_placement_for_test(scenario.source_window.window_id(), app)
            })
            .is_none(),
            "mixed-DPI release placement must not exist before the primary-button release is adopted"
        );
        inject_system_pointer(release_point, MOUSEEVENTF_MOVE)?;
        wait_for_cursor(
            cx,
            release_point,
            "captured move from the source-DPI reveal point to the target-DPI release point",
        )
        .await?;
        wait_until(
            cx,
            "the source WndProc observed the target-DPI captured move",
            |_| {
                Ok(source_native_captured_move_observed(
                    scenario,
                    release_point,
                ))
            },
        )
        .await?;
    }

    inject_system_pointer(release_point, MOUSEEVENTF_LEFTUP)?;
    pointer_guard.primary_button_down = false;
    wait_until(
        cx,
        "captured primary-button release reached the source WndProc",
        |_| {
            Ok(source_native_primary_button_up_observed(
                scenario,
                release_point,
            ))
        },
    )
    .await
    .with_context(|| {
        format!(
            "native input observations: {:?}",
            scenario.native_observation.events()
        )
    })?;
    assert_source_only_native_capture_trace(scenario, release_point)?;
    if let Some((_, target_display)) = mixed_dpi_target {
        wait_until(
            cx,
            "the adopted mixed-DPI release to publish its exact placement intent",
            |cx| {
                Ok(cx
                    .update(|app| {
                        native_captured_release_placement_for_test(
                            scenario.source_window.window_id(),
                            app,
                        )
                    })
                    .is_some())
            },
        )
        .await?;
        let expected_bounds = cx
            .update(|app| {
                native_captured_release_placement_for_test(scenario.source_window.window_id(), app)
            })
            .context("the adopted mixed-DPI release did not expose its placement intent")?;
        mixed_dpi_exact_placement = Some((
            expected_bounds,
            target_display.display_id(),
            target_display.scale_factor(),
        ));
        wait_until(
            cx,
            "the provisional HWND to converge exact client bounds on the target-DPI display",
            |cx| {
                let facts = cx.update(|app| window_platform_facts(app, provisional_window))?;
                let Some(geometry) = facts.physical_geometry else {
                    return Ok(false);
                };
                Ok(geometry.client_bounds() == expected_bounds
                    && facts.display_id == Some(target_display.display_id())
                    && (geometry.scale_factor() - target_display.scale_factor()).abs() <= 0.001
                    && geometry.client_bounds().size.width.0 > 0
                    && geometry.client_bounds().size.height.0 > 0)
            },
        )
        .await?;
    }
    let mut promoted_space = None;
    let last_promotion_observation = Rc::new(RefCell::new(String::new()));
    let last_observation_for_wait = Rc::clone(&last_promotion_observation);
    wait_until(
        cx,
        "same-HWND provisional promotion and interaction-gate release",
        |cx| {
            let (promoted, observation) = cx.update(|app| -> Result<_> {
                let registered_spaces = scenario.surface.registered_viewport_spaces(app);
                let new_space = registered_spaces
                    .iter()
                    .find(|space| {
                        **space != DockSpaceId::from(SPACE)
                            && **space != DockSpaceId::from(SECONDARY_SPACE)
                    })
                    .cloned();
                let snapshot = scenario.surface.export_snapshot(app);
                let graph = DockGraph::import_layout(snapshot.layout())?;
                let runtime_status = scenario.surface.viewports().runtime_status(app);
                let facts = window_platform_facts(app, provisional_window)?;
                let source_facts = window_platform_facts(app, scenario.source_window)?;
                let capture = unsafe { GetCapture() };
                let Some(new_space) = new_space else {
                    return Ok((
                        None,
                        format!(
                            "revision={} registered={registered_spaces:?} capture={capture:?} active_drag={} facts={facts:?} source_facts={source_facts:?} runtime={runtime_status:?} pre_tear_off_source_facts={pre_tear_off_source_facts:?} pre_tear_off_runtime={pre_tear_off_runtime:?}",
                            snapshot.revision(),
                            app.has_active_drag(),
                        ),
                    ));
                };
                let exact_window_registered =
                    runtime_status.viewport_lifecycle.iter().any(|record| {
                        record.space == new_space
                            && record.window_id == provisional_window.window_id()
                    });
                let payload_committed = graph
                    .find_item_in_space(&new_space, &DockItemId::from("preview"))
                    .is_some()
                    && graph
                        .find_item_in_space(
                            &DockSpaceId::from(SECONDARY_SPACE),
                            &DockItemId::from("preview"),
                        )
                        .is_none();
                let same_hwnd = raw_hwnd(app, provisional_window)? == provisional_hwnd;
                let promoted = (payload_committed
                    && exact_window_registered
                    && same_hwnd
                    && facts.accepts_pointer_input
                    && facts.accepts_activation
                    && facts.focus_on_click
                    && app.windows().len() == scenario.initial_window_count + 1)
                    .then_some(new_space.clone());
                Ok((
                    promoted,
                    format!(
                        "revision={} registered={registered_spaces:?} new_space={new_space} payload_committed={payload_committed} exact_window_registered={exact_window_registered} same_hwnd={same_hwnd} capture={capture:?} active_drag={} facts={facts:?} source_facts={source_facts:?} runtime={runtime_status:?} pre_tear_off_source_facts={pre_tear_off_source_facts:?} pre_tear_off_runtime={pre_tear_off_runtime:?}",
                        snapshot.revision(),
                        app.has_active_drag(),
                    ),
                ))
            })?;
            *last_observation_for_wait.borrow_mut() = observation;
            if let Some(space) = promoted {
                promoted_space = Some(space);
                return Ok(unsafe { GetCapture() } != scenario.source_hwnd);
            }
            Ok(false)
        },
    )
    .await
    .with_context(|| {
        format!(
            "last promotion observation: {}",
            last_promotion_observation.borrow()
        )
    })?;
    let promoted_space = promoted_space
        .context("same-HWND promotion completed without exposing its committed dock space")?;

    let promoted_snapshot = cx.update(|app| scenario.surface.export_snapshot(app));
    ensure!(
        promoted_snapshot.revision() == pre_tear_off_revision + 1,
        "same-HWND promotion must publish exactly one durable surface transaction after drag activation: baseline={}, current={}",
        pre_tear_off_revision,
        promoted_snapshot.revision()
    );
    ensure!(
        unsafe { IsWindow(Some(provisional_hwnd)).as_bool() }
            && unsafe { IsWindowVisible(provisional_hwnd).as_bool() },
        "the exact provisional HWND must remain live and visible after promotion"
    );
    if let Some((expected_bounds, expected_display, expected_scale)) = mixed_dpi_exact_placement {
        let accepted_before_refresh = cx.update(|app| {
            app.update_window(provisional_window, |_, window, _| {
                let accepted = window.presentation_facts().frame_accepted_generation;
                window.refresh();
                accepted
            })
        })?;
        wait_until(
            cx,
            "the mixed-DPI promoted HWND to accept a later stability frame",
            |cx| {
                let accepted = cx.update(|app| {
                    app.update_window(provisional_window, |_, window, _| {
                        window.presentation_facts().frame_accepted_generation
                    })
                })?;
                Ok(accepted.is_some_and(|accepted| {
                    accepted_before_refresh.is_none_or(|before| accepted > before)
                }))
            },
        )
        .await?;
        let stable_facts = cx.update(|app| window_platform_facts(app, provisional_window))?;
        let stable_geometry = stable_facts
            .physical_geometry
            .context("mixed-DPI promotion lost its exact physical geometry")?;
        ensure!(
            stable_geometry.client_bounds() == expected_bounds
                && stable_facts.display_id == Some(expected_display)
                && (stable_geometry.scale_factor() - expected_scale).abs() <= 0.001,
            "same-HWND mixed-DPI promotion changed the exact release placement: expected_bounds={expected_bounds:?}, expected_display={expected_display:?}, expected_scale={expected_scale}, facts={stable_facts:?}"
        );
    }
    if let Some(barrier) = opaque_barrier.as_ref() {
        barrier.assert_point_scoped_band(provisional_hwnd, release_point)?;
        ensure!(
            window_rect(provisional_hwnd)? != provisional_rect,
            "the final provisional placement must move away from its earlier reveal geometry"
        );
    }
    let promoted_hit = unsafe {
        SendMessageW(
            provisional_hwnd,
            WM_NCHITTEST,
            Some(WPARAM::default()),
            Some(screen_point_lparam(provisional_hit_point)),
        )
    };
    ensure!(
        promoted_hit.0 != HTTRANSPARENT as isize,
        "same-HWND promotion must remove the exact provisional native hit gate"
    );
    {
        let provisional_trace = provisional_trace
            .lock()
            .map_err(|_| anyhow!("provisional native mouse trace lock was poisoned"))?;
        ensure!(
            !provisional_trace.contains(&NativeMouseKind::Move)
                && !provisional_trace.contains(&NativeMouseKind::Down)
                && !provisional_trace.contains(&NativeMouseKind::Up),
            "captured input must never be replayed into the provisional HWND: {provisional_trace:?}"
        );
    }
    let provisional_native_messages = scenario
        .native_observation
        .events()
        .into_iter()
        .filter(|event| event.window().window_id() == provisional_window.window_id())
        .filter(|event| {
            matches!(
                event.kind(),
                NativeWindowTestEventKind::WindowMessage { message, .. }
                    if is_native_mouse_message(message)
            )
        })
        .collect::<Vec<_>>();
    ensure!(
        provisional_native_messages.is_empty(),
        "captured input reached the provisional WndProc despite its interaction gate: {provisional_native_messages:?}"
    );

    scenario.native_observation.clear();
    provisional_trace
        .lock()
        .map_err(|_| anyhow!("provisional native mouse trace lock was poisoned"))?
        .clear();
    let preferred_click_point = {
        let scale_factor = cx.update(|app| window_scale_factor(app, provisional_window))?;
        logical_client_point_to_screen(provisional_hwnd, provisional_tab.center(), scale_factor)?
    };
    let click_point = visible_client_point_for_window(provisional_hwnd, preferred_click_point)?;
    ensure!(
        root_window_from_point(click_point) == provisional_hwnd,
        "the system click must target a visible client point on the exact promoted HWND: point={click_point:?}, promoted={provisional_hwnd:?}"
    );
    inject_system_pointer(click_point, MOUSEEVENTF_MOVE)?;
    wait_for_cursor(
        cx,
        click_point,
        "system pointer movement onto the exact promoted HWND",
    )
    .await?;
    let mut delivered_click_point = POINT::default();
    unsafe { GetCursorPos(&mut delivered_click_point) }
        .context("the native Dock scenario could not sample the promoted click point")?;
    ensure!(
        root_window_from_point(delivered_click_point) == provisional_hwnd,
        "the delivered system pointer must resolve to the exact promoted HWND: requested={click_point:?}, delivered={delivered_click_point:?}, promoted={provisional_hwnd:?}"
    );

    pointer_guard.primary_button_down = true;
    inject_system_pointer(delivered_click_point, MOUSEEVENTF_LEFTDOWN)?;
    wait_until(
        cx,
        "system primary-button down reached the promoted WndProc and GPUI",
        |_| {
            let gpui_down = provisional_trace
                .lock()
                .map_err(|_| anyhow!("provisional native mouse trace lock was poisoned"))?
                .contains(&NativeMouseKind::Down);
            Ok(gpui_down
                && native_system_mouse_message_observed(
                    &scenario.native_observation,
                    provisional_window,
                    NativeWindowTestMessage::PrimaryButtonDown,
                    delivered_click_point,
                ))
        },
    )
    .await?;

    inject_system_pointer(delivered_click_point, MOUSEEVENTF_LEFTUP)?;
    pointer_guard.primary_button_down = false;
    wait_until(
        cx,
        "system primary-button up reached the promoted WndProc and GPUI",
        |_| {
            let trace = provisional_trace
                .lock()
                .map_err(|_| anyhow!("provisional native mouse trace lock was poisoned"))?;
            Ok(trace.contains(&NativeMouseKind::Down)
                && trace.contains(&NativeMouseKind::Up)
                && native_system_mouse_message_observed(
                    &scenario.native_observation,
                    provisional_window,
                    NativeWindowTestMessage::PrimaryButtonUp,
                    delivered_click_point,
                ))
        },
    )
    .await
    .with_context(|| {
        format!(
            "promoted GPUI trace={:?}; native events={:?}",
            provisional_trace.lock(),
            scenario.native_observation.events()
        )
    })?;

    if behavior.closes_committed_destination() {
        unsafe {
            PostMessageW(
                Some(provisional_hwnd),
                WM_CLOSE,
                WPARAM::default(),
                LPARAM::default(),
            )
        }
        .context("the committed-loss scenario could not post WM_CLOSE to the promoted HWND")?;
        let promoted_space_for_wait = promoted_space.clone();
        let mut recovery_entry = None;
        wait_until(
            cx,
            "committed destination loss to expose one visible Restore entry after retiring native authority",
            |cx| {
                let (registered, runtime, window_count, active_drag) = cx.update(|app| {
                    (
                        scenario.surface.registered_viewport_spaces(app),
                        scenario.surface.viewports().runtime_status(app),
                        app.windows().len(),
                        app.has_active_drag(),
                    )
                });
                let entry = cx.update(|app| {
                    unique_recovery_entry_bounds(
                        app,
                        &[scenario.source_window, scenario.target_window],
                    )
                })?;
                let settled = !registered.contains(&promoted_space_for_wait)
                    && runtime.window_ownership.owned_window_count
                        == scenario.initial_owned_window_count
                    && runtime.window_ownership.opening_window_count == 0
                    && runtime.window_ownership.retiring_window_count == 0
                    && window_count == scenario.initial_window_count
                    && !active_drag
                    && unsafe { GetCapture() } == HWND::default()
                    && !unsafe { IsWindow(Some(provisional_hwnd)).as_bool() }
                    && entry.is_some();
                if settled {
                    recovery_entry = entry;
                }
                Ok(settled)
            },
        )
        .await?;
        let (recovery_window, recovery_bounds) = recovery_entry
            .context("committed viewport loss settled without retaining its Restore entry")?;
        let lost_snapshot = cx.update(|app| scenario.surface.export_snapshot(app));
        let retained_graph = DockGraph::import_layout(lost_snapshot.layout())?;
        ensure!(
            retained_graph
                .find_item_in_space(&promoted_space, &DockItemId::from("preview"))
                .is_some(),
            "RetainLayout committed-loss recovery must preserve the promoted payload topology"
        );
        ensure!(
            lost_snapshot.revision() == promoted_snapshot.revision() + 1,
            "committed viewport loss must publish exactly one surface revision: promoted={}, lost={}",
            promoted_snapshot.revision(),
            lost_snapshot.revision()
        );
        ensure!(
            transition_count(
                &scenario.surface_changes.borrow(),
                DockSurfaceTransition::ViewportLostAfterPromotion,
            ) == 1,
            "committed viewport loss must publish exactly one named loss transition: {:?}",
            scenario.surface_changes.borrow()
        );

        scenario.native_observation.clear();
        let recovery_hwnd = cx.update(|app| raw_hwnd(app, recovery_window))?;
        let recovery_scale = cx.update(|app| window_scale_factor(app, recovery_window))?;
        let recovery_point = logical_client_point_to_screen(
            recovery_hwnd,
            recovery_bounds.center(),
            recovery_scale,
        )?;
        ensure!(
            root_window_from_point(recovery_point) == recovery_hwnd,
            "the Restore system click must resolve to the surviving anchor HWND"
        );
        inject_system_pointer(recovery_point, MOUSEEVENTF_MOVE)?;
        wait_for_cursor(
            cx,
            recovery_point,
            "system pointer movement onto the committed Restore entry",
        )
        .await?;
        pointer_guard.primary_button_down = true;
        inject_system_pointer(recovery_point, MOUSEEVENTF_LEFTDOWN)?;
        wait_until(
            cx,
            "Restore primary-button down to reach the anchor WndProc",
            |_| {
                Ok(native_system_mouse_message_observed(
                    &scenario.native_observation,
                    recovery_window,
                    NativeWindowTestMessage::PrimaryButtonDown,
                    recovery_point,
                ))
            },
        )
        .await?;
        inject_system_pointer(recovery_point, MOUSEEVENTF_LEFTUP)?;
        pointer_guard.primary_button_down = false;
        wait_until(
            cx,
            "the native Restore click to re-home the payload and remove its recovery entry",
            |cx| {
                let (snapshot, recovery_entry) = cx.update(|app| {
                    (
                        scenario.surface.export_snapshot(app),
                        unique_recovery_entry_bounds(
                            app,
                            &[scenario.source_window, scenario.target_window],
                        ),
                    )
                });
                let graph = DockGraph::import_layout(snapshot.layout())?;
                Ok(recovery_entry?.is_none()
                    && graph
                        .find_item_in_space(&DockSpaceId::from(SPACE), &DockItemId::from("preview"))
                        .is_some()
                    && graph
                        .find_item_in_space(&promoted_space_for_wait, &DockItemId::from("preview"))
                        .is_none()
                    && snapshot.revision() == lost_snapshot.revision() + 1)
            },
        )
        .await?;
        ensure!(
            native_system_mouse_message_observed(
                &scenario.native_observation,
                recovery_window,
                NativeWindowTestMessage::PrimaryButtonUp,
                recovery_point,
            ),
            "the Restore system click did not deliver primary-button up to the anchor WndProc"
        );
        ensure!(
            transition_count(
                &scenario.surface_changes.borrow(),
                DockSurfaceTransition::ViewportRecovered,
            ) == 1,
            "Restore must publish exactly one named recovery transition: {:?}",
            scenario.surface_changes.borrow()
        );
    }

    log::info!(
        "scenario={} source_hwnd={:?} provisional_hwnd={:?} promoted_space={} revision={} completed",
        scenario_id,
        scenario.source_hwnd,
        provisional_hwnd,
        promoted_space,
        promoted_snapshot.revision()
    );
    Ok(())
}

async fn run_captured_surface_shutdown_scenario(
    cx: &mut AsyncApp,
    scenario: &mut NativeDockScenario,
    behavior: NativeDockBehavior,
    scenario_id: &str,
) -> Result<()> {
    ensure!(
        matches!(
            behavior,
            NativeDockBehavior::SurfaceShutdown | NativeDockBehavior::ProcessConvergence
        ),
        "scenario `{scenario_id}` is not a surface-shutdown behavior"
    );
    let mut pointer_guard = begin_native_captured_drag(cx, scenario).await?;
    unsafe {
        PostMessageW(
            Some(scenario.target_hwnd),
            WM_CLOSE,
            WPARAM::default(),
            LPARAM::default(),
        )
    }
    .context("the native Dock scenario could not post WM_CLOSE to its primary anchor")?;

    wait_until(
        cx,
        "surface-owned capture cancellation and dependent-before-anchor convergence",
        |cx| {
            let (session_closed, runtime_empty, no_gpui_windows, active_drag, tickets_settled) = cx
                .update(|app| {
                    let status = scenario.surface.window_session_status(app);
                    (
                        status.phase() == DockSurfaceWindowSessionPhase::Closed,
                        status.runtime_empty() == Some(true),
                        app.windows().is_empty(),
                        app.has_active_drag(),
                        status.pending_terminal_ticket_count() == 0
                            && status.failed_terminal_ticket_count() == 0,
                    )
                });
            Ok(session_closed
                && runtime_empty
                && no_gpui_windows
                && !active_drag
                && tickets_settled
                && unsafe { GetCapture() } == HWND::default()
                && !unsafe { IsWindow(Some(scenario.source_hwnd)).as_bool() }
                && !unsafe { IsWindow(Some(scenario.target_hwnd)).as_bool() })
        },
    )
    .await?;

    assert_native_shutdown_lifecycle(scenario)?;

    ensure!(
        inject_primary_button_up_best_effort(),
        "the native Dock scenario could not restore the primary button after shutdown"
    );
    pointer_guard.primary_button_down = false;
    log::info!(
        "scenario={} source_hwnd={:?} anchor_hwnd={:?} completed",
        scenario_id,
        scenario.source_hwnd,
        scenario.target_hwnd
    );
    Ok(())
}

fn assert_source_only_native_capture_trace(
    scenario: &NativeDockScenario,
    release_point: POINT,
) -> Result<()> {
    let events = scenario.native_observation.events();
    let source_window_id = scenario.source_window.window_id();
    let target_window_id = scenario.target_window.window_id();
    ensure!(
        !events.iter().any(|event| {
            matches!(
                event.kind(),
                NativeWindowTestEventKind::WindowMessage {
                    disposition: NativeWindowTestMessageDisposition::Panicked,
                    ..
                }
            )
        }),
        "the real WndProc input stream contained a recovered callback panic: {events:?}"
    );

    let target_mouse = events
        .iter()
        .copied()
        .filter(|event| event.window().window_id() == target_window_id)
        .filter(|event| {
            matches!(
                event.kind(),
                NativeWindowTestEventKind::WindowMessage {
                    message,
                    extra_info,
                    ..
                } if is_native_mouse_message(message)
                    && extra_info == INPUT_CANARY as isize
            )
        })
        .collect::<Vec<_>>();
    ensure!(
        target_mouse.is_empty(),
        "the injected captured-input sequence selected the target WndProc: {target_mouse:?}"
    );

    let source_mouse = events
        .iter()
        .copied()
        .filter(|event| event.window().window_id() == source_window_id)
        .filter(|event| {
            matches!(
                event.kind(),
                NativeWindowTestEventKind::WindowMessage { message, .. }
                    if is_native_mouse_message(message)
            )
        })
        .collect::<Vec<_>>();
    let down = source_mouse.iter().position(|event| {
        matches!(
            event.kind(),
            NativeWindowTestEventKind::WindowMessage {
                message: NativeWindowTestMessage::PrimaryButtonDown,
                extra_info,
                capture_before: NativeWindowTestCaptureOwner::None,
                capture_after: NativeWindowTestCaptureOwner::Recipient,
                disposition: NativeWindowTestMessageDisposition::Returned(_),
                ..
            } if extra_info == INPUT_CANARY as isize
        )
    });
    let captured_move = source_mouse.iter().position(|event| {
        matches!(
            event.kind(),
            NativeWindowTestEventKind::WindowMessage {
                message: NativeWindowTestMessage::MouseMove,
                extra_info,
                screen_point: Some(point),
                capture_before: NativeWindowTestCaptureOwner::Recipient,
                capture_after: NativeWindowTestCaptureOwner::Recipient,
                disposition: NativeWindowTestMessageDisposition::Returned(_),
                ..
            } if extra_info == INPUT_CANARY as isize && point == native_point(release_point)
        )
    });
    let up = source_mouse.iter().position(|event| {
        matches!(
            event.kind(),
            NativeWindowTestEventKind::WindowMessage {
                message: NativeWindowTestMessage::PrimaryButtonUp,
                extra_info,
                screen_point: Some(point),
                capture_before: NativeWindowTestCaptureOwner::Recipient,
                capture_after: NativeWindowTestCaptureOwner::None,
                disposition: NativeWindowTestMessageDisposition::Returned(_),
                ..
            } if extra_info == INPUT_CANARY as isize && point == native_point(release_point)
        )
    });
    let (Some(down), Some(captured_move), Some(up)) = (down, captured_move, up) else {
        bail!(
            "the source WndProc did not expose the typed captured down/move/up sequence at {release_point:?}: {source_mouse:?}"
        );
    };
    ensure!(
        down < captured_move && captured_move < up,
        "the source WndProc captured input sequence was reordered: {source_mouse:?}"
    );
    Ok(())
}

fn source_native_primary_button_up_observed(
    scenario: &NativeDockScenario,
    release_point: POINT,
) -> bool {
    let source_window_id = scenario.source_window.window_id();
    scenario.native_observation.events().iter().any(|event| {
        event.window().window_id() == source_window_id
            && matches!(
                event.kind(),
                NativeWindowTestEventKind::WindowMessage {
                    message: NativeWindowTestMessage::PrimaryButtonUp,
                    extra_info,
                    screen_point: Some(point),
                    disposition: NativeWindowTestMessageDisposition::Returned(_),
                    ..
                } if extra_info == INPUT_CANARY as isize && point == native_point(release_point)
            )
    })
}

fn source_native_primary_button_down_captured_observed(
    scenario: &NativeDockScenario,
    press_point: POINT,
) -> bool {
    let source_window_id = scenario.source_window.window_id();
    scenario.native_observation.events().iter().any(|event| {
        event.window().window_id() == source_window_id
            && matches!(
                event.kind(),
                NativeWindowTestEventKind::WindowMessage {
                    message: NativeWindowTestMessage::PrimaryButtonDown,
                    extra_info,
                    screen_point: Some(point),
                    capture_before: NativeWindowTestCaptureOwner::None,
                    capture_after: NativeWindowTestCaptureOwner::Recipient,
                    disposition: NativeWindowTestMessageDisposition::Returned(_),
                    ..
                } if extra_info == INPUT_CANARY as isize && point == native_point(press_point)
            )
    })
}

fn source_native_captured_move_observed(scenario: &NativeDockScenario, point: POINT) -> bool {
    let source_window_id = scenario.source_window.window_id();
    scenario.native_observation.events().iter().any(|event| {
        event.window().window_id() == source_window_id
            && matches!(
                event.kind(),
                NativeWindowTestEventKind::WindowMessage {
                    message: NativeWindowTestMessage::MouseMove,
                    extra_info,
                    screen_point: Some(observed),
                    capture_before: NativeWindowTestCaptureOwner::Recipient,
                    capture_after: NativeWindowTestCaptureOwner::Recipient,
                    disposition: NativeWindowTestMessageDisposition::Returned(_),
                    ..
                } if extra_info == INPUT_CANARY as isize && observed == native_point(point)
            )
    })
}

fn native_system_mouse_message_observed(
    observation: &NativeWindowTestObservation,
    window: AnyWindowHandle,
    expected_message: NativeWindowTestMessage,
    expected_point: POINT,
) -> bool {
    observation.events().iter().any(|event| {
        event.window().window_id() == window.window_id()
            && matches!(
                event.kind(),
                NativeWindowTestEventKind::WindowMessage {
                    message,
                    extra_info,
                    screen_point: Some(point),
                    disposition: NativeWindowTestMessageDisposition::Returned(_),
                    ..
                } if message == expected_message
                    && extra_info == INPUT_CANARY as isize
                    && point == native_point(expected_point)
            )
    })
}

fn assert_native_shutdown_lifecycle(scenario: &NativeDockScenario) -> Result<()> {
    let events = scenario.native_observation.events();
    let source = native_shutdown_ordinals(&events, scenario.source_window.window_id())?;
    let anchor = native_shutdown_ordinals(&events, scenario.target_window.window_id())?;
    ensure!(
        source.presentation_quiesced < source.destroy_entered
            && source.destroy_entered < source.non_client_destroy
            && source.non_client_destroy < source.native_terminal,
        "the dependent HWND did not quiesce presentation before native terminal: {source:?}"
    );
    ensure!(
        anchor.presentation_quiesced < anchor.destroy_entered
            && anchor.destroy_entered < anchor.non_client_destroy
            && anchor.non_client_destroy < anchor.native_terminal,
        "the anchor HWND did not quiesce presentation before native terminal: {anchor:?}"
    );
    ensure!(
        source.ticket_generation == source.destroy_ticket_generation
            && source.ticket_generation == source.terminal_ticket_generation,
        "the dependent HWND lifecycle crossed presentation-ticket generations: {source:?}"
    );
    ensure!(
        anchor.ticket_generation == anchor.destroy_ticket_generation
            && anchor.ticket_generation == anchor.terminal_ticket_generation,
        "the anchor HWND lifecycle crossed presentation-ticket generations: {anchor:?}"
    );
    ensure!(
        source.non_client_destroy < anchor.destroy_entered,
        "the anchor began native destruction before its dependent reached WM_NCDESTROY: dependent={source:?}, anchor={anchor:?}"
    );
    Ok(())
}

#[derive(Debug)]
struct NativeShutdownOrdinals {
    presentation_quiesced: u64,
    destroy_entered: u64,
    non_client_destroy: u64,
    native_terminal: u64,
    ticket_generation: u64,
    destroy_ticket_generation: u64,
    terminal_ticket_generation: u64,
}

fn native_shutdown_ordinals(
    events: &[NativeWindowTestEvent],
    window_id: open_gpui::WindowId,
) -> Result<NativeShutdownOrdinals> {
    let mut presentation_quiesced = None;
    let mut destroy_entered = None;
    let mut non_client_destroy = None;
    let mut native_terminal = None;
    for event in events
        .iter()
        .copied()
        .filter(|event| event.window().window_id() == window_id)
    {
        match event.kind() {
            NativeWindowTestEventKind::PresentationQuiesced { ticket_generation } => {
                presentation_quiesced = Some((event.ordinal(), ticket_generation));
            }
            NativeWindowTestEventKind::DestroyEntered { ticket_generation } => {
                destroy_entered = Some((event.ordinal(), ticket_generation));
            }
            NativeWindowTestEventKind::WindowMessage {
                message: NativeWindowTestMessage::NonClientDestroy,
                capture_before: NativeWindowTestCaptureOwner::None,
                capture_after: NativeWindowTestCaptureOwner::None,
                disposition: NativeWindowTestMessageDisposition::Returned(_),
                ..
            } => non_client_destroy = Some(event.ordinal()),
            NativeWindowTestEventKind::NativeTerminal { ticket_generation } => {
                native_terminal = Some((event.ordinal(), ticket_generation));
            }
            _ => {}
        }
    }
    let Some((presentation_quiesced, ticket_generation)) = presentation_quiesced else {
        bail!("window {window_id:?} never published presentation quiescence: {events:?}");
    };
    let Some((destroy_entered, destroy_ticket_generation)) = destroy_entered else {
        bail!("window {window_id:?} never entered native destruction: {events:?}");
    };
    let Some(non_client_destroy) = non_client_destroy else {
        bail!("window {window_id:?} never crossed WM_NCDESTROY without capture: {events:?}");
    };
    let Some((native_terminal, terminal_ticket_generation)) = native_terminal else {
        bail!("window {window_id:?} never acknowledged native terminal: {events:?}");
    };
    Ok(NativeShutdownOrdinals {
        presentation_quiesced,
        destroy_entered,
        non_client_destroy,
        native_terminal,
        ticket_generation,
        destroy_ticket_generation,
        terminal_ticket_generation,
    })
}

fn open_desktop_release_point(source: HWND, target: HWND) -> Result<POINT> {
    open_desktop_release_point_with_separation(source, target, None)
}

fn open_desktop_release_point_on_display(
    source: HWND,
    target: HWND,
    display: NativeTestDisplay,
    previous: Option<POINT>,
) -> Result<POINT> {
    let source_rect = window_rect(source)?;
    let target_rect = window_rect(target)?;
    let bounds = display.physical_bounds();
    let inset = 64_i32;
    let left = bounds.origin.x.0.saturating_add(inset);
    let top = bounds.origin.y.0.saturating_add(inset);
    let width = bounds.size.width.0.saturating_sub(inset.saturating_mul(2));
    let height = bounds.size.height.0.saturating_sub(inset.saturating_mul(2));
    ensure!(
        width > 0 && height > 0,
        "native display is too small for a separated open-desktop point: {display:?}"
    );

    let sufficiently_separated = |point: POINT| {
        previous.is_none_or(|previous| {
            let dx = i64::from(point.x) - i64::from(previous.x);
            let dy = i64::from(point.y) - i64::from(previous.y);
            dx.saturating_mul(dx) + dy.saturating_mul(dy) >= 320_i64.pow(2)
        })
    };
    for x_fraction in 1..8 {
        for y_fraction in 1..8 {
            let candidate = POINT {
                x: left.saturating_add(((i64::from(width) * i64::from(x_fraction)) / 8) as i32),
                y: top.saturating_add(((i64::from(height) * i64::from(y_fraction)) / 8) as i32),
            };
            if !rect_contains(source_rect, candidate)
                && !rect_contains(target_rect, candidate)
                && sufficiently_separated(candidate)
                && point_is_open_desktop(candidate)
            {
                return Ok(candidate);
            }
        }
    }

    bail!(
        "scenario suite `{NATIVE_DOCK_SUITE_ID}` could not find an open-desktop point on display {display:?}, outside source={source_rect:?} and target={target_rect:?}, separated from {previous:?}"
    )
}

fn open_desktop_release_point_away_from(
    source: HWND,
    target: HWND,
    previous: POINT,
) -> Result<POINT> {
    open_desktop_release_point_with_separation(source, target, Some(previous))
}

fn open_desktop_release_point_with_separation(
    source: HWND,
    target: HWND,
    previous: Option<POINT>,
) -> Result<POINT> {
    let source_rect = window_rect(source)?;
    let target_rect = window_rect(target)?;
    let virtual_screen = VirtualScreenBounds::current()?;
    let left = source_rect.left.min(target_rect.left);
    let right = source_rect.right.max(target_rect.right);
    let top = source_rect.top.min(target_rect.top);
    let bottom = source_rect.bottom.max(target_rect.bottom);
    let center_x = midpoint(left, right);
    let center_y = midpoint(top, bottom);
    let inset = 64;
    let mut candidates = vec![
        POINT {
            x: center_x,
            y: bottom.saturating_add(inset),
        },
        POINT {
            x: center_x,
            y: top.saturating_sub(inset),
        },
        POINT {
            x: left.saturating_sub(inset),
            y: center_y,
        },
        POINT {
            x: right.saturating_add(inset),
            y: center_y,
        },
        POINT {
            x: virtual_screen.left.saturating_add(inset),
            y: virtual_screen.top.saturating_add(inset),
        },
        POINT {
            x: virtual_screen
                .left
                .saturating_add(virtual_screen.width)
                .saturating_sub(inset),
            y: virtual_screen
                .top
                .saturating_add(virtual_screen.height)
                .saturating_sub(inset),
        },
    ];
    for x_fraction in 1..8 {
        for y_fraction in 1..8 {
            candidates.push(POINT {
                x: virtual_screen.left.saturating_add(
                    ((i64::from(virtual_screen.width) * i64::from(x_fraction)) / 8) as i32,
                ),
                y: virtual_screen.top.saturating_add(
                    ((i64::from(virtual_screen.height) * i64::from(y_fraction)) / 8) as i32,
                ),
            });
        }
    }
    // Fractional probes can all land on unrelated maximized application windows. Walk each
    // physical monitor at a fixed stride as the deterministic fallback so the test discovers an
    // actual shell-owned desktop point instead of assuming one from virtual-screen geometry.
    let scan_stride = 64_i32;
    for display in native_test_displays() {
        let bounds = display.physical_bounds();
        let left = bounds.origin.x.0.saturating_add(scan_stride / 2);
        let top = bounds.origin.y.0.saturating_add(scan_stride / 2);
        let right = bounds
            .origin
            .x
            .0
            .saturating_add(bounds.size.width.0)
            .saturating_sub(scan_stride / 2);
        let bottom = bounds
            .origin
            .y
            .0
            .saturating_add(bounds.size.height.0)
            .saturating_sub(scan_stride / 2);
        let mut y = top;
        while y < bottom {
            let mut x = left;
            while x < right {
                candidates.push(POINT { x, y });
                x = x.saturating_add(scan_stride);
            }
            y = y.saturating_add(scan_stride);
        }
    }
    let eligible = |point: &POINT| {
        let sufficiently_separated = previous.is_none_or(|previous| {
            let dx = i64::from(point.x) - i64::from(previous.x);
            let dy = i64::from(point.y) - i64::from(previous.y);
            dx.saturating_mul(dx) + dy.saturating_mul(dy) >= 320_i64.pow(2)
        });
        virtual_screen.contains(*point)
            && unsafe { !MonitorFromPoint(*point, MONITOR_DEFAULTTONULL).is_invalid() }
            && !rect_contains(source_rect, *point)
            && !rect_contains(target_rect, *point)
            && sufficiently_separated
    };
    let mut first_eligible_hits = Vec::new();
    for point in candidates {
        if !eligible(&point) {
            continue;
        }
        let root = root_window_from_point(point);
        if root == HWND::default() || native_window_is_shell_desktop(root) {
            return Ok(point);
        }
        if first_eligible_hits.len() < 16 {
            first_eligible_hits.push(native_window_probe(point, root));
        }
    }
    bail!(
        "scenario suite `{NATIVE_DOCK_SUITE_ID}` could not find an open-desktop release point outside source={source_rect:?} and target={target_rect:?}, separated from {previous:?}, within {virtual_screen:?}; displays={:?}; first eligible native hits={first_eligible_hits:?}",
        native_test_displays(),
    )
}

fn point_is_open_desktop(point: POINT) -> bool {
    let root = root_window_from_point(point);
    if root == HWND::default() {
        return true;
    }
    native_window_is_shell_desktop(root)
}

fn root_window_from_point(point: POINT) -> HWND {
    let hit = unsafe { WindowFromPoint(point) };
    if hit == HWND::default() {
        return HWND::default();
    }
    let root = unsafe { GetAncestor(hit, GA_ROOT) };
    if root == HWND::default() { hit } else { root }
}

fn native_window_is_above(upper: HWND, lower: HWND) -> bool {
    if upper == HWND::default() || lower == HWND::default() || upper == lower {
        return false;
    }
    let mut current = upper;
    for _ in 0..4096 {
        let Ok(next) = (unsafe { GetWindow(current, GW_HWNDNEXT) }) else {
            return false;
        };
        if next == lower {
            return true;
        }
        current = next;
    }
    false
}

fn native_window_is_shell_desktop(hwnd: HWND) -> bool {
    if hwnd == HWND::default() || hwnd == unsafe { GetDesktopWindow() } {
        return true;
    }
    let shell = unsafe { GetShellWindow() };
    if shell == HWND::default() {
        return false;
    }
    let mut shell_process = 0;
    let mut candidate_process = 0;
    unsafe {
        GetWindowThreadProcessId(shell, Some(&mut shell_process));
        GetWindowThreadProcessId(hwnd, Some(&mut candidate_process));
    }
    if shell_process == 0 || candidate_process != shell_process {
        return false;
    }
    let mut class_name = [0_u16; 64];
    let length = unsafe { GetClassNameW(hwnd, &mut class_name) };
    if length <= 0 {
        return false;
    }
    matches!(
        String::from_utf16_lossy(&class_name[..length as usize]).as_str(),
        "Progman" | "WorkerW"
    )
}

struct NativeWindowProbe {
    point: NativeWindowTestPoint,
    hwnd: isize,
    class_name: String,
    process_id: u32,
    rect: Option<RECT>,
}

impl std::fmt::Debug for NativeWindowProbe {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeWindowProbe")
            .field("point", &self.point)
            .field("hwnd", &self.hwnd)
            .field("class_name", &self.class_name)
            .field("process_id", &self.process_id)
            .field("rect", &self.rect)
            .finish()
    }
}

fn native_window_probe(point: POINT, hwnd: HWND) -> NativeWindowProbe {
    let mut class_name = [0_u16; 128];
    let class_length = unsafe { GetClassNameW(hwnd, &mut class_name) }.max(0) as usize;
    let mut process_id = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }
    NativeWindowProbe {
        point: native_point(point),
        hwnd: hwnd.0 as isize,
        class_name: String::from_utf16_lossy(&class_name[..class_length]),
        process_id,
        rect: window_rect(hwnd).ok(),
    }
}

fn window_rect(hwnd: HWND) -> Result<RECT> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }
        .context("the native Dock scenario could not sample an HWND rectangle")?;
    Ok(rect)
}

fn rects_overlap(left: RECT, right: RECT) -> bool {
    left.left < right.right
        && right.left < left.right
        && left.top < right.bottom
        && right.top < left.bottom
}

fn rect_contains_rect(outer: RECT, inner: RECT) -> bool {
    outer.left <= inner.left
        && outer.top <= inner.top
        && outer.right >= inner.right
        && outer.bottom >= inner.bottom
}

fn visible_client_point_for_window(hwnd: HWND, preferred: POINT) -> Result<POINT> {
    let mut client_rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut client_rect) }
        .context("the native Dock scenario could not sample the promoted client rectangle")?;
    let width = client_rect.right.saturating_sub(client_rect.left);
    let height = client_rect.bottom.saturating_sub(client_rect.top);
    ensure!(
        width > 2 && height > 2,
        "the promoted HWND must expose a non-empty client rectangle: {client_rect:?}"
    );

    let mut candidates = vec![preferred];
    for x_fraction in 1..8 {
        for y_fraction in 1..8 {
            let mut candidate = POINT {
                x: client_rect
                    .left
                    .saturating_add(((i64::from(width) * i64::from(x_fraction)) / 8) as i32),
                y: client_rect
                    .top
                    .saturating_add(((i64::from(height) * i64::from(y_fraction)) / 8) as i32),
            };
            ensure!(
                unsafe { ClientToScreen(hwnd, &mut candidate) }.as_bool(),
                "the native Dock scenario could not convert a promoted client probe to screen coordinates"
            );
            candidates.push(candidate);
        }
    }
    candidates
        .into_iter()
        .find(|point| root_window_from_point(*point) == hwnd)
        .with_context(|| {
            format!(
                "the promoted HWND has no system-visible client point: hwnd={hwnd:?}, client={client_rect:?}"
            )
        })
}

fn screen_point_lparam(point: POINT) -> LPARAM {
    let packed = ((point.y as u32 & 0xffff) << 16) | (point.x as u32 & 0xffff);
    LPARAM(packed as isize)
}

fn rect_contains(rect: RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

fn midpoint(first: i32, second: i32) -> i32 {
    ((i64::from(first) + i64::from(second)) / 2) as i32
}

async fn wait_until(
    cx: &mut AsyncApp,
    description: &str,
    mut predicate: impl FnMut(&mut AsyncApp) -> Result<bool>,
) -> Result<()> {
    let deadline = Instant::now() + SCENARIO_TIMEOUT;
    loop {
        if predicate(cx)? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("scenario suite `{NATIVE_DOCK_SUITE_ID}` timed out waiting for {description}");
        }
        cx.background_executor().timer(POLL_INTERVAL).await;
    }
}

async fn wait_for_cursor(cx: &mut AsyncApp, expected: POINT, description: &str) -> Result<()> {
    wait_until(cx, description, |_| {
        let mut observed = POINT::default();
        unsafe { GetCursorPos(&mut observed) }.context("failed to sample the system cursor")?;
        Ok(observed.x.abs_diff(expected.x) <= 1 && observed.y.abs_diff(expected.y) <= 1)
    })
    .await
}

fn raw_hwnd(cx: &mut App, window: AnyWindowHandle) -> Result<HWND> {
    cx.update_window(window, |_, window, _| {
        let handle = HasWindowHandle::window_handle(window)
            .map_err(|error| {
                anyhow!("scenario suite `{NATIVE_DOCK_SUITE_ID}` could not read HWND: {error:?}")
            })?
            .as_raw();
        match handle {
            RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut c_void)),
            other => {
                bail!("scenario suite `{NATIVE_DOCK_SUITE_ID}` expected Win32, received {other:?}")
            }
        }
    })?
}

fn window_has_non_empty_frame(cx: &mut App, window: AnyWindowHandle) -> Result<bool> {
    cx.update_window(window, |_, window, _| {
        let facts = window.presentation_facts();
        facts.native_visible
            && facts.non_empty_presented_generation.is_some()
            && facts.latest_present_attempt.is_some_and(|attempt| {
                attempt.outcome == PlatformWindowPresentOutcome::Submitted
                    && attempt.contained_valid_primitives
                    && Some(attempt.generation) == facts.non_empty_presented_generation
            })
    })
}

fn window_scale_factor(cx: &mut App, window: AnyWindowHandle) -> Result<f32> {
    cx.update_window(window, |_, window, _| window.scale_factor())
}

fn window_platform_facts(cx: &mut App, window: AnyWindowHandle) -> Result<WindowPlatformFacts> {
    cx.update_window(window, |_, window, _| window.platform_facts().clone())
}

fn unique_matching_debug_bounds(
    cx: &mut App,
    window: AnyWindowHandle,
    prefix: &str,
    suffix: &str,
) -> Result<Bounds<Pixels>> {
    let matches = cx.update_window(window, |_, window, _| {
        window
            .committed_debug_bounds_for_test()
            .into_iter()
            .filter(|(selector, _)| selector.starts_with(prefix) && selector.ends_with(suffix))
            .collect::<Vec<_>>()
    })?;
    match matches.as_slice() {
        [(_, bounds)] => Ok(*bounds),
        [] => bail!(
            "scenario suite `{NATIVE_DOCK_SUITE_ID}` found no committed selector matching `{prefix}*{suffix}`"
        ),
        _ => bail!(
            "scenario suite `{NATIVE_DOCK_SUITE_ID}` found ambiguous committed selectors matching `{prefix}*{suffix}`: {matches:?}"
        ),
    }
}

fn unique_recovery_entry_bounds(
    cx: &mut App,
    windows: &[AnyWindowHandle],
) -> Result<Option<(AnyWindowHandle, Bounds<Pixels>)>> {
    let mut matches = Vec::new();
    for window in windows.iter().copied() {
        let window_matches = cx.update_window(window, |_, window, _| {
            window
                .committed_debug_bounds_for_test()
                .into_iter()
                .filter(|(selector, _)| {
                    selector.starts_with("dock:payload-recovery:") && selector.ends_with(":restore")
                })
                .map(|(_, bounds)| bounds)
                .collect::<Vec<_>>()
        })?;
        matches.extend(window_matches.into_iter().map(|bounds| (window, bounds)));
    }
    match matches.as_slice() {
        [] => Ok(None),
        [(window, bounds)] => Ok(Some((*window, *bounds))),
        _ => bail!(
            "scenario suite `{NATIVE_DOCK_SUITE_ID}` found ambiguous committed Restore entries: {matches:?}"
        ),
    }
}

fn transition_count(events: &[DockSurfaceChangeEvent], transition: DockSurfaceTransition) -> usize {
    events
        .iter()
        .filter(|event| event.transitions().contains(&transition))
        .count()
}

fn exact_debug_bounds(
    cx: &mut App,
    window: AnyWindowHandle,
    selector: &str,
) -> Result<Option<Bounds<Pixels>>> {
    cx.update_window(window, |_, window, _| {
        window
            .committed_debug_bounds_for_test()
            .into_iter()
            .find_map(|(candidate, bounds)| (candidate == selector).then_some(bounds))
    })
}

fn logical_client_point_to_screen(
    hwnd: HWND,
    point: open_gpui::Point<Pixels>,
    scale_factor: f32,
) -> Result<POINT> {
    let mut point = POINT {
        x: (point.x.as_f32() * scale_factor).round() as i32,
        y: (point.y.as_f32() * scale_factor).round() as i32,
    };
    ensure!(
        unsafe { ClientToScreen(hwnd, &mut point).as_bool() },
        "scenario suite `{NATIVE_DOCK_SUITE_ID}` could not convert a committed client point for HWND {hwnd:?}"
    );
    Ok(point)
}

fn inject_system_pointer(point: POINT, flags: MOUSE_EVENT_FLAGS) -> Result<()> {
    let (dx, dy) = VirtualScreenBounds::current()?.absolute_coordinates(point)?;
    let flags = if flags.contains(MOUSEEVENTF_MOVE) {
        flags | MOUSEEVENTF_MOVE_NOCOALESCE
    } else {
        flags
    };
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK | flags,
                time: 0,
                dwExtraInfo: INPUT_CANARY,
            },
        },
    };
    ensure!(
        unsafe { SendInput(&[input], size_of::<INPUT>() as i32) } == 1,
        "scenario suite `{NATIVE_DOCK_SUITE_ID}` system input was rejected by the desktop or UIPI boundary"
    );
    Ok(())
}

fn inject_primary_button_up_best_effort() -> bool {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_LEFTUP,
                time: 0,
                dwExtraInfo: INPUT_CANARY,
            },
        },
    };
    (unsafe { SendInput(&[input], size_of::<INPUT>() as i32) }) == 1
}

#[derive(Clone, Copy, Debug)]
struct VirtualScreenBounds {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

impl VirtualScreenBounds {
    fn current() -> Result<Self> {
        let bounds = Self {
            left: unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) },
            top: unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) },
            width: unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
            height: unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
        };
        ensure!(
            bounds.width > 1 && bounds.height > 1,
            "scenario suite `{NATIVE_DOCK_SUITE_ID}` requires a non-empty virtual desktop: {bounds:?}"
        );
        Ok(bounds)
    }

    fn contains(self, point: POINT) -> bool {
        let right = i64::from(self.left) + i64::from(self.width);
        let bottom = i64::from(self.top) + i64::from(self.height);
        i64::from(point.x) >= i64::from(self.left)
            && i64::from(point.x) < right
            && i64::from(point.y) >= i64::from(self.top)
            && i64::from(point.y) < bottom
    }

    fn absolute_coordinates(self, point: POINT) -> Result<(i32, i32)> {
        let right = i64::from(self.left) + i64::from(self.width);
        let bottom = i64::from(self.top) + i64::from(self.height);
        ensure!(
            i64::from(point.x) >= i64::from(self.left)
                && i64::from(point.x) < right
                && i64::from(point.y) >= i64::from(self.top)
                && i64::from(point.y) < bottom,
            "scenario suite `{NATIVE_DOCK_SUITE_ID}` injection point is outside the virtual desktop: point={point:?}, bounds={self:?}"
        );
        Ok((
            absolute_input_coordinate(point.x, self.left, self.width),
            absolute_input_coordinate(point.y, self.top, self.height),
        ))
    }
}

fn absolute_input_coordinate(value: i32, origin: i32, extent: i32) -> i32 {
    let numerator = (i64::from(value) - i64::from(origin)) * 65_535;
    (numerator / i64::from(extent - 1)) as i32
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "native Dock foreground task panicked with a non-string payload".to_owned()
    }
}
