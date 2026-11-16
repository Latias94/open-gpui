---
type: Verification Evidence
title: Dockspace Core Follow-ups and GPUI Integration Repairs
status: partial
timestamp: 2026-11-16T14:26:00+08:00
git_commit: b42e138306195ac03e7d8288b5fe3fdd6742ba2a
---

# Dockspace Core Follow-ups and GPUI Integration Repairs

The later [e812 adoption record](dockspace-e812-adoption-20261116.md) tracks
upstream fixes for this backlog and their GPUI migration. This record retains
the earlier dependency pin and original qualification results.

## Baseline and ownership

The investigation starts at Open-GPUI `77eaead0b388` and dockspace
`dcd65e983220e17662b1f02929fcf84348cd8eb3`. The dependency pin is unchanged.
The [single-display physical record](dockspace-windows-physical-20261116.md)
remains the authority for the preceding 13/14 selected Windows cohort, the
excluded mixed-DPI row, and the opaque-over-Foreign contract failure.

## Copyable dockspace core follow-up list

1. **P1: Explicit native placement over an opaque foreign window.**
   A Foreign hit must continue blocking docking into covered application content.
   The current route also revokes Native preview and cancels release, preventing
   the existing provisional window from being promoted over that foreign window.
   Add explicit native-placement authorization with independent work-area
   authority. Preserve the original release, reservation, recovery policy and
   first-live protocol. Acceptance requires the original View, the same native
   window, the point-scoped Z-order barrier and native terminal ordering.

2. **P2: Complete repaint reporting for interaction-only visual changes.**
   `HostFrameReport::repaint_surfaces()` currently derives from scene deltas.
   Preview, guide and hover changes can require rendering without changing the
   scene stamp. Include their affected old/new surfaces in the committed repaint
   report while retaining the provenance-only optimization. A retained consumer
   should be able to invalidate the affected surfaces using the public report.

3. **P2: Define terminal behavior after permanently rejected host ingress.**
   Retaining a rejected FIFO head preserves ordering, but a permanent protocol
   rejection can prevent semantic drain after physical windows have retired.
   Specify fatal-state admission, remaining physical cleanup, terminal failure
   and fresh-session reconstruction conditions. Do not clear or replay affine
   results, fabricate successful quiescence, or allow stale callbacks to acquire
   a new lifetime. Adapters should distinguish retry, waiting for an original
   terminal, and irrecoverable failure.

4. **P3: Strengthen cross-adapter conformance coverage.**
   Add public-contract cases for interaction-only invalidation, native placement
   over Foreign hits, and fatal ingress during retirement. OS input, native hit
   testing and physical presentation remain backend qualifications. Reuse the
   existing test infrastructure without adding another authority registry.

Stationary continuations, focus commands, pane selection and close semantics
already exist in core. Their GPUI bindings, fresh OS observations, retained Views,
chrome, report history and native restore orchestration belong in Open-GPUI.

## GPUI implementation scope

- Fresh finite native pointer sampling for the original active drag generation;
  no cached coordinates, synthesized mouse edge or new ingress sequence.
- Default focus capability mapping and original window-activation observations.
- Dynamic pane title/minimum updates with core measurement invalidation, plus
  conservative release of closed View registrations and durable key retention.
- Default close/strip controls, keyboard and accessibility actions, optional
  content focus, and a source-hosted custom-factory preview.
- Bounded observational report history, preserving unresolved deferred close
  decisions; original internal ingress and lifecycle receipts remain unbounded
  by this diagnostic policy and retain their existing owners.
- Read-only native FIFO head/rejection diagnostics and fatal application-intent
  admission without clearing errors or bypassing native cleanup.
- Fresh-host native restoration from validated documents and current displays.
- Repaired legacy benchmark invocation and separately labeled shared-host CPU
  measurements without substituting them into legacy/ImGui comparison rows.

## Qualification boundary

The shared default and compatibility regression suites pass. This follow-up
executes a new selected fourteen-row physical cohort; it does not reuse the
preceding cohort's successful receipts. Mixed-DPI remains excluded for the
user's single-display remote session. Real IME composition across windows,
display disconnect/reconnect and each other backend retain separate qualification.

CPU tests exercise ownership and correlation contracts. The new shared benchmark
uses TestPlatform, Noop text and simulated presentation; it makes no GPU or native
input latency claim. Core-dependent follow-ups above remain open.


## Executed regression verification

All Cargo runs use the existing target directory, `CARGO_BUILD_JOBS=1`, and
nextest `--test-threads=1`.

| Check | Result | Original nextest run ID |
| --- | --- | --- |
| Default shared docking | 143/143 | `c048966d-2d0e-46dd-a620-665e8360c76d` |
| Reviewed all docking features, including legacy | 1607/1607 | `9f913c3e-afbb-4bec-b444-804001ef2f78` |
| GPUI and Windows library baseline | 2090/2090; 10 existing ignored tests excluded | `08d17190-8509-4533-ade5-0b939524452e` |
| Fresh sampling and physical-coordinate controls | 14/14 | `abf5fbe3-6ef7-42a3-a523-614d1c6f0167` |
| Original multi-workspace restore consumer | 1/1 | `5900341c-a1af-4494-b90b-1e8d98a71980` |
| Reviewed quiescence and restore cleanup | 3/3 | `42521fa0-8157-4c65-bd40-bed24a85faef` |
| Native example baseline | 35/35; 13 existing ignored tests excluded | `43835775-b7a1-4fd4-af3a-841563bf4524` |
| DevTools inspection consumer | 6/6 | `ff8725fb-4eaf-4259-9ba9-8e41fb0e718c` |
| Real Windows pointer/capture and fresh stationary sample sentinel | 1/1 | `a0562190-22c5-4c8f-88ee-1608d2de11cc` |

