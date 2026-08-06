use serde_json::{Value, json};
use std::{
    env, fs,
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const EXAMPLE_DIR: &str = "crates/gpui_web/examples/smoke_web";
const DIST_DIR: &str = "target/open-gpui-web-smoke/smoke_web";
const WEBGPU_PREFLIGHT_PATH: &str = "/__open-gpui-web-smoke-preflight";
const WEBGPU_PREFLIGHT_HTML: &[u8] = br#"<!doctype html>
<html>
<head><meta charset="utf-8"><title>open-gpui web smoke preflight</title></head>
<body>open-gpui web smoke preflight</body>
</html>
"#;
const BROWSER_START_TIMEOUT: Duration = Duration::from_secs(90);
const BROWSER_TIMEOUT: Duration = Duration::from_secs(30);
const BROWSER_LOG_TAIL_BYTES: usize = 16 * 1024;
const CDP_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(250);

const WEBGPU_PREFLIGHT_EXPRESSION: &str = r#"
(async () => {
    const expectedPath = "/__open-gpui-web-smoke-preflight";
    const path = globalThis.location?.pathname ?? null;
    const readyState = document.readyState;
    if (path !== expectedPath || readyState === "loading") {
        return {
            status: "pending",
            path,
            readyState,
        };
    }

    if (!globalThis.isSecureContext) {
        return {
            status: "unavailable",
            reason: "WebGPU preflight page is not a secure context",
            path,
            readyState,
        };
    }

    const gpu = navigator.gpu ?? null;
    if (!gpu) {
        return {
            status: "unavailable",
            reason: "navigator.gpu is unavailable",
            path,
            readyState,
        };
    }

    try {
        const adapter = await gpu.requestAdapter({ forceFallbackAdapter: true });
        if (!adapter) {
            return {
                status: "unavailable",
                reason: "fallback WebGPU adapter is unavailable",
                path,
                readyState,
            };
        }

        return {
            status: "available",
            featureCount: adapter.features ? Array.from(adapter.features).length : null,
            path,
            readyState,
        };
    } catch (error) {
        return {
            status: "unavailable",
            reason: String(error && (error.stack || error.message || error)),
            path,
            readyState,
        };
    }
})()
"#;

const SMOKE_STATE_EXPRESSION: &str = r#"
(() => {
    const canvas = document.querySelector("canvas");
    const input = document.querySelector("input");
    const rect = canvas ? canvas.getBoundingClientRect() : null;
    const probe = globalThis.__OPEN_GPUI_WEB_SMOKE__ ?? null;
    const diagnostics = globalThis.__OPEN_GPUI_WEB_SMOKE_DIAGNOSTICS__ ?? null;
    return {
        readyState: document.readyState,
        bodyReady: document.body?.dataset?.openGpuiWebSmokeReady === "true",
        canvas: canvas ? {
            count: document.querySelectorAll("canvas").length,
            width: canvas.width,
            height: canvas.height,
            rect: rect ? {
                left: rect.left,
                top: rect.top,
                width: rect.width,
                height: rect.height,
            } : null,
            mousePointerCaptured: canvas.hasPointerCapture(1),
        } : null,
        input: input ? {
            count: document.querySelectorAll("input").length,
            focused: document.activeElement === input,
        } : null,
        probe,
        diagnostics,
    };
})()
"#;

pub(crate) fn web_smoke(root: &Path, allow_unavailable: bool) -> Result<(), ()> {
    ensure_tool("trunk")?;

    let example_dir = root.join(EXAMPLE_DIR);
    let dist_dir = root.join(DIST_DIR);
    fs::create_dir_all(&dist_dir).map_err(|error| {
        eprintln!("failed to create web smoke dist directory: {error}");
    })?;

    run_in_dir(
        &example_dir,
        "trunk",
        &[
            "build",
            "--dist",
            dist_dir
                .to_str()
                .ok_or_else(|| {
                    eprintln!("web smoke dist path is not UTF-8: {}", dist_dir.display());
                })
                .map_err(|_| ())?,
        ],
    )?;

    let server = StaticServer::start(dist_dir).map_err(|error| {
        eprintln!("failed to start web smoke static server: {error}");
    })?;
    let url = format!("http://{}/?smoke=1", server.addr());
    let preflight_url = format!("http://{}{}", server.addr(), WEBGPU_PREFLIGHT_PATH);

    let mut browser = BrowserProcess::launch(&preflight_url).map_err(|error| {
        eprintln!("{error}");
    })?;
    let websocket_url = browser
        .wait_for_page_websocket(&preflight_url)
        .map_err(|error| {
            eprintln!("{error}");
        })?;
    let mut cdp = CdpClient::connect(&websocket_url).map_err(|error| {
        eprintln!("{error}");
    })?;

    enable_browser_domains(&mut cdp).map_err(|error| {
        eprintln!("{error}");
    })?;

    let preflight = wait_for_webgpu_preflight(&mut cdp).map_err(|error| {
        eprintln!("{error}");
    })?;
    match decide_webgpu_preflight(preflight, allow_unavailable) {
        Ok(WebGpuPreflightDecision::Run) => {}
        Ok(WebGpuPreflightDecision::SkipAllowed(reason)) => {
            println!("web smoke explicitly allowed WebGPU-unavailable skip: {reason}");
            return Ok(());
        }
        Err(error) => {
            eprintln!("{error}");
            return Err(());
        }
    }

    cdp.call("Page.navigate", json!({ "url": url }))
        .map_err(|error| {
            eprintln!("{error}");
        })?;

    run_browser_smoke(&mut cdp).map_err(|error| {
        eprintln!("{error}");
    })?;

    for (mode, accepts_activation, focus_on_click) in [
        ("programmatic-only", true, false),
        ("click-only", false, true),
    ] {
        let policy_url = format!("{url}&activation={mode}");
        cdp.call("Page.navigate", json!({ "url": policy_url }))
            .map_err(|error| {
                eprintln!("{error}");
            })?;
        run_activation_policy_smoke(&mut cdp, mode, accepts_activation, focus_on_click).map_err(
            |error| {
                eprintln!("{error}");
            },
        )?;
    }

    println!(
        "web smoke passed: default input and activation flow, independent programmatic-only and click-only activation policies, platform viewport capability gate, and DockSurface unsupported path"
    );

    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum WebGpuPreflight {
    Pending,
    Available,
    Unavailable(String),
}

#[derive(Debug, PartialEq, Eq)]
enum WebGpuPreflightDecision {
    Run,
    SkipAllowed(String),
}

fn decide_webgpu_preflight(
    preflight: WebGpuPreflight,
    allow_unavailable: bool,
) -> Result<WebGpuPreflightDecision, String> {
    match preflight {
        WebGpuPreflight::Available => Ok(WebGpuPreflightDecision::Run),
        WebGpuPreflight::Unavailable(reason) if allow_unavailable => {
            Ok(WebGpuPreflightDecision::SkipAllowed(reason))
        }
        WebGpuPreflight::Unavailable(reason) => Err(format!(
            "WebGPU is unavailable, so the browser behavior gate cannot run: {reason}. Pass `--allow-unavailable` only for an explicit local diagnostic skip."
        )),
        WebGpuPreflight::Pending => Err("WebGPU preflight remained pending after wait".to_string()),
    }
}

fn enable_browser_domains(cdp: &mut CdpClient) -> Result<(), String> {
    cdp.call("Page.enable", json!({}))?;
    cdp.call("Runtime.enable", json!({}))?;
    Ok(())
}

fn wait_for_webgpu_preflight(cdp: &mut CdpClient) -> Result<WebGpuPreflight, String> {
    let started = Instant::now();
    let mut last_state = Value::Null;
    while started.elapsed() < BROWSER_TIMEOUT {
        match cdp.evaluate(WEBGPU_PREFLIGHT_EXPRESSION) {
            Ok(state) => match webgpu_preflight_result(&state)? {
                WebGpuPreflight::Pending => {
                    last_state = state;
                }
                result => return Ok(result),
            },
            Err(error) => {
                last_state = json!({ "evaluation_error": error });
            }
        }
        thread::sleep(POLL_INTERVAL);
    }

    Err(format!(
        "timed out waiting for WebGPU preflight page; last state: {last_state}"
    ))
}

fn webgpu_preflight_result(state: &Value) -> Result<WebGpuPreflight, String> {
    match state.pointer("/status").and_then(Value::as_str) {
        Some("pending") => Ok(WebGpuPreflight::Pending),
        Some("available") => Ok(WebGpuPreflight::Available),
        Some("unavailable") => {
            let reason = state
                .pointer("/reason")
                .and_then(Value::as_str)
                .unwrap_or("browser WebGPU fallback adapter is unavailable")
                .to_string();
            Ok(WebGpuPreflight::Unavailable(reason))
        }
        _ => Err(format!(
            "WebGPU preflight returned malformed state: {state}"
        )),
    }
}

fn run_browser_smoke(cdp: &mut CdpClient) -> Result<(), String> {
    enable_browser_domains(cdp)?;

    let ready = wait_for_state(cdp, "app ready, canvas, and hidden input", |state| {
        canvas_ready(state)
            && state.pointer("/input/count").and_then(Value::as_u64) == Some(1)
            && state.pointer("/probe/ready").and_then(Value::as_bool) == Some(true)
    })?;

    let rect = ready
        .pointer("/canvas/rect")
        .ok_or_else(|| "web smoke state did not include canvas bounds".to_string())?;
    let center_x = number_field(rect, "left")? + number_field(rect, "width")? / 2.0;
    let center_y = number_field(rect, "top")? + number_field(rect, "height")? / 2.0;

    cdp.mouse_click(center_x, center_y)?;
    wait_for_state(
        cdp,
        "pointer click delivery and hidden input focus",
        |state| {
            state.pointer("/probe/clickEvents").and_then(Value::as_u64) == Some(1)
                && state.pointer("/input/focused").and_then(Value::as_bool) == Some(true)
                && state
                    .pointer("/probe/shellInteractions")
                    .and_then(Value::as_u64)
                    .is_some_and(|count| count >= 1)
        },
    )?;

    cdp.key_press("s", "KeyS", 83)?;
    let keyboard_state = wait_for_state(cdp, "keyboard delivery", |state| {
        state.pointer("/probe/keyEvents").and_then(Value::as_u64) == Some(1)
            && state.pointer("/input/focused").and_then(Value::as_bool) == Some(true)
            && state
                .pointer("/probe/shellInteractions")
                .and_then(Value::as_u64)
                .is_some_and(|count| count >= 2)
    })?;

    let pointer_moves_before_capture = keyboard_state
        .pointer("/probe/pointerMoveEvents")
        .and_then(Value::as_u64)
        .ok_or_else(|| "web smoke probe did not expose pointer move events".to_string())?;
    cdp.mouse_down(center_x, center_y)?;
    wait_for_state(cdp, "DOM and GPUI pointer capture", |state| {
        state
            .pointer("/canvas/mousePointerCaptured")
            .and_then(Value::as_bool)
            == Some(true)
            && state
                .pointer("/probe/pointerCaptureRequests")
                .and_then(Value::as_u64)
                == Some(2)
    })?;

    let outside_x = number_field(rect, "left")? + number_field(rect, "width")? + 20.0;
    let outside_y = center_y;
    cdp.mouse_move(outside_x, outside_y, 1)?;
    wait_for_state(cdp, "captured pointer move outside canvas", |state| {
        state
            .pointer("/probe/pointerMoveEvents")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > pointer_moves_before_capture)
            && state
                .pointer("/canvas/mousePointerCaptured")
                .and_then(Value::as_bool)
                == Some(true)
    })?;

    cdp.evaluate(
        r#"(() => {
            const canvas = document.querySelector("canvas");
            if (!canvas || !canvas.hasPointerCapture(1)) {
                throw new Error("web smoke canvas does not own pointer 1");
            }
            canvas.releasePointerCapture(1);
            return true;
        })()"#,
    )?;
    cdp.mouse_move(outside_x, outside_y, 1)?;
    wait_for_state(cdp, "lost pointer capture cancellation", |state| {
        state
            .pointer("/canvas/mousePointerCaptured")
            .and_then(Value::as_bool)
            == Some(false)
            && state
                .pointer("/probe/pointerCancelEvents")
                .and_then(Value::as_u64)
                == Some(1)
            && state
                .pointer("/probe/platformCaptureLostEvents")
                .and_then(Value::as_u64)
                == Some(1)
    })?;

    cdp.mouse_up(outside_x, outside_y)?;
    wait_for_state(
        cdp,
        "terminal pointer cancellation remains unique",
        |state| {
            state
                .pointer("/probe/pointerCancelEvents")
                .and_then(Value::as_u64)
                == Some(1)
                && state
                    .pointer("/probe/platformCaptureLostEvents")
                    .and_then(Value::as_u64)
                    == Some(1)
                && state.pointer("/probe/clickEvents").and_then(Value::as_u64) == Some(1)
        },
    )?;

    cdp.mouse_down(center_x, center_y)?;
    wait_for_state(cdp, "pointer capture before input blur", |state| {
        state
            .pointer("/canvas/mousePointerCaptured")
            .and_then(Value::as_bool)
            == Some(true)
            && state
                .pointer("/probe/pointerCaptureRequests")
                .and_then(Value::as_u64)
                == Some(3)
            && state.pointer("/input/focused").and_then(Value::as_bool) == Some(true)
    })?;
    cdp.evaluate(
        r#"(() => {
            const input = document.querySelector("input");
            if (!input) {
                throw new Error("web smoke input is missing");
            }
            input.blur();
            return true;
        })()"#,
    )?;
    wait_for_state(cdp, "input blur pointer cancellation", |state| {
        state
            .pointer("/canvas/mousePointerCaptured")
            .and_then(Value::as_bool)
            == Some(false)
            && state
                .pointer("/probe/pointerCancelEvents")
                .and_then(Value::as_u64)
                == Some(2)
            && state
                .pointer("/probe/platformCaptureLostEvents")
                .and_then(Value::as_u64)
                == Some(1)
            && state
                .pointer("/probe/windowDeactivatedEvents")
                .and_then(Value::as_u64)
                == Some(1)
    })?;
    cdp.mouse_up(outside_x, outside_y)?;

    cdp.mouse_down(center_x, center_y)?;
    wait_for_state(
        cdp,
        "pointer capture before repeated window blur",
        |state| {
            state
                .pointer("/canvas/mousePointerCaptured")
                .and_then(Value::as_bool)
                == Some(true)
                && state
                    .pointer("/probe/pointerCaptureRequests")
                    .and_then(Value::as_u64)
                    == Some(4)
                && state.pointer("/input/focused").and_then(Value::as_bool) == Some(true)
        },
    )?;
    cdp.evaluate(
        r#"(() => {
            Object.defineProperty(document, "hasFocus", {
                configurable: true,
                value: () => false,
            });
            window.dispatchEvent(new Event("blur"));
            window.dispatchEvent(new Event("blur"));
            return true;
        })()"#,
    )?;
    wait_for_state(
        cdp,
        "repeated window blur emits one deactivation edge",
        |state| {
            state
                .pointer("/canvas/mousePointerCaptured")
                .and_then(Value::as_bool)
                == Some(false)
                && state
                    .pointer("/probe/pointerCancelEvents")
                    .and_then(Value::as_u64)
                    == Some(3)
                && state
                    .pointer("/probe/platformCaptureLostEvents")
                    .and_then(Value::as_u64)
                    == Some(1)
                && state
                    .pointer("/probe/windowDeactivatedEvents")
                    .and_then(Value::as_u64)
                    == Some(2)
        },
    )?;
    cdp.mouse_up(outside_x, outside_y)?;

    cdp.evaluate(
        r#"(async () => {
            delete document.hasFocus;
            delete document.visibilityState;
            window.dispatchEvent(new Event("focus"));
            const input = document.querySelector("input");
            if (!document.hasFocus()
                || document.visibilityState !== "visible"
                || document.activeElement !== input) {
                throw new Error("window focus recovery did not restore DOM activation facts");
            }
            await new Promise((resolve) => {
                requestAnimationFrame(() => requestAnimationFrame(resolve));
            });
            return true;
        })()"#,
    )?;
    cdp.mouse_down(center_x, center_y)?;
    wait_for_state(cdp, "capture after window focus recovery", |state| {
        state
            .pointer("/canvas/mousePointerCaptured")
            .and_then(Value::as_bool)
            == Some(true)
            && state
                .pointer("/probe/pointerCaptureRequests")
                .and_then(Value::as_u64)
                == Some(5)
    })?;
    cdp.evaluate(
        r#"(() => {
            Object.defineProperty(document, "visibilityState", {
                configurable: true,
                get: () => "hidden",
            });
            document.dispatchEvent(new Event("visibilitychange"));
            document.dispatchEvent(new Event("visibilitychange"));
            Object.defineProperty(document, "hasFocus", {
                configurable: true,
                value: () => false,
            });
            window.dispatchEvent(new Event("blur"));
            window.dispatchEvent(new Event("blur"));
            return true;
        })()"#,
    )?;
    wait_for_state(
        cdp,
        "hidden and blur signals emit one deactivation edge",
        |state| {
            state
                .pointer("/canvas/mousePointerCaptured")
                .and_then(Value::as_bool)
                == Some(false)
                && state
                    .pointer("/probe/pointerCancelEvents")
                    .and_then(Value::as_u64)
                    == Some(4)
                && state
                    .pointer("/probe/windowDeactivatedEvents")
                    .and_then(Value::as_u64)
                    == Some(3)
        },
    )?;
    cdp.mouse_up(outside_x, outside_y)?;

    cdp.evaluate(
        r#"(async () => {
            delete document.hasFocus;
            delete document.visibilityState;
            document.dispatchEvent(new Event("visibilitychange"));
            window.dispatchEvent(new Event("focus"));
            const input = document.querySelector("input");
            if (!document.hasFocus()
                || document.visibilityState !== "visible"
                || document.activeElement !== input) {
                throw new Error("visible recovery did not restore DOM activation facts");
            }
            await new Promise((resolve) => {
                requestAnimationFrame(() => requestAnimationFrame(resolve));
            });
            return true;
        })()"#,
    )?;
    cdp.mouse_down(center_x, center_y)?;
    wait_for_state(cdp, "capture after visible recovery", |state| {
        state
            .pointer("/canvas/mousePointerCaptured")
            .and_then(Value::as_bool)
            == Some(true)
            && state
                .pointer("/probe/pointerCaptureRequests")
                .and_then(Value::as_u64)
                == Some(6)
    })?;
    cdp.evaluate(
        r#"(() => {
            Object.defineProperty(document, "hasFocus", {
                configurable: true,
                value: () => false,
            });
            window.dispatchEvent(new Event("blur"));
            window.dispatchEvent(new Event("blur"));
            Object.defineProperty(document, "visibilityState", {
                configurable: true,
                get: () => "hidden",
            });
            document.dispatchEvent(new Event("visibilitychange"));
            document.dispatchEvent(new Event("visibilitychange"));
            return true;
        })()"#,
    )?;
    wait_for_state(
        cdp,
        "blur and hidden signals emit one deactivation edge",
        |state| {
            state
                .pointer("/canvas/mousePointerCaptured")
                .and_then(Value::as_bool)
                == Some(false)
                && state
                    .pointer("/probe/pointerCancelEvents")
                    .and_then(Value::as_u64)
                    == Some(5)
                && state
                    .pointer("/probe/windowDeactivatedEvents")
                    .and_then(Value::as_u64)
                    == Some(4)
        },
    )?;
    cdp.mouse_up(outside_x, outside_y)?;

    cdp.evaluate(
        r#"(() => {
            delete document.hasFocus;
            delete document.visibilityState;
            const restored = document.hasFocus()
                && document.visibilityState === "visible"
                && document.activeElement === document.querySelector("input");
            if (!restored) {
                throw new Error("already-focused reactivation prerequisites were not restored");
            }
            return true;
        })()"#,
    )?;
    cdp.mouse_down(center_x, center_y)?;
    wait_for_state(cdp, "already-focused input reactivation", |state| {
        state
            .pointer("/canvas/mousePointerCaptured")
            .and_then(Value::as_bool)
            == Some(true)
            && state
                .pointer("/probe/pointerCaptureRequests")
                .and_then(Value::as_u64)
                == Some(7)
            && state.pointer("/input/focused").and_then(Value::as_bool) == Some(true)
    })?;
    cdp.evaluate(
        r#"(() => {
            const input = document.querySelector("input");
            if (!input) {
                throw new Error("web smoke input is missing");
            }
            input.blur();
            input.dispatchEvent(new Event("blur"));
            Object.defineProperty(document, "visibilityState", {
                configurable: true,
                get: () => "hidden",
            });
            document.dispatchEvent(new Event("visibilitychange"));
            Object.defineProperty(document, "hasFocus", {
                configurable: true,
                value: () => false,
            });
            window.dispatchEvent(new Event("blur"));
            delete document.hasFocus;
            delete document.visibilityState;
            return true;
        })()"#,
    )?;
    wait_for_state(cdp, "reactivated input blur", |state| {
        state
            .pointer("/canvas/mousePointerCaptured")
            .and_then(Value::as_bool)
            == Some(false)
            && state
                .pointer("/probe/pointerCancelEvents")
                .and_then(Value::as_u64)
                == Some(6)
            && state
                .pointer("/probe/windowDeactivatedEvents")
                .and_then(Value::as_u64)
                == Some(5)
    })?;
    cdp.mouse_up(outside_x, outside_y)?;

    let final_state = wait_for_state(cdp, "DOM activation edges remain unique", |state| {
        state
            .pointer("/probe/pointerCancelEvents")
            .and_then(Value::as_u64)
            == Some(6)
            && state
                .pointer("/probe/windowDeactivatedEvents")
                .and_then(Value::as_u64)
                == Some(5)
            && state.pointer("/probe/clickEvents").and_then(Value::as_u64) == Some(1)
    })?;

    if final_state
        .pointer("/probe/platformViewportWindows")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(
            "web smoke expected platform viewport windows to be an unsupported capability"
                .to_string(),
        );
    }

    if final_state
        .pointer("/probe/dockingViewportReadiness")
        .and_then(Value::as_str)
        != Some("backend_unsupported")
    {
        return Err(
            "web smoke expected DockSurface viewport readiness to report backend_unsupported"
                .to_string(),
        );
    }

    if final_state
        .pointer("/probe/dockingViewportOutcome")
        .and_then(Value::as_str)
        != Some("backend_unsupported")
    {
        return Err(
            "web smoke expected DockSurface viewport open outcome to report backend_unsupported"
                .to_string(),
        );
    }

    if final_state
        .pointer("/probe/dockingViewportOpened")
        .and_then(Value::as_bool)
        != Some(false)
        || final_state
            .pointer("/probe/dockingViewportWindowDelta")
            .and_then(Value::as_u64)
            != Some(0)
        || final_state
            .pointer("/probe/dockingViewportRegisteredSpaces")
            .and_then(Value::as_u64)
            != Some(0)
    {
        return Err(
            "web smoke expected unsupported DockSurface viewport request to avoid windows and registrations"
                .to_string(),
        );
    }

    Ok(())
}

