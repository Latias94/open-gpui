# Open GPUI Docking

`open-gpui-docking` contains retained docking graph, workspace, host, and viewport primitives for
Open GPUI applications. It separates durable layout state from GPUI runtime state so applications
can persist dock spaces while keeping panel views, platform windows, drag sessions, and visual
affordances behind runtime adapters.

Use this crate when an application needs IDE-style tab stacks, split panes, floating panels, and
optional platform viewport windows.

## Public API Tiers

The crate root and `open_gpui_docking::prelude` contain the common application API: graph/layout types, `DockController`, `DockWorkspace`, `DockHost`, panel registry/catalog types, policy types, viewport placement layout, viewport open/close outcomes, and `DockViewportRuntimeHandle`.

Diagnostics and transition internals live behind `open_gpui_docking::advanced`. Import that module explicitly for runtime status records, visual-affordance debug summaries, transition plans, and transition execution states. These types are useful for tests and tooling, but they are not part of the default application surface.

## What This Crate Owns

- `DockGraph` and `DockLayout` for logical dock spaces, tab stacks, splits, in-window floating
  layout, serialization, validation, and graph operations.
- `DockController` and `DockWorkspace` as the preferred shared owner for rendered hosts and
  programmatic layout commands.
- `DockHost` as the GPUI renderer for one logical dock space, including tab chrome, splitter
  interaction, floating panels, drop previews, accessibility descriptors, and motion-backed visual
  affordances.
- `DockHostOptions::motion_preference` as the host-owned reduced-motion policy input for zoom,
  unzoom, and visual-affordance transitions.
- `DockPanelRegistry` and `DockPanelCatalog` for lazy panel factories, descriptor-only restore
  metadata, GPUI view attachment, close/reopen policy, and tab labels.
- `DockViewportRuntimeHandle` and internal viewport runtime modules for controller-backed platform
  window routing, placement snapshots, lifecycle cleanup, and cross-window drop routing. Runtime
  status diagnostics are available through the explicit `advanced` API tier.

## Capability Gates

In-window floating and platform viewport windows are separate capabilities.

Platform viewport windows fail closed unless both gates are true:

- Application policy allows them through `DockPolicy::allow_platform_viewports(true)`.
- The active backend reports `PlatformViewportCapabilities::platform_viewport_windows`.

Unsupported backends should record unsupported viewport status and no-op for open or tear-off
requests instead of constructing partial runtime state. Web and other backends without platform
window support stay on the single-window route.

Persist `DockLayout` separately from viewport placement data. The layout restores logical dock
spaces; `DockViewportPlacementLayout` restores platform-window hints for the runtime adapter.

## Examples

Run the minimal single-window docking example first:

```sh
cargo run -p open-gpui-docking-minimal
```

This example uses `DockController::builder`, lazy panel factories, `DockHost::from_controller`, and
`DockViewportRuntimeHandle` without importing `open_gpui_docking::advanced`. It enables in-window
floating, but it does not enable platform viewport windows.

Run the normal-checkout native dogfood example when working on viewport runtime behavior or
diagnostics:

```sh
cargo run -p open-gpui-docking-native
```

The dogfood example demonstrates a controller-backed host with registered panels, tab stacks, split
layout, floating behavior, capability-gated platform viewport windows, and the runtime diagnostic
paths used by the docking tests.

## Minimal Shape

```rust
use open_gpui::{AnyView, App};
use open_gpui_docking::prelude::{DockController, DockPanelPlacement};

fn panel_factory(_cx: &mut App) -> AnyView {
    unreachable!("create and return a GPUI view for the panel")
}

let controller = DockController::builder("main")
    .panel_placements([
        DockPanelPlacement::left_rail("explorer").fraction(0.24),
        DockPanelPlacement::center("editor").selected(),
        DockPanelPlacement::bottom_rail("terminal").fraction(0.30),
    ])
    .panel_factory("explorer", "Explorer", panel_factory)
    .panel_factory("editor", "Editor", panel_factory)
    .panel_factory("terminal", "Terminal", panel_factory)
    .allow_floating(true)
    .try_build()
    .expect("dock controller setup should validate");

let _ = controller;
```

Use `DockPanelPlacement::stacked_with(item, anchor)` to add tabs beside another panel without
depending on generated node ids. If an anchor may be missing during restore, attach a fallback such
as `.fallback(DockPanelPlacementTarget::right_rail())`.

Only call `allow_platform_viewports(true)` when the application intends to use platform-window
docking routes and is prepared for unsupported runtime capability results on web or compositor
backends without viewport-window support.

## Verification

For focused docking changes, run:

```sh
cargo fmt -p open-gpui-docking
cargo check -p open-gpui-docking --tests --locked
cargo check -p open-gpui-docking-minimal --locked
cargo check -p open-gpui-docking-native --locked
cargo nextest run -p open-gpui-docking host_viewport_platform_capability_tests --no-fail-fast
```

For render-authority, preview, splitter, motion, accessibility, and viewport work, use the narrower
or full docking gates in `docs/verification.md`.
