use serde_json::{Value, json};

mod clipboard;
mod font_fallback;
mod platform_info;
mod text_input;
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
const WEB_PREFLIGHT_PATH: &str = "/__open-gpui-web-smoke-preflight";
const WEB_PREFLIGHT_HTML: &[u8] = br#"<!doctype html>
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
const PIXEL_RGB_TOLERANCE: u8 = 3;
const RENDERER_SAMPLES: [(&str, [u32; 2], [u8; 4]); 22] = [
    ("grid-first", [17, 17], [230, 57, 70, 255]),
    ("grid-next", [19, 17], [46, 196, 182, 255]),
    ("grid-row", [17, 19], [46, 196, 182, 255]),
    ("grid-middle", [145, 145], [230, 57, 70, 255]),
    ("grid-last", [271, 271], [230, 57, 70, 255]),
    ("outer-rounded-cut", [306, 18], [16, 19, 24, 255]),
    ("outer-only", [310, 80], [38, 53, 74, 255]),
    ("inner-fill", [340, 70], [244, 185, 66, 255]),
    ("inner-rounded-cut", [440, 28], [38, 53, 74, 255]),
    ("transform-outside", [310, 180], [16, 19, 24, 255]),
    ("transform-background", [320, 180], [143, 91, 215, 255]),
    ("transform-child", [350, 176], [74, 222, 128, 255]),
    ("alpha-over", [512, 40], [128, 0, 127, 255]),
    ("image-red", [496, 104], [255, 0, 0, 255]),
    ("image-green", [528, 104], [0, 255, 0, 255]),
    ("image-blue", [496, 136], [0, 0, 255, 255]),
    ("image-white", [528, 136], [255, 255, 255, 255]),
    ("path-fill", [512, 200], [244, 114, 182, 255]),
    ("path-outside", [482, 228], [16, 19, 24, 255]),
    ("white-marker", [300, 284], [255, 255, 255, 255]),
    ("green-marker", [332, 284], [74, 222, 128, 255]),
    ("pink-marker", [364, 284], [244, 114, 182, 255]),
];

const WEB_PREFLIGHT_EXPRESSION: &str = r#"
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

    let webgl;
    const canvas = document.createElement("canvas");
    try {
        const context = canvas.getContext("webgl2", { antialias: false });
        webgl = context
            ? { status: "available" }
            : { status: "unavailable", reason: "WebGL2 context creation returned null" };
        context?.getExtension("WEBGL_lose_context")?.loseContext();
    } catch (error) {
        webgl = { status: "unavailable", reason: String(error) };
    } finally {
        canvas.remove();
    }

    let webgpu;
    const gpu = navigator.gpu ?? null;
    if (!globalThis.isSecureContext || !gpu) {
        webgpu = {
            status: "unavailable",
            reason: !globalThis.isSecureContext
                ? "WebGPU preflight page is not a secure context"
                : "navigator.gpu is unavailable",
        };
    } else {
        try {
            const adapter = await gpu.requestAdapter({ forceFallbackAdapter: true });
            webgpu = adapter
                ? { status: "available", featureCount: Array.from(adapter.features).length }
                : { status: "unavailable", reason: "fallback WebGPU adapter is unavailable" };
        } catch (error) {
            webgpu = {
                status: "unavailable",
                reason: String(error && (error.stack || error.message || error)),
            };
        }
    }
    return { status: "complete", webgpu, webgl, path, readyState };
})()
"#;

