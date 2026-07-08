# Open GPUI Docking

`open-gpui-docking` contains a facade-first retained docking system for Open GPUI applications. It separates durable layout state from GPUI runtime state so applications can persist dock spaces while keeping panel views, platform windows, drag sessions, and visual affordances behind runtime adapters.

Use this crate when an application needs IDE-style tab stacks, split panes, floating panels, and
optional platform viewport windows.

## Public API Tiers

The crate root and `open_gpui_docking::prelude` contain the common application API: `DockSurface`, `DockSurfaceBuilder`, durable layout and placement data, panel registry/catalog types, policy types, and typed facade outcomes. The root intentionally does not re-export raw graph, action, host, workspace, runtime-handle, or raw layout-node types.

Low-level controller, graph, action, workspace, host, raw layout parts, and runtime-handle APIs live behind explicit modules. Use `open_gpui_docking::model` for `DockController`, `DockControllerBuilder`, graph/layout mutation tools, command objects, and `layout_from_raw_parts`/`layout_into_raw_parts`; use `open_gpui_docking::runtime` for `DockHost`, `DockHostOptions`, and direct viewport runtime integrations; use `open_gpui_docking::advanced` for diagnostics and transition internals. These types are useful for tests and tooling, but they are not part of the default application surface.

## What This Crate Owns

- `DockSurface` as the app-level owner for controller state, host-window creation, panel commands, and typed viewport capability outcomes.
- `DockSurfaceViewportSession`, `DockSurfaceViewportSpec`, `DockSurfaceViewportOpenReport`, and
  `DockSurfaceViewportRestoreReport` as facade-level platform window lifecycle, requests, and batch
  outcomes for multi-viewport applications.
- `DockSurfaceViewportShouldCloseOutcome` and `DockSurfaceViewportCloseOutcome` as facade-level
  lifecycle results for platform close hooks, including merge-back close policies.
- `DockLayout` as the common durable persistence type for serialization and validation. Raw
  `DockLayoutSpace`/`DockLayoutNode` construction is available only through the explicit `model`
  tier.
- `DockGraph` for model-tier logical dock spaces, tab stacks, splits, in-window floating layout, and
  graph operations.
- `open_gpui_docking::model::DockController` and `open_gpui_docking::model::DockWorkspace` as the
  low-level shared owner for rendered hosts and programmatic layout commands.
- `open_gpui_docking::runtime::DockHost` as the GPUI renderer for one logical dock space, including
  tab chrome, splitter interaction, floating panels, drop previews, accessibility descriptors, and
  motion-backed visual affordances.
- `open_gpui_docking::runtime::DockHostOptions::motion_preference` as the host-owned reduced-motion
  policy input for zoom, unzoom, and visual-affordance transitions.
- `DockPanelRegistry` and `DockPanelCatalog` for lazy panel factories, descriptor-only restore
  metadata, GPUI view attachment, close/reopen policy, and tab labels.
- `open_gpui_docking::runtime::DockViewportRuntimeHandle` and internal viewport runtime modules for
  controller-backed platform window routing, placement snapshots, lifecycle cleanup, and cross-window
  drop routing. Runtime handles are available through the explicit `runtime` API tier, and runtime
  status diagnostics are available through `advanced`.

## Capability Gates

In-window floating and platform viewport windows are separate capabilities.

Platform viewport windows fail closed unless both gates are true:

- Application policy allows them through `DockSurfaceBuilder::allow_platform_viewports(true)` or `DockPolicy`.
- The active backend reports `PlatformViewportCapabilities::platform_viewport_windows`.

`DockSurface::viewports` returns a `DockSurfaceViewportSession` for the common multi-window path. The session opens detached dock spaces, restores saved placement data, exports placement snapshots, and handles GPUI close hooks while keeping applications away from raw runtime handles. `DockSurface::open_viewport_spec` and `DockSurface::open_viewports` remain available for direct request construction and return facade outcomes so applications can distinguish policy-disabled, backend-unsupported, and backend-open failures without parsing opaque errors. Unsupported backends should no-op for open or tear-off requests instead of constructing partial runtime state. Web and other backends without platform window support stay on the single-window route.

Persist `DockLayout` separately from viewport placement data. The layout restores logical dock spaces; `DockViewportPlacementLayout` restores platform-window hints, and `DockSurfaceViewportSpec::with_saved_placement` applies those hints to fallback GPUI window options before a viewport opens. Applications can call `DockSurface::export_viewport_placement` to snapshot facade-opened platform windows and `DockSurface::check_viewport_placement_restore` to validate saved placement before reopening windows, without importing `DockViewportRuntimeHandle`.

Use `DockSurfaceBuilder::close_policy` or `DockSurface::set_viewport_close_policy` to choose how detached platform windows close. `DockViewportClosePolicy::RetainLayout` removes only the runtime window mapping, `Prevent` vetoes the platform close, and `MergeBack` moves a closing viewport's dock content into a fallback space. Applications with custom GPUI window hooks can call `DockSurface::handle_viewport_window_should_close`, `DockSurface::handle_viewport_window_closed`, and `DockSurface::cancel_viewport_window_close` without importing the low-level runtime handle.

