# Docking Architecture Audit - 2026-06-09

## Summary

The docking crate now matches ADR 0002's layering for the current product surface:

- `DockGraph`, `DockOp`, and `DockLayout` remain pure logical data.
- `DockWorkspace` and `DockController` own durable commits through `DockAction`.
- `DockHost` renders one logical `DockSpaceId` from a shared controller; render snapshots and
  transient interaction sessions live in focused helper modules.
- `DockViewportRuntime` and `DockViewportRuntimeHandle` are the product path for GPUI platform
  windows; `DockViewportAdapter` remains the lower-level mapping, coordinate, and placement
  primitive.
- `DockPanelRegistry` separates descriptor metadata from live GPUI view lifecycle state through
  `DockPanelCatalog`, `DockPanelViewStore`, and explicit attach APIs.

## Evidence

Host depth:

- `crates/gpui_docking/src/host.rs` keeps the controller entity, rendered space id,
  interaction runtime, and test debug state private.
- `DockHost` is controller-backed only; it stores the controller entity plus rendered space id and
  no longer carries an owned-workspace source path.
- `crates/gpui_docking/src/host_render_session.rs` snapshots read-only render facts before element
  construction.
- `crates/gpui_docking/src/host_render_actions.rs` is the render-callback commit entry point.
- Render modules build elements and collect pointer facts; they do not directly own workspace
  commit policy or viewport runtime mapping.

Viewport productization:

- Runtime-opened windows install GPUI should-close hooks in
  `crates/gpui_docking/src/viewport_runtime.rs`.
- `DockViewportClosePolicy::Prevent` is applied before close cleanup through
  `DockViewportCloseGate` and `DockViewportShouldCloseOutcome`.
- Target resolution ranks hovered window, active window, front-to-back window stack, then
  deterministic fallback in `crates/gpui_docking/src/viewport_target_resolver.rs`.
- Viewport hit testing and tear-off resolution require explicit `DockViewportTargetContext`
  arbitration input, even when callers choose an empty fallback context.

Panel lifecycle:

- `DockPanelCatalog` stores descriptor-only metadata for restore, policy, and tab chrome.
- `DockPanelViewStore` stores eager or lazy live view lifecycle handles.
- `DockPanelRegistry::has_view_lifecycle` makes live view presence separately queryable from
  metadata presence.
- `attach_view` and `attach_factory` bind restored metadata to live view state without rewriting
  titles or close policy.
- Live GPUI view resolution stays crate-private through the render snapshot path; public
  `DockPanelRegistration` exposes descriptor metadata only.

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
- Add richer product behavior through the existing seams: tab reorder, whole-stack drag, viewport
  release polish, focus restoration, and accessibility behavior.
- Continue splitting future viewport or graph code only when the extracted module passes the
  deletion test and gives callers a smaller, deeper interface.
- Revisit whether `host_test_support` should share a smaller ID fixture with graph and viewport
  support after the next host/render test cleanup.
