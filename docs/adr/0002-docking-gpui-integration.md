# ADR 0002: Docking GPUI Integration

**Status**: Accepted
**Date**: 2026-06-08

## Context

Open GPUI now has an optional `open-gpui-docking` crate with a pure dock graph, static host
rendering, panel registration, layout import/export, and a native smoke example. The next docking
work will add tab activation, drag/drop, splitter resizing, in-window floating chrome, and later
OS-level detach.

GPUI already has authoritative modules for platform windows, input, focus, and retained view
lifecycle:

- `App` owns platform window creation, active window lookup, and window stack queries.
- `Window` owns the draw surface, event routing, hitboxes, pointer capture, frame scheduling, and
  bounds.
- `FocusHandle` and `Focusable` own focus semantics inside a window.
- `Entity`, `AnyView`, and `Render` own retained view state and rendering.
- `WindowOptions::tabbing_identifier` is native window tabbing, not editor-style docking tabs.

Docking needs to use these modules without becoming a second GPUI runtime or a second window
manager.

## Decision

Docking will model logical dock layout separately from GPUI platform windows.

`DockGraph` remains pure data. It stores dock spaces, nodes, tab stacks, split fractions, in-window
floating bounds, and stable `DockItemId` values. It must not store `WindowHandle`, `WindowId`,
`Entity`, `AnyView`, `FocusHandle`, or platform-window state.

`DockPanelRegistry` remains the adapter from `DockItemId` to GPUI panel metadata and retained view
content. A graph can be serialized and tested without this registry.

`DockHost` is a GPUI render adapter for one logical dock space in one GPUI window. Long-term docking
state coordination belongs behind a deeper owner module in `open-gpui-docking`, not in the render
adapter.

GPUI `App` and `Window` remain the only modules that create and manage platform windows. Future
OS-level detach work must use a separate adapter that maps `DockSpaceId` values to GPUI
`WindowHandle` values through `App::open_window`; that mapping must stay outside `DockGraph`.

GPUI focus remains authoritative. Dock active-tab state is layout selection, not a second focus
system. Docking interactions may ask GPUI to focus a rendered view, but they must not maintain a
parallel focus table.

Docking tabs and native window tabs remain separate concepts. `DockNode::Tabs` is an editor-style
dock tab stack. `WindowOptions::tabbing_identifier` and platform tab controllers are native
window-tabbing behavior.

In-window floating containers and platform floating windows remain separate concepts. A
`DockNode::Floating` is layout data inside a dock host. A platform floating window is created by
GPUI platform-window machinery.

Follow-up implementation note, 2026-06-09: rendered docking interactions now pass through
crate-internal interaction and transaction modules before mutating the graph. Render callbacks
collect pointer facts, the drop resolver produces a resolved target, the workspace transaction
validates and commits that target, and viewport tear-off uses `DockViewportRuntime` to coordinate
window creation before graph mutation. The active drop session stores a resolved target from layout
facts rather than a tab-only intent, so preview and commit stay tied to the same resolver output.
Splitter and floating pointer sessions emit crate-private resize/bounds requests that the host
commits through controller/workspace transactions, rather than constructing public `DockAction`
values from render callbacks.
`DockAction` remains the public programmatic interface for explicit non-move commands such as
selection, panel close/reopen, floating, and split resize. `DockOp` is crate-internal graph
mutation machinery, so render code and applications do not need to understand source/target node
ids, zones, and insertion indexes to commit ordinary drag/drop.

## Architecture

```mermaid
flowchart TB
    App[GPUI App] --> Window[GPUI Window]
    Window --> Host[DockHost render adapter]
    Host --> Owner[Dock owner module]
    Owner --> Graph[DockGraph]
    Owner --> Registry[DockPanelRegistry]
    Registry --> View[GPUI Entity / AnyView]
    Focus[GPUI FocusHandle] --> Window
    Graph -.stores item ids only.-> Registry
    Owner -.does not own focus.-> Focus
```

Future platform detach is an adapter, not graph state:

```mermaid
flowchart LR
    Owner[Dock owner module] --> Space[DockSpaceId]
    DetachAdapter[Future detach adapter] --> AppOpen[App::open_window]
    DetachAdapter --> WindowMap[DockSpaceId to WindowHandle map]
    Space -.serializable.-> Graph[DockGraph]
```

## Alternatives Considered

### Option A: Keep docking independent and adapt to GPUI windows

Pros:

- Keeps `DockGraph` serializable and testable.
- Preserves GPUI's existing platform-window and focus modules.
- Allows single-window docking to ship before OS-level detach.
- Keeps `open-gpui-docking` optional.

Cons:

- Requires a docking owner module before interaction work can stay clean.
- Future OS-level detach needs an adapter and additional tests.

Decision: chosen.

### Option B: Store GPUI window handles in `DockGraph`

Pros:

- Looks direct for OS-level detach.

Cons:

- Makes graph serialization platform-dependent.
- Couples layout mutation to GPUI window lifecycle.
- Makes pure graph tests unable to cover the authoritative layout model.

Decision: rejected.

### Option C: Move docking into `crates/gpui`

Pros:

- Gives direct access to GPUI internals.

Cons:

- Makes docking part of the core framework even when applications do not need it.
- Increases public surface before interaction semantics are stable.
- Conflicts with the current optional crate strategy.

Decision: rejected unless a future missing primitive is proven and documented separately.

### Option D: Reuse native window tabbing for docking tabs

Pros:

- Reuses platform behavior on macOS.

Cons:

- Native window tabs and editor-style dock tabs have different semantics.
- It would not work consistently across all GPUI targets.
- It would tie ordinary panel tab stacks to platform-window grouping.

Decision: rejected.

## Consequences

- The next docking refactor should introduce a deep owner module and narrow `DockHost` into a render
  adapter.
- Tab activation, drag/drop, splitter resize, and in-window floating should enter through docking
  actions or intents applied by that owner module.
- OS-level detach remains deferred until a separate platform-window adapter is planned.
- Tests should separate graph layout state, owner coordination, GPUI rendering, and future platform
  window routing.
