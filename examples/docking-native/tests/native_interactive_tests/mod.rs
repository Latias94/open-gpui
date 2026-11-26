use anyhow::{Context as _, Result, anyhow, bail, ensure};
use open_gpui::{
    AnyWindowHandle, App, AppContext as _, AsyncApp, Bounds, DevicePixels, Empty,
    NativeInputInvariantViolation, Pixels, PlatformWindowHit, PlatformWindowHitTerminus,
    PlatformWindowPresentOutcome, Window, WindowBounds, WindowMutationDispatch, WindowOptions,
    WindowPlatformFacts, point, size,
};
use open_gpui_windows::{
    NATIVE_TEST_INPUT_CANARY, NativeTestDisplay, NativeTestOpaqueWindow,
    NativeWindowTestCaptureOwner, NativeWindowTestEvent, NativeWindowTestEventKind,
    NativeWindowTestIdentity, NativeWindowTestMessage, NativeWindowTestMessageDisposition,
    NativeWindowTestObservation, NativeWindowTestPoint,
    native_test_client_screen_bounds as native_client_screen_bounds,
    native_test_confirm_scenario_behavior, native_test_displays,
    native_test_mixed_dpi_display_pair, native_test_non_shell_root_window_at,
    native_test_raise_window as raise_native_window,
    native_test_release_primary_button_best_effort as inject_primary_button_up_best_effort,
    native_test_virtual_screen_bounds, native_test_window_is_above as native_window_is_above,
    native_test_window_probe as native_window_probe, native_test_window_rect as window_rect,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT, RECT},
    Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromPoint},
    UI::{
        Input::KeyboardAndMouse::GetCapture,
        WindowsAndMessaging::{GetCursorPos, GetForegroundWindow},
    },
};

const NATIVE_DOCK_SUITE_ID: &str = "docking-native.windows.interactive";
const NATIVE_SCENARIO_ENV: &str = "OPEN_GPUI_NATIVE_SCENARIO_ID";
const INPUT_CANARY: usize = NATIVE_TEST_INPUT_CANARY;
const SCENARIO_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(5);
const NATIVE_SCENARIO_MANIFEST: &str =
    include_str!("../native_windows_interactive.native-scenarios.toml");

#[cfg(feature = "legacy-docking")]
mod legacy_benchmark;
mod process_harness;
mod shared_interactive;
mod shared_native;
mod shared_placement;
mod shared_recovery;
mod shared_session;
mod shared_shutdown;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OrderedMixedDpiDisplayPlan<T, P> {
    source: T,
    target: T,
    negative_display: T,
    negative_point: P,
}

type NativeMixedDpiDisplayPair = OrderedMixedDpiDisplayPlan<NativeTestDisplay, POINT>;

fn order_mixed_dpi_display_pair<T: Copy + PartialEq, P: Copy>(
    lower_higher: (T, T),
    negative_route: (T, P),
    subcase: process_harness::NativeWorkerSubcase,
) -> Result<OrderedMixedDpiDisplayPlan<T, P>> {
    ensure!(
        negative_route.0 == lower_higher.0 || negative_route.0 == lower_higher.1,
        "the mixed-DPI negative-route display must belong to the selected pair"
    );
    let (source, target) = match subcase {
        process_harness::NativeWorkerSubcase::MixedDpiLowerToHigher => lower_higher,
        process_harness::NativeWorkerSubcase::MixedDpiHigherToLower => {
            (lower_higher.1, lower_higher.0)
        }
        _ => bail!("mixed-DPI display selection requires a mixed-DPI worker subcase"),
    };
    Ok(OrderedMixedDpiDisplayPlan {
        source,
        target,
        negative_display: negative_route.0,
        negative_point: negative_route.1,
    })
}