fn run_activation_policy_smoke(
    cdp: &mut CdpClient,
    mode: &str,
    accepts_activation: bool,
    focus_on_click: bool,
) -> Result<(), String> {
    let ready = wait_for_state(cdp, &format!("{mode} activation policy"), |state| {
        canvas_ready(state)
            && state.pointer("/input/count").and_then(Value::as_u64) == Some(1)
            && state
                .pointer("/probe/activationMode")
                .and_then(Value::as_str)
                == Some(mode)
            && state
                .pointer("/probe/requestedAcceptsActivation")
                .and_then(Value::as_bool)
                == Some(accepts_activation)
            && state
                .pointer("/probe/requestedFocusOnClick")
                .and_then(Value::as_bool)
                == Some(focus_on_click)
            && state
                .pointer("/probe/creationFocusOnAppearing")
                .and_then(Value::as_bool)
                == Some(false)
            && state
                .pointer("/probe/observedAcceptsActivation")
                .and_then(Value::as_bool)
                == Some(accepts_activation)
            && state
                .pointer("/probe/observedFocusOnClick")
                .and_then(Value::as_bool)
                == Some(focus_on_click)
            && state
                .pointer("/probe/programmaticActivationAttempted")
                .and_then(Value::as_bool)
                == Some(true)
            && state.pointer("/input/focused").and_then(Value::as_bool) == Some(accepts_activation)
            && state
                .pointer("/probe/observedActive")
                .and_then(Value::as_bool)
                == Some(accepts_activation)
    })?;

    let rect = ready
        .pointer("/canvas/rect")
        .ok_or_else(|| format!("{mode} activation probe did not include canvas bounds"))?;
    let center_x = number_field(rect, "left")? + number_field(rect, "width")? / 2.0;
    let center_y = number_field(rect, "top")? + number_field(rect, "height")? / 2.0;

    cdp.evaluate(
        r#"(() => {
            const input = document.querySelector("input");
            if (!input) {
                throw new Error("activation probe input is missing");
            }
            input.blur();
            return true;
        })()"#,
    )?;
    wait_for_state(cdp, &format!("{mode} input blur"), |state| {
        state.pointer("/input/focused").and_then(Value::as_bool) == Some(false)
            && state
                .pointer("/probe/observedActive")
                .and_then(Value::as_bool)
                == Some(false)
    })?;

    cdp.mouse_click(center_x, center_y)?;
    wait_for_state(cdp, &format!("{mode} click focus"), |state| {
        state.pointer("/probe/clickEvents").and_then(Value::as_u64) == Some(1)
            && state.pointer("/input/focused").and_then(Value::as_bool) == Some(focus_on_click)
            && state
                .pointer("/probe/observedActive")
                .and_then(Value::as_bool)
                == Some(focus_on_click)
    })?;

    Ok(())
}

