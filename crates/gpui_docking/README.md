# Open GPUI Docking

`open-gpui-docking` contains a facade-first retained docking system for Open GPUI applications. It separates durable layout state from GPUI runtime state so applications can persist dock spaces while keeping panel views, platform windows, drag sessions, and visual affordances behind runtime adapters.

Use this crate when an application needs IDE-style tab stacks, split panes, floating panels, and
optional platform viewport windows.

## Public API Tiers

The crate root and `open_gpui_docking::prelude` contain the common application API: `DockSurface`, `DockSurfaceBuilder`, durable layout and placement data, panel registry/catalog types, policy types, and typed facade outcomes. The root intentionally does not re-export raw graph, action, host, workspace, runtime-handle, or raw layout-node types.

Low-level controller, graph, action, workspace, host, raw layout parts, and runtime-handle APIs live behind explicit modules. Use `open_gpui_docking::model` for `DockController`, `DockControllerBuilder`, graph/layout mutation tools, command objects, and `layout_from_raw_parts`/`layout_into_raw_parts`; use `open_gpui_docking::runtime` for `DockHost`, `DockHostOptions`, and direct viewport runtime integrations; use `open_gpui_docking::advanced` for diagnostics and transition internals. These types are useful for tests and tooling, but they are not part of the default application surface.

## What This Crate Owns

- `DockSurface` as the app-level owner for controller state, host-window creation, panel commands,
  one monotonic committed revision stream, stable-item activation, and typed viewport capability
  outcomes.
- `DockSurfaceSnapshot` as the revision-consistent app-level persistence payload that combines
  durable layout with facade-opened viewport placement hints.
- `DockSurfaceViewports`, `DockSurfaceViewportSpec`, `DockSurfaceViewportReadinessReport`,
  `DockSurfaceViewportOpenReport`, and `DockSurfaceViewportRestoreReport` as facade-level platform
  window lifecycle, requests, capability checks, and batch outcomes for multi-viewport applications.
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
- `DockVisualStyle` as the complete immutable paint authority and `DockVisualStyleResolver` as the
  read-only per-surface or explicit-host adapter for application-owned window and subtree themes.
  `DockDropGuideMetrics` remains structural and does not carry colors.
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

`DockSurface::viewports` returns a `DockSurfaceViewports` facade for the common multi-window path. First open the surface anchor through `DockSurface::open_primary_window` and match its typed `DockSurfacePrimaryWindowOpenOutcome`; managed viewport readiness and open requests return `SessionInactive` until that exact session generation is active. The facade can then check `readiness`, `readiness_many`, or `restore_readiness`, open detached dock spaces, restore saved placement data, export placement snapshots, and handle GPUI close hooks while keeping applications away from raw runtime handles. Readiness and open outcomes distinguish inactive sessions, policy-disabled, backend-unsupported, unsupported requested platform flags, invalid placement, and backend-open failures without parsing opaque errors. Unsupported backends should no-op for open or tear-off requests instead of constructing partial runtime state. Web and other backends without platform window support stay on the single-window route.

Every surface owns an independent window session with a monotonic generation and exact primary anchor. Closing that anchor freezes new managed work, retires dependent viewports before the anchor, and reaches `Closed` only after its runtime registry and terminal window tickets converge. Close dispatch and logical GPUI removal are not native terminal facts: ordinary teardown waits for the platform's exact `Closed` callbacks, while App shutdown may confirm absence only after the window registry has been cleared. Docking never calls `App::quit`; GPUI's application quit policy remains authoritative. `DockSurface::window_session_status` exposes phase, generation, anchor, terminal reason, ticket counts, and runtime convergence for diagnostics. Embedded `host_view` content renders without creating an anchor or registering managed route and activation authority, so facade activation reports `Unavailable` for that embedded-only host. Applications that intentionally own a custom window lifetime use the explicit low-level runtime and host APIs instead.

