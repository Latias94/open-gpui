# Open GPUI

Open GPUI is an independent Apache-2.0 Rust UI framework forked from Zed's GPUI codebase.

The project keeps the GPUI framework lineage while separating it from the Zed editor workspace, package names, and fork dependency ownership. The public Cargo package is `open-gpui`, and the Rust import path is `open_gpui::...`.

## Status

Open GPUI is pre-1.0 and in active fork cleanup. The workspace package names are prepared for crates.io as `open-gpui-*`, and Rust crate names use the corresponding underscore form such as `open_gpui`, `open_gpui_platform`, and `open_gpui_wgpu`.

Open GPUI currently requires Rust 1.92 or newer. The floor follows the resolved dependency graph and is checked by `cargo run -p xtask -- dependency-health`.

Open GPUI depends on Open GPUI-maintained forks for screen capture and font handling:

- `open-gpui-scap`, published as `open-gpui-scap`, from `https://github.com/Latias94/scap`, licensed under MIT.
- `open-gpui-font-kit`, published as `open-gpui-font-kit`, from `https://github.com/Latias94/font-kit`, licensed under `MIT OR Apache-2.0`.

Their upstream copyright notices and license terms are preserved separately from Open GPUI's own license.

The workspace is published in dependency order: leaf crates such as `open-gpui-core-util` must be published before crates that depend on them, such as `open-gpui-collections` and `open-gpui`.

## Usage

Add the main framework crate:

```toml
[dependencies]
open_gpui = { package = "open-gpui", version = "0.2.0" }
open_gpui_platform = { package = "open-gpui-platform", version = "0.2.0" }
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

## First-Party Add-Ons

- Use `open-gpui-platform` for application startup across native and web targets.
- Use `open-gpui-ui-components` for the official GPUI component library, including typed action projection, host-controlled `VirtualizedList`, and component contract evidence.
- Use `open-gpui-motion` when a component or domain crate needs deterministic, renderer-neutral motion samples and frame-demand facts. It is not a global animation engine.
- Use `open-gpui-docking` for retained tab stacks, splits, product panel placement, in-window floating panels, and capability-gated platform viewport windows. Start with the minimal example before the diagnostic dogfood example.
- Use `open-gpui-web` only for web backend work; most applications should continue to enter through `open-gpui-platform`.

## Product Correctness Primitives

Open GPUI is pre-1.0, so the framework favors explicit product contracts over compatibility shims:

- Scroll surfaces return typed `ScrollWheelIntent` values, publish committed `ScrollViewportSnapshot` facts, and expose test probes such as `TestInputDispatchSnapshot` for final input outcomes.
- Docking apps declare product intent with `DockPanelPlacement`, descriptor default placement, and last-known reopen placement instead of holding graph node ids in normal product code.
- Component apps project command/action metadata through `CommandIconDescriptor`, `ActionDescriptor`, and `ResolvedActionState`, then reuse that resolved state across buttons, toolbars, menus, command palettes, and sidebars.
- `VirtualizedList` keeps stable-key state in `VirtualizedListState` while application shells may provide their own GPUI `ScrollHandle` and request keyed reveals through `scroll_target_for_key` or `scroll_target_for_key_with_snapshot`.

See [docs/ui/command-ecosystem.md](docs/ui/command-ecosystem.md), [docs/ui/component-contract.md](docs/ui/component-contract.md), [crates/ui_components/README.md](crates/ui_components/README.md), and [crates/gpui_docking/README.md](crates/gpui_docking/README.md) for the current public surfaces.

## Repository Layout

- `crates/gpui`: main `open-gpui` framework crate; see [crates/gpui/README.md](crates/gpui/README.md)
- `crates/gpui_platform`: platform selector crate; see [crates/gpui_platform/README.md](crates/gpui_platform/README.md)
- `crates/gpui_linux`, `crates/gpui_macos`, `crates/gpui_windows`: native platform backends
- `crates/gpui_web`: WebAssembly platform backend; see [crates/gpui_web/README.md](crates/gpui_web/README.md)
- `crates/gpui_wgpu`: renderer backend
- `crates/gpui_macros`: Open GPUI proc macros
- `crates/ui_core`: renderer-neutral UI contracts, geometry, virtualizer math, and component state helpers
- `crates/ui_components`: official component library surfaces such as Listbox, Command, Table,
  Tree, and VirtualizedList; see [crates/ui_components/README.md](crates/ui_components/README.md)
- `crates/motion`: renderer-neutral `open-gpui-motion` timing, spring, policy, projection, and
  frame-demand primitives; see [crates/motion/README.md](crates/motion/README.md)
- `crates/gpui_docking`: retained docking graph, workspace, host, and viewport primitives; see
  [crates/gpui_docking/README.md](crates/gpui_docking/README.md)
- `crates/canvas`: reusable `open-gpui-canvas` model and interaction primitives for infinite canvas applications
- `examples/canvas-notes`: native JSON Canvas note-map example
- `examples/docking-minimal`: minimal single-window docking example using common public APIs
- `examples/docking-native`: native docking dogfood example with viewport runtime diagnostics
- `examples/smoke-native`: native smoke example
- `examples/ui-foundation-gallery`: native UI component gallery and conformance surface
- `xtask`: workspace verification and import-boundary checks

Run normal-checkout examples with:

```sh
cargo run -p open-gpui-canvas-notes
cargo run -p open-gpui-docking-minimal
cargo run -p open-gpui-docking-native
cargo run -p open-gpui-ui-foundation-gallery
```

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
