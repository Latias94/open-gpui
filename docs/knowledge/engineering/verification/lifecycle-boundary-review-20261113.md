# Native lifecycle boundary review

## Scope and design assessment

This review starts from Open-GPUI `1686f58c2259644471f54cabb70f58f333ebe40c`
after its fast-forward publication on `main`. The official reference remains
Zed `27bbbc1cc480a7500cec122f120e796d1fb8500c`; dockspace remains pinned to
`c200977d0f34943284a125c492919791650e1bff`.
No new upstream commits or dependency revisions are selected by these fork
lifecycle corrections.

The owner asks whether two fork-introduced lifecycle defects indicate a design
problem and requests further checking. The review follows three bounded paths:
macOS renderer/native close, Core shutdown ownership, and the docking adapter's
physical-window versus logical-placement ownership. Two read-only reviewers
inspect Core and docking independently; verified defects then receive separate
implementation scopes. Cargo is serialized with one build job and the existing
target directory.

The exact presentation ticket and retained native owner have concrete consumers:
deferred frame submission, native docking and eventual external composition.
Logical removal cannot prove GPU quiescence or native terminal. Likewise, one
physical window can contain several independently retiring placements. Those
boundaries remain necessary. The observed defects instead require shared cleanup
entry points, preserved framework identities and explicit ownership of delayed
reports. This review does not select a new state system or another lifecycle
registry.

## Framework and adapter responsibilities

| Owner | Necessary responsibility | Refactoring boundary |
| --- | --- | --- |
| GPUI Core | Full WindowId identity, exact accepted/submitted frame correlation, logical receipt terminal settlement, retained renderer/native retirement | Preserve official window identity semantics and ordinary single-window consumers. All close/rollback/shutdown entries share the same cleanup mechanisms. Core must not own dock layout or close policy. |
| macOS and Apple renderer | Stop frame production, detach AppKit layers outside locks, drain GPU use, acknowledge renderer/native terminal in order | Keep platform effects in the backend. A stronger fork shutdown contract requires an adapted backing-layer callback rather than copying upstream's callback alone. |
| `gpui_docking::adapter` | Dockspace window/placement bindings, original callback correlation, one-time report consumption, application close policy | Keep physical-owner and per-placement obligations distinct. Reuse Core capture/submission authority rather than introducing another event identity system. |
| Application Views | Document sharing, local draft state and retained entity identity | Continue composing GPUI Entity values. Multiview does not require replacing Core state or moving application history into the framework. |

The next refactor should remove duplicate teardown branches and replace ambiguous
ownership with existing typed operations. After the selected consumers and native
platform gates pass, migrate the remaining facade callers together and remove
their superseded graph/controller owners under ADR 0034. A broad state rewrite,
additional lifecycle manager or expanded compositor is not selected by this audit.

## Previously repaired defects and provenance

| Defect | Fork introduction | Existing correction | Official reference |
| --- | --- | --- | --- |
| A retired Metal layer makes `makeBackingLayer` return nil | `fd937dbd` adds early renderer shutdown without adapting the NSView callback | `158d558d` shares renderer quiescence with native close and supplies NSView's ordinary layer during retirement | Official `make_backing_layer` returns the renderer layer, but `destroy()` is a no-op and has no equivalent early layer-clearing path |
| Shared-window continuation omits a placement retirement | `68d52b00` places per-placement notification inside physical-resource movement | `02288c04`; the independent notification remains after Registry adoption in `620ee058` | Official GPUI has no dockspace Root-continuation/per-placement retirement protocol |

Both corrections are ancestors of the reviewed baseline. Their existence is
evidence of fork integration defects, not missing upstream fixes. The earlier
targeted nextest run `f13bef70-6263-4443-a29e-c1b26a51ceb8` passes the two Mac
authority cases and shared-placement regression, **3/3**; 1,651 cases outside
that filter are not counted as new qualification.

## Newly reproduced Core defects

| Defect | Trigger and failure evidence | Correction |
| --- | --- | --- |
| P1: nonterminating shutdown resets window slot generations | Open A, finish shutdown, open B. Both originally receive `WindowId(1v1)`, allowing A's old handle to name B. New identity assertion fails in run `f2bbaec9-25d5-4089-9d06-c5beff71f620`, `/tmp/open-gpui-lifecycle-window-identity-before-20261113.log`. | Drain the original SlotMap into retained retirement owners. Preserve its generation history, matching the identity semantics of upstream `clear()`. Regression also refuses old-handle update and close while B remains live. The replacement-map operation traces to `16594280`. |
| P2: shutdown and creation rollback leave logical receipts pending | A root builder emits an exact frame ticket then panics; the original ticket remains `Pending`. A window with pending placement mutation enters shutdown; its ticket has no observation or terminal delivery. Both regressions fail in run `5d817a37-2a1b-492d-bcc3-9e1f10d9c2d1`, `/tmp/open-gpui-lifecycle-logical-terminal-before-20261113.log`. | Share logical receipt settlement at presentation-shutdown claim, ordinary removal and final removal. Retain separate renderer/native retirement; logical ticket completion does not fabricate native terminal. |