For an already-open detached viewport, Dock projects GPUI's property-specific window-mutation
capabilities from that viewport window's actual immutable kind and target-display profile. Its
diagnostics retain one profile per registered space/window and record immediate `dispatches`
separately from terminal `observations`: queued dispatch is intent only, while each observation
retains the typed request and committed facts that settled it. Route geometry, exported placement,
and `DockSurface` observed-placement revisions consume committed `WindowPlatformFacts`.
Applications should not infer success from a Dock sync attempt; inspect the terminal observation
when a live window mutation matters. Dock suppresses per-frame retries after a terminal failure
until the requested target or relevant committed facts change.

Use `DockSurface::export_snapshot` for the common persistence path. The snapshot stores `DockLayout` for logical dock spaces plus `DockViewportPlacementLayout` for platform-window hints, without storing GPUI views or platform window handles. Applications that need custom storage can still persist `DockLayout` and viewport placement separately; `DockSurfaceViewportSpec::with_saved_placement` applies placement hints to fallback GPUI window options before a viewport opens. Applications can call `DockSurface::export_viewport_placement` to snapshot only facade-opened platform windows and `DockSurface::check_viewport_placement_restore` to validate saved placement before reopening windows, without importing `DockViewportRuntimeHandle`.

Use `DockSurfaceBuilder::close_policy` or `DockSurface::set_viewport_close_policy` to choose how detached platform windows close. `DockViewportClosePolicy::RetainLayout` removes only the runtime window mapping, `Prevent` vetoes the platform close, and `MergeBack` moves a closing viewport's dock content into a fallback space. Applications with custom GPUI window hooks can call `DockSurface::handle_viewport_window_should_close`, `DockSurface::handle_viewport_window_closed`, and `DockSurface::cancel_viewport_window_close` without importing the low-level runtime handle.

## Change Events, Persistence, And Activation

Every `DockSurface` clone points to one private owner and observes the same monotonic revision.
`subscribe_changes` publishes metadata only after a durable root operation commits. An event
contains the revision and stable categories for layout, selection, panel lifecycle, viewport
topology, and observed viewport placement. Rendering, focus intent, visual style changes, rejected
or unchanged commands, and queued platform mutation requests do not create persistence revisions.

Applications own debounce and storage. Keep the subscription alive, coalesce events according to
product policy, then call `export_snapshot`. The returned `DockSurfaceSnapshot::revision()` is
paired with both its layout and viewport-placement facts; Docking does not allocate a snapshot for
each event, start a timer, or write files.

Use `select_panel` when only tab selection is intended. Use stable-item `activate_panel` or
`activate_panel_with_completion` when the application also needs window activation and descendant
GPUI focus. The completion callback is `FnOnce(outcome, &mut App)` and runs after the owner state
has been released, so it may safely issue a follow-up activation. Completion reports `Committed`,
`Rejected`, `Superseded`, `Unavailable`, `DuplicateHostConflict`, or `WindowClosed`. Dropping the
completion subscription stops observation without cancelling the issued intent. Generated graph
node ids are not a product focus API.

## Restoring Platform Viewports

Use `DockSurfaceSnapshot` with `DockSurface::builder(...).try_snapshot(...)` and `DockSurface::viewports().restore_snapshot(...)` for the common restore path. This keeps layout validation, placement validation, backend capability checks, and batch outcome reporting on the facade surface:

```rust
use open_gpui::{Bounds, WindowBounds, WindowOptions, px, size};
use open_gpui_docking::prelude::{
    DockSurface, DockSurfacePrimaryWindowOpenOutcome, DockSurfaceSnapshot,
    DockSurfaceViewportOpenOutcome,
};

fn panel_factory(_cx: &mut open_gpui::App) -> open_gpui::AnyView {
    unreachable!("create and return a GPUI view for the panel")
}

fn restore_surface(
    saved: &DockSurfaceSnapshot,
    cx: &mut open_gpui::App,
) -> Result<DockSurface, open_gpui_docking::prelude::DockLayoutValidationError> {
    let surface = DockSurface::builder("main")
        .try_snapshot(saved)?
        .panel_factory("editor", "Editor", panel_factory)
        .panel_factory("preview", "Preview", panel_factory)
        .allow_platform_viewports(true)
        .build(cx)
        .expect("registered panels should satisfy the restored layout");

    let primary = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(960.0), px(640.0)),
            cx,
        ))),
        ..Default::default()
    };
    let DockSurfacePrimaryWindowOpenOutcome::Opened(_) =
        surface.open_primary_window(primary, cx)
    else {
        panic!("the managed Dock surface anchor must open before restoring viewports");
    };

    let viewports = surface.viewports();
    let report = viewports.restore_snapshot(
        saved,
        |_| {
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(720.0), px(480.0)),
                    cx,
                ))),
                ..Default::default()
            }
        },
        cx,
    );

    for outcome in report.outcomes() {
        if let DockSurfaceViewportOpenOutcome::Unavailable(reason) = outcome.outcome() {
            eprintln!("dock viewport unavailable: {reason:?}");
        }
    }

    Ok(surface)
}
```

