use super::*;

use anyhow::{Context as _, Result, anyhow, bail, ensure};
use futures::FutureExt as _;
use open_gpui::{
    AnyWindowHandle, AsyncApp, PlatformWindowPresentOutcome, QuitMode, Subscription,
    WindowMouseEvent, WindowPlatformFacts,
};
use open_gpui_docking::{DockSurfaceWindowSessionPhase, model::DockGraph};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    ffi::c_void,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, set_hook, take_hook},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT, RECT, WPARAM},
    Graphics::Gdi::ClientToScreen,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, GetCapture, INPUT, INPUT_0, INPUT_MOUSE, MOUSE_EVENT_FLAGS,
            MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
            MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, ReleaseCapture, SendInput, VK_LBUTTON,
        },
        WindowsAndMessaging::{
            GetCursorPos, GetForegroundWindow, GetSystemMetrics, GetWindowRect, IsWindow,
            IsWindowVisible, PostMessageW, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
            SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SetCursorPos, SetForegroundWindow, WM_CLOSE,
        },
    },
};

const NATIVE_DOCK_SUITE_ID: &str = "docking-native.windows.interactive";
const INPUT_CANARY: usize = 0x4f47_5044;
const SCENARIO_TIMEOUT: Duration = Duration::from_secs(10);
const SCENARIO_PROCESS_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Clone, Copy, Debug)]
enum NativeDockCase {
    SourceCapture,
    ProvisionalSameHwndPromotion,
    SurfaceShutdown,
}

impl NativeDockCase {
    const fn id(self) -> &'static str {
        match self {
            Self::SourceCapture => "native.u27.source-capture",
            Self::ProvisionalSameHwndPromotion => "native.u29.provisional-same-hwnd-promotion",
            Self::SurfaceShutdown => "native.u28.anchor-shutdown",
        }
    }

    async fn run(self, cx: &mut AsyncApp, scenario: &mut NativeDockScenario) -> Result<()> {
        match self {
            Self::SourceCapture => run_captured_host_drop_scenario(cx, scenario).await,
            Self::ProvisionalSameHwndPromotion => {
                run_provisional_same_hwnd_promotion_scenario(cx, scenario).await
            }
            Self::SurfaceShutdown => run_captured_surface_shutdown_scenario(cx, scenario).await,
        }
    }
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

struct NativeDockScenario {
    surface: DockSurface,
    source_window: AnyWindowHandle,
    target_window: AnyWindowHandle,
    source_hwnd: HWND,
    target_hwnd: HWND,
    initial_revision: u64,
    initial_window_count: usize,
    initial_owned_window_count: usize,
    trace: Arc<Mutex<NativeMouseTrace>>,
    _source_interceptor: Subscription,
    _target_interceptor: Subscription,
}

struct SystemPointerGuard {
    original_position: POINT,
    original_foreground: HWND,
    capture_owner: HWND,
    primary_button_down: bool,
}

struct NativeScenarioWatchdog {
    completed: Arc<AtomicBool>,
}

impl NativeScenarioWatchdog {
    fn arm(scenario_id: &'static str) -> Self {
        let completed = Arc::new(AtomicBool::new(false));
        let completed_for_watchdog = Arc::clone(&completed);
        thread::Builder::new()
            .name(format!("{scenario_id}-watchdog"))
            .spawn(move || {
                thread::sleep(SCENARIO_PROCESS_TIMEOUT);
                if completed_for_watchdog.load(Ordering::Acquire) {
                    return;
                }
                let mut cursor = POINT::default();
                let cursor_known = unsafe { GetCursorPos(&mut cursor) }.is_ok();
                let foreground = unsafe { GetForegroundWindow() };
                eprintln!(
                    "native Dock scenario `{scenario_id}` exceeded its process deadline: cursor_known={cursor_known}, cursor={cursor:?}, foreground={foreground:?}"
                );
                let _ = inject_primary_button_up_best_effort();
                std::process::abort();
            })
            .expect("native Dock scenario watchdog thread must start");
        Self { completed }
    }
}

