# Open GPUI Web

`open-gpui-web` is the WebAssembly platform backend for Open GPUI. It owns the browser event loop adapter, canvas setup, browser input translation, default web font registration, and WebGPU renderer integration needed by `open-gpui-platform` on `wasm32-unknown-unknown`.

Use this crate directly only when building a web-specific Open GPUI adapter or example. Most applications should depend on `open-gpui-platform` and let the platform selector choose the backend for the active target.

## What This Crate Owns

- Browser initialization through `web_init`.
- Stable single-threaded web runtime construction.
- Optional multithreaded runtime support behind the `multithreaded` feature and browser shared-memory capability checks.
- DOM canvas sizing, hidden input focus routing, pointer/keyboard/wheel event delivery, and browser clipboard/input adapter plumbing.
- Web backend capability facts, including the current unsupported platform-viewport window capability.

## Stable Browser Smoke

The stable browser proof lives in `crates/gpui_web/examples/smoke_web` and is exercised through:

```sh
cargo run -p xtask -- web-smoke
```

The smoke builds a single-threaded web example with Trunk, serves it locally, opens a headless Chrome/Chromium/Edge browser, and verifies app readiness, canvas initialization, focus/input delivery, a single-window shell interaction, explicit unsupported platform viewport windows, and a `DockSurface` viewport probe that returns typed `backend_unsupported` without opening a browser popout window.

The older `hello_web` example remains useful for local shared-memory and atomics experiments, but it is not the required CI smoke path.

## Browser Fetch

Create a Fetch client from the originating foreground executor and explicitly install it on the
Application before startup. `WebPlatform::fetch_http_client` and
`fetch_http_client_with_user_agent` use that platform's same executor. The client can be called
from workers; browser Fetch and the response pump stay on the main thread. Response chunks use
a bounded nonblocking channel, and dropping the response body cancels its browser stream.
Request bodies remain buffered before Fetch. `FetchCredentials` defaults to `SameOrigin` and
also supports `Omit` and `Include`.

The separate `http-client-test.html` entry in `examples/hello_web` retains the official eight
worker requests with 32,768 ordered chunks each and adds request/error, backpressure, body-drop
and missing-`waitAsync` fallback controls. Build with its existing nightly/shared-memory Cargo
configuration. Its single Rust pipeline avoids concurrent Cargo builds. From that directory,
build the test entry and serve its output with the isolation headers from `trunk.toml`:

```sh
trunk build http-client-test.html --filehash false --locked --html-output index.html
```

Then, from the repository root, run:

```sh
node crates/gpui_web/examples/hello_web/test_http_client.mjs http://127.0.0.1:8080
```

Set `CHROME` when the driver cannot find the browser. The driver uses Node's built-in WebSocket
and an external watchdog. Its hardware-concurrency override supplies eight worker slots without
depending on the host CPU count. Keep the standalone lock's event-listener version aligned with
the root and inspect the actual wasm feature graph: `event-listener/std` must remain disabled.
Browser execution and stable smoke remain distinct from wasm compilation. The exact source,
results and limits belong to the [verification record](../../docs/verification.md#sendable-foreground-submission-and-web-fetch).

## Browser Font Fallback

`CanvasFontFallback` is selected before platform construction. The default `Emoji` policy
uses browser fonts for eligible complete emoji graphemes when loaded fonts lack the glyph or
requested emoji presentation. `Disabled` uses only loaded fonts. `EmojiAndCjk` additionally
permits approximate independent rendering of a conservative horizontal CJK subset.

On `wasm32-unknown-unknown`, configure the ordinary platform directly:

```rust
use open_gpui_web::{CanvasFontFallback, WebBackendPreference, WebPlatform};

let platform = WebPlatform::new_with_backend_and_font_fallback(
    false,
    WebBackendPreference::Auto,
    CanvasFontFallback::EmojiAndCjk,
);
```

`WebPlatformOptions::canvas_font_fallback` exposes the same immutable choice through the existing
options constructor and `open-gpui-platform` factory. Stock WebPlatform registers its bundled
Inter Variable/JetBrains fonts; applications may install additional fonts through the ordinary App API.
The wrapper preserves actual selected-face weight/style, primary baseline/line spacing and
source indices. Canvas measurements and rasters enter the existing glyph atlas. Missing-glyph
notifications report unresolved text after browser fallback has run.

Browser font availability and appearance depend on the environment. Independent CJK graphemes
do not provide general contextual shaping, vertical text or complete font-feature fidelity.
The [qualification](../../docs/verification.md#web-canvas-font-fallback) distinguishes public
consumption, native portable tests, actual browser drawing and unmeasured performance.

## Boundaries

This crate does not provide browser platform windows, docking tear-off windows, DOM component rendering, CSS animation integration, or a public web-specific component library. A supported WebGPU or WebGL2 backend is required for rendering; shared-memory worker mode is optional and must fail back to the stable single-threaded path when unavailable.

Single-window Open GPUI shells can run on web through the platform selector when the renderer and
browser capabilities are available. Docking platform viewport routes must still report unsupported
capability results instead of pretending that browser popout windows are available.

## Verification

For focused web backend work, run:

```sh
cargo check -p open-gpui-web --target wasm32-unknown-unknown --tests --locked -j 1
cargo check -p open-gpui-web --target wasm32-unknown-unknown --locked -j 1
cargo run -p xtask -- web-smoke
```
