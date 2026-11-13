# Dockspace native host and motion integration review

## Scope and source

The initial review compared the production Open-GPUI dependency
`46da973bcc8db7cb50c72aef0f65b6acd6b59338` with dockspace
`94f8b404eb2d8da39305effcb74b191553f4836b`, using Open-GPUI
`76e2c6463944ddfaba00ae71676d14d3741dfc12` as the adapter baseline.
It follows [ADR 0034](../../../adr/0034-shared-dockspace-session-integration.md).
The accepted integration follow-up below records the later dependency selection
and adapter changes separately from that initial comparison.

The original dockspace checkout is `/Users/frankorz/codes/rust/egui_docking`.
It had existing GPUI harness edits and acquired extensive concurrent source/test
reorganization during the review. Those changes are outside the fixed comparison.
Only review-owned changes were copied into the separate
`fix/gpui-host-contracts` branch, based on `94f8b404`, at
`/tmp/dockspace-gpui-host-contracts-20261113`. The main checkout and its concurrent
changes were preserved. Later uncommitted motion defaults are not attributed to
the fixed revision or included in the qualification below.

The concurrent release/layout refactor subsequently committed as `6e4a2366`.
The original checkout retains working copies of this review's corrections;
the final terminal-wakeup correction was also applied to its relocated egui
modules, preserving that refactor. The isolated commits remain the reviewable
delivery, and no merge into the concurrently owned main branch was performed.

## Adoption decisions

| Addition | Recommendation for Open-GPUI | Required integration condition |
| --- | --- | --- |
| `runtime::NativeWindows<K, T>` (`fdc7eb3c`) | Use as the established physical-window resource registry when the adapter can remove its equivalent indexing. Consume it directly from core. | GPUI has no `WindowId` before native construction, and failed opens can retain original rollback tickets. Preserve those staging owners; do not add a second authoritative callback map solely to use this helper. |
| Core `DockspaceActionStatus` and owned semantic outcomes (`598689f3`) | Reuse when adding typed product-command completion to the application facade. | Preserve submitted-action and presentation-transition correlation; `Applied(RootDockRequested)` does not mean ownership has transferred. |
| Current recovery geometry for native Dock Back (`4bbbda81`) | Qualify source and recovery geometry through the public GPUI adapter. | `complete_unpainted_surfaces(Deferred)` preserves the current scene and only finishes pending pointer input; `commit()` already does that. Explicit `defer_surface` supplies an unavailable contribution. A frame without new paint is not evidence of an unavailable scene. |
| `SurfaceSplitterRange`, Minimum/Maximum adjustments (`c4b57b31`) | Use for keyboard and accessibility integration. | Expose only currently operable controls from the admitted artifact. The target model alone cannot authorize an intermediate splitter. |
| Shared core motion (`84fcbc39`) | Adopt as an explicit follow-up after the fixes and host scheduling gates below. | Render the sampled program geometry, clips and order directly; retain GPUI entity identity and independent exact terminal settlement. |
| egui guide customization and motion decoration (`69d6a756`, `94f8b404`) | Treat as adapter examples; keep GPUI's existing theme/paint owners. | Canonical target geometry and passive decoration must remain distinct. No egui dependency is needed in the GPUI adapter. |
| `dockspace_winit` resources, observations and placement (`fdc7eb3c`, `4568d24e`) | Reference their lifecycle handling, while using GPUI's native backend. | Winit observations cannot substitute for GPUI's original frame, activation, capture-release or rollback authority. |

`NativeWindows` is a useful small module: it handles duplicate keys, exact
bindings, non-cloneable resources and physical continuation without owning the
layout or OS event loop. Its container is intentionally unbounded. Turning it
into a general window lifecycle manager would duplicate existing native session
and host responsibilities. Missing event provenance cannot be reconstructed by
another map or inferred from an OS handle.

## Confirmed corrections

