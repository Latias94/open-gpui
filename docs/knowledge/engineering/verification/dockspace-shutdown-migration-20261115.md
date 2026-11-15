# Shared Dockspace Shutdown and Failure Diagnostics

## Source and scope

This continuation starts at Open-GPUI
`4018e762c0e9e9bf67c3b10271cf6b0caf8e7a8b`. The published dockspace dependency
remains `dcd65e983220e17662b1f02929fcf84348cd8eb3`; a fresh `ls-remote` observes
that same remote `main`. The previous [44-commit inventory](dockspace-source-qualification-20261114.md)
remains the complete adopted range. No Zed comparison endpoint changes here.

| Source or reference | Disposition in this continuation |
| --- | --- |
| dockspace `dcd65e983220e17662b1f02929fcf84348cd8eb3` | Keep the published pin; use its typed runtime errors, original native input/output ownership and owned layout checkpoint. |
| sibling `a375fb899bdac26ff601959170cc0d97cfcbde82` | Locally committed pending-drop interleaving tests; absent from remote `main` at observation. Reference only, not a new dependency or claimed GPUI execution. |
| sibling `7e35fc13f50e87086e3af7ddd32f574a87026a54` | Locally committed egui-native fatal-service and fresh-host recovery reference. Not published at observation and not included in the GPUI dependency. |
| Open-GPUI `6af090b9` | Reject application updates and pane registration after native admission freezes. Original cleanup owners continue. |
| Open-GPUI `09f7a97d` | Preserve original typed errors and the first protocol/internal cause; copy diagnostic data without retaining failed affine owners. |
| Open-GPUI `1fe0d9b5` | Wait for exact capture cleanup before close dispatch; retain the pre-motion start separately from FIFO event admission and stop retrying terminal failures. |
| Open-GPUI `6f8d3747` | Migrate the existing Windows captured-shutdown coordinate to the shared worker while retaining the native manifest, process receipt and actual input/terminal oracles. |

The sibling checkout contains other untracked work and is left untouched. Its
egui-native coordinator and journal are reference mechanisms, not replacement
owners for GPUI windows, captured input or renderer tickets.

## Closed admission during native shutdown

`begin_native_shutdown` already freezes root binding, pointer input and native
effects. The public `update` and `open_pane` paths previously continued to enter
the same session. Two regressions fail on the original implementation because
both operations return success after shutdown starts.

The new check runs before invoking application update code or changing pane
registration. It returns the existing typed `NativeBridgeError::Draining`.
Read-only inspection, persistence capture and the original presentation/native
cleanup paths retain their own behavior. The existing delayed-dependent-terminal
regression now attempts a rejected update while the anchor waits, then releases
the real terminal and requires complete quiescence.

## Typed failure diagnostics and retained ownership

The native collector previously rewrapped `NativeBridgeError` with
`anyhow!(error.to_string())`, losing its original runtime/native categories.
`HostState::fail` also replaced the initial diagnostic on every cleanup error.
The adapter now exposes an owned `DockHostFailure` through `DockSurface::failure`;
`error` remains the text convenience method. The diagnostic contains only text
and typed categories. It does not retain an affine error/output/result.

Classification reads the actual source chain, including the transparent bridge
`Core` variant and native effect-report category. Capacity remains an operation
conflict; unknown adapter errors remain unclassified. The first classified
protocol/internal/foreign-update failure becomes the retained primary cause.
Later errors are logged without replacing that cause. This does not introduce
automatic fatal recovery, retry every conflict, or clear the original FIFO.

The regression uses two actual managed sessions. A real binding from the second
provider enters the first provider's public native pointer transport before a
real Deferred-to-Submitted renderer terminal. The original conversion produces
no typed category; the corrected path retains `HostProtocol / InvalidFacts`.
The rejected native head and its original output suffix remain queued. The old
presentation artifact stays current, the original ticket stays Submitted, and
shutdown cannot falsely report semantic quiescence. An independent healthy host
still drains normally. A separate contextual capacity control verifies the
retry category without matching error strings.

## Capture release before close dispatch

Independent review of the first migrated Windows worker finds one P2: its final
capture-release and destruction checks do not preserve the old journey's
release-before-close-dispatch requirement. Source inspection also confirms the
production defect. `NativeBridge::remove_window` previously called
`cancel_capture` and immediately removed the window, while the original release
continuation runs through GPUI's post-borrow native FIFO.

The bridge now retains a window while its original capture, release barrier,
queued cancellation or failed release remains unresolved. Only collection of the
exact terminal clears that custody and permits close dispatch. A later native
destruction cannot retroactively authorize an earlier close dispatch. The actual
native-terminal callback still retires its physical authority through the
original path. The existing callback wake resumes cleanup; there is no polling
task or second cleanup owner.

