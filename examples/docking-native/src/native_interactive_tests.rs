use super::*;

use anyhow::{Context as _, Result, anyhow, bail, ensure};
use futures::FutureExt as _;
use open_gpui::{
    AnyWindowHandle, AsyncApp, PlatformWindowPresentOutcome, QuitMode, Subscription,
    WindowMouseEvent,
};
use open_gpui_docking::model::DockGraph;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    ffi::c_void,
    mem::size_of,
    panic::AssertUnwindSafe,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Gdi::ClientToScreen,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, GetCapture, INPUT, INPUT_0, INPUT_MOUSE, MOUSE_EVENT_FLAGS,
            MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
            MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, ReleaseCapture, SendInput, VK_LBUTTON,
        },
        WindowsAndMessaging::{
            GetCursorPos, GetForegroundWindow, GetSystemMetrics, IsWindow, IsWindowVisible,
            SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
            SetCursorPos, SetForegroundWindow,
        },
    },
};

const SCENARIO_ID: &str = "docking-native.windows.two-hwnd-captured-drop";
const INPUT_CANARY: usize = 0x4f47_5044;
const SCENARIO_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(5);

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
    ensure_native_interactive_runner();
    let outcome = Arc::new(Mutex::new(None::<std::result::Result<(), String>>));
    let launch_outcome = Arc::clone(&outcome);

    application()
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| match build_native_scenario(cx) {
            Ok(mut scenario) => {
                let task_outcome = Arc::clone(&launch_outcome);
                cx.spawn(async move |cx| {
                    let result = AssertUnwindSafe(run_native_scenario(cx, &mut scenario))
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
        });

    let result = outcome
        .lock()
        .expect("native Dock outcome lock must not be poisoned")
        .take()
        .expect("native Dock scenario must publish a terminal outcome before quitting");
    if let Err(error) = result {
        panic!("native Dock scenario `{SCENARIO_ID}` failed: {error}");
    }
}

fn ensure_native_interactive_runner() {
    assert!(
        std::env::var("OPEN_GPUI_NATIVE_INTERACTIVE")
            .ok()
            .is_some_and(|value| value == "1"),
        "scenario `{SCENARIO_ID}` requires OPEN_GPUI_NATIVE_INTERACTIVE=1 on the named ephemeral runner"
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

    Ok(NativeDockScenario {
        surface,
        source_window,
        target_window,
        source_hwnd,
        target_hwnd,
        initial_revision,
        trace,
        _source_interceptor: source_interceptor,
        _target_interceptor: target_interceptor,
    })
}

async fn run_native_scenario(cx: &mut AsyncApp, scenario: &mut NativeDockScenario) -> Result<()> {
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
    let target_selector_prefix = format!("dock:{SPACE}:tabs:");
    let source_bounds = cx.update(|app| {
        unique_matching_debug_bounds(
            app,
            scenario.source_window,
            &source_selector_prefix,
            ":tab:preview",
        )
    })?;
    let target_bounds = cx.update(|app| {
        unique_matching_debug_bounds(
            app,
            scenario.target_window,
            &target_selector_prefix,
            ":tab:editor",
        )
    })?;
    let source_scale = cx.update(|app| window_scale_factor(app, scenario.source_window))?;
    let target_scale = cx.update(|app| window_scale_factor(app, scenario.target_window))?;
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
    let target_point =
        logical_client_point_to_screen(scenario.target_hwnd, target_bounds.center(), target_scale)?;

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
            Ok(committed && unsafe { GetCapture() } != scenario.source_hwnd)
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
        !trace.target.contains(&NativeMouseKind::Down)
            && !trace.target.contains(&NativeMouseKind::Up),
        "the target HWND must not become a parallel raw-input authority during captured drag: {trace:?}"
    );
    ensure!(
        unsafe { IsWindow(Some(scenario.source_hwnd)).as_bool() }
            && unsafe { IsWindow(Some(scenario.target_hwnd)).as_bool() },
        "both exact HWNDs must remain live through durable drop completion"
    );
    log::info!(
        "scenario={SCENARIO_ID} source_hwnd={:?} target_hwnd={:?} revision={} completed",
        scenario.source_hwnd,
        scenario.target_hwnd,
        snapshot.revision()
    );
    Ok(())
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
            bail!("scenario `{SCENARIO_ID}` timed out waiting for {description}");
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
            .map_err(|error| anyhow!("scenario `{SCENARIO_ID}` could not read HWND: {error:?}"))?
            .as_raw();
        match handle {
            RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut c_void)),
            other => bail!("scenario `{SCENARIO_ID}` expected Win32, received {other:?}"),
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
            "scenario `{SCENARIO_ID}` found no committed selector matching `{prefix}*{suffix}`"
        ),
        _ => bail!(
            "scenario `{SCENARIO_ID}` found ambiguous committed selectors matching `{prefix}*{suffix}`: {matches:?}"
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
        "scenario `{SCENARIO_ID}` could not convert a committed client point for HWND {hwnd:?}"
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
        "scenario `{SCENARIO_ID}` system input was rejected by the desktop or UIPI boundary"
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
            "scenario `{SCENARIO_ID}` requires a non-empty virtual desktop: {bounds:?}"
        );
        Ok(bounds)
    }

    fn absolute_coordinates(self, point: POINT) -> Result<(i32, i32)> {
        let right = i64::from(self.left) + i64::from(self.width);
        let bottom = i64::from(self.top) + i64::from(self.height);
        ensure!(
            i64::from(point.x) >= i64::from(self.left)
                && i64::from(point.x) < right
                && i64::from(point.y) >= i64::from(self.top)
                && i64::from(point.y) < bottom,
            "scenario `{SCENARIO_ID}` injection point is outside the virtual desktop: point={point:?}, bounds={self:?}"
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