fn wait_for_state(
    cdp: &mut CdpClient,
    label: &str,
    predicate: impl Fn(&Value) -> bool,
) -> Result<Value, String> {
    let started = Instant::now();
    let mut last_state = Value::Null;
    while started.elapsed() < BROWSER_TIMEOUT {
        match cdp.evaluate(SMOKE_STATE_EXPRESSION) {
            Ok(state) => {
                if predicate(&state) {
                    return Ok(state);
                }
                last_state = state;
            }
            Err(error) => {
                last_state = json!({ "evaluation_error": error });
            }
        }
        thread::sleep(POLL_INTERVAL);
    }

    Err(format!(
        "timed out waiting for web smoke condition `{label}`; last state: {last_state}"
    ))
}

fn canvas_ready(state: &Value) -> bool {
    state.pointer("/canvas/count").and_then(Value::as_u64) == Some(1)
        && state
            .pointer("/canvas/width")
            .and_then(Value::as_u64)
            .is_some_and(|width| width > 0)
        && state
            .pointer("/canvas/height")
            .and_then(Value::as_u64)
            .is_some_and(|height| height > 0)
        && state
            .pointer("/canvas/rect/width")
            .and_then(Value::as_f64)
            .is_some_and(|width| width > 0.0)
        && state
            .pointer("/canvas/rect/height")
            .and_then(Value::as_f64)
            .is_some_and(|height| height > 0.0)
}