const SMOKE_STATE_EXPRESSION: &str = r#"
(() => {
    const canvas = document.querySelector("canvas");
    const input = document.querySelector("textarea");
    const rect = canvas ? canvas.getBoundingClientRect() : null;
    const probe = globalThis.__OPEN_GPUI_WEB_SMOKE__ ?? null;
    const diagnostics = globalThis.__OPEN_GPUI_WEB_SMOKE_DIAGNOSTICS__ ?? null;
    return {
        readyState: document.readyState,
        url: location.href,
        bodyReady: document.body?.dataset?.openGpuiWebSmokeReady === "true",
        viewport: {
            width: innerWidth,
            height: innerHeight,
            devicePixelRatio,
            scale: visualViewport?.scale ?? 1,
            scrollX,
            scrollY,
        },
        webgpuUnavailableControl:
            globalThis.__OPEN_GPUI_WEBGPU_UNAVAILABLE_CONTROL__ === true,
        navigatorGpuAvailable: navigator.gpu !== undefined,
        canvas: canvas ? {
            count: document.querySelectorAll("canvas").length,
            id: canvas.getAttribute("data-smoke-canvas-id"),
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
            count: document.querySelectorAll("textarea").length,
            focused: document.activeElement === input,
            mode: input.getAttribute("inputmode"),
        } : null,
        coarsePointer: matchMedia("(pointer: coarse)").matches,
        touchCore: globalThis.__OPEN_GPUI_WEB_TOUCH_CORE__ ?? null,
        touchDom: globalThis.__OPEN_GPUI_WEB_TOUCH_DOM__ ?? null,
        probe,
        canvases: globalThis.__OPEN_GPUI_WEB_SMOKE_CANVASES__ ?? null,
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
    let preflight_url = format!("http://{}{}", server.addr(), WEB_PREFLIGHT_PATH);

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

    if let Err(error) = qualify_browser(&mut cdp, &url, allow_unavailable) {
        eprintln!("{error}\n{}", browser.output_summary());
        return Err(());
    }
    Ok(())
}

fn qualify_browser(cdp: &mut CdpClient, url: &str, allow_unavailable: bool) -> Result<(), String> {
    enable_browser_domains(cdp)?;
    set_viewport(cdp, 800, 600)?;
    let preflight = wait_for_web_preflight(cdp)?;
    println!("web smoke backend preflight: {preflight:?}");
    let (webgpu, webgl) = match decide_web_preflight(preflight, allow_unavailable)? {
        WebPreflightDecision::Run { webgpu, webgl } => (webgpu, webgl),
        WebPreflightDecision::SkipAllowed(reason) => {
            println!("web smoke explicitly allowed no-supported-backend skip: {reason}");
            return Ok(());
        }
    };
    let mut cases = Vec::new();
    if let Ok(case) = env::var("OPEN_GPUI_WEB_SMOKE_CASE") {
        let case_url = format!(
            "{url}&backend={}&software={}",
            if webgl { "webgl" } else { "webgpu" },
            !webgl
        );
        return match case.as_str() {
            "text-input" => text_input::qualify(cdp, &case_url),
            "font-fallback" => font_fallback::qualify(cdp, &case_url),
            "clipboard" => clipboard::qualify(cdp, &case_url),
            "platform-info" => platform_info::qualify(cdp, &case_url),
            _ => Err(format!(
                "OPEN_GPUI_WEB_SMOKE_CASE must be text-input, font-fallback, clipboard, or platform-info; got `{case}`"
            )),
        };
    }
    if let Ok(requested) = env::var("OPEN_GPUI_WEB_SMOKE_TOUCH_BACKEND") {
        let actual = match requested.as_str() {
            "auto" => {
                if webgpu {
                    "WebGPU"
                } else {
                    "Gl"
                }
            }
            "webgl" if webgl => "Gl",
            "webgpu" if webgpu => "WebGPU",
            "webgl" | "webgpu" => {
                return Err(format!(
                    "requested touch smoke backend `{requested}` is unavailable"
                ));
            }
            _ => {
                return Err(format!(
                    "OPEN_GPUI_WEB_SMOKE_TOUCH_BACKEND must be auto, webgl, or webgpu; got `{requested}`"
                ));
            }
        };
        return run_touch_smoke_case(cdp, url, &requested, actual);
    }
    if webgl {
        cases.push(("webgl", "Gl"));
    }
    cases.push(("auto", if webgpu { "WebGPU" } else { "Gl" }));
    if webgpu {
        cases.push(("webgpu", "WebGPU"));
    }
    for (requested, actual) in cases {
        set_viewport(cdp, 800, 600)?;
        let software = actual != "Gl";
        let case_url = format!("{url}&backend={requested}&software={software}");
        navigate_case(cdp, &case_url)?;
        run_browser_smoke(cdp)?;
        check_backend_state(
            &cdp.evaluate(SMOKE_STATE_EXPRESSION)?,
            requested,
            actual,
            false,
        )?;
        for (mode, accepts_activation, focus_on_click) in [
            ("programmatic-only", true, false),
            ("click-only", false, true),
        ] {
            navigate_case(cdp, &format!("{case_url}&activation={mode}"))?;
            run_activation_policy_smoke(cdp, mode, accepts_activation, focus_on_click)?;
            check_backend_state(
                &cdp.evaluate(SMOKE_STATE_EXPRESSION)?,
                requested,
                actual,
                false,
            )?;
        }
        navigate_case(cdp, &format!("{case_url}&renderer=1"))?;
        run_renderer_smoke(cdp, requested, actual, false)?;
        println!(
            "web smoke backend={requested} actual={actual}: complete input/capture/docking/activation and renderer/resize passed"
        );
    }
    platform_info::qualify(
        cdp,
        &format!(
            "{url}&backend={}&software={}",
            if webgl { "webgl" } else { "webgpu" },
            !webgl
        ),
    )?;
    clipboard::qualify(
        cdp,
        &format!(
            "{url}&backend={}&software={}",
            if webgl { "webgl" } else { "webgpu" },
            !webgl
        ),
    )?;
    text_input::qualify(
        cdp,
        &format!(
            "{url}&backend={}&software={}",
            if webgl { "webgl" } else { "webgpu" },
            !webgl
        ),
    )?;
    font_fallback::qualify(
        cdp,
        &format!(
            "{url}&backend={}&software={}",
            if webgl { "webgl" } else { "webgpu" },
            !webgl
        ),
    )?;
    run_touch_smoke_case(
        cdp,
        url,
        if webgl { "webgl" } else { "webgpu" },
        if webgl { "Gl" } else { "WebGPU" },
    )?;
    if webgl {
        expect_graphics_refusal(
            cdp,
            &format!("{url}&backend=webgl&software=true"),
            "webgl",
            "Only WebGl was tried",
            true,
            false,
        )?;
        println!(
            "web smoke preserved explicit WebGL software-adapter refusal; GL success cases request software=false"
        );
        let registration = cdp.call(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": r#"
                Object.defineProperty(navigator, "gpu", {
                    configurable: true,
                    get: () => undefined,
                });
                globalThis.__OPEN_GPUI_WEBGPU_UNAVAILABLE_CONTROL__ = true;
            "# }),
        )?;
        let identifier = registration
            .get("identifier")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "WebGPU unavailability control returned no script identifier: {registration}"
                )
            })?;
        let result = (|| {
            set_viewport(cdp, 800, 600)?;
            expect_graphics_refusal(
                cdp,
                &format!("{url}&backend=webgpu&software=false&case=webgpu-unavailable-control"),
                "webgpu",
                "Only WebGpu was tried",
                false,
                true,
            )?;
            navigate_case(
                cdp,
                &format!(
                    "{url}&backend=auto&software=false&renderer=1&case=webgpu-unavailable-control"
                ),
            )?;
            run_browser_smoke(cdp)?;
            run_renderer_smoke(cdp, "auto", "Gl", true)
        })();
        let removal = cdp.call(
            "Page.removeScriptToEvaluateOnNewDocument",
            json!({ "identifier": identifier }),
        );
        if let Err(error) = result {
            return Err(format!(
                "WebGPU-unavailable test control failed: {error}; init-script removal: {removal:?}"
            ));
        }
        removal?;
        println!(
            "web smoke Auto fallback passed with navigator.gpu unavailable by explicit test control; this is not a hardware/device-failure qualification"
        );
    }
    println!(
        "web smoke passed: available browser backends, ordinary public consumers, actual PNG pixels and resize at DPR 1; application counters are not submission receipts"
    );
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum BackendPreflight {
    Available,
    Unavailable(String),
}

