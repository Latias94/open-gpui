---
type: Verification Evidence
title: Windows Shared Dockspace Readiness
status: partial
timestamp: 2026-11-15T22:48:00+08:00
git_commit: 479c48011efb15930025a71ad5578daf414cc2ac
---

# Windows Shared Dockspace Readiness

## Source and scope

The assessment starts from clean Open-GPUI
`479c48011efb15930025a71ad5578daf414cc2ac` on Windows 11. The dependency remains
the locked dockspace revision `dcd65e983220e17662b1f02929fcf84348cd8eb3`.
A fresh read-only remote check also observes that revision at `HEAD` and `main`.
The local Zed reference is `6ceb6fee8738274589b59139df27efc191ec2231`; this
assessment does not select or implement another Zed synchronization range.

[ADR 0034](../../../adr/0034-shared-dockspace-session-integration.md) remains the
architecture authority. The [source-migration record](dockspace-shutdown-migration-20261115.md)
establishes that all ten docking workers dispatch to shared sessions. It does
not establish physical Windows parity. The complete native manifest also
contains five Windows-platform behaviors.

The inspected dockspace reference is Cargo's checkout of the actual locked
source, including `crates/egui_dockspace`, `crates/egui_dockspace_native`, and
`crates/dockspace_winit/examples/multiview`. The older local `egui_docking`
repository is a different implementation and is not the shared-core reference.
No reference checkout or dependency pin is changed.

## Local physical capacity

An independent User32/Shcore inventory reports an interactive session and:

| Display | Physical bounds | Physical work area | DPI |
| --- | --- | --- | --- |
| Secondary | `(-2560, 533)..(0, 1973)` | `(-2560, 533)..(0, 1913)` | 120 / 125% |
| Primary | `(0, 0)..(3840, 2160)` | `(0, 0)..(3840, 2088)` | 144 / 150% |

This provides the distinct-DPI pair and negative display origin needed by the
declared mixed-DPI scenario. It is not a pointer-delivery, rendering, or
same-HWND-promotion receipt.

## Source findings

These findings describe the fixed assessment base. Implementation follow-up
below records changes that supersede them. They are source observations unless
the execution section explicitly records a reproduction. Preserve conservative unavailable results while filling
the missing observations; an active framework drag is not an OS capture fact.

1. **Physical capture observation is missing.**
   `adapter/native/input.rs::captured_pointer_input` supplies
   `NativePointerOwner::Unknown` as capture ownership.
   `native/transport.rs` publishes an `Unsupported` report for the first accepted
   captured event of a generation. `SharedSession::check_health` rejects that
   report. GPUI's immutable physical pointer frame currently carries position,
   source geometry and target display, but no independent native capture-owner
   observation. Add the observation at the owning backend callback and carry it
   through the original ingress; do not substitute a later global query.

2. **Managed desktop work-area authority is not connected.**
   `NativeBridge` starts with `NativeWorkAreaRoster::Unknown`. Its public transport
   accepts real work-area snapshots, but `host_native::enroll` does not publish
   them, and captured input always supplies `None` as its work-area binding.
   The locked core's native preview path requires a current provider/generation
   binding for the work area. Publish coherent platform topology through the
   existing FIFO, commit it, then resolve the original event's target display to
   the core-issued work-area binding.

3. **Pre-release native preview has no physical owner.**
   `shared_interactive::run_provisional` waits for a visible provisional HWND
   before releasing the pointer. The core's move path provides a Native preview;
   `element/recording.rs` currently ignores that visual. The managed adapter's
   default factory opens an ordinary window when the release requests native
   creation. There is no shared-path connection to GPUI's provisional-window
   lifecycle. Moving the wait after release would weaken the scenario. Design
   the preview-to-create handoff against the core's public identities first;
   preserve one physical HWND, original output correlation and native retirement.

4. **The mixed-DPI fixture loses the selected source display.**
   `two_window_bounds_on_display` calculates bounds for the selected display,
   while `SharedSession::build` does not put its ID into `WindowOptions`.
   Windows can then select the primary display and replace out-of-display bounds.
   Carry display selection with the fixture options and verify actual display,
   scale and client geometry after opening. The worker currently compares outer
   rectangles for inequality; it does not check exact target-side client bounds
   or require the actual route to visit the negative-coordinate probe.

5. **Process-convergence has inconsistent anchor roles and an exit race.**
   The two-root fixture marks source as anchor (`index == 0`); the journey closes
   target and expects a group drain with source retiring first. Closing a
   non-anchor follows ordinary close policy instead. The last-window branch
   also waits asynchronously after requesting the final close and only later
   publishes completion; the native message loop can return first. Retain
   both sessions and pending assertions before the final exit request, then
   inspect the retained Application after the message loop returns. AppShutdown
   must exercise active capture and pending native creation rather than only
   shutting down the survivor after primary cleanup.
   `SharedSession::shutdown` also waits for the entire App window registry to
   become empty. A per-session cleanup helper must instead wait for its own
   bindings while an independent survivor remains live; the worker owns the
   final App-wide census.

6. **The committed-loss worker exercises ordinary close recovery.**
   It posts `WM_CLOSE`, allowing `RecoverPresentation` before native destruction,
   then uses the test-only shared Dock Back command. This is different from an
   admitted destination unexpectedly reaching its native terminal. Establish
   exact admission/submission first, inject loss for that physical lifetime, and
   verify the original View entity, local state, anchor and output terminal
   ordering. An `Empty` pane and an existing item ID do not establish View-state
   retention.

7. **The complex-workflow migration weakens the fixture contract.**
   The shared worker hard-codes four transfers instead of executing the selected
   fixture. Its first transfer targets a tab although the fixture requires a
   right-edge split. Item membership and selection checks do not replace the
   declared per-step layout, exact submitted preview and native-lifetime oracles.
   Retain the checked-in scenario vocabulary and compare shared-session layout
   through its public model, without restoring the legacy reducer as a writer.

The common shared drag helper also reads the first recorded event for a source
window from its entire history. A second drag from that source can select an old
generation. Establish a fresh observation cursor and an actual post-activation
native move before selecting the current generation.

## Module direction from the reference adapters

This is a proposed implementation sequence, not a replacement for ADR 0034.
Keep the existing crate and public facade. Extract private modules when a real
owner becomes clearer; avoid a new crate or forwarding interface per helper.

| Owner | Responsibility and proposed organization |
| --- | --- |
| `host.rs` | Keep the sole semantic session, retained pane entities, mounts and application facade. Documents/history remain application-owned. |
| `input.rs` and private `host_native/receiver.rs` | Move `InputFrame` out of `continuation.rs`. Keep native receiver resolution with the original serial, hitboxes, binding and submitted artifact. Preserve its injectable target-rank query and existing tests. |
| Managed desktop observation | Connect real display topology, target-display work areas and callback-scoped capture facts. Keep OS observation in the platform crate and docking token translation in the adapter. |
| `native/capture.rs` | Consolidate callback-owned pre-motion start custody, accepted-event cursor, cancellation and exact release barrier. Preserve the failed-release latch and close blocking. |
| `native/effects.rs` | Separate create/placement/focus/input/close execution from callback installation. Continue using the original bridge, pending request and core `NativeWindows` index. |
| `host_native.rs` | Keep managed enrollment and one pump: collect original prefix, commit, release the host borrow, execute effects, process receipts and retirement. Preserve reentrant `pending_reports`. |
| `element/frame.rs` | Extract candidate building, measurements, program recording and suspend from GPUI element layout/prepaint/paint. Reuse `recording.rs` and `chrome.rs`. |
| `presentation.rs` | Retain ownership of the original per-Window ticket, candidate acceptance, renderer terminal and content input admission. Renderer deferral must not retain the semantic lease. |
| Native preview lifecycle | Add one owner for preview placement, same-HWND promotion and cancellation/retirement, using existing GPUI provisional-window authority and explicit core correlation. |
| Windows test harness | Share only application/report/census boilerplate in the existing `process_harness`. Keep core-created Child, external Root, survivor and pending-open fixtures distinct. |

The shared workers still compile inside the legacy-feature-required dogfood
binary and import helpers through the parent test module. Move their common
platform observation and process support toward an independent test owner as
the scenarios are qualified, preserving unique manifest coordinates. This does
not require another crate or a generic GUI scenario language.

The reference egui receiver joins completed-pass hit authority, scale, native
binding and presentation artifact. GPUI should retain its own equivalent facts.
The reference capture coordinator keeps outstanding release duties through
retirement. GPUI additionally owns its original drag generation, post-borrow
release barrier and asynchronous renderer tickets; those obligations cannot be
replaced by the example's synchronous assumptions.

