# Docking Architecture Audit - 2026-06-09

## Summary

The docking crate now matches ADR 0002's layering for the current product surface:

- `DockGraph` and `DockLayout` remain pure logical data; `DockOp` is crate-internal graph mutation
  machinery rather than public application API.
- `DockWorkspace` and `DockController` own durable commits through named public command methods;
  `DockAction` remains an explicit command-object surface, while rendered tab drag/drop and
  viewport tear-off commit through crate-internal move transactions.
- `DockHost` renders one logical `DockSpaceId` from a shared controller; render snapshots and
  transient interaction sessions live in focused helper modules.
- `drop_target`, `drop_runtime`, `workspace_transaction`, `interaction`, and `geometry` now form
  the interaction foundation: resolver, resolved-target session, commit transaction, pointer
  session, and split/drop math each have one authority.
- `DockViewportRuntime` is the internal testable platform-viewport lifecycle core, while
  `DockViewportRuntimeHandle` is the application-facing entry point for runtime-aware GPUI
  windows; `DockViewportAdapter` remains the internal lower-level mapping, coordinate, and
  placement primitive.
- `DockPanelRegistry` separates descriptor metadata from live GPUI view lifecycle state through
  `DockPanelCatalog`, `DockPanelViewStore`, and explicit attach APIs.
- Rendered product paths now cover item drag, whole-stack drag, host-level drop previews,
  cross-viewport dock-back through destination host scenes, runtime tear-off transactions, viewport
  activation after routed drops, and tab close policy from rendered chrome.

## Evidence

Host depth:

- `crates/gpui_docking/src/host.rs` keeps the controller entity, rendered space id,
  interaction runtime, and test debug state private.
- `DockHost` is controller-backed only; it stores the controller entity plus rendered space id and
  no longer carries an owned-workspace source path.
- `crates/gpui_docking/src/host_render_session.rs` snapshots read-only render facts before element
  construction.
- `crates/gpui_docking/src/host_render_actions.rs` is the render-callback commit entry point.
- `DockHost` centralizes controller mutation/notification in one private helper and exposes
  render-facing commit methods for select, drop, resize, and floating pointer sessions.
- Render modules build elements and collect pointer facts; they do not directly own workspace
  commit policy or viewport runtime mapping.

Interaction foundation:

- `crates/gpui_docking/src/drop_target.rs` resolves tab bars, leaves, root edges, floating title
  bars, empty dock spaces, known viewports, and tear-off candidates into one resolved target shape.
- `crates/gpui_docking/src/drop_runtime.rs` stores the active resolved target from layout facts,
  preserves tab-reorder stability during pointer movement, and feeds host-level preview rendering
  from the resolved target instead of a tab-only preview projection.
- `crates/gpui_docking/src/workspace_transaction.rs` commits resolved drop targets, so render code
  no longer constructs graph-shaped `MoveTab` commands for ordinary drag/drop.
- `crates/gpui_docking/src/workspace_move_transaction.rs`,
  `workspace_panel_transaction.rs`, `workspace_floating_transaction.rs`, and
  `workspace_resize_transaction.rs` keep graph op projection behind workspace transactions.
  Applications can use named `DockWorkspace`/`DockController` command methods for common flows,
  while public `DockAction` remains available for explicit command-object pipelines.
- `crates/gpui_docking/src/geometry.rs` is the split and drop geometry authority for render
  bounds, hit testing, resize fractions, and central-region remaining space allocation.
- `DockSplitLayout` is the shared split layout plan consumed by graph layout, rendered pane shares,
  visual handle placement, splitter hit testing, and resize session start state.
- `DockCentralRegion` is dock-space metadata, not a special graph node. It can stay alive while
  empty, expose passthrough semantics to render, and mark the root subtree used by central drop
  policy.
- Empty-space graph mutations rebind keep-alive central-region metadata when a new root is created.
  The same invariant is covered for programmatic reopen, item move, tabs-stack move, and floating
  subtree promotion, so a recovered central space does not silently degrade into ordinary root-only
  content.

Viewport productization:

- Runtime-opened windows install GPUI should-close hooks in
  `crates/gpui_docking/src/viewport_runtime.rs`.
- `DockViewportClosePolicy::Prevent` is applied before close cleanup through
  `DockViewportCloseGate` and `DockViewportShouldCloseOutcome`.