fn select_native_mixed_dpi_display_pair(
    subcase: process_harness::NativeWorkerSubcase,
    expected_plan: process_harness::NativeMixedDpiPairPlan,
) -> Result<NativeMixedDpiDisplayPair> {
    let pair = native_test_mixed_dpi_display_pair()?;
    expected_plan.validate_pair(pair)?;
    let negative_point = pair.negative_physical_point();
    order_mixed_dpi_display_pair(
        (pair.lower(), pair.higher()),
        (
            pair.negative_display(),
            POINT {
                x: negative_point.x.0,
                y: negative_point.y.0,
            },
        ),
        subcase,
    )
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum NativeDockBehavior {
    SourceCapture,
    ComplexDockspaceWorkflow,
    #[cfg(feature = "legacy-docking")]
    TabDragLatencyBenchmark,
    OpaqueOcclusion,
    SurfaceShutdown,
    ProvisionalSameHwndPromotion,
    LiveRouteAndReleaseLock,
    CommittedLossRecovery,
    ProcessConvergence,
    NoInputPassThrough,
    MixedDpiPlacement,
}

impl NativeDockBehavior {
    const fn as_str(self) -> &'static str {
        match self {
            Self::SourceCapture => "source-capture",
            Self::ComplexDockspaceWorkflow => "complex-dockspace-workflow",
            #[cfg(feature = "legacy-docking")]
            Self::TabDragLatencyBenchmark => "tab-drag-latency-benchmark",
            Self::OpaqueOcclusion => "opaque-occlusion",
            Self::SurfaceShutdown => "surface-shutdown",
            Self::ProvisionalSameHwndPromotion => "provisional-same-hwnd-promotion",
            Self::LiveRouteAndReleaseLock => "live-route-and-release-lock",
            Self::CommittedLossRecovery => "committed-loss-recovery",
            Self::ProcessConvergence => "process-convergence",
            Self::NoInputPassThrough => "no-input-pass-through",
            Self::MixedDpiPlacement => "mixed-dpi-placement",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeScenarioManifest {
    schema: u16,
    suite: String,
    scenario: Vec<NativeScenarioManifestRegistration>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeScenarioManifestRegistration {
    id: String,
    requirement_owner: String,
    package: NativeScenarioPackage,
    ignored: bool,
    test: String,
    observation_domains: BTreeSet<String>,
    behavior: NativeScenarioManifestBehavior,
    #[serde(default)]
    dockspace_scenario: Option<String>,
    #[serde(default)]
    dockspace_step: Option<String>,
    #[serde(default)]
    dockspace_oracles: BTreeSet<NativeDockspaceManifestOracle>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
enum NativeScenarioPackage {
    #[serde(rename = "open-gpui-docking-native")]
    DockingNative,
    #[serde(rename = "open-gpui-windows")]
    Windows,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum NativeDockspaceManifestOracle {
    Graph,
    Layout,
    Presentation,
    FocusSelection,
    ViewportRoute,
    NativeLifetime,
    Performance,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum NativeScenarioManifestBehavior {
    SourceCapture,
    ComplexDockspaceWorkflow,
    OpaqueOcclusion,
    OpaqueHitTransparentPrefix,
    SurfaceShutdown,
    ProvisionalSameHwndPromotion,
    LiveRouteAndReleaseLock,
    CommittedLossRecovery,
    ActivationTerminal,
    ProvisionalActivationZOrder,
    ClientGeometryReconciliation,
    EventDrivenGeometryWake,
    ProcessConvergence,
    NoInputPassThrough,
    MixedDpiPlacement,
}

impl NativeScenarioManifestBehavior {
    const fn as_str(self) -> &'static str {
        match self {
            Self::SourceCapture => "source-capture",
            Self::ComplexDockspaceWorkflow => "complex-dockspace-workflow",
            Self::OpaqueOcclusion => "opaque-occlusion",
            Self::OpaqueHitTransparentPrefix => "opaque-hit-transparent-prefix",
            Self::SurfaceShutdown => "surface-shutdown",
            Self::ProvisionalSameHwndPromotion => "provisional-same-hwnd-promotion",
            Self::LiveRouteAndReleaseLock => "live-route-and-release-lock",
            Self::CommittedLossRecovery => "committed-loss-recovery",
            Self::ActivationTerminal => "activation-terminal",
            Self::ProvisionalActivationZOrder => "provisional-activation-z-order",
            Self::ClientGeometryReconciliation => "client-geometry-reconciliation",
            Self::EventDrivenGeometryWake => "event-driven-geometry-wake",
            Self::ProcessConvergence => "process-convergence",
            Self::NoInputPassThrough => "no-input-pass-through",
            Self::MixedDpiPlacement => "mixed-dpi-placement",
        }
    }

    const fn dock_behavior(self) -> Option<NativeDockBehavior> {
        match self {
            Self::SourceCapture => Some(NativeDockBehavior::SourceCapture),
            Self::ComplexDockspaceWorkflow => Some(NativeDockBehavior::ComplexDockspaceWorkflow),
            Self::OpaqueOcclusion => Some(NativeDockBehavior::OpaqueOcclusion),
            Self::SurfaceShutdown => Some(NativeDockBehavior::SurfaceShutdown),
            Self::ProvisionalSameHwndPromotion => {
                Some(NativeDockBehavior::ProvisionalSameHwndPromotion)
            }
            Self::LiveRouteAndReleaseLock => Some(NativeDockBehavior::LiveRouteAndReleaseLock),
            Self::CommittedLossRecovery => Some(NativeDockBehavior::CommittedLossRecovery),
            Self::ActivationTerminal
            | Self::ProvisionalActivationZOrder
            | Self::OpaqueHitTransparentPrefix
            | Self::ClientGeometryReconciliation
            | Self::EventDrivenGeometryWake => None,
            Self::ProcessConvergence => Some(NativeDockBehavior::ProcessConvergence),
            Self::NoInputPassThrough => Some(NativeDockBehavior::NoInputPassThrough),
            Self::MixedDpiPlacement => Some(NativeDockBehavior::MixedDpiPlacement),
        }
    }
}

#[derive(Clone, Debug)]
struct NativeScenarioRegistration {
    id: String,
    test: String,
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
    let registration = native_scenario_manifest()
        .scenario
        .into_iter()
        .find(|scenario| scenario.id == scenario_id)
        .unwrap_or_else(|| {
            panic!("native interactive scenario `{scenario_id}` is not present in the manifest")
        });
    assert_eq!(
        registration.package,
        NativeScenarioPackage::DockingNative,
        "native interactive Dock worker `{scenario_id}` must select an open-gpui-docking-native manifest row"
    );
    assert!(
        registration.ignored,
        "native interactive Dock worker `{scenario_id}` must select an ignored manifest row"
    );
    let behavior = registration.behavior.dock_behavior().unwrap_or_else(|| {
        panic!(
            "native interactive Dock worker `{scenario_id}` cannot execute owning-platform behavior `{}`",
            registration.behavior.as_str()
        )
    });
    NativeScenarioRegistration {
        id: registration.id,
        test: registration.test,
        behavior,
    }
}

struct NativeOpaqueBarrier {
    window: NativeTestOpaqueWindow,
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
        Ok(Self { window })
    }

    fn hwnd(&self) -> HWND {
        self.window.hwnd()
    }

    fn present(&self) -> Result<()> {
        self.window.present()
    }

    fn assert_point_scoped_band(&self, provisional_hwnd: HWND, reveal_point: POINT) -> Result<()> {
        let barrier_hwnd = self.hwnd();
        let barrier_rect = window_rect(barrier_hwnd)?;
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
            native_window_is_above(barrier_hwnd, provisional_hwnd),
            "the point-scoped reveal must keep the normal opaque barrier above the provisional HWND: barrier={:?}, provisional={:?}, provisional_above_barrier={}, foreground={:?}, point_hit={:?}",
            barrier_hwnd,
            provisional_hwnd,
            native_window_is_above(provisional_hwnd, barrier_hwnd),
            unsafe { GetForegroundWindow() },
            native_test_non_shell_root_window_at(reveal_point),
        );
        ensure!(
            native_test_non_shell_root_window_at(reveal_point) == Some(barrier_hwnd),
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

struct NativeNoInputOverlay {
    window: AnyWindowHandle,
    hwnd: HWND,
}

#[test]
fn native_interactive_scenario_registry_matches_cases() {
    let manifest = native_scenario_manifest();
    assert_eq!(manifest.schema, 7);
    assert_eq!(manifest.suite, NATIVE_DOCK_SUITE_ID);

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
                        registration.dockspace_scenario,
                        registration.dockspace_step,
                        registration.dockspace_oracles,
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
            .all(|(_, test, domains, scenario, step, oracles, behavior)| {
                let valid_dockspace_binding = oracles.is_empty()
                    || (scenario
                        .as_deref()
                        .is_some_and(|value| !value.trim().is_empty())
                        && if *behavior == NativeScenarioManifestBehavior::ComplexDockspaceWorkflow
                        {
                            step.is_none()
                        } else {
                            step.as_deref()
                                .is_some_and(|value| !value.trim().is_empty())
                        });
                !test.is_empty() && !domains.is_empty() && valid_dockspace_binding
            }),
        "every manifest row must carry one exact test coordinate and typed observation evidence"
    );
}

#[test]
#[ignore = "requires the local Windows physical-input gate"]
fn native_interactive_two_hwnd_captured_drag_routes_preview_and_drop() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::SourceCapture);
}

#[test]
#[ignore = "requires the local Windows physical-input gate"]
fn native_interactive_complex_dockspace_workflow_runs_four_cross_window_steps() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::ComplexDockspaceWorkflow);
}

#[cfg(feature = "legacy-docking")]
#[test]
#[ignore = "requires an interactive Windows desktop"]
fn native_interactive_tab_drag_input_to_submit_latency_benchmark() {
    legacy_benchmark::run_latency_benchmark();
}

#[test]
#[ignore = "requires the local Windows physical-input gate"]
fn native_interactive_opaque_occlusion_blocks_underlay_and_preserves_same_hwnd() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::OpaqueOcclusion);
}