#[derive(Debug, PartialEq, Eq)]
enum WebPreflight {
    Pending,
    Complete {
        webgpu: BackendPreflight,
        webgl: BackendPreflight,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum WebPreflightDecision {
    Run { webgpu: bool, webgl: bool },
    SkipAllowed(String),
}

fn decide_web_preflight(
    preflight: WebPreflight,
    allow_unavailable: bool,
) -> Result<WebPreflightDecision, String> {
    match preflight {
        WebPreflight::Complete { webgpu, webgl } => {
            if webgpu == BackendPreflight::Available || webgl == BackendPreflight::Available {
                return Ok(WebPreflightDecision::Run {
                    webgpu: webgpu == BackendPreflight::Available,
                    webgl: webgl == BackendPreflight::Available,
                });
            }
            let reason = format!("WebGPU: {webgpu:?}; WebGL2: {webgl:?}");
            if allow_unavailable {
                Ok(WebPreflightDecision::SkipAllowed(reason))
            } else {
                Err(format!(
                    "No supported browser graphics backend is available: {reason}. Pass `--allow-unavailable` only for an explicit local diagnostic skip."
                ))
            }
        }
        WebPreflight::Pending => {
            Err("browser graphics preflight remained pending after wait".to_string())
        }
    }
}

fn enable_browser_domains(cdp: &mut CdpClient) -> Result<(), String> {
    cdp.call("Page.enable", json!({}))?;
    cdp.call("Runtime.enable", json!({}))?;
    Ok(())
}

fn wait_for_web_preflight(cdp: &mut CdpClient) -> Result<WebPreflight, String> {
    let started = Instant::now();
    let mut last_state = Value::Null;
    while started.elapsed() < BROWSER_TIMEOUT {
        match cdp.evaluate(WEB_PREFLIGHT_EXPRESSION) {
            Ok(state) => match web_preflight_result(&state)? {
                WebPreflight::Pending => {
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
        "timed out waiting for browser graphics preflight page; last state: {last_state}"
    ))
}

fn web_preflight_result(state: &Value) -> Result<WebPreflight, String> {
    match state.pointer("/status").and_then(Value::as_str) {
        Some("pending") => Ok(WebPreflight::Pending),
        Some("complete") => {
            let backend = |name: &str| -> Result<BackendPreflight, String> {
                let value = &state[name];
                match value["status"].as_str() {
                    Some("available") => Ok(BackendPreflight::Available),
                    Some("unavailable") => value["reason"]
                        .as_str()
                        .filter(|reason| !reason.is_empty())
                        .map(|reason| BackendPreflight::Unavailable(reason.to_owned()))
                        .ok_or_else(|| {
                            format!(
                                "browser {name} preflight omitted its unavailable reason: {state}"
                            )
                        }),
                    _ => Err(format!(
                        "browser {name} preflight returned malformed state: {state}"
                    )),
                }
            };
            Ok(WebPreflight::Complete {
                webgpu: backend("webgpu")?,
                webgl: backend("webgl")?,
            })
        }
        _ => Err(format!(
            "browser graphics preflight returned malformed state: {state}"
        )),
    }
}

fn set_viewport(cdp: &mut CdpClient, width: u32, height: u32) -> Result<(), String> {
    cdp.call(
        "Emulation.setDeviceMetricsOverride",
        json!({
            "width": width,
            "height": height,
            "deviceScaleFactor": 1,
            "mobile": false,
            "screenWidth": width,
            "screenHeight": height,
        }),
    )?;
    Ok(())
}

fn navigate_case(cdp: &mut CdpClient, url: &str) -> Result<(), String> {
    let navigation = cdp.call("Page.navigate", json!({ "url": url }))?;
    if let Some(error) = navigation.get("errorText").and_then(Value::as_str) {
        return Err(format!("web smoke navigation to {url} failed: {error}"));
    }
    wait_for_state(cdp, &format!("fresh fixture at {url}"), |state| {
        state.get("url").and_then(Value::as_str) == Some(url)
            && canvas_ready(state)
            && state.pointer("/probe/ready").and_then(Value::as_bool) == Some(true)
    })?;
    Ok(())
}

fn expect_graphics_refusal(
    cdp: &mut CdpClient,
    url: &str,
    requested: &str,
    reason: &str,
    force_fallback_adapter: bool,
    unavailable_control: bool,
) -> Result<(), String> {
    let navigation = cdp.call("Page.navigate", json!({ "url": url }))?;
    if let Some(error) = navigation.get("errorText").and_then(Value::as_str) {
        return Err(format!(
            "graphics refusal navigation to {url} failed: {error}"
        ));
    }
    let state = wait_for_state(cdp, "explicit backend initialization refusal", |state| {
        state.get("url").and_then(Value::as_str) == Some(url)
            && state
                .get("diagnostics")
                .and_then(Value::as_array)
                .is_some_and(|entries| {
                    entries.iter().any(|entry| {
                        entry
                            .get("message")
                            .and_then(Value::as_str)
                            .is_some_and(|message| {
                                message.contains("Failed to initialize browser graphics")
                                    && message.contains(reason)
                            })
                    })
                })
    })?;
    let candidates = state
        .pointer("/canvases/canvases")
        .and_then(Value::as_array);
    if state
        .pointer("/probe/requestedBackend")
        .and_then(Value::as_str)
        != Some(requested)
        || state.pointer("/probe/ready").and_then(Value::as_bool) != Some(false)
        || state.pointer("/probe/stage").and_then(Value::as_str) != Some("graphics-initializing")
        || state
            .pointer("/probe/forceFallbackAdapter")
            .and_then(Value::as_bool)
            != Some(force_fallback_adapter)
        || state
            .get("webgpuUnavailableControl")
            .and_then(Value::as_bool)
            != Some(unavailable_control)
        || (unavailable_control
            && state.get("navigatorGpuAvailable").and_then(Value::as_bool) != Some(false))
        || !state
            .pointer("/probe/graphicsBackend")
            .is_some_and(Value::is_null)
        || !state.get("canvas").is_some_and(Value::is_null)
        || !state.get("input").is_some_and(Value::is_null)
        || state
            .pointer("/canvases/uniqueAppDomCanvases")
            .and_then(Value::as_u64)
            != Some(0)
        || !candidates.is_some_and(|entries| {
            entries.len() == 1
                && entries.iter().all(|entry| {
                    entry.get("connected").and_then(Value::as_bool) == Some(false)
                        && entry.get("wasInDom").and_then(Value::as_bool) == Some(false)
                        && entry
                            .get("removeCalls")
                            .and_then(Value::as_u64)
                            .is_some_and(|calls| calls > 0)
                        && entry
                            .get("contexts")
                            .and_then(Value::as_array)
                            .is_some_and(|attempts| {
                                attempts.iter().all(|attempt| {
                                    matches!(
                                        (requested, attempt.get("kind").and_then(Value::as_str)),
                                        ("webgpu", Some("webgpu"))
                                            | ("webgl", Some("webgl" | "webgl2"))
                                    )
                                })
                            })
                })
        })
    {
        return Err(format!(
            "forced {requested} refusal launched an app or retained its canvas candidate: {state}"
        ));
    }
    Ok(())
}

fn check_backend_state(
    state: &Value,
    requested: &str,
    actual: &str,
    unavailable_control: bool,
) -> Result<(), String> {
    if state
        .pointer("/probe/requestedBackend")
        .and_then(Value::as_str)
        != Some(requested)
        || state
            .pointer("/probe/graphicsBackend")
            .and_then(Value::as_str)
            != Some(actual)
        || state
            .pointer("/probe/forceFallbackAdapter")
            .and_then(Value::as_bool)
            != Some(actual != "Gl")
        || state
            .pointer("/probe/appWindowCount")
            .and_then(Value::as_u64)
            != Some(1)
        || state
            .pointer("/probe/dockingSessionPhase")
            .and_then(Value::as_str)
            != Some("Active")
        || !state
            .pointer("/probe/dockingSessionGeneration")
            .and_then(Value::as_u64)
            .is_some_and(|generation| generation > 0)
        || !canvas_ready(state)
    {
        return Err(format!(
            "web smoke expected requested={requested}, actual={actual}, one active Dock primary and one canvas: {state}"
        ));
    }
    if state
        .get("webgpuUnavailableControl")
        .and_then(Value::as_bool)
        != Some(unavailable_control)
        || (unavailable_control
            && state.get("navigatorGpuAvailable").and_then(Value::as_bool) != Some(false))
    {
        return Err(format!(
            "web smoke WebGPU unavailability control did not match its case: {state}"
        ));
    }
    let instrumentation = state
        .get("canvases")
        .ok_or("web smoke canvas instrumentation is missing")?;
    let entries = instrumentation
        .get("canvases")
        .and_then(Value::as_array)
        .ok_or("web smoke canvas identity records are missing")?;
    let connected: Vec<_> = entries
        .iter()
        .filter(|entry| entry.get("connected").and_then(Value::as_bool) == Some(true))
        .collect();
    let current_id = state
        .pointer("/canvas/id")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or("web smoke DOM canvas has no instrumentation identity")?;
    if instrumentation
        .get("uniqueAppDomCanvases")
        .and_then(Value::as_u64)
        != Some(1)
        || connected.len() != 1
        || connected[0].get("id").and_then(Value::as_u64) != Some(current_id)
        || connected[0].get("wasInDom").and_then(Value::as_bool) != Some(true)
    {
        return Err(format!(
            "web smoke expected one unique connected app DOM canvas, including across DOM moves: {state}"
        ));
    }
    let context_kind = if actual == "WebGPU" {
        "webgpu"
    } else {
        "webgl2"
    };
    if !connected[0]
        .get("contexts")
        .and_then(Value::as_array)
        .is_some_and(|attempts| {
            attempts.iter().any(|attempt| {
                attempt.get("kind").and_then(Value::as_str) == Some(context_kind)
                    && attempt.get("returnedContext").and_then(Value::as_bool) == Some(true)
            })
        })
    {
        return Err(format!(
            "web smoke selected {actual} without a matching successful {context_kind} context on its connected canvas: {state}"
        ));
    }
    if requested == "auto"
        && actual == "Gl"
        && !entries.iter().any(|entry| {
            entry
                .get("id")
                .and_then(Value::as_u64)
                .is_some_and(|identity| identity < current_id)
                && entry
                    .get("removeCalls")
                    .and_then(Value::as_u64)
                    .is_some_and(|calls| calls > 0)
                && entry.get("connected").and_then(Value::as_bool) == Some(false)
                && entry.get("wasInDom").and_then(Value::as_bool) == Some(false)
        })
    {
        return Err(format!(
            "Auto fallback did not record removal of its detached earlier canvas candidate: {state}"
        ));
    }
    Ok(())
}

fn run_renderer_smoke(
    cdp: &mut CdpClient,
    requested: &str,
    actual: &str,
    unavailable_control: bool,
) -> Result<(), String> {
    for (width, height) in [(800, 600), (960, 720)] {
        set_viewport(cdp, width, height)?;
        let state = wait_for_state(
            cdp,
            "renderer fixture and resized canvas geometry",
            |state| {
                canvas_ready(state)
                    && state
                        .pointer("/probe/rendererMode")
                        .and_then(Value::as_bool)
                        == Some(true)
                    && state
                        .pointer("/probe/paintCalls")
                        .and_then(Value::as_u64)
                        .is_some_and(|calls| calls > 0)
                    && state
                        .pointer("/probe/denseQuadRequests")
                        .and_then(Value::as_u64)
                        .is_some_and(|calls| calls >= 16384)
                    && state.pointer("/viewport/width").and_then(Value::as_u64)
                        == Some(u64::from(width))
                    && state.pointer("/viewport/height").and_then(Value::as_u64)
                        == Some(u64::from(height))
                    && state
                        .pointer("/probe/viewport/width")
                        .and_then(Value::as_f64)
                        == Some(f64::from(width))
                    && state
                        .pointer("/probe/viewport/height")
                        .and_then(Value::as_f64)
                        == Some(f64::from(height))
                    && state.pointer("/canvas/width").and_then(Value::as_u64)
                        == Some(u64::from(width))
                    && state.pointer("/canvas/height").and_then(Value::as_u64)
                        == Some(u64::from(height))
            },
        )?;
        check_backend_state(&state, requested, actual, unavailable_control)?;
        let pixels = wait_for_renderer_pixels(cdp, requested, actual, unavailable_control)?;
        println!(
            "web smoke PNG backend={actual} viewport={width}x{height} DPR=1: 22/22 fixed samples including high-index quad, text={}; RGB tolerance={PIXEL_RGB_TOLERANCE}/255 for HSL and 8-bit compositor rounding, exact alpha; app frameRevision={} paintCalls={} denseQuadRequests={}",
            pixels["text"],
            state["probe"]["frameRevision"],
            state["probe"]["paintCalls"],
            state["probe"]["denseQuadRequests"],
        );
    }
    Ok(())
}

fn wait_for_renderer_pixels(
    cdp: &mut CdpClient,
    requested: &str,
    actual: &str,
    unavailable_control: bool,
) -> Result<Value, String> {
    let started = Instant::now();
    let mut last_error = String::new();
    while started.elapsed() < BROWSER_TIMEOUT {
        let state = cdp.evaluate(SMOKE_STATE_EXPRESSION)?;
        check_backend_state(&state, requested, actual, unavailable_control)?;
        match capture_renderer_pixels(cdp, &state).and_then(|pixels| {
            check_renderer_pixels(&pixels)
                .map_err(|error| format!("{error}; samples: {pixels}"))?;
            Ok(pixels)
        }) {
            Ok(pixels) => return Ok(pixels),
            Err(error) => {
                last_error = format!(
                    "{error}; probe: {}; diagnostics: {}",
                    state["probe"], state["diagnostics"],
                );
            }
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(format!(
        "timed out waiting for actual renderer PNG pixels: {last_error}"
    ))
}

fn renderer_css_point(state: &Value, local: [f64; 2]) -> Result<[f64; 2], String> {
    let canvas = &state["canvas"]["rect"];
    let viewport = &state["probe"]["viewport"];
    let panel = &viewport["paintBounds"];
    let logical_width = number_field(viewport, "width")?;
    let logical_height = number_field(viewport, "height")?;
    if logical_width <= 0.0 || logical_height <= 0.0 {
        return Err("renderer logical viewport must have positive dimensions".to_string());
    }
    Ok([
        number_field(canvas, "left")?
            + (number_field(panel, "left")? + local[0]) * number_field(canvas, "width")?
                / logical_width,
        number_field(canvas, "top")?
            + (number_field(panel, "top")? + local[1]) * number_field(canvas, "height")?
                / logical_height,
    ])
}

fn capture_renderer_pixels(cdp: &mut CdpClient, state: &Value) -> Result<Value, String> {
    if state
        .pointer("/viewport/devicePixelRatio")
        .and_then(Value::as_f64)
        != Some(1.0)
        || state.pointer("/viewport/scale").and_then(Value::as_f64) != Some(1.0)
        || state.pointer("/viewport/scrollX").and_then(Value::as_f64) != Some(0.0)
        || state.pointer("/viewport/scrollY").and_then(Value::as_f64) != Some(0.0)
        || state
            .pointer("/probe/viewport/scaleFactor")
            .and_then(Value::as_f64)
            != Some(1.0)
    {
        return Err(format!(
            "renderer PNG qualification requires its declared DPR 1, unzoomed, unscrolled scope: {state}"
        ));
    }
    let mut samples = Vec::with_capacity(RENDERER_SAMPLES.len());
    for (name, at, _) in RENDERER_SAMPLES {
        samples.push(json!({
            "name": name,
            "css": renderer_css_point(state, [f64::from(at[0]), f64::from(at[1])])?,
        }));
    }
    let text_start = renderer_css_point(state, [304.0, 240.0])?;
    let text_end = renderer_css_point(state, [560.0, 268.0])?;
    cdp.evaluate(
        r#"(async () => {
            await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
            return true;
        })()"#,
    )?;
    let screenshot = cdp.call(
        "Page.captureScreenshot",
        json!({
            "format": "png",
            "fromSurface": true,
            "captureBeyondViewport": false,
        }),
    )?;
    let png = screenshot
        .get("data")
        .and_then(Value::as_str)
        .filter(|data| !data.is_empty())
        .ok_or("Page.captureScreenshot returned no PNG data")?;
    let input = json!({
        "png": png,
        "url": state["url"],
        "width": state["viewport"]["width"],
        "height": state["viewport"]["height"],
        "samples": samples,
        "textStart": text_start,
        "textEnd": text_end,
    });
    cdp.evaluate(&format!(
        r#"(async () => {{
            const input = {input};
            if (location.href !== input.url || innerWidth !== input.width || innerHeight !== input.height) {{
                throw new Error("fixture navigated or resized while its screenshot was captured");
            }}
            const response = await fetch("data:image/png;base64," + input.png);
            const bitmap = await createImageBitmap(await response.blob());
            try {{
                if (bitmap.width !== input.width || bitmap.height !== input.height) {{
                    throw new Error("captured PNG dimensions differ from the declared DPR 1 viewport");
                }}
                const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
                const context = canvas.getContext("2d", {{ willReadFrequently: true }});
                if (!context) throw new Error("screenshot decoding needs OffscreenCanvas2D");
                context.drawImage(bitmap, 0, 0);
                const pixels = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
                const toPixel = css => {{
                    const point = [
                        Math.floor(css[0] * bitmap.width / input.width),
                        Math.floor(css[1] * bitmap.height / input.height),
                    ];
                    if (point[0] < 0 || point[1] < 0 || point[0] >= bitmap.width || point[1] >= bitmap.height) {{
                        throw new Error("source sample is outside screenshot: " + JSON.stringify({{ css, point }}));
                    }}
                    return point;
                }};
                const samples = input.samples.map(sample => {{
                    const point = toPixel(sample.css);
                    const offset = (point[1] * bitmap.width + point[0]) * 4;
                    return {{ name: sample.name, point, rgba: Array.from(pixels.slice(offset, offset + 4)) }};
                }});
                const textStart = toPixel(input.textStart);
                const textEnd = toPixel(input.textEnd);
                let brightPixels = 0;
                let minimumColumn = textEnd[0], maximumColumn = textStart[0] - 1;
                let minimumRow = textEnd[1], maximumRow = textStart[1] - 1;
                for (let row = textStart[1]; row < textEnd[1]; row++) {{
                    for (let column = textStart[0]; column < textEnd[0]; column++) {{
                        const offset = (row * bitmap.width + column) * 4;
                        if (pixels[offset] >= 200 && pixels[offset + 1] >= 200
                            && pixels[offset + 2] >= 200 && pixels[offset + 3] === 255) {{
                            brightPixels++;
                            minimumColumn = Math.min(minimumColumn, column);
                            maximumColumn = Math.max(maximumColumn, column);
                            minimumRow = Math.min(minimumRow, row);
                            maximumRow = Math.max(maximumRow, row);
                        }}
                    }}
                }}
                return {{
                    width: bitmap.width,
                    height: bitmap.height,
                    samples,
                    text: {{
                        brightPixels,
                        regionPixels: (textEnd[0] - textStart[0]) * (textEnd[1] - textStart[1]),
                        inkWidth: Math.max(0, maximumColumn - minimumColumn + 1),
                        inkHeight: Math.max(0, maximumRow - minimumRow + 1),
                    }},
                }};
            }} finally {{
                bitmap.close();
            }}
        }})()"#
    ))
}

fn pixel_matches(actual: [u8; 4], expected: [u8; 4]) -> bool {
    actual[3] == expected[3]
        && actual[..3]
            .iter()
            .zip(&expected[..3])
            .all(|(actual, expected)| actual.abs_diff(*expected) <= PIXEL_RGB_TOLERANCE)
}

fn check_renderer_pixels(pixels: &Value) -> Result<(), String> {
    let samples = pixels
        .get("samples")
        .and_then(Value::as_array)
        .filter(|samples| samples.len() == RENDERER_SAMPLES.len())
        .ok_or_else(|| {
            format!("screenshot did not return all 22 fixed reference samples: {pixels}")
        })?;
    for (sample, (name, _, expected)) in samples.iter().zip(RENDERER_SAMPLES) {
        if sample.get("name").and_then(Value::as_str) != Some(name) {
            return Err(format!("screenshot sample order/name changed: {sample}"));
        }
        let mut actual = [0_u8; 4];
        for (channel, value) in actual.iter_mut().enumerate() {
            *value = sample["rgba"][channel]
                .as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| format!("screenshot sample has invalid RGBA bytes: {sample}"))?;
        }
        if !pixel_matches(actual, expected) {
            return Err(format!(
                "screenshot `{name}` at {} is {actual:?}, expected {expected:?} (RGB tolerance {PIXEL_RGB_TOLERANCE}, exact alpha)",
                sample["point"]
            ));
        }
    }
    let text = &pixels["text"];
    let bright = text["brightPixels"].as_u64().unwrap_or(0);
    let area = text["regionPixels"].as_u64().unwrap_or(0);
    if bright < 64
        || bright.saturating_mul(2) >= area
        || text["inkWidth"].as_u64().unwrap_or(0) < 80
        || text["inkHeight"].as_u64().unwrap_or(0) < 9
    {
        return Err(format!(
            "screenshot lacks visible multi-glyph text coverage in its fixed source region: {text}"
        ));
    }
    Ok(())
}