- `DockViewportRuntimeHandle` exposes application-level viewport open, close observation, runtime
  status, and placement APIs; rendered drop routing and pending tear-off begin/complete/expire
  hooks are kept as crate-internal runtime transaction seams.
- Target resolution ranks hovered window, active window, front-to-back window stack, then
  deterministic fallback in `crates/gpui_docking/src/viewport_target_resolver.rs`.
- Viewport hit testing and tear-off resolution require explicit crate-private
  `DockViewportTargetContext` arbitration input, even when callers choose an empty fallback
  context.
- Runtime-opened viewports publish host scenes through the handle path; known-viewport routes are
  resolved from the destination host scene before preview and again before commit. A viewport hit
  without a current host-scene target is treated as unavailable rather than as an accepted
  cross-window route.
- Tear-off pending state tracks item and tabs-stack payloads, duplicate requests, expiration,
  source-moved/source-missing cancellation, commit failure cleanup, and controller-backed viewport
  registration.
- Tear-off completion uses the same runtime-owned replacement cleanup as ordinary viewport open, so
  a window rebound between open and completion does not leave the superseded runtime window alive.
- Viewport close policy now covers retain, prevent, and merge-back behavior. Merge-back close moves
  closing viewport content into an explicit fallback dock space before unregistering the runtime
  mapping.
- `crates/gpui_docking/src/host_outside_release.rs` gives rendered drags a host-local polling
  fallback for release outside every GPUI window while preserving GPUI as the input authority.
- `Platform::mouse_button_is_pressed` is optional: macOS implements it with
  `NSEvent::pressedMouseButtons`, Windows implements it with `GetAsyncKeyState`, and unsupported
  platforms return `None`.
- `runtime_poll_released_left_button_tears_off_without_mouse_up_event` covers the product path
  where no GPUI mouse-up event is delivered but the platform reports the left button was released.
- Platform viewport routing assumes live window bounds are in a shared desktop coordinate space.
  macOS now preserves CoreGraphics display origins for `PlatformDisplay::bounds()` and live
  `PlatformWindow::bounds()`, while saved `WindowOptions` placement remains an application input
  rather than a live hit-test source.
- Linux X11 and Wayland update their stored hover state when platform enter/leave events fire, so
  `PlatformWindow::is_hovered()` matches the registered hover callbacks.

Panel lifecycle:

- `DockPanelCatalog` stores descriptor-only metadata for restore, policy, and tab chrome.
- `DockPanelViewStore` stores eager or lazy live view lifecycle handles.
- `DockPanelRegistry::has_view_lifecycle` makes live view presence separately queryable from
  metadata presence.
- `attach_view` and `attach_factory` bind restored metadata to live view state without rewriting
  titles or close policy.
- Descriptor metadata includes optional dock-class ids. `DockPolicy` owns the per-space allow-list,
  and workspace validation applies the same class policy to item, tabs-stack, floating-subtree,
  open, and empty-space commits.
- Live GPUI view resolution stays crate-private through the render snapshot path; public
  `DockPanelRegistration` exposes descriptor metadata only.
- Rendered tab close controls read closable metadata from the render snapshot, omit the affordance
  for non-closable panels, and still commit through panel lifecycle policy.

Native dogfood:

- `examples/docking-native` opens primary and secondary spaces through `DockViewportRuntimeHandle`
  and restores their window placement separately from `DockLayout`.
- The secondary viewport starts with a two-tab stack so manual dogfood can drag a whole stack back
  into the primary viewport.
- The primary viewport starts with an in-window floating stack and a non-closable pinned tab, so
  manual dogfood covers floating merge and rendered close-policy behavior.
- An additional empty central-region viewport opens with passthrough metadata, giving manual
  dogfood a retained central-space target without adding placeholder graph nodes.
- The native dogfood layout assigns explicit dock classes: primary panels can dock in the main
  space, secondary `Preview` / `Diff` panels can dock in the preview space, and the central dogfood
  space only accepts the central-note panel. Local previews and commit validation both reject
  incompatible class routes.
- The runtime status panel exposes close-policy switching, placement reapply, viewport reopen, and
  descriptor-backed panel restore controls so close/reopen paths can be exercised without code
  changes.