## Restoring Platform Viewports

Use `DockSurface::viewports` when restoring detached spaces. This keeps placement validation, backend capability checks, and batch outcome reporting on the facade surface:

```rust
use open_gpui::{Bounds, WindowBounds, WindowOptions, px, size};
use open_gpui_docking::prelude::{
    DockSurfaceViewportOpenOutcome, DockViewportPlacementLayout,
};

fn restore_viewports(
    surface: &open_gpui_docking::prelude::DockSurface,
    saved_placement: &DockViewportPlacementLayout,
    cx: &mut open_gpui::App,
) {
    let viewports = surface.viewports();
    let report = viewports.restore(
        saved_placement,
        |_| {
            let fallback_options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(720.0), px(480.0)),
                    cx,
                ))),
                ..Default::default()
            };
            fallback_options
        },
        cx,
    );

    for outcome in report.outcomes() {
        if let DockSurfaceViewportOpenOutcome::Unavailable(reason) = outcome.outcome() {
            eprintln!("dock viewport unavailable: {reason:?}");
        }
    }
}
```

## Examples

Run the minimal single-window docking example first:

```sh
cargo run -p open-gpui-docking-minimal
```

This example uses `DockSurface::builder`, lazy panel factories, and `DockSurface::open_primary_window`. It enables in-window floating, but it does not enable platform viewport windows.

Run the facade-level multi-viewport example when checking native platform-window behavior:

```sh
cargo run -p open-gpui-docking-multiviewport
```

This example opts into platform viewport windows through `DockSurfaceBuilder::allow_platform_viewports(true)`, detaches a preview panel into a child dock space through `DockSurface::detach_panel_to_space`, opens that space through `DockSurface::viewports`, and handles unsupported backends through typed facade outcomes.

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
use open_gpui_docking::prelude::{DockPanelPlacement, DockSurface};

fn panel_factory(_cx: &mut App) -> AnyView {
    unreachable!("create and return a GPUI view for the panel")
}

let surface = DockSurface::builder("main")
    .panel_placements([
        DockPanelPlacement::left_rail("explorer").fraction(0.24),
        DockPanelPlacement::center("editor").selected(),
        DockPanelPlacement::bottom_rail("terminal").fraction(0.30),
    ])
    .panel_factory("explorer", "Explorer", panel_factory)
    .panel_factory("editor", "Editor", panel_factory)
    .panel_factory("terminal", "Terminal", panel_factory)
    .allow_floating(true)
    .build(cx)
    .expect("dock surface setup should validate");

let _ = surface;
```

Use `DockPanelPlacement::stacked_with(item, anchor)` to add tabs beside another panel without
depending on generated node ids. If an anchor may be missing during restore, attach a fallback such
as `.fallback(DockPanelPlacementTarget::right_rail())`.

## Product Panel Placement

Product code should describe where a panel belongs with `DockPanelPlacement` and
`DockPanelPlacementTarget`, not by storing generated graph node ids. The builder accepts
`panel_placements` for the initial layout, and panel descriptors may carry a default target through
`DockPanelDescriptor::with_default_placement`.

Close and reopen flows preserve placement as product facts. `DockPanelDescriptor::last_known_placement`
records the most recent close/open target, and `DockPanelOpenOutcome::placement_source` tells callers
whether a reopen used an explicit placement, last-known placement, descriptor default, or implicit
center fallback. This keeps lazy panel restore descriptor-driven without mounting views early.

Product commands should call `DockSurface::open_panel_at` for explicit destinations, `DockSurface::open_panel` for descriptor-backed restore, and `DockSurface::dock_panel_at` when moving an in-window floating panel back into the layout. Graph-targeted operations remain available through `open_gpui_docking::model`, but normal application restore flows should not persist tab or split node ids.

Use `DockSurface::detach_panel_to_space` for product flows that move an already-open panel into a child dock space before opening that space in a platform viewport. This keeps common panel-to-subwindow workflows on ids and facade outcomes instead of graph node ids or low-level drop targets.

Only call `allow_platform_viewports(true)` when the application intends to use platform-window docking routes and is prepared for `DockSurfaceViewportUnavailable::BackendUnsupported` on web or compositor backends without viewport-window support.

## Verification

For focused docking changes, run:

```sh
cargo fmt -p open-gpui-docking
cargo check -p open-gpui-docking --tests --locked
cargo check -p open-gpui-docking-minimal --locked
cargo check -p open-gpui-docking-multiviewport --locked
cargo check -p open-gpui-docking-native --locked
cargo nextest run -p open-gpui-docking host_viewport_platform_capability_tests --no-fail-fast
```

For render-authority, preview, splitter, motion, accessibility, and viewport work, use the narrower
or full docking gates in `docs/verification.md`.