| Finding | Trigger and impact | Correction and regression |
| --- | --- | --- |
| GPUI shared Root drops a placement-retirement notification | One physical Root contains two logical placements. Reset returns two continuation pairs sharing the same old/new window. Moving the callback entry for the first pair made the second skip retirement, retaining an old binding. | `02288c04`: record each predecessor independently of physical movement; deduplicate outstanding retirement notifications. The original implementation fails the actual two-placement regression. |
| Winit example loses an unrecorded retirement suffix | Input capacity accepts one quiescence record and refuses the next. The example had already removed the complete retirement entry. | `9a2ac972`: retain the entry and pop a placement only after successful recording; remove its snapshot after the suffix drains. A real one-record capacity reproduces the original missing-entry failure. The regression proves explicit retry, not automatic recovery by the example's event loop. |
| A foreign surface's popup snaps an active layout to target | Core trajectory preparation scoped popup settlement to the current surface, while the presentation compiler bypassed sampling for any popup in the workspace. After the popup closed, the surviving model could resume behind the displayed target. | `b8c00a5f`: make both decisions surface-scoped. A two-surface public-runtime regression checks sampled rectangles while the foreign menu is open and forward progress after dismissal. No paint/hit mismatch is claimed. |
| Settled egui output can fail to wake deferred input | Capacity one retains the final inactive motion output. A subsequent owned run returns its untouched input with backpressure. The final receipt had saved no motion repaint request, so settlement could leave the caller asleep. | `7fa3c6bd`: register the current waiting context/viewport and consume that waiter on terminal settlement, independently of active motion. Test Presented, Dropped and receipt drop, exact retained input, repeated refusal and absence of a static repaint loop. |
| Native registry lifetime documentation was incomplete | Capturing a bare reused framework ID after queuing can resolve an old event to a replacement. Also, `continue_root` returning false does not finish each shared placement's retirement. | `907dd365`: document generation/event-lane requirements and physical-versus-placement obligations. Add two distinct continuation pairs sharing one resource, with separate exact quiescence. |

The registry itself did not show a confirmed resource-loss or cross-incarnation
lookup defect under its documented contract. Registration conflicts return the
unmodified resource. A committed continuation moves physical lookup only; old
output/callback owners must retain their own exact identity until terminal
settlement. A generic registry cannot make that settlement true.

Independent review of the initial egui correction found one more required case:
after a context/viewport rebind, a predecessor receipt must wake the current
waiting lane when its output retires. Its old presentation must still be refused
by the new lane. The final correction stores the waiting target, drops stale
output before notifying it, and retains a retirement signal if the terminal races
with waiter registration. Cross-context and cross-viewport regressions cover
all three terminal paths without consuming the deferred text/IME input early.
The initial same-lane wake condition fails both new rebind regressions; the
final implementation passes all cases and independent review reports no further
confirmed P1/P2 in the corrected scope.

The initial four dockspace commits remain ordered on `fix/gpui-host-contracts`:
`907dd365` (registry contract and shared-pair coverage), `9a2ac972` (winit retirement
suffix), `b8c00a5f` (surface-scoped popup motion) and `7fa3c6bd` (terminal output
wakeups, including rebind). That initial review left local deliverables. The
accepted follow-up ports them onto the committed layout refactor on a separate
integration branch; neither delivery merges the concurrently owned main branch.

## Initial review verification

All Cargo builds and nextest runs were serialized with one build job. The local
development compiler is Rust 1.100; nextest 0.9.151 was used through its existing
temporary binary, leaving the shared installed nextest unchanged. No native GUI
probe or physical input driver was launched.

| Executed boundary | Result | Evidence |
| --- | --- | --- |
| Original GPUI shared Root implementation with new regression | Fails: one predecessor notification instead of two | `/tmp/open-gpui-shared-root-review-before-20261113.log` |
| Final GPUI shared adapter cohort | 47/47 | Run `18bf55e2-4e6d-4f01-89d8-f78431f5e5ae`; `/tmp/open-gpui-native-helpers-reviewed-final-20261113.log` |
| Original winit retirement loop with capacity regression | Fails after the first record: retirement entry is gone | `/tmp/dockspace-native-retirement-before-20261113.log` |
| Old core popup guard in the isolated tree | Fails: target rectangles replace the running sample | Run `33c8ea95-4d4c-4fa8-9084-8fb102f5e46b`; `/tmp/dockspace-popup-motion-before-20261113.log` |
| Old terminal wake condition in the isolated tree | Fails: zero wakeups instead of one | Run `308d3d37-58ed-4ab1-9b15-8a0a75ea84fe`; `/tmp/dockspace-final-motion-wake-before-20261113.log` |
| Initial same-lane-only wake with rebind regressions | Both fail: current context/viewport receives no notification | Run `64dc26c9-265c-4fae-877a-bf8fd5202014`; `/tmp/dockspace-rebound-output-wake-before-20261113.log` |
| Corrected core and egui focused motion regressions | 1/1 each | `/tmp/dockspace-fixed-motion-focused-20261113.log` |
| Complete dockspace library with motion | 2,132 passed, 3 pre-existing ignored | `/tmp/dockspace-gpui-host-contracts-full-focused-20261113.log` |
| Final complete egui adapter library with motion | 221/221 | Run `fb13c1ab-26bd-4dbe-94be-832148e22265`; `/tmp/dockspace-egui-output-wake-final-20261113.log` |
| Final egui output settlement/owned-run/rebind cases without motion | 40/40 | Run `0da9badc-05eb-4368-8245-48f1a31c8961`; same final-output-wake log |
| Native resource registry, no default features | 5/5 | Same full-cohort log |
| Winit multiview example's headless capacity regression | 1/1 | Same full-cohort log |
| Minimal core and registry without motion | 2,109 passed, 3 pre-existing ignored | `/tmp/dockspace-gpui-host-contracts-additional-gates-20261113.log` |
| Independently locked public host motion consumer | 1/1 | Same additional-gates log |
| Core/egui and winit-example correctness/suspicious Clippy, core dependency check and workspace format check | Pass | Same additional-gates log |
| Final egui correctness/suspicious Clippy and workspace format check after rebind correction | Pass | `/tmp/dockspace-egui-output-wake-final-20261113.log` |
| Compatibility of the working corrections with reorganized dockspace `6e4a2366` | Motion/rebind 4/4, registry 5/5, winit retirement 1/1 | `/tmp/dockspace-reorganized-source-compatibility-20261113.log` |