fn run_touch_smoke_case(
    cdp: &mut CdpClient,
    url: &str,
    requested: &str,
    actual: &str,
) -> Result<(), String> {
    cdp.call(
        "Emulation.setTouchEmulationEnabled",
        json!({
            "enabled": true, "maxTouchPoints": 2,
        }),
    )?;
    let result = (|| {
        cdp.call(
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": 800, "height": 600, "deviceScaleFactor": 1,
                "mobile": true, "screenWidth": 800, "screenHeight": 600,
            }),
        )?;
        let case_url = format!(
            "{url}&backend={requested}&software={}&touch=1",
            actual != "Gl"
        );
        navigate_case(cdp, &case_url)?;
        let ready = wait_for_state(
            cdp,
            "Core gesture consumer and editable scroll target",
            |state| {
                state.pointer("/touchCore/ready").and_then(Value::as_bool) == Some(true)
                    && state.get("coarsePointer").and_then(Value::as_bool) == Some(true)
                    && state.pointer("/input/mode").and_then(Value::as_str) == Some("none")
            },
        )?;
        if ready
            .pointer("/probe/graphicsBackend")
            .and_then(Value::as_str)
            != Some(actual)
        {
            return Err(format!("touch smoke selected unexpected renderer: {ready}"));
        }
        run_touch_smoke(cdp)?;
        println!(
            "web touch smoke backend={requested} actual={actual}: trusted DOM pointer phases/IDs, recognized Core outputs, pointer/touch scroll sources and unique scroll cancellation passed; Core TouchId identity is covered separately by portable/ingress tests"
        );
        Ok(())
    })();
    let reset_touch = cdp.call(
        "Emulation.setTouchEmulationEnabled",
        json!({ "enabled": false }),
    );
    let reset_viewport = set_viewport(cdp, 800, 600);
    if let Err(error) = result {
        return Err(format!(
            "{error}; touch reset: {reset_touch:?}; viewport reset: {reset_viewport:?}"
        ));
    }
    reset_touch?;
    reset_viewport
}