fn number_field(value: &Value, field: &str) -> Result<f64, String> {
    value
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("web smoke state missing numeric `{field}` in {value}"))
}

fn ensure_tool(program: &str) -> Result<(), ()> {
    let available = Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());

    if available {
        Ok(())
    } else {
        eprintln!("`{program}` is required for `xtask web-smoke`");
        Err(())
    }
}

fn run_in_dir(cwd: &Path, program: &str, args: &[&str]) -> Result<(), ()> {
    let display = std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    println!("==> (cd {} && {display})", cwd.display());

    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env_remove("NO_COLOR")
        .status()
        .map_err(|error| {
            eprintln!("failed to run `{display}`: {error}");
        })?;

    if status.success() {
        Ok(())
    } else {
        eprintln!("command failed: {display}");
        Err(())
    }
}

struct StaticServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl StaticServer {
    fn start(root: PathBuf) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if let Err(error) = serve_static_request(stream, &root) {
                            if !matches!(
                                error.kind(),
                                ErrorKind::BrokenPipe
                                    | ErrorKind::ConnectionAborted
                                    | ErrorKind::ConnectionReset
                            ) {
                                eprintln!("web smoke server request failed: {error}");
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => {
                        eprintln!("web smoke server accept failed: {error}");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            addr,
            stop,
            thread: Some(thread),
        })
    }

