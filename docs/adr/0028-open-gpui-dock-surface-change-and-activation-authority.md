# ADR 0028: Open GPUI Dock Surface Change And Activation Authority

**Status**: Accepted
**Date**: 2026-07-25

## Context

`DockSurface` was a convenient facade over a controller and viewport runtime, but the pair had no
single application-level commit authority. Applications could observe generic entity
notifications or compare snapshots, yet neither technique distinguishes a committed layout change
from rendering, focus intent, platform mutation dispatch, or an unchanged command. End-of-App-turn
batching also cannot tell one multi-step tear-off from two independent commands issued in the same
turn.

Panel focus had a similar ownership gap. Selection by stable item id and node-id focus commands
were separate operations, while multiple retained hosts could transiently claim the same logical
space. A stale callback could therefore target a replacement host, and applications could not
observe whether descendant GPUI focus actually committed.

Dear ImGui remains useful evidence for the behavior of a mature docking system: one dock context
owns node changes, selection and visibility are distinct from navigation focus, viewport requests
are processed before settings are persisted, and one node has one effective host. Its global
immediate-mode context, pointer-addressed requests, frame-liveness inference, synchronous
fire-and-forget focus, and automatic `.ini` timer do not match Open GPUI's retained entities,
stable product ids, or application-owned persistence.

## Decision

Every facade-created `DockSurface` clone references one private `DockSurfaceOwner` entity. The
owner holds the controller, viewport runtime, primary space, monotonic committed revision,
activation state, and the private window-session authority described by
[ADR 0030](0030-open-gpui-dock-surface-window-session-authority.md). The owner type, transaction
identity, and exact-generation window leases are not public APIs.

Each facade, host, or runtime root mutation allocates a private `DockSurfaceTransactionId`.
Synchronous nested work carries that identity through controller and viewport commit paths.
Committed categories within the transaction are deduplicated and published once in stable order:
layout, selection, panel lifecycle, viewport topology, and observed viewport placement. Independent
root commands never coalesce merely because they share an App turn. An asynchronous platform
observation starts a new root transaction.

Only durable committed facts advance the revision. Failed, rejected, superseded, unchanged,
focus-only, style-only, and dispatch-only work does not publish a persistence event. In
particular, successfully queueing a native viewport mutation is not evidence that the platform
accepted or observed it.

`DockSurfaceChangeEvent` contains only the revision and bounded change categories. Applications
subscribe with `DockSurface::subscribe_changes`, choose their own debounce policy, then call
`DockSurface::export_snapshot`. Export reads layout, viewport placement, and the associated
revision from one owner turn. Docking does not start a timer, select a filesystem path, write a
file, or include a full snapshot in every event. Legacy snapshots without a revision deserialize
with revision zero.

Each logical space has one committed activation-host generation. The first live host owns the
space until release; duplicate live registration is a typed conflict and cannot silently replace
the incumbent. Window close, host migration, or release retires that generation, and every
activation callback validates both request and host generations.

Applications activate by stable `DockItemId` through `DockSurface::activate_panel` or
`activate_panel_with_completion`. Selection remains a separate durable commit. The activation
request settles exactly once from descendant GPUI focus completion as `Committed`, `Rejected`,
`Superseded`, `Unavailable`, `DuplicateHostConflict`, or `WindowClosed`. Dropping the returned
subscription stops callback delivery but does not cancel the issued intent. Node-id
`DockHost::focus_pane` is crate-private.

## Consequences

- Applications can persist from one monotonic, metadata-only commit stream without inferring
  changes from rendering or generic notifications.
- One logical operation such as merge-back publishes one revision even when it changes layout,
  selection, panel lifecycle, and viewport topology together.
- Snapshot storage cadence, serialization destination, retry, and I/O failure handling remain
  application policy.
- Selection may commit even when focus later rejects; persistence events and activation outcomes
  report those facts independently.
- Stable item activation works through hosts nested below arbitrary window roots and cannot be
  retargeted by a stale equal-item callback.
- Low-level controller and runtime APIs remain available through explicit modules, but they do not
  create a second public surface owner.
- Window-session shutdown and revision commits remain separate: close intent and terminal ticket
  settlement are not durable layout changes, while one authoritative runtime cleanup may publish
  at most one normal committed change.

## Rejected Alternatives

- Generic `notify` or snapshot comparison cannot prove which durable domain committed.
- End-of-turn batching merges unrelated commands and splits asynchronous observations from their
  actual authority.
- Publishing a full snapshot per event forces allocation and persistence cadence on every consumer.
- A built-in debounce timer or file writer would make Docking own application storage policy.
- Silent duplicate-host replacement makes focus depend on render order.
- Treating selection, platform activation dispatch, or a focus request as terminal success would
  report state that GPUI or the platform may later reject.
- Porting ImGui's global context, pointer identities, frame-liveness rules, or `.ini` writer would
  replace Open GPUI's retained ownership model rather than learning from its interaction behavior.