#[test]
#[ignore = "requires the local Windows physical-input gate"]
fn native_interactive_anchor_close_releases_capture_and_retires_dependent_hwnds() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::SurfaceShutdown);
}

#[test]
#[ignore = "requires the local Windows physical-input gate"]
fn native_interactive_provisional_gate_presents_and_promotes_same_hwnd() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::ProvisionalSameHwndPromotion);
}

#[test]
#[ignore = "requires the local Windows physical-input gate"]
fn native_interactive_live_route_reuses_same_hwnd_and_locks_release() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::LiveRouteAndReleaseLock);
}

#[test]
#[ignore = "requires the local Windows physical-input gate"]
fn native_interactive_committed_destination_loss_retires_runtime_authority() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::CommittedLossRecovery);
}

#[test]
#[ignore = "requires the local Windows physical-input gate"]
fn native_interactive_process_converges_after_active_surface_shutdown() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::ProcessConvergence);
}

#[test]
#[ignore = "requires the local Windows physical-input gate"]
fn native_interactive_no_input_prefix_passes_through_and_fails_closed_on_generation_drift() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::NoInputPassThrough);
}

#[test]
#[ignore = "requires the local Windows physical-input gate"]
fn native_interactive_mixed_dpi_final_client_bounds_are_exact() {
    run_native_interactive_manifest_scenario(NativeDockBehavior::MixedDpiPlacement);
}