    const fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for StaticServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve_static_request(mut stream: TcpStream, root: &Path) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let Some(request_line) = request.lines().next() else {
        return write_response(&mut stream, 400, "text/plain", b"bad request");
    };
    let parts = request_line.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 || parts[0] != "GET" {
        return write_response(&mut stream, 405, "text/plain", b"method not allowed");
    }
    if is_webgpu_preflight_request(parts[1]) {
        return write_response(
            &mut stream,
            200,
            "text/html; charset=utf-8",
            WEBGPU_PREFLIGHT_HTML,
        );
    }

    let Some(relative_path) = request_path_to_file(parts[1]) else {
        return write_response(&mut stream, 400, "text/plain", b"bad path");
    };
    let path = root.join(&relative_path);
    let Ok(bytes) = fs::read(&path) else {
        return write_response(&mut stream, 404, "text/plain", b"not found");
    };

    write_response(&mut stream, 200, content_type(&relative_path), &bytes)
}

fn is_webgpu_preflight_request(path: &str) -> bool {
    path.split_once('?').map_or(path, |(path, _)| path) == WEBGPU_PREFLIGHT_PATH
}

fn request_path_to_file(path: &str) -> Option<PathBuf> {
    let path = path.split_once('?').map_or(path, |(path, _)| path);
    let path = path.trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if path
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
        || path.contains('\\')
    {
        return None;
    }

    Some(PathBuf::from(path))
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Length: {}\r\n\
         Content-Type: {content_type}\r\n\
         Cache-Control: no-store\r\n\
         Cross-Origin-Embedder-Policy: require-corp\r\n\
         Cross-Origin-Opener-Policy: same-origin\r\n\
         Cross-Origin-Resource-Policy: cross-origin\r\n\
         Connection: close\r\n\
         \r\n",
        body.len()
    )?;
    stream.write_all(body)
}

