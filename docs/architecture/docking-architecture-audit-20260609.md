# Docking Architecture Audit - 2026-06-09

## Summary

The docking crate now matches ADR 0002's layering for the current product surface:

- `DockGraph` and `DockLayout` remain pure logical data; `DockOp` is crate-internal graph mutation
  machinery rather than public application API.
- `DockWorkspace` and `DockController` own durable commits through public `DockAction` for explicit
  non-move programmatic commands, while rendered tab drag/drop and viewport tear-off commits through
  crate-internal move transactions.
- `DockHost` renders one logical `DockSpaceId` from a shared controller; render snapshots and
  transient interaction sessions live in focused helper modules.
- `drop_target`, `drop_runtime`, `workspace_transaction`, `interaction`, and `geometry` now form
  the interaction foundation: resolver, resolved-target session, commit transaction, pointer
  session, and split/drop math each have one authority.
- `DockViewportRuntime` is the testable platform-viewport lifecycle core, while
  `DockViewportRuntimeHandle` is the application-facing entry point for runtime-aware GPUI
  windows; `DockViewportAdapter` remains the lower-level mapping, coordinate, and placement
  primitive.
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
  `workspace_resize_transaction.rs` keep graph op projection behind workspace transactions while
  public `DockAction` remains an explicit application command surface.
- `crates/gpui_docking/src/geometry.rs` is the split and drop geometry authority for render
  bounds, hit testing, resize fractions, and central-region remaining space allocation.
- `DockSplitLayout` is the shared split layout plan consumed by graph layout, rendered pane shares,
  visual handle placement, splitter hit testing, and resize session start state.
- `DockCentralRegion` is dock-space metadata, not a special graph node. It can stay alive while
  empty, expose passthrough semantics to render, and mark the root subtree used by central drop
  policy.

Viewport productization:

- Runtime-opened windows install GPUI should-close hooks in
  `crates/gpui_docking/src/viewport_runtime.rs`.
- `DockViewportClosePolicy::Prevent` is applied before close cleanup through
  `DockViewportCloseGate` and `DockViewportShouldCloseOutcome`.
- `DockViewportRuntimeHandle` exposes application-level viewport open, tear-off open, close
  observation, and placement APIs; pending tear-off begin/complete/expire hooks are kept as
  crate-internal runtime transaction seams.
- Target resolution ranks hovered window, active window, front-to-back window stack, then
  deterministic fallback in `crates/gpui_docking/src/viewport_target_resolver.rs`.
- Viewport hit testing and tear-off resolution require explicit `DockViewportTargetContext`
  arbitration input, even when callers choose an empty fallback context.
- Runtime-opened viewports publish host scenes through the handle path; known-viewport routes are
  resolved again in the destination host scene before commit, and successful routed drops activate
  the destination viewport.
- Tear-off pending state tracks item and tabs-stack payloads, duplicate requests, expiration,
  source-moved/source-missing cancellation, commit failure cleanup, and controller-backed viewport
  registration.
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

Panel lifecycle:

- `DockPanelCatalog` stores descriptor-only metadata for restore, policy, and tab chrome.
- `DockPanelViewStore` stores eager or lazy live view lifecycle handles.
- `DockPanelRegistry::has_view_lifecycle` makes live view presence separately queryable from
  metadata presence.
- `attach_view` and `attach_factory` bind restored metadata to live view state without rewriting
  titles or close policy.
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
- The runtime status panel exposes close-policy switching, placement reapply, viewport reopen, and
  descriptor-backed panel restore controls so close/reopen paths can be exercised without code
  changes.

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
- Add richer product behavior through the existing seams: route-preview polish, focus restoration,
  accessibility behavior, and broader backend coverage for outside-window release polling.
- Continue splitting future viewport or graph code only when the extracted module passes the
  deletion test and gives callers a smaller, deeper interface.
- Revisit whether `host_test_support` should share a smaller ID fixture with graph and viewport
  support after the next host/render test cleanup.
