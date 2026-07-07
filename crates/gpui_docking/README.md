# Open GPUI Docking

`open-gpui-docking` contains retained docking graph, workspace, host, and viewport primitives for
Open GPUI applications. It separates durable layout state from GPUI runtime state so applications
can persist dock spaces while keeping panel views, platform windows, drag sessions, and visual
affordances behind runtime adapters.

Use this crate when an application needs IDE-style tab stacks, split panes, floating panels, and
optional platform viewport windows.

## What This Crate Owns

- `DockGraph` and `DockLayout` for logical dock spaces, tab stacks, splits, in-window floating
  layout, serialization, validation, and graph operations.
- `DockController` and `DockWorkspace` as the preferred shared owner for rendered hosts and
  programmatic layout commands.
- `DockHost` as the GPUI renderer for one logical dock space, including tab chrome, splitter
  interaction, floating panels, drop previews, accessibility descriptors, and motion-backed visual
  affordances.
- `DockPanelRegistry` and `DockPanelCatalog` for lazy panel factories, descriptor-only restore
  metadata, GPUI view attachment, close/reopen policy, and tab labels.
- `DockViewportRuntimeHandle` and internal viewport runtime modules for controller-backed platform
  window routing, placement snapshots, lifecycle cleanup, cross-window drop routing, and status
  diagnostics.

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

## Demo

Run the normal-checkout native docking example:

```sh
cargo run -p open-gpui-docking-native
```

The example demonstrates a controller-backed host with registered panels, tab stacks, split layout,
floating behavior, and the runtime paths used by the docking tests.

## Minimal Shape

```rust
use open_gpui::{AnyView, App};
use open_gpui_docking::{DockController, EditorDockLayoutSpec};

fn panel_factory(_cx: &mut App) -> AnyView {
    unreachable!("create and return a GPUI view for the panel")
}

let controller = DockController::builder("main")
    .default_editor_layout(EditorDockLayoutSpec::new(
        ["explorer"],
        ["editor"],
        ["terminal"],
    ))
    .panel_factory("explorer", "Explorer", panel_factory)
    .panel_factory("editor", "Editor", panel_factory)
    .panel_factory("terminal", "Terminal", panel_factory)
    .allow_floating(true)
    .allow_platform_viewports(true)
    .try_build()
    .expect("dock controller setup should validate");

let _ = controller;
```

## Verification

For focused docking changes, run:

```sh
cargo fmt -p open-gpui-docking
cargo check -p open-gpui-docking --tests --locked
cargo check -p open-gpui-docking-native --locked
cargo nextest run -p open-gpui-docking host_viewport_platform_capability_tests --no-fail-fast
```

For render-authority, preview, splitter, motion, accessibility, and viewport work, use the narrower
or full docking gates in `docs/verification.md`.
