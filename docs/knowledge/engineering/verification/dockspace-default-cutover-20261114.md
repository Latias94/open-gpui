# Default Shared Docking Cutover

## Source and decision

This increment starts at Open-GPUI `f73afd5bcf9d6af2c78275a72988fb089d368bff`
on `main` and follows [ADR 0034](../../../adr/0034-shared-dockspace-session-integration.md).
The owner authorized breaking changes, implementation, commits and publication.
It advances U7's production entry point; it does not close U7's complete
incumbent-removal or physical-platform qualification scope.

| Source | Fixed revision | Use in this increment |
| --- | --- | --- |
| Open-GPUI baseline | `f73afd5bcf9d6af2c78275a72988fb089d368bff` | Already qualified session, persistence, native reports, Root continuation and capture-release custody. |
| Shared dockspace | `c200977d0f34943284a125c492919791650e1bff` | Unchanged production dependency; its complete session remains the only semantic authority of a migrated workspace. |
| Official Zed | `27bbbc1cc480a7500cec122f120e796d1fb8500c` | Unchanged fixed GPUI comparison. This increment changes the fork-owned docking facade and consumers, with no new upstream Core/platform import. |

| Open-GPUI delivery | Commit | Qualification linkage |
| --- | --- | --- |
| Structured diagnostic text redaction | `28ce49635639617bf6d2a68c2031f59f37f24a44` | Generic JSON structure/idempotence test and complete DevTools cohort below. |
| Default session cutover, inspection and close decisions | `e7bf156a6dcafe49f34ea81e4d578159d1dad3ee` | Final source for the deterministic and compiler gates below; includes compatibility isolation, migrated consumers, root roster cleanup and wasm clock correction. The successful native retry below builds documentation-only descendant `410f7650`; earlier native attempts retain their explicitly intermediate build scope. |

## Ownership and API cutover

The root and `prelude::DockSurface` now denote the shared-session implementation.
`DockPane`, `DockRegion`, configuration, owned inspection, `dockspace`, `native`
and `persistence` are ordinary public entry points. The earlier `adapter` path
continues to name the same implementation. It is not another owner.

The displaced graph/controller/interaction engine and its deterministic tests
move to `src/legacy/`: 215 Rust source files, plus their fixture corpus. They
compile only with `legacy-docking`. Motion, UI core and the old graph container
dependencies are optional under that feature. Enabling it never substitutes a
different root or prelude type. No migrated workspace dispatches to the old
engine, mirrors its topology or uses it for recovery. Legacy data migration
remains in the default `persistence` module without that runtime dependency.

| Consumer | Selected owner | Compatibility retention |
| --- | --- | --- |
| Minimal and multiviewport examples | Shared session through root/prelude | Ordinary retained pane content, native roots and isolated workspace. |
| Dockspace editor and independently locked dependency consumer | Same root implementation | Existing shared-session native and downstream compilation controls. |
| DevTools `docking` | Owned shared-session inspection | Pure projection; no report consumption, pumping, commit or retained mutation authority. |
| DevTools `legacy_docking` | Explicit incumbent records | Requires its `legacy-docking` feature; old diagnostic contracts stay available. |
| Native dogfood / Windows manifest scenarios | Explicit incumbent workspace | Package `legacy-docking` and required binary feature; CI and manifest runner opt in. |
| Browser smoke's unsupported-native-window probe | Explicit incumbent workspace | Dependency selects `legacy-docking`; earlier browser execution is not reattributed to the new session. |

`DockSurface::inspection` derives model counts, mount/region facts, exact pending
submission snapshots and managed lifecycle counts from their existing owners.
It exposes no entities, tickets or callbacks. A mutable host borrow returns
`DockInspectionError::Busy` without reentry or a diagnostic cache. Native quiescence uses the same
predicate as the public lifecycle query. DevTools converts only supplied facts,
keeps workspace/provider IDs opaque and sanitizes retained error text. An absent
submission is absence of a retained pending ticket, not evidence that a window
never submitted. The snapshot is not an additional capability or completion
authority.

Independent example review found two production close gaps: a tab request had no
application decision delivery, and an ordinary secondary root inherited a
child-only recovery action. The adapter now offers original committed close
plans through `set_close_handler`, outside mutable borrows and unaccepted/native
input boundaries. Decisions still use the core's exact tokens; the adapter does
not auto-approve document loss. Handler-driven updates cannot recursively enter
the next handler. Application roots default to `RetainLayout`, while children
keep `RecoverPresentation`; explicit policy remains authoritative. Root IDs
leave the managed roster only after their exact callbacks quiesce, avoiding
historical growth across repeated open/close. The examples explicitly allow
their demo View closes while retaining the application Entity and document.
Each dispatch round processes its initial queue only; a close requested by a
handler receives another wake after the original dispatch guard releases.