fn dom_touch_count(state: &Value, kind: &str) -> usize {
    state
        .pointer("/touchDom/events")
        .and_then(Value::as_array)
        .map_or(0, |events| {
            events
                .iter()
                .filter(|event| event["pointerType"] == "touch" && event["kind"] == kind)
                .count()
        })
}

fn recognized_touch_count(state: &Value, phase: &str) -> usize {
    state
        .pointer("/touchCore/scroll")
        .and_then(Value::as_array)
        .map_or(0, |events| {
            events
                .iter()
                .filter(|event| event["source"] == "touch" && event["phase"] == phase)
                .count()
        })
}

fn run_touch_smoke(cdp: &mut CdpClient) -> Result<(), String> {
    let ready = cdp.evaluate(SMOKE_STATE_EXPRESSION)?;
    let rect = ready
        .pointer("/canvas/rect")
        .ok_or("touch smoke canvas bounds are missing")?;
    let x = number_field(rect, "left")? + number_field(rect, "width")? * 0.5;
    let y = number_field(rect, "top")? + number_field(rect, "height")? * 0.5;
    cdp.touch("touchStart", &[(7, x, y)])?;
    cdp.touch("touchEnd", &[])?;
    wait_for_state(
        cdp,
        "trusted tap reaches Core and focuses the editable target",
        |state| {
            dom_touch_count(state, "pointerup") == 1
                && state.pointer("/touchCore/clicks").and_then(Value::as_u64) == Some(1)
                && state.pointer("/input/focused").and_then(Value::as_bool) == Some(true)
                && state.pointer("/input/mode").and_then(Value::as_str) == Some("text")
        },
    )?;
    cdp.evaluate(
        r#"(async () => {
        await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        const input = document.querySelector("textarea");
        input.blur();
        input.setAttribute("inputmode", "none");
        return true;
    })()"#,
    )?;
    let baseline = wait_for_state(cdp, "dismissed keyboard control before panning", |state| {
        state.pointer("/input/focused").and_then(Value::as_bool) == Some(false)
            && state.pointer("/input/mode").and_then(Value::as_str) == Some("none")
    })?;
    let focus_changes = baseline
        .pointer("/touchDom/focusChanges")
        .and_then(Value::as_u64)
        .ok_or("DOM focus trace is missing")?;
    let pan_dom_start = baseline
        .pointer("/touchDom/events")
        .and_then(Value::as_array)
        .ok_or("DOM touch trace is missing")?
        .len();

    cdp.touch("touchStart", &[(7, x, y)])?;
    cdp.touch("touchMove", &[(7, x, y - 40.)])?;
    cdp.touch("touchMove", &[(7, x, y - 80.)])?;
    wait_for_state(
        cdp,
        "trusted DOM touch pan scrolls the real Core consumer",
        |state| {
            recognized_touch_count(state, "started") == 1
                && state
                    .pointer("/touchCore/offsetY")
                    .and_then(Value::as_f64)
                    .is_some_and(|y| y < 0.)
        },
    )?;
    cdp.touch("touchCancel", &[])?;
    wait_for_state(
        cdp,
        "trusted DOM touchcancel terminates Core touch scrolling",
        |state| {
            dom_touch_count(state, "pointercancel") == 1
                && recognized_touch_count(state, "cancelled") == 1
        },
    )?;

    cdp.touch("touchStart", &[(7, x, y)])?;
    cdp.touch("touchMove", &[(7, x, y - 40.)])?;
    wait_for_state(cdp, "second touch pan before capture loss", |state| {
        recognized_touch_count(state, "started") == 2
    })?;
    let released_pointer_id = cdp
        .evaluate(
            r#"(() => {
        const downs = __OPEN_GPUI_WEB_TOUCH_DOM__.events.filter(
            event => event.kind === "pointerdown" && event.pointerType === "touch");
        const pointerId = downs.at(-1)?.pointerId;
        const canvas = document.querySelector("canvas");
        if (pointerId == null || !canvas.hasPointerCapture(pointerId)) {
            throw new Error("touch smoke canvas does not own the live touch pointer");
        }
        canvas.releasePointerCapture(pointerId);
        return pointerId;
    })()"#,
        )?
        .as_i64()
        .ok_or("capture-loss control did not return a real pointer ID")?;
    cdp.touch("touchMove", &[(7, x, y - 60.)])?;
    wait_for_state(
        cdp,
        "capture loss cancels Core touch scrolling exactly once",
        |state| recognized_touch_count(state, "cancelled") == 2,
    )?;
    cdp.touch("touchEnd", &[])?;

    cdp.touch("touchStart", &[(7, x - 20., y), (8, x + 20., y)])?;
    cdp.touch("touchCancel", &[])?;
    wait_for_state(
        cdp,
        "both simultaneous DOM touch pointers terminate",
        |state| {
            dom_touch_count(state, "pointerdown") == 5
                && dom_touch_count(state, "pointercancel") == 3
        },
    )?;
    cdp.mouse_move(x, y, 0)?;
    cdp.call(
        "Input.dispatchMouseEvent",
        json!({
            "type": "mouseWheel", "x": x, "y": y, "deltaX": 0, "deltaY": 40,
            "button": "none", "buttons": 0,
        }),
    )?;
    wait_for_state(
        cdp,
        "trusted wheel retains Pointer source through Core",
        |state| {
            state
                .pointer("/touchCore/scroll")
                .and_then(Value::as_array)
                .is_some_and(|events| events.iter().any(|event| event["source"] == "pointer"))
        },
    )?;
    cdp.evaluate(
        r#"(async () => {
        await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        return true;
    })()"#,
    )?;
    let state = cdp.evaluate(SMOKE_STATE_EXPRESSION)?;
    validate_touch_smoke(&state, focus_changes, pan_dom_start, released_pointer_id)?;
    println!(
        "web touch metadata trace: {}",
        json!({
            "core": state.get("touchCore"), "dom": state.get("touchDom"),
        })
    );
    Ok(())
}

