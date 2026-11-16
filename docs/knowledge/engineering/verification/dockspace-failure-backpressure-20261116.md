---
type: Verification Evidence
title: Dockspace Failed Generation Termination and Output Backpressure
status: verified
timestamp: 2026-11-16T19:04:00+08:00
git_commit: 3a1736747a9257b27aee67e0cf3db2c4783d9090
---

# Dockspace Failed Generation Termination and Output Backpressure

## Fixed source and scope

This follow-up starts at Open-GPUI `44b39e758bbb93c3ccf2577606825fb3d33a6ce3`
with dockspace `e8120864e06de445e1b588b07d6b322bb7125e41`. It addresses the GPUI
adapter gaps retained in the [dependency adoption record](dockspace-e812-adoption-20261116.md).
The dependency source and latest-format-only persistence policy are unchanged.
The verified implementation is committed at
`3a1736747a9257b27aee67e0cf3db2c4783d9090`; the scheduler-clock correction is
`8cc31c65e6632d962bfebbb112605a86d74f4246`. The physical cohort below ran the
same production file contents before the docking commit was created.

## Failed generation ownership

A public native-input regression records two original Primary presses in the
accepted core FIFO. The button journal rejects that immutable prefix. Normal
shutdown physically removes the Child but cannot publish its semantic retirement,
so the anchor originally waits indefinitely. The original failing nextest run is
`cba2972a-a16f-47f3-aba2-f80a38dc7cfd`, in
`%TEMP%/open-gpui-failed-generation-red-20261116.log`.

`DockSurface::begin_failed_native_shutdown` is an explicit, irreversible choice
after a classified protocol/internal failure. It does not classify every
`HostProtocol` error as irreparable. The failed path stops semantic collection and
new presentation acceptance, retains the complete old session, queued callbacks,
undispatched effects and affine results, and drives backend cleanup independently.
Actual capture release is read from its original terminal without popping or
reordering the old FIFO. A failed capture release remains blocked until the owning
backend supplies its required release or native terminal fact.

Physical teardown retains GPUI's original presentation shutdown tickets at close,
including construction rollback and unbound Root registrations. Child owners
retire before Roots. Non-anchor Roots must remain dispatchable while an anchor's
rollback ticket waits for their destruction. A currently borrowed live Window
causes one deferred retry after the borrow ends, rather than a timer or repeated
polling while waiting for a backend terminal.

An installed Window waits for its original native-terminal callback even if the
shutdown ticket already observes OS destruction. Failed opens with no installed
Window retain their original rollback ticket. Capture-release completions and
foreground mutation/focus wake handoffs remain counted until their original
callbacks finish. Native terminal cannot silently retire those asynchronous
owners. Display admission stops when failure termination begins; global and mount
subscriptions detach after physical termination.

`DockNativeTermination::Failed` is separate from `Drained`.
`native_is_quiescent` stays false for the retained failed protocol. The one-shot
`when_native_terminated` callback runs outside the host borrow. Inspection and
DevTools expose the disposition without copying arbitrary failure text into the
new status fields. Applications can reconstruct a fresh managed session from the
committed layout and their original panes after termination. Queued records,
drag identities, bindings and outputs never enter that fresh authority domain.
App's last-window policy still applies: in-process reconstruction requires a
surviving application recovery window or a policy that keeps App running. The
regression retains an independent sibling, verifies that it remains usable and
reuses the original pane entities under fresh bindings.

## Renderer capacity

Output capacity is ordinary renderer backpressure rather than a protocol failure.
When a later region exhausts the shared output allowance, the entire physical
encoding pass must roll back before publishing the candidate without new outputs.
The old displayed packets, serial, native binding and presentation artifacts keep
their original identities; core still decides whether each receiver remains
valid after the semantic update. No new docking submission or first-live proof is
created by that zero-output publication.

One `capacity_blocked` flag keeps retained dirt parked until a committed
presentation terminal releases output room. The managed pump skips that mount;
real input can still commit semantic work. The original presentation commit
wakes blocked siblings without an additional terminal observer or timer.

Moving a parked region originally combined its newly accepted hitboxes with the
previous encoding's `RegionState.bounds`. A normal click then produced a receiver
receipt outside its manipulation point and remained at the input queue head.
Source coordinates now come from the accepted `InputFrame` snapshot. A changed
region geometry also bypasses the parked replay shortcut so managed placements
can publish their original current facts. The regression translates a retained
region from x=0 to x=400, selects a tab under full capacity, and later observes
the ordinary output replacement without changing the old packet or serial early.

## Scheduler clock

The backpressure parking tests also exercise GPUI's ordinary frame demand.
`InputRateTracker` and frame pacing originally used `Instant::now()` directly,
while the test scheduler and docking continuations use the executor clock.
Advancing virtual time therefore left the high-rate hold and inactive throttle
unexpired. Scheduling decisions need one monotonic clock shared with their
executor; profiler and platform signal timestamps retain their original sources.