The first deterministic regression retains an actual GPUI captured drag through
same-window Root continuation, rejects native release, then starts shutdown. It
fails on the original path because logical close already occurs. After the fix,
the dependent and anchor stay live. Making release succeed still cannot close
them while its real terminal waits uncollected in the FIFO. Collecting that
original source/generation permits dependent-first logical close.

A second regression identifies an earlier interval: the shared wrapper has
activated GPUI's native captured drag, but its first native motion callback has
not arrived. Previously callback custody began only at that motion, so the close
check saw no capture and still closed early. The existing start continuation now
retains the activated `PreparedNativeCapturedDragConsumer` generation in a
cleanup-only callback slot, separate from the accepted-event generation. It adds
no generation or protocol. The regression requires
zero native motion events, a real active source, rejected release keeping both
windows live, and then one exact cancellation/release followed by quiescence.

Review of the first correction identifies two more P2 paths, both corrected:

- A failed original release must not reenter cancellation every pump and grow
  unsupported reports. The existing failure latch now stops further cancellation
  on that physical callback until its actual native terminal. Failure continues
  to block close; no new generation or successful-looking retry clears it. The
  regression advances real bounded retries through 8/32/128 ms to `Failed`, then
  requires repeated service calls to stop producing work while retaining the
  unclosed source.
- Cleanup custody for activated G2 cannot advance G1's accepted-event cursor.
  The separate start slot preserves a legitimate queued G1 terminal while a
  suspended session update holds collection. The regression uses actual GPUI
  G1 release and an active, unmoved G2 prepared consumer. Reinstating just the
  initial shared-cell setter makes it fail with `StaleCallback`; restoring the
  separate slot accepts the original prefix and leaves G2 active.

The initial attempt to exercise the second path through a complete docking UI
does not retain the intended suspended candidate across the test renderer's
frame callbacks. That fixture is replaced by the existing native transport
test layer with the same real GPUI events and semantic update lease. No
production admission check is relaxed and no private bridge test API is added.

All four capture regressions pass after the owning corrections. These are GPUI
adapter defects in the fork's added native docking mechanism; no official Zed
GPUI mechanism is replaced by the fix. General recovery after failed/unobservable
release remains outside this change; failure is not reclassified as success.

## Windows surface-shutdown migration

The existing manifest coordinate
`native_interactive_tests::native_interactive_anchor_close_releases_capture_and_retires_dependent_hwnds`
now dispatches a dedicated shared-session worker after the same parent/child
START handshake. The legacy implementation of this one journey is removed.
The other native scenarios continue through their existing owners. The package
still opts into `legacy-docking` because its ordinary dogfood and the remaining
journeys have not migrated.

The worker reuses the existing nonce report, process job, physical pointer guard,
HWND census, native-generation observations and RELEASE/OS-exit protocol. It
creates a normal anchor and a core-requested child, waits for two exact Submitted
frames and current presented receivers, then uses actual SendInput at the
painted child's tab. Canary WndProc observations, GetCapture and the original
native captured-drag generation establish admission. A real WM_CLOSE on the
anchor must cancel that generation, settle its exact release barrier, retire the
dependent before the anchor, and reach shared binding quiescence. Each HWND and
core binding must publish exactly one terminal. Errors attempt original shared
cleanup and publish a failed worker result; they do not become a passing receipt.

The corrected worker also samples GetCapture, active GPUI drag/native source,
the original generation's cancellation and the returned source WM_CAPTURECHANGED
at each `on_window_closed` callback. It requires one dependent close followed by
one anchor close, with those capture/route observations already clean. This is a
logical-close boundary observation. The deterministic bridge regression above
separately establishes the stricter original-terminal-before-dispatch gate; the
worker does not mislabel a later callback as the dispatch entry point.

Final review removes a P2 in the new observer: WM_CAPTURECHANGED is delivered
after ownership changes, so its entry-time GetCapture snapshot need not still
name the recipient. The worker keeps exact source identity, returned-message and
capture-after-None checks. Its independent pre-close WndProc/GetCapture admission
still proves that the source originally owned capture.