Open-GPUI's existing `scan-doc-links` gate passes. The new report's and updated
evidence record's local Markdown targets were also checked directly; the strict
public scan does not include every engineering verification record.

The original `fix/gpui-host-contracts` commits retain the fixed baseline. Results
from the reorganized main checkout establish only the listed compatibility
cases; they do not replace the fixed branch's package results or qualify all
concurrent changes. Physical OS dragging, native capture/hover,
mixed-DPI placement, IME across moved panes and GPU frame pacing remain separate
platform work.

## Accepted integration follow-up

The accepted follow-up starts at Open-GPUI `42ebd8df` and integrates the review
corrections with dockspace's committed release/layout refactor `6e4a2366`.
The worktree now carries `integrate/gpui-host-contracts`, published on the
dockspace origin; `main` and its concurrent harness changes remain untouched.
Open-GPUI selects immutable revision
`c200977d0f34943284a125c492919791650e1bff`, with defaults disabled and only
`serde` enabled. Its core dependency and docking consumers retain Rust 1.95.
This update does not enable shared core motion in GPUI.

During this follow-up, the concurrently owned dockspace main branch advanced to
`9df72e42`, including `ae67e035`/`dcf05398` for the earlier corrections and a
configurable retained-motion example in its separate GPUI harness. This is a
later reference for explicit motion integration, not the production dependency
selected or qualified here. No main-branch commits were changed or pushed by
this integration.

| Correction | Initial review commit | Integrated commit |
| --- | --- | --- |
| Registry event provenance and shared-placement retirement contract | `907dd365` | `cdf00844` |
| Winit retirement suffix under ingress backpressure | `9a2ac972` | `2699acdf` |
| Surface-scoped popup motion | `b8c00a5f` | `12c64571` |
| Terminal output wakeups, including context/viewport rebind | `7fa3c6bd` | `ae3e1ebb` |
| Native Dock Back availability uses the submission's recovery preparation | New integration finding | `c200977d` |

The production bridge uses `NativeWindows<WindowId, NativeCallbacks>` in place
of its equivalent map and linear reverse lookup. One bounded `opening` slot
retains callbacks before a factory produces a real ID. Failed opens preserve
the original GPUI rollback ticket; registration conflicts return the original
resource. The existing managed-host callback custody remains available while
the bridge is temporarily moved out for factory dispatch, without adding another
semantic or lifecycle authority.

Shared Root continuation moves the physical resource once and retires every
distinct predecessor placement separately. A delayed submission carries its
original binding and snapshot plus weak callback identity: physical continuation
does not retarget it to the successor or accept a replacement callback owner.
The regression obtains the real test renderer's deferred submission token;
initial creation events drain before the ticket is installed. A deferred
candidate remains Pending until its original token completes, then both accepted
and submitted generations must match that token.

The public adapter regression also exposed two missing tear-off staging paths.
An exact native placement can exist before its surface enters the published
workspace. Mount validation now accepts that placement only for its owning
window; the encoder uses the core's first-live presentation roster instead of
discarding the projected surface as a removed slot. Before preparation, its empty
deferred packet provides no painted receipt and source ownership stays unchanged.
The original mount fails with `unknown logical surface`; fixing only the mount
leaves the pane on its source because the first-live program is still skipped.
The failure logs are `/tmp/open-gpui-dockspace-recovery-backtrace-20261113.log`
and `/tmp/open-gpui-dockspace-recovery-mount-20261113.log`.