fn run_native_interactive_manifest_scenario(expected_behavior: NativeDockBehavior) {
    native_test_confirm_scenario_behavior(expected_behavior.as_str());
    let registration = native_scenario_registration_from_environment();
    assert_eq!(
        registration.behavior, expected_behavior,
        "the manifest scenario must execute through its behavior-specific ignored test wrapper"
    );
    ensure_native_interactive_runner(&registration.id);
    if process_harness::is_worker(&registration.id, registration.behavior) {
        run_native_interactive_worker(registration);
    }
    process_harness::run_case_in_worker(&registration);
}

fn run_native_interactive_worker(registration: NativeScenarioRegistration) -> ! {
    process_harness::await_worker_start();
    match registration.behavior {
        NativeDockBehavior::SurfaceShutdown => shared_shutdown::run_worker(registration),
        #[cfg(feature = "legacy-docking")]
        NativeDockBehavior::TabDragLatencyBenchmark => legacy_benchmark::run_worker(registration),
        NativeDockBehavior::SourceCapture
        | NativeDockBehavior::ComplexDockspaceWorkflow
        | NativeDockBehavior::OpaqueOcclusion
        | NativeDockBehavior::ProvisionalSameHwndPromotion
        | NativeDockBehavior::LiveRouteAndReleaseLock
        | NativeDockBehavior::CommittedLossRecovery
        | NativeDockBehavior::ProcessConvergence
        | NativeDockBehavior::NoInputPassThrough
        | NativeDockBehavior::MixedDpiPlacement => shared_interactive::run_worker(registration),
    }
}