impl Drop for NativeScenarioWatchdog {
    fn drop(&mut self) {
        self.completed.store(true, Ordering::Release);
    }
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
#[ignore = "requires the open-gpui-windows-native-interactive-ephemeral runner"]
fn native_interactive_two_hwnd_captured_drag_routes_preview_and_drop() {
    run_native_interactive_case(NativeDockCase::SourceCapture);
}

#[test]
#[ignore = "requires the open-gpui-windows-native-interactive-ephemeral runner"]
fn native_interactive_provisional_is_presented_before_release_and_promotes_same_hwnd() {
    run_native_interactive_case(NativeDockCase::ProvisionalSameHwndPromotion);
}

#[test]
#[ignore = "requires the open-gpui-windows-native-interactive-ephemeral runner"]
fn native_interactive_anchor_close_releases_capture_and_retires_dependent_hwnds() {
    run_native_interactive_case(NativeDockCase::SurfaceShutdown);
}

fn run_native_interactive_case(case: NativeDockCase) {
    let scenario_id = case.id();
    ensure_native_interactive_runner(scenario_id);
    let _watchdog = NativeScenarioWatchdog::arm(scenario_id);
    let outcome = Arc::new(Mutex::new(None::<std::result::Result<(), String>>));
    let launch_outcome = Arc::clone(&outcome);
    let observed_panics = Arc::new(Mutex::new(Vec::<String>::new()));
    let observed_panics_for_hook = Arc::clone(&observed_panics);
    let previous_panic_hook = take_hook();
    set_hook(Box::new(move |panic| {
        if let Ok(mut observed_panics) = observed_panics_for_hook.lock() {
            observed_panics.push(panic.to_string());
        }
    }));

    let application_result = catch_unwind(AssertUnwindSafe(|| {
        application().with_quit_mode(QuitMode::Explicit).run(
            move |cx| match build_native_scenario(cx) {
                Ok(mut scenario) => {
                    let task_outcome = Arc::clone(&launch_outcome);
                    cx.spawn(async move |cx| {
                        let result = AssertUnwindSafe(case.run(cx, &mut scenario))
                            .catch_unwind()
                            .await
                            .map_err(panic_message)
                            .and_then(|result| result.map_err(|error| error.to_string()));
                        *task_outcome
                            .lock()
                            .expect("native Dock outcome lock must not be poisoned") = Some(result);
                        cx.update(|cx| cx.quit());
                    })
                    .detach();
                }
                Err(error) => {
                    *launch_outcome
                        .lock()
                        .expect("native Dock outcome lock must not be poisoned") =
                        Some(Err(error.to_string()));
                    cx.quit();
                }
            },
        );
    }));
    set_hook(previous_panic_hook);

    if let Err(payload) = application_result {
        panic!(
            "native Dock scenario `{scenario_id}` application panicked: {}",
            panic_message(payload)
        );
    }

    let observed_panics = observed_panics
        .lock()
        .expect("native Dock panic observation lock must not be poisoned");
    assert!(
        observed_panics.is_empty(),
        "native Dock scenario `{scenario_id}` observed an inner panic: {observed_panics:?}"
    );

    let result = outcome
        .lock()
        .expect("native Dock outcome lock must not be poisoned")
        .take()
        .expect("native Dock scenario must publish a terminal outcome before quitting");
    if let Err(error) = result {
        panic!("native Dock scenario `{scenario_id}` failed: {error}");
    }
}

fn ensure_native_interactive_runner(scenario_id: &str) {
    assert!(
        std::env::var("OPEN_GPUI_NATIVE_INTERACTIVE")
            .ok()
            .is_some_and(|value| value == "1"),
        "scenario `{scenario_id}` requires OPEN_GPUI_NATIVE_INTERACTIVE=1 on the named ephemeral runner"
    );
}

fn build_native_scenario(cx: &mut App) -> Result<NativeDockScenario> {
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

    Ok(NativeDockScenario {
        surface,
        source_window,
        target_window,
        source_hwnd,
        target_hwnd,
        initial_revision,
        initial_window_count,
        initial_owned_window_count,
        trace,
        _source_interceptor: source_interceptor,
        _target_interceptor: target_interceptor,
    })
}

async fn run_captured_host_drop_scenario(
    cx: &mut AsyncApp,
    scenario: &mut NativeDockScenario,
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
    ensure!(
        !trace.target.contains(&NativeMouseKind::Move)
            && !trace.target.contains(&NativeMouseKind::Down)
            && !trace.target.contains(&NativeMouseKind::Up),
        "the target HWND must not receive any raw mouse event during captured drag: {trace:?}"
    );
    ensure!(
        unsafe { IsWindow(Some(scenario.source_hwnd)).as_bool() }
            && unsafe { IsWindow(Some(scenario.target_hwnd)).as_bool() },
        "both exact HWNDs must remain live through durable drop completion"
    );
    log::info!(
        "scenario={} source_hwnd={:?} target_hwnd={:?} revision={} completed",
        NativeDockCase::SourceCapture.id(),
        scenario.source_hwnd,
        scenario.target_hwnd,
        snapshot.revision()
    );
    Ok(())
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
    .await?;
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

    ensure!(
        unsafe { SetForegroundWindow(scenario.source_hwnd).as_bool() },
        "the native Dock runner could not foreground the source HWND"
    );
    wait_until(cx, "source HWND foreground activation", |_| {
        Ok(unsafe { GetForegroundWindow() } == scenario.source_hwnd)
    })
    .await?;
    *scenario
        .trace
        .lock()
        .map_err(|_| anyhow!("native mouse trace lock was poisoned"))? =
        NativeMouseTrace::default();

    inject_system_pointer(source_point, MOUSEEVENTF_MOVE)?;
    wait_for_cursor(cx, source_point, "source tab pointer move").await?;
    pointer_guard.primary_button_down = true;
    inject_system_pointer(source_point, MOUSEEVENTF_LEFTDOWN)?;
    wait_until(cx, "source HWND pointer capture", |_| {
        let captured = unsafe { GetCapture() } == scenario.source_hwnd;
        let observed_down = scenario
            .trace
            .lock()
            .map_err(|_| anyhow!("native mouse trace lock was poisoned"))?
            .source
            .contains(&NativeMouseKind::Down);
        Ok(captured && observed_down)
    })
    .await?;

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
) -> Result<()> {
    let mut pointer_guard = begin_native_captured_drag(cx, scenario).await?;
    let desktop_point = desktop_point_outside_windows(scenario.source_hwnd, scenario.target_hwnd)?;
    let registered_before_release =
        cx.update(|app| scenario.surface.registered_viewport_spaces(app));

    inject_system_pointer(desktop_point, MOUSEEVENTF_MOVE)?;
    wait_for_cursor(
        cx,
        desktop_point,
        "captured move onto an opaque desktop tear-off target",
    )
    .await?;

    let provisional_trace = Arc::new(Mutex::new(Vec::new()));
    let mut provisional_interceptor = None;
    let mut provisional_window = None;
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
                return Ok(false);
            };
            let runtime_status = cx.update(|app| scenario.surface.viewports().runtime_status(app));
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
    .await?;
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
        !provisional_facts.accepts_pointer_input
            && !provisional_facts.accepts_activation
            && !provisional_facts.focus_on_click,
        "the pre-release provisional must remain input and activation gated: {provisional_facts:?}"
    );
    let provisional_tab = cx.update(|app| {
        unique_matching_debug_bounds(app, provisional_window, "dock:", ":tab:preview")
    })?;
    ensure!(
        provisional_tab.size.width > px(0.0) && provisional_tab.size.height > px(0.0),
        "the provisional's non-empty presented frame must contain the real payload tab"
    );

    let pre_release_snapshot = cx.update(|app| scenario.surface.export_snapshot(app));
    ensure!(
        pre_release_snapshot.revision() == scenario.initial_revision,
        "opening and presenting a provisional must not publish durable surface topology: initial={}, current={}",
        scenario.initial_revision,
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

    inject_system_pointer(desktop_point, MOUSEEVENTF_LEFTUP)?;
    pointer_guard.primary_button_down = false;
    let mut promoted_space = None;
    wait_until(
        cx,
        "same-HWND provisional promotion and interaction-gate release",
        |cx| {
            let promoted = cx.update(|app| -> Result<Option<DockSpaceId>> {
                let new_space = scenario
                    .surface
                    .registered_viewport_spaces(app)
                    .into_iter()
                    .find(|space| {
                        *space != DockSpaceId::from(SPACE)
                            && *space != DockSpaceId::from(SECONDARY_SPACE)
                    });
                let Some(new_space) = new_space else {
                    return Ok(None);
                };
                let graph =
                    DockGraph::import_layout(scenario.surface.export_snapshot(app).layout())?;
                let runtime_status = scenario.surface.viewports().runtime_status(app);
                let facts = window_platform_facts(app, provisional_window)?;
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
                Ok((payload_committed
                    && exact_window_registered
                    && same_hwnd
                    && facts.accepts_pointer_input
                    && facts.accepts_activation
                    && facts.focus_on_click
                    && app.windows().len() == scenario.initial_window_count + 1)
                    .then_some(new_space))
            })?;
            if let Some(space) = promoted {
                promoted_space = Some(space);
                return Ok(unsafe { GetCapture() } != scenario.source_hwnd);
            }
            Ok(false)
        },
    )
    .await?;
    let promoted_space = promoted_space
        .context("same-HWND promotion completed without exposing its committed dock space")?;

    let promoted_snapshot = cx.update(|app| scenario.surface.export_snapshot(app));
    ensure!(
        promoted_snapshot.revision() == scenario.initial_revision + 1,
        "same-HWND promotion must publish exactly one durable surface transaction: initial={}, current={}",
        scenario.initial_revision,
        promoted_snapshot.revision()
    );
    ensure!(
        unsafe { IsWindow(Some(provisional_hwnd)).as_bool() }
            && unsafe { IsWindowVisible(provisional_hwnd).as_bool() },
        "the exact provisional HWND must remain live and visible after promotion"
    );
    let provisional_trace = provisional_trace
        .lock()
        .map_err(|_| anyhow!("provisional native mouse trace lock was poisoned"))?;
    ensure!(
        !provisional_trace.contains(&NativeMouseKind::Move)
            && !provisional_trace.contains(&NativeMouseKind::Down)
            && !provisional_trace.contains(&NativeMouseKind::Up),
        "captured input must never be replayed into the provisional HWND: {provisional_trace:?}"
    );
    log::info!(
        "scenario={} source_hwnd={:?} provisional_hwnd={:?} promoted_space={} revision={} completed",
        NativeDockCase::ProvisionalSameHwndPromotion.id(),
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
) -> Result<()> {
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

    ensure!(
        inject_primary_button_up_best_effort(),
        "the native Dock scenario could not restore the primary button after shutdown"
    );
    pointer_guard.primary_button_down = false;
    log::info!(
        "scenario={} source_hwnd={:?} anchor_hwnd={:?} completed",
        NativeDockCase::SurfaceShutdown.id(),
        scenario.source_hwnd,
        scenario.target_hwnd
    );
    Ok(())
}

fn desktop_point_outside_windows(source: HWND, target: HWND) -> Result<POINT> {
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
    let candidates = [
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
    candidates
        .into_iter()
        .find(|point| {
            virtual_screen.contains(*point)
                && !rect_contains(source_rect, *point)
                && !rect_contains(target_rect, *point)
        })
        .with_context(|| {
            format!(
                "scenario `{}` could not find a desktop point outside source={source_rect:?} and target={target_rect:?} within {virtual_screen:?}",
                NativeDockCase::ProvisionalSameHwndPromotion.id()
            )
        })
}

fn window_rect(hwnd: HWND) -> Result<RECT> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }
        .context("the native Dock scenario could not sample an HWND rectangle")?;
    Ok(rect)
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
