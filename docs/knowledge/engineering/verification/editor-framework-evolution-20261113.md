# Editor framework evolution evidence

The November 13 implementation follows the [framework evolution plan](../../../plans/2026-11-13-1145-refactor-editor-framework-evolution-plan.md).
The user subsequently authorized breaking changes and replacement of qualified implementations.
This authorizes implementation and local commits; it does not waive submission, shutdown or native qualification requirements.

## Source and compiler boundary

| Input | Revision | Scope |
| --- | --- | --- |
| Open-GPUI implementation base | `22399cb09e6399f6d247cc60bc665b45ab92d5df` | Plan commit retained on `refactor/editor-framework-evolution`. |
| Official GPUI adoption endpoint | `27bbbc1cc480a7500cec122f120e796d1fb8500c` | Existing reconciled baseline; no new core port is implied by adapter work. |
| dockspace original integration | `46da973bcc8db7cb50c72aef0f65b6acd6b59338` | Historical adapter origin and initial compiler/consumer qualification. |
| dockspace current integration | `c200977d0f34943284a125c492919791650e1bff` | Immutable Git dependency, defaults disabled, `serde` enabled. Apache-2.0 core; reviewed native helpers and recovery preflight. |

The production adapter consumes only `dockspace`, not its egui/winit adapters or its standalone GPUI harness.
The source harness's existing uncommitted changes are excluded from this dependency and remain untouched.
The adapter keeps one complete `DockspaceSession` as its logical authority; it does not mirror that session into the incumbent graph/controller.
Its GPUI chrome/content integration starts from the clean Apache-2.0 harness at
the original `46da973b` revision, then adds multiple regions, exact output correlation and
managed native transport in the production package. The source contains no NOTICE
file; Open-GPUI retains the Apache license and records the origin here.

For the original `46da973b` integration, an independently locked temporary consumer compiled the core and `serde` feature on macOS ARM64 with Rust 1.93.0 using `--ignore-rust-version`.
The same lockfile compiled with Rust 1.95.0 without that override.
The override is diagnostic evidence, not a supported consumption command: the selected immutable package declares Rust 1.95.
Accordingly `open-gpui-docking` explicitly declares Rust 1.95; its DevTools, Gallery, docking-example and workspace-tooling consumers declare the same floor so their complete feature sets remain covered. The workspace default, GPUI core, platform/rendering packages and UI components remain at 1.93.
Lowering the shared crate's published compiler floor requires a later upstream manifest change and qualification of its own package/test closure.

Commands used for this dependency probe:

```text
CARGO_BUILD_JOBS=2 cargo +1.93.0 check --manifest-path <temporary-consumer>/Cargo.toml --ignore-rust-version --all-features
CARGO_BUILD_JOBS=2 cargo +1.95.0 check --locked --manifest-path <temporary-consumer>/Cargo.toml --all-features
```

Both completed successfully. The temporary consumer used the clean core source at the recorded dockspace revision.
The repository lockfile independently records the immutable Git source and has one GPUI source identity.
Compiler success does not qualify native windows, input, renderer submission or Nara's Windows consumer.

## Ownership obligations

ADRs 0030 and 0031 continue to govern physical ownership: accepted candidates are distinct from submitted outputs, native close intent is distinct from destruction, and retirement requires the original renderer/capture/native facts.
One physical Window must aggregate all its logical surface outputs behind one original exact frame ticket.
A deferred Window must not retain the session's affine update lease and starve another Window.
Each View occurrence uses a separate GPUI entity; sharing a document does not authorize mounting the same entity twice.

The migration explicitly selects weighted N-ary layout semantics. Binary structural identities, absolute `SizeRef` behavior, rounding and central remainder rules cannot be silently presented as equivalent.
The migration receipt preserves their durable provenance and records differences for the host. It must never recreate an authoritative legacy graph.

## Rust 1.100 development toolchain