Native Dock Back requires source outer geometry as well as a valid recovery
scene. The old core query could return Ready while submission rejected
`DockBackUnavailable`. Its new query reuses the existing read-only recovery
compiler. The negative control fails the actual public availability assertion
against `ae3e1ebb`, run `17cf81e6-b3a3-40fb-8d69-75708126bd14`;
`/tmp/dockspace-native-dockback-preflight-before-20261113.log`.
No client-to-outer fallback is introduced into production. The undecorated
TestWindow reports its own known outer bounds from the same physical observation;
when physical client geometry is absent, the complete observation remains absent.

The earlier blanket-Deferred warning was inaccurate: both old and new core
`complete_unpainted_surfaces(Deferred)` only finish pending pointer input.
Four calls immediately before `commit()` are removed as redundant work, not as
a scene-loss correction. Calls needed before further validation or suspension
remain. Explicit `defer_surface` still revokes scene availability.

Final dockspace qualification is **2,362 passed, 3 pre-existing skipped** with
core and egui motion enabled, run `ccc3bf70-c30f-4bfe-95c0-a1dcf7d5d07d`.
The same source passes actual Rust 1.95 core-only `serde` compilation,
correctness/suspicious Clippy and workspace formatting; advisory warnings remain.
Evidence: `/tmp/dockspace-native-dockback-preflight-qualified-20261113.log`.
The integrated registry **5/5** and winit retirement **1/1** ran before the
additional read-only Dock Back guard, in
`/tmp/dockspace-integrated-host-contracts-qualification-20261113.log`.

The first positive full run retains its two fixture failures: transferring a
low-level TestPresentationHost fixture into a runtime session permits the
read-only query but does not transfer its pending presentation writer state.
The new core regressions therefore query availability and verify unchanged
workspace/version; they do not add another writer or a test-only injection API.
Actual submission, transfer, original View interaction and shutdown are covered
by the GPUI end-to-end case. Independent bounded reviews found no remaining
confirmed P1/P2 in the integrated corrections, resource bridge or staging paths.

The framework TestWindow seam was rechecked against official Zed
`27bbbc1cc480a7500cec122f120e796d1fb8500c`. Its ordinary test window and draw
owners remain the reference; exact physical outer observations and deferred
submission receipts are fork extensions. The follow-up changes only the fork's
test geometry model and matching snapshot expectation, without changing an OS
backend's outer/client contract.

| Open-GPUI integration unit | Commit | Executed boundary |
| --- | --- | --- |
| Immutable dependency and independently locked consumer | `ffef5715` | Current `c200977d` resolves from the published dockspace integration branch. Defaults remain disabled. |
| Shared native resource registry and predecessor submission custody | `620ee058` | Failed-open FIFO, original rollback tickets, shared placements and real deferred test-renderer submission. |
| Coherent undecorated TestWindow outer observation | `64e0fbb7` | Same-observation geometry and complete frozen drag-start snapshot; absent geometry stays unavailable. |
| Pending tear-off first-live and recovery integration | `14f03fb4` | Real pane identity/click after Dock Back, no-paint preservation, minimize/restore, explicit unavailability and final quiescence. |
| Existing route-lock and closure-capture lint clarification | `5c0b8430` | Six redundant Copy aliases removed; statement-local Arc explanation preserves the UI-thread API contract. |

| Final framework gate | Actual result | Evidence |
| --- | --- | --- |
| Complete Core and old/new docking libraries, all features | 3,505 passed, 1 failed, zero skips | Run `428164de-d936-44cd-85b5-52e91d69dbf1`; `/tmp/open-gpui-dockspace-integrated-qualification-20261113.log`. The 49 shared-adapter cases all pass. |
| Corrected physical geometry and frozen drag-start snapshot expectations | 3/3 | Run `881e23b5-afdc-4ee6-9fea-c2a3386ec36f`; `/tmp/open-gpui-dockspace-integrated-final-gates-20261113.log`. The sole full-run failure expected the old absent TestWindow outer bounds; complete snapshot equality is retained. |
| Final adapter, native-capture and tab-body cohort after lint cleanup | 88/88 | Run `c9981dbb-ebb2-4e8c-ab50-1753ef120fa9`; `/tmp/open-gpui-dockspace-final-compiler-lint-20261113.log`; 1,439 cases outside the filter. |
| Actual Rust 1.95 docking and editor consumer, all targets/features | Pass | Same final compiler/lint log. |
| Actual Rust 1.95 independently locked public consumer, all targets/features | Pass | Same final compiler/lint log, without adding test-support to the consumer. |
| Core/docking/editor all-target/all-feature Clippy | Pass with correctness and suspicious categories denied | Same final compiler/lint log; advisory warnings remain. Initial strict lint diagnostics and their localized correction are retained in the preceding final-gates log. |
| Workspace formatting, import boundary, public API, doc links and release docs | Pass | Same final compiler/lint log and `/tmp/open-gpui-dockspace-static-gates-20261113.log`; the four edited engineering documents' relative targets were checked separately. |