The controlled shared tests execute a real native-producer hover deadline without
another captured move and preserve the original gesture when fresh samples are
unavailable. Framework sampler tests reject changed geometry, capture, button,
point-stack, stale generation and earlier pending ingress, while preserving the
callback-scoped frame and native ingress sequence. Actual native stationary
sampling is additionally asserted inside the system-input sentinel after real
WndProc capture acquisition and before physical MouseUp.

Pane tests cover title remeasurement, minimum-size layout, original entity and
focus retention, closed-key persistence, fatal-intent rejection and original
shutdown. A weak-entity regression removes the fixture's extra strong references
and observes actual View release after packets retire and the returned pane is
dropped. The original native pump delivers one-shot quiescence notifications outside
host borrows; restored application registry entries are released after actual cleanup,
including failed restores without another user action. Prepared chrome cannot
overtake earlier native ingress. Deferred close
reports survive history pressure; repeated identical decisions do not grow the
protected set. A stale focus binding cannot block the original terminal suffix.

Final `cargo fmt --all --check`, public API tier scanning, public documentation
link scanning and `git diff --check` pass. Consumer checks include shared docking,
DevTools with docking, multiviewport and the editor, with all targets. Python
benchmark tests pass 23/23. The shared-host CPU benchmark executes seven samples
of 100 two-step cycles after warmup, with current-packet and retained-View checks.
Its debug-profile execution is an executable-workload check, not a release
performance or GPU qualification. Legacy and shared benchmark data remain
separate. An initial Windows executable stack overflow is corrected by running
the test-renderer workload on an explicitly sized 16 MiB thread.

Original regression logs use `%TEMP%/open-gpui-followups-*-20261116.log`.

## Final single-display physical cohort

The final implementation is committed at `b42e138306195ac03e7d8288b5fe3fdd6742ba2a`.
This cohort executes that fixed source after the quiescence-notification review
correction. Its original evidence is retained under:

```text
%TEMP%/open-gpui-physical-acceptance-20261116-101601/focused-141729/
```

The unchanged manifest rows execute serially with their original exact selectors,
ignored policy, worker Jobs, nonce handshake, independent HWND census, capture
terminals, process exit and token-bound behavior receipt. The fourteen original
behavior receipts match. Outcome: **13 passed, 1 failed; mixed-DPI excluded**.
The complete selected gate therefore remains failed/partial rather than 15/15.
The earlier pre-review cohort remains separately in `focused-134834/` (also
13/14); no receipt or nonce is merged into the final cohort.

| Manifest row | Result | Original nextest run ID |
| --- | --- | --- |
| `native.u27.source-capture` | Passed | `18df1d51-2e4d-42db-b973-a327fe7062fa` |
| `native.u29.complex-dockspace-workflow` | Passed | `9fda1171-90bf-4184-9b0b-59386326d815` |
| `native.u27.opaque-occlusion` | Failed: opaque same-HWND promotion timeout | `3e8c44fd-6d31-43f0-b6c5-d64fe4a0995f` |
| `native.u27.opaque-hit-transparent-prefix` | Passed | `201e34f2-3ae4-469c-a2cb-244616086ad3` |
| `native.u27.surface-shutdown` | Passed | `8fbc622e-d55e-4bd0-a38d-70f9391e41b7` |
| `native.u29.provisional-same-hwnd-promotion` | Passed | `fc41047e-4399-4f94-a69c-97d0fdc3ff9a` |
| `native.u29.live-route-and-release-lock` | Passed | `dadbbb3a-f08e-4030-b0a3-8f3002d8407c` |
| `native.u29.committed-loss-recovery` | Passed | `8f7cb834-1c94-4f90-b144-cec1af4ca923` |
| `native.u28.activation-terminal` | Passed | `d6771293-e2d9-4030-9a41-0c73f37a4be0` |
| `native.u28.provisional-activation-z-order` | Passed | `d52bff4b-a822-4b73-b982-59811eb5d446` |
| `native.u28.client-geometry-reconciliation` | Passed | `46c4a318-3e41-479b-8d75-644dc54e9ee9` |
| `native.u28.event-driven-geometry-wake` | Passed | `89f20294-77bf-47e6-8295-a40df8484cdf` |
| `native.u28.process-convergence` | Passed | `3c722d21-c96a-4946-ade4-7963974423d6` |
| `native.u28.no-input-pass-through` | Passed | `c26cf83e-374c-4657-a35e-68ad2b1b86d6` |

The opaque row still reports the actual Foreign barrier and retains the original
View on cancellation. It is not reclassified as desktop, promoted through a new
window, retried with another release, or counted as a pass. The core follow-up
above remains required.

Original successful worker nonces include:

| Worker | Nonce |
| --- | --- |
| `native.u28.process-convergence / last_window_closed` | `96676-last_window_closed-18e8723430580bec` |
| `native.u28.process-convergence / app_shutdown` | `96676-app_shutdown-18e872349d15efec` |
| `native.u29.committed-loss-recovery / primary` | `96184-primary-18e8722da91b0180` |
| `native.u29.live-route-and-release-lock / primary` | `86144-primary-18e8722c27c5693c` |

The corresponding original stdout/stderr and census reports remain under the
cohort directory. The selected cohort passes both process-exit paths, same-HWND
promotion, immediate pointer divergence after release, committed-loss dock-back,
activation/Z-order, native geometry and no-input routing. No unrelated window is
moved or minimized, and no multi-monitor result is inferred from this session.