The fixed reference files are
[egui receiver](https://github.com/Latias94/dockspace/blob/dcd65e983220e17662b1f02929fcf84348cd8eb3/crates/egui_dockspace_native/src/receiver.rs),
[capture coordinator](https://github.com/Latias94/dockspace/blob/dcd65e983220e17662b1f02929fcf84348cd8eb3/crates/egui_dockspace_native/src/coordinator/capture.rs),
[native owned update](https://github.com/Latias94/dockspace/blob/dcd65e983220e17662b1f02929fcf84348cd8eb3/crates/egui_dockspace_native/src/native_owned_update.rs),
and [winit multiview host](https://github.com/Latias94/dockspace/blob/dcd65e983220e17662b1f02929fcf84348cd8eb3/crates/dockspace_winit/examples/multiview/host.rs).

Keep one ordered native FIFO. Splitting modules must not create independent
queues, let a rejected head lose its affine owner, or let a suffix bypass it.
Retain distinct resource maps when they describe different lifetimes: core
placement, physical callback, mount and renderer retirement are not interchangeable.

## Recommended implementation order

0. Diagnose the baseline frame-signal failure and the two shared-worker census
   timeouts recorded below. Add bounded phase messages to the existing worker
   stderr/milestones so a timeout identifies startup, root readiness, gesture or
   cleanup. Retain post-loop Application/probe inspection and the original Job
   deadline. Do not infer that the static capture or work-area findings caused
   these currently unlocalized hangs.
1. Connect native capture facts and desktop topology/work-area bindings. Organize
   receiver/input evidence at the same time, with focused existing regressions.
2. Restore shared cross-HWND drop and exact current-generation terminal checks.
   Correct fixture anchor/display selection and post-loop process inspection.
3. Implement the native preview-to-create handoff and same-HWND promotion. If the
   selected public core protocol cannot express the handoff, define that core
   contract explicitly before adding adapter state.
4. Restore the full manifest oracles for complex workflow, two-way mixed DPI,
   pending-open AppShutdown and actual committed destination loss. Use retained
   stateful pane Views for recovery and movement checks.
5. Run the complete manifest-owned gate serially. Then audit legacy consumers
   before removing compatibility code. Keep general fatal-host reconstruction a
   separately defined lifecycle contract.

## Execution

The exact manifest-owned command ran with `OPEN_GPUI_NATIVE_INTERACTIVE=1` and
`CARGO_BUILD_JOBS=1`, using the checkout's default target directory. The compiler
is Rust 1.100.0 and nextest is 0.9.138. This is local Windows execution, not a CI
runner attestation or a new Rust 1.95 compiler-floor receipt. No executable source
changed during the run; only this assessment and its documentation links changed.

| Stage | Actual result |
| --- | --- |
| Federated UI contract | All 16 exact registered tests pass; unrelated tests are filtered. |
| Native system-input sentinel | **1/1 pass**, 1.415 s, run `8a679646-c98a-4dea-90e7-d190db04cf5d`. Real pointer delivery and capture acquisition/release execute. |
| Windows backend baseline | **321 pass, 1 fail, 10 ignored**, 189.614 s, run `fb7eea61-73f4-4d3e-9e18-94a1fa27e23b`. |
| Complete gate | **Exit 1**, before the docking registry and fifteen manifest rows. |
| Separate docking registry check | **1/1 pass**, run `42c66d5c-442b-4b1d-92a4-d28d7c82b248`; other tests are filtered. |
| Separate manifest `source-capture` diagnostic | **Fail**, 60.171 s test time, run `976aab9f-eedc-4181-8278-22bcc06ea355`; no pre-exit census. |
| Separate manifest `surface-shutdown` diagnostic | **Fail**, 60.151 s test time, run `47203fdf-9554-46d7-af70-65a8b39ae168`; no pre-exit census. |

The baseline failure is
`platform::native_test_support::native_power_mode_tests::native_low_power_configuration_defers_callbacks_and_recovers_submitted_source`.
At `crates/gpui_windows/src/native_power_mode_tests.rs:769`, the test does not
observe its expected exact `signal_at` with `NativeCallback`, without forced
rendering or required presentation. This observation does not establish whether
the cause is the fixture or the production frame path, and is not attributed to
docking. The original guard is retained; the full gate is not reported as passed.

The two separate diagnostics run serially after the failed complete-gate attempt,
using rows read directly from the manifest, exact selectors, fresh nonce-bound
entry receipts and the original worker/Job protocol. Both nextest processes exit
100. Their entry receipts match, but neither child publishes its pre-exit report
before the original 60-second deadline. Each retained worker stdout contains
only the Rust test-harness entry; both worker stderr files are empty. The test
parent's timeout is at `process_harness.rs:1150`.

Consequently, these executions do not identify whether startup, root submission,
gesture admission or cleanup stalls. They do not reproduce a particular static
finding above, prove the normal RELEASE/exit handshake, or establish semantic
quiescence. A subsequent process inventory finds no remaining `docking-native`
process. Process cleanup after a timeout is not a successful native-lifecycle
receipt. No independent execution of process-convergence, mixed-DPI placement or
committed-loss recovery is credited.

The frozen manifest SHA-256 is
`e0a4e783b64b94d0bf767af74e39eeb41082aebe7b883182a26bc84496847b92`.

The two executions and retained entry-receipt paths are recorded in
`%TEMP%/open-gpui-windows-focused-20261115.json`; full logs are
`%TEMP%/open-gpui-windows-source-capture-diagnostic-20261115.log` and
`%TEMP%/open-gpui-windows-surface-shutdown-diagnostic-20261115.log`.
The separate registry log is
`%TEMP%/open-gpui-windows-dock-registry-20261115.log`.

These diagnostics cannot replace the failed baseline or qualify the full gate.
No parallel Cargo or native scenario is launched by this assessment. Independent
read-only review finds no P1/P2 defect in the source assessment and proposed
module direction. Public documentation links and diff whitespace checks pass.

The local full output is retained outside the repository at
`%TEMP%/open-gpui-windows-gate-baseline-20261115.log`. Existing successful worker
stdout/stderr files and scenario receipt files are deleted by their owning
RAII/cleanup paths; failed worker logs are retained. The current runner verifies
nonce-bound reports and Job/process exit, but does not provide a durable archive
of every successful worker report.

## Implementation follow-up

The user authorized continuing the implementation from this assessment. The
following observations supersede the previously unlocalized timeout; they do not
qualify the complete Windows manifest.

Bounded worker phase diagnostics localize the source-capture timeout to the
returning event loop after scenario cleanup and census. The original journey
first fails because its input point is occluded: foreground acquisition occurs
after its canary checks. After terminal App shutdown, the worker calls ordinary
`App::quit`, whose native quit callback cannot complete that already-closed
ingress. Both shared worker shells now request the existing
`shutdown_and_quit_for_native_exit_test` authority after storing completion.
The drag helper acquires foreground before canary checks, selects the current
generation after a real post-activation move, and waits only for its own managed
session during per-session shutdown.

The next serial source-capture and surface-shutdown runs publish nonce-bound
failed reports in 12.260 s and 11.997 s respectively, rather than hitting the
parent's 60-second census deadline. Both reports contain an empty process HWND
census, zero App windows, settled native exit authority, released capture, and
two observed/two terminal native generations. The remaining failure is native
drag activation. These are successful failure-report/cleanup receipts, not
passing docking behaviors. Logs are
`%TEMP%/open-gpui-windows-source-capture-exit-fix-20261115.log` and
`%TEMP%/open-gpui-windows-surface-shutdown-exit-fix-20261115.log`.

A subsequent source-capture diagnostic distinguishes an accepted core pointer
press/gesture from an absent GPUI active drag and an empty captured-event list.
Temporary input probes and a press/redraw/move adapter regression investigate
that boundary. An intermittent canary failure is retained separately; it is not
silently retried into a passing result. The full native assertion set remains.

The baseline low-power test unconditionally expects `Some(at)` although the
default build disables profiling. `PlatformFrameSignal::capture` deliberately
returns `None` in that configuration. The helper now uses that same public
feature contract while retaining the NativeCallback and non-forced-presentation
checks. Execution of the corrected exact test is pending.

The implementation also introduces callback-owned native capture observations,
freezes their translated core bindings at FIFO enqueue, and connects complete
Windows physical display publications to work-area ingress. Desktop publication
is a commit barrier before its suffix can resolve a core-issued work-area
binding; target identity, topology generation, bounds and scale must still match
the original event. No single target-display sample is promoted into a complete
roster. `GetCapture() == NULL` remains unavailable desktop capture authority.

Private native receiver resolution moves into `host_native/receiver.rs` and
`InputFrame` moves from continuation to input. The session, native index, queue,
submission tickets and release barriers retain their original owners. Adapter,
backend and native-scenario verification of these changes is in progress.

A later real source-capture run establishes the native drag and routes captured
motion into the second HWND. It then reports an unobservable original release:
the managed router's release locker did not subscribe before natural MouseUp
retired the framework drag. Its post-borrow cancellation could arrive after the
original release barrier had already completed and been removed. This replaces
the activation failure as the latest localized source-capture blocker; the log
is `%TEMP%/open-gpui-windows-source-capture-div-check-20261115.log`.

Natural MouseUp now reserves the original generation's release before route
locking and user listeners. An observation-only hook attaches the original
continuation without cancelling the drag or creating another barrier. The
managed release locker uses that hook, and terminal delivery retains the
existing native FIFO and callback custody. Callback state retains every pending
original `(binding, barrier)`, so a second drag can complete while the first
terminal awaits host collection. Exact terminal identity removes only its own
entry. Failed release still quarantines the physical callback until its actual
native terminal; service cannot retry an already retired failure.

The default-feature focused run `518a86db-7e52-4060-a02b-67b01b2b0970` executes
180 tests: 179 pass and one new two-drag test fails during shutdown collection.
The physical capture-owner backend test, registry-generation tests, corrected
low-power test, GPUI display publications and pointer-session tests all pass.
Both original natural-release terminals are already observed in the failing
test; its helper incorrectly unwraps a roster boundary instead of committing
the accepted prefix and retrying the preserved front, as the production host
does. After correcting that helper, all twelve FIFO/capture tests pass in run
`230dcec1-d9b2-4b81-824c-25e3ec46249b` (4.725 s), including consecutive natural
releases and bounded natural-release failure. This is focused regression
evidence, not a completed interactive manifest. Logs are
`%TEMP%/open-gpui-windows-dock-release-resume-20261115.log` and
`%TEMP%/open-gpui-windows-capture-fifo-check-20261115.log`.

Adversarial follow-up identifies one additional interleaving: G2 can request
shutdown cancellation behind G1's uncollected release, then naturally release
before either terminal is collected. Accepting G2's original terminal must also
retire that generation's queued cancellation, rather than later treating its
already-retired GPUI drag as unobservable. Exact terminal completion now removes
only the matching source/generation cancellation. The two-drag regression
retains G2's actual prepared consumer and requests shutdown before its natural
MouseUp. All twelve tests pass again in run
`96aea854-0cde-4d49-baf4-e55bf2feefc8` (0.422 s), including final native
quiescence. Read-only adversarial re-review finds no remaining P1/P2 issue in
this correction. The log is
`%TEMP%/open-gpui-windows-capture-close-check-20261115.log`.

Fresh native binaries compile with the example's profiler/test-support feature
set. The first post-fix real source run fails its original WndProc canary before
drag activation (11.301 s), and the next run fails cursor admission (11.372 s).
Subsequent runs stop at the original physical-primary-button precondition,
including a run preceded by an independent one-second idle-pointer observation.
These failures do not exercise the repaired release path and are not attributed
to it. Every failed worker still publishes its nonce-bound census, completes
RELEASE, retires both observed generations and leaves empty App/HWND registries
with released capture. Bounded canary diagnostics now include requested/current
pointer coordinates, original mouse messages, foreground and HWND hit facts;
the worker preserves the complete error chain. Original failures and deadlines
remain. Latest logs include
`%TEMP%/open-gpui-windows-source-capture-release-check-20261115.log`,
`%TEMP%/open-gpui-windows-source-coordinates-diagnostic-20261115.log`, and
`%TEMP%/open-gpui-windows-source-idle-diagnostic-20261115.log`.

The process fixture now marks the target root as anchor. Process convergence
retains both sessions, probes and Application through loop return, without an
await or explicit-cleanup substitute after the final natural exit request.
Post-loop inspection checks both sessions, native capture, original HWNDs and
shutdown ordering. The mixed-DPI fixture carries the selected display into root
WindowOptions and checks actual display/scale and GPUI/Win32 client geometry
before dragging; reveal and release preserve the frozen negative-coordinate
route. Native execution of these paths remains pending. The full AppShutdown
capture/provisional/pending-open setup and an expected final target client
rectangle derived from its original placement ticket remain separate work.

Reference inspection also resolves the preview rendezvous: the core's Native
visual reserves `target_surface`; the release-created request carries that same
surface in its exact binding. A GPUI-owned, noninteractive provisional shell can
therefore be adopted once by `NativeCallbacks::install` in the existing factory.
It must complete final provisional placement, reuse the destination-semantics
ticket for the real output, and wait for both original renderer submission and
exact core admission before opening interaction. Before adoption, cancellation
still owns the shell's native retirement and anchor dependency. The locked core
does not provide a destination presentation stream before release; a full
pre-release target layout would require a separate explicit core contract.
This establishes an implementation path, not a same-HWND execution receipt.

## ImGui Win32 reference and module ownership

The local ImGui docking reference is version `1.93.1 WIP`, revision
`cf53467ab0db7a783f04b32dd9160e6aa5b720ef` (2026-11-10). Inspection covers
`backends/imgui_impl_win32.cpp`, `backends/imgui_impl_dx11.cpp`, and the core's
`UpdatePlatformWindows` / `DestroyPlatformWindow` paths. No reference code changes.

ImGui recommends one context with multiple OS windows. Per-viewport platform
data owns HWND identity, native ownership and styles; renderer data separately
owns the swap chain and render target. Platform creation precedes renderer
creation, and renderer destruction precedes HWND destruction. This supports the
existing single-session decision and physical lifecycle ownership in ADR 0034.
It does not justify another docking controller for each GPUI window.

The Win32 secondary-window procedure turns `WM_CLOSE` into a core request and
suppresses default destruction. Native ownership uses `GWLP_HWNDPARENT`, not
`SetParent`; the latter changes coordinate and clipping semantics. Pointer
capture and hovered viewport are independent observations. Before destroying a
captured secondary window, ImGui transfers capture to its main HWND so the
shared context can receive the later MouseUp. GPUI must retain its own exact
source/generation release proof rather than treating that transfer as release.

ImGui's viewport geometry is client geometry. Its setter converts to an outer
rectangle with the selected DPI, while monitor enumeration keeps `rcMonitor`
and `rcWork` separately and preserves negative desktop origins. `SPI_SETWORKAREA`
and display changes invalidate monitor data. The GPUI physical roster and target
observation therefore belong to the platform boundary, without reconstructing
them from scaled logical display coordinates.

ImGui can show a new viewport before its first Render/Present and consumes
frame-scoped DrawData. Its DX11 Present call does not supply a session-level
terminal receipt. GPUI's exact submission ticket, first-live admission and
capture/native terminal barriers remain necessary for its asynchronous renderer.

The current module boundaries are:

| Owner | Responsibility |
| --- | --- |
| `dockspace::DockspaceSession` | Layout, selection, gesture semantics, native effect identities and recovery policy. |
| `adapter/host.rs`, `element.rs`, `input.rs` | Retained View entities, physical mounts, logical region aggregation and local input. |
| `adapter/host_native.rs` | Enrollment, the one session pump, factory integration and group shutdown. |
| `adapter/host_native/receiver.rs` | Receiver answers from the committed hit graph and original submitted artifact. |
| `adapter/native/transport.rs`, `input.rs`, `desktop.rs` | Original callback FIFO, immutable event translation and complete physical desktop publications. |
| `adapter/native/gpui.rs` | Physical callback custody, native effects, capture barriers and dependent retirement. |
| `adapter/presentation.rs` | Candidate acceptance and original renderer terminal correlation. |
| `gpui_windows` | HWND generations, Win32 input, physical DPI/geometry and native destruction. |

Keep the queue, session and callback custody shared across these private modules.
Extract a provisional-window owner only when implementing preview/adoption; it
must retain both pre-adoption and post-adoption retirement obligations. File
splitting alone must not introduce a second event queue, semantic owner or
presentation correlation table.

## Complete gate and receiver follow-up

The complete manifest-owned command runs serially with one Cargo build job:

```powershell
$env:OPEN_GPUI_NATIVE_INTERACTIVE = '1'
$env:CARGO_BUILD_JOBS = '1'
cargo run --locked -p xtask -- native-windows-interactive
```

The log is `%TEMP%/open-gpui-windows-gate-followup-20261115.log`. This execution
finishes with exit code 1. Its baselines pass: UI contract 16/16, system-input
sentinel 1/1, Windows backend 323/323 (10 ignored), scenario registry 1/1, and
native example 34/34 (12 ignored). The example baseline run is
`47f75952-aaf4-486a-97d0-f1c206a929ac` (5.847 s). These are different test
populations; their totals must not be added to the manifest result.

Three of fifteen manifest rows pass in that execution:
`native.u27.opaque-hit-transparent-prefix`,
`native.u28.client-geometry-reconciliation`, and
`native.u28.event-driven-geometry-wake`. The other twelve rows fail. This is
not Windows docking qualification. In particular:

- Process convergence passes its `LastWindowClosed` subcase. Its `AppShutdown`
  worker exits with `0xC0000005` before publishing a census. The earlier phase
  log ends at `scenario.running`, so it does not localize that crash to the
  final shutdown call. Additional bounded process phases now distinguish root
  opening, readiness, capture, primary retirement and final exit request.
- The complex workflow exposes a real adapter protocol defect: an exposed
  framework background returns `EmptyLane` for `HoverHit` even though the core
  derives a dynamic `TabGap` drop target after earlier pointer edges. Framework
  exposure proves that hover is clear of occlusion; it cannot prove semantic
  emptiness. The receiver now returns `ClearHoverLane` for that precise exposed
  background. Click and drag delivery remain `EmptyLane`, external occlusion
  remains `Blocked`, and missing presentation authority remains `Unknown`.
- Source-capture fails its original input canary before drag activation. The
  requested and observed cursor coordinates differ and original WndProc records
  contain additional mouse moves with `extra_info=0`. A separate platform row
  sees an unmarked Backspace key-up. These are retained input-provenance
  failures, not passing release or activation receipts.
- Open-desktop probes encounter existing Qt and Chrome windows on both
  monitors. Mixed-DPI fixtures reach their selected-display checks but do not
  establish final target geometry. Other failures, including the no-input tab
  selector and activation timeouts, still require their own diagnosis.

All preserved failed docking workers except the access-violation worker publish
their original reports and complete RELEASE. Their App and process HWND
registries are empty, capture is released, native exit authority is settled,
and every observed native generation has a terminal. Failure cleanup is distinct
from scenario success.

The process fixture also fixes a separate observation-ownership error. Building
each `SharedSession` previously installed a replacement App-wide ordinary drag
observer; opening the survivor displaced the primary session's journal. One
private App global now retains the observer and shares its event journal among
the fixture's sessions. Each drag still filters by source and its fresh cursor.

After the receiver fix, a fresh nonce-bound manifest invocation passes the
process-convergence row, including both `LastWindowClosed` and `AppShutdown`,
in run `998a222f-2d60-4238-83c9-6d3f6f6a8b7c` (3.533 s). The original parent,
Job, pre-exit census and RELEASE protocol all remain in use. The log is
`%TEMP%/open-gpui-windows-process-phase-check-20261115.log`. A subsequent pass
does not establish the cause of the earlier native access violation. This
fixture also still shuts down App only after retiring the primary group;
active-capture/provisional/pending-open AppShutdown remains unqualified.

The receiver regression exercises a real native-host-frame press and move,
an original submitted artifact and the committed GPUI hit graph. Restoring the
old background answer alone makes it fail with `ReceiverWinnerMismatch` in
`HoverDrop`, claiming no receiver against the core's `SurfaceBackground`
`DropTarget`. That mutation run is `f6ea12e9-2ba2-45de-9c73-e4a792f12d77`;
the original native failure involved `TabGap`, so the regression establishes
the same protocol error without claiming an exact replay of its geometry.
The fix is restored afterward. The focused three-package run passes all 181
tests, including both capture generations, failed-release quarantine, display
publication, native capture observations and all receiver tests. Logs are
`%TEMP%/open-gpui-windows-hover-red-20261115.log` and
`%TEMP%/open-gpui-windows-hover-green-20261115.log`.

A fresh source-capture run after this fix still stops before activation at the
original WndProc canary (11.451 s): requested `(1099, 711)`, observed
`(1075, 800)`, with unmarked mouse moves. It publishes a clean 2/2 native
generation census and completes RELEASE. The log is
`%TEMP%/open-gpui-windows-source-hover-check-20261115.log`. No input assertion,
desktop requirement or timeout is relaxed, and no other application is closed.
The natural cross-HWND release still needs an uninterrupted physical run.

The next implementation slice remains the provisional native-window owner and
its one-time adoption by the existing factory. Keep the reserved target surface,
same HWND, final physical placement and exact semantics/submission ticket joined
through first-live admission. Then extend the original process, mixed-DPI,
committed-loss and workflow oracles; preserve the legacy runtime until those
replacement behaviors actually execute.

## Fixture readiness and reentrant native destruction

The no-input selector failure exposes an insufficient readiness predicate.
The model correctly retains selected item 102 in the target surface. A native
window can submit a non-empty bootstrap background while its docking packet is
still deferred, without any committed tab selector. `SharedSession::wait_ready`
now requires the existing read-only inspection's non-deferred packet and exact
current presentation for each root, plus a unique positive-area committed tab
selector. Visibility and original non-empty Submitted checks remain. Missing
selectors keep waiting; duplicates fail. A timeout retains the actual inspection.

That correction moves the native no-input run past root readiness to an actual
point-stack failure. Comparison with the original worker finds another omitted
prerequisite: `set_accepts_pointer_input(false)` returns a queued mutation. The
shared worker now waits for both original overlay policies and native visibility,
acquires the lower target's foreground, then raises both overlays. It checks their
actual Z-order, coverage, `HTTRANSPARENT` and lower-HWND hit independently.
Run `917775c7-d30b-451b-9fef-f7999de5dba5` passes this current worker (2.256 s),
including captured movement/release and the original census/RELEASE protocol.
The logs are `%TEMP%/open-gpui-windows-no-input-readiness-20261115.log` (failure)
and `%TEMP%/open-gpui-windows-no-input-policy-20261115.log` (pass).
The shared worker still lacks the original generation-drift and external opaque
terminal probes. Its passing test name does not establish those unexecuted
oracles; they remain required before claiming full replacement qualification.

A later process-convergence run reproduces `0xC0000005` in both subcases. Its
bounded phases place one crash after the final last-window request and another
after capture during primary-group retirement, before AppShutdown itself. This
supersedes an interpretation of the earlier single passing run as crash closure.
The log is `%TEMP%/open-gpui-windows-process-readiness-20261115.log`.

ProcDump attaches outside the worker Job after the real capture phase. A temporary
750 ms diagnostic rendezvous permits attachment without interfering with the
initial input canary; it is removed before final validation. The original parent,
nonce and Job protocol remain. A first-chance access violation dump is captured
at 15:12:44 in the AppShutdown subcase's primary retirement. Existing local SDK
`cdb` resolves the original binary's symbols without installing another debugger.
The diagnostic artifacts remain outside the repository at
`%TEMP%/open-gpui-native-process-dumps-20261115/`.

The exception reads address `0x0000000200000043`. The resolved stack is:

```text
Option<AttachedWindowState>::is_some
WindowsWindowInner::is_attached
WindowsWindowInner::handle_msg + 0x1460
window_procedure_inner
window_procedure
DispatchMessageW
WindowsPlatform::run
```

The ordinary WndProc borrows `&Rc<WindowsWindowInner>` directly from its
`GWLP_USERDATA` Box. A destructive callback can reenter `WM_NCDESTROY`, releasing
that Box, before `handle_msg(self: &Rc<Self>)` performs its post-callback
`is_attached()` check. The outer method therefore reads through a freed Rc slot.
The owned-window dispatch now clones the Rc into its own stack slot before any
callback. It retains both the receiver and the Rc storage through dispatch,
matching the existing subclass path. No native terminal or renderer/capture
authority is fabricated or deferred by a timeout.

A new real-HWND regression enters `WM_CLOSE` through `SendMessageW`, destroys
the native window synchronously inside its callback, and checks that dispatch
owns an independent receiver before entering that destructive callback. It also
checks native destruction, receiver survival through callback return and final
owner retirement after logical cleanup. The old borrowed-slot code fails the
specific independent-owner assertion in run
`7b3d3325-e2dc-408f-bcbd-7e2946f23c48` (0.951 s). This reproducer needs neither
system pointer input nor random heap reuse. The red log is
`%TEMP%/open-gpui-windows-reentrant-dispatch-red-20261115.log`.

With the owned dispatch receiver, serial run
`248f049d-ec43-4a5d-a7a9-3922d928705c` passes all 501 selected tests in
149.365 s. The selection includes the complete non-ignored Windows library
suite and the adapter, pointer-session and App display suites from the same
default-feature three-package build. The new destructive-close regression
passes in 0.955 s; 1,675 other tests remain outside that selection. The log is
`%TEMP%/open-gpui-windows-reentrant-close-green-20261115.log`. Independent
read-only correctness review finds no P1/P2 issue in the dispatch receiver,
receiver answers or fixture readiness changes. It does not establish a separate
panic-recovery reentrant-destruction test; that branch rereads its current slot
after unwind and is not the pointer retained by the reproduced normal dispatch.

The final native-example rebuild passes its complete ordinary baseline again:
run `93faa6ca-1510-4478-ac0b-5ce408b42aa2`, 34/34 (12 ignored), 6.233 s.
The log is `%TEMP%/open-gpui-windows-native-baseline-final-20261115.log`.
Three fresh manifest-owned parent invocations then run serially on the final
binary, with independent receipt tokens and no debugger or rendezvous pause:

| Behavior | Final focused result | Limit |
| --- | --- | --- |
| Process convergence | `LastWindowClosed` passes; `AppShutdown` fails waiting for initial source native capture. Run `abfe0e8c-84bd-40fe-9d8a-2f94e7635a79`, 13.653 s. | The AppShutdown worker does not reach final shutdown admission in this run. No access violation occurs; this is not a passing complete row. |
| Source capture | Original WndProc canary fails before activation. Run `e3ed87b8-b62d-4771-b7ba-514e56642bc9`, 11.374 s. | Requested `(1099, 711)`, observed `(1403, 892)`, different foreground/root HWND and additional unmarked moves. No final logical drop assertion is exercised. |
| No-input pass-through | Current worker passes. Run `66e73e54-1541-4521-88a7-32912d08df30`, 2.185 s. | This still does not execute the missing generation-drift and opaque-terminal oracles. |

The logs are `%TEMP%/open-gpui-windows-final-<behavior>-20261115.log`.
Both failed workers publish their original nonce-bound census, complete RELEASE,
leave empty App/top-level/GPUI-message HWND registries, settle native exit and
release capture. Process convergence observes and retires 3/3 generations;
source capture observes and retires 2/2. These final runs reproduce no
`0xC0000005`, but the initial capture timeout remains separately unresolved.
The full fifteen-row gate is not rerun after these focused changes, and no
focused result is substituted into its earlier three-of-fifteen receipt.

Final `cargo fmt --all -- --check` and `git diff --check` pass. Temporary debug
instrumentation is absent. The task's native workers, ProcDump/cdb processes and
subagents are closed. No dependency/reference pin, manifest row, input oracle,
worker deadline or legacy-runtime qualification requirement is relaxed.

## Provisional physical owner and same-window adoption

The shared default factory now connects the core's Native preview to a GPUI-owned
provisional window. `element/recording.rs` retains the original preview identity
and outer placement in its packet. Candidate acceptance publishes that visual
with the captured source/generation and original physical display route. The
private `host_native/provisional.rs` owner creates a non-interactive visual shell;
it does not register the reserved surface as a root or invent a target paint
stream before release.

The original native FIFO reports each accepted release with its full
`NativeCapture` identity and optional physical frame. Only the matching opening
retains that packet. This replaces an intermediate global last-release sidecar,
which could associate a later drag's coordinates with an earlier create. Ordinary
local input invalidates old captured desktop authority. A release, idle gesture,
capture-generation change or adoption also revokes its cached preview.

When the create effect's surface matches the reserved target, the default factory
installs its original callbacks on the existing GPUI Window and HWND. It retains
both the final placement mutation and provisional native placement receipt.
Destination semantics share `semantics_ticket.frame_submission()` with the
ordinary pending docking output. Interaction opens only after that exact renderer
submission and the core's original `Admitted` report. A withdrawn candidate can
reinstall the same submission authority; it cannot substitute an ordinary ticket.

Unadopted previews participate in anchor retirement dependencies and the workspace
quiescence predicate. Closing retains GPUI's original presentation-shutdown ticket,
including independent renderer quiescence and native terminal. A borrowed Window
retains a close intent for retry, including when the borrow conflict occurs inside
the create factory itself. Failed opening keeps the actual construction rollback
ticket. Installing an application custom factory opts out of the default preview
and retires an outstanding shell on the next pump, preserving the custom factory's
creation ownership. The adapter does not silently bypass that factory.

GPUI exposes hidden, authority-preserving cross-crate operations to retain/query
the existing retirement ticket, compare exact submission identity, and resume the
same destination-semantics submission after withdrawal. Deterministic test support
can explicitly publish a complete physical display roster and hold renderer
shutdown after a logical Window has left the registry. Neither seam changes
production native observations or defaults an unavailable roster to known.

Five new lifecycle regressions exercise:

- Same-window adoption with renderer Deferred, the same submission reinstalled
  after explicit ticket withdrawal,
  and interaction remaining gated until first-live admission.
- Separate renderer shutdown and native callback delivery while the anchor waits.
- Custom factory ownership and retirement of its displaced default preview.
- Close retry after a temporary Window borrow.
- A real affine create effect dispatched inside the already borrowed shell,
  retaining cleanup custody through the failed adoption and eventual shutdown.

The existing two-natural-release regression additionally checks both original
source/generation/physical-frame packets after both generations accumulate behind
one suspended semantic update. Independent read-only follow-up confirms closure
of the identified withdrawal, retry, cross-generation release and stale-preview
findings; that review is source evidence, separate from execution.

Final executed results, all serial with one Cargo build job and one test thread:

| Check | Result |
| --- | --- |
| Full adapter plus GPUI provisional, exact-submission and display cohorts | 143/143 pass, 1,704 filtered; run `4b0cb6fa-0647-4a36-8998-00a75325b065`, 6.563 s. |
| Native example nonignored baseline with `legacy-docking` | 34/34 pass, 12 ignored; run `30a2efa0-f8b2-4183-a622-a9eec318bc4d`, 6.128 s. |
| Fresh manifest-owned `provisional-same-hwnd-promotion` parent/worker | Fails before drag activation at the original pointer canary; run `3bebf043-4590-4615-a6de-07e996cdc06f`, 11.372 s. |

The final physical run requests `(1099, 711)` and observes `(1720, 197)` with a
different foreground/root HWND. It does not reach pre-release reveal or adoption.
The nonce-bound worker still completes RELEASE, reports renderer/session closure,
observes and retires 2/2 native generations, releases capture, settles native exit
and leaves empty App/top-level/message-only HWND registries. No access violation
occurs. An earlier invocation of the same row also fails before activation, with
requested `(1099, 711)` and observed `(2063, 651)`, run
`6bdd622e-6406-4ae0-b628-53627608fdb2`, 11.346 s. The canary is not weakened and no
passing physical same-HWND or mixed-DPI result is claimed.

Final logs are `%TEMP%/open-gpui-provisional-regression-final-20261115.log`,
`%TEMP%/open-gpui-provisional-native-baseline-20261115.log`, and
`%TEMP%/open-gpui-provisional-native-final-20261115.log`. The earlier diagnostic
native log is `%TEMP%/open-gpui-provisional-native-20261115.log`.

Physical reveal, live routing, mixed-DPI final expected client geometry,
active-capture/provisional shutdown, and unexpected committed-destination loss
still require their declared native oracles. The complete fifteen-row gate is
not rerun here; its earlier three-of-fifteen receipt remains unchanged. Legacy
runtime removal remains unqualified. The dependency pin, reference checkouts,
manifest and physical-input deadlines remain unchanged.

`cargo fmt --all -- --check` and `git diff --check` pass. The manifest SHA256
remains `e0a4e783b64b94d0bf767af74e39eeb41082aebe7b883182a26bc84496847b92`.
This follow-up is uncommitted; HEAD remains the revision in the frontmatter.
Its native workers and four read-only review agents are closed. Other local
Cargo activity is left untouched.

## Post-admission native loss and shared dock-back

The next continuation adds a real Windows recovery journey that does not inject
system pointer input. It issues the public
`DockspaceHostFrame::move_to_new_window_current` command, waits for the exact
destination binding's `Admitted` and original `Submitted` evidence, and checks
the current non-deferred presentation of a stateful retained View. The physical
committed-loss worker uses the same post-admission oracle after its captured
tear-off; it no longer substitutes ordinary `WM_CLOSE` for unexpected loss.

The private example module `native_interactive_tests/shared_recovery.rs` owns
the shared oracle. `SharedSession` retains the typed source entity, and that
entity records the actual Window and local state used for rendering. Each
checkpoint also checks committed content bounds, current docking presentation
and the real renderer's nonempty submission. Reading an entity clone alone is
not accepted as proof that the adapter rendered that View in the right Window.

The Windows test-support helper
`native_test_retire_window_without_close_request` resolves the current HWND on
its owning UI thread, checks its full GPUI WindowId and rejects attached windows.
It uses the backend's original presentation-shutdown ticket and real native
destruction. It does not post a close request, remove the logical Window, or
publish a synthetic terminal. This is native loss to dockspace with a correctly
quiesced renderer, not a claim about arbitrary external destruction while the
renderer is borrowed. A mismatched Window/HWND pair is rejected in the real
Windows regression.

An initial executed test incorrectly required immediate contained recovery.
Run `e0ae69e0-4217-48d7-95a9-1f27a91b3651` fails that predicate in 12.080 s:
the original HWND is terminal, but core admits a new native binding for the
same surface and root. Source inspection and independent review establish
that this is an allowed recovery path. At the fixed dependency revision,
`engine/surface_runtime.rs` preserves a blocked recovery obligation;
`platform/frame.rs` can request an exact-placement native replacement, and
`engine/native_admission.rs` updates the obligation's binding only after the
replacement's first-live admission. The execution does not identify the
specific missing contained-recovery fact, so no adapter defect is inferred
from replacement alone.

The corrected oracle requires the original binding's true terminal and
retirement before accepting either core-owned outcome: the same root contained
on its source recovery surface, or the same surface/root under a different,
currently mapped and admitted native binding. It then changes the original
View's local state from 41 to 42 and requires that state to be presented by the
automatic recovery owner before submitting any recovery command. A replacement
uses the shared presentation command to dock back to the source contained root
and retire its own HWND. Both outcomes then use the contained Dock Back command
to merge into the original source main root; the former tear-off root must be
absent and the original item selected. The source and target HWNDs and all three
pane occurrences survive.

Original loss and replacement retirement each retain their own native identity
and ticket generation, with
`PresentationQuiesced < DestroyEntered < WM_NCDESTROY < NativeTerminal`.
The loss has neither a `WM_CLOSE` observation nor a semantic close request.
No adapter layout writer, legacy recovery registry or forged presentation
acknowledgement is introduced. The first corrected real Windows run,
`39a088af-79c8-4378-94ab-0bae7f3829d1`, passes in 2.272 s.

The expanded serial cohort passes 38/38 in 11.645 s, run
`725fc410-2e72-4f28-aeb2-6a99fbee57ed`: all 35 nonignored native-example tests
plus three Windows regressions for reentrant close ownership, borrowed-renderer
quiescence rejection and retry after failed destruction. The new recovery test
takes 2.224 s. Its checkpoints identify the replacement branch, then Dock Back;
all 4 observed native generations reach terminal. The process window census
equals its pre-application census after App/platform retirement.

A final review removes post-pump raw `IsWindow(old_hwnd)` predicates from the
oracle: Windows may reuse that numeric value for a new owner. The full original
WindowId/binding must retire, and its exact native generation and original
shutdown authority must independently prove terminal. Synchronous backend
destruction and the complete native lifecycle assertions remain required.
The final targeted run after that refinement passes in 2.296 s,
`59166335-8bd0-4def-9f0f-1cbaeb57f5bd`, again recording the replacement path
and 4/4 terminal native generations. No Rust source changes follow that run.

The final focused command is:

```powershell
$env:CARGO_BUILD_JOBS = '1'
cargo nextest run --locked -p open-gpui-docking-native -p open-gpui-windows `
  --features open-gpui-docking-native/legacy-docking `
  -E 'test(shared_recovery::real_windows_committed_destination_loss)' `
  --test-threads=1 --no-fail-fast --no-tests fail --success-output final
```

Logs for this continuation are
`%TEMP%/open-gpui-provisional-native-continue-20261115.log`,
`%TEMP%/open-gpui-committed-loss-native-first-20261115.log` (the initial
contained-only red test),
`%TEMP%/open-gpui-committed-loss-native-recovery-20261115.log`,
`%TEMP%/open-gpui-committed-loss-final-cohort-20261115.log`, and
`%TEMP%/open-gpui-committed-loss-final-generation-20261115.log`.

A fresh manifest-owned same-HWND attempt in this continuation is refused before
drag activation because the physical primary button is already pressed:
`71d847fb-d35e-4b87-b41b-b07bb6caa53c`, 1.419 s. Its nonce-bound parent/worker
still completes RELEASE and reports 2/2 native generations terminal, released
capture, settled App exit and empty process window census. No additional
global-pointer retries are used. The manifest, its deadlines and canary remain
unchanged.

This post-admission recovery test does not qualify physical preview/adoption,
mixed-DPI final placement, active-capture App shutdown, or the earlier legacy
pre-first-live loss plus real `Restore` control journey. Accepting both recovery
outcomes also does not establish that the contained branch executed in a
particular run. The complete fifteen-row gate retains its earlier receipt, and
legacy-runtime removal remains unqualified.

Final `cargo fmt --all -- --check` and `git diff --check` pass. The native
manifest SHA256 remains the value recorded above. Both read-only research/review
lanes are closed; the bounded follow-up reports no remaining actionable finding
in its inspected recovery scope. Its last two generation-predicate refinements
are verified by the final executed regression rather than credited as a fresh
independent source review. No task-owned native worker or debugger remains.
This continuation is uncommitted and leaves the earlier workspace changes intact.

## Target-DPI placement and original caption anchors

The next continuation inspects the locked core's tear-off geometry and the
Win32 reference backend. Core's `solve_tear_off_geometry` scales the source
cursor offset at the target display and clamps the outer frame into that
display's work area. The original pointer can therefore lie on a native
caption outside the resulting client rectangle. ImGui's Win32 backend also
keeps client coordinates distinct from decoration conversion and supplies the
viewport's DPI to `AdjustWindowRectExForDpi`. This supports preserving the
original pointer and target DPI across conversion; it does not transfer
ImGui's ownership model into GPUI.

The executed GPUI regression
`display_bound_placement_preserves_a_caption_anchor_outside_the_client` fails
before the fix, run `952f7e3b-ce7c-4a05-9377-4b1fb2fca78d`: outer
`(-1600, -200, 824, 657)`, client `(-1588, -155, 800, 600)`, and original
anchor `(-1560, -190)`. The point is inside the native caption and selected
display, but the constructor incorrectly requires it to be inside the client.
The fix removes that client-containment requirement while retaining positive
client size, checked coordinate overflow, and exact target-display ownership
of the point. Both LiveRoute and FinalRelease requests retain the original
anchor. Windows still checks that the requested outer frame contains the
anchor before preparing the point-scoped opaque-barrier band. No client-center
substitution or relaxed native barrier proof is introduced.

The physical provisional oracle also needs stronger geometry and identity
predicates. A preview and final rectangle can legitimately be equal when
release occurs at the reveal point; inequality is not placement evidence.
The previous window-count predicate also omitted the fixture's two existing
roots. The revised oracle checks the original final-placement receipt,
complete create/admission binding, target display and scale, exact native and
GPUI client geometry, and the original core outer intent independently. Hit
testing after promotion uses the final client rectangle. The retained source
View carries state 41 and must actually render at the admitted destination
with a current exact input binding and presentation. Both original roots must
retain their WindowIds, native generations, bindings, main roots, client/outer
geometry and display observations without a terminal event.

Before mixed-DPI release and at each distinct live-route checkpoint, the
oracle now waits for the existing provisional owner's original LiveRoute
ticket, outer intent, target-DPI native geometry and transparent hit test
while the source still owns capture. The test-support inspector borrows that
owner directly and has no separate history or mutation authority. Adopted
callbacks retain the original core create request and final ticket; they do
not mint a new receipt or acknowledge presentation.

The additional ignored real Windows test
`real_windows_shared_native_placement_matches_target_dpi_in_both_directions`
uses the public semantic tear-off command and a custom factory that opts out
of default preview. It captures the original request and the actual owning
backend's outer-to-client conversion before returning from the factory and
before bridge placement. It requires lower-to-higher and higher-to-lower DPI
creation, an actual negative-coordinate target rectangle in one direction,
exact client and independent outer geometry, the original View's first-live
presentation, unchanged original roots, and one new native generation per
direction. Missing hardware is a failure when explicitly selected, rather
than a silent skip. A small returning-application harness is shared with the
post-admission loss test so both retain panic-safe session/App cleanup, native
terminal observations and the pre/post process window census.

The intermediate serial cohort passes 65/65 in 10.525 s, run
`a1b16ed8-0093-44da-a5b6-da092edff820`: GPUI display-bound/provisional request
tests, provisional adoption tests, Windows target-DPI/frame conversion tests,
and all 35 nonignored native-example tests. The caption regression is green.
The migrated post-admission loss test retains the replacement branch and 4/4
terminal generations. `cargo fmt --all -- --check` and `git diff --check` pass.

The first explicitly selected hardware run,
`50326170-dc2a-49ed-8656-e0da0b176a85`, fails in 1.834 s before destination
admission with `unknown logical surface`; subsequent session cleanup rejects
a retained presentation stream. All 3 observed native generations reach
terminal and the separate App cleanup succeeds. The failed session-cleanup
assertion prevents crediting the later process-census assertion in that run.

Source diagnosis identifies a managed factory contract defect, rather than a
coordinate mismatch: only the default factory's root builder inserts its
original callback route in `ManagedHost.callbacks`. A custom factory's real
docking element consequently has no managed route proving that its surface
is the core's pending native staging surface. The fix moves route publication
to the shared factory dispatch. During a potentially reentrant factory call,
the managed host temporarily retains the same original callbacks; prepaint
resolves them by the full installed WindowId before validating the surface.
After factory return, the common dispatch publishes the route before bridge
placement. The default-only insertion is removed. `NativeCallbacks` itself
continues to avoid borrowing `SharedHost`, and no second root registration,
surface, binding, layout writer or acknowledgement is minted.

These focused results do not qualify the physical mixed-DPI or same-HWND rows.
Fresh execution receipts after the common factory-route fix follow below.

The expanded intermediate cohort passes 86/86 in 11.825 s, run
`e8616263-2c4e-4b0c-b2ca-3b514a8230a1`, including all managed-host tests and
the new staged custom-factory regression. That regression observes the first
real root render before factory return, keeps the original source item in place
while placement/submission is deferred, then admits the original binding once,
retains the same stateful pane and drains its native lifetime.

The next hardware run, `51b68c19-0b2c-4445-aca9-a16c4aebc81c`, reaches the
125%-to-150% geometry checkpoint: requested/observed outer
`(1560, 774, 720, 540)`, pre-mutation and actual client
`(1571, 819, 698, 484)`, the original View presented, and both roots unchanged.
It then fails session shutdown with the retained-stream invariant. All 3 native
generations are terminal and separate App cleanup succeeds; the reverse
direction and final census are not executed in this failing test.

This exposes a distinct group-lifetime ordering gap. In this fixture the
tear-off's SOURCE recovery host is a non-anchor application root, while TARGET
is the group's final retirement anchor. The old bridge closes every non-anchor
in one loop, including SOURCE and the native child. The one-root headless test
uses SOURCE itself as anchor and therefore does not cover that relationship.
The locked engine confirms that recovery needs the registered SOURCE to remain
routable with coordinates and a ready candidate across a committed boundary;
the global TARGET cannot substitute for it. Recovery records do not directly
own presentation emissions, so this does not establish which emission holder
caused the particular stream serial to remain retained.

The staged drain now retires factory-created children before application
roots, retaining roots until the original child bindings are permanently
quiescent and removed from the bridge. Non-anchor application roots also
declare the real child-terminal dependency to GPUI. The regression topology
is extended to SOURCE plus a distinct global TARGET anchor and a delayed child
terminal. Fresh receipts after this ordering change are recorded below.

The staged-drain cohort passes 93/93 in 11.388 s,
`0ddba41a-603b-486e-a38d-31df5ba0262b`, including the two-root delayed-terminal
test. Hardware run `89df788d-6ef7-4eef-ab96-2ad061256860` still fails after
the same successful first-direction geometry checkpoint. This rules out
crediting staged close order alone as the retained-stream fix.

A failure-only snapshot before fallback App cleanup makes the boundary
visible. Run `5cf82f80-e852-44e4-97f6-52ec8273b58c` reports all 3 native
generations terminal, no registered Windows, no mounts, and no pending
presentation outputs. The model still contains the three preserved logical
surfaces; one application-root binding and a native FIFO suffix remain. Child
terminal and SOURCE terminal are already delivered, while the final TARGET
terminal has not crossed the failing committed-prefix reclamation. The
snapshot establishes a logical cleanup failure after real native retirement.

Pinned-engine inspection identifies a legal host fact that the adapter has
omitted: a removed measuring/rendering mount makes its retained logical
surface unavailable. A per-surface `defer_surface` contribution with
`SurfaceUnavailableReason::SurfaceUnavailable`, successfully committed before
permanent binding quiescence, demotes the old Ready scene and clears its
interaction authority. `complete_unpainted_surfaces` would preserve a Ready
contribution and cannot substitute for this fact. No raw presentation
observation or native terminal is synthesized.

The adapter now sends actual mount removal through the original
`NativeTransport::presentation` barrier alongside renderer terminals. It
commits the preceding native prefix, applies unavailability only to still
rostered surfaces without a currently attached owner, and commits that fact
before permanent binding quiescence. A different current owner suppresses
stale removal. Ordinary prepaint rejection does not report mount loss: that
path can still own a valid previously displayed scene. A regression holds the
original native terminal, removes the real Window, and requires the former
scene's presented authority to be absent while the original layout and View
remain. Fresh execution receipts follow below.

The complete focused cohort after that correction passes 94/94 in 11.890 s,
`959e05e1-1fdc-468d-9303-941a8e8af82c`. The held-terminal logical-removal test
passes, as do the existing rejected-prefix/exact-output tests, two-root
factory/retirement test, and real post-admission recovery. A broader run of
every shared adapter test passes 95/95 in 5.005 s,
`806567c7-05c2-4f7e-b9fe-b16c700786cf`.

The real mixed-DPI test now passes both directions in 3.190 s,
`a4b0d9e8-f231-45de-89d2-75eea51d285f`:

| Direction | Original and observed outer frame | Pre-mutation and observed client |
| --- | --- | --- |
| 125% to 150% | `(1560, 774, 720, 540)` | `(1571, 819, 698, 484)` |
| 150% to 125% | `(-1640, 953, 720, 540)` | `(-1631, 991, 702, 493)` |

Both directions retain the original stateful View at the exact admitted
destination and keep both original roots unchanged. Each direction separately
records 3/3 native generations terminal, successful session and App cleanup,
and the same process-window census after returning from and releasing its
Application. The second direction executes on the actual negative-coordinate
125% display. This establishes the custom-factory semantic native-placement
journey through full cleanup; it still does not qualify physical pointer
delivery, default preview, LiveRoute or same-HWND promotion.

The mount-unavailability correction resolves the executed retained-stream
failure without changing the dependency revision or weakening its ledger
invariant. A source review also refines the scope: mount-removal barriers are
ordered where actual cleanup enqueues them, not retroactively at every earlier
LogicalClosed record. Existing FIFO heads remain authoritative. A settlement
retry flushes its committed report into the bridge before collecting later
records. Unavailability clears only after a successful complete commit.

The main logs for this continuation are:

- `%TEMP%/open-gpui-caption-anchor-red-20261115.log`
- `%TEMP%/open-gpui-mixed-dpi-cohort-20261115.log`
- `%TEMP%/open-gpui-mixed-dpi-native-directions-20261115.log`
- `%TEMP%/open-gpui-mixed-dpi-factory-final-cohort-20261115.log`
- `%TEMP%/open-gpui-mixed-dpi-native-directions-final-20261115.log`
- `%TEMP%/open-gpui-mixed-dpi-drain-cohort-20261115.log`
- `%TEMP%/open-gpui-mixed-dpi-native-before-app-cleanup-20261115.log`
- `%TEMP%/open-gpui-mixed-dpi-unavailable-verified-cohort-20261115.log`
- `%TEMP%/open-gpui-mixed-dpi-shared-adapter-20261115.log`
- `%TEMP%/open-gpui-mixed-dpi-native-directions-unavailable-20261115.log`

A fresh complete manifest-owned gate is then started sequentially with the
manifest and existing pointer canary unchanged. Its result is recorded below.

Its federated UI contract registrations and declared accessibility/gallery
cases pass. The Windows system-pointer/capture runner sentinel passes in
1.350 s, `afee4b8f-3c8f-4e80-ac92-7efac1e4a341`. The standalone Windows
platform baseline passes 324/324 in 142.552 s,
`0b8afd1a-ef4c-491b-aa7e-680cf72ac53e`, with 10 explicitly skipped tests.
These prerequisites use the gate's own package/feature selection and do not
replace the following fifteen manifest-row receipts.

The complete gate finishes with 7/15 manifest rows passing. It is a new receipt,
not a rewrite of the earlier 3/15 run:

| Manifest scenario | Result | Nextest run |
| --- | --- | --- |
| `native.u27.source-capture` | Fail: source item did not commit into target | `37ff3af0-6453-4bbd-9fcf-75b6924fdff6` |
| `native.u29.complex-dockspace-workflow` | Fail: first cross-window move did not commit | `fe547fd6-74b5-474c-9e3d-c7dff04a0827` |
| `native.u27.opaque-occlusion` | Fail: no shell-desktop release point | `a432aaf6-9289-4016-89b2-1d2f7415088c` |
| `native.u27.opaque-hit-transparent-prefix` | Pass | `8ca02545-e83e-4a68-9d89-6b1de8603498` |
| `native.u27.surface-shutdown` | Pass | `bdb40629-b516-4c5c-b164-66d85b8977cd` |
| `native.u29.provisional-same-hwnd-promotion` | Fail: no shell-desktop release point | `22fc687f-5449-4017-aef8-14b456a6f434` |
| `native.u29.live-route-and-release-lock` | Fail: no shell-desktop release point | `04b2b71e-2f9b-4680-9351-c498a31b5bcc` |
| `native.u29.committed-loss-recovery` | Fail: no shell-desktop release point | `edd55e5f-9877-4bc1-a922-c71d8b2e05d8` |
| `native.u28.activation-terminal` | Pass | `9c410e0a-b1ed-466a-9f5a-435697254dc1` |
| `native.u28.provisional-activation-z-order` | Fail: no shell-desktop point for disjoint provisional placement | `2d169f30-7c65-4c65-8c7a-a68859360a93` |
| `native.u28.client-geometry-reconciliation` | Pass | `d0ccf98b-9b15-4acc-8d68-d4ddcfd585f0` |
| `native.u28.event-driven-geometry-wake` | Pass | `27678adf-25fc-4a10-a427-50d1fa0f8071` |
| `native.u28.process-convergence` | Pass: LastWindowClosed and AppShutdown subcases | `c1ae5280-3fd7-42bf-8988-ea6d9b644ad6` |
| `native.u28.no-input-pass-through` | Pass | `8b594541-bce1-4561-92f7-c9063e85f606` |
| `native.u28.mixed-dpi-client-bounds` | Fail: neither direction finds its required shell-desktop point | `ce9e77d0-ae7e-4541-8da3-82e32fa7f46b` |

The failed worker receipts still prove their cleanup, capture release and
complete native generations; the open-desktop failures retain the original
native hit census naming real unrelated windows. No unrelated user windows are
closed, minimized or reclassified. The source-capture and workflow failures
do not establish whether target occlusion or another receiver/presentation
condition prevented commit; they require separate target-hit evidence.

The gate's final example baseline initially passes 34/35,
`ed446428-aca4-41bb-be57-b4d93d29282c`: its recovery test completes the
contained-recovery/Dock Back checkpoints and all 3 native terminals, but an
overstrict comparison in the new returning harness rejects a lazy process
message window. The existing manifest worker already permits precisely
`SystemUserAdapterWindowClass`, while requiring exact top-level/GPUI message
window restoration and rejecting every unknown added class. The returning
harness now reuses that same validator; the parent/worker live census equality
check remains unchanged. This does not introduce a new whitelist entry.

Fresh standalone execution identifies the actual remaining HWND as
`SystemUserAdapterWindowClass`. The example baseline passes 35/35 in 7.689 s,
`809059aa-d773-49d2-9db1-66500a9be2fe`, including contained recovery and
3/3 terminal generations. The standalone mixed-DPI hardware test then passes
both directions in 3.282 s, `e8633cd7-edf7-4bf2-8619-811f405f1ead`; each
direction records 3/3 native terminals and a valid process census under the
same established policy. Unknown message windows continue to fail validation.

The full-gate log is `%TEMP%/open-gpui-mixed-dpi-manifest-gate-20261115.log`.
The two final standalone logs are
`%TEMP%/open-gpui-mixed-dpi-standalone-baseline-final-20261115.log` and
`%TEMP%/open-gpui-mixed-dpi-standalone-directions-final-20261115.log`.
Its 7/15 receipt remains a complete-gate observation rather than being
retroactively changed by focused baseline success.

Read-only triage confirms a target-hit prerequisite gap in the two failed
cross-window drop workers. They previously proved the captured move arrived
at the source and then asserted item transfer, without proving that the real
native hit at release was the destination. Nonactivating target creation does
not raise it above unrelated windows. The full-gate log does not contain the
actual release hit for those two failures, so neither an environment cause nor
a semantic defect is inferred from their old messages.

The bounded fixture correction raises only the test-owned destination before
starting each drag, retains the source's foreground/capture acquisition, and
requires the native root hit to equal that destination immediately before
MouseUp while source capture remains owned. Release uses the same delivered
point. Existing item-transfer, selection and original release-terminal checks
remain. It does not alter opaque barriers or unrelated windows. A second
complete manifest-owned run verifies this change and the shared census
validator; its receipt follows below.

That second complete gate again passes 7/15 rows. Both drop workers pass the
new exact-target/source-capture prerequisite, then fail the original immediate
item-transfer assertion: source-capture
`b77cdc04-69c9-46eb-926d-7c11c2d9efaa`, workflow
`60285379-da9a-4d8e-84ed-0a785e5f1ba6`. Their failures therefore cannot be
attributed to an opaque unrelated HWND at the checked release point. Platform
baseline remains 324/324 in 140.491 s, and the final example baseline is now
35/35 in 7.717 s (`b8cc0937-058a-4c3b-b126-9a372b71de91`). The complete log
is `%TEMP%/open-gpui-mixed-dpi-manifest-target-hit-20261115.log`.

The drop workers also read the layout immediately after capture release.
Capture terminal is not the semantic/presentation terminal. A bounded
settlement check now waits, under the existing deadline, for the item on its
selected destination surface, the current exact native region binding, current
presentation, an actually painted committed tab and a real nonempty frame.
It pumps existing work only; it neither injects another release nor submits a
move command. A third complete gate verifies that change, with the same
manifest, native canary and deadlines.

The final complete gate again finishes at 7/15, with the same passed scenario
set as the first table. UI contract and runner sentinel pass; Windows baseline
passes 324/324 in 139.633 s (`295619ae-bcae-417d-ad6a-af95b4962b89`), and
the example baseline passes 35/35 in 7.757 s
(`81b6330c-dcfb-4fbe-ae08-25d87b08db7c`). Process-convergence again passes
both LastWindowClosed and AppShutdown,
`cd7dbab9-b87a-4942-8b44-19504edd8b97`; surface shutdown and no-input
pass-through also remain green. The final complete log is
`%TEMP%/open-gpui-mixed-dpi-manifest-transfer-settlement-20261115.log`.

Source-capture (`64d4093a-15b4-4958-991b-3c2d0167f64d`) and workflow
(`d2f72e10-2be4-4bb4-8451-b92c24920c98`) still fail the bounded transfer
settlement. Their final inspections establish current target presentation and
input binding, no active gesture, no pending semantic update/input/create or
presentation, an empty native transport, no host error, and the original item
still on SOURCE. The exact target hit was checked before release. This is an
unresolved real receiver/commit journey, rather than evidence of simple
post-release scheduling delay or unrelated-window occlusion. The next
implementation scope is the original captured ingress, WindowHit/Surface
receiver answers and the core's release decision at those exact generations.
No semantic action is substituted for the failed drag.

The other six failed rows still cannot establish their required shell-desktop
placement area: opaque occlusion, same-HWND promotion, live route/release lock,
physical committed loss, provisional activation/Z-order and physical mixed-DPI.
Their physical qualification remains outstanding. The successful custom-factory
bidirectional placement and programmatic post-admission recovery results do
not fill those physical-preview requirements. Legacy runtime removal remains
unqualified.

Final production changes preserve the dependency pin and manifest SHA256
recorded above. All scoped implementation/review agents are closed. The final
read-only mount-removal review reports no actionable finding after its retry
ordering correction. The last fixture refinements are verified by the final
manifest run. Work remains uncommitted, with pre-existing workspace changes
preserved.

## Native cross-window delivery capabilities

The next continuation reproduces `native.u27.source-capture` independently
through the manifest's exact test wrapper and its existing Job, worker nonce,
capture terminal and census protocol. The behavior receipt matches. Run
`aa353fe5-c549-4273-bc83-8b558ff99de1` fails the original bounded item-transfer
assertion in 11.621 s. Event-time evidence shows the correct original source
WindowId and drag generation, native source capture, the actual destination
HWND, coherent physical/client conversion, a rank-zero destination hitbox,
`WindowHit::Surface` and `ClearHoverLane`. A second instrumented run,
`7ac1dfc5-95d2-4e97-b382-ad31a7b67c7c`, reports the core's terminal
`Rejected(TargetUnavailable)` rather than a pending submission or target
occlusion. Its retained trace is
`%TEMP%/open-gpui-source-capture-feedback-20261115.log`.

The adapter starts its capability roster from `none_supported` but had omitted
`HoveredWindow`, `DesktopPointerPosition` and `AuthoritativeButtonState`.
The pinned core requires those declarations for physical cross-surface
delivery, independently of the correct facts carried by a particular event.
Its conservative capability check therefore clears the preview and cannot
deliver the release. The core remains the semantic owner of this decision.

`PlatformViewportCapabilities` now declares native physical desktop-position
and button-state support explicitly. Windows implements both contracts;
other production backends retain their default unsupported declarations.
`NativeBridge::gpui_capabilities` maps these declarations and native point-stack
support to the corresponding core capabilities. It supplies no pointer fact,
initial empty roster, capture transfer or semantic action. Individual missing
facts still remain unknown. The test platform advertises button-state support
only when its explicit state observation is available.

The behavior regression uses two GPUI test windows, committed hit graphs,
original native bindings and the managed transport to execute press, move,
cross-window hover and release. Before the fix,
`49881566-7b4f-4bb0-8f59-a6d01c74031a` fails with the same
`Rejected(TargetUnavailable)` and item remaining on SOURCE. After the fix,
`3db711ec-fc82-4155-bfac-b9523235b592` passes both the delivery case and an
independent case requiring the source to retain its item without authoritative
button-state support (2/2 in 0.164 s). This is a semantic/adapter regression;
physical qualification continues through the original manifest worker.

The user-requested reference comparison reads
`repo-ref/imgui/backends/imgui_impl_win32.cpp` at the fixed reference revision.
Its initialization declares `PlatformHasViewports` and
`HasMouseHoveredViewport`; mouse updates determine the hovered viewport
independently of the capture owner. The WndProc owns button edges and
`SetCapture`/`ReleaseCapture`, and `NoInputs` windows answer `HTTRANSPARENT`.
Its frame adjustment uses the viewport's target DPI with
`AdjustWindowRectExForDpi`, consistent with keeping the existing GPUI
target-DPI outer/client conversion separate from logical layout bounds.
These responsibilities support the existing GPUI split between the Windows
backend, callback/transport owner and shared semantic session. No reference
checkout is modified. All temporary `[DEBUG-drop-20261115]` instrumentation
is removed from the production source before green verification.

The real source-capture worker then passes in 1.528 s,
`1ec9ca4c-c77d-4a7d-9017-ef045d3bfdbc`, with its original behavior receipt
and cleanup protocol. The complete adapter library suite passes 99/99 in
4.539 s, `cee21557-7139-4be5-99da-bb9abf0136cc`.

The three-window fixture now retains the same stateful source View as the
two-window fixture. Both physical transfer workers seed its local state to
41 and require the original View's render owner, committed debug bounds,
current native binding and current presentation to follow the item. The
four-step workflow checks this in the first destination and again after the
same item moves to the third window.

The first strengthened workflow execution,
`56049a44-efc5-445c-ae68-bb45fd6ca6e6`, stops at the pointer canary with
untagged cursor movement away from the injected point. A retry gets beyond
the first transfer but loses the next capture. Failure-only inspection in
`9942555f-9e85-4428-bfa4-7ae4ff54bd65` identifies step 1, the original
second drag generation and `Cancelled(PlatformCaptureLost)`, with the first
item already moved to TARGET. Native cleanup still reaches 3/3 terminals.

This second defect belongs to fixture lifetime: `begin_captured_drag` created
a new system-pointer restoration guard for every step, then replaced the
previous guard after acquiring the new capture. Dropping the previous guard
correctly restores its saved empty capture owner, thereby releasing the new
generation. The fixture now reuses one guard for the whole journey and
restores the original desktop state only at journey cleanup. No platform
capture semantics or release oracle is changed. The failure-only snapshot
reads the existing last four drag events and current owners; it keeps no new
history or authority registry.

The strengthened four-step workflow passes in 2.302 s,
`708cd571-de54-4143-9666-ea511c4d1216`, with the original behavior receipt,
retained View state and native cleanup. The reproduction and correction
are retained in `%TEMP%/open-gpui-complex-workflow-route-facts-20261115.log`
and `%TEMP%/open-gpui-complex-workflow-guard-green-20261115.log`.

The following complete-gate attempt passes the federated UI contract but
stops at the original system-input sentinel,
`efb340bd-adf7-4fea-9532-63034553b246`: injected position `(1920, 1044)`
is observed at `(1569, 2001)`. Neither the Windows baseline nor the fifteen
manifest rows execute in that attempt. Its log is
`%TEMP%/open-gpui-cross-window-capabilities-manifest-20261115.log`.
It supplies no new complete fifteen-row result and does not weaken or
bypass the sentinel. A separate invocation uses the now-built artifacts
with the same gate and unchanged physical preconditions.

That invocation passes the UI contract and original sentinel
(`7351e6c3-93be-4e8c-a618-fad55546155f`), then stops at the Windows
baseline: `29d80dcb-cb53-4fa7-8403-9996dc1bd693` passes 323/324 in
155.540 s. The existing
`native_windowing_same_app_round_trip_renders_receives_input_retires_and_exits_once`
test times out waiting for its exact native Submitted/readback generation.
The fifteen manifest rows again do not execute. The cause is not inferred
from the earlier mouse-position failure. A focused unchanged follow-up,
`29c2a0d6-6cb7-4510-b9d3-c0362467b595`, passes in 4.944 s. No renderer
logic, deadline or readback oracle is altered, and the failed baseline is
retained in `%TEMP%/open-gpui-cross-window-capabilities-manifest-retry-20261115.log`.
The focused result does not retrospectively make that baseline pass.

The next full-gate invocation again passes the UI contract but its original
sentinel detects the primary pointer button already pressed at entry,
`cc6148d9-a83a-458d-9938-bc63bada93ea`. It fails under the existing
contaminated-runner rule, including that rule's existing best-effort release.
No Windows baseline or manifest row executes in this invocation. The log is
`%TEMP%/open-gpui-cross-window-capabilities-manifest-final-20261115.log`.
No further system-pointer injection is attempted after that failure.

The final standalone example baseline passes 35/35 in 7.914 s,
`a926299f-be82-466e-8413-812afb4a9a1c`. This includes the real programmatic
native-loss/recovery/Dock Back case and uses no system-pointer injection.
The final source-capture fixture's added stateful-View assertion still needs
its own execution on an idle input desktop; the earlier source-capture
pass and the strengthened workflow pass are retained at their exact scopes.

No complete fifteen-row result after the capability fix is claimed. The
previous completed gate's 7/15 receipt remains historical; neither focused
success nor an interrupted gate changes it to 9/15. The remaining physical
preview/promotion cases need exposed shell-desktop space, and the original
sentinel needs idle, released system input. The isolated readback timeout
also remains recorded rather than being explained away by the input failures.
The dependency pin, manifest, input canary and all deadlines remain unchanged.
Legacy removal remains unqualified.

## Authorized complete-gate retry

The next user-authorized retry invokes the unchanged complete manifest-owned
gate, with `OPEN_GPUI_NATIVE_INTERACTIVE=1`, `CARGO_BUILD_JOBS=1` and the shared
target directory. The preflight finds no remaining Cargo, xtask or native
scenario process. Its log is
`%TEMP%/open-gpui-cross-window-capabilities-manifest-retry2-20261115.log`.

The federated UI contract passes. The original system-input sentinel passes
in 1.037 s, `672645ba-03eb-4741-a65f-faf3cf16ec81`. The Windows baseline,
`f1445bc9-5296-47fd-9db0-984f137de117`, passes 323/324 in 143.845 s. This time
the exact native Submitted/readback round-trip test passes in 5.596 s. The
failing baseline test is
`clipboard::tests::external_text_replacement_refuses_stale_metadata_without_dropping_private_formats`:
the final public read at `crates/gpui_windows/src/clipboard.rs:605` returns
`None` instead of the expected `external 世界` text without stale metadata.
The baseline failure stops the gate before native registration, all fifteen
manifest rows and the example baseline. It is not a docking-worker failure.

The unchanged clipboard test passes its focused follow-up in 0.108 s,
`4f83de59-ec79-4796-8f01-8012c1dc5d0d`. Its log is
`%TEMP%/open-gpui-clipboard-baseline-focused-20261115.log`. The failed attempt
does not distinguish native clipboard-lock refusal, an external clipboard
change or an admitted-read defect. A passing follow-up neither establishes
the cause nor changes the failed complete-gate result. No clipboard behavior,
fixture, gate precondition or assertion is changed.

A separate exact source-capture execution then closes the outstanding final
stateful-View check. It uses the selected manifest row and the existing native
worker, Job, nonce, capture-terminal and HWND-census protocol. Run
`c97484ab-a512-421a-8863-296f13dd6417` passes in 1.529 s and its original
behavior receipt matches. The original View and local state 41 must render in
the destination with committed View bounds, current native binding and current
presentation. The log is
`%TEMP%/open-gpui-source-capture-final-state-retry2-20261115.log`.

This focused result qualifies the final source-capture assertion only. There
is still no complete fifteen-row result after the capability fix. The earlier
7/15 receipt remains historical, and physical preview/promotion qualification
and legacy removal remain incomplete. The manifest SHA-256 remains
`e0a4e783b64b94d0bf767af74e39eeb41082aebe7b883182a26bc84496847b92`.

## Clipboard fixture admission

The next continuation investigates the prerequisite failure before another
complete gate. The unchanged exact stale-metadata test runs serially for
100 iterations: `aea1a5ae-660f-43b6-a85c-c4d623b8a8f3` records 98 passes and
two original final-read `None` failures in 10.796 s. Nextest stress mode exits
successfully despite failed iterations in this version; the iteration summary,
not the process exit status alone, supplies the result.

A temporary test diagnostic at `OpenClipboard` then distinguishes actual
acquisition refusal. Run `c9a186e2-a80a-4a7b-a830-e9c4103c3754` records 97/100
passes in 10.817 s. One failure is the original final-read `None` with native
`0x80070005`, and another refuses the mutation-phase acquisition. The third
is a separate private-metadata read returning `None` after text replacement
inside an admitted guard. Its cause is not established by the acquisition
diagnostic and is not described as fixed lock contention.

Adding temporary `GetClipboardData` and `GlobalLock` failure diagnostics yields
`dc636b25-429b-4f91-b8d6-8a4617112a72`: 289/300 passes in 32.426 s. All eleven
failures report `OpenClipboard` refusal, including the original final-read
symptom. The separate admitted metadata failure does not recur in that run.
The traces identify no external clipboard owner and print no clipboard content
beyond the fixture's existing assertion values.

The clipboard unit fixtures now use the existing `retry_clipboard_open` for
ordinary setup, mutation-lock admission, final public read and post-contention
lock recovery. The normalization matrix uses it only to acquire its one guard.
The helper's existing one-second bound and native-acquisition observer are
unchanged: it retries only an actual `OpenClipboard` error and immediately
returns an admitted operation's original result, including `None`. Every
normalized value, retained private-format check and stale-metadata assertion
remains exact. The controlled hidden-HWND contention test still calls the
unwrapped public read while its other thread holds the native lock and must
observe immediate refusal. Production clipboard I/O is unchanged.

The corrected exact test passes 300/300 iterations in 32.052 s,
`50456895-9fe8-4079-bfa3-a4848f0c347d`. Ten iterations observe actual admission
refusals before success, totaling twelve refused acquisition attempts. No
admitted content result is retried. The single earlier private-metadata
failure remains recorded rather than being retrospectively assigned a cause.
All `[DEBUG-clipboard-20261115]` diagnostics are removed before the following
complete-gate attempt. The final clipboard Rust delta is inside the unit-test module.

The reproduction and follow-up logs are:

- `%TEMP%/open-gpui-clipboard-baseline-stress-red-20261115.log`
- `%TEMP%/open-gpui-clipboard-open-trace-20261115.log`
- `%TEMP%/open-gpui-clipboard-data-trace-20261115.log`
- `%TEMP%/open-gpui-clipboard-admission-stress-20261115.log`

## Independent gate-result aggregation

The first complete-gate attempt after the clipboard fixture correction passes
the UI contract and original system-input sentinel,
`fc3dce0c-8ef9-4f25-9c85-f0400f138de2` (1.051 s). Windows baseline
`201447b7-3eaf-4be7-934f-3d190c211db3` passes 323/324 in 142.514 s, including
all three clipboard unit tests and the previously failing native-windowing
round trip. The generic-view composition consumer fails after its initial
exact Submitted/readback receipt: the expected typed View has one pointer
input and one local input, but the next exact frame does not converge within
the original 512 delayed message-pump attempts. The gate stops before its
registry and all fifteen manifest rows. This failure is separate from the
clipboard acquisition defect and does not establish its own cause.

The unchanged generic-view test passes five focused iterations in 8.849 s,
`9c180ef5-42e4-453c-bf02-b85ed78b5962`. Neither the rendering implementation,
its exact-generation oracle nor its message-pump budget is changed. The logs
are `%TEMP%/open-gpui-clipboard-admission-manifest-20261115.log` and
`%TEMP%/open-gpui-native-view-baseline-focused-20261115.log`.

Repeated independent baseline failures have prevented the manifest from
producing any post-capability-fix row receipts. The runner now records the
Windows baseline in its existing `NativeGateReport`, like the rows and final
Dock baseline. A failed Windows baseline remains a failed step and makes the
gate fail after the independent rows finish. Manifest/UI-contract validation,
the physical-input sentinel and registry validation remain hard prerequisites.
Execution stays serial, with every row's original selector, nonce, Job,
capture-terminal, HWND-census and behavior-receipt requirements. No new runner
framework, test retry, deadline, manifest entry or acceptance oracle is added.

The runner cohort passes 11/11 in 0.361 s,
`a1b286ce-8e7a-40c5-b11a-f5a9a2f67f1c`, including aggregation after a failed
baseline and the existing selector, manifest, environment and receipt controls.
The complete follow-up log is
`%TEMP%/open-gpui-clipboard-admission-aggregate-manifest-20261115.log`.
It passes the UI contract, original sentinel
`c2e3748f-cfd0-48a4-a949-96abd64fe0c5` (1.077 s), Windows baseline
`e6400bfa-e5d9-4f38-9a8a-6c675281c13b` (324/324 in 142.115 s), and registry
`c3ec24d8-505b-4351-b781-50d27672efb6` (0.044 s). Both earlier native
Submitted/readback failures pass in this baseline. Their earlier failures
remain recorded and are not described as fixed by the aggregation change.

All fifteen manifest rows execute independently:

| Scenario | Result | Nextest run ID |
| --- | --- | --- |
| `native.u27.source-capture` | Failed: initial native capture timeout | `66631457-df22-46b3-9609-55d1b3f610d0` |
| `native.u29.complex-dockspace-workflow` | Passed | `80973474-2492-4603-bbdb-5fbc6e4cfd3a` |
| `native.u27.opaque-occlusion` | Failed: no open-desktop release point | `c8c6bbc5-7b3d-4100-a02b-cb7bb235f672` |
| `native.u27.opaque-hit-transparent-prefix` | Passed | `96e2557d-e3c1-47e2-a13c-d1b136ee81ce` |
| `native.u27.surface-shutdown` | Passed | `a054cf0d-8d40-41da-aed6-c6e74ada16a1` |
| `native.u29.provisional-same-hwnd-promotion` | Failed: no open-desktop release point | `57083983-c6fd-426a-85f4-3e1093f805e5` |
| `native.u29.live-route-and-release-lock` | Failed: no open-desktop release point | `11aedec0-4f6e-4561-af5b-109fa9986c97` |
| `native.u29.committed-loss-recovery` | Failed: no open-desktop release point | `2b305d4f-dfed-43f7-b7dc-9a945966a429` |
| `native.u28.activation-terminal` | Passed | `459d4ec8-9a33-408e-bfa2-a04d0e4ff52f` |
| `native.u28.provisional-activation-z-order` | Failed: no shell-desktop placement point | `14f0e9f3-15c4-41c2-8c01-accee16a97b3` |
| `native.u28.client-geometry-reconciliation` | Passed | `f4f78f5a-af45-47d4-beb0-ad06640cbce0` |
| `native.u28.event-driven-geometry-wake` | Passed | `c142716d-0a43-4449-a33c-93b81c2af82a` |
| `native.u28.process-convergence` | Passed | `411de65f-24a8-4daf-84ab-bff6e477b244` |
| `native.u28.no-input-pass-through` | Passed | `b24921a4-ae8d-47c8-a16e-6758abf8e7a6` |
| `native.u28.mixed-dpi-client-bounds` | Failed: no open-desktop point in either display direction | `2764f0a1-f692-4959-bf53-64e448958922` |

The final Dock baseline passes 35/35 in 7.870 s,
`8712e855-28b7-476b-88ec-422956405eaa`, including the programmatic native
destination-loss/recovery trajectory. The complete gate returns failure with
exactly the seven failed rows above: its latest manifest result is **8/15**.
It is neither the historical 7/15 result nor a combined 9/15 inferred from
separate focused passes. The four-step workflow preserves the original View
and local state 41. The process-convergence worker matrix executes both
`LastWindowClosed` and `AppShutdown` with the original protocol checks.

The source-capture failure occurs before cross-window delivery: the worker
times out waiting for `shared source native capture`. Its original failed
report still completes the nonce-bound report/release handshake with capture
released, no App windows or active drag, two observed/two terminal native
generations and no unterminated generation. The retained worker nonce is
`468-primary-18e83cb64917c9a0`. This receipt does not explain the initial
capture timeout, and the earlier focused source-capture pass does not replace
this failed row.

The other six failures report missing actual shell-desktop placement. Native
point probes see a maximized `Chrome_WidgetWin_1` over the primary work area
and two `CASCADIA_HOSTING_WINDOW_CLASS` windows covering the secondary work
area. Taskbar points also remain ineligible. The distinct 125%/150% display
pair and negative secondary origin are still present; this run cannot reach
the physical preview/promotion or final mixed-DPI client-bounds assertions.
No unrelated application window is moved or minimized, and no opaque point
is reclassified as desktop. These observations do not qualify physical
committed-loss recovery or legacy removal.

## Successful worker receipt retention

Auditing the successful row receipts finds that the existing harness removes
its stdout/stderr logs after success. Consequently the complete run above
retains failed worker reports and per-row nextest results, but no durable
original successful-worker nonces or census logs. Those missing values are
not reconstructed from a later run.

The harness now keeps the original worker stdout/stderr files for successful
cases as well as failures, and prints their exact paths with the original
scenario, subcase and protocol nonce after the existing checks succeed. The
manifest runner adds `--success-output final` only for scenario invocations
so those references reach the complete-gate log. The existing nonce-bound
worker JSON remains the census and capture-terminal record; this adds no
second receipt schema or authority registry. The report/release, independent
parent census, Job lifetime and exit checks are unchanged.

The focused real process-convergence follow-up passes in 3.620 s,
`96c0e270-fb2e-4afb-aed4-8c1d96fed02c`, using the actual manifest row and its
matching original behavior receipt. Its log is
`%TEMP%/open-gpui-process-convergence-retained-receipts-20261115.log`.
Both original worker stdout/stderr pairs remain on disk:

| Exit path | Original protocol nonce | Retained worker log stem in `%TEMP%` |
| --- | --- | --- |
| `LastWindowClosed` | `80096-last_window_closed-18e83d0af4648308` | `open-gpui-native-dock-80096-native-u28-process-convergence-last_window_closed-1794751568766337800` |
| `AppShutdown` | `80096-app_shutdown-18e83d0b5eb0e170` | `open-gpui-native-dock-80096-native-u28-process-convergence-app_shutdown-1794751570549727600` |

Reading each retained original stdout report confirms its matching exit path
and `passed` outcome, capture released, three observed/three terminal native
generations, no unterminated generations, no App windows or active drag,
settled native exit authority, a closed/empty surface and zero pending/failed
terminal tickets. The ordinary HWND and GPUI message-only sets are empty;
the existing process-scoped system message-window exception remains distinct.
The parent verifies the original independent census and Job/exit protocol
before emitting each `verified native worker` artifact reference.

After the retention change, the runner cohort passes 11/11 in 0.335 s,
`624981d6-c53f-4f5e-9ccd-b9662ce6b466`, and the example baseline passes
35/35 in 7.596 s, `a5ce95fe-b6d5-4c50-a0af-0e4e5f9d7998`. Their logs are
`%TEMP%/open-gpui-native-gate-receipt-tests-20261115.log` and
`%TEMP%/open-gpui-native-receipt-example-baseline-20261115.log`.
The focused retention validation does not replace any failed row or change
the latest complete manifest result from 8/15.

Final `cargo fmt --all --check`, the legacy-feature native test inventory
compilation and `git diff --check` under the repository's existing line-ending
configuration pass. The clipboard diagnostic tag is absent from the Rust
source. The manifest SHA-256 is unchanged, and the final process inventory
finds no remaining task-owned Cargo, xtask or native test worker. Work remains
uncommitted, with pre-existing workspace changes preserved.

## CI execution capacity

The next read-only GitHub check at 22:45 +08:00 finds zero registered repository
Actions runners. The existing dedicated interactive workflow run
[43670141240](https://github.com/Latias94/open-gpui/actions/runs/43670141240)
is queued at published commit `479c48011efb15930025a71ad5578daf414cc2ac`.
Its required `open-gpui-windows-native-interactive-ephemeral` runner has not
executed the gate. Another dispatch would not supply the missing runner.

The hosted [Verify run 43670141224](https://github.com/Latias94/open-gpui/actions/runs/43670141224)
at that same published commit completes 12/13 jobs successfully. Its Windows
native job fails in
`native_synthetic_a11y_activation_snapshot_update_and_release` at
`native_accessibility_tree_tests.rs:261`. The compiler-floor, shared/default
libraries, complete libraries, GPU-host, browser and platform/consumer jobs
pass. These remote results predate the uncommitted local fixes and cannot
qualify them. No remote CI run of the current workspace is claimed.

The interactive workflow now prepares an isolated directory under
`RUNNER_TEMP` for its gate process's `TEMP`/`TMP`, keeping this job's native
worker logs separate from unrelated runner files. A pinned
`actions/upload-artifact` step runs on success or failure and uploads the
original `open-gpui-native-dock-*.stdout.log` and `*.stderr.log` files for
14 days. The full gate output stays in the Actions job log. Missing files
before worker creation warn without suppressing the original failure. The
typed manifest command, runner label, machine preflight, single-Cargo queue
and native acceptance requirements are unchanged.

Hosted CI can validate the compilation, adapter and native-programmatic
layers through the existing Verify workflow. Completing the physical
fifteen-row gate requires an eligible unlocked ephemeral runner with native
rendering, distinct-DPI/negative-origin displays and exposed shell-desktop
space on both displays. A local desktop is not relabeled or registered as
that runner. The workflow must execute published source to verify the local
implementation; no commit, push, runner registration or workflow dispatch
is performed in this continuation.

Local workflow validation passes actionlint, the five native UI-contract
tests (`64fe1271-6d7c-4ab9-857b-d5f45467984c`, 0.163 s), and an isolated
PowerShell execution of the evidence-directory preparation block. That
execution verifies that `TEMP`, `TMP` and the upload directory resolve to
the same newly created directory. Repository diff whitespace checks pass,
and the native manifest hash is unchanged. These checks do not execute an
Actions upload or establish remote runner capacity.
