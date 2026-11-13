# Open GPUI

Open GPUI is an independent Apache-2.0 Rust UI framework forked from Zed's GPUI codebase.

The project keeps the GPUI framework lineage while separating it from the Zed editor workspace, package names, and fork dependency ownership. The public Cargo package is `open-gpui`, and the Rust import path is `open_gpui::...`.

## Status

Open GPUI is pre-1.0 and in active fork cleanup. The workspace package names are prepared for crates.io as `open-gpui-*`, and Rust crate names use the corresponding underscore form such as `open_gpui`, `open_gpui_platform`, and `open_gpui_wgpu`.

The GPUI core, platform/rendering packages and UI components require Rust 1.93 or newer. Docking and its DevTools, Gallery, example and workspace-tooling consumers require Rust 1.95 because the selected shared `dockspace` package declares that floor. The package boundaries follow the resolved dependency graph and are checked by `cargo run -p xtask -- dependency-health`; CI checks the two minimum toolchains independently.

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
- Use `open-gpui-form` for renderer-neutral form ownership: field identity, dirty/touched/visited meta, validation generations, submit/reset lifecycle, typed lenses, dynamic JSON values, and redacted snapshots.
- Use `open-gpui-resource` for renderer-neutral async query and mutation ownership: deterministic query keys, observers, stale/invalidate state, retry policy, pagination snapshots, mutation lifecycle, and redacted cache diagnostics. Fetching stays protocol-agnostic.
- Use `open-gpui-devtools` for read-only local inspection: app-owned probes and feature-gated first-party adapters collect target/domain/event captures, legacy snapshot envelopes, command facts, timeline events, layout geometry, diagnostics, redaction summaries, schema-versioned headless artifacts, and an optional GPUI inspector surface behind the `gpui` feature.
- Use `open-gpui-motion` when a component or domain crate needs deterministic, renderer-neutral motion samples and frame-demand facts. It is not a global animation engine.
- Use `open-gpui-docking` for a shared `DockspaceSession` with retained tab Views, weighted splits, floating content and native windows. Start with the minimal example, then multiviewport or the editor example.
- Use `open-gpui-web` only for web backend work; most applications should continue to enter through `open-gpui-platform`.
- Use `open-gpui-wgpu` for [application-owned Scene recording and retained GPU images](docs/external-wgpu.md). The recording module reuses your device and encoder; complete external window/event-loop hosting remains in development.

## Product Correctness Primitives

Open GPUI is pre-1.0, so the framework favors explicit product contracts over compatibility shims:

- Scroll surfaces return typed `ScrollWheelIntent` values, publish committed `ScrollViewportSnapshot` facts, and expose test probes such as `TestInputDispatchSnapshot` for final input outcomes.
- Native window placement and independent flag requests expose property-specific capabilities, seven generation domains, typed dispatch, generation-bound terminal observations, and one committed `WindowPlatformFacts` cache; a queued request is never treated as an applied OS fact.
- Docking apps retain one shared session and use its revision-bound `DockPlacement` actions; each View has a distinct pane identity and entity while documents and history remain application-owned.
- Component apps project command/action metadata through `CommandIconDescriptor`, `ActionDescriptor`, and `ResolvedActionState`, then reuse that resolved state across buttons, toolbars, menus, command palettes, and sidebars.
- `VirtualizedList` keeps stable-key state in `VirtualizedListState`; GPUI shells opt into host-owned `ScrollHandle` and custom row rendering through `gpui_adapter::VirtualizedListGpuiExt`. `bring_key_into_view` materializes the keyed row before GPUI performs final nested physical alignment through its window-owned reveal authority.
- Form and resource apps keep long-lived state in `open-gpui-form` and `open-gpui-resource`, then project snapshots into existing UI components through `FormFieldProjection`, `ResourceCollectionProjection`, and `ResourceMutationProjection`. Devtools consumes redacted snapshots through feature-gated adapters and does not mutate app state.

See [docs/ui/command-ecosystem.md](docs/ui/command-ecosystem.md), [docs/ui/component-contract.md](docs/ui/component-contract.md), [crates/ui_components/README.md](crates/ui_components/README.md), [crates/form/README.md](crates/form/README.md), [crates/resource/README.md](crates/resource/README.md), [crates/devtools/README.md](crates/devtools/README.md), and [crates/gpui_docking/README.md](crates/gpui_docking/README.md) for the current public surfaces.

## Repository Layout

- `crates/gpui`: main `open-gpui` framework crate; see [crates/gpui/README.md](crates/gpui/README.md)
- `crates/gpui_platform`: platform selector crate; see [crates/gpui_platform/README.md](crates/gpui_platform/README.md)
- `crates/gpui_linux`, `crates/gpui_macos`, `crates/gpui_windows`: native platform backends
- `crates/gpui_apple`: shared Apple renderer, dispatcher, text and system services; see [crates/gpui_apple/README.md](crates/gpui_apple/README.md)
- `crates/gpui_web`: WebAssembly platform backend; see [crates/gpui_web/README.md](crates/gpui_web/README.md)
- `crates/gpui_wgpu`: renderer backend
- `crates/gpui_macros`: Open GPUI proc macros
- `crates/ui_core`: renderer-neutral UI contracts, geometry, virtualizer math, and component state helpers
- `crates/ui_components`: official component library surfaces such as Listbox, Command, Table,
  Tree, and VirtualizedList; see [crates/ui_components/README.md](crates/ui_components/README.md)
- `crates/form`: renderer-neutral form state, validation, submission, lenses, and redacted snapshots; see [crates/form/README.md](crates/form/README.md)
- `crates/resource`: renderer-neutral async query/mutation state, observers, retry policy, pagination, and redacted snapshots; see [crates/resource/README.md](crates/resource/README.md)
- `crates/devtools`: read-only target/domain/event capture, probe registry, serializable snapshot envelopes, artifact writer/sink APIs, command-line query/assert/follow diagnostics, command/timeline/layout adapters, and optional GPUI inspector UI; see [crates/devtools/README.md](crates/devtools/README.md)
- `crates/motion`: renderer-neutral `open-gpui-motion` timing, spring, policy, projection, and
  frame-demand primitives; see [crates/motion/README.md](crates/motion/README.md)
- `crates/gpui_docking`: shared DockspaceSession, retained Views and GPUI native transport; see
  [crates/gpui_docking/README.md](crates/gpui_docking/README.md)
- `crates/canvas`: reusable `open-gpui-canvas` model and interaction primitives for infinite canvas applications
- `examples/canvas-notes`: native JSON Canvas note-map example
- `examples/docking-minimal`: minimal single-window docking example using common public APIs
- `examples/docking-native`: explicit legacy native docking qualification and diagnostics
- `examples/smoke-native`: native smoke example
- `examples/ui-foundation-gallery`: native UI component gallery and conformance surface
- `xtask`: workspace verification and import-boundary checks

Run normal-checkout examples with:

```sh
cargo run -p open-gpui-canvas-notes
cargo run -p open-gpui-docking-minimal
cargo run -p open-gpui-docking-multiviewport
cargo run -p open-gpui-dockspace-editor
cargo run -p open-gpui-docking-native --features legacy-docking
cargo run -p open-gpui-ui-foundation-gallery
cargo run -p open-gpui-ui-foundation-gallery -- --page devtools
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
