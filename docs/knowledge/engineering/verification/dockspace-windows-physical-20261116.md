---
type: Verification Evidence
title: Windows Dockspace Physical Follow-up
status: partial
timestamp: 2026-11-16T11:45:00+08:00
git_commit: fffd7b93a50bdab029e22a9669052c858d4b49ed
---

# Windows Dockspace Physical Follow-up

## Scope and execution

This follow-up starts at published commit
`d423b75339b013953f99f5bd32e84ac01876b576` and continues the
[Windows readiness investigation](dockspace-windows-readiness-20261115.md).
The revision in this record's frontmatter contains the qualified corrections.
The dockspace dependency remains locked to
`dcd65e983220e17662b1f02929fcf84348cd8eb3`. No dependency source or manifest
scenario is changed. Original physical acceptance requirements are retained,
release-lock observations are strengthened, and unrelated application windows
are left in place.

The user is controlling Windows remotely with one exposed 3840 × 2160 display
at 150% scale. The user explicitly excludes mixed-DPI execution in this session.
The earlier programmatic two-display placement result and the earlier failed
physical mixed-DPI row retain their original scope. Neither is a new physical
mixed-DPI pass.

All Cargo and native invocations are serial, with `CARGO_BUILD_JOBS=1`, the
existing target directory, and nextest `--test-threads=1`. The original full
manifest invocation was interrupted after the user excluded mixed-DPI. Its
already-running Windows baseline completed successfully; the interruption is
not a successful full gate.

The selected continuation executes all fourteen other manifest rows with their
original package, exact nextest selector, ignored-test policy, scenario ID,
behavior, and fresh token-bound behavior receipt. The original worker process
Job, START/REPORT/RELEASE nonce protocol, native HWND census, capture terminals,
and process exit checks remain the acceptance authority. The temporary Python
invocation wrapper adds no substitute oracle or committed runner framework.

Evidence is retained under:

```text
%TEMP%/open-gpui-physical-acceptance-20261116-101601/
```

The unchanged schema-7 manifest SHA-256 is
`21500cab76443cfa0d4cf67297957dcb29af53ed60aa893ea0796f82a8ecce7b`.

## Reproduced defects and corrections

### Native input did not invalidate interaction-only packets

The shared adapter committed the authoritative outside-all pointer input with
the original source capture and current work-area binding. Core generated a
Native preview, but the source retained its previous encoded packet. At the
locked core revision, `HostFrameReport::repaint_surfaces()` derives from scene
authority deltas; this interaction change did not change the scene stamp.

The added managed-adapter regression fails before the production correction:
`managed_desktop_drag_outside_all_records_a_native_preview`, run
`239363e9-8f2a-4ab1-963c-2ee8d73e5dfa`. Its two existing routing/button-authority
controls pass. A temporary source-packet invalidation makes the same test pass,
isolating encoding invalidation from core routing and native capability checks.

The bridge now retains an interaction repaint request only after a pointer,
capture packet, pointer roster, or device-removal record is accepted. The managed
host consumes it after the corresponding successful core commit and dirties
retained packets only when published state changed. Failed commits retain the
request; renderer and presentation callbacks alone cannot perpetuate repaint.
The regression also verifies cancellation clears the preview and repeated
presentation feedback preserves the encoded packet serial.

### A hidden shell depended on another ordinary render to arm reveal

After the input correction, the physical worker creates the third original
Window/HWND. Its only render occurs during construction, with an Unbound
provisional session and pending initial presentation. The initial-presentation
observer only refreshed the hidden window. No ordinary visible-window paint
pump exists to run the later render that would arm reveal.

The original initial-presentation terminal callback now advances the same
provisional owner directly. Rendering and that callback share one private
presentation-advance function. The exact reveal, placement, semantics, first-live,
admission, and physical retirement authorities are unchanged.

The original physical same-HWND case first passes in run
`aa2ff101-6441-4970-854b-2fcc90c5237b`, with nonce
`36696-primary-18e866fe894eeacc`. The successful original worker logs remain in
`focused-105418/`. The default docking cohort passes **100/100** in 4.848 seconds
in `docking-after-input-and-reveal-fix.log`.

The test dispatcher eagerly draws dirty windows, so the existing CPU provisional
fixture cannot independently reproduce the hidden native paint-pump absence.
The failed and successful original physical runs provide that regression signal;
CPU fixture success is not described as equivalent coverage.

### The platform test installed its semantics marker after the first frame

The platform provisional test requested its semantics frame, released its App
borrow, and only then installed the marker authority. App-borrow release can
already drain that exact frame. A later ordinary paint can be throttled until
the inactive frame interval, while the test exhausted 512 nonblocking message
polls without waiting for native VSync.

