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
const BROWSER_TIMEOUT: Duration = Duration::from_secs(30);
const CDP_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(250);

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

pub(crate) fn web_smoke(root: &Path) -> Result<(), ()> {
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

    let mut browser = BrowserProcess::launch(&url).map_err(|error| {
        eprintln!("{error}");
    })?;
    let websocket_url = browser.wait_for_page_websocket(&url).map_err(|error| {
        eprintln!("{error}");
    })?;
    let mut cdp = CdpClient::connect(&websocket_url).map_err(|error| {
        eprintln!("{error}");
    })?;

    run_browser_smoke(&mut cdp).map_err(|error| {
        eprintln!("{error}");
    })?;

    Ok(())
}

fn run_browser_smoke(cdp: &mut CdpClient) -> Result<(), String> {
    cdp.call("Page.enable", json!({}))?;
    cdp.call("Runtime.enable", json!({}))?;

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
    let final_state = wait_for_state(cdp, "keyboard delivery", |state| {
        state.pointer("/probe/keyEvents").and_then(Value::as_u64) == Some(1)
            && state.pointer("/input/focused").and_then(Value::as_bool) == Some(true)
            && state
                .pointer("/probe/shellInteractions")
                .and_then(Value::as_u64)
                .is_some_and(|count| count >= 2)
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

    println!(
        "web smoke passed: app ready, canvas initialized, input delivered, shell interaction observed, platform viewports unsupported"
    );
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

    let Some(relative_path) = request_path_to_file(parts[1]) else {
        return write_response(&mut stream, 400, "text/plain", b"bad path");
    };
    let path = root.join(&relative_path);
    let Ok(bytes) = fs::read(&path) else {
        return write_response(&mut stream, 404, "text/plain", b"not found");
    };

    write_response(&mut stream, 200, content_type(&relative_path), &bytes)
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

        let mut args = vec![
            "--headless=new".to_string(),
            "--disable-background-networking".to_string(),
            "--disable-dev-shm-usage".to_string(),
            "--disable-gpu-sandbox".to_string(),
            "--no-first-run".to_string(),
            "--no-sandbox".to_string(),
            "--enable-unsafe-webgpu".to_string(),
            "--remote-allow-origins=*".to_string(),
            format!("--remote-debugging-port={remote_port}"),
            format!("--user-data-dir={}", user_data_dir.display()),
        ];
        if cfg!(target_os = "linux") {
            args.push("--enable-unsafe-swiftshader".to_string());
            args.push("--use-angle=vulkan".to_string());
            args.push("--enable-features=Vulkan,VulkanFromANGLE".to_string());
            args.push("--disable-vulkan-surface".to_string());
        }
        if let Ok(extra_args) = env::var("OPEN_GPUI_WEB_SMOKE_BROWSER_ARGS") {
            args.extend(extra_args.split_whitespace().map(ToOwned::to_owned));
        }
        args.push(url.to_string());

        println!("==> {} {}", browser.display(), args.join(" "));
        let child = Command::new(&browser)
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                format!("failed to launch browser `{}`: {error}", browser.display())
            })?;

        Ok(Self {
            child,
            remote_port,
            user_data_dir,
        })
    }

    fn wait_for_page_websocket(&mut self, url: &str) -> Result<String, String> {
        let started = Instant::now();
        let mut last_error = String::new();
        while started.elapsed() < BROWSER_TIMEOUT {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|error| format!("failed to inspect browser status: {error}"))?
            {
                return Err(format!(
                    "browser exited before web smoke connected: {status}"
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

        Err(format!(
            "timed out waiting for browser remote debugging page; last error: {last_error}"
        ))
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
        self.call(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseMoved",
                "x": x,
                "y": y,
                "button": "none",
                "buttons": 0,
            }),
        )?;
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
}
