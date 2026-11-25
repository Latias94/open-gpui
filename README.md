# Open GPUI

Open GPUI is an independent Apache-2.0 Rust UI framework forked from Zed's GPUI codebase.

The project keeps the GPUI framework lineage while separating it from the Zed editor workspace, package names, and fork dependency ownership. The public Cargo package is `open-gpui`, and the Rust import path is `open_gpui::...`.

## Status

Open GPUI is pre-1.0 and in active fork cleanup. The workspace package names are prepared for crates.io as `open-gpui-*`, and Rust crate names use the corresponding underscore form such as `open_gpui`, `open_gpui_platform`, and `open_gpui_wgpu`.

GPUI core, desktop backends, renderers and UI components require Rust 1.93 or newer. Docking, its
examples, and workspace tooling require Rust 1.95. Use Rust 1.95.0 for local workspace verification.

Desktop backends use DirectWrite on Windows, CoreText on macOS and the Open GPUI-maintained
`open-gpui-font-kit` fork where enabled.

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

- Use `open-gpui-platform` to start desktop applications.
- Use `open-gpui-ui-components` for the official GPUI component library, including typed action projection, host-controlled `VirtualizedList`, and component contract evidence.
- Use `open-gpui-resource` for renderer-neutral async query and mutation ownership: deterministic query keys, observers, stale/invalidate state, retry policy, pagination snapshots, mutation lifecycle, and redacted cache diagnostics. Fetching stays protocol-agnostic.
- Use `open-gpui-docking` for a shared `DockspaceSession` with retained tab Views, weighted splits, floating content and native windows. Start with the minimal example, then multiviewport or the editor example.
- Use `open-gpui-wgpu` for [application-owned Scene recording and retained GPU images](docs/external-wgpu.md). The recording module reuses your device and encoder; complete external window/event-loop hosting remains in development.

## Product Correctness Primitives

Open GPUI is pre-1.0, so the framework favors explicit product contracts over compatibility shims:

- Scroll surfaces return typed `ScrollWheelIntent` values, publish committed `ScrollViewportSnapshot` facts, and expose test probes such as `TestInputDispatchSnapshot` for final input outcomes.
- Native window placement and independent flag requests expose property-specific capabilities, seven generation domains, typed dispatch, generation-bound terminal observations, and one committed `WindowPlatformFacts` cache; a queued request is never treated as an applied OS fact.
- Docking apps retain one shared session and use its revision-bound `DockPlacement` actions; each View has a distinct pane identity and entity while documents and history remain application-owned.
- Component apps project command/action metadata through `ActionDescriptor` and `ResolvedActionState`, then reuse that resolved state across buttons, toolbars, menus, command palettes, and sidebars.
- `VirtualizedList` keeps stable-key state in `VirtualizedListState`; GPUI shells opt into host-owned `ScrollHandle` and custom row rendering through `gpui_adapter::VirtualizedListGpuiExt`. `bring_key_into_view` materializes the keyed row before GPUI performs final nested physical alignment through its window-owned reveal authority.
- Resource-backed components project app-owned snapshots through `ResourceCollectionProjection` and `ResourceMutationProjection`.

See the [component guide](crates/ui_components/README.md), [resource guide](crates/resource/README.md),
and [docking guide](crates/gpui_docking/README.md).

## Repository Layout

- `crates/gpui`: main `open-gpui` framework crate; see [crates/gpui/README.md](crates/gpui/README.md)
- `crates/gpui_platform`: native Windows, macOS, Linux, and FreeBSD backends. See the [platform guide](crates/gpui_platform/README.md).
- `crates/gpui_wgpu`: renderer backend
- `crates/gpui_macros`: Open GPUI proc macros
- `crates/ui_core`: renderer-neutral UI contracts, geometry, virtualizer math, and component state helpers
- `crates/ui_components`: official component library surfaces such as Listbox, Command, Table,
  Tree, and VirtualizedList; see [crates/ui_components/README.md](crates/ui_components/README.md)
- `crates/resource`: renderer-neutral async query/mutation state, observers, retry policy, pagination, and redacted snapshots; see [crates/resource/README.md](crates/resource/README.md)
- `crates/gpui_util`: core utilities and shared motion primitives
- `crates/gpui_docking`: shared DockspaceSession, retained Views and GPUI native transport; see
  [crates/gpui_docking/README.md](crates/gpui_docking/README.md)
- `examples/docking-minimal`: minimal single-window docking example using common public APIs
- `examples/smoke-native`: native smoke example
- `xtask`: workspace verification and import-boundary checks

Run normal-checkout examples with:

```sh
cargo run -p open-gpui-docking-minimal
cargo run -p open-gpui-docking-multiviewport
cargo run -p open-gpui-dockspace-editor
cargo run -p open-gpui-smoke-native
```

The [dockspace editor example](examples/dockspace-editor/README.md) demonstrates
explicit workspace reconstruction with retained View entities and document edits. The
[Windows WGPU example](examples/wgpu-native/README.md) exercises native and external hosts.

Use local files, `AssetSource`, or application-provided image data with `img()`. The
`legacy/v0-pre-slim` tag preserves the previous package tree and qualification records.

## Verification

Run the local verification gate with:

```sh
cargo run -p xtask -- verify
```

For details, see [docs/verification.md](docs/verification.md).

## Acknowledgements

Open GPUI builds on the work of several open-source projects and communities:

- [Zed GPUI](https://github.com/zed-industries/zed), developed by Zed Industries, is the upstream Apache-2.0 GPUI framework lineage that Open GPUI was forked from.
- [font-kit](https://github.com/servo/font-kit) provides the cross-platform font loading library lineage used by the Open GPUI-maintained `open-gpui-font-kit` fork.
- The Rust and crates.io ecosystem provides the third-party crates listed by Cargo metadata and the lockfile.

## License and Attribution

Open GPUI is a fork of the Apache-2.0 GPUI framework code originally developed in the Zed repository by Zed Industries. This repository is not the Zed editor and does not include Zed's GPL application crates.

Open GPUI is licensed under Apache-2.0. The root [LICENSE-APACHE](LICENSE-APACHE) file preserves the original Zed copyright notice, and [NOTICE](NOTICE) records the fork attribution and Open GPUI modification notice. New Open GPUI-specific work is maintained under the same Apache-2.0 license unless a file explicitly states otherwise.

Third-party dependencies, including forked dependencies, retain their own licenses and copyright notices. Before publishing release artifacts, generate or update a dependency license inventory from the resolved Cargo graph and include it with the distribution when required by those licenses.