struct NativeWorkerCompletion {
    outcome: process_harness::NativeWorkerOutcome,
    milestones: Vec<String>,
    exit_path: process_harness::NativeWorkerExitPath,
    app_census: process_harness::NativeWorkerAppCensus,
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
        "scenario `{scenario_id}` requires OPEN_GPUI_NATIVE_INTERACTIVE=1 on a local interactive Windows desktop"
    );
}

fn viewport_window_options(bounds: Bounds<Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    }
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

fn native_system_mouse_message_observed(
    observation: &NativeWindowTestObservation,
    window: AnyWindowHandle,
    expected_message: NativeWindowTestMessage,
    expected_point: POINT,
) -> bool {
    native_system_mouse_message_observed_for_admission(
        observation,
        window,
        expected_message,
        expected_point,
        true,
    )
}

fn native_system_mouse_message_observed_for_admission(
    observation: &NativeWindowTestObservation,
    window: AnyWindowHandle,
    expected_message: NativeWindowTestMessage,
    expected_point: POINT,
    require_input_canary: bool,
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
                    && input_provenance_matches(extra_info, require_input_canary)
                    && point == native_point(expected_point)
            )
    })
}

fn input_provenance_matches(extra_info: isize, require_input_canary: bool) -> bool {
    !require_input_canary || extra_info == INPUT_CANARY as isize
}

fn assert_native_shutdown_order(label: &str, lifecycle: &NativeShutdownOrdinals) -> Result<()> {
    ensure!(
        lifecycle.presentation_quiesced < lifecycle.destroy_entered
            && lifecycle.destroy_entered < lifecycle.non_client_destroy
            && lifecycle.non_client_destroy < lifecycle.native_terminal,
        "the {label} HWND did not quiesce presentation before native terminal: {lifecycle:?}"
    );
    ensure!(
        lifecycle.ticket_generation == lifecycle.destroy_ticket_generation
            && lifecycle.ticket_generation == lifecycle.terminal_ticket_generation,
        "the {label} HWND lifecycle crossed presentation-ticket generations: {lifecycle:?}"
    );
    Ok(())
}

fn unique_native_identity_for_window(
    events: &[NativeWindowTestEvent],
    window_id: open_gpui::WindowId,
    label: &str,
) -> Result<NativeWindowTestIdentity> {
    let mut identity = None;
    for event in events
        .iter()
        .copied()
        .filter(|event| event.window().window_id() == window_id)
    {
        match identity {
            Some(existing) if existing != event.window() => {
                bail!(
                    "the {label} reused its GPUI WindowId across native generations: existing={existing:?}, observed={:?}",
                    event.window()
                );
            }
            Some(_) => {}
            None => identity = Some(event.window()),
        }
    }
    identity.with_context(|| format!("the {label} never published a native identity"))
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
    identity: NativeWindowTestIdentity,
) -> Result<NativeShutdownOrdinals> {
    let mut presentation_quiesced = None;
    let mut destroy_entered = None;
    let mut non_client_destroy = None;
    let mut native_terminal = None;
    for event in events
        .iter()
        .copied()
        .filter(|event| event.window() == identity)
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
        bail!("window {identity:?} never published presentation quiescence: {events:?}");
    };
    let Some((destroy_entered, destroy_ticket_generation)) = destroy_entered else {
        bail!("window {identity:?} never entered native destruction: {events:?}");
    };
    let Some(non_client_destroy) = non_client_destroy else {
        bail!("window {identity:?} never crossed WM_NCDESTROY without capture: {events:?}");
    };
    let Some((native_terminal, terminal_ticket_generation)) = native_terminal else {
        bail!("window {identity:?} never acknowledged native terminal: {events:?}");
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
    open_desktop_release_point_with_separation(source, target, &[])
}