The second negative control removes only the newly added settlement call from
`claim_presentation_shutdown`, then restores that exact hunk. It does not undo
unrelated edits or the identity correction. Its tests retain blocked renderer
shutdown/native terminal so early owner release cannot masquerade as success.

Implementation review adds one necessary panic boundary: a root builder may
already be unwinding while its pending mutation's terminal subscriber panics.
`WindowCreationRollback::drop` catches the retirement preparation failure after
the coordinator has retained the owner. During existing unwinding it preserves
the original panic and suppresses the secondary payload, including its destructor;
otherwise it propagates the preparation failure normally. The added regression
checks the original panic message, delivery to a sibling subscriber exactly once,
logical ticket settlement, retained native resources and eventual native terminal.

| Core correction | Commit | Qualification |
| --- | --- | --- |
| Preserve window identity through shutdown/reopen | `2012c525` | Actual pre-fix failure above; final old-handle update/close refusals and replacement survival |
| Shared logical receipt settlement and rollback unwind boundary | `e34ed609` | Two pre-fix failures above; final exact-frame/mutation and secondary-subscriber panic regressions |

The first corrected complete Core `test-support` library run passes **1,722/1,722**,
`d9673908-3ced-4085-8177-24588d5dcd2d`, before the final panic-boundary addition.
The final complete run passes **1,723/1,723**, zero skipped,
`b3ca8550-4f87-4095-95fb-cbac437b4c1a`, in 44.509 seconds of execution.
Actual Rust **1.93.0** locked Core library compilation and all-target Core Clippy
with correctness/suspicious denied both pass after that final addition. Package
formatting passes. Existing advisory lint/linker warnings remain. Log:
`/tmp/open-gpui-lifecycle-core-final-20261113.log`.

## Docking continuation and report corrections

| Finding | Confirmed behavior and correction | Regression |
| --- | --- | --- |
| P2: delayed capture release is lost after Root continuation | The same physical callback owner moves to a successor binding. A release queued with the predecessor binding used to be rejected as stale, leaving its release barrier set and allowing predecessor quiescence too early. Reuse the original callback object's weak identity and full source WindowId; preserve the original binding/barrier and block predecessor quiescence until that exact release settles. | Two real GPUI capture regressions cover delayed release through continuation, predecessor quiescence and preserving/cancelling a subsequent drag generation. A third refuses a replaced callback owner. |
| P2: close policy failure loses the current report and terminal suffix | Draining the complete report queue before fallible close-policy work destroyed its unprocessed suffix. Consume successful prefixes one item at a time, retain the current item and suffix on error, and explicitly end a stale close only when Core confirms its original binding is retained but no longer current. | Real close observation plus same-frame native layout reset; a separate policy-failure/retry test preserves both the current request and a later native terminal. |
| P2: continuation discards exact physical inventory with independent observation proofs | Clearing all callback facts makes a surviving Root's inventory Unknown until another observation. A different window closing in that interval cannot publish its exact destroyed snapshot, yet the host could proceed to quiescence without Core's terminal guard. Preserve only `NativeWindowFacts::live()` or `destroyed()` for the same continued physical owner; geometry, input, presentation, readiness and effect acknowledgements still refresh. | The close/reset regression checks current inventory is exact before another draw, then completes actual native termination, predecessor cleanup and group shutdown without a missing terminal guard. |
| API ownership gap: a consumed frame report could be replayed after quiescence | `HostFrameReport` is non-Clone and owns affine native effects, but `accept_report(&mut report)` left already-consumed observations available for replay. Consume the report by value, matching its one-shot ownership; do not add an unbounded completed-binding history. Surface unexpected quiescence errors instead of silently retrying all errors. | All managed host and bridge callers move the report after extracting required output facts. The public signature and migration guide document one-time consumption. Rustdoc examples describe the move contract; this crate has doctests disabled, so no executed doctest result is claimed. |

The combined negative control retains the new by-value API but removes the
capture-correlation and report-queue production fixes. Run
`f425499c-abef-412b-9d82-776544d9e47f` executes five regressions: **four fail**
with the expected lost release, early quiescence, stale-close error and lost
suffix; the replacement-owner refusal control already passes in the old code.
Log: `/tmp/open-gpui-lifecycle-docking-before-fixed-fixture-20261113.log`.
The first attempt fails compilation because a test called crate-private
`test_window`; the fixture is corrected to the existing public
`set_pointer_capture_release_callback`, without expanding Core's test API.
Both production patches are restored precisely after each attempt.

The first corrected complete docking library run is **1,531 passed and 1 failed**,
zero skipped, `f575d4a6-c7dd-4446-852e-ced99e6dcc21`. The close/reset regression
passes its original report-prefix assertions but then reports a missing terminal
binding retention guard while completing cleanup. Log:
`/tmp/open-gpui-lifecycle-docking-after-20261114.log`. This result is retained
separately from its subsequent diagnosis and rerun.