The test now installs its marker and notifies its root within the same App
update. It uses the existing one-millisecond delayed message-pump helper with
the same attempt count and exact Submitted/accepted-generation checks. Failure
diagnostics retain the ticket and session snapshots. No production cadence,
native terminal requirement, or acceptance condition is relaxed.

The original physical platform case passes in run
`76f0dc2d-aacc-432b-8c3d-0574ce2808c6` and in the selected cohort below.

### Native non-client transparency was rejected as a contradictory target

After LiveRoute placement, the original pointer can be inside the provisional
native caption but outside its client bounds. Windows returns that exact HWND
from `WindowFromPoint` even though its native `WM_NCHITTEST` returns
`HTTRANSPARENT`. The point-stack collector rejected this disagreement before
independently enumerating the underlying windows. Subsequent same-point moves
and release became Unavailable, causing core to revoke the preview and cancel.

The private provisional observation now carries the original sample's native
non-client transparency result and existing placement epoch. The exception
requires a Gated session, exact registration/session/physical geometry, a point
outside the client but inside the observed outer frame, and a real transparent
native hit test. Lifetime, session, epoch and geometry are revalidated after
that query and across the independent samples. Coordinates that cannot be
represented losslessly in `WM_NCHITTEST` are rejected. Placement ABA therefore
cannot preserve a proof merely by restoring equal geometry.

The enumeration must still complete and find the exact HWND named by
`WindowFromPoint`. Its independently verified suffix and full observation roster
must agree. An opaque window underneath remains OpaqueBarrier. Ordinary and
no-input windows retain their original disagreement rejection. No public hit
type or capability is expanded.

The new classifier regression fails in run
`fcfccc92-a792-4387-8302-d06bb875db4e`, then passes with its eight negative and
stabilization controls in `385c12dc-6852-44ee-8c26-f9a5219bb5f9` (**9/9**).
The physical live-route case then passes in
`8fd88a5d-f8db-44ba-a797-7d6f3098d759`. Its strengthened worker injects PrimaryUp
and an immediate divergent move in one native sequence, then requires the
original Released packet's position/typed terminus and the original final
placement receipt. Cursor movement after release cannot redirect the HWND.
The live-route worker also waits for actual WndProc delivery rather than
asserting synchronously after the system cursor changes position.

Review identifies a second boundary: a visible, quiesced registered window
must not remain a pass-through observation below an otherwise valid prefix.
The real native mutation test now quiesces its settled no-input window before
native destruction and requires inspection to fail closed. It fails without
the guard in `696ee16a-702c-4864-ae16-d56e3d9cb6d4`. Inspection now rejects
quiescence both before sampling and after the native query. That regression
and the nine point-stack controls pass **10/10** in
`3972c564-139a-457f-af3f-b556d01c167e`. This final review correction precedes
the reviewed fourteen-row cohort below.

## Final selected fourteen-row cohort

`focused-114241/results.json` and its original per-row/worker logs retain one
complete final selected cohort: **13 passed, 1 failed, 1 explicitly excluded**. This is
not a 15/15 full manifest gate and is not combined with later focused passes.
Its immediately preceding original input sentinel passes in 1.134 seconds,
run `9cae4a29-496a-46e0-ae42-afb406b90d1c`.

| Manifest row | Result | Nextest run ID |
| --- | --- | --- |
| `native.u27.source-capture` | Passed | `12b6a49f-7ae3-4f59-8906-a38291974095` |
| `native.u29.complex-dockspace-workflow` | Passed | `3f5ff455-2eee-4276-9ee0-ac634290b213` |
| `native.u27.opaque-occlusion` | Failed: same-HWND promotion timeout | `dbe3c4c2-9e7c-4b22-b71e-cd98144f6c1f` |
| `native.u27.opaque-hit-transparent-prefix` | Passed | `8f4c8df6-af2d-49d2-b9a8-1ba4ab443719` |
| `native.u27.surface-shutdown` | Passed | `7d53b3a5-ac97-4121-96b9-887504d29953` |
| `native.u29.provisional-same-hwnd-promotion` | Passed | `bbc213cb-e9ec-4bc3-a3e6-e61c84b4bf22` |
| `native.u29.live-route-and-release-lock` | Passed: original release survives immediate pointer divergence | `b9398957-6511-4616-a187-6d731bcf3a72` |
| `native.u29.committed-loss-recovery` | Passed | `eaadd73a-1e47-479b-ab34-e382104a0984` |
| `native.u28.activation-terminal` | Passed | `c8e55a3e-1bd3-446a-8202-e73cb222a8eb` |
| `native.u28.provisional-activation-z-order` | Passed | `3ce280bd-e176-4384-9c26-99d6217f67a5` |
| `native.u28.client-geometry-reconciliation` | Passed | `88495d85-b77b-4d7c-a649-8cebfbac8586` |
| `native.u28.event-driven-geometry-wake` | Passed | `091a2e05-5e65-4032-b9f8-689804869873` |
| `native.u28.process-convergence` | Passed: both original exit paths | `76a0c4b9-2350-44e8-b5fd-4c28677fe115` |
| `native.u28.no-input-pass-through` | Passed | `c53f9905-8e0f-4c91-a0ae-8c20316461e8` |
| `native.u28.mixed-dpi-client-bounds` | Excluded by user: single-display session | Not run |