fn open_desktop_release_point_on_display(
    source: HWND,
    target: HWND,
    display: NativeTestDisplay,
    previous: Option<POINT>,
    negative_probe: Option<POINT>,
) -> Result<POINT> {
    let source_rect = window_rect(source)?;
    let target_rect = window_rect(target)?;
    let bounds = display.physical_visible_bounds();
    let left = bounds.origin.x.0;
    let top = bounds.origin.y.0;
    let width = bounds.size.width.0;
    let height = bounds.size.height.0;
    ensure!(
        width > 0 && height > 0,
        "native display is too small for a separated open-desktop point: {display:?}"
    );
    if let Some(probe) = negative_probe {
        ensure!(
            probe.x < 0 || probe.y < 0,
            "the mixed-DPI negative probe must carry an actual negative physical coordinate: {probe:?}"
        );
        ensure!(
            bounds.contains(&point(DevicePixels(probe.x), DevicePixels(probe.y))),
            "the mixed-DPI negative probe must belong to the selected display's visible bounds: display={display:?}, probe={probe:?}"
        );
    }

    let sufficiently_separated = |point: POINT| {
        previous.is_none_or(|previous| {
            let dx = i64::from(point.x) - i64::from(previous.x);
            let dy = i64::from(point.y) - i64::from(previous.y);
            dx.saturating_mul(dx) + dy.saturating_mul(dy) >= 320_i64.pow(2)
        })
    };
    let is_candidate = |candidate: POINT| {
        !rect_contains(source_rect, candidate)
            && !rect_contains(target_rect, candidate)
            && sufficiently_separated(candidate)
            && (negative_probe.is_none() || candidate.x < 0 || candidate.y < 0)
            && point_is_open_desktop(candidate)
    };
    if let Some(candidate) = negative_probe.filter(|candidate| is_candidate(*candidate)) {
        return Ok(candidate);
    }
    for x_fraction in 1..8 {
        for y_fraction in 1..8 {
            let candidate = POINT {
                x: left.saturating_add(((i64::from(width) * i64::from(x_fraction)) / 8) as i32),
                y: top.saturating_add(((i64::from(height) * i64::from(y_fraction)) / 8) as i32),
            };
            if is_candidate(candidate) {
                return Ok(candidate);
            }
        }
    }
    let scan_stride = 64_i32;
    let right = left.saturating_add(width);
    let bottom = top.saturating_add(height);
    let mut y = top.saturating_add(scan_stride / 2);
    while y < bottom {
        let mut x = left.saturating_add(scan_stride / 2);
        while x < right {
            let candidate = POINT { x, y };
            if is_candidate(candidate) {
                return Ok(candidate);
            }
            x = x.saturating_add(scan_stride);
        }
        y = y.saturating_add(scan_stride);
    }

    bail!(
        "scenario suite `{NATIVE_DOCK_SUITE_ID}` could not find an open-desktop point on display {display:?}, outside source={source_rect:?} and target={target_rect:?}, separated from {previous:?}, with negative-coordinate requirement={}",
        negative_probe.is_some()
    )
}