Diagnosis confirms an adapter inventory gap rather than a need to infer geometry
or weaken Core's exact terminal guard. The first inventory assertion incorrectly
expected both pre-reset roots to remain current; that run remains **1,531 passed,
1 failed**, `f25c410c-a3eb-453b-974f-4d15008f3b72`, in
`/tmp/open-gpui-lifecycle-docking-final-20261114.log`. The assertion is corrected
to compare the exact current roster, with minimal facts for every surviving
physical owner. The focused complete close/reset/retirement case then passes
`50781958-1b24-4a3e-bd5c-1994a9ae818a`; it does not insert a prerequisite draw.
An independent reviewer confirms that continuation preserves physical existence
without upgrading coordinate, input, presentation or effect authority.

| Docking correction | Commit | Qualification |
| --- | --- | --- |
| One-time report ownership, fallible prefix retention and continued physical inventory | `ba4506ae` | Real close/reset/native-terminal suffix and failed-policy retry regressions; public move-based API migration recorded |
| Original capture-release callback custody and predecessor quiescence | `ec7089b8` | Real capture release delayed through continuation, subsequent drag generation and replacement callback refusal |

The final complete all-feature docking library run passes **1,532/1,532**, zero
skipped, `e1751d37-881a-4806-a8a5-b967e32ff880`, in 25.458 seconds of execution.
Actual Rust **1.95.0** all-target/all-feature adapter and editor compilation passes,
as does the independently locked public consumer on the same compiler without
adding test-support. Final adapter/editor all-target/all-feature Clippy passes
with correctness/suspicious denied, and complete workspace formatting passes.
Log: `/tmp/open-gpui-lifecycle-final-gates-20261114.log`. Existing advisory
warnings remain; the report API's ownership change does not raise compiler floors.

The two implementation lanes receive independent cross-review of callback
identity, predecessor release obligations, consumed report ownership and
fallible-prefix handling. Neither review confirms an additional blocker in that
bounded implementation scope. Release retry exhaustion remains a separate
existing path; these new continuation regressions do not claim a new execution
receipt for its `Failed` outcome.

## Native renewal on an active desktop

At `2026-11-13T23:29:48+08:00`, an independent CoreGraphics query returns status
0 and **one active display**. Console inspection reports an on-console session
without the earlier locked-session marker. Native binaries are rebuilt from
the baseline through Cargo JSON artifact discovery, then copied by the existing
LaunchServices runner into invocation-owned bundles.

| Executed case | Actual result | Log |
| --- | --- | --- |
| Baseline production editor `--smoke-native` | Exit 0 in 2.114 s; child admission and actual Submitted frames, retained View/local/shared state after movement, dependent/anchor native terminal and binding quiescence, surviving independent sibling and final retirement | `/tmp/open-gpui-lifecycle-audit-native-smoke-20261113.log` |
| `native_windowing native-close` | Exit 0 in 1.155 s; exact renderer-before-native shutdown, repeated/reentrant close, one callback and ordinary layer after retirement | `/tmp/open-gpui-lifecycle-audit-native-close-20261113.log` |
| `native_layer_shutdown --ancestor-backed --exercise-window` | Exit 0 in 1.278 s; actual active/key/unoccluded window, attached Metal frame source, newer Submitted generation after resize and successful retirement-tail layout | `/tmp/open-gpui-lifecycle-audit-native-layer-20261113.log` |
| Final editor `--smoke-native` at `ec7089b8` | Exit 0 in 2.364 s after all Core/docking corrections; repeats the complete child/Submitted/View-edit/group-retirement/isolated-sibling trajectory | `/tmp/open-gpui-lifecycle-final-native-smoke-20261114.log` |

Each runner observes the native OS wait status **0**, rather than relying only
on `open -W` or a success marker. All four cases run serially and their temporary
apps are removed. The first three precede the Core corrections and retain that
baseline scope; the final editor is rebuilt after every correction at `ec7089b8`.
No native process or temporary app from these invocations remains.

The editor result closes the previous baseline's unsuccessful native renewal
recorded in the [integration review](dockspace-integration-review-20261113.md).
It does not retrospectively prove the original `ChildAdmitted` timeout was caused
by locking the screen. Neither ordinary native success nor deterministic tests
cover forced first-attempt Mac state-lock contention, physical capture, mixed-DPI
dragging, IME transfer, every OS backend or measured performance.

## Final static checks and next work

The existing import-boundary, public-API, document-link and release-document
checks pass through the built xtask. Relative links in the five edited migration,
handoff and verification documents are also checked directly, including this
engineering record outside the strict public-document scan.

Continue with direct injection of the first rejected Mac close attempt and the
capture-release exhaustion path, then physical capture/hit-stack/mixed-DPI/IME
qualification on the owning platforms. Keep ordinary standalone consumers in
those gates. After that selected qualification, retire superseded docking
facade/graph/controller owners together with their callers; retain GPUI's common
frame/native lifetime primitives and the one-way durable migration. The audit
does not identify a need to move docking policy into Core or introduce a new
application state architecture.
