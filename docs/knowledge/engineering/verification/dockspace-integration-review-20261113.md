# Dockspace native host and motion integration review

## Scope and source

This review compares the production Open-GPUI dependency
`46da973bcc8db7cb50c72aef0f65b6acd6b59338` with dockspace
`94f8b404eb2d8da39305effcb74b191553f4836b`, using Open-GPUI
`76e2c6463944ddfaba00ae71676d14d3741dfc12` as the adapter baseline.
It follows [ADR 0034](../../../adr/0034-shared-dockspace-session-integration.md).
The newer core remains a candidate dependency; this review does not update the
immutable production pin.

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
| Current recovery geometry for native Dock Back (`4bbbda81`) | Include in the next candidate-pin qualification. | Verify source and recovery geometry through input-only frames. Existing blanket `Deferred` completion in GPUI's update/native pump needs attention: a frame with no new paint is not necessarily an unavailable scene. |
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

The four dockspace commits are ordered on `fix/gpui-host-contracts`:
`907dd365` (registry contract and shared-pair coverage), `9a2ac972` (winit retirement
suffix), `b8c00a5f` (surface-scoped popup motion) and `7fa3c6bd` (terminal output
wakeups, including rebind). They are local deliverables for integration with the
concurrent dockspace work; this review did not merge or push that branch.

## Verification

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

The isolated tree's lockfile and manifests retain the fixed baseline. Results
from the reorganized main checkout establish only the listed compatibility
cases; they do not replace the fixed branch's package results or qualify all
concurrent changes. Physical OS dragging, native capture/hover,
mixed-DPI placement, IME across moved panes and GPU frame pacing remain separate
platform work.

## Recommended development sequence

1. Integrate the small dockspace corrections with its concurrent refactor, then
   select a new immutable dependency revision. Preserve `default-features = false`
   and qualify the existing Rust 1.95 adapter floor. Verify Dock Back with valid,
   hidden and explicitly unavailable recovery scenes before changing the pin.
2. Consolidate established physical resource indexing. Keep pre-open callbacks
   and failed-open rollback custody explicit until a real `WindowId` exists;
   move resource lookup into `NativeWindows` only if the old index disappears.
   Use exact bindings through all deferred callbacks. Retiring OS keys remain
   reserved until their original event lanes are drained.
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