fn open_desktop_release_point_away_from(
    source: HWND,
    target: HWND,
    previous: POINT,
) -> Result<POINT> {
    open_desktop_release_point_away_from_all(source, target, &[previous])
}

fn open_desktop_release_point_away_from_all(
    source: HWND,
    target: HWND,
    previous: &[POINT],
) -> Result<POINT> {
    open_desktop_release_point_with_separation(source, target, previous)
}

fn open_desktop_release_point_with_separation(
    source: HWND,
    target: HWND,
    previous: &[POINT],
) -> Result<POINT> {
    let source_rect = window_rect(source)?;
    let target_rect = window_rect(target)?;
    let displays = native_test_displays();
    let virtual_screen = native_desktop_search_bounds()?;
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
            x: virtual_screen.right.saturating_sub(inset),
            y: virtual_screen.bottom.saturating_sub(inset),
        },
    ];
    let virtual_width = virtual_screen.right.saturating_sub(virtual_screen.left);
    let virtual_height = virtual_screen.bottom.saturating_sub(virtual_screen.top);
    for x_fraction in 1..8 {
        for y_fraction in 1..8 {
            candidates.push(POINT {
                x: virtual_screen.left.saturating_add(
                    ((i64::from(virtual_width) * i64::from(x_fraction)) / 8) as i32,
                ),
                y: virtual_screen.top.saturating_add(
                    ((i64::from(virtual_height) * i64::from(y_fraction)) / 8) as i32,
                ),
            });
        }
    }
    let scan_stride = 64_i32;
    for display in &displays {
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
        let sufficiently_separated = previous.iter().all(|previous| {
            let dx = i64::from(point.x) - i64::from(previous.x);
            let dy = i64::from(point.y) - i64::from(previous.y);
            dx.saturating_mul(dx) + dy.saturating_mul(dy) >= 320_i64.pow(2)
        });
        rect_contains(virtual_screen, *point)
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
        let Some(root) = native_test_non_shell_root_window_at(point) else {
            return Ok(point);
        };
        if first_eligible_hits.len() < 16 {
            first_eligible_hits.push(native_window_probe(point, root));
        }
    }
    bail!(
        "scenario suite `{NATIVE_DOCK_SUITE_ID}` could not find an open-desktop release point outside source={source_rect:?} and target={target_rect:?}, separated from {previous:?}, within {virtual_screen:?}; displays={displays:?}; first eligible native hits={first_eligible_hits:?}"
    )
}

fn native_desktop_search_bounds() -> Result<RECT> {
    let native_bounds = native_test_virtual_screen_bounds()?;
    let bounds = RECT {
        left: native_bounds.origin.x.0,
        top: native_bounds.origin.y.0,
        right: native_bounds
            .origin
            .x
            .0
            .saturating_add(native_bounds.size.width.0),
        bottom: native_bounds
            .origin
            .y
            .0
            .saturating_add(native_bounds.size.height.0),
    };
    ensure!(
        bounds.right > bounds.left && bounds.bottom > bounds.top,
        "the native Dock scenario requires a non-empty physical desktop: {bounds:?}"
    );
    Ok(bounds)
}

fn point_is_open_desktop(point: POINT) -> bool {
    native_test_non_shell_root_window_at(point).is_none()
}