struct BrowserProcess {
    child: Child,
    remote_port: u16,
    user_data_dir: PathBuf,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

impl BrowserProcess {
    fn launch(url: &str) -> Result<Self, String> {
        let browser = find_browser().ok_or_else(|| {
            "no Chrome/Chromium/Edge executable found; set OPEN_GPUI_WEB_SMOKE_BROWSER to run `xtask web-smoke`".to_string()
        })?;
        let remote_port = reserve_port()?;
        let user_data_dir = env::temp_dir().join(format!(
            "open-gpui-web-smoke-{}-{remote_port}",
            std::process::id()
        ));
        fs::create_dir_all(&user_data_dir)
            .map_err(|error| format!("failed to create browser profile dir: {error}"))?;
        let stdout_path = user_data_dir.join("browser.stdout.log");
        let stderr_path = user_data_dir.join("browser.stderr.log");
        let stdout = fs::File::create(&stdout_path)
            .map_err(|error| format!("failed to create browser stdout log: {error}"))?;
        let stderr = fs::File::create(&stderr_path)
            .map_err(|error| format!("failed to create browser stderr log: {error}"))?;

        let mut args = vec![
            "--headless=new".to_string(),
            "--disable-background-networking".to_string(),
            "--disable-dev-shm-usage".to_string(),
            "--disable-gpu-sandbox".to_string(),
            "--no-first-run".to_string(),
            "--no-sandbox".to_string(),
            "--enable-unsafe-webgpu".to_string(),
            "--remote-allow-origins=*".to_string(),
            "--remote-debugging-address=127.0.0.1".to_string(),
            format!("--remote-debugging-port={remote_port}"),
            format!("--user-data-dir={}", user_data_dir.display()),
        ];
        if cfg!(target_os = "linux") {
            args.push("--ignore-gpu-blocklist".to_string());
            args.push("--enable-unsafe-swiftshader".to_string());
            args.push("--use-gl=angle".to_string());
            args.push("--use-angle=swiftshader".to_string());
            args.push("--use-vulkan=swiftshader".to_string());
            args.push("--enable-features=Vulkan,VulkanFromANGLE".to_string());
        }
        if let Ok(extra_args) = env::var("OPEN_GPUI_WEB_SMOKE_BROWSER_ARGS") {
            args.extend(extra_args.split_whitespace().map(ToOwned::to_owned));
        }
        args.push(url.to_string());

        println!("==> {} {}", browser.display(), args.join(" "));
        let child = Command::new(&browser)
            .args(&args)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|error| {
                format!("failed to launch browser `{}`: {error}", browser.display())
            })?;

        Ok(Self {
            child,
            remote_port,
            user_data_dir,
            stdout_path,
            stderr_path,
        })
    }

    fn wait_for_page_websocket(&mut self, url: &str) -> Result<String, String> {
        let started = Instant::now();
        let mut last_error = String::new();
        while started.elapsed() < BROWSER_START_TIMEOUT {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|error| format!("failed to inspect browser status: {error}"))?
            {
                return Err(format!(
                    "browser exited before web smoke connected: {status}\n{}",
                    self.output_summary()
                ));
            }

            match http_get_json(self.remote_port, "/json/list") {
                Ok(Value::Array(pages)) => {
                    if let Some(websocket_url) = pages.iter().find_map(|page| {
                        let page_type = page.get("type").and_then(Value::as_str)?;
                        let page_url = page.get("url").and_then(Value::as_str)?;
                        let websocket_url =
                            page.get("webSocketDebuggerUrl").and_then(Value::as_str)?;
                        (page_type == "page" && page_url.starts_with(url))
                            .then(|| websocket_url.to_owned())
                    }) {
                        return Ok(websocket_url);
                    }
                    last_error = format!("browser page list did not include `{url}`: {pages:?}");
                }
                Ok(other) => {
                    last_error = format!("unexpected browser page list response: {other}");
                }
                Err(error) => {
                    last_error = error;
                }
            }
            thread::sleep(POLL_INTERVAL);
        }

        let _ = self.child.kill();
        let _ = self.child.wait();
        Err(format!(
            "timed out waiting for browser remote debugging page after {:?}; last error: {last_error}\n{}",
            BROWSER_START_TIMEOUT,
            self.output_summary()
        ))
    }

    fn output_summary(&self) -> String {
        format!(
            "browser stdout tail:\n{}\nbrowser stderr tail:\n{}",
            read_log_tail(&self.stdout_path),
            read_log_tail(&self.stderr_path)
        )
    }
}

impl Drop for BrowserProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.user_data_dir);
    }
}

fn reserve_port() -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("failed to reserve local port: {error}"))?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|error| format!("failed to read reserved local port: {error}"))
}

fn find_browser() -> Option<PathBuf> {
    for env_var in [
        "OPEN_GPUI_WEB_SMOKE_BROWSER",
        "CHROME",
        "CHROME_BIN",
        "CHROMIUM_BIN",
        "EDGE_BIN",
    ] {
        if let Some(path) = env::var_os(env_var).map(PathBuf::from) {
            if path.is_file() {
                return Some(path);
            }
        }
    }

    for path in platform_browser_candidates() {
        if path.is_file() {
            return Some(path);
        }
    }

    for name in [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "microsoft-edge",
        "microsoft-edge-stable",
        "msedge",
    ] {
        if let Some(path) = find_in_path(name) {
            return Some(path);
        }
    }

    None
}

fn platform_browser_candidates() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        vec![
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into(),
            "/Applications/Chromium.app/Contents/MacOS/Chromium".into(),
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge".into(),
        ]
    }

    #[cfg(target_os = "windows")]
    {
        let mut candidates = Vec::new();
        for env_var in ["ProgramFiles", "ProgramFiles(x86)", "LocalAppData"] {
            if let Some(root) = env::var_os(env_var).map(PathBuf::from) {
                candidates.push(root.join("Google/Chrome/Application/chrome.exe"));
                candidates.push(root.join("Chromium/Application/chrome.exe"));
                candidates.push(root.join("Microsoft/Edge/Application/msedge.exe"));
            }
        }
        candidates
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    env::split_paths(&paths)
        .map(|path| path.join(name))
        .find(|candidate| candidate.is_file())
}