Inspection review also found that quoted JSON credentials in retained error
text bypassed the existing whitespace-based text sanitizer. The shared sanitizer
now recognizes complete JSON object/array bodies and reuses its existing
structured redaction. A regression covers compact and formatted quoted
passwords, values with spaces, nested array credentials, every new docking export
and the redaction counter without mutating the original source.

The independent WebAssembly compile found a pre-existing shared-adapter clock
mismatch: GPUI's executor returns `web_time::Instant`, while the adapter stored
`std::time::Instant`. The adapter now uses the same cross-platform type as GPUI.
Native targets retain its standard-library alias; the real wasm target compile
qualifies the browser type rather than a source-shape test. The browser's
separately locked smoke had not yet recorded the already selected dockspace
dependency; offline resolution adds that exact fixed source and its existing
hashing dependencies without refreshing unrelated versions.

## Review and verification

The main thread owns public entry points, compatibility isolation, consumer pins,
tooling and verification. Two workers have disjoint example and inspection /
DevTools write scopes. A read-only reviewer checks feature unification,
compatibility lifecycle preservation and whether native gates still select tests.
Cargo runs serially with `CARGO_BUILD_JOBS=1`, using the existing `target`.

The independent reviews cover default feature unification, mechanical legacy
source preservation, native gate selection, inspection authority/redaction and
the real application close integration. The two close findings and JSON error
redaction finding were corrected before publication. CI default/all-feature
commands are separate PowerShell steps so a later success cannot hide a failure.

The final source uses Rust 1.100.0 for development and Rust 1.95.0 for the
docking/tooling minimum. Core's floor remains 1.93. Test execution uses the
isolated nextest 0.9.151 runner because the shared older nextest cannot reconstruct
the Rust 1.100 Cargo intermediate layout. The federated UI runner still invokes
the installed tool for its existing exact registrations; that command passes
its declared scenarios without changing their qualification scope.

| Gate | Result and exact scope |
| --- | --- |
| Default shared docking | **65/65**, zero skipped; `95937bae-3b92-48d8-8bbb-d6a71c846529`. Includes original candidate/submission, multiregion, native transport, persistence, non-consuming inspection and application close decisions. |
| All-feature docking plus DevTools | **1663/1663**, zero skipped; `605d9a45-1742-49fe-bfec-ba6b8f7bf7e7`. Docking contributes **1541** cases, including the explicit incumbent cohort. DevTools contributes 122 in this run, including the JSON export fix. |
| Final complete DevTools | **123/123**, zero skipped; `82ff7227-7e4d-42ac-aa05-1724a1ddbee5`, after adding the generic JSON text structure/idempotence regression. No production change follows the preceding complete cohort. |
| DevTools `docking` alone | **6/6**, zero skipped; `2e4274be-c41d-42c2-83a1-6fd22916e26c`. `cargo tree` confirms this feature does not enable `legacy-docking` or the five displaced direct dependencies. |
| Migrated ordinary examples | **2/2**, zero skipped; `bde6a275-c324-427e-b573-a37c68a178f7`. Includes Explicit Quit shutdown, edited Preview close/reopen, standalone secondary close/reopen, exact root roster cleanup and independent workspace survival. |
| Incumbent native example | **26/26**, one deliberately ignored fixture-regeneration utility; `be7cdb41-4714-4877-88d9-377605b0015d`. macOS deterministic rendered/headless execution; this is not Windows physical input. |
| Tool regressions | **95/95**, zero skipped; `241deee0-8540-4cc6-a667-006cd503c0ee`. Public module fixtures follow the explicit legacy module; native manifest fixture paths follow their source move. |
| Rust 1.95 | Locked all-target/all-feature check passes for docking, DevTools, minimal, multiviewport, native dogfood, dockspace editor and xtask. The independently locked dependency consumer passes its own all-target/all-feature check. |
| Browser compilation | Rust 1.95 locked `wasm32-unknown-unknown` smoke compiles with its explicit incumbent feature and the shared adapter. This proves clock/dependency compilation, not browser execution of the new docking implementation. |
| Correctness and documentation | Strict `clippy::correctness` / `clippy::suspicious`, root formatting, rustdoc broken-link denial, public API, import boundaries, public doc links and release inventory pass. Existing non-denied warnings remain; this is not a warning-free claim. |
| Federated UI gate | `scan-ui-contract` passes its static ownership checks and exact registered scenarios. The existing visual scan now recognizes files named `tests.rs` as tests, matching its prior `_test` exclusion. |
| Native editor | **Passed** on clean `410f7650e6f2154108b54328b6ca2663481b0789`, after the two historical startup refusals below. The rebuilt editor completes the native trajectory in 2.026 s with actual OS exit 0; final inventory contains zero probe processes/apps. |