fn ensure_open_desktop_point_is_current(point: POINT, description: &str) -> Result<()> {
    let root = native_test_non_shell_root_window_at(point);
    ensure!(
        root.is_none(),
        "{description} is no longer verified shell-owned desktop space: point={point:?}, root={root:?}, probe={:?}",
        root.map(|root| native_window_probe(point, root))
    );
    Ok(())
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

async fn wait_for_cursor(cx: &mut AsyncApp, expected: POINT, description: &str) -> Result<POINT> {
    wait_until(cx, description, |_| {
        let mut observed = POINT::default();
        unsafe { GetCursorPos(&mut observed) }.context("failed to sample the system cursor")?;
        Ok(observed.x.abs_diff(expected.x) <= 1 && observed.y.abs_diff(expected.y) <= 1)
    })
    .await?;
    let mut observed = POINT::default();
    unsafe { GetCursorPos(&mut observed) }
        .context("failed to sample the delivered system cursor")?;
    Ok(observed)
}

fn raw_hwnd(cx: &mut App, window: AnyWindowHandle) -> Result<HWND> {
    cx.update_window(window, |_, window, _| raw_hwnd_from_window(window))?
}

fn raw_hwnd_from_window(window: &Window) -> Result<HWND> {
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

fn native_test_display_at(point: POINT) -> Result<NativeTestDisplay> {
    let physical_point = open_gpui::point(DevicePixels(point.x), DevicePixels(point.y));
    let matches = native_test_displays()
        .into_iter()
        .filter(|display| display.physical_bounds().contains(&physical_point))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [display] => Ok(*display),
        [] => bail!(
            "scenario suite `{NATIVE_DOCK_SUITE_ID}` found no display owning physical point {point:?}"
        ),
        _ => bail!(
            "scenario suite `{NATIVE_DOCK_SUITE_ID}` found ambiguous displays for physical point {point:?}: {matches:?}"
        ),
    }
}

#[test]
fn mixed_dpi_worker_direction_preserves_the_negative_route_plan() {
    assert_eq!(
        order_mixed_dpi_display_pair(
            (10, 30),
            (10, (-960, 540)),
            process_harness::NativeWorkerSubcase::MixedDpiLowerToHigher,
        )
        .expect("the lower-to-higher pair should be available"),
        OrderedMixedDpiDisplayPlan {
            source: 10,
            target: 30,
            negative_display: 10,
            negative_point: (-960, 540),
        }
    );
    assert_eq!(
        order_mixed_dpi_display_pair(
            (10, 30),
            (10, (-960, 540)),
            process_harness::NativeWorkerSubcase::MixedDpiHigherToLower,
        )
        .expect("the higher-to-lower pair should be available"),
        OrderedMixedDpiDisplayPlan {
            source: 30,
            target: 10,
            negative_display: 10,
            negative_point: (-960, 540),
        }
    );
}

#[test]
fn mixed_dpi_worker_direction_rejects_unavailable_subcases() {
    let error = order_mixed_dpi_display_pair(
        (10, 30),
        (10, (-960, 540)),
        process_harness::NativeWorkerSubcase::Primary,
    )
    .expect_err("a non-mixed-DPI subcase must fail closed");

    assert!(error.to_string().contains("mixed-DPI worker subcase"));
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

#[derive(Clone, Copy)]
enum NativeReleasePointAuthority {
    OpenDesktop,
    OpaqueBarrier(HWND),
}

impl NativeReleasePointAuthority {
    fn matches_hit_terminus(self, terminus: PlatformWindowHitTerminus) -> bool {
        match self {
            Self::OpenDesktop => matches!(terminus, PlatformWindowHitTerminus::OpenDesktop),
            Self::OpaqueBarrier(_) => matches!(
                terminus,
                PlatformWindowHitTerminus::Window(PlatformWindowHit::OpaqueBarrier { .. })
            ),
        }
    }

    fn validate_current(self, release_point: POINT) -> Result<()> {
        match self {
            Self::OpenDesktop => ensure_open_desktop_point_is_current(
                release_point,
                "the final release point immediately before LEFTUP",
            ),
            Self::OpaqueBarrier(expected) => {
                let observed = native_test_non_shell_root_window_at(release_point);
                ensure!(
                    observed == Some(expected),
                    "the final release point immediately before LEFTUP no longer resolves to the exact opaque barrier: point={release_point:?}, expected={expected:?}, observed={observed:?}, probe={:?}",
                    observed.map(|observed| native_window_probe(release_point, observed))
                );
                Ok(())
            }
        }
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    panic_payload_message(payload.as_ref())
}

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(violation) = payload.downcast_ref::<NativeInputInvariantViolation>() {
        violation.to_string()
    } else {
        "native Dock foreground task panicked with a non-string payload".to_owned()
    }
}