- Native example tests now assert the dogfood layout facts, viewport placement titles,
  descriptor-backed close/reopen controls, whole-stack float/merge behavior through the public
  `DockController` API, class-policy rejection for incompatible secondary stacks, central-note
  recovery into the empty central region, and rendered cross-window tab/stack drag through
  runtime-opened `DockHost` windows using GPUI test mouse events and public selector strings. This
  keeps the example from drifting while physical native-window drag dogfood remains the final proof
  for backend pixel and event delivery.
- `docs/verification.md` now carries the manual native-window dogfood checklist so the final
  physical verification run has a stable command and acceptance path.

Application surface authority:

- Every facade-created `DockSurface` clone shares one private owner entity for its controller,
  viewport runtime, monotonic revision, and activation state. Advanced low-level construction
  remains available through explicit modules without exposing that owner.
- Facade, host, and runtime root mutations allocate explicit private transaction identities.
  Nested controller and viewport commits carrying one identity coalesce into one typed metadata
  event; independent commands in one App turn remain independent revisions.
- Durable categories are layout, selection, panel lifecycle, viewport topology, and observed
  viewport placement. Rendering, style, focus requests, unchanged work, and platform mutation
  dispatch are not persistence facts.
- `DockSurface::export_snapshot` pairs layout and viewport placement with the current committed
  revision in one owner read. Applications subscribe, debounce, serialize, and store explicitly;
  Docking owns no timer, path, or file I/O.
- Stable-item activation uses one committed host generation per logical space and settles from
  exact descendant GPUI focus completion. Duplicate live hosts reject instead of silently
  replacing the incumbent, and stale request or host generations cannot retarget a later host.
- Dear ImGui remains a behavior oracle for one dock owner, request/commit ordering,
  selection-versus-focus separation, and effective host uniqueness. Its global immediate context,
  pointer requests, frame-liveness lifecycle, and automatic settings writer are intentionally not
  ported.

Test locality:

- `crates/gpui_docking/src/viewport_test_support.rs` keeps viewport mapping, placement, close, and
  target tests on one shared set of window/space/bounds fixtures.
- `crates/gpui_docking/src/graph_test_support.rs` keeps graph, layout, controller-builder, and
  fixture tests on one shared item/space/root-tabs fixture.
- The remaining local fixtures are intentionally domain-specific, such as geometry-only bounds or
  interaction-runtime bounds.

## Residual Backlog

- Legacy compatibility pressure around graph-based `DockHost` and `DockController` constructors,
  host-owned state accessors, the `DockHostSource` owned/controller split, and context-free
  viewport target helpers has been removed from the public docking API.
- The rendered release-outside path now has a platform button-state polling seam for macOS,
  Windows, and tests; Linux/Wayland and other unsupported backends intentionally return `None`
  until a reliable platform primitive is available.
- Full Dear ImGui PlatformIO parity is intentionally not claimed. DPI-scale conversion, monitor
  work-area scale, live programmatic window move, input passthrough, no-focus-on-appearing, alpha,
  topmost/no-taskbar flags, and reliable Wayland global window position need a future GPUI platform
  capability design.
- Windows uses per-monitor logical bounds that can diverge across mixed-DPI displays, and Wayland
  does not expose compositor global toplevel positions. Docking runtime tests should keep using
  live host-scene facts and should not treat saved placement snapshots as a global routing source.
- macOS build, native-launch smoke, and TestApp-level rendered cross-window drag have been verified
  for `examples/docking-native`. The repository's Windows workflow already checks the verification
  gate, `xtask`, `open-gpui-windows --all-features`, and the shared WGPU font-kit path on a
  Windows runner; `xtask verify` also compile-checks `examples/docking-native` through
  `cargo check --workspace`. However, the current local `x86_64-pc-windows-msvc` cross-check is
  still blocked by the absence of the MSVC `lib.exe` toolchain, and the newly added
  docking-native rendered test step still needs a remote Windows result for this branch. Physical
  native-window dogfood remains manual proof beyond CI.
- Add richer product behavior through the existing seams: route-preview polish, focus restoration,
  accessibility behavior, and broader backend coverage for outside-window release polling.
- Continue splitting future viewport or graph code only when the extracted module passes the
  deletion test and gives callers a smaller, deeper interface.
- Revisit whether `host_test_support` should share a smaller ID fixture with graph and viewport
  support after the next host/render test cleanup.