These are separate executions. The complete library invocation is not rewritten
as a single 3,506/3,506 passing run after its focused correction. Physical OS
dragging, capture, mixed-DPI, IME across transferred panes and GPU pacing remain
the owning platform gates.

### Native renewal remains incomplete

The production editor executable was rebuilt from Cargo's JSON artifact path and
run only through the existing bounded LaunchServices runner. The initial attempt
times out after 30 seconds in `ChildAdmitted`: root and child have real Submitted
receipts, but no core admission is observed. This is an actual failed renewal,
not a passing native result. Log:
`/tmp/open-gpui-dockspace-native-smoke-20261113.log`, bundle receipt directory
`open-gpui-macos-probe-7fzjhex5` under the task temporary root.

Three bounded diagnostic launches then fail before the first window because an
atomic display publication is unavailable. A temporary native diagnostic exposes
the underlying failure: **CoreGraphics returned no active displays**. The online
display reported by `system_profiler` does not establish active presentation.
Evidence: `/tmp/open-gpui-dockspace-native-display-diagnostic-20261113.log`.
A process-scoped `caffeinate -d -u` assertion allows collection and Prepared root
observations, but two diagnostic runs time out in `RootSubmitted`. Separating
the two root rectangles does not resolve that result.

Read-only console inspection then confirms `CGSSessionScreenIsLocked = true`.
No further GUI launches are attempted. The original child-admission timeout is
not conclusively attributed to the later observed lock state; it still needs
renewal on an unlocked interactive desktop. Core comparison finds no changed
first-live guard in `46da973b..c200977d`, which is source evidence rather than
proof that the native path works.

Every attempted process reaches its bounded failure/normal exit 1, and the
runner removes its temporary app. Temporary diagnostic statements and the
experimental root geometry are removed from source. The ordinary executable is
rebuilt without them. No production display check, output admission guard,
timeout or native assertion is relaxed. Positive qualification for this current
source, including actual create/admit/move/retire and isolated sibling survival,
remains required after desktop unlock. Earlier positive receipts belong to their
recorded earlier revisions.

The final cleanup receipt is
`/tmp/open-gpui-dockspace-native-cleanup-20261113.json`: six sequential attempts,
zero live probe processes, zero task probe apps remaining and zero probe
fixed/recent Dock entries. The clean executable rebuild, formatting and static
checks are recorded in `/tmp/open-gpui-dockspace-final-clean-build-20261113.log`.

## Recommended development sequence

1. Keep the integrated dependency immutable, with `default-features = false` and
   the existing Rust 1.95 adapter floor. Preserve the separate source, recovery
   scene, outer geometry and exact presentation conditions of native Dock Back.
2. Keep pre-open callbacks and failed-open rollback custody explicit until a real
   `WindowId` exists. Established lookup uses `NativeWindows`; exact bindings
   remain attached to deferred callbacks and predecessor submissions. Retiring
   OS keys remain reserved until their original event lanes are drained.
3. Qualify two logical placements in one real window, two independent native
   windows, reused IDs, delayed predecessor outputs and close during creation.
   Preserve distinct callback retirement, output retirement, native destruction
   and source capture-release facts. Reuse semantic conformance scenarios with
   thin GPUI executors instead of building a second reducer or a generic GUI DSL.
4. Add core motion behind an explicit GPUI configuration. Supply one monotonic
   timestamp per update, preserve stable pane entities and schedule visible
   active surfaces. A terminal output must wake capacity waiters even when motion
   has settled. Pause hidden surfaces, resume at current time and test reduced
   motion. Keep native first-live transfers and direct manipulation immediate.
5. Complete native-input qualification and move existing facade consumers to the
   shared session together. Then delete displaced graph/controller/animation
   owners under ADR 0034. The library does not need another document/history
   state architecture to accomplish this cutover.

The largest remaining simplification opportunity is removing duplicated host
orchestration and displaced docking owners after qualification. Add a shared
helper when two concrete hosts would otherwise repeat the same rule; do not
expand scripts into source-level ownership or callback proof engines.