The first selected cohort before the user exposed desktop space was 8/14. Its
six failures and the intermediate focused retries remain in `partial-results.json`
and `desktop-cleared-102413/results.json`. The intermediate 12/14 cohort remains
in `focused-105629/results.json`; its live-route failure is
`2c53eb29-c680-4c62-ba12-8f893df35077`. None is erased or combined with the final cohort.
The earlier 13/14 cohort before the final review guard remains separately
recorded in `focused-112824/results.json`.
The original Windows baseline passes **324/324**, run
`9dce9848-7266-4718-9f55-a2c1c5a95faa`; the earlier final example baseline passes
**35/35**, run `cc3662c0-8046-4e1a-a6eb-e20d6c93bc8c`.

## Lifecycle receipts

The selected cohort retains these original worker protocol nonces:

| Scenario/subcase | Original nonce |
| --- | --- |
| Process `LastWindowClosed` | `32972-last_window_closed-18e869b57a635764` |
| Process `AppShutdown` | `32972-app_shutdown-18e869b5e6935024` |
| Committed destination loss/recovery | `75840-primary-18e869aed50f2c94` |
| Same-HWND promotion | `84248-primary-18e869ac0227077c` |
| Live route and release lock | `81608-primary-18e869ad63c7db90` |

Each original report is Passed with three observed/three terminal native
generations, no unterminated native generation, released capture, no registered
App windows or active drag, settled native exit authority, a closed/empty docking
session, and zero pending or failed terminal tickets. Ordinary top-level HWND
and GPUI message-only sets are empty. The existing process-scoped Windows system
message-window exception remains separately reported. The parent verifies the
original independent census, Job lifetime and process exit before publishing
each successful artifact reference.

## Remaining contract boundary

The opaque row's original contract releases above an actual opaque foreign
HWND, blocks docking into covered application content, and requires the same
provisional HWND to promote in the point-scoped lower Z-order band. The locked
core maps that Foreign route to `OpaqueBlocker`, revokes its Native preview and
cancels release. The original failed receipt retains the source View and retires
the provisional window. This is not an open-desktop setup failure.

The adapter must continue reporting the actual opaque hit. Reclassifying it as
OutsideAll, replaying a different release, replacing the HWND, or accepting
cancellation as promotion would change the oracle. The row remains failed while
the shared-core contract and its migration requirement are reconciled.

The remaining native-over-foreign placement decision belongs in dockspace core,
with explicit placement/recovery authorization and physical behavior coverage.
The adapter must keep ownership of the actual native hit, exact physical tickets,
same-HWND adoption and terminal ordering, rather than introduce another gesture
reducer or synthesize a desktop release. The retained legacy code and physical
row remain necessary until that contract is reconciled and executed.

## Library and repository validation

| Check | Result | Nextest run ID |
| --- | --- | --- |
| Default shared docking | 100/100 in 4.848 s | `7bb242de-8420-4547-bc03-38cbf88e92ed` |
| Windows baseline before the final quiescence guard | 325/325 in 142.063 s; 10 existing ignored tests excluded | `6f7d32a2-6b10-4936-8227-e36ec32d3bc8` |
| Docking all features, including retained legacy | 1564/1564 in 73.060 s | `5e75395c-8341-4e80-94d6-ccaaceb54331` |
| Native example baseline | 35/35 in 7.906 s; 13 ignored tests excluded | `c73a9e97-ae53-43be-ad77-f69e64ba3dd4` |
| Final reviewed native quiescence and point-stack controls | 10/10 in 2.159 s | `3972c564-139a-457f-af3f-b556d01c167e` |

Logs use `docking-after-input-and-reveal-fix.log`, `final-windows-baseline.log`,
`final-docking-all-features.log`, `final-docking-native-baseline.log`, and
`final-quiesced-hit-green.log` under the evidence directory. The final guard
changes only registered native inspection; the subsequent focused controls
and complete selected physical cohort qualify that correction separately.

Final `cargo fmt --all --check`, `cargo run --locked -p xtask -- scan-doc-links`,
and repository diff checks pass. Tagged temporary preview/live-route diagnostics
are absent from Rust source. The manifest hash above is unchanged. The final
native worker reports verify capture release and native/process retirement;
an independent final `GetAsyncKeyState(VK_LBUTTON)` read also observes no pressed
primary button. Pre-existing unrelated workspace changes are preserved.
