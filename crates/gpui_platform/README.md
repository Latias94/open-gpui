# Open GPUI Platform

`open-gpui-platform` selects the desktop backend for an Open GPUI application. It is the normal application entry point for constructing an `Application` without importing a specific backend crate.

Use this crate when an application wants one dependency that maps the current Rust target to the correct Open GPUI backend:

```toml
[dependencies]
open_gpui = { package = "open-gpui", version = "0.2.0" }
open_gpui_platform = { package = "open-gpui-platform", version = "0.2.0" }
```

```rust
use open_gpui::{App, Context, Render, Window, div, prelude::*};
use open_gpui_platform::application;

struct Hello;

impl Render for Hello {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child("Hello from Open GPUI")
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(Default::default(), |_, cx| cx.new(|_| Hello))
            .expect("failed to open window");
    });
}
```

## What This Crate Owns

- Target-specific backend selection for macOS, Windows, Linux and FreeBSD.
- Feature forwarding for backend options such as `font-kit`, `wayland`, `x11`, and `runtime_shaders`.
- A small application-facing import surface so examples and applications do not need to choose backend crates manually.

## Boundaries

This crate does not own rendering primitives, window layout policy, component APIs, docking behavior, or backend-specific platform services. Those live in `open-gpui`, `open-gpui-wgpu`, `open-gpui-ui-components`, `open-gpui-docking`, and the platform backend crates.

Backend capabilities such as native viewport windows and compositor features are discovered at runtime.

OpenHarmony is excluded from Linux backend selection; applications on that target inject
their own backend with `Application::with_platform`.

For desktop startup, see [`smoke-native`](../../examples/smoke-native/src/main.rs). For native
managed windows and recovery, see the [Dockspace editor](../../examples/dockspace-editor/README.md).

With `test-support`, `current_headless_renderer()` returns an optional renderer for visual tests.
The default factory supplies Metal on macOS and WGPU on Linux. It returns `Ok(None)` on Windows,
where tests can inject a WGPU renderer explicitly.

## Verification

For focused platform-selector work, run:

```sh
cargo check -p open-gpui-platform --locked
cargo run -p xtask -- verify-release-docs
```