## Qualification

Cargo checks use the existing target directory, `CARGO_BUILD_JOBS=1`, and serial
nextest execution.

| Check | Result | Original nextest run identity |
| --- | --- | --- |
| GPUI library, including scheduler-clock regressions | 1765/1765 passed | `52bf0313-d764-4dd3-bc74-da5bf3cb33fb` |
| Default shared docking, including parked-region translation | 159/159 passed | `e7ec5eb1-b645-4ebd-81e5-1867aa7304b0` |
| All docking features, including the legacy engine | 1623/1623 passed | `d5530324-93fc-43d6-8e75-36cf59ac33d8` |
| Four finite-capacity regression scenarios | 4/4 passed | `dda46a39-f11c-4ea3-9ac1-2716eb6af4bd` |
| Failed-generation shutdown, callback custody and rollback regressions | 11/11 passed | `d4bf5507-d704-4881-b972-98e94b596fb5` |
| Final DevTools inspection and redaction consumer | 7/7 passed | `efc93f47-2d09-48a6-9d28-fe811cc871fd` |
| Native, editor, multiviewport and DevTools consumers | All-target check passed with `legacy-docking` | `%TEMP%/open-gpui-failure-capacity-consumers-final-20261116.log` |
| Workspace formatting, public API tiers and documentation links | Passed | `%TEMP%/open-gpui-failure-capacity-api-final-20261116.log` and `%TEMP%/open-gpui-failure-capacity-docs-final-20261116.log` |
| Original scheduler-clock regression | Failed before the clock correction, as expected | `3cf734dc-e043-48e1-ba69-6ad61e3b01d7` |
| Original parked-region translation regression | Failed with receiver geometry outside the edge point before the coordinate correction | `83577f61-4c96-4e8b-b00e-337b57fb65dc` (1622 passed, this one failed) |

The independent failed-generation review found and verified repairs for a borrowed
Window without a wake, a capture completion delivered after native destruction,
and an anchor rollback waiting on its own surviving Root dependency. Its final
review has no blocking findings. A combined simplification review reused the
existing Child-termination predicate and removed a redundant scheduling comment;
the ownership and terminal checks remain intact.

Normal binding quiescence and successful drain also wait for the original
callback handoff count to reach zero. The existing two-window shutdown regression
holds that owner past native destruction, then releases it before the original
binding-quiescence path succeeds. This preserves the same owner boundary for
`Drained` and `Failed` without requiring semantic consumption in the failed path.

The final capacity and scheduler-clock review also has no blocking findings after
the accepted-input coordinate correction. Profiler signal timestamps retain their
original sources; the clock regression changes scheduling decisions and test
fixtures, not instrumentation provenance.

## Selected Windows physical regression

Six original manifest rows execute serially, with no selector or receipt-protocol
changes, under
`%TEMP%/open-gpui-physical-acceptance-20261116-101601/focused-185406/`.
The cohort runs from 18:54:06 to 18:58:10 +08:00, including the initial native
consumer build. All six tests pass and all six token-bound behavior receipts
match. This is a selected six-row regression, not the full fifteen-row manifest
gate. No older cohort's receipt is substituted.

| Manifest row | Result | Original nextest run identity |
| --- | --- | --- |
| `native.u27.source-capture` | Passed | `875a9a81-fc47-4040-846f-e7dd32f246cd` |
| `native.u27.opaque-occlusion` | Passed | `4152a77b-5da4-4f72-bea1-35a7824b6ef9` |
| `native.u27.surface-shutdown` | Passed | `b41bc5a5-3b18-4a4e-8439-74fc34c2a5b6` |
| `native.u29.provisional-same-hwnd-promotion` | Passed | `48f1c46e-a459-4b09-8d31-cf85c5c71099` |
| `native.u29.committed-loss-recovery` | Passed | `4f0e8453-b433-4a7c-986e-57b0566bd091` |
| `native.u28.process-convergence` | Passed | `ad5f86e7-65ef-4a6c-96e7-741dccaf2aa5` |

The original worker nonces, Jobs, independent HWND census, capture terminals,
stdout/stderr and process-exit receipts remain in that directory. The process
row includes both LastWindowClosed and AppShutdown. Its `results.json` retains
each original command, run identity and receipt verification result. The wrapper
log is `%TEMP%/open-gpui-failure-capacity-physical-final-20261116.log`.

## Qualification limits

Failed-generation injection and finite-output-capacity scenarios use TestPlatform.
They exercise original framework capture, renderer and shutdown authorities but
do not constitute an independent real-GPU failure-injection qualification. Real
Windows scenario receipts are reported separately. The session has one remotely
controlled display, so mixed-DPI remains excluded. Display disconnect/reconnect,
cross-window IME composition and other backends retain their own qualification.
