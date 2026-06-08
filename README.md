# Open GPUI

Open GPUI is an independent Apache-2.0 Rust UI framework forked from Zed's GPUI codebase.

The project keeps the GPUI framework lineage while separating it from the Zed editor workspace, package names, and fork dependency ownership. The public Cargo package is `open-gpui`, and the Rust import path is `open_gpui::...`.

## Status

Open GPUI is pre-1.0 and in active fork cleanup. The workspace package names are prepared for crates.io as `open-gpui-*`, and Rust crate names use the corresponding underscore form such as `open_gpui`, `open_gpui_platform`, and `open_gpui_wgpu`.

Open GPUI depends on Open GPUI-maintained forks for screen capture and font handling:

- `open-gpui-scap`, published as `open-gpui-scap`, from `https://github.com/Latias94/scap`, licensed under MIT.
- `open-gpui-font-kit`, published as `open-gpui-font-kit`, from `https://github.com/Latias94/font-kit`, licensed under `MIT OR Apache-2.0`.

Their upstream copyright notices and license terms are preserved separately from Open GPUI's own license.

The workspace is published in dependency order: leaf crates such as `open-gpui-core-util` must be published before crates that depend on them, such as `open-gpui-collections` and `open-gpui`.

## Usage

Add the main framework crate:

```toml
[dependencies]
open_gpui = { package = "open-gpui", version = "0.1.0" }
open_gpui_platform = { package = "open-gpui-platform", version = "0.1.0" }
```

Use `open_gpui::...` in Rust code:

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

During local development, use workspace path dependencies instead of registry versions.

## Repository Layout

- `crates/gpui`: main `open-gpui` framework crate
- `crates/gpui_platform`: platform selector crate
- `crates/gpui_linux`, `crates/gpui_macos`, `crates/gpui_windows`, `crates/gpui_web`: platform backends
- `crates/gpui_wgpu`: renderer backend
- `crates/gpui_macros`: Open GPUI proc macros
- `crates/canvas`: reusable `open-gpui-canvas` model and interaction primitives for infinite canvas applications
- `examples/smoke-native`: native smoke example
- `xtask`: workspace verification and import-boundary checks

## Verification

Run the local verification gate with:

```sh
cargo run -p xtask -- verify
```

For details, see [docs/verification.md](docs/verification.md).

## Acknowledgements

Open GPUI builds on the work of several open-source projects and communities:

- [Zed GPUI](https://github.com/zed-industries/zed), developed by Zed Industries, is the upstream Apache-2.0 GPUI framework lineage that Open GPUI was forked from.
- [scap](https://github.com/CapSoftware/scap) provides the screen capture library lineage used by the Open GPUI-maintained `open-gpui-scap` fork.
- [font-kit](https://github.com/servo/font-kit) provides the cross-platform font loading library lineage used by the Open GPUI-maintained `open-gpui-font-kit` fork.
- [JSON Canvas](https://jsoncanvas.org/) provides the open canvas interchange format used by the `open-gpui-canvas` JSON Canvas adapter.
- The Rust and crates.io ecosystem provides the third-party crates listed by Cargo metadata and the lockfile.

## License and Attribution

Open GPUI is a fork of the Apache-2.0 GPUI framework code originally developed in the Zed repository by Zed Industries. This repository is not the Zed editor and does not include Zed's GPL application crates.

Open GPUI is licensed under Apache-2.0. The root [LICENSE-APACHE](LICENSE-APACHE) file preserves the original Zed copyright notice, and [NOTICE](NOTICE) records the fork attribution and Open GPUI modification notice. New Open GPUI-specific work is maintained under the same Apache-2.0 license unless a file explicitly states otherwise.

Third-party dependencies, including forked dependencies, retain their own licenses and copyright notices. Before publishing release artifacts, generate or update a dependency license inventory from the resolved Cargo graph and include it with the distribution when required by those licenses.