## Examples

Run the minimal single-window docking example first:

```sh
cargo run -p open-gpui-docking-minimal
```

This example uses `DockSurface::builder`, lazy panel factories, and `DockSurface::open_primary_window`. It enables in-window floating, but it does not enable platform viewport windows.
It deliberately uses Docking's deterministic built-in visual fallback.

Run the facade-level multi-viewport example when checking native platform-window behavior:

```sh
cargo run -p open-gpui-docking-multiviewport
```

This example opens two independent managed surface anchors. The primary surface opts into platform viewport windows through `DockSurfaceBuilder::allow_platform_viewports(true)`, detaches preview panels into a child dock space, and opens that space through `DockSurface::viewports`; the isolated surface remains an independent primary. It demonstrates that identical logical space ids do not merge session generations or window ownership across surfaces, while retaining typed unsupported-backend outcomes, committed revision logging, application-owned debounced snapshot export, and stable-item activation completion.

Run the normal-checkout native dogfood example when working on viewport runtime behavior or
diagnostics:

```sh
cargo run -p open-gpui-docking-native
```

The dogfood example demonstrates a controller-backed host with registered panels, tab stacks, split
layout, floating behavior, capability-gated platform viewport windows, and the runtime diagnostic
paths used by the docking tests. Its application-side adapter maps UI Components light, dark, and
high-contrast theme snapshots into complete Dock styles. Deterministic tests prove that the source
drag visual keeps its opening-generation style while target guides use the destination host's
current style. Real captured cross-window transport and pre-release native visibility require the
owning-platform evidence described in the verification plan.

## Visual Style

Applications may use the built-in fallback, install a fixed style, or resolve a complete style from
their own theme authority:

```rust
use open_gpui_docking::{
    DockSurface, DockVisualPalette, DockVisualStyle, DockVisualStyleResolver,
};

let resolver = DockVisualStyleResolver::new(|window, cx| {
    let palette = application_dock_palette(window, cx);
    DockVisualStyle::from_palette(palette)
});

let surface = DockSurface::builder("main")
    .visual_style_resolver(resolver)
    .build(cx)?;
```

The resolver callback receives read-only `Window` and `App` references. Prepare mutable theme or
registry state before rendering; the callback cannot update entities, notify, dispatch, register,
refresh, or reenter Dock style resolution. A multi-window low-level integration installs the same
resolver through `DockViewportRuntimeHandle::with_visual_style_resolver`; a single explicit host can
use `DockHost::from_controller_with_visual_style_resolver`.

The style does not belong in `DockDragPayload`. Runtime metadata freezes the source drag visual for
one opening generation, while destination guides and previews resolve their target host live.
Cancellation and close retire that snapshot before reopen. The crate has no production dependency
on UI Components; the native example is the reference application-owned theme adapter.

Dear ImGui informs docking interaction behavior, including tab states, inner and outer targets,
accepted/rejected previews, tear-off, one effective host per node, and commit-before-settings
ordering. Open GPUI retains its own `DockGraph`, n-ary splits, explicit transactions, viewport and
activation generations, typed focus completion, and application-owned persistence. It does not
copy ImGui's default colors, immediate Dock context, pointer identities, binary node tree, builder
API, frame-liveness inference, or settings format.

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
cargo run -p xtask -- scan-ui-contract
```

For render-authority, preview, splitter, motion, accessibility, and viewport work, use the narrower
or full docking gates in `docs/verification.md`.