fn read_log_tail(path: &Path) -> String {
    let Ok(bytes) = fs::read(path) else {
        return "<unavailable>".to_string();
    };
    if bytes.is_empty() {
        return "<empty>".to_string();
    }
    let start = bytes.len().saturating_sub(BROWSER_LOG_TAIL_BYTES);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

fn http_get_json(port: u16, path: &str) -> Result<Value, String> {
    let response = http_get(port, path)?;
    serde_json::from_str(&response).map_err(|error| format!("invalid browser JSON: {error}"))
}

fn http_get(port: u16, path: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .map_err(|error| format!("failed to connect to browser debugging port {port}: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("failed to set browser HTTP timeout: {error}"))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| format!("failed to write browser HTTP request: {error}"))?;

    let response = read_http_response(&mut stream, Duration::from_secs(2))?;
    let (headers, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| format!("malformed browser HTTP response: {response}"))?;
    if !headers.starts_with("HTTP/1.1 200") && !headers.starts_with("HTTP/1.0 200") {
        return Err(format!("browser HTTP request failed: {headers}"));
    }
    Ok(body.to_string())
}

fn read_http_response(stream: &mut TcpStream, timeout: Duration) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    let mut response = Vec::new();
    let mut buffer = [0_u8; 4096];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => response.extend_from_slice(&buffer[..read]),
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                if Instant::now() >= deadline {
                    if response.is_empty() {
                        return Err(format!("timed out reading browser HTTP response: {error}"));
                    }
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                return Err(format!("failed to read browser HTTP response: {error}"));
            }
        }
    }

    String::from_utf8(response)
        .map_err(|error| format!("browser HTTP response was not UTF-8: {error}"))
}

struct CdpClient {
    stream: TcpStream,
    next_id: u64,
}

impl CdpClient {
    fn connect(websocket_url: &str) -> Result<Self, String> {
        let (host, port, path) = parse_ws_url(websocket_url)?;
        let mut stream = TcpStream::connect((host.as_str(), port))
            .map_err(|error| format!("failed to connect to CDP websocket: {error}"))?;
        stream
            .set_read_timeout(Some(CDP_TIMEOUT))
            .map_err(|error| format!("failed to set CDP read timeout: {error}"))?;
        stream
            .set_write_timeout(Some(CDP_TIMEOUT))
            .map_err(|error| format!("failed to set CDP write timeout: {error}"))?;

        write!(
            stream,
            "GET {path} HTTP/1.1\r\n\
             Host: {host}:{port}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
             Sec-WebSocket-Version: 13\r\n\
             \r\n"
        )
        .map_err(|error| format!("failed to write CDP websocket handshake: {error}"))?;

        let mut headers = Vec::new();
        let mut byte = [0_u8; 1];
        while !headers.ends_with(b"\r\n\r\n") {
            stream
                .read_exact(&mut byte)
                .map_err(|error| format!("failed to read CDP websocket handshake: {error}"))?;
            headers.push(byte[0]);
            if headers.len() > 8192 {
                return Err("CDP websocket handshake headers were too large".to_string());
            }
        }
        let headers = String::from_utf8_lossy(&headers);
        if !headers.starts_with("HTTP/1.1 101") && !headers.starts_with("HTTP/1.0 101") {
            return Err(format!("CDP websocket handshake failed: {headers}"));
        }

        Ok(Self { stream, next_id: 1 })
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let payload = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_text_frame(&payload.to_string())?;

        loop {
            let message = self.read_text_frame()?;
            let value = serde_json::from_str::<Value>(&message)
                .map_err(|error| format!("invalid CDP JSON: {error}; message: {message}"))?;
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(format!("CDP call `{method}` failed: {error}"));
            }
            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    fn evaluate(&mut self, expression: &str) -> Result<Value, String> {
        let result = self.call(
            "Runtime.evaluate",
            json!({
                "expression": expression,
                "awaitPromise": true,
                "returnByValue": true,
            }),
        )?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(format!("browser evaluation threw: {exception}"));
        }
        result
            .pointer("/result/value")
            .cloned()
            .ok_or_else(|| format!("browser evaluation returned no JSON value: {result}"))
    }

    fn mouse_click(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.mouse_move(x, y, 0)?;
        self.mouse_down(x, y)?;
        self.mouse_up(x, y)
    }

    fn mouse_move(&mut self, x: f64, y: f64, buttons: u8) -> Result<(), String> {
        let button = if buttons & 1 == 1 { "left" } else { "none" };
        self.call(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseMoved",
                "x": x,
                "y": y,
                "button": button,
                "buttons": buttons,
            }),
        )?;
        Ok(())
    }

    fn mouse_down(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.call(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mousePressed",
                "x": x,
                "y": y,
                "button": "left",
                "buttons": 1,
                "clickCount": 1,
            }),
        )?;
        Ok(())
    }

    fn mouse_up(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.call(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseReleased",
                "x": x,
                "y": y,
                "button": "left",
                "buttons": 0,
                "clickCount": 1,
            }),
        )?;
        Ok(())
    }

    fn key_press(&mut self, key: &str, code: &str, virtual_key: u32) -> Result<(), String> {
        self.call(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyDown",
                "key": key,
                "code": code,
                "text": key,
                "unmodifiedText": key,
                "windowsVirtualKeyCode": virtual_key,
                "nativeVirtualKeyCode": virtual_key,
            }),
        )?;
        self.call(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "key": key,
                "code": code,
                "windowsVirtualKeyCode": virtual_key,
                "nativeVirtualKeyCode": virtual_key,
            }),
        )?;
        Ok(())
    }

    fn write_text_frame(&mut self, message: &str) -> Result<(), String> {
        let payload = message.as_bytes();
        let mut frame = Vec::with_capacity(payload.len() + 14);
        frame.push(0x81);
        if payload.len() < 126 {
            frame.push(0x80 | payload.len() as u8);
        } else if payload.len() <= u16::MAX as usize {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        }
        let mask = [0x13, 0x37, 0x42, 0x99];
        frame.extend_from_slice(&mask);
        for (index, byte) in payload.iter().enumerate() {
            frame.push(byte ^ mask[index % mask.len()]);
        }
        self.stream
            .write_all(&frame)
            .map_err(|error| format!("failed to write CDP websocket frame: {error}"))
    }

    fn read_text_frame(&mut self) -> Result<String, String> {
        loop {
            let mut header = [0_u8; 2];
            self.stream
                .read_exact(&mut header)
                .map_err(|error| format!("failed to read CDP websocket frame header: {error}"))?;
            let opcode = header[0] & 0x0f;
            let masked = header[1] & 0x80 != 0;
            let mut length = u64::from(header[1] & 0x7f);
            if length == 126 {
                let mut bytes = [0_u8; 2];
                self.stream.read_exact(&mut bytes).map_err(|error| {
                    format!("failed to read CDP websocket frame length: {error}")
                })?;
                length = u64::from(u16::from_be_bytes(bytes));
            } else if length == 127 {
                let mut bytes = [0_u8; 8];
                self.stream.read_exact(&mut bytes).map_err(|error| {
                    format!("failed to read CDP websocket frame length: {error}")
                })?;
                length = u64::from_be_bytes(bytes);
            }

            let mask = if masked {
                let mut mask = [0_u8; 4];
                self.stream
                    .read_exact(&mut mask)
                    .map_err(|error| format!("failed to read CDP websocket frame mask: {error}"))?;
                Some(mask)
            } else {
                None
            };

            let mut payload = vec![0_u8; length as usize];
            self.stream
                .read_exact(&mut payload)
                .map_err(|error| format!("failed to read CDP websocket frame payload: {error}"))?;
            if let Some(mask) = mask {
                for (index, byte) in payload.iter_mut().enumerate() {
                    *byte ^= mask[index % mask.len()];
                }
            }

            match opcode {
                0x1 => {
                    return String::from_utf8(payload)
                        .map_err(|error| format!("CDP websocket returned non-UTF8 text: {error}"));
                }
                0x8 => return Err("CDP websocket closed".to_string()),
                0x9 => self.write_control_frame(0xA, &payload)?,
                0xA => {}
                other => return Err(format!("unsupported CDP websocket opcode: {other}")),
            }
        }
    }

    fn write_control_frame(&mut self, opcode: u8, payload: &[u8]) -> Result<(), String> {
        if payload.len() > 125 {
            return Err("websocket control frame payload too large".to_string());
        }
        let mut frame = vec![0x80 | opcode, 0x80 | payload.len() as u8];
        let mask = [0x55, 0xAA, 0x11, 0xEE];
        frame.extend_from_slice(&mask);
        for (index, byte) in payload.iter().enumerate() {
            frame.push(byte ^ mask[index % mask.len()]);
        }
        self.stream
            .write_all(&frame)
            .map_err(|error| format!("failed to write websocket control frame: {error}"))
    }
}

