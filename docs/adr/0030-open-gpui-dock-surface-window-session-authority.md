# ADR 0030: Open GPUI Dock Surface Window Session Authority

**Status**: Accepted
**Date**: 2026-07-28

## Context

`DockSurface` previously exposed a stateless viewport facade over a retained viewport runtime. The
runtime knew which detached windows were registered, but no authority owned the lifetime of the
native window group. Examples opened a low-level runtime viewport as a de facto primary and called
`App::quit` from its close observer. Dependent windows could therefore survive the primary, one
surface could not be isolated from another by an exact generation, and application exit was used
to hide missing teardown semantics.

Native window creation and destruction are reentrant. A platform close can arrive while a window
is being created, mapped, built, initially drawn, or presented. Close observers can also run in
either child/anchor order, while application shutdown clears the GPUI registry without promising
an ordinary close callback for every window. A boolean active flag or a set of `WindowId` values
cannot distinguish a delayed prior-generation callback from current work, and mirroring runtime
handles in the facade owner would create competing ownership authorities.

Dear ImGui's docking branch is useful behavioral evidence: floating viewports form one platform
window group, logical close is separated from native destruction, renderer resources retire before
the platform window, and viewport creation/teardown is centrally coordinated. Its global immediate
context, raw pointers, frame-liveness garbage collection, and platform callback ownership do not
fit Open GPUI's retained entities and typed application transactions.

## Decision

Each facade-created `DockSurface` owns one private window-session authority with the lifecycle
`Vacant -> Opening -> Active -> ShuttingDown -> Closed`. It stores a monotonic generation, an exact
opening token or committed anchor, a rollback or shutdown reason, and a terminal ticket snapshot.
`DockSurfaceWindowSessionStatus` is the read-only public projection; admission tokens and leases
remain crate-private.

`DockSurface::open_primary_window` reserves `Opening` before calling GPUI's typed synchronous
window-open boundary. GPUI distinguishes platform create/map failure, native close during
create/map, close during root construction, close during initial draw, close during hidden initial
presentation, before-visibility presentation rejection, and registry commit rejection. Only a
fully committed `WindowId` can consume the opening token and activate the exact generation. A
pre-commit failure rolls the token back to `Closed`; a later presentation failure starts ordinary
forced shutdown and never revives the consumed token.

Every facade-managed viewport open/restore, ownership record, scene publication, activation,
drag/route reservation, platform mutation, and terminal observation carries the exact opaque
session lineage. The runtime remains the only owner of committed, opening, tear-off, and
provisional window handles. The surface owner asks it for a deduplicated teardown snapshot; it does
not mirror those handles. Unmanaged low-level runtimes carry an explicit unmanaged lineage rather
than fabricating a surface lease.

The first ordinary close request for the exact active anchor freezes new work and returns `false`
to keep the anchor alive while dependents drain. Shutdown cancels generation-bound drag,
activation, mutation, opening, tear-off, and provisional work, bypasses per-viewport `Prevent` and
`MergeBack` policy, dispatches dependent closes outside all owner/runtime/entity borrows, and
explicitly removes the still-live anchor last. Dispatching an exact close is an idempotent intent;
neither a failed handle update nor logical GPUI registry removal proves native destruction. The
ticket remains pending until the platform publishes the exact generation's `Closed` event. Only
the post-registry-clear App-shutdown path may confirm that a native window is absent. `Closed`
requires the exact anchor to be terminal or confirmed absent, every snapshotted terminal ticket to
settle, and the current-generation runtime registry to be empty. Retired runtime ownership remains
present until that same terminal fact settles it.

Direct native anchor destruction enters the same convergence path and freezes dependent work
regardless of logical-close observer order. App shutdown has an explicit pre-clear freeze/snapshot
path and a post-clear confirmed-absent path; it does not wait for close observers that registry
clearing does not emit. Reopen remains rejected while even one exact native terminal is delayed.
Delayed callbacks validate their old lineage and cannot affect a reopened generation. Managed
viewports are peer top-level windows by default. Dock never derives native `transient_for`
ownership from the surface anchor or the window that requested an open; callers may opt into an
explicit presentation/grouping owner when their product semantics require one, and that hint is
not the lifetime authority.

GPUI prepares each native pointer-capture release once, before post-borrow dispatch can be delayed,
and every retry reuses the platform pointer-session identity captured by that prepared operation.
A stale release is therefore an idempotent no-op against a newer pointer session. During App
shutdown, logical window-registry retirement is allowed to start before the capture-release fence
settles; the exact native-window terminal is the final capture-release fact when the platform keeps
rejecting an explicit release. Native window retirement retains the platform window owner across a
failed destroy request and retries it, so registry removal never drops the only owner of a live
native window.

`DockSurfaceViewports` replaces the misleading `DockSurfaceViewportSession` name without an alias.
Managed readiness/open results expose `SessionInactive`; primary opening returns typed opened,
conflict, rollback, and not-yet-closed outcomes. DevTools projects session phase, generation,
anchor, reason, terminal counts, runtime convergence, and the owned low-level runtime as one
surface target tree.

Docking never calls `App::quit`. Closing one surface affects only windows carrying that exact
surface lease. GPUI's last-window or explicit quit policy remains the application-exit authority.
An embedded `host_view` renders without creating an anchor or registering managed route and
activation authority. Custom application-owned lifetimes use the explicit low-level runtime and
host APIs rather than borrowing a facade session.

## Consequences

- An ordinary guarded primary close deterministically receives every dependent native terminal
  before dispatching the anchor close; direct native destruction still freezes and drains the same
  exact generation.
- Two surfaces may use identical logical space ids without sharing window ownership or generation.
- Reopen is rejected until the prior anchor, runtime registry, and terminal ticket snapshot have
  converged, so delayed native callbacks fail closed.
- A saturated capture-release retry cannot block native retirement forever, and an earlier prepared
  release cannot clear a newer pointer session.
- GPUI classifies window construction failures by exact stage; Dock maps those stages into typed
  rollback categories instead of exposing an opaque `anyhow` string.
- Forced surface shutdown does not merge layout, restore focus, or honor a child close veto.
  Ordinary child close still uses the configured close policy.
- DevTools can distinguish an inactive session, active work, pending native terminals, and a fully
  converged closed generation without receiving private leases or native handles.
- Real Win32 capture, pre-release provisional presentation, renderer/native teardown ordering, and
  process lifetime remain platform verification responsibilities rather than claims derived from
  `TestWindow` simulation.

## Rejected Alternatives

- Calling `App::quit` on primary close conflates one Dock surface with the application process and
  breaks multi-surface isolation.
- Native owner chains alone do not define close ordering, generation admission, or registry
  convergence and vary by backend.
- Inferring the primary from the first rendered host makes render order a lifetime authority and
  lets embedded content accidentally own application windows.
- Mirroring every runtime handle in `DockSurfaceOwner` creates two registries that can disagree
  during reentrant creation and teardown.
- Optional anchor equality, ever-increasing drag ids, or delayed cleanup without an opaque lineage
  cannot reject old work after a new generation opens.
- Porting ImGui's global context, raw viewport pointers, `LastFrameActive` collection, or platform
  callback ownership would replace Open GPUI's retained transaction model rather than adapting the
  proven lifecycle behavior.
