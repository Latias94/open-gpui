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

## Boundaries

This crate does not provide browser platform windows, docking tear-off windows, DOM component rendering, CSS animation integration, or a public web-specific component library. WebGPU availability is a required runtime fact for rendering; shared-memory worker mode is optional and must fail back to the stable single-threaded path when unavailable.

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