fn parse_ws_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = url
        .strip_prefix("ws://")
        .ok_or_else(|| format!("unsupported websocket URL `{url}`"))?;
    let (host_port, path) = rest
        .split_once('/')
        .ok_or_else(|| format!("websocket URL missing path: {url}"))?;
    let (host, port) = host_port
        .rsplit_once(':')
        .ok_or_else(|| format!("websocket URL missing port: {url}"))?;
    let port = port
        .parse::<u16>()
        .map_err(|error| format!("websocket URL has invalid port `{port}`: {error}"))?;
    Ok((host.to_string(), port, format!("/{path}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_path_maps_root_to_index() {
        assert_eq!(
            request_path_to_file("/?smoke=1").as_deref(),
            Some(Path::new("index.html"))
        );
    }

    #[test]
    fn request_path_rejects_traversal() {
        assert_eq!(request_path_to_file("/../Cargo.toml"), None);
        assert_eq!(request_path_to_file("/assets/../../Cargo.toml"), None);
        assert_eq!(request_path_to_file("/assets\\main.js"), None);
    }

    #[test]
    fn preflight_path_is_not_static_file_mapped() {
        assert!(is_webgpu_preflight_request(WEBGPU_PREFLIGHT_PATH));
        assert!(is_webgpu_preflight_request(&format!(
            "{WEBGPU_PREFLIGHT_PATH}?cache-bust=1"
        )));
        assert!(!is_webgpu_preflight_request("/"));
    }

    #[test]
    fn websocket_url_parser_accepts_cdp_urls() {
        assert_eq!(
            parse_ws_url("ws://127.0.0.1:9222/devtools/page/ABC").unwrap(),
            (
                "127.0.0.1".to_string(),
                9222,
                "/devtools/page/ABC".to_string()
            )
        );
    }

    #[test]
    fn webgpu_preflight_result_accepts_available_adapter() {
        assert_eq!(
            webgpu_preflight_result(&json!({ "status": "available", "featureCount": 0 })).unwrap(),
            WebGpuPreflight::Available
        );
    }

    #[test]
    fn webgpu_preflight_result_reports_unavailable_reason() {
        assert_eq!(
            webgpu_preflight_result(&json!({
                "status": "unavailable",
                "reason": "fallback WebGPU adapter is unavailable"
            }))
            .unwrap(),
            WebGpuPreflight::Unavailable("fallback WebGPU adapter is unavailable".to_string())
        );
    }

    #[test]
    fn webgpu_preflight_result_keeps_loading_state_pending() {
        assert_eq!(
            webgpu_preflight_result(&json!({
                "status": "pending",
                "path": WEBGPU_PREFLIGHT_PATH,
                "readyState": "loading"
            }))
            .unwrap(),
            WebGpuPreflight::Pending
        );
    }

    #[test]
    fn webgpu_preflight_result_rejects_malformed_state() {
        assert!(webgpu_preflight_result(&json!({ "available": false })).is_err());
    }

    #[test]
    fn webgpu_preflight_policy_runs_when_available() {
        assert_eq!(
            decide_webgpu_preflight(WebGpuPreflight::Available, false),
            Ok(WebGpuPreflightDecision::Run)
        );
    }

    #[test]
    fn webgpu_preflight_policy_fails_closed_when_unavailable() {
        let error = decide_webgpu_preflight(
            WebGpuPreflight::Unavailable("adapter missing".to_string()),
            false,
        )
        .unwrap_err();
        assert!(error.contains("adapter missing"));
        assert!(error.contains("--allow-unavailable"));
    }

    #[test]
    fn webgpu_preflight_policy_requires_an_explicit_local_skip() {
        assert_eq!(
            decide_webgpu_preflight(
                WebGpuPreflight::Unavailable("adapter missing".to_string()),
                true,
            ),
            Ok(WebGpuPreflightDecision::SkipAllowed(
                "adapter missing".to_string()
            ))
        );
    }

    #[test]
    fn webgpu_preflight_policy_rejects_pending_state() {
        assert!(decide_webgpu_preflight(WebGpuPreflight::Pending, true).is_err());
    }
}
