# Open GPUI Platform

`open-gpui-platform` owns the native desktop backends for Open GPUI. Use it to construct an `Application` for the current Rust target.

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

## Desktop Backends

- On Windows, `windows::WindowsPlatform` and the renderer factory traits support native hosts. `test-support` enables the native window probes.
- On macOS, `macos::MacPlatform` owns AppKit lifecycle and windowing. The backend includes Apple dispatcher and process services from the Apache-2.0 Zed GPUI implementation. `font-kit` enables its CoreText/OpenType text system.
- On Linux and FreeBSD, enable `wayland`, `x11`, or both for a graphical application. The headless backend is available without these features.

`application_with_shared_wgpu` creates a native application whose windows use a caller-supplied WGPU context. `open-gpui-wgpu` owns rendering and atlas resources. Native capabilities such as desktop viewport windows are discovered at runtime.

OpenHarmony applications can inject a backend with `Application::with_platform`.

For desktop startup, see [`smoke-native`](../../examples/smoke-native/src/main.rs). For native
managed windows and recovery, see the [Dockspace editor](../../examples/dockspace-editor/README.md).

With `test-support`, `current_headless_renderer()` can create a WGPU renderer for desktop visual tests.

## Verification

For focused platform work, run:

```sh
cargo check -p open-gpui-platform --locked
cargo run -p xtask -- verify-release-docs
```