This is a source migration with deterministic and cross-compiler qualification.
Windows physical execution remains required before retiring its old package-wide
compatibility dependencies. The pending Windows runner cannot be substituted by
Mac execution or compiler success. The [previous ten-behavior table](dockspace-remote-adoption-20261114.md#legacy-removal-boundary)
continues to track the full physical replacement scope; one implementation is
now migrated and nine still use the old runtime.

## Verification

Cargo runs serially, with one build worker and the existing target directory.

| Gate | Result |
| --- | --- |
| Original shutdown admission regressions | Both fail before the production guard, with successful forbidden operations. |
| Default docking after the admission guard | **79/79**, zero skipped; run `186df9fe-b0b5-430e-818c-80f2f2da1401`. |
| Original native error conversion with the typed output regression | Fails with `runtime_kind == None` instead of `HostProtocol`; the capacity control passes. |
| Default docking after typed diagnostics | **81/81**, zero skipped; run `47f99847-3ccf-4938-b938-227b85a5cb30`. |
| All-feature docking before the capture follow-up | **1545/1545**, zero skipped; run `232271c1-843c-4c83-9b84-aa7ee9cb1c40`. The prior count of 1541 gains four new regressions; none is physical Windows execution. |
| Affected native dogfood, DevTools and xtask tests before the capture follow-up | **255/255**, one existing ignored fixture-regeneration test; run `7d553f3d-4aee-4289-adc0-b4e751502d07`. Windows-only cases are not compiled on this Mac. |
| Initial real Windows SDK cross-check | MSVC target all-targets check of the migrated native example passes in **50.65 s**; no physical input execution. |
| Capture-close and pre-motion controls | **2/2**, run `c5ca6b73-725e-4886-be1a-c66f10a8b426`, after both original-path failures. |
| Rust 1.95 before the capture follow-up | Four affected package all-feature/all-target checks pass; package floors remain unchanged. |
| Correctness/suspicious Clippy before the capture follow-up | Passes on the same four affected packages; existing advisory warnings remain. |
| Independently locked Rust 1.95 consumer before the capture follow-up | All-target check passes in **2m 22s**, sharing the existing target. No lock or dependency changes. |
| Real macOS editor before the capture follow-up | Actual native wait **0**, normal exit **0**; two real Submitted windows, retained entity/local/shared edits, dependent/anchor terminal, quiescence and sibling isolation. Temporary app removed. |
| Final reviewed default docking | **85/85**, zero skipped; run `8428ac97-4448-4aa4-b5cf-a85afe1c1002`. |
| Final reviewed all-feature docking | **1549/1549**, zero skipped; run `c903c202-cf5f-4e88-889f-e2dfdaaa02a0`. Eight new regressions compared with the previous increment's 1541; physical Windows cases remain separate. |
| Final reviewed native dogfood, DevTools and xtask | **255/255**, one existing ignored fixture-regeneration test; run `7b370b9e-7149-4fa6-bffb-8d61afbbfe0d`. |
| Final reviewed Rust 1.95 and Clippy | Docking all-feature/all-target check passes; correctness/suspicious Clippy passes. Other affected packages passed the earlier unchanged-contract cohort. |
| Final reviewed Windows SDK cross-check | The native example and all targets pass against `x86_64-pc-windows-msvc` in **4.77 s**, including the corrected logical-close observer. This is not physical Windows execution. |
| Final reviewed independent consumer | Independently locked Rust 1.95 all-target check passes in **6.26 s**, using the shared target; no lock changes. |
| Final reviewed real macOS editor at `6f8d3747` | Rebuilt source completes the same actual two-Submitted-window/entity/edit/shutdown/isolation trajectory. Native wait **0**, normal exit **0**; temporary app removed. Each invocation is sequential and bounded. |
| Repository checks | Formatting, import boundaries, public API tiers, document links and release-document verification pass. |

Logs use `/tmp/open-gpui-dockspace-{admission-red,admission-green,typed-failure-red,typed-failure-green}-20261115.log`.
Capture failures use `capture-close-red` and `capture-start-red` with that same
prefix/date. The initial worker/compiler/native receipts use
`continuation-{msrv,clippy,independent-consumer,native-build,native-smoke}` and
`shared-shutdown-windows-check` with the same prefix/date. Final reviewed
qualification follows the capture corrections rather than inheriting those
earlier receipts.
The failure-lock and old-setter negative controls are
`/tmp/open-gpui-dockspace-capture-{failure,start-order}-red-20261115.log`.

Final logs use `/tmp/open-gpui-dockspace-final-reviewed-{default,allfeatures,consumers-tests,msrv,clippy,windows-check,independent-consumer,native-build,native-smoke,static}-20261115.log`.
Independent review closes the admission, typed-diagnostic, capture-lifecycle and
corrected Windows-observer findings, with no remaining P1/P2 in the changed
scope. The review does not claim physical Windows execution or general fatal
reconstruction.

## Remaining work

Continue migrating the nine old Windows journeys in behavior-sized slices,
starting with process convergence and its last-window/App-shutdown subcases.
Keep creation-in-progress, provisional-window and injected cleanup-failure
coverage visible rather than replacing them with a simpler happy path. Execute
the migrated shutdown journey on the owning Windows interactive runner.

General fatal recovery remains open. An ordinary unrecorded pointer carries no
affine result, but native effect results, capture acknowledgements, lifecycle
facts and presentation outputs have different owners. Define rejection and
terminal behavior per record before allowing any suffix to bypass a permanently
rejected head. Physical HWND disappearance alone does not establish semantic
drain or permit fresh-host reconstruction. Keep the earlier measured wasm debug
stack optimization separate from this lifecycle work.