The local default is `rustc 1.100.0 (0ef1bc386 2026-11-09)` / Cargo 1.100.0,
host `aarch64-apple-darwin`, LLVM 23.1.5. The
[official announcement](https://blog.rust-lang.org/2026/11/12/Rust-1.100.0/) and
[fixed release notes](https://github.com/rust-lang/rust/blob/1.100.0/RELEASES.md)
were read against the current source and tools. A newer development compiler does not
change the declared package floors or supply their minimum-compiler qualification.

| Change | Project consequence and decision |
| --- | --- |
| New Cargo intermediate artifact layout | `compare_benchmarks.py` already selects executables from `compiler-artifact.executable`. The native probe runner copies an explicitly supplied executable. The EGL fixture now has independently locked Cargo roots and uses JSON `filenames` for its native mock, retaining a private copy during execution. Historical `target/debug/deps` paths remain labeled as the original 1.99.1 receipt. |
| Cargo dependency and manifest lints | Four unused workspace aliases were confirmed against all manifests and removed: `open_gpui_github_releases`, `open_gpui_tokio`, `serde_urlencoded`, `tokio-util`. The two actual framework packages and direct independent consumers remain present. The instrumentation macro now inherits workspace lints. Redundant homepage metadata is diagnostic cleanup rather than an API defect. |
| `Infallible = !`, more precise closure capture, generalized allocator inference for `Box` | Recompile actual package/consumer targets under 1.100 and fix concrete failures. Keep `Infallible` source spelling for the supported old compilers; do not add dual implementations that assume it differs from `!`. NumberInput's 21 focused editing/capture/IME tests pass under 1.100, nextest run `a667bec1-bdde-4f7c-ae2b-afeb7845c16b`. Gallery test-target compilation also passes. These are scoped results, not full-workspace qualification. |
| Stable `Allocator`, custom `Vec`/`Box` allocation | A possible later measured arena/allocation experiment. It does not replace GPUI's heterogeneous element destruction, scoped lifetime or resource-retirement owners. No 1.100-only allocator API is introduced into packages declaring 1.93/1.95. |
| Stable `DropGuard` | Potentially useful for local unconditional cleanup after a future floor decision. Existing typed candidate/acceptance/native-retirement guards own more than running a closure on exit and are not replaced on that basis. |
| Native compatibility and Unicode changes | Rust ABI changes on x86 concern independently compiled Rust components and hot-reload artifacts, not a new C/Objective-C ABI guarantee. Rebuild app and patches with the same toolchain. Unicode 18 character properties do not change the framework's explicit UTF-8/UTF-16 offset contract; existing text/IME regressions remain required. |
| Minimum publish age | Optional policy for future registry resolution, not required for these locked builds. No new cooldown or dependency-update approval policy is installed. Immutable Git pins and explicit Cargo.lock evidence remain the selected source boundary. |

The supported core spellings remain compatible with their existing floors. For example,
deprecated `std::usize` imports can be removed while retaining `usize::MAX`; replacing
`AtomicU64::fetch_update` with its newly named successor would require checking that
successor's stabilization against Rust 1.93 rather than applying `cargo fix` blindly.
The affected atomic operations keep their existing algorithm and ordering, with
statement-local deprecation allowances explaining the 1.93 floor. This does not
disable deprecated-API diagnostics for the crate.

The Cargo/JSON EGL fixture passed 3/3 tests under 1.100 (nextest
`3fd38718-dfeb-4f55-9f77-b21c7fe56999`). Its identical registry control passed 2 and
failed 1 with the original `initialize` unwrap panic, returning status 100 (nextest
`03198153-a2ce-4d61-86b0-3d7b29a628eb`). This is a real negative control, not a passing
production gate. The [fixture receipt](../../../../tests/fixtures/egl-errors/README.md)
records the exact source and runtime boundary.

## Core extension verification

The source comparison used the now-available official checkout at
`27bbbc1cc480a7500cec122f120e796d1fb8500c`, including `App::open_window` and
`PlatformWindow`. Normal application construction, initial drawing, callbacks and
platform owners remain official-derived. The extra detailed error/retirement
boundary serves the fork's existing native rollback contract and the docking
transport; upstream's general error alone cannot identify that authority.

`WindowOpenError::retirement_ticket()` exposes the original rollback ticket after
the opening transaction's guards have unwound. A failure before native ownership
has no ticket. A pre-root map failure can have one even though no root factory or
adapter callback ran; callers must retain it until actual retirement, rather than
reporting `NotApplied` or manufacturing a native terminal.

The physical geometry extension keeps native outer bounds optional and distinct
from client bounds. Validation rejects non-containing and overflowing rectangles.
Placement conversion remains owned by each backend at the current target display's
DPI and grants no mutation or native capability itself.

On Rust 1.100, the detailed-open/rollback and physical-observation cohort passed
**13/13** tests, nextest `e860d6a7-2566-40c4-bfbb-1ac952c3ee68`. This includes the
pre-platform failure without authority, the map failure carrying the same original
ticket, its later quiesced/native-terminal transition, and valid/invalid outer
geometry. It is deterministic test-platform execution, not native OS qualification.

The installed nextest is 0.9.115 (2025-12-15). It executes these tests but warns that
it cannot find some build-script output under Cargo 1.100's intermediate layout.
That warning is retained as a tooling compatibility limit; a passing test receipt
does not establish that this old runner reconstructs every build-script environment.
The self-update check found nextest 0.9.151 (`6b3f994577a98874a2e0ea5bef77fbbab3434044`,
2026-11-02). A copy in the task's temporary directory was updated and used for the
subsequent tests, leaving the shared installation at 0.9.115. Its Gallery/Mac run
has no missing build-script-output warning.

## Composable editor controls

The existing TextInputController, Entity notification, activation and overlay
owners remain authoritative. NumberInput now supports drafts, preview/commit/cancel
and captured scrub; Tree supports controlled selection/expansion, custom rows and
appearance; Combobox separates external query/results from durable selection and
supports per-row images/metadata through Listbox's original row shell. No callback,
GPUI element, document or engine history enters renderer-neutral state.

The integration runs retained their failures while corrections were made. The new
Listbox seam initially queried a dispatch-only keyboard context during render;
an existing accessibility test exposed that invalid stage. Nested key isolation
now checks that context only during actual input dispatch. The taller-row probe
then exposed missing original ScrollHandle linkage and focus-following on active
rows. Listbox now binds its existing scroll owner, retains a stable focus handle
for active-row movement and prevents its variable-height rows from shrinking.
The Combobox image probe establishes editor focus through a real click before
checking selection restoration; a default-open popup alone does not claim focus.

The affected semantic activation, accessibility, native-text/IME, form, choice,
layout/reveal, resource, theme, overlay and four new editing/composition test
binaries pass **361/361**, nextest `ad75df59-0757-4a65-8911-382cd80f55b4`, Rust 1.100.
The ordinary Gallery `editor-controls` page exercises Entity-owned Apply/Cancel,
rejected property proposals, controlled Tree and asynchronous material search,
including per-row color thumbnails. Its three workflow tests and the fourteen
Mac physical-placement/scoped-pointer tests pass **17/17**, nextest
`11cf5ae6-0431-4694-81c0-9dbbe0cf7bf8`, with nextest 0.9.151. These are deterministic
test-renderer workflows, not real OS input or screen-capture qualification.

A single-pass independent simplification review inspected 2,058 changed production
code lines against all five reuse/quality/efficiency/test/comment rubrics. It found
repeated full-result scans in grouped Combobox projection and temporary per-subtree
Tree sets. Both now use the already ordered data and one accumulator; two redundant
private module comments were removed. Proof tests were preserved. The complete
361-test regression above includes the resulting code.

## Compiler and platform receipts

The per-package dependency-floor tests pass **6/6**, nextest
`9678c4f4-3657-4429-9975-55ecee4abfcf`; the actual `xtask dependency-health` gate also
passes against the resolved graph. The independently locked full consumer adds
only the selected dockspace core and its required hashing closure to its previous
versions. Its compiler declaration is 1.95; core-only/test-support and hot-reload
consumers retain separate 1.93 CI checks.

The first plain Windows cross-check stops in `zstd-sys`: Apple Clang lacks the
MSVC C headers (`string.h`). Retrying through the repository's existing real SDK
path succeeds:

```text
PATH=/opt/homebrew/opt/llvm/bin:$PATH CARGO_BUILD_JOBS=2 cargo xwin check --cross-compiler clang-cl --locked --target x86_64-pc-windows-msvc -p open-gpui-windows --all-targets --features test-support
```

This Rust 1.100 check includes the new native outer-frame/conversion and hidden
placement/reveal tests. It does not execute Windows, FXC, physical capture or
mixed-DPI behavior. Independent review found that the old pending hidden-placement
projection discarded outer geometry, causing strong reveal equality to reject a
valid native window. The corrected owner retains the complete same-sample native
observation through the reveal ticket; comparison remains exact. Four additional
regressions cover complete projection, missing observations and topology drift.

The actual Rust 1.93 locked `--all-features --all-targets` check for GPUI core,
UI components and macOS passes after the platform additions. The Rust 1.95
locked equivalent for the eight docking/tooling consumers also passes, including
DevTools, all docking examples, Gallery and xtask. These checks use the declared
compilers rather than `--ignore-rust-version`.

The subsequent 1.100 all-feature library regression across Core, macOS and both
docking implementations passes **3,612/3,612**, no skips, nextest
`fd65e01a-e03d-4841-a5fe-cb1caed745db` in 84.057 seconds. This covers the complete
old docking library alongside the new adapter, window candidate/submission/
retirement and platform unit regressions. Linker output retains a large DWARF
unwind-table warning for the Core test binary; this is neither a runtime failure
nor a measured performance conclusion. Independent Mac source review finds no
remaining source-proven P1/P2 in the typed mutation, observation and scoped event
paths. Its common-scale/mixed-scale boundary and missing native runner result
remain explicit below.

## Implementation commits

| Open-GPUI commit | Source/reference | Delivered behavior | Executed qualification |
| --- | --- | --- | --- |
| `2c930324385c4ca9e8c1b9dd4b528c5f1b807d68` | dockspace `46da973bcc8db7cb50c72aef0f65b6acd6b59338` | Immutable dependency and explicit docking compiler floor. | Independent locked core/serde compiler probes recorded above. |
| `f2407cda` | Existing GPUI text/activation/overlay owners; pinned component references in the plan. | Numeric editing, controlled/composable Tree and external/per-row choice workflows; ordinary Gallery page and migration guide. | 361 component tests and 3 Gallery workflow tests pass on 1.100; completed review findings are resolved in `ac60cc74`. |
| `d3d65d5c` | Rust/Cargo 1.100 release; unchanged EGL patch `42214b36347b108fd9630ca6254b96dde59e04fe`. | Cargo JSON artifact lookup, independently locked EGL controls and floor-compatible compiler warning cleanup. | EGL positive 3/3, negative control reproduces original panic; real 1.93 Core/components/Mac all-feature/all-target check and Windows 1.100 cross-check pass. |
| `ac60cc74` | Independent controls review `20261113-140042-dec60131`, findings 1 and 2. | Durable disabled selection, retained latest result label with explicit owner updates, and real Gallery query/select/Apply workflow. | Both new regressions fail before correction; 547 affected component/Gallery tests pass after correction (5 unrelated Gallery library tests filtered out). |
| `c024dab5` | Official `App::open_window`, `PlatformWindow` and Mac input owners at the fixed adoption endpoint; ADRs 0030/0031. | Original open-rollback ticket, coherent outer/client geometry, target-DPI conversion, Mac typed placement/input mutation and scoped pointer coordinates; complete Windows hidden-placement projection. | 13 focused Core tests, 14 Mac unit tests, full 3,612-test cohort, real 1.93 checks and Windows SDK cross-check pass. Native mixed-DPI/input qualification remains open. |
| `6a026917` | Actual Cargo metadata resolution at the declared package floors. | Check each package's resolved dependency closure against its own compiler floor; qualify the independent full consumer at 1.95. | Six dependency-health tests, actual dependency-health gate and independent 1.95 all-feature/all-target consumer pass. |
| `1f9de1f1` | Complete Nara control-page review and multiline review `20261113-154636-f2c44c04`. | Multiline keyboard editing, original scroll-based caret reveal, revision-bound delayed paste consumed only by an accepted control publication, and protected Copy propagation. | 236 component/input/semantic/a11y/choice/overlay cases pass, real Rust 1.93 all-feature/all-target check and 1.100 Clippy pass; four review findings independently rechecked as resolved. |
| `0a00812a` | Rust 1.100 workspace formatter. | Format one pre-existing Linux clipboard license-comment terminator. | Full workspace `cargo fmt --all --check` passes; no executable change. |
| `68d52b009dde3c784b01268208cb1d0ed690e99c` | dockspace `46da973b`, ADR 0034, complete platform/adapter review `20261113-153719-084f4a64` and its bounded follow-up. | Shared semantic session, retained independent Views, exact output/input admission, managed native lifecycle, historical-pane reopening and live persistence capture; ordinary independent consumer. | 45 adapter / 1,523 entire docking tests pass, complete Core/Mac cohort and two real native smoke receipts, real Rust 1.95 check, Clippy and public/consumer gates. Full U7 physical-input qualification remains separate. |
| `6561969d` | Existing shared adapter's original ticket, candidate and affine-update owners. | Real A-Submitted/B-unaccepted-candidate overlap regression and thin existing CPU-token test facilities. | Shared adapter 46/46, independent bounded source review without confirmed P1/P2. |
| `98fb59fe` | Existing L-TOKIO adoption and later unused workspace-alias removal. | Correct the bridge README's direct consumer dependency example. | Manifest/source inspection and public/import/doc gates pass. |
| `b51c2163` | Fixed official macOS active callback timing plus the fork's original ingress/ticket contract. | Defer exact key confirmation, preserve native generation and original ingress source across delivery. | Final native 14/14, Core/Mac 2,105 plus corrected focused 1/1, Rust 1.93 all-target Mac check, correctness Clippy and public consumer 8/8. |
| `104f5dca` | User-reported accumulated probe Dock entries and existing strict OS-exit runner. | Reuse one bundle identity and unregister/remove each invocation's app in `finally`, retaining logs. | Fixed-identity native 14/14 and assertion-failure negative control; zero probe processes or Dock entries after execution. |

The upstream SHA-to-local-port mapping remains in the existing
[564-row reconciliation ledger](../../../imports/zed-gpui-commit-reconciliation-2026-11-12.md)
and [later fixed incremental record](../../../imports/zed-gpui-incremental-sync-2026-11-13.md).
These editor-layer changes extend that adopted baseline; they are not additional
official GPUI commits or a reason to recount already reconciled changes.

## Selected unit status

| Unit | Current result | Remaining qualification or follow-up |
| --- | --- | --- |
| U1 | Immutable core-only dependency, declared per-package compiler policy and independent compiler probes. | A later published dockspace compiler-floor change needs its own qualification. |
| U2–U4 | Composable scalar, Tree and searchable-choice workflows; ordinary Gallery consumer and resolved independent review. | Physical OS capture/IME results remain platform-specific. |
| U5 | One complete shared session; retained View entities; same-Window output aggregation and independent Deferred sibling progress. All 46 adapter tests pass, including original terminal delivery behind a sibling candidate. | CPU tests do not establish actual native input; precise remaining physical capabilities are listed under U6. |
| U6 | Managed transport, original creation rollback, close policy and exact group retirement have deterministic coverage. The real Mac smoke creates/submits a child, moves the original View and retires the exact group while an isolated sibling survives. | Independent OS capture ownership, native hit-stack observation, stationary desktop sampling, mixed-DPI and owning Windows runtime. |
| U7 | ADR 0034 selects shared-session/N-ary semantics; v2/v3 persistence migration has atomic validation and provenance coverage. | Production facade/caller cutover and incumbent deletion await the declared physical-input qualification. |
| U8 | Nara `0caee7fd` supplies two-View Dialogue editing, field/choice conflict resolution and original-index resource lookup; source/lock selects `68d52b00` with one Git identity. Final source passes 93 pure library cases on 1.95/1.100 and the canonical 1.95 Windows SDK compile. | Supported filesystem execution and Windows native DX12/Vulkan workflow remain separate gates. The selected development source is local and unpublished. |

The adapter's final single-pass simplification reviewed 7,428 production lines
against the reuse, quality, efficiency, test and comment rubrics. It applied nine
changes in five production files: private signature cleanup, fewer pane/title
copies, ownership transfer in terminal submission and reuse of native binding
lookup. All three adapter proof-test files and native smoke assertions remained
byte-identical. This was one combined review context, not five independent agents.

## Integration failures retained for follow-up

The first fully joined adapter run passed its seventeen persistence and five
native transport tests, then failed to park in the Deferred-window content test.
After 129 seconds the task-owned runner was interrupted. A process sample shows
repeated GPUI native ingress, App effect flushing and adapter prepaint. The adapter
treated presentation-provenance changes as repaint requests and marked its own
newly encoded regions dirty again. The correction distinguishes
`repaint_surfaces` from `affected_surfaces`, clears encoded regions on candidate
acceptance and does not redraw a still-pending output merely for its dirty flag.
The original test, strengthened with bounded redraw, stable idle serial and the
original ticket's eventual Submitted observation, passes in **1/1** nextest
`4ffd9d5f-7558-40f6-ade9-afa3ad702f24`.

The subsequent complete adapter run executes all **31 tests: 28 pass, 3 fail**.
The remaining failures concern real View content after relocation and managed
mounting; they are not satisfied by a recording-only packet or a diagnostic
placeholder. Their original assertions remain and the integration unit is open.

The next correction preserves mounted empty space when core deletes a vacated
ordinary source surface, and re-encodes explicitly Deferred bootstrap packets when
real native snapshots become ready. The strengthened original View/entity/click
assertions now pass. The complete adapter cohort is **31/31 passing**, nextest
`f3935ddb-2f1d-4161-b0a5-dcde87e4c796` (1,476 incumbent tests outside this filter).
This includes the single-Window shared-ticket and multiwindow submitted-artifact
paths, original entity state across relocation, Deferred-sibling progress,
pre-root creation rollback, exact shutdown and durable migration. Its virtual
platform does not establish actual desktop capture or AppKit execution.

The native consumer builds on Rust 1.100. Its first real LaunchServices run exits
nonzero before creating a window: the platform cannot establish an atomic display
publication. An immediate independent `CGGetActiveDisplayList` query returns success
with **0 active displays**. The retained invocation log directory is
`/var/folders/tg/q14b6w8d35d06hr_g4n7t8vw0000gp/T/open-gpui-macos-probe-t3qtxq_1`.
This is a failed native run with an unavailable display environment, not a passed
two-window qualification or evidence that the new native placement code executed.

The active display later returned. Actual AppKit runs then exposed three joined
timing defects that the initial deterministic cohort did not cover: native ingress
started another host frame while an original unaccepted candidate held the affine
update; child bootstrap queried a surface outside the current presentation roster;
and one Window's terminal output tried to settle while another Window still held
its unaccepted candidate. Their failed invocation logs remain at
`open-gpui-macos-probe-2de_f3kw`, `open-gpui-macos-probe-1_6b4gp7` and
`open-gpui-macos-probe-fmbrmw1c` under the same task temporary root.

Ingress now retains its original FIFO until acceptance or withdrawal. A waiting
sibling retains visual content, exposes no new docking hit facts and wakes when
the lease ends. Terminal output reports remain in the original presentation
sidecar and settle in a fresh frame after the active candidate resolves. A real
child outside the presentation roster emits an empty Deferred staging packet,
without a content-readiness acknowledgment. The application can inspect
`DockSurface::is_update_pending` before issuing a synchronous semantic intent;
an accepted output awaiting renderer completion does not hold that lease.

The test platform now supplies its zero-decoration outer-to-client request
conversion, leaving physical observations under its original mutation owner.
The new child test actually flushes that queued mutation before claiming Prepared,
then checks the original content, click, native admission and group quiescence.
The second new regression renders a sibling while the original affine update is
held, verifies that it publishes no docking input facts, then withdraws and proves
normal content/presentation recovery. The final adapter cohort is **33/33**,
nextest `0ace1188-33f9-4853-abfa-c83b6b4c3588`. The broader Core/Mac/docking cohort
with the test-platform conversion passes **3,614/3,614**, no skips, nextest
`a10fc786-7041-45b8-8ea9-5c55a722db91`; the subsequent timing correction is covered
by the fresh 33-case adapter receipt. The final docking/example all-feature,
all-target check also passes on the declared Rust 1.95 compiler.

After correcting the probe to wait for update readiness only when it actually
issues a semantic intent, the real LaunchServices run passes and exits normally
with status 0. Its retained invocation log directory is
`/var/folders/tg/q14b6w8d35d06hr_g4n7t8vw0000gp/T/open-gpui-macos-probe-lwose4a6`.
The success marker reports a created child, two real Submitted Windows, original
entity/local-state retention through relocation, shared edits, dependent/anchor
native terminal plus binding quiescence, and survival/retirement of an isolated
sibling workspace. The timeout from the earlier overly broad probe readiness
guard remains in `open-gpui-macos-probe-8vn6uzkl`. This finite native result does
not qualify physical pointer capture, mixed-DPI travel, OS IME or Windows runtime.

The initial Nara canonical fixture selects Open-GPUI `f75c10b2a9bf7b4dcb2c8c2aa446203997601369`.
Its first locked run fails resolution: Nara UI's exact Taffy 0.10.0 conflicts with
the sibling Jellyflow adapter's registry GPUI 0.2.0 requiring exact Taffy 0.10.1.
A temporary independent manifest selects current local GPUI paths and patches
the registry identity, retaining the original source/test files and Nara target
directory. This compiles the actual Dialogue workflow and all twelve document
tests with Rust 1.100. Execution is **0/12 passing**: every test fails at original
Session Open with `project.editor.prefabs-open-failed`. Nara's existing
`nara_fs::platform::preflight_exact_spelling` explicitly supports Windows/Linux
only, and non-Linux Unix does not prove local file identity semantics. These
contracts were not bypassed for a Mac test receipt. Windows/Linux Session/file
execution remains required; the temporary path-selected build does not establish
a new remotely fetchable source pin or native Editor qualification.

The Windows SDK cross-check of the actual Dialogue page subsequently passes with
the temporary current-source manifest, Rust 1.100, `x86_64-pc-windows-msvc`,
`--example gpui_view --features dialogue-workflow,ui-work-observation`. The first
attempt found a wrong `ListboxOption` import and an untyped keyboard callback;
the corrected run is `/tmp/nara-editor-dialogue-workflow-windows-fixed-check.log`.
The portable Nara consumer library also passes **83/83**, no skips, nextest
`b82a860b-2faf-461c-aec3-b931e2d7d2ac`. These pure document/graph tests keep their
scope separate from the twelve failed real Session/file tests.
The actual Windows example also passes the same SDK check on its declared Rust
1.95.0 compiler, `/tmp/nara-editor-dialogue-workflow-windows-msrv.log`, without a
compiler-floor override.

The final canonical Nara manifest and lock select immutable local Open-GPUI
`68d52b009dde3c784b01268208cb1d0ed690e99c`. Registry overrides for GPUI/components
resolve the earlier optional Jellyflow/Taffy conflict, and the EGL Git correction
remains explicit. Cargo metadata records **23 GPUI packages from one exact Git
source**, one selected WGPU Git identity, and separate Nara/GPUI Taffy versions.
Resolution fetched the exact Git objects through a per-process URL rewrite to the
adjacent repository; no global/repository Git config changed and no source was
pushed. Another runner needs those objects or a later authorized publication.
The actual **canonical** locked Windows SDK example check passes on Rust 1.95,
`/tmp/nara-editor-dialogue-canonical-windows-msrv.log`. Its canonical library
tests pass **83/83**, no skips, nextest
`d608554f-2dd5-4f6d-838e-7938c008673f`. These replace temporary path compilation
evidence for the selected development source, without claiming native execution.

An independent review of Nara's complete page found two additional P2 defects:
multiline controls lacked ordinary editing-key actions, and deleting a property
target prevented selecting a surviving node because selection reused the old
node-incarnation gate. The latter now checks the View/document/source lifetime
for selection, while Text/Apply retain strict node identity. Native smoke source
adds delete-without-Undo recovery, replays the retired callbacks and uses the
ordinary Winit converter for character input, Enter, Backspace and four arrows
with the original UTF-16 caret and one history boundary. This is compiled source,
not a native DX12/Vulkan execution receipt.

The generic multiline correction stays in `TextareaController` and its original
`EditableTextDocument`. Shared text initialization registers separate multiline
bindings. Editing, vertical preferred-column navigation, directional selection,
clipboard, disabled/read-only and active-composition behavior retain those owners.
Delayed paste rechecks current focus, rendered membership and editability. Seven
new real-input regressions plus TextInput, accessibility, semantic activation,
choice and overlay cases pass **228/228**, no skips, nextest
`d824b277-0369-4646-a296-d24195724841`. The first test-target build failed because
`Keystroke::parse` requires a static string; test helpers now state that lifetime
explicitly. The actual 1.93 all-feature/all-target component check passes after
the correction. Nara's clipboard interoperability and physical IME remain separate.

## Independent controls review and closure

The completed `ce-code-review` receipt is
`/tmp/compound-engineering-502/ce-code-review/20261113-140042-dec60131/review.json`,
with scope fixed to `2c930324..f2407cda` in a separate immutable checkout. It records
six lenses and two confirmed P2 findings: a disabled external result dropping
durable selection, and a previously renamed result falling back to unchanged stale
`selected_item` metadata. Both were reproduced in a real **0/2** run before fixing
the owning model/runtime. Selection eligibility remains distinct from activation;
runtime tracks explicit owner metadata changes instead of overwriting a later
result label on every render. Duplicate identities still fail closed.

The review also identified that the Gallery catalog test exercised the provider
directly. A separate complete workflow now uses the actual text input, receives
results, selects a material with the keyboard, applies it and navigates to another
object and back to inspect the saved value. The original provider-order/error
tests remain. The resulting component-library, choice/composition/semantic/theme/
overlay and Gallery selection is **547/547 passing**, nextest
`58176a13-642f-4994-9fdb-dbf570834e1a`; five unrelated Gallery library cases are
filtered out. The two retained findings are applied locally in `ac60cc74`; no
confirmed control-review finding remains unresolved. Physical OS capture/IME
qualification is still separate.

Public API, import-boundary, release-doc and public-link gates pass. The UI
contract gate initially rejects duplicated hardcoded dock chrome colors/shadows.
The adapter now maps its complete inputs from the existing `DockVisualStyle`
owner, preserving all supplied floating-shadow layers; the scanner and allowlist
are unchanged. The complete federated UI-contract gate then passes, including its
registered test invocations. The isolated 1.100 EGL positive rerun with `--locked`
and nextest 0.9.151 passes **3/3**, run `548dfed3-68c3-48d4-a396-3ea0a95f7267`.

## Final review follow-up

Two additional actual `ce-code-review` runs completed against immutable review
snapshots. Their receipts remain unchanged; the following resolution work is
separate from those original reports.

| Review | Scope | Confirmed follow-up |
| --- | --- | --- |
| `20261113-153719-084f4a64` | `ac60cc74..1f607e83a06a1571d4e79581bda935ff6f4bfbaa`, platform/compiler/shared adapter | Seven findings: pending-output pane input, failed root registration, reopened historical pane registration, root rollback ticket preservation, live document capture, failed-open mount cleanup and captured-event FIFO. |
| `20261113-154636-f2c44c04` | `6a026917..434236218b90b2b61198e68d2377b686174b66ea`, multiline editing | Four findings: caret reveal, original paste basis, owner changes before redraw and Copy propagation during composition/disabled state. |

Each receipt has `status: complete` and `verdict: Ready with fixes`. The first
uses six independent Codex review contexts; the second uses five. They are not
different-provider reviews. No reviewer ran Cargo or a native application, and
the reports do not claim measured defect incidence. Both review trees remained
clean. The parent owns source changes and serial verification.

The adapter corrections are verified: pane content uses the
existing `SubtreePresentation::Inert` until its original Submitted artifact is
admitted; the terminal callback requests one replay to enable content channels.
An original native-terminal callback additionally retires provisional mounts when
creation rollback emits no ordinary close event. Historical-pane opening and
live capture stay on the sole session, and root rollback/FIFO fixes preserve
the original typed native authorities. A bounded independent source recheck marks
all seven original findings resolved. It found one further P2: a queued old drag
terminal set a Boolean cancellation flag which could later cancel the replacement
generation. Cancellation now retains each original source/generation, queues
behind an existing release barrier and clears only that matching capture. Two
real GPUI drag regressions retain G1 terminal plus G2 movement, collect/dispatch
them together and prove that G2 stays active and continues delivering its own
generation. No drag generation is manufactured by those tests.

The first joined review regression executes **40/43 passing**: two root tests
incorrectly use initial completion-command refusal as if it rejected renderer
submission, and the FIFO fixture asserts an empty queue before settling its
original root/activation observations. The FIFO fixture now collects and commits
those original observations before taking the test's update lease. The next root
attempt forcibly closes the native window during first presentation and retains
the resulting failed quiescence assertions; native destruction before ordered
renderer retirement is not the intended rejection scenario. The small public
test facility `set_next_window_present_outcome` selects Rejected before the first
draw instead. Both tests then prove the original ordered rollback ticket,
mount/registration retirement and exact same-surface retry, without inventing
native-terminal or quiescence facts.

Final review qualification:

| Scope | Actual result |
| --- | --- |
| All shared adapter tests | **45/45**, nextest `5fc9a54b-986b-483b-9c32-541363dd7221`; 1,478 incumbent tests outside this focused filter. |
| Entire old and new docking library | **1,523/1,523**, no skips, nextest `583fc1ad-b3e7-433e-9b32-00ea85679936`. |
| Core and Mac complete all-feature libraries after the test-platform addition | **2,103/2,103**, no skips, nextest `b54baf47-bda9-4d45-a5a2-a185aef82f47`. Together with the docking run this covers 3,626 library cases on the final source. |
| Textarea and affected input/semantic/a11y/choice/overlay cases | **236/236**, no skips, nextest `2d5c5c63-4bd5-4513-876c-0d8d4f6c329a`, including fifteen keyboard regressions. |
| Real Mac native smoke after all review fixes | Normal exit 0; `open-gpui-macos-probe-s3o4gorf` under the recorded temporary root. Original child/Submitted/entity/state/group/sibling assertions all pass. |
| Compiler floors | Actual locked 1.93 all-feature/all-target component and 1.95 docking/example checks pass after review fixes. |
| Clippy | Actual 1.100 all-feature/all-target component/docking/example run exits 0. Existing and advisory warnings remain; this is not a warning-free claim. |

The final ordinary independent consumer initially catches a feature-closure error:
the new TestPlatform first-present injection called a `test-support`-only method
from an unconditional branch. Its storage, setter and call now share the same
test/test-support condition. The corrected actual Rust 1.95 locked all-feature,
all-target consumer passes without enabling test support. Full workspace format,
public API, import-boundary, documentation/release and federated UI-contract gates
pass after the final edits. The latter includes its original registered tests;
neither scanner nor allowlist was relaxed.

The multiline source recheck independently resolves all four findings. Its
accepted-publication paste path validates the edit revision again after owner
properties synchronize; changed text, directional selection (including ABA),
composition, read-only/disabled state, retirement or publication discard cancels
the request. Caret assertions use the original input handler's actual rectangle
inside the original scroll viewport. Copy tests assert both unchanged clipboard
content and delivery to the parent during protected states. The passing suite
retains the original tests and their full assertions.

## Fixed-upstream closure recheck and native activation follow-up

Two independent source lanes recheck the original fixed goal at local `68d52b00`.
The canonical set is **564 expected / 564 rows / 564 unique SHAs**, with no missing,
extra or duplicate entry; all 149 root/toolchain slices, the original eleven queued
commits and the partial WGPU branch retain their actual local owner mappings. The
[canonical ledger](../../../imports/zed-gpui-commit-reconciliation-2026-11-12.md#inventory-and-state-counts)
records the exact pathspec and set fingerprint. A separate final-tree inspection
confirms the seven iOS risk chains, shared Apple, byte-equivalent normalized Tokio,
five pinned EGL roots and the final Windows/WGPU sprite/COLR paths. This adds no
official commit row and does not move either fixed endpoint.

The original raw receipts were located and read under the task temporary root
`open-gpui-completion-20261112-t87dkbpe`: `complete-affected-nextest-final.log`
contains F-ALL's run `a5c5f0e9-7fcf-45bc-b489-5393662e1e27`, **3,615/3,615**, three
existing skips; `independent-public-runtime-final.log` contains F-PUBLIC's
`a7159fe9-15dd-487b-9d4f-061cb549ce2b`, **8/8**, zero skips. The complete
`ios-native-simulator-final.log` contains the actual install/console launch and
the **2026-11-13 01:39:02 +08:00** `IOS NATIVE WINDOWING OK` marker, retaining the
appearance-transition warning. These remain historical Rust 1.99.1 receipts;
their existence is not inferred from an app bundle or a launcher exit code. No
new simulator GUI is needed to recover that evidence.

The first current Rust 1.100 AppKit rerun is **11/14**. Default, windowed and
headless fail the original exact-window/positive-callback assertion, while the
other eleven finish normally. The original **10:40** successful binary also fails
windowed/headless in the same current environment. A pure AppKit control, with
no GPUI or renderer dependency, records `DidBecomeKey` while `isKeyWindow=true`
but `NSApplication.keyWindow` still names a different value; after the notification
returns, the exact key-window identity matches. Diagnostics do not remove or
relax the probe's callback assertion.

The fixed official `6ceb6fee:crates/gpui_macos/src/window.rs` defers the final
application callback through its foreground executor while retaining synchronous
frame delivery. The fork restores that timing in the existing observation path.
It preserves the spurious-popup `resignKeyWindow` correction, synchronous frame
request, native FIFO and accessibility effects. The deferred confirmation uses a
weak window owner, checks close/quiescence, rechecks live exact key authority and
retains the original native-event generation across pending observation batches.
Reentrant key changes cannot give an old event a new positive authority.

Independent review additionally finds that deferral must preserve the original
application-ingress source: an old A gain delivered after a queued B activation
could otherwise receive a newer sequence and incorrectly settle B as Superseded.
`on_active_status_source` captures from the existing Core sequencer at the native
edge, outside backend locks. The deferred observation carries that opaque source;
ticket comparison uses its original sequence while the ordinary delivery envelope
keeps its actual FIFO sequence. No second sequencer or activation owner is added.
The real AppCell/TestPlatform regression fails with the old delivery sequence,
**0/1**, run `67e517b6-7cbb-4c6c-a55f-51bb7dc59ef3`, reporting the wrong
`Terminal(Superseded)`.

The first integrated source run passes **2,105/2,106**; its only failure is the
new fixture expecting Dispatched even though the real TestPlatform already emits
B's own successful key gain. The fixture now subscribes before the original
queue drains and requires **Activated exactly once**, including after replaying
the old A observation. It passes **1/1**,
`2c49cadb-09ef-45ad-be4d-8fb7eb27cffd`; the other 2,105 unchanged library cases
retain their passing combined-run receipt. This is not reported as a single
2,106/2,106 invocation.

Before the ingress-source follow-up, the deferred native timing passes the full
**14/14** matrix in **26.535 s**, every case with marker plus real OS exit 0,
receipt `open-gpui-goal-macos-deferred-activation-7svucaev/receipt.json` under the
task temporary root. The native fixture additionally requires callback delivery
after the original activation request returns and validates the actual key-window
pointer. Final-source native qualification is recorded separately after the
ingress-source change.

The final `b51c2163` implementation and `104f5dca` runner execute all **14/14**
cases again in **21.832 s** using the fixed identity, Rust 1.100 and current SDK
27.2. Receipt: `open-gpui-goal-macos-original-source-final-on0xe1jh/receipt.json`
under the task temporary root. Each case has its marker and actual normal exit
0. Process and Dock-preference inspection after the complete run shows **zero**
live GPUIProbe processes and **zero** probe fixed/recent entries. The final
fixed-identity invalid-mode control still returns 1 from the original assertion,
retains stderr and removes the app. No additional Dock refresh is needed after
these final runs.

The final source passes actual Rust **1.93** macOS all-target/all-feature
compilation, complete root formatting and Core/Mac all-target/all-feature Clippy
with correctness and suspicious categories denied. Advisory warnings remain.
Independent bounded review marks the original-ingress P2 resolved and finds no
further confirmed P1/P2 in the activation or runner changes. Public API, import,
doc-link and release-document scans pass. The exact independent public consumer
renewal now passes **8/8**, run `965687fa-9528-455e-87d2-e551145ae8fc`, without
test-support, after the user's Xcode license correction. Its original failure
log remains retained.

The independent WGPU/ordinary `linear_scene` Rust 1.100 cohort is **131/131**,
zero skips, run `e1518ecb-ed87-4e4f-aa08-a9048977786b`. The independent public
consumer's initial renewal stops at the actual Xcode license error; the user
subsequently resolves it. Current Xcode reports macOS SDK **27.2**, so later
commands do not inherit the historical SDK 26.5 claim.

## Probe cleanup and original cross-window regression

The user reports accumulated GPUIProbe Dock icons. Process inspection and
NSWorkspace show no live probe, but a fixed Dock entry names one task-created
temporary bundle. That exact entry is removed and Dock is refreshed. **59** old
task probe bundles are unregistered and removed after checking no matching process
is alive; stdout/stderr and bare binaries are retained. No application process is
terminated during the Dock refresh.

The runner now reuses one probe bundle identifier, still uses `open -n` and an
isolated executable path for every fresh process, and removes its own temporary
app in `finally` after actual native exit. It retains logs and fails on cleanup
errors. LaunchServices' already-unregistered `-10814` outcome is handled explicitly.
The 14-case positive run and an invalid-mode negative run verify cleanup before
the fixed-identifier amendment; the negative case preserves the assertion and
normal status 1 with no live app. Final fixed-identifier execution is recorded
separately rather than credited from the earlier unique identities.

`6561969d` adds a real cross-window regression: A has its original accepted but
renderer-deferred output, B retains an original unaccepted deferred candidate
and affine update, and A's Submitted terminal waits without granting input until
B's own token completes. The test then verifies admission of A's original exact
artifact and an actual pane click. Two thin test-support wrappers use existing
TestWindow delivery/token facilities. Three earlier manual-lease fixture attempts
are retained as failed setups; they are replaced by the real original-token path,
without changing the production scheduler or reducing assertions. The final
shared adapter cohort is **46/46**, run `09a5b51e-a285-4fa8-b81e-ccc39819d1d9`,
with 1,478 incumbent tests outside this filter. An independent bounded source
review finds no confirmed P1/P2 in the test, wrappers or migration documentation.

## Final Nara consumer source

The original-owner Browser seam is local Nara
`e6cd99a09d414700a7329180af43e0fbba0ac85f`; the complete Dialogue consumer is
`0caee7fdf73af0ddf5274c9fcfc8a677875a364b`. These are commits in the separate Nara
repository, not Open-GPUI ports or new official SHAs. The consumer deliberately
retains exact framework pin `68d52b009dde3c784b01268208cb1d0ed690e99c`.

The completed six-lens review `20261113-165020-dff6809b` confirms four findings
against immutable predecessor `1e2dc6be47bfbe5b34d7e8b5271de779fd3fc3d7`. All four
now have source corrections: three-way field and stable-choice merging with
explicit Current/Draft decisions, source/incarnation-bound node actions, independent
bounded preview scrolling and resource lookup over the original Project Browser
index. A bounded independent follow-up catches Review/Remove/Current silently
clearing a missing choice field. Resolution now restores the complete current
member or retains the conflict on failure; the independent recheck closes it.
The original full review receipt remains unchanged.

The resource UI reuses the original immutable index and existing per-View query
state, with generation/epoch refusal and original asynchronous Scene/Prefab Open
results. The original Session/document/history/file and RenderHost owners remain.
Native smoke source includes Chinese/long paths, independent queries and selection,
late-index refusal, actual surviving-View edits/Cancel restoration, and Apply/Undo
queued before retirement. Compilation does not qualify that Windows trajectory.

Final actual pure library execution is **93/93** on Rust 1.95,
`734e2acd-0b1b-43d0-ada7-745f76cc9eb4`, and **93/93** on Rust 1.100,
`358ed07d-bd75-471e-b2ce-2e1034af87fb`. The exact canonical Windows SDK example
passes `cargo +1.95.0 xwin check --cross-compiler clang-cl` with
`dialogue-workflow,ui-work-observation` in 22.75 seconds. The current Session test
target also compiles without running. All use one Cargo build job and the existing
Nara target directory. Historical Mac Session **0/12** remains an actual original
Host Open failure; NaraFS's unsupported exact-spelling/local-identity guarantees
were not bypassed. Windows DX12/Vulkan execution, graph gestures, physical input
and Editor-default selection remain separate follow-up.

## Shared native Root continuation follow-up

Reviewing dockspace's newer native-window helpers exposed a production-adapter
retirement defect at Open-GPUI `76e2c6463944ddfaba00ae71676d14d3741dfc12`.
Two logical placements in one physical Root produce two continuation pairs with
the same predecessor/successor window. Moving the physical callback entry while
processing the first pair made the second pair skip its placement-retirement
notification. The bridge now records each distinct predecessor independently
of physical callback movement and deduplicates pending retirement notifications.

The new shared-Root regression fails against the original implementation with
one reported predecessor instead of two. After the correction, the full shared
adapter cohort passes **47/47**, nextest run
`e5e9f1d8-9a15-4e3b-80ae-1dcff77748b1`, with 1,476 incumbent tests outside the
filter. Independent review finds no confirmed P1/P2. Adding explicit notification
cardinality and nonempty late-callback assertions retains **47/47**, run
`18bf55e2-4e6d-4f01-89d8-f78431f5e5ae`. The regression also checks callback identity, the surviving anchor,
repeated report delivery, late predecessor callbacks, and quiescence of both
placements while the physical window remains live. Its intermediate fixture
needed to drain and consume the original queued stale callback before claiming
quiescence; this did not change production retirement requirements.

At the end of that initial review, production remained dockspace `46da973b`;
inspecting a newer local source did not update its immutable pin or qualify its
uncommitted changes. The accepted integration follow-up selects `c200977d` after
separately integrating and qualifying the committed source.
These are deterministic GPUI test-renderer results, not new physical-input or
native-renderer qualification. No native probe was launched for this follow-up.

The [dockspace integration review](dockspace-integration-review-20261113.md)
records the newer native helpers, candidate motion fixes, isolated source pins,
test results and recommended adoption order.

## Accepted dockspace helper integration

The follow-up upgrades the production dependency from `46da973b` to
`c200977d0f34943284a125c492919791650e1bff` on the published dockspace
`integrate/gpui-host-contracts` branch. The integration record maps all four
earlier corrections to their commits on `6e4a2366`, plus the newly discovered
native Dock Back preflight defect. Core/egui qualification is **2,362 passed,
3 pre-existing skipped**, with actual Rust 1.95 core-only `serde` compilation.
Concurrent dockspace main/harness development remains outside that source claim.

Open-GPUI `ffef5715` pins the dependency; `620ee058` replaces the bridge's map
and reverse scan with `NativeWindows`, retaining bounded opening custody and
original rollback tickets. `64e0fbb7` reports the undecorated test backend's
known physical outer observation. `14f03fb4` accepts exact native staging
placements and encodes their first-live roster even before the published layout
contains the surface. Its regression retains one View through partial tear-off,
Dock Back, actual content interaction and shutdown. Core preflight now refuses
missing source outer geometry using the same recovery preparation as submission.

The complete all-feature Core+docking run is **3,505 passed and 1 failed**,
`428164de-d936-44cd-85b5-52e91d69dbf1`; all **49** shared-adapter cases pass.
The only failure expected the test backend's old absent outer field. Its
full-snapshot expectation was corrected, and the geometry/drag-start cohort is
**3/3**, `881e23b5-afdc-4ee6-9fea-c2a3386ec36f`. Final adapter/native-capture/tab
coverage is **88/88**, `c9981dbb-ebb2-4e8c-ab50-1753ef120fa9`, after the small
existing-path lint cleanup in `5c0b8430`. These remain distinct receipts.

Actual Rust 1.95 all-target/all-feature docking, editor and independently locked
public-consumer compilation passes. Core/docking/editor Clippy with correctness
and suspicious categories denied, workspace formatting and the existing public
static gates pass; advisory warnings remain. The
[integration record](dockspace-integration-review-20261113.md) retains log paths,
initial failures, source/compiler boundaries and remaining physical-input gates.

The initial real Mac renewal of this integration does **not** pass. Its first bounded attempt times
out in `ChildAdmitted`; diagnostic renewals first hit unavailable atomic display
publication (`CoreGraphics returned no active displays`), then time out at root
submission after a temporary display-wake assertion. Read-only console inspection
confirms `CGSSessionScreenIsLocked = true`. The first child timeout is not thereby
proven environmental. All diagnostic probes and temporary apps are closed, and
diagnostic source/root-layout changes are removed. A later exact-source run at
`1686f58c` passes on the active desktop in 2.114 seconds, including child
admission/submission, retained View edits, group retirement and surviving sibling
isolation. The [lifecycle review](lifecycle-boundary-review-20261113.md) owns its
actual OS exit-0 receipt and cleanup, the additional native close/layer checks,
and subsequent Core/docking defect corrections. This later success does not
retrospectively establish the cause of the initial child-admission timeout.