Logs use `/tmp/open-gpui-docking-cutover-*-20261114.log`. The default/shared,
all-feature, example, tool and diagnostic test run IDs above preserve their
individual cohorts instead of adding overlapping runs into one total.

### Native execution boundary

The cutover editor build, before the final follow-up-close wake regression and
DevTools sanitizer-only additions, was attempted twice serially through the
existing bounded runner. Its window/display owners match this increment's final
source. The first attempt exited with OS status 1 after 1.487 s: CoreGraphics
reported one online display but no active display, and GPUI refused creation
without an atomic display publication. A three-second `caffeinate -u` restored
one observed active display. The subsequent process still lacked a committed
atomic display topology and exited with OS status 1 after 1.041 s. This is an
unpassed native creation check; the active-display query alone does not prove
AppKit's complete publication. No synthetic display facts or weaker creation
gate were introduced to bypass that refusal.

Both temporary apps were removed after actual process exit. The final inventory
contains **zero live probe processes and zero temporary apps**. Logs are
`/tmp/open-gpui-docking-cutover-native-smoke-20261114.log` and
`/tmp/open-gpui-docking-cutover-native-smoke-awake-20261114.log`; the cleanup
receipt is `/tmp/open-gpui-docking-cutover-native-final-cleanup-20261114.json`.
Earlier successful native receipts keep their original revisions and are not
relabelled as execution of this cutover.

At the owner's request, the ordinary editor trajectory was retried at
2026-11-14 09:09:50 +08:00 on clean
`410f7650e6f2154108b54328b6ca2663481b0789`. A locked, serial incremental build
selected the actual Cargo-reported executable. CoreGraphics reported one active
and one online display before and after execution. A bounded `caffeinate -d -u`
assertion kept the display awake during the existing LaunchServices runner;
the assertion process was terminated and reaped afterward. No runtime or runner
source changed to obtain this result.

The retry **passed** in 2.026 s, with runner exit 0 and actual native wait status
0. Its `DOCKSPACE NATIVE SMOKE OK:` receipt confirms child creation, two real
Submitted windows, retained Entity identity and local/shared edits,
dependent-before-anchor native terminal handling and binding quiescence, and
survival followed by independent retirement of the isolated sibling workspace.
The temporary app was removed after process exit. The post-run inventory again
contains **zero live probe processes and zero temporary apps**. This qualifies
the ordinary native trajectory for the final cutover source; the physical input
and foreign-platform gates below remain separate.

| Retry artifact | Path |
| --- | --- |
| Cargo build log | `/tmp/open-gpui-docking-native-retry-20261114-090950-build.log` |
| Cargo executable selection | `/tmp/open-gpui-docking-native-retry-20261114-090950-artifacts.jsonl` |
| Native smoke log | `/tmp/open-gpui-docking-native-retry-20261114-090950-smoke.log` |
| Source, display, exit and cleanup receipt | `/tmp/open-gpui-docking-native-retry-20261114-090950-receipt.json` |

## Remaining removal and platform gates

This default cutover does not establish physical captured dragging, current
native hit-stack arbitration, mixed-DPI movement, OS IME or foreign-platform
termination. Existing CPU controls and earlier Mac smoke receipts retain their
recorded source and scope. Neither moving tests nor compiling the compatibility
feature supplies replacement-platform evidence.

Before removing the compatibility engine, run each declared replacement native
scenario on its owning backend, including real capture cancellation/release,
negative coordinates and target-display conversion, allow/veto/defer, failed
first-live recovery and dependent-before-anchor shutdown. Migrate the browser
unsupported-native-window probe through the new session and run its actual
browser gate. Keep legacy absolute sizing / binary topology differences explicit
rather than recreating that engine inside the adapter.

The next product increment is a coherent engine editing workflow using the
selected shared session and retained document Views. A new global state system
or a compositor rewrite is not a dependency of this cutover. Performance work
still starts with the measured C0 workload in the evolution plan.