fn validate_touch_smoke(
    state: &Value,
    focus_changes: u64,
    pan_dom_start: usize,
    released_pointer_id: i64,
) -> Result<(), String> {
    let require = |condition: bool, label: &str| {
        if condition {
            Ok(())
        } else {
            Err(format!("touch smoke {label}; state: {state}"))
        }
    };
    let scroll = state
        .pointer("/touchCore/scroll")
        .and_then(Value::as_array)
        .ok_or("actual Core scroll trace is missing")?;
    let dom = state
        .pointer("/touchDom/events")
        .and_then(Value::as_array)
        .ok_or("DOM touch observations are missing")?;
    require(
        state
            .pointer("/touchCore/clickObservation")
            .and_then(Value::as_str)
            == Some("core-element-on-click")
            && state
                .pointer("/touchCore/scrollObservation")
                .and_then(Value::as_str)
                == Some("core-window-interceptor"),
        "did not install supported Core consumer observations",
    )?;
    require(
        state.pointer("/touchCore/overflow") == Some(&json!(false))
            && state.pointer("/touchDom/overflow") == Some(&json!(false)),
        "trace overflowed",
    )?;
    require(
        recognized_touch_count(state, "started") == 2
            && recognized_touch_count(state, "cancelled") == 2
            && recognized_touch_count(state, "ended") == 0,
        "touch scroll source or terminal phases were lost",
    )?;
    let touch_phases: Vec<_> = scroll
        .iter()
        .filter(|event| event["source"] == "touch")
        .filter_map(|event| event["phase"].as_str())
        .collect();
    let sessions: Vec<_> = touch_phases
        .split_inclusive(|phase| *phase == "cancelled")
        .collect();
    require(
        sessions.len() == 2
            && sessions.iter().all(|phases| {
                phases.len() >= 2
                    && phases.first() == Some(&"started")
                    && phases.last() == Some(&"cancelled")
                    && phases[1..phases.len() - 1]
                        .iter()
                        .all(|phase| *phase == "moved")
            }),
        "recognized scroll continued or cancelled twice after a terminal phase",
    )?;
    require(
        scroll.iter().any(|event| event["source"] == "pointer"),
        "wheel did not preserve Pointer source in the actual Core consumer",
    )?;
    let touch_dom: Vec<_> = dom
        .iter()
        .filter(|event| {
            event["pointerType"] == "touch"
                || event["kind"] == "touchend"
                || event["kind"] == "wheel"
        })
        .collect();
    require(
        !touch_dom.is_empty() && touch_dom.iter().all(|event| event["trusted"] == true),
        "used synthetic DOM input instead of trusted browser input",
    )?;
    require(
        dom_touch_count(state, "pointerdown") == 5
            && dom_touch_count(state, "pointerup") == 2
            && dom_touch_count(state, "pointercancel") == 3,
        "DOM pointer phase trajectory differs from the browser-injected contacts",
    )?;
    require(
        touch_dom
            .iter()
            .filter(|event| event["pointerType"] == "touch")
            .all(|event| event["pointerId"].as_i64().is_some()),
        "DOM pointerId metadata is missing",
    )?;
    require(
        touch_dom
            .iter()
            .any(|event| event["kind"] == "pointerup" && event["userActivation"] == true),
        "DOM tap did not retain browser user activation",
    )?;
    require(
        touch_dom
            .iter()
            .filter(|event| {
                event["cancelable"] == true
                    && matches!(
                        event["kind"].as_str(),
                        Some("pointerdown" | "pointermove" | "pointerup" | "touchend" | "wheel")
                    )
            })
            .all(|event| event["defaultPrevented"] == true),
        "browser defaults were not suppressed",
    )?;
    require(
        touch_dom.iter().any(|event| {
            event["kind"] == "touchend"
                && event["cancelable"] == true
                && event["defaultPrevented"] == true
        }),
        "never exercised the non-passive touchend default suppression",
    )?;
    require(
        touch_dom
            .iter()
            .any(|event| event["kind"] == "pointercancel")
            && touch_dom.iter().any(|event| {
                event["kind"] == "lostpointercapture"
                    && event["pointerId"].as_i64() == Some(released_pointer_id)
            }),
        "did not exercise both OS cancellation and capture loss",
    )?;
    require(
        state.pointer("/input/focused") == Some(&json!(false))
            && state.pointer("/input/mode") == Some(&json!("none"))
            && state
                .pointer("/touchDom/focusChanges")
                .and_then(Value::as_u64)
                == Some(focus_changes)
            && dom.get(pan_dom_start..).is_some_and(|events| {
                events
                    .iter()
                    .all(|event| event["kind"] != "inputmode" || event["value"] != "text")
            }),
        "pan or cancellation changed focus or re-enabled the keyboard",
    )?;
    require(
        state.pointer("/touchCore/clicks").and_then(Value::as_u64) == Some(1),
        "pan or cancelled contact produced a phantom tap",
    )?;
    require(
        state.pointer("/viewport/scale").and_then(Value::as_f64) == Some(1.)
            && state.pointer("/viewport/scrollX").and_then(Value::as_f64) == Some(0.)
            && state.pointer("/viewport/scrollY").and_then(Value::as_f64) == Some(0.),
        "browser defaults scrolled or zoomed the document",
    )
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
        r#"(async () => {
            const input = document.querySelector("textarea");
            if (!input) {
                throw new Error("web smoke input is missing");
            }
            input.blur();
            await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
            return true;
        })()"#,
    )?;
    wait_for_state(
        cdp,
        "textarea blur preserves browser window activity",
        |state| {
            state.pointer("/input/focused").and_then(Value::as_bool) == Some(false)
                && state
                    .pointer("/canvas/mousePointerCaptured")
                    .and_then(Value::as_bool)
                    == Some(true)
                && state
                    .pointer("/probe/pointerCancelEvents")
                    .and_then(Value::as_u64)
                    == Some(1)
                && state
                    .pointer("/probe/windowDeactivatedEvents")
                    .and_then(Value::as_u64)
                    == Some(0)
        },
    )?;
    cdp.evaluate(
        r#"(() => {
            const button = document.createElement("button");
            button.id = "open-gpui-smoke-external-focus";
            button.textContent = "External focus control";
            button.style.position = "fixed";
            button.style.left = "-10000px";
            document.body.appendChild(button);
            button.focus();
            if (document.activeElement !== button) {
                throw new Error("external DOM focus control did not acquire focus");
            }
            return true;
        })()"#,
    )?;
    wait_for_state(cdp, "external DOM focus pointer cancellation", |state| {
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
            const input = document.querySelector("textarea");
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
            const input = document.querySelector("textarea");
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
                && document.activeElement === document.querySelector("textarea");
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
            const input = document.querySelector("textarea");
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
            const input = document.querySelector("textarea");
            if (!input) {
                throw new Error("activation probe input is missing");
            }
            const button = document.createElement("button");
            button.id = "open-gpui-activation-external-focus";
            button.textContent = "External activation control";
            button.style.position = "fixed";
            button.style.left = "-10000px";
            document.body.appendChild(button);
            button.focus();
            if (document.activeElement !== button) {
                throw new Error("activation probe external control did not acquire focus");
            }
            return true;
        })()"#,
    )?;
    wait_for_state(cdp, &format!("{mode} external DOM focus"), |state| {
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
    if is_web_preflight_request(parts[1]) {
        return write_response(
            &mut stream,
            200,
            "text/html; charset=utf-8",
            WEB_PREFLIGHT_HTML,
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

fn is_web_preflight_request(path: &str) -> bool {
    path.split_once('?').map_or(path, |(path, _)| path) == WEB_PREFLIGHT_PATH
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
            "--force-color-profile=srgb".to_string(),
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

    fn touch(&mut self, event_type: &str, points: &[(i32, f64, f64)]) -> Result<(), String> {
        let points: Vec<_> = points
            .iter()
            .map(|(id, x, y)| {
                json!({
                    "id": id, "x": x, "y": y, "radiusX": 1, "radiusY": 1, "force": 1,
                })
            })
            .collect();
        self.call(
            "Input.dispatchTouchEvent",
            json!({
                "type": event_type, "touchPoints": points,
            }),
        )?;
        Ok(())
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

    fn editing_key(
        &mut self,
        key: &str,
        code: &str,
        virtual_key: u32,
        modifiers: u8,
    ) -> Result<(), String> {
        // Chromium's macOS CDP path needs native editor commands. Its native
        // key codes differ from Windows virtual keys, so leave native codes unset.
        let command = if cfg!(target_os = "macos") {
            match (key, modifiers) {
                ("ArrowUp", 4) => Some("moveToBeginningOfDocument"),
                ("ArrowLeft", 0) => Some("moveLeft"),
                ("ArrowRight", 0) => Some("moveRight"),
                ("ArrowRight", 8) => Some("moveRightAndModifySelection"),
                ("Backspace", 0) => Some("deleteBackward"),
                ("Delete", 0) => Some("deleteForward"),
                ("a", 4) => Some("selectAll"),
                ("v", 4) => Some("paste"),
                _ => None,
            }
        } else {
            None
        };
        for kind in ["keyDown", "keyUp"] {
            let mut event = json!({
                "type": kind,
                "key": key,
                "code": code,
                "modifiers": modifiers,
                "windowsVirtualKeyCode": virtual_key,
            });
            if kind == "keyDown"
                && let Some(command) = command
            {
                event["commands"] = json!([command]);
            }
            self.call("Input.dispatchKeyEvent", event)?;
        }
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
        assert!(is_web_preflight_request(WEB_PREFLIGHT_PATH));
        assert!(is_web_preflight_request(&format!(
            "{WEB_PREFLIGHT_PATH}?cache-bust=1"
        )));
        assert!(!is_web_preflight_request("/"));
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
    fn web_preflight_result_accepts_both_available_backends() {
        assert_eq!(
            web_preflight_result(&json!({
                "status": "complete",
                "webgpu": { "status": "available", "featureCount": 0 },
                "webgl": { "status": "available" },
            }))
            .unwrap(),
            WebPreflight::Complete {
                webgpu: BackendPreflight::Available,
                webgl: BackendPreflight::Available,
            }
        );
    }

    #[test]
    fn web_preflight_result_preserves_unavailable_reason_and_working_webgl() {
        assert_eq!(
            web_preflight_result(&json!({
                "status": "complete",
                "webgpu": {
                    "status": "unavailable",
                    "reason": "fallback WebGPU adapter is unavailable"
                },
                "webgl": { "status": "available" },
            }))
            .unwrap(),
            WebPreflight::Complete {
                webgpu: BackendPreflight::Unavailable(
                    "fallback WebGPU adapter is unavailable".to_string()
                ),
                webgl: BackendPreflight::Available,
            }
        );
    }

    #[test]
    fn web_preflight_result_keeps_loading_state_pending() {
        assert_eq!(
            web_preflight_result(&json!({
                "status": "pending",
                "path": WEB_PREFLIGHT_PATH,
                "readyState": "loading"
            }))
            .unwrap(),
            WebPreflight::Pending
        );
    }

    #[test]
    fn web_preflight_result_rejects_malformed_or_incomplete_state() {
        for state in [
            json!({ "available": false }),
            json!({ "status": "complete", "webgpu": { "status": "available" } }),
            json!({
                "status": "complete",
                "webgpu": { "status": "unavailable" },
                "webgl": { "status": "available" },
            }),
            json!({
                "status": "complete",
                "webgpu": { "status": "available" },
                "webgl": { "status": "unavailable", "reason": "" },
            }),
        ] {
            assert!(web_preflight_result(&state).is_err(), "{state}");
        }
    }

    #[test]
    fn web_preflight_policy_runs_for_either_or_both_backends_even_when_skips_are_allowed() {
        for (webgpu, webgl) in [(true, true), (true, false), (false, true)] {
            for allow_unavailable in [false, true] {
                let availability = |available| {
                    if available {
                        BackendPreflight::Available
                    } else {
                        BackendPreflight::Unavailable("backend missing".to_string())
                    }
                };
                assert_eq!(
                    decide_web_preflight(
                        WebPreflight::Complete {
                            webgpu: availability(webgpu),
                            webgl: availability(webgl),
                        },
                        allow_unavailable
                    ),
                    Ok(WebPreflightDecision::Run { webgpu, webgl })
                );
            }
        }
    }

    #[test]
    fn web_preflight_policy_fails_closed_when_both_backends_are_unavailable() {
        let error = decide_web_preflight(
            WebPreflight::Complete {
                webgpu: BackendPreflight::Unavailable("adapter missing".to_string()),
                webgl: BackendPreflight::Unavailable("context missing".to_string()),
            },
            false,
        )
        .unwrap_err();
        assert!(error.contains("adapter missing"));
        assert!(error.contains("context missing"));
        assert!(error.contains("--allow-unavailable"));
    }

    #[test]
    fn web_preflight_policy_requires_an_explicit_local_skip_for_both_missing_backends() {
        let decision = decide_web_preflight(
            WebPreflight::Complete {
                webgpu: BackendPreflight::Unavailable("adapter missing".to_string()),
                webgl: BackendPreflight::Unavailable("context missing".to_string()),
            },
            true,
        )
        .unwrap();
        let WebPreflightDecision::SkipAllowed(reason) = decision else {
            panic!("both missing backends must use the explicit skip outcome");
        };
        assert!(reason.contains("adapter missing"));
        assert!(reason.contains("context missing"));
    }

    #[test]
    fn web_preflight_policy_rejects_pending_state_even_when_skips_are_allowed() {
        for allow_unavailable in [false, true] {
            assert!(decide_web_preflight(WebPreflight::Pending, allow_unavailable).is_err());
        }
    }

    #[test]
    fn renderer_sample_coordinates_apply_panel_offset_and_css_scale_once() {
        let state = json!({
            "canvas": { "rect": { "left": 10, "top": 20, "width": 800, "height": 600 } },
            "probe": { "viewport": {
                "width": 400,
                "height": 300,
                "paintBounds": { "left": 4, "top": 12 },
            } },
        });
        assert_eq!(
            renderer_css_point(&state, [17.0, 19.0]).unwrap(),
            [52.0, 82.0]
        );
        let mut invalid = state;
        invalid["probe"]["viewport"]["width"] = json!(0);
        assert!(renderer_css_point(&invalid, [17.0, 19.0]).is_err());
    }

    #[test]
    fn screenshot_color_tolerance_preserves_exact_alpha_and_rejects_channel_errors() {
        let expected = [230, 57, 70, 255];
        assert!(pixel_matches([233, 54, 72, 255], expected));
        assert!(!pixel_matches([234, 57, 70, 255], expected));
        assert!(!pixel_matches([230, 57, 70, 254], expected));
        assert!(!pixel_matches([70, 57, 230, 255], expected));
    }
}
