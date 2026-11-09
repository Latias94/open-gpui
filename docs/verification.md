# Verification

Run the local Open GPUI gate with:

```sh
cargo run --locked -p xtask -- verify
```

The gate runs:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --locked` (including workspace examples such as `open-gpui-docking-minimal`)
- `cargo check -p open-gpui-smoke-native --locked`
- `cargo nextest run -p open-gpui-motion --locked`
- `cargo test -p open-gpui-motion --doc --locked`
- `cargo nextest run -p open-gpui-form -p open-gpui-resource -p open-gpui-devtools --locked`
- `cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked`
- `cargo test -p open-gpui-devtools --all-features --doc --locked`
- package-wide locked nextest gates for `open-gpui`, `open-gpui-ui-core`,
  `open-gpui-ui-components`, and `open-gpui-ui-foundation-gallery`
- locked doctests for `open-gpui-ui-core` and `open-gpui-ui-components`
- `cargo run -p xtask -- verify-release-docs`
- `cargo run -p xtask -- scan-doc-links`
- `cargo run -p xtask -- dependency-health`
- `cargo run -p xtask -- scan-theme-drift`
- `cargo run -p xtask -- scan-theme-schema`
- `cargo run -p xtask -- scan-import-boundary`
- `cargo run -p xtask -- scan-public-api --check`
- `cargo run -p xtask -- scan-ui-contract`

On Windows, `xtask verify` sets `CARGO_BUILD_JOBS=1` only on the two DevTools all-features child
processes. It does not mutate the parent shell environment.

## Independent Dependency Consumer

The separately locked [dependency consumer](../tests/fixtures/dependency-consumer/README.md)
compiles the public framework, native platform, components and docking together with application-
selected Taffy 0.15.0 and WGPU 30.0.1. Its optional `wgpu-renderer` feature also compiles the actual
WGPU renderer under that selected patch. On macOS it additionally selects Core Foundation 0.10.1.
The library accepts compatible versions; the application and workspace lockfiles retain their own
resolved versions.

Run from the repository root, reusing the ordinary target and one build job:

```sh
cargo check --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all-targets --all-features --target-dir target -j 1
```

For a deliberately fresh dependency-resolution check, create a temporary copy of the consumer
without its lockfile and point its path dependencies at this workspace. The original `spin`
0.10.0 entry remains buildable in an existing lockfile but is withdrawn from fresh resolution;
the current direct dependency uses 0.12.3. Do not regenerate the workspace lockfile just to make a
consumer resolve, or replace its other tested package versions incidentally.

This gate proves source consumption and public composition only. It launches no native window,
requests no GPU, and does not qualify renderer sharing, OS text input, docking parity or a game-
engine integration. The macOS-specific dependency is compiled only on its owning target.

The November 5 maintenance successor to `89cb76ba` was checked on Windows 11 with Rust 1.95.0,
one build job and the consumer's own lockfile. The all-target/all-feature consumer check passed,
including the actual WGPU renderer with WGPU 30.0.1 and Taffy 0.10.0. The root lockfile retained
WGPU 30.0.0 and Taffy 0.10.1; only its `spin` record and reference changed to 0.12.3. The selected
core/components/docking library cohort passed all 2,590 cases, with no skipped selected cases
(nextest `447739ea-48cb-4e78-a85f-5cb05769a64f`). The locked minimal docking example compiled;
root/consumer formatting, import-boundary and public-document checks passed. Independent review
found no outstanding correctness findings in this dependency/consumer change. These results do
not qualify Linux/macOS compilation, the declared Rust 1.92 floor, OS input, a rendered engine
View, the pending upstream correctness ports or the canonical-docking cutover.

## Element Arena And Native Frame Maintenance

The November 5 arena correction follows upstream `ec3d887507` and `2529740084` while preserving
this fork's AppCell-owned native event delivery and candidate-frame transactions. App holds its
element arena in a separate `Rc` allocation. Draw scopes retain that allocation, restore TLS on
unwind and defer nested clears. A clear token holds weak allocation identity and requires the
original App; it does not dereference a raw captured App field or retain abandoned elements.
All downstream draw/clear callers use the same captured App explicitly.

The allocator also checks integer bounds before pointer adjustment and stores the exact allocation
layout for deallocation. Miri exposed an existing assumption that an alignment-1 allocation could
always fit an 8-byte value in an 8-byte chunk. New chunks request the needed alignment; reuse
skips unsuitable existing chunks without moving live storage. The regression changes allocation
order from low-alignment chunks to over-aligned values and proves stable repeated capacity.

Run the focused framework gate serially:

```sh
cargo nextest run --locked -p open-gpui --lib -E 'test(arena::tests::) or test(window::element_arena_tests::) or test(window::frame_pump::tests::) or test(native_event_ingress::) or test(cached_paint_atlas_tests::)' --test-threads=1 --no-fail-fast
```

The [Miri fixture](../tests/fixtures/arena-miri/README.md) compiles the production allocator file
directly, with its original tests and new deferred-clear/alignment tests. Run both default Stacked
Borrows and `MIRIFLAGS=-Zmiri-tree-borrows` using nightly. This is allocator evidence, not Miri
qualification of the complete App, TLS scope, Window, renderer or native callback graph. The
ordinary framework gate separately exercises real nested `open_window`, held outer allocations,
cross-App restoration, unwind, wrong-App token refusal and one-time forced cache refresh after
throttling. Native HWND and OS IME qualification remain separate checks.

The Windows native gate is:

```sh
cargo nextest run --locked -p open-gpui-windows --lib --test-threads=1 --no-fail-fast
```

`two_hwnd_reentrant_paint_defers_ime_until_native_frame_command` paints window A while sending
two synchronous `WM_PAINT` messages to window B with a committed text handler. It proves immediate
update-region validation, coalesced but retained requests, post-borrow IME query/command delivery,
no invariant failure or caught native panic, and restored `WM_CHAR` input. The backend uses the
existing native command queue and exact registration checks, rather than adding another draw
coordinator. Initial-presentation completion and renderer submission retain their original owners.

DirectManipulation sampling now occurs in that safe command after the frame draw. Changes from
that sampling can appear in a later frame. Real touchpad scrolling, inertia, pinch and response
latency have not been qualified by this HWND/IME test or by the state-only docking tests; retain
that specific timing comparison before claiming input/performance parity. Full OS composition,
interactive global-pointer tests, Linux/macOS behavior and the complete App/TLS Miri graph also
remain outside this local result.

Executed on Windows 11 with Rust 1.95.0, one build job and the shared ordinary target:

| Gate | Actual result |
| --- | --- |
| Focused arena/frame-pump/native-ingress/atlas cohort | 42/42 passed; nextest `417e0ac1-b294-409f-be06-dd5c13ed25e7`. |
| Core/components/docking/Canvas/DevTools library cohort | 2,984/2,984 passed with no skipped selected cases; nextest `606c3993-5108-4a41-bd76-9a34fa4677c2`. |
| Component integration cohort | 598/598 passed across 18 integration binaries, no skipped selected cases; nextest `5dc812ad-ae15-4789-a3a5-daaa62790093`. The already-covered library binary was excluded. |
| DevTools all-feature framework adapters | 10/10 passed; nextest `407828e0-5607-40cb-9aa1-88287f9d5bdc`. |
| Gallery composition workflow | The U11 single-window convergence workflow passed; nextest `284f46d5-7852-4f89-a855-dc79befc646f`. This is a GPUI test-platform workflow, not an OS-native input claim. |
| Workspace and independent source consumers | Locked workspace all-target compilation and the separately locked consumer's all-target/all-feature check passed, including its WGPU 30.0.1 renderer. Existing unfinished-docking and test-helper warnings remain; these are not warning-free or native-integration claims. |
| Windows backend production library | Locked compilation passed. |
| Windows backend library | 162/162 passed, four existing interactive tests skipped; nextest `cc0dfdf0-b5c0-4a1f-abee-6ee7a56470c0`. This includes the new dual-HWND paint regression. |
| Production-file Miri fixture | 10/10 passed separately in Stacked Borrows and Tree Borrows, using `rustc 1.98.0-nightly (57d06900f 2026-05-27)`. No allocator source was copied into the fixture. |
| Independent correctness review | No outstanding P1/P2 in arena custody, post-borrow frame maintenance or forced-refresh retention; the DirectManipulation timing gap above remains explicit. |
| Correctness lint and formatting | Core/Windows all-target Clippy passed with `-D clippy::correctness`; root and direct-file Miri-fixture formatting checks passed. Existing non-denied warnings remain. |
| Repository contract checks | Import boundary, public API tiers, public document links and release documentation passed. UI contract validation passed all 16 registered scenarios after local `.context` worktrees were excluded from discovery; its focused validator cohort passed 16/16 (`4d470a1c-07b8-44f7-ab27-e7b5f9005d09`). The filesystem regression preserves the local copy and admits only the current-workspace artifact. |

## TextSystem Pool Lifetime

The November 5 TextSystem unit ports upstream `e4a3732b94` into the existing text owner. The pool
previously retained complete LineWrapper values, each strongly referencing the TextSystem that
owned the pool. Returning any measured wrapper therefore made both the TextSystem and its font
backend survive their last external owner. The new pool stores only per-font/per-size character
width caches. A live wrapper still owns its TextSystem; dropping it returns the cache without
returning that strong reference. Font resolution and wrapper construction occur outside the pool
mutex. Public wrapping/truncation APIs and cache partitioning retain their existing behavior.

The focused gate is:

```sh
cargo nextest run --locked -p open-gpui --lib -E 'test(text_system::line_wrapper_pool_tests::) or test(text_system::line_wrapper::tests::)' --test-threads=1 --no-fail-fast --no-tests fail
```

The lifetime regression first failed against the original pool (`4540ef73-8680-46e7-b93a-b2798e19d13c`).
After the correction, the 20-case wrapper/lifetime cohort passed
(`504174d7-ba19-4865-8150-445aa748fb64`). Counting-platform tests prove ASCII and Unicode width
reuse without another platform layout, resolved-font and size separation, overlapping checked-out
handles, last-handle lifetime and cache return after unwind. A public headless consumer renders
ordinary text controls, closes its context and proves release through Weak handles over three
reconstructions. These are ownership and work-reuse assertions, not process-memory or timing
benchmarks. Font-generation invalidation and later line-breaking/layout corrections remain in
their own upstream units.

### Native Application Reconstruction

`native_app_reconstruction_releases_wrapped_text_system_and_direct_write` creates a real Windows
platform and App, measures mixed Unicode text with DirectWrite, paints a wrapped-text HWND,
retires its exact native registration and drops the application. It proves TextSystem and backend
Weak handles expire, then repeats the complete sequence three times in the same process.

This complete consumer exposed an independent baseline native lifecycle fault: the second
window creation aborted in the windows-core process-cached UISettings activation factory after
the first OLE apartment was uninitialized. A debugger stack placed the fault in
`IGenericFactory::ActivateInstance -> UISettings::new -> system_appearance`; a minimal test with
only repeated OleInitialize/theme-query/OleUninitialize reproduced the same access violation
(`9ad94219-22cb-46ba-8cb8-a0e6508ca680`). The native owner now acquires a fresh activation factory
through the public windows-core `factory` function for both appearance and scrollbar queries.
The factory and returned settings stay inside the current query's apartment lifetime; OLE retains
its existing balanced ownership. The minimal and complete native regressions passed together
(`be794f02-b89c-4101-ba61-9c7472aad346`); the final cohort also queries scrollbar preferences.
The activation-query cost was not benchmarked by these lifetime tests.

Run native lifecycle coverage separately:

```sh
cargo nextest run --locked -p open-gpui-windows --lib -E 'test(native_app_reconstruction_releases_wrapped_text_system_and_direct_write) or test(system_appearance_survives_ole_apartment_recreation)' --test-threads=1 --no-fail-fast --no-tests fail
```

These assertions prove the exercised text and platform ownership can reconstruct in-process.
They do not establish renderer-memory bounds, editor code hot reload, a general leak-free App,
full OS composition, touchpad latency, Linux/macOS execution or Nara's engine-View integration.

The complete Core/Windows library run `148d842a-52c0-46a9-8f08-799380429e1f` passed all 841 Core
cases and 163 of 164 selected Windows cases, with four existing interactive cases skipped. The
unchanged `asymmetric_activation_policy_preserves_click_and_programmatic_independence` timed out
at its 512-message-pump bound. Its unchanged focused reruns passed separately
(`755328c7-6405-4c64-9852-d5e7973eff9e`, `bc30d4ef-eee6-4648-a547-2c2e90b0fffd`). Retain the initial
failure as a timing observation; the successful reruns do not establish its cause or convert
that initial cohort into a first-run pass.

The subsequent complete Windows library run passed 164/164 selected cases with the same four
interactive cases skipped (`c000ab90-1774-49fe-be60-c2cd706b1415`), including the unchanged
activation case, both new lifecycle regressions and the preceding dual-HWND paint regression.
The component integration cohort separately passed 598/598 across 18 binaries, with no skipped
selected cases (`3e28da72-1012-4f31-8c8b-22d2e53f0275`). These runs used Windows 11, Rust 1.95.0,
one build job and the ordinary shared target. Independent correctness review found no outstanding
P1/P2 in either the text-pool correction or the fresh-factory query owner.
Locked workspace all-target and independent-consumer all-target/all-feature compilation passed,
including the latter's WGPU 30.0.1 renderer. Core/Windows all-target Clippy passed with
`-D clippy::correctness`, and root formatting passed. Existing non-denied test/docking warnings
remain. Import-boundary, public-document and release-document checks passed; no dependency or
declared toolchain floor changed. The separate native factory correction is `432d079675`.

## Cached Dependencies, Deferred Replay And Opacity

The November 5 cache unit adapts upstream `5e982c6bdc`, `decd2399b1` and `8fa085e397` through
the fork's existing EntityMap, view and frame journal owners. Per-callback read scopes replace
whole-frame clone/difference recording. Draw guards isolate nested windows and restore the outer
scope on normal exit and unwind; released generations are filtered through reference-count
membership, including entities temporarily leased outside storage. Fresh and replayed deferred
work retains its paint/prepaint reads. Subscription publication remains after candidate acceptance.

Deferred prepaint now processes round indices in the original journal rather than temporarily
moving its entries into separate vectors. The first focused run passed 18 of 20 cases and failed
the two deferred-subscription regressions (`8d40c659-99d8-45e1-a053-fc579ed70b65`): replay
reported `node 1 was not part of the reused subtree 3..4`. The imported per-round implementation
recorded ranges against a transient vector, then replayed them against the combined journal. This
is the same upstream defect corrected by `5e982c6bdc`; preserve its nested-popover regression as
well as the complete ordinary/deferred parent/child subscription matrix. The corrected focused
run passed 22/22 (`380fccfa-4379-4196-9da2-42ccec633c21`).

Before the later stationary-pointer hover extension, the full Core library passed 864/864 with
no skipped cases
(`9ee81cc1-73e0-4be7-99dd-afe887e8706b`). It includes the added
nested-menu replay/rebuild regression, exact nested-window read ownership on inner success,
unwind, failed open and real atlas rejection, and committed D to candidate E dependency change.
The refused candidate retains D without publishing E; accepted retry switches the actual tracked
dependencies and window invalidators. The suite also retains the existing geometry, atlas,
presentation, focus, reveal and portal controls. The additional upstream menu fixture initially
needed adaptation from `Entity::cached` to this fork's existing `AnyView::cached` API.

Ordinary deferred content and window portals retain logical inherited opacity. Exact accepted
Scene primitives prove parent/child multiplication, changed parent alpha and unchanged cached
render/paint counts. This is not offscreen group compositing. Cached keys compare opacity by
bits; hover opacity discovered after prepaint requests a bounded follow-up rebuild and keeps the
old key until rebuilding succeeds. Existing same-AABB rounded-clip changes still invalidate replay.

The stationary-pointer extension first reproduced stale deferred opacity in all four direct/cached
parent and ordinary-deferred/window-portal combinations
(`76707405-ad53-41a2-89fe-409f9899dee6`). The accepted parent painted at the new alpha while
deferred content retained its prepaint capture. Private opacity predictions now reconcile against
the final admitted hit test without changing input-latched layout refinements. A first correction
requested two callbacks instead of one (`f6b075bb-92e4-44f1-a8ee-f9bbea2795dc`); view notifications
now coalesce through the existing Window animation owner. A subsequent full run passed 871/873
(`b7a1c49c-915e-45c7-b948-293c94aa85c7`); its two new width-hover fixtures positioned
an absolute root incorrectly. Both now use a real root container and assert the initial position,
width and unhovered state before testing convergence.

The corrected fixtures and extended acceptance cases then reproduced three remaining failures
in a focused 10-case run (`b026d4dc-a0dd-4ac0-8482-7b675665d7ba`, seven passed). Fresh cached
parents could record their paint alpha while retaining deferred captures from an earlier prepaint
alpha; a refused Inert candidate could also reset an accepted opacity prediction. Fresh cached
views now record geometry-valid rebuild requests in the existing candidate journal, and an
admitted prediction change requests their rebuild together with its declaring view. Suppression
resets commit with the candidate rather than modifying the prediction before acceptance.

The resulting full Core library passed 873/873, with no skipped cases
(`80f4ca3c-e6eb-4c18-86ef-d55fccb392eb`). The regressions exercise same-frame ancestor expansion
with an explicit cached-parent notification, refused Inert candidates and retry without MouseMove,
ordinary and group hover, unchanged reuse, width-only hover and mixed width/opacity hover. They
assert actual admitted primitives, bounded follow-up callbacks and preserved layout latches.
An expired group hitbox outside the cached view is treated as unknown; changes to that external
group context still require notifying the owning view.

Independent review found no remaining P1/P2 in these corrections. Prediction-driven follow-ups
may rebuild other valid mismatched deferred views in the same candidate; this is not a precise
ancestor dependency graph. Arbitrary custom paint-only opacity scopes do not gain a general
prediction guarantee. The existing normal-commit boundary remains after atlas admission and
before frame swap; scoped EntityMap unwind restoration does not establish atomic recovery from
every possible draw or commit callback panic.

The scoped and full framework commands are:

```sh
cargo nextest run --locked -p open-gpui --lib -E 'test(app::entity_map::access_tests::) or test(view::cache_tests::) or test(elements::deferred::tests::) or test(window::cache_dependency_tests::)' --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked -p open-gpui --lib --test-threads=1 --no-fail-fast --no-tests fail
```

Before that extension, the affected UI core, components, docking and Canvas library/integration
cohort passed
2,964/2,964 across 25 binaries, with no skipped cases
(`db313e6d-1398-4d78-9416-38b7a51bbec6`). Run it serially:

```sh
cargo nextest run --locked -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-docking -p open-gpui-canvas --lib --tests --test-threads=1 --no-fail-fast --no-tests fail
```

After the complete hover and rejected-candidate corrections, that same cohort passed
2,964/2,964 across 25 binaries, with no skipped cases
(`89c1a547-d813-4042-8467-bae6561ebfdd`). This is the affected consumer result for the final
runtime implementation, separate from the earlier pre-extension run.

The new native regression uses a real Windows App/HWND, cached canvas and the complete
ordinary/deferred parent/child matrix. Its test host starts the existing production VSync driver
and pumps native messages; it does not enter the full `Application::run` lifecycle. Initialization
waits for the exact registration's real WM_PAINT and completed initial presentation before taking
the cache baseline. Each explicit root notification and subsequent model-only notification must
reach a newer accepted, nonempty renderer-Submitted generation, while child render/paint counts
stay unchanged. Explicit child notification then rebuilds and paints the changed value. Teardown
checks the original HWND registration, ordered retirement and platform/OLE release. These receipts
are not a screen-pixel readback or proof of exactly two total native redraws.

The first native fixture run failed its first replay count check, observing `(2, 2, 1)` instead of
`(1, 1, 1)` (`c1cabd00-7e18-4223-be54-32117b960304`). After adding the explicit initialization
boundary, it timed out with no dispatched messages (`3049ec99-df57-4344-bfef-3229386b3632`).
`update_for_test` does not call `Platform::run`, where the normal VSync driver starts, so the
fixture now explicitly starts that production driver once. The first count failure alone does
not establish which native initialization notification caused the extra rebuild. No assertion
was removed, and no notification-stage forced refresh substitutes for the subscription proof.

Earlier Windows cohorts retain their failures. Run `808cec66-8f4d-45f3-9770-8aa117f403f1`
passed 164/165 selected cases, with four existing interactive cases skipped. The new cache
regression passed; the existing asymmetric-activation test timed out. Diagnosis observed a
terminal Rejected ticket while the owner was only thread-active and an external HWND held
foreground. The fixture now prepares actual foreground before each scenario and asserts exact
Activated/Rejected tickets as well as the native foreground result. It does not force the child
foreground after the product action.

A subsequent Windows run passed 163/165, with the same four skipped cases
(`14ee36f5-63d6-48ff-8486-e8d9be789d42`). Cache and asymmetric activation passed, but the activation
terminal test observed an external foreground during creation and the rejection-restoration test
timed out. An unchanged focused run passed restoration and failed the terminal test's whole-span
keyboard-message assertion (`9de22e75-369d-46c5-ac61-ace782b20456`). Those results do not establish
who generated the keyboard messages or which later focus transition caused the earlier timeout.
The test-only native observation now retains raw wParam/lParam and reports exact filtered message
identity, ordinal and nearest test phase on failure. Creation also checks the exact target for
positive WM_ACTIVATE/WM_SETFOCUS while preserving the global sentinel assertions. Restoration
rechecks its exact foreground/active/focus prerequisite immediately before the rejected request.
The original full-span keyboard predicate, activation tickets, terminal-once and exact restoration
assertions remain; no production activation logic changed.

With those fixture corrections and the final runtime implementation, the focused asymmetric,
cache, activation-terminal and rejection-restoration cohort passed 4/4
(`9d013ab1-d8be-42bd-826b-c37ef8b1786e`). The subsequent complete Windows library passed 165/165
selected cases, with the same four existing interactive cases skipped
(`d34ab2ff-52cf-42a9-a2e4-8a3c855df787`). Both commands enabled all features and ran one test at
a time on Windows 11 with Rust 1.95.0 and one build job. A successful final run does not establish
the source of the earlier keyboard messages or remove the need for a stable native desktop when
qualifying foreground behavior.

```sh
cargo nextest run --locked -p open-gpui-windows --all-features --lib --test-threads=1 --no-fail-fast --no-tests fail
```

Locked workspace all-target compilation and the independently locked consumer's all-target/
all-feature check passed, including its actual WGPU renderer. Core/Windows all-target Clippy
passed with `-D clippy::correctness`; existing non-denied test and docking warnings remain.
Root/consumer formatting, import-boundary, public-document links, release documents and public
API tier checks passed. No dependency or declared Rust 1.92 floor changed; execution used Rust
1.95.0. Linux/macOS/Web target execution and the complete refresh's broad gate remain separate.

Non-view notification remains the upstream documented limitation: it redraws subscribed windows
but does not dirty an otherwise clean cached view. Notify that view or explicitly refresh when
its content must rebuild. These ownership/cache regressions establish no frame-time, allocation,
font-generation, OS composition, engine View or complete upstream-refresh qualification.

## Native Text Layout And Wrapped Hit Testing

The successor to `7a1bf6d73d4` ports `0294cfb58e`, `979cd3fdf0`, `ac6818fb2d` and
`e71963a599` against the fixed Zed target `635026fabc38440fce5e1767d125026a17b12012`.
DirectWrite keeps its native tab policy and the original UTF-8 source. Zero-glyph tab drawing
runs query native hit-test advances and retain invisible glyphs for caret/index mapping.
U+2028/U+2029 native segments are rebased onto one logical baseline, retaining their source byte
indices, font runs and aggregate vertical metrics. Empty feature configuration now applies
liga/clig/calt defaults, while explicit per-tag disabling remains authoritative.

Wrapped exact hit testing returns the current row's end in the floating-point gap where its
unwrapped lookup returns `None`. The constructed shaper regression uses a trailing zero-width
glyph two float steps beyond the unwrapped width; it reproduces the consuming boundary, not a
specific native font's shaping process. Exact/closest distinctions, offset rows, UTF-8 byte
indices and caret affinity retain their existing contracts.

The first Core run passed two controls and reproduced two `unwrap()` panics
(`abd96d72-a6cc-44db-9060-7feb3c1e4ac1`). After the owning fix, the four focused cases passed
(`14e48768-4c45-4ba5-a3ca-623034408286`). The Windows fixture first required API adaptation:
this fork constructs DirectWrite from its existing `DirectXDevices`, and its `ShapedLine::split_at`
preserves the upstream fixture's piece/text/width assertions without adding the later cursor
optimization. The corrected red baseline passed the explicit-disable control and failed default
typography, tabs and both separator cases (`fbc5ba64-c7fc-497b-baa8-ef710f049cf2`). Default
typography contained zero configured features instead of three; a standalone tab had no indexed
caret glyph. No failing behavioral assertion was removed.

The initial corrected Windows cohort passed 6/6
(`8266f330-6bb1-4d68-87b5-ab758f613ace`), including nullable native-array controls. The callback
array helper and mutable context pointer adapt relevant prerequisites from `89e8a4b9ec`; the
pointer originates from a mutable binding and stays within synchronous native Draw. The remaining
color staging-map and atlas upload changes are separately queued. In particular, skipping an
upload while publishing a tile would violate the fork's admission/lease contract.

Independent review found no P1/P2 in the Core or DirectWrite changes. It identified a coverage
limit: equal-font TextRun fragments can coalesce before native shaping. A further distinct-font
tab fixture therefore compares actual font identities, native hit positions and complete native
width. Live typography checks establish configured feature tables, without claiming a particular
font's final ligature glyph or screen pixels. Non-null native pointers still rely on the native
length/alignment/lifetime contract; helper admission is not a replacement for that contract.

The final Core library passed 877/877, with no skipped cases
(`f2907a0c-0a38-46c6-a059-64df8cbab295`). The affected UI core/components/docking/Canvas
library and integration cohort passed 2,964/2,964 across 25 binaries, with no skipped cases
(`9d9eea36-f49a-4cee-9acd-d43109cb6417`). The final focused native cohort passed 8/8, including
the distinct-font oracle and the new submitted-frame trajectory
(`cd1bb896-419c-438b-99a0-ef742161d154`). The full Windows library then passed 173/173, with the
same four existing interactive cases skipped (`4cd55b68-f23d-4433-8c33-46580e03c7e2`). All ran
on Windows 11 with Rust 1.95.0, one build job and serial tests in the shared target.

The new native fixture uses public Render/canvas, WindowTextSystem and ShapedLine paint through
the platform's real shared DirectWrite/DirectX path. It retains the full source and actual paint
result for text containing tabs, supplementary characters and both Unicode separators. The paint
record is tied to the newer accepted/nonempty renderer-Submitted generation, current rendered
generation and submitted diagnostics. A changed source followed by `cx.notify()` reaches a newer
submitted generation without a forced-refresh substitute. VSync also requests frames; its request
counter is activity evidence rather than independent proof of notification causality. Teardown
checks the original HWND registration, one retirement ticket's ordered phases, platform window,
OLE and text-system release.

These receipts are not glyph/screen pixel readback, native keyboard/IME, full bidi/complex shaping,
separator/tab policy generalization or a performance comparison. Locked workspace all-target and
independently locked consumer all-target/all-feature checks passed, including the actual WGPU
renderer. Core/Windows all-target Clippy passed with `-D clippy::correctness`; existing non-denied
test/docking warnings remain. Root/consumer formatting, import boundary, public-document links,
release documents and public API tier checks passed. The declared Rust 1.92 floor and dependency
graph remain unchanged; this unit executes Rust 1.95.0 rather than qualifying that compiler floor.

```sh
cargo nextest run --locked -p open-gpui --lib --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked -p open-gpui-windows --all-features --lib -E 'test(direct_write::layout_tests::) or test(native_text_layout_tests::)' --test-threads=1 --no-fail-fast --no-tests fail
```

## Native Atlas Upload And Color Readback

The successor to `0861f8d52b1` reconciles the remaining native resource changes from
`89e8a4b9ec7e0fa8a49987a3c1dbf29343dc6999` against fixed target
`635026fabc38440fce5e1767d125026a17b12012`. The earlier text-layout unit adopted mutable
DirectWrite callback provenance and nullable arrays; this unit owns source/destination upload
admission and mapped color-glyph staging release.

Both DirectXAtlas insertion APIs now share one private path. Checked source dimensions, one- or
four-byte pixel stride, row pitch and complete byte length are admitted before allocation. The
destination size and checked box must fit the actual resident texture before UpdateSubresource.
Malformed input returns an error without publishing a key or consuming an allocation/generation.
A later destination/format refusal cancels only its unpublished allocation; existing published
regions and exact-instance visual leases retain their previous lifetime rules. Texture and shader
view creation precede free-slot/generation reservation. Successful parameter admission and an
issued upload call establish neither GPU completion nor presentation.

The existing color-raster owner now validates BGRA dimensions and checked byte/row spans,
allocates output before Map, and pairs a successful Map with RAII Unmap on return, error and
Rust unwind. Copying respects native RowPitch; the existing straight-alpha conversion follows
the released map. Review also found that the new blended glyph target had no defined initial
contents. It is now cleared to transparent before drawing. The actual Segoe UI Emoji path
checks visible pixels, repeated identical BGRA bytes and every pixel of an expanded transparent
border. This is an output control; without poisoning the native target it does not promise a
deterministic missing-clear red baseline.

The baseline's two insertion paths passed unchecked source pointers to the native driver. The
fixed upstream target adds a short-buffer early return inside a void upload function but its
caller still publishes a successful tile. That skip policy was not copied. Source inspection
establishes the original missing check; executing an unsafe short-source upload is not required
for a red baseline. Pure tests exercise exact/short/extra bytes, invalid and maximum dimensions
without giant allocations, and final destination bounds.

For the separate mapped-readback defect, the existing row-copy body was extracted without
fixing its missing Unmap. The safe WARP/debug-layer baseline passed three atlas admission cases
and failed repeated readback on the first iteration
(`db4d3ca5-c7c6-46c4-991b-389841fbd5af`): remapping returned a stored native debug error while
the test required an empty queue. The source data was complete, and output pixels matched before
the remap check. Both WARP and the debug layer are required by this regression; initialization
failure fails the test instead of being treated as a successful skip.

The focused resource cohort passes 13/13 after transparent initialization
(`6b183591-44d9-4f3c-bad6-8c5e929ade80`). It includes three pure admission cases, eight real
WARP atlas cases and two color-readback cases. Both insertion APIs are exercised for all three
texture kinds, including short-source refusal, valid same-key retry, cache hits, atomic texture
leases, rollback of an unpublished allocation and old leased-region preservation after key
removal. Multi-row readback compares exact source bytes at nonzero atlas origins. The repeated
color-map test also exercises post-map error and unwind cleanup. The padded consumer fixture
first required the fork's supported DevicePixels addition API; no pixel assertion was removed.

Core's affected retained-resource/cache/deferred cohort passes 54/54
(`ff9c8e65-8bef-4dee-8192-a6c8d2ffd3d3`). A first complete Windows run passed 186/186 with four
existing interactive cases skipped (`7fe95317-17df-43d6-a6ae-9cc55b3ac103`); it preceded the
transparent initialization correction. The final complete Windows library passes 186/186
with the same four skipped cases after that correction
(`8ead5c2b-1f4f-48bd-b1eb-a4639ccb5fbb`). It retains the existing real HWND text/cache paint,
Submitted-frame and exact retirement trajectories. All ran on Windows 11/Rust 1.95.0 with one
build job, serial tests and the shared target.

Locked workspace all-target and independently locked consumer all-target/all-feature
compilation pass, including the actual WGPU renderer. The previous text-layout unit's 2,964
affected UI/component/docking/Canvas tests were not rerun for this Windows-only resource change;
the affected Core 54 and complete Windows 186 own this unit's executed runtime coverage.
Core/Windows all-target/all-feature Clippy passes with `-D clippy::correctness`; existing
non-denied test and docking warnings remain. Independent review finds no remaining P1/P2.
Root/consumer formatting, import-boundary, document-link, release-document and public-API checks
pass. The declared Rust 1.92 floor and dependency graph remain unchanged; this unit executes
Rust 1.95.0 rather than qualifying that compiler floor.

WARP/debug-layer initialization failures fail these tests. Unmap cleanup covers ordinary errors
and Rust unwind, not process abort or native exception termination. Cancelling an unpublished
bucket allocation preserves existing keys, leases and pixels but does not guarantee immediate
recovery of every shared bucket's reserved space. UpdateSubresource provides no HRESULT; an
Inserted result means parameter admission and an issued update, not asynchronous GPU success.
These results are atlas/color-glyph resource readback, not screen compositing, complete native
input, frame-time measurements, non-Windows backend execution or Nara renderer integration.

## Pending Input, Modifier Recognition And Blur

The successor to `f1850835fbb` adapts `bc7041c600`, `ba42ab9d98` and `f66ed399cd`, including
the direct IME, multi-modifier and dropped-focus prerequisites `dc1e815e47`, `a8cae3bd77` and
`e463c5ab74`, against fixed target `635026fabc38440fce5e1767d125026a17b12012`.
The complete upstream scenarios and fork controls are registered in
`window/pending_input_tests.rs`; this unit does not adopt target-only pending-status/pause APIs.

The first registered baseline compiled and ran 20 cases: eight passed and twelve failed
(`706f4d38-f906-4597-b2bb-0b4a2a0fef16`). Behavioral failures include absent standalone-modifier
interception/observation, missing modifier timeout, false standalone synthesis after multiple
modifiers, IME preference during a pending chord, and pending input retained after accepted blur.
Refused focus candidates and suppressed/FocusStable mutations already passed their controls.
One failure was a fixture prerequisite: closing the last window shuts down the test App, so
opening its replacement requires a separate live keeper window. The fixture now retains that
window without weakening exact retired-handle or no-replay assertions.

Recognized keystrokes, including standalone modifier releases, now reach interceptors and
observers independently of raw event type. Multi-modifier gestures disarm standalone synthesis;
a later genuine modifier tap re-arms it. Modifier-only chord prefixes use the existing timeout,
and an intercepted modifier leaves the prior chord and its original deadline intact.

The existing focus owner now settles pending input on accepted exact/empty transitions and
final focused-handle release, with one deferred notification only when state existed. Candidate
clears stay staged until admission, including a sealed callback's promoted successor before
prepaint starts. Refusal restores the original sequence and timer. Suppressed/FocusStable
mutations remain forbidden; repeated empty blur can clear a global-context chord while further
focus claims are disabled. The platform handler suppresses its IME-first preference only for
the current focus's pending sequence, using the existing native callback lease.

The first corrected run passed 19/20 (`ddb376ba-3c64-4c45-862f-44e52129ff08`). The remaining
fixture removed a temporary control without notifying its retained view, so the old rendered
element still held the focus handle. Removing it with the owning view notification retains the
last-release/notification assertion. The next focused run passes 21/21
(`bece25ab-741d-4870-b7d9-96ca2f56d55f`), including a second accepting text handler whose IME
preference recovers after focus transfer, no old-prefix replay into either handler, and ordinary
text delivery only to the new target. A second pending observer cannot execute after the first
retires the window.

Upstream regression behavior is adapted to the fork's existing candidate/focus/native-handler
owners. The final target's TextInputStateChange event log has no fork counterpart; this unit
tests actual handler admission, lease queries and cleanup rather than adding another public
notification API. The fixture has no active marked-text composition and does not qualify the
macOS Japanese IME's composition-first OS route. Final broad and native-consumer evidence
is recorded below.

The first complete Core run passes 898/898, but subsequent review identified a missing callback
case. Extending the two-handler trajectory to move focus inside the timeout observer reproduces
old-prefix text delivery after that callback (`c1069260-eda9-4188-8d92-765a4d7e91b4`). Ordinary
pre-timeout focus transfer does not cover this. Revalidate the original focus/context after
observer and replay-listener callbacks before admitting further actions or text. Review also
requires same-focus context changes to restore IME preference and preserves the upstream
singleton-modifier timeout condition: a modifier in the middle of an otherwise untimed chord
must not introduce a timeout.

Both additional controls fail before their fixes
(`3c5cd0ec-9273-4aa6-89a0-651eb5e5ff0b`): same-focus context replacement still reports active
pending input, and a modifier inside a longer chord unexpectedly owns a timer. The corrected
active-pending/replay checks share the existing dispatch-context projection through a private
borrowed iterator, without cloning each context merely to compare it. Replay retains the
original focus/context and stops after a user callback changes either. The modifier condition
matches the complete singleton pending slice; existing text/short-binding timeouts remain.

A further review control catches an overly early replay guard: a timeout short action moves
focus successfully but its recognized-keystroke report is lost
(`53b71765-5300-456f-8b66-a4ff63674255`). Replay now preserves that completed-action report
while the window remains live, then refuses further old-recipient input. A closed window still
stops before observer delivery. The first native consumer passes
(`ba55347f-9008-45bc-984a-a333b0f8c094`); it predates this last Core ordering correction.

The final Core library passes 901/901 with no skipped cases
(`488f7d36-c53f-4f88-ba0d-13429dfa2913`), including all 24 pending-input scenarios after the
completed-action correction. Independent source/fixture review finds no remaining P1/P2 in this
unit. The native consumer composes two real text handlers, production PlatformInput callbacks,
leased IME-preference queries and targeted HWND WM_CHAR delivery. It checks accepted/nonempty
renderer-Submitted generations, correct text recipient, old-handler release, deferred blur
settlement, exact HWND/native retirement and platform/OLE/text-system teardown. Synthetic
PlatformInput does not establish OS key translation, and WM_CHAR does not establish live IME
composition. The timeout uses Core's fake clock rather than a native wall-clock wait.

The affected UI core/components/docking/Canvas library and integration cohort passes
2,964/2,964 across 25 binaries, with no skipped cases
(`0653fd29-aded-4598-af49-043fc8674d4f`). The first complete Windows run passed 186 cases and
failed the existing native cache matrix (`ac05f2f7-40f2-4200-9734-0c11c4f779fa`), with four
interactive cases skipped. Its first ordinary-parent/child replay observed counters `(3,3,1)`
instead of `(2,2,1)`. The new native input trajectory passed. An unchanged focused cache rerun
passes (`d5ea70fa-dacc-4806-b69d-72081427f547`); no counter assertion was relaxed and no cache
source was changed. The initial extra render/paint's trigger is not established by that rerun.
A second unchanged complete run reproduces the same first-case counter mismatch
(`bebc452e-1473-4011-a485-782d3f5119ad`), again passing the other 186 cases. That result was not
accepted as a resolved intermittent failure. A diagnostic run with expensive refresh backtraces
changed timing and failed the separate native mouse-leave control
(`a5b1a3db-69b3-4a52-89d3-9d6b1fd240bc`); it is not qualification evidence. The lower-overhead
cache-key/caller trace then reproduced the cache mismatch on replay 2
(`fd30a26c-d614-4765-8c76-0f83b285978d`). Every key and resource check matched, dirty was false,
and refreshing was true after native_hover_status_changed called Window::refresh. This is a
legitimate whole-window invalidation entering the fixture's supposedly unchanged replay.

The cache fixture now requests pointer-transparent, nonactivating creation and checks those
three facts through native HWND readback. After initial show/follow-up it performs one explicit
initialization refresh and requires a strictly newer accepted/nonempty Submitted generation,
matching rendered generation and submitted diagnostics, before freezing its single baseline.
All original replay, non-view model notification, explicit child invalidation and exact teardown
assertions remain. No miss is retried or rebased. Temporary probes were removed; the production
AnyView cache owner and Window refresh body retain their original code.
The separate fixture correction is `fb196f8a9c876c2981c18d58b54c59966dce4084`.

The final complete Windows library passes 187/187 with the same four existing interactive cases
skipped (`3234e5f8-32bb-40bc-9984-e944f3e21300`), including the corrected native cache matrix
and the new input consumer after the final Core ordering correction. These results run on
Windows 11/Rust 1.95.0 with one build job, serial tests and the shared target. They establish no
screen-pixel input latency, IME composition, touchpad timing, non-Windows execution, engine
rendering integration or complete upstream-refresh qualification. The declared Rust 1.92 floor
and dependency graph remain unchanged; that compiler floor is not executed by this unit.

Locked workspace all-target and independently locked consumer all-target/all-feature checks
pass, including the actual WGPU renderer. Core/Windows all-target/all-feature Clippy passes with
`-D clippy::correctness`; existing non-denied test/docking warnings remain. Root/consumer
formatting, import-boundary, document-link, release-document and public-API tier scans pass. Independent reviews of the
input owner, upstream scenario mapping, native consumer and corrected cache fixture find no
remaining P1/P2 within this unit.

```sh
cargo nextest run --locked -p open-gpui --lib -E 'test(window::pending_input_tests::)' --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked -p open-gpui-windows --all-features --lib -E 'test(native_pending_input_tests::)' --test-threads=1 --no-fail-fast --no-tests fail
```

## GPU Primitive Flags And Windows Color Transfer

The successor to `cde23d44ada` reconciles `f28177003499ec9c5052d1883fee62b148451c59` and
`b9419ae72814157164b09592ab2466f9f4709f09` against fixed target
`635026fabc38440fce5e1767d125026a17b12012`. The fork retains its existing clip envelopes,
40-byte atlas tiles with exact texture generations, and terminal primitive transforms rather
than copying the target's different record order.

The first safe layout regression fails on the existing sprite field's width: one byte instead
of four (`eccc7c75-1851-446f-8fdb-b0dba28206a7`). It does not read the old uninitialized
padding or issue an unsafe old-record GPU upload. Aggregate record sizes alone would miss this
defect: both old and corrected PolychromeSprite records are 128 bytes. The shared PaddedBool32
now provides private transparent u32 storage with only bool construction and false default.
Underline.wavy and PolychromeSprite.grayscale use it, while author-facing style/paint inputs
remain bool. Complete zero/one bytes, field width, offsets and existing record sizes are checked.

Windows previously widened the bool through its own DirectXPolychromeSprite. The corrected
shared record removes that duplicate layout and per-frame vector conversion; DirectX uploads
the shared slice. WGSL and HLSL consume the full unsigned flag without masking low bytes.
This removes source-level work but establishes no measured performance gain. The existing
native/retained/lease/clip contracts still govern record admission and presentation.

The corrected Core/WGPU ABI cohort passes 7/7
(`1f1e31c5-da4a-4b56-832f-4bd80decef76`). The native old-shader regression then fails on an
actual Oklab black/white interior pixel: RGBA `[1,1,1,255]` instead of expected RGB `[99,99,99]`,
with per-channel margin 6 (`297eb7a2-5bbc-4891-9339-eddf048c4660`). This is scene-shader pixel
readback through a real HWND/DirectX consumer, not a transfer-function round trip or endpoint
comparison. The fixture closes its exact HWND and releases platform/OLE/text-system owners
before evaluating the pixel oracle, preserving teardown evidence on a behavioral failure.

Windows now decodes sRGB with exponent 2.2 and encodes linear RGB with its reciprocal through
the existing HLSL owner. This retains the backend's approximate gamma contract; it does not
select a complete engine color-management policy or change WGPU/Metal transfer functions.
The unchanged native gradient oracle passes after only those exponent changes
(`8b036cd1-da76-4e4a-b2b1-ac3758699191`). It checks six Oklab/sRGB/alpha gradients at five
interior physical pixel centers each, four solid/alpha controls and a notified color witness in
a newer exact captured scene generation. Complete Core/WGPU library execution passes 941/941
with no skipped cases (`99917b88-750f-481f-b70c-c350f73d726d`): Core 902 and WGPU 39, including
real adapter/device atlas controls and production WGSL pipeline creation. The latter compiles
the actual shader but is not WGPU scene-pixel readback.
All 18 production HLSL entry points also compile with installed Windows SDK 10.0.26100.0 x64
FXC `/O3`, using the same modules, profiles and output form as build.rs. That standalone check
establishes shader compilation only. The same native trajectory also executes the production
non-debug embedded artifacts and passes 1/1 (`7e8edfa3-bab2-4c24-b68f-d0cb21a74760`). This run
uses the release profile with Rust opt-level 0, debug info 0 and debug assertions false; build.rs
still invokes FXC `/O3`, and RawShaderBytes selects its embedded byte arrays. The fixture reports
`embedded FXC true`. This qualifies those shader bytes through actual DirectX scene pixels and
teardown, not optimized Rust performance or complete release acceptance.

The expanded scene uses nine image instances with exact ImageId/frame-index diagnostics:
alternating false/true/false/true grayscale at full and half opacity, plus a translated rounded
clip. Independent source-color/weighted-grayscale samples retain opacity controls. Straight
underlines, wavy underlines and strikethroughs are checked at both opacities, including wave
centroid, ink fraction and peak color. Rounded corners, translated-away source and exterior
pixels remain black. After an unchanged root notify, the entire RGBA image equals the initial
capture; after changing only the root color, all consumer pixels still pass. Cached child render
and all three canvas paint counts remain unchanged across both newer submitted frames.

Actual cache mode is captured during root prepaint and published by a stable
PrepaintPublicationId through the existing accepted-frame transaction. Capture requires its
generation to equal the exact Submitted generation. This avoids confusing frame-end AccessKit
activation with the mode used to draw that frame. When accessibility or inspector picking
requires bypass, the test instead requires the child and all three canvases to rebuild; it does
not disable that production policy or drop pixel assertions. The executed captures report
scale 1.5 and cached replay true, with counts `(3,3,3,3)` unchanged on both notifications.
They do not qualify a native accessibility-activation matrix, mixed-DPI migration or other
monitor scales. Wave-height sampling retains native f32 scale precision at half-pixel ties.

Affected UI core/components/docking/Canvas execution passes 2,964/2,964 with no skipped cases
(`0a9c4ce7-9bfe-457e-834a-b3895c63dbe1`). The complete Windows library passes 188/188 with four
existing interactive cases skipped (`b3ad10d3-03d5-4baf-b2e9-3e6437228a41`), preserving atlas
admission/lease/refusal, native text/cache/input, exact submitted-frame and retirement controls.
The final fixture-only cache-mode refinement passes its focused debug rerun 1/1
(`05918ee8-3e14-41df-a1a8-359d34764fcd`), reporting `embedded FXC false`. No production semantics
changed after the complete Core/WGPU and affected-consumer cohorts.

Locked workspace all-target and independently locked consumer all-target/all-feature checks
pass, including the consumer's actual WGPU renderer. Core/Windows/WGPU all-target/all-feature
Clippy passes with `-D clippy::correctness`; existing non-denied test/docking warnings remain.
Root and independent-consumer formatting, import boundary, public API tiers, public document
links and release-document checks pass. The release inventory's field names were aligned with
the changelog after its initial symbol-coverage refusal. Independent source/fixture review
finds no outstanding P1/P2 after the draw-time accessibility-mode correction. The declared
Rust 1.92 floor and dependency graph remain unchanged; this unit executes Rust 1.95.0 rather
than qualifying that compiler floor.

The macOS source includes flag-width/field-type/offset tests and a Metal PaddedBool32 size
assertion. Its existing cbindgen owner still generates the scene header from the shared source.
Generated-header correctness, Metal compilation and native pixels were not executed on Windows,
including the runtime-shaders feature path. Linux and browser execution, OS compositor output,
live keyboard/IME, frame-time/retained-byte measurements and Nara rendering integration remain
outside this unit. Window capture rerenders the current scene to an offscreen target; matching
submitted-generation metadata is not an OS screenshot. WGPU's successful production pipeline
creation is still distinct from WGPU scene-pixel qualification.

Focused reproduction commands, using one build job and the caller's shared target:

```powershell
cargo +1.95.0 nextest run --locked -p open-gpui-windows --all-features --lib -E 'test(native_scene_color_tests::)' --test-threads=1 --no-fail-fast --no-tests fail --success-output immediate
cargo +1.95.0 nextest run --locked --release --config 'profile.release.opt-level=0' --config 'profile.release.debug=0' --config 'profile.release.debug-assertions=false' -p open-gpui-windows --features test-support --lib -E 'test(native_scene_color_tests::)' --test-threads=1 --no-fail-fast --no-tests fail --success-output immediate
```

## Runtime Font Installation And Missing-Glyph Reporting

The successor to `871c012c96c` reconciles `9e6e14160c192f682c4d35f4e8d420cc0577ad5f`,
`3389d87577312c126faa3bcbf79951babd6161d7`, `b7d50b95e8db53607438ef6f5c29d55b7b38f852`
and `b68add5b37a125b3f9d6d04f23a2f93f8eaa57bb` against fixed Zed
`635026fabc38440fce5e1767d125026a17b12012`. It explicitly includes the adjacent retained-font-
source change `d0f797a38c751cff745108e458bbc5ec73c70738` and fallback-only face correction
`d6358b2d3cd04de4d8f8f69610cccd2e390b6a3e`; they are not prerequisites inferred from dynamic
installation. Font prewarming, available-name policy, glyph-raster reuse, Unicode wrapping,
layered text, window-local Rc layout state and Taffy/API changes remain separately queued.

The initial four-case Core baseline passes the failed-installation control and fails three
regressions (`9e2b13f0-57d1-4cfd-8ecd-304493208196`): a successfully installed descriptor still
resolves to its old fallback, a repeated layout retains the old Arc, and a resolved fallback
is not retained as the requested descriptor's result. The new cache stores Direct/Fallback/
Missing results, preserves direct-only reverse lookup and warns only for explicitly requested
missing descriptors. Successful installation invalidates descriptor selection and advances
the font generation. Existing loaded FontIds remain usable; a reselected descriptor's numeric
identity is backend-specific. A backend error is not a promise of atomic rollback of an entire
partially processed font batch.

Window layout caches now keep both frames and their font generation under one private mutex.
Range/index/reuse/truncate/finish/abort operations observe that same protected state. Window
entry points capture generation before resolving FontRuns; stale resolved runs can finish
without populating a newer cache. Wrapped results revalidate their generation when published
after out-of-lock computation. Controlled threaded tests cover lock-delayed range operations
and an old wrapped result published after a new-generation layout. The first test establishes
ordering around the held cache lock, not a scheduler-specific instant at which a worker blocks.
Character-width pools retain independent resolution and width-cache generations. Pooled and
already borrowed wrappers re-resolve their requested Font and discard stale widths; returning
a wrapper under its changed FontId does not rely on the old pool entry remaining present.

Same-generation rejection returns transferred layouts to the previous cache. The public
Window/HeadlessAppContext trajectory exercises actual atlas-lease refusal for ordinary,
wrapped and hash layouts, checking unchanged committed revision, current-candidate Arc reuse
on retry and no preinstallation Arc revival. Hash retry discovers restored map entries rather
than relying on the committed frame's use-index vector. This is candidate-admission evidence,
not a GPU device-fault injection.

MissingGlyph retains whole graphemes and source proportional/monospace identity. Shaping only
enqueues through the optional sink; a bounded 1,024-item queue and bounded receiver deduplication
retain overflow retry and yield on duplicate-only work. Font installation or subscription
reset invalidates queued generations. One App-owned receiver task invokes the current
replaceable callback after shaping locks are released. Delivery waits for App borrow release
without spinning and then rechecks quitting and batch generation. Tests cover replacement,
old/current subscription drop, ready batches invalidated while waiting, callback/destructor
reentry and exact App/callback/task release. Slot unwind tests prove restoration before panic
propagation; they do not prove that a receiver task restarts after its callback panics.

Cosmic clears family resolution after installation, preserving loaded database-face/configuration
identity and refreshing fallback chains. Actual IBM Plex/Lilex tests cover fallback-to-direct,
unchanged face Arc/glyph/metrics/advance/bounds, configuration partition, font-source pointer
ownership and shaping through the App's real TextSystem. Missing-grapheme tests cover combining
and ZWJ clusters, source classes, unordered/repeated indices and observer delivery. Replacing
or disabling the backend sink drops its previous Arc outside the text-state lock; a destructor
regression verifies reentrant font queries remain available. Retained font bytes avoid the
borrowed-source copy, without establishing a measured memory or timing benefit.

The [tracked font fixtures](../tests/fixtures/fonts/README.md) preserve unmodified font bytes,
licenses and fixed Zed/egui provenance. An IBM-only control cannot shape the emoji in `A😀B`.
Staged whitespace checking initially flags original trailing spaces in the Lilex/IBM licenses;
a fixture-local Git attribute retains the exact upstream license blobs without changing them.
After adding Noto Emoji, explicitly selecting it as primary still fails because it has no `m`;
its database face remains available for actual IBM-run fallback. The regression checks exact
source byte indices, the selected Noto face/glyph, absence of missing reports and visible,
repeatable software glyph raster at scales 1.0 and 1.5. This is not native GPU or screen evidence.

The first expanded Core/WGPU cohort runs 37 cases: 35 pass and two Window fixtures abort on a
shutdown-generation assertion (`26ec48d4-80bb-4e1b-a982-f4fd328a9e23`). The fixtures had closed
the last window and requested shutdown again before HeadlessAppContext's own shutdown. They
now select Explicit quit mode and let the existing headless owner shut down once. A later Core
correction cohort passes 36/36 (`ae969980-1286-4cc4-95e4-0b8238833625`), including App borrow
waiting, cache epochs and width invalidation. The final focused cohort passes 51/51
(`84a76c0b-c804-4a64-a5fa-23394c93146d`): 37 Core and 14 Cosmic cases, including the final
wrapper-construction boundary, all three actual Window candidate caches, sink destruction and
Noto fallback/raster controls. Complete all-feature Core/WGPU libraries pass 997/997 with no
skipped cases (`25793f50-14c1-4adf-9e86-96416a46e824`): Core 940 and WGPU 57. These retain
actual production WGSL pipeline creation and GPU atlas lifetime controls; the new Cosmic raster
tests themselves execute software glyph rasterization.

The public-context follow-up then removes an independently constructed TextSystem from both
HeadlessAppContext and TestAppContext. Their `text_system()` accessors now share the actual
App/Window owner, so installation through the public headless accessor invalidates the
window's descriptors and layouts. An identity assertion plus an actual fallback/layout change
covers that public workflow. Both focused accessor cases pass 2/2
(`f63981f8-f14d-4ae0-b85f-1954e07cb557`). The complete all-feature Core/WGPU libraries then
pass 999/999 with no skipped cases (`3faede22-f765-4b53-8ab9-2add1709c971`): Core 942 and
WGPU 57. The preceding 997-case result keeps its pre-follow-up scope.
Affected UI core/components/docking/Canvas passes 2,964/2,964 across 25 binaries with no skips
(`6427bf6d-9d9e-4ff3-802f-b04dc1e6bfaa`). The same cohort's earlier 2,964/2,964 result
(`6f5df537-9da8-4d36-b92c-d91b4f140f41`) predates the public-context correction; it is retained
as an intermediate checkpoint rather than substituted for this final source result.

The native installation trajectory passes 1/1 (`35a852ee-bbeb-4ad4-9ad0-b0ed6ad8c902`). It
requires Lilex to be absent from the initial OS collection and fails explicitly otherwise.
Font bytes enter only the application's private DirectWrite collection, not the OS font store.
A public Render child shapes complete source with tabs, Unicode separators and multi-byte
characters; after installation, explicit child notify produces a newer exact accepted/nonempty
renderer-Submitted generation with the direct face. It paints the old retained layout again,
preserves old ASCII glyph advance/bounds and completes exact HWND/platform/OLE/text-system
retirement. The FontId change also changes the cache key: same-ID invalidation is established
separately by Core/Cosmic controls. Paint success and submission are not glyph-pixel appearance
or proof that every CJK/Deseret character has a nonzero glyph.
The complete Windows library passes 189/189 with four existing interactive cases skipped
(`db1b15f5-01fc-449a-b058-078ab6bce539`), retaining the atlas/refusal, cached/deferred scene,
native text/input, submitted-generation and exact retirement cohorts. This run includes the
public-context owner correction; later changes before final gates are formatting/docs only.

Final locked workspace all-target and independently locked consumer all-target/all-feature
checks pass on Windows 11 with Rust 1.95.0 and one build job. Core/Windows/WGPU all-target/
all-feature Clippy passes with `-D clippy::correctness`; existing nonblocking warnings remain.
Root and independent-consumer formatting, import-boundary, public-document, release-document
and public-API-tier checks pass. Independent Core/App, Cosmic/native and public-context owner
reviews leave no outstanding P1/P2 in this unit. The declared Rust 1.92 floor and dependency
versions remain unchanged; Rust 1.92 execution is not part of this qualification.

Windows/CoreText retain the default optional missing-glyph sink; this unit does not implement
their reporting. The fork's current Web platform uses Cosmic directly. The fixed target's
later Canvas fallback wrapper and post-fallback missing-index scan remain in the separate Web
platform-closure migration. No unregistered wrapper was added. Applications own fetching and
explicit text-view notification/Window refresh; clean cached children do not rebuild from a
parent-only notification. Native selection caches for an already matched descriptor keep their
existing backend policy; adding new weights/styles to that family or replacing native fallback
selection requires separate reconciliation. The native test qualifies a previously missing
family, not that broader reselection workflow. OS IME/input, native accessibility, full platform execution, measured
costs, complete upstream parity and Nara rendering integration remain outside this qualification.

## Native Font Reselection And Retained Faces

This successor to `ed2a55383f4` addresses already matched native families and privately installed
fallbacks. The fixed Zed target remains `635026fabc38440fce5e1767d125026a17b12012`; its
DirectWrite source retains the same descriptor-cache and system-only fallback gaps. This is an
owning correction found during the framework comparison, not a claim that those fixes were
already present upstream. The earlier missing-family trajectory did not qualify these paths.

The first expanded five-case baseline runs one passing original native trajectory and four
failures (`f522ebff-6576-4f9d-b914-4e41a7e5401b`). Three weight-fixture assumptions required
correction: DirectWrite synthesizes Bold from Regular, reports weight 700 already, and gives
the synthetic `m` a 620-unit advance rather than the real Regular/Bold fixtures' 600 units.
The corrected baseline again passes the original native case and fails all four new paths
(`dc83e89b-bb50-4989-a6c2-227015470f37`): private fallback is not selected, newly installed
Bold remains simulated, successful retry retains that simulation, and the native Bold request
retains its old FontId. Actual simulation flags and fixture outlines distinguish the faces.

An independent review also identifies an unknown-face callback that reconstructs a descriptor
from family/weight/style and can bind an old simulated run to a newer real Bold face. Its
isolated cold-cache regression fails before the owning correction
(`54edf648-aa57-475e-b6b6-834af7539847`). The test deliberately drops the auxiliary native-face
cache and clears the selected descriptor to isolate this callback from the separate stale
selection defect; it still shapes a real retained native font collection and compares actual
simulation, glyph, advance and bounds. No fabricated native presentation receipt is used.

DirectWrite selection now checks its private published-collection generation. A reselected
unchanged native face keeps its FontId while its collection/fallback metadata refreshes;
a changed actual face appends a record, leaving the old ID usable. Complete Font descriptors
keep feature/fallback configuration separate. Private fallback mappings precede system ones
and carry their actual collection. Auxiliary identity keys retain the native face whose
IUnknown address they index, so releasing a temporary layout cannot leave a dangling identity
key. Unknown callback faces are retained verbatim with a collection built from that actual
face reference; they are never substituted by a current name-based selection.
Successful system-collection refresh also advances the backend selection generation. This
revalidation does not install OS fonts or provide a Core-level OS font-change watcher. Identity
reuse keeps exact COM equality and otherwise requires native FontFace5 equality; an unsupported
or unconfirmed comparison appends a record rather than merging its glyph identity.

The Windows batch builder can retain valid fonts processed before a later invalid file, but
its published collection and selection generation change only after successful collection
publication. A failed batch keeps the prior visible selection; the next successful publication
can include that builder's retained fonts. This is a concrete Windows recovery policy, not a
cross-platform atomic-rollback guarantee.

The first corrected focused cohort passes 6/6 (`4818c428-27a5-4ceb-840b-4e5d48ea86c9`),
including both actual native installation trajectories, private Noto fallback/raster at scales
1.0/1.5, failed-batch recovery and retained simulated callbacks. The native fixture shares one
Render/shape/paint/Submitted/retirement path, checks the original App/Window TextSystem,
explicit child notification, exact new generations and old-layout repaint. Real Lilex `m`
outline bounds distinguish installed Regular/Bold. Submission is not glyph-pixel appearance
or OS compositor evidence; private font bytes do not enter the OS font store.

The expanded complete Windows run then passes 194/195 with four existing interactive cases
skipped (`6ef63fd7-83d3-42cc-85d2-36e1db2555b6`). A second shape through an observed synthetic
FontId exposes a real remaining defect: putting its simulated face reference directly into a
new collection produces a later 600-unit advance instead of the original 620 units. The
collection now uses that face's backing file/index without simulation, while the record keeps
the exact observed face and its effective weight/style. DirectWrite reapplies the simulation
when shaping from the isolated backing collection. The regression retains its second-shaping
assertions and adds an actual previously unregistered synthetic face, so finding an already
loaded record cannot stand in for the corrected native construction.

The corrected focused cohort passes 9/9 (`57d4cde2-b931-4938-9af3-4c81c2b53600`). The final
complete Windows library passes 197/197 with four existing interactive cases skipped
(`dec391f9-e4c6-4fbd-bbdc-1207999faa5a`), retaining the actual glyph upload/readback,
cached/deferred pixel, native input, candidate refusal/retry and exact retirement cohorts.
The prior Core/WGPU 999 and component/consumer 2,964 results retain the earlier font unit's
scope; those unchanged shared owners are not presented as newly executed by this native-only
correction. No compiler/dependency version changes are required.

```text
cargo +1.95.0 nextest run --locked -p open-gpui-windows --all-features --lib -E 'test(direct_write::font_installation_tests::) or test(native_font_installation_tests::)' --test-threads=1 --no-fail-fast --no-tests fail
cargo +1.95.0 nextest run --locked -p open-gpui-windows --all-features --lib --test-threads=1 --no-fail-fast --no-tests fail
```

CoreText source now invalidates descriptor/family selection after installation, resolves private
fallback descriptors and preserves configured native-font identity across feature/cascade and
size changes. Its five new tests cover actual shaping/raster, additional weight, fallback,
configuration identity and rejected-install retry. The former unused OpenType module is removed.
This Windows host has not compiled or executed the macOS paths; source review, fixture checks
and a passing Windows build cannot establish macOS qualification.
The independent source review finds and corrects a definite typed-dictionary equality mismatch
against locked CoreFoundation 0.10.0 without changing dependencies. Complete attributes compare
through CFType. Shared `find/downcast` traits validation precedes font-kit properties for both
primary and fallback candidates, retaining valid fallback fonts without `m`. Prepared mixed
emoji shaping/raster tests cover 12/16/32px, actual glyph/source identities and old-layout raster;
these remain unexecuted Mac tests. Nondefault variable-axis reconstruction, live OS font-change
watching, measured costs, full platform parity and Nara rendering integration remain open.

Final locked workspace all-target and independently locked consumer all-target/all-feature
compilation pass on Windows 11/Rust 1.95.0 with one build job. Windows all-target/all-feature
Clippy passes with `-D clippy::correctness`; existing nonblocking warnings remain. Root and
independent-consumer formatting, import-boundary, public-document, release-document and API-tier
checks pass. Independent Windows review and Mac source review, followed by the owning corrections
and main integration review, leave no outstanding confirmed P1/P2 in the declared scope. The
declared Rust 1.92 floor remains unchanged and unexecuted. Formatting/docs after the final
197-case run do not change its native behavior; no Mac compilation credit is inferred.

## Cosmic Paragraph Shaping And Public Paint

This successor to `b803d542b67` adapts upstream
`c214057e086517920b214800725c5d16294ddf0d` against fixed target
`635026fabc38440fce5e1767d125026a17b12012`. Cosmic 0.19.0 requires all BiDi paragraphs passed
to one ShapeLine to share direction. The first four-case baseline fails all four
(`2644c7d5-c95a-49cd-9b26-0eda22bc5ad0`): three direction assertions and a width-oracle failure
later traced to the fixture's separator assumption. No source is rewritten or stripped to make
these cases shape.

The adapter shapes each `Bidi_Class=B` separator and neighboring paragraph separately, then
restores absolute UTF-8 indices and horizontal positions while summing widths and maximum
ascent/descent on one logical baseline. A private monotone FontRun cursor preserves original
configuration and consumes zero-length boundaries without rescanning all previous runs per
segment. The unchanged path covers text without paragraph separators, including U+2028.
Existing `shape_text` newline/wrapping semantics and caller-owned valid run coverage remain.
Promoting already resolved `unicode-bidi` 0.3.18 to a direct WGPU dependency changes only the
workspace dependency declaration and package edges in the two lockfiles, not package versions.

The first correction passes three of four (`34d7413e-ee18-4003-ab02-abf0fdb0db80`); the width
fixture had incorrectly assumed separators have zero width under this font/shaper. It now
compares independently shaped pieces including separators. The expanded cohort passes 8/8
(`046c83f5-b536-4ee5-be7c-4ad37bc77205`), including all seven B-class controls in both direction
orders, leading/trailing/repeated separators, CRLF, exact run/glyph/metric comparison, original
source indices and real Hebrew bases/zero-advance marks with repeated raster at scales 1.0/1.5.

A subsequent whole-grapheme reporting regression fails
(`30a8d851-8d4e-4059-be9e-700676557f79`): splitting CRLF for shaping emits a standalone newline
report even though the source has one CRLF grapheme. The raw missing indices are now collected
at their original pre-output-conversion stage, rebased across segments and reported once using
the full source and original FontRuns. This preserves complete clusters and source spacing
classes, including glyphs that might later fail fallback-output conversion.

The public focused run passes nine unit cases and fails its one public consumer
(`e404813c-2184-4ab1-9d3a-dfb984f27826`). Its LTR control reveals an existing Core/fixed-target
defect: `closest_index_for_x` always returns the line end after the last glyph origin, except
for a one-byte line. A position in the final glyph's left quarter therefore returns byte 16
instead of 15. The test assertion remains; the owning Core method must compare the final
cluster origin with the terminal width, keeping left-affinity on midpoint ties, complete UTF-8
cluster indices and empty-layout behavior. This does not qualify general visual RTL caret
mapping or introduce a new text-position model.
The isolated Core regression also fails before correction
(`c783dd64-6595-4015-851a-9280446d392f`): `"ab"` at x=12.5 selects byte 2 instead of 1.
It covers final ASCII, two-/four-byte characters, a ligature cluster, multiple font runs,
midpoint affinity and empty/unshaped controls. This is an owning contract correction discovered
by the paragraph consumer, not a behavior already fixed by the pinned upstream commit.
That independent Core correction is `3d0714f44e206a14fda7d6d830c7a58ce06bc94a`; the paragraph
integration follows it without claiming source parity with the whole fixed target.

The next focused run passes ten of eleven (`2dcfc8cc-8215-4fe3-9eb2-7ea6760936ae`), including
the Core correction and all paragraph/reporting cases. The consumer's glyph-atlas diagnostic
assumption is invalid: glyph paint uses the plain atlas insertion API, while that diagnostic
collection observes image paint. A public PlatformTextSystem delegate now records successful
actual paint rasterizations; direct software-raster oracle calls use the original Cosmic backend
and cannot inflate that observation. The accepted initial frame must rasterize actual Hebrew
Aleph/Bet, and reconstruction under a new window atlas must trigger new raster work. Original
source, layout, hit, notification, wrapping, reconstruction and shutdown assertions remain.
The corrected public consumer passes 1/1 (`8f46966e-df64-4ed9-8b8b-80b306030121`), with an
independent review of the observation seam finding no P1/P2. The earlier ten passing focused
cases retain their source scope; the final complete cohort qualifies their combined source.
The final all-feature Core/WGPU/Windows libraries plus public paragraph consumer pass
1,207/1,207 (`0b561894-7883-47da-9f29-3f0a768af220`): Core 943, WGPU 66, public consumer 1 and
Windows 197. Four existing interactive Windows cases remain skipped. This retains actual
production WGSL pipeline creation, atlas lifetime/refusal, App observer/callback cleanup,
cached/deferred native pixels, text/input and exact Windows release. The paragraph consumer
itself remains headless/software raster; running the other native backend does not turn it
into a native Cosmic rendering qualification.
The affected UI core/components/docking/Canvas library/integration cohort passes 2,964/2,964
across 25 binaries with no skipped cases (`e99f26f8-0455-44e8-81f8-c3c2f751c8fb`), retaining
the actual text-field, virtualized collection, deferred overlay and docking consumers of the
shared closest-hit contract.

```text
cargo +1.95.0 nextest run --locked -p open-gpui -p open-gpui-wgpu -p open-gpui-windows --all-features --lib --test paragraph_text --test-threads=1 --no-fail-fast --no-tests fail
cargo +1.95.0 nextest run --locked -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-docking -p open-gpui-canvas --lib --tests --test-threads=1 --no-fail-fast --no-tests fail
```

The [public consumer](../crates/gpui_wgpu/tests/paragraph_text.rs) uses the real Cosmic backend
through public HeadlessAppContext, Render, canvas, shape and paint APIs. It compares explicit
independent segment recipes, raw visual order, complete source/newline/run mapping, actual
Hebrew glyph IDs and repeated software raster. Notification changes text size, canvas layout
bounds change wrapping, repeated frames preserve output and different-size windows reconstruct
under the same exact App TextSystem with one final shutdown. This is TestPlatform accepted paint
and actual paint-raster evidence, not native HWND, GPU submission, compositor output or a backend
resize acknowledgement.

The [tracked Hebrew fixture](../tests/fixtures/fonts/README.md) is unmodified Noto Sans Hebrew
Regular 3.000 from official notofonts revision `ffebf8c1ee449e544955a7e813c54f9b73848eac`, with
its original OFL notice, source blob and SHA-256. It contains real Hebrew letters and marks but
no Latin `m`. The existing primary-family filter can omit it from the explicit fallback chain;
the retained database face still serves Cosmic's automatic fallback. The tests verify the
selected face and glyphs, not general configured fallback priority. Full Unicode wrapping,
complex-script/RTL caret coverage, fallback-policy reconciliation, measured costs, remaining
platform execution and Nara engine integration remain separate work.

Final locked workspace all-target and independently locked consumer all-target/all-feature
checks pass on Windows 11/Rust 1.95.0 with one build job. Core/WGPU/Windows all-target/all-feature
Clippy passes with `-D clippy::correctness`; existing nonblocking warnings remain. Root and
independent-consumer formatting, import-boundary, public-document, release-document and API-tier
checks pass. Independent production and public-observation reviews leave no outstanding P1/P2
in this unit. The declared Rust 1.92 floor remains unchanged and unexecuted; native Linux/macOS,
browser execution, measured costs and full framework parity remain unqualified. Documentation
closure does not require repeating the already green product cohorts.

## Unicode Wrapping, Indentation And Middle Truncation

The successor to `f48d20d27862e45ff27fcd7ba4f261fd9119d0f7` reconciles fixed-target
`f0210f1621c6be9df4510a4356fa5049680e1c01`,
`4f70d91bda7ec5f5600a000a0dc34391d9f1e96f` and the earlier omitted
`03a8544040ff95ec2e7921ca71c83799ad7f17cd`. The baseline run fails both selected regressions
(`b30402fc-e926-4119-acc2-801f06ccd65a`): `aaa/bbbb` breaks before the solidus, and repeated
combining graphemes break inside their source clusters. Preserve these failures as the owning
motivation rather than treating a symbol-level source match as qualification.

The wrapper now segments complete fragment text with Unicode line-break opportunities, retains
inline-element byte accounting and atomic widths, and keeps emergency cuts at complete grapheme
boundaries. Explicit indentation covers ordinary, extra-column, saturated, narrow, all-space and
carried-suffix cases. Shaped wrapping filters break opportunities through actual native cluster
indices and retains the fixed target's RTL workaround. This is the non-complex-scripts segmenter,
not full complex-script segmentation or general visual RTL caret qualification.

Middle truncation flows through `TextOverflow`, fluent `Styled` and normal text measurement,
preserving both source ends and exact output TextRuns. The target's character-wise cuts can
detach combining marks in Middle, Start/End and wrapped End. Those owning walks now use source
graphemes. A separate tiny-budget Start correction returns only the affix when no source content
fits. An affix wider than the budget remains visible, preserving the existing truncation policy.
The existing descriptor/width generations, independent width pool and single-Mutex candidate
cache ownership remain; no global text state or second layout cache is introduced.

The first focused run passes 52/52 (`9f3cc52c-f9f7-41b3-bdde-392d8b3ea373`), covering wrapper,
shaped layout and pool/reconstruction cases. It precedes the additional tiny-budget Start test.
The public Cosmic consumer initially fails to compile because its pointer assertion does not
coerce `Arc<WindowTextSystem>` through to `TextSystem`; the explicit typed reference corrects the
test. It then passes 1/1 (`f2ef743f-dfcf-463d-9f81-eebe41f69f11`). Its public Render/canvas
shape/paint trajectory includes style-driven Middle output, exact multistyle byte redistribution,
Unicode/emoji boundaries, explicit notifications, changed text size, narrower allocation, repeated
frames, window reconstruction and final TextSystem release. It uses actual licensed Cosmic font
fixtures and TestPlatform accepted paint; it does not establish native Cosmic GPU/compositor output
or dedicated CJK glyph coverage.

The root lock resolves ICU segmenter/data 2.3, with required ICU/zero-copy prerequisites, and the
independent consumer resolves compatible ICU 2.4 alongside its existing Taffy/WGPU pins. The initial
Cargo resolution proposed unrelated platform/HTTP/proc-macro upgrades; they were removed before
acceptance. The retained root version changes are the necessary ICU/zero-copy closure, including
Syn 3 for `zerovec-derive` alongside existing Syn 2. The existing duplicate policy records this
actual coexistence. These dependency facts alone do not qualify a compiler floor or numeric cost.

The independently locked public consumer passes both API tests
(`c4a631af-08c4-44d4-b7c0-e21354778540`) using its own ICU 2.4 and WGPU/Taffy resolution.
This first run precedes the shaped/truncation review corrections below and uses NoopTextSystem;
it qualifies public API consumption rather than actual-font or native behavior.

Independent correctness review identifies three remaining workflow defects in the adapted target:
distinct shaped indices can still split one source grapheme, a fitting suffix carried into the
last row is discarded by wrapped End, and wrapped Start's summed-width budget can exceed its
actual row count. A filter retaining only source grapheme starts is insufficient for RTL marks:
the boundary must move the first visual glyph with its source grapheme across font runs. These
regressions require owning fixes and executed reruns before this unit closes.

All four added regressions fail before correction
(`8695b6cd-6c19-46d5-ac8a-b53a4f7e7a2a`), including both LTR/emoji source clusters and an RTL
mark/base split across font runs. The layout owner now maps existing native indices to source
graphemes in one source/glyph pass, groups adjacent visual glyphs by that identity and keeps each
group's first actual glyph as the WrapBoundary. Painting retains all original glyphs. The UAX/native
break intersection and source metrics remain. This removes the reproduced mark/base row split
without claiming general visual RTL hit testing or logical reconstruction of reordered rows.

The initial Wrapped End correction immediately records the furthest carried suffix that fits with continuation
indentation and affix, including immediate overflow on entering the last row. Wrapped Start first
checks real wrapper rows, then probes complete-grapheme suffixes with the same wrapper/newline
rules. Its bounded search retains a fitting candidate instead of assuming summed widths prove
row fit. It performs at most logarithmically many source probes; no new cache/shaping owner or
measured speedup is claimed. Context-dependent exotic affixes are not a global maximal-suffix
guarantee. The complete affix remains visible when it cannot fit by itself.
The corrected focused cohort passes 57/57 (`c9fb7849-69d9-48dc-99cf-2f4c584d529b`), retaining
all original wrapper, shaped-layout, font-generation, rejected-candidate and lifetime assertions.

The expanded public style and two Windows native trajectories pass 3/3
(`35a46ddc-ce9f-47f2-bdc5-bf4a497a3089`). Public `line_clamp` Start/End now checks the actual
displayed affix, retained suffix, carried content and independently shaped row widths. Native
DirectWrite paints the complete finite-width Unicode source and redistributed Middle runs in the
same frame. The existing receipt predicate requires the exact accepted, Submitted, nonempty,
rendered and diagnostics generation. Notification narrows 240px to 112px, increases wrap rows and
shortens Middle, whose actual shaped width must fit each budget. Original tab/separator/source,
notification, retirement and TextSystem/platform-release assertions remain. These fixtures are
native paint/submission evidence, not compositor glyph-pixel or general fallback-priority proof.

The complete all-feature Core/WGPU/Windows libraries plus both public text consumers pass
1,245/1,245 (`669adc81-342d-4d69-9076-06e663266714`): Core 980, WGPU 66, Windows 197 and two
public consumers. Four existing interactive Windows cases remain skipped. This retains the actual
production renderer pipeline, native cached/deferred pixels, input, candidate refusal/retry, font
installation and exact native release. It does not convert the Cosmic consumers' headless/software
scope into native Cosmic GPU qualification.

Further review identifies a zero-deletion Start candidate with incomplete TextRun coverage, an
unhandled explicit newline at a one-row limit and a CRLF probe that removes a character retained
by `shape_text`. Four additional regressions fail before correction
(`1f4d5f23-384e-45ef-92d2-ae409df21ada`). They include public unclamped `shape_text` row counts,
all three directions at a one-row limit and whole-grapheme Middle paragraph ends. The first
component-cohort build was interrupted before execution to correct these known owning defects;
the complete cohort must be rerun after the successor below.

The final successor removes the duplicate End wrapping walk. Start and End share bounded actual
wrapper probes over exact layout paragraph contents, including retained CR, and strictly remove
source graphemes before adding an affix. Empty trailing LF/CRLF paragraphs can be omitted without
an affix, retaining the established visible-content policy and exact adjusted runs. Wrapped Middle
keeps complete first/last paragraph ends together through the affix; direct flat `truncate_line`
semantics remain. Both overflowing multiline directions have bounded logarithmic probe counts;
ordinary flat single-row labels retain their existing path. This is a correctness/ownership
consolidation, not a measured performance improvement.
The expanded focused cohort passes 62/62 (`30260147-a359-48b7-80ec-136b0120e73a`), combining
61 owner regressions with the complete real-Cosmic public style/paint trajectory. The latter adds
actual one-row Start/End/Middle controls, multi-paragraph Middle and indented Start. It checks
adjusted styles and UTF-8 coverage, public unclamped shaping row counts, independently shaped
row widths and existing notified reflow, reconstruction and final release.

The final bounded review also reproduces an End trim splitting `Prepend + ASCII punctuation`
(`11beb561-85fa-48d0-986c-0c53f14b0b2a`). Both flat and wrapped End now share a whole-grapheme
trim: a trailing grapheme is removed only when every character meets the existing whitespace or
ASCII-punctuation condition. The regression covers exact flat output, wrapped widths 1 through 6
and exact TextRun coverage.

The first actual Rust 1.92.0 framework check fails with E0658 at
`gpui_util::TypeIdHasher`: `slice::as_array` is unstable on that compiler. Independent correction
`8d35827da9a0bd335c3e85201ee82dbd77135224` uses `first_chunk::<8>()`, retaining the first eight
native-endian bytes and the existing short-input policy. Its existing TypeId variety test and
new exact eight-/nine-byte test pass 2/2 under Rust 1.92.0
(`c2fab855-337e-4fbc-b7cd-6a2c21c476d3`). The declared floor remains unchanged; the complete
affected/independent checks and final runtime cohorts follow this correction.
The fixed target's `rust-toolchain.toml` pins Rust 1.98.1. That source environment is distinct
from the extracted framework's actual minimum-compiler requirement.

The next floor check exposes the existing profiler's unstable `std::hint::cold_path`; a private
`#[cold]`/`#[inline(never)]` marker preserves its hint intent and leaves sampling/statistics work
unchanged. A further check reaches Windows and rejects an existing `if let` match guard. The
native owner now matches the decoder's exact eight button-message constants and chains the same
decoder/handler; invalid XBUTTON still returns the existing default-handler path without pointer
side effects. Independent bounded source review confirms message-set equivalence, disjoint arms,
callback order and unchanged profiler state, with no P1/P2. Explicit malformed-XBUTTON native
message execution is not newly qualified by that source review.
The affected Core/WGPU/Windows all-feature/all-target check then passes under actual Rust 1.92.0
on Windows 11. These are source-compatibility corrections rather than a raised compiler floor or
measured performance claim; the independently locked consumer and final runtime cohorts follow.
The independently locked consumer's all-target/all-feature check also passes on Rust 1.92.0,
retaining its ICU 2.4, Taffy 0.10.0 and WGPU 30.0.1 resolution. Together with the affected
framework check, this executes the declared compiler floor for the selected Windows closure.
It does not qualify Linux/macOS/browser compilation or their runtime behavior.

```text
cargo +1.92.0 nextest run --locked -p open-gpui-core-util --lib -E 'test(type_id_hasher)' --test-threads=1 --no-fail-fast --no-tests fail
cargo +1.92.0 check --locked -p open-gpui -p open-gpui-wgpu -p open-gpui-windows --all-features --all-targets
cargo +1.92.0 check --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all-targets --all-features
```

After the final paragraph, whole-grapheme trim and compiler-compatibility corrections, the complete
all-feature Core/WGPU/Windows libraries plus both public text consumers pass 1,250/1,250
(`b25bbe64-bdd7-4170-94bc-f6574df16ca0`): Core 985, WGPU 66, Windows 197 and public consumers 2.
Four existing interactive Windows cases remain skipped. This supersedes the earlier 1,245-case
source checkpoint for this unit and retains the complete native Unicode/Submitted trajectory,
candidate refusal/retry, cached/deferred pixels, font installation and exact release assertions.

The first final consumer build stops before test execution with MSVC `LNK1318: Unexpected PDB
error; LIMIT (12)` while linking the Canvas test program. An owned retry is interrupted before
execution after inspection finds less than 0.1 GiB free on the target volume. Twenty-five verified
old Open-GPUI PDB files are removed without deleting source or the current test programs. The
same component command then passes 2,964/2,964 across 25 binaries with no skipped cases
(`197fc02e-5fe1-430c-8f9c-c6366522dacf`), retaining existing field, collection, overlay,
docking and Canvas consumers. No acceptance assertion or product source changes for this retry.
The independently locked public API consumer then passes all three tests
(`ad32664d-0dad-4026-a2e2-1b4e68eb83c2`), including fragmented graphemes and wrapped Start's
real row budget. This consumer retains NoopTextSystem scope. Eight additional obsolete
prior-checkpoint symbols are removed to keep space for its build and the final checks.

The verification workflow now includes a narrow Windows Rust 1.92 job for both locked checks above.
The official toolchain action's `1.92.0` ref was verified, and the workflow parses as YAML. These
commands passed locally; the new hosted job has not been executed or published in this continuation.

Locked workspace all-target and independently locked consumer all-target/all-feature compilation,
plus Core/WGPU/Windows all-target/all-feature correctness Clippy, pass on Windows 11/Rust 1.95.0
with one build job. Existing nonblocking warnings remain. The existing dependency-policy tests
first stop during compilation with `no space on device`, before execution. After removing verified
ordinary object/cache files from eight old Open-GPUI incremental checkpoints, their unchanged
command passes 2/2 (`067a7706-a6be-41f1-855f-324430ce415a`; 86 unrelated cases filtered out).
Root and independent-consumer formatting checks also pass; they are explicitly rerun after the
interruption. Earlier completed behavioral results retain their source scope after the user's
subsequent build-cache cleanup; no product source changes for these environment recoveries.

Independent text and compiler/native-dispatch reviews leave no outstanding P1/P2 in this unit.
Existing import-boundary, document-link, release-document and public-API checks pass again after
rebuilding the original xtask executable following build-cache cleanup.
The new Cosmic public trajectory remains headless/software evidence; native DirectWrite/Submitted
frames qualify their own backend. Complete complex-script segmentation, general visual RTL caret
and logical reconstruction of reordered wrapped rows, configured fallback priority, other native
platforms/browser, numeric costs, engine View integration and the full fixed-target refresh remain
open. These changes establish no new Nara Editor or game-UI default.

```text
cargo +1.95.0 nextest run --locked -p open-gpui -p open-gpui-wgpu -p open-gpui-windows --all-features --lib --test paragraph_text --test wrapping_text --test-threads=1 --no-fail-fast --no-tests fail
cargo +1.95.0 nextest run --locked -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-docking -p open-gpui-canvas --lib --tests --test-threads=1 --no-fail-fast --no-tests fail
cargo +1.95.0 nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all-features --test wrapping --test-threads=1 --no-fail-fast --no-tests fail
cargo +1.95.0 check --locked --workspace --all-targets
cargo +1.95.0 check --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all-targets --all-features
cargo +1.95.0 clippy --locked -p open-gpui -p open-gpui-wgpu -p open-gpui-windows --all-targets --all-features -- -D clippy::correctness
cargo +1.95.0 nextest run --locked -p xtask --lib -E 'test(dependency_health::tests::)' --test-threads=1 --no-fail-fast --no-tests fail
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 fmt --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all -- --check
```

## Font Preparation, Inventory And Raster Reuse

The successor to `b466037e87e17f11e7acce70706b7866edda3ca1` reconciles fixed-target
`f1d27d545e79f98ffde7ebf3229f8eb8bad791aa`,
`5b4a2153a087055a7f9cb3464f188f1cab9fd678` and
`1ff7cb669b6351eaf2a5b79ccb40f5cd1676b8f8`. The two initial inventory regressions fail on the
baseline (`8a8c2f1f-e125-4a8d-b094-c4933ab04bc5`): a deterministic backend's two actual families
grow into twelve suggestions, and an empty backend reports guessed families and virtual aliases.
The owner now returns only sorted unique backend names. Alias resolution is unchanged.

The public explicit prewarm entry resolves full descriptors using the existing cache and sends
distinct resolved IDs to the optional platform hook. Core tests count actual descriptor queries,
consume the prepared fonts through normal shaping and retain failed-installation reuse and
successful-installation invalidation. The selected Core installation, window-candidate and width-
pool cohort passes 21/21 (`de182c08-de7b-45a8-9269-b48c60c94f8f`; 968 unrelated cases filtered).
This moves cold lookup work before consumption; ordinary repeated resolution already had a cache.

Cosmic's prepared match properties are shared with primary/fallback shaping. The pending image
handoff uses complete RenderGlyphParams under the existing backend lock. The fork adds local
256-entry/8-MiB image-buffer-capacity admission to prevent unbounded measurement-only pixel
retention; pressure discards temporary images and rasterization regenerates them. It introduces
no glyph output limit, new atlas or second lifetime authority. These limits apply only to the new
handoff, not the existing bounds cache, complete font memory or process retention.

The first backend build stops with E0282 before execution because the measured `Bounds` local
needs an explicit `Bounds<DevicePixels>` annotation before its component fields are inspected.
The annotation corrects this integration error; the backend/public rerun follows. Independent
source review finds no confirmed P1/P2 and adds full cold-byte comparisons to interleaved
consumption and oversized-image controls before their first execution.

The corrected selected Cosmic installation/work-reuse suite and both complete public text
trajectories pass 29/29 (`520f8449-7fc3-4771-803f-86d7e465c31c`; 52 unrelated cases filtered).
Thirteen new backend cases count actual Swash render attempts, compare independent cold size/
placement/bytes at scales 1.0/1.5, and exercise grayscale, subpixel and actual Windows Segoe UI Emoji
color output. Complete key partition, replacement/consumption capacity accounting, interleaved
hits, entry/byte eviction, oversized recomputation, empty/error recovery and old installed-face
retention pass. The prewarm controls verify actual cached match identity and complete attributes,
then compare cold/warm features, fallback, paragraph layouts and missing-grapheme reports.

The public paragraph trajectory forwards preparation to real Cosmic, observes complete bounds/
raster keys through actual paint, and retains notified reflow, window reconstruction and final
TextSystem/wrapper/backend release. Unchanged atlas reuse remains an existing control rather than
a new benefit. The measured new effect is removal of the second image-generation attempt in the
bounds-to-raster handoff; these results are not frame-time or retained-process measurements.

The first selected Windows run passes 2/3 and fails the original MissingFamily trajectory
(`08e91d6e-5726-46ef-8e35-a471d8fd7332`): prewarming an additional alias replaces an already
loaded native face's callback-cache FontId. The actual drawn first glyph reports FontId 2 rather
than the retained fallback's FontId 0. AdditionalBold and complete native App/TextSystem/backend
reconstruction pass in the same run. The existing source/face-identity assertion is retained.

A deterministic DirectWrite alias regression reproduces the same owner defect independently:
shape installed IBM Plex, load `.ZedSans`, then shape the original FontId again. The last step
incorrectly reports the alias ID (`24d8049f-12dc-4593-98d0-76493af1473e`). Selection now keeps
the first native callback-cache mapping instead of overwriting that physical face's identity.
Per-descriptor IDs/features/fallbacks, retained COM identity and unknown native-face handling
stay in their original owners. The added regression also retains complete glyph count/IDs,
positions/source indices and width. Corrected complete font/native and broader reruns follow.

The corrected selected DirectWrite installation/identity suite and all three complete native
installation/reconstruction trajectories pass 11/11 (`a79e2577-8459-4544-b233-76b85d9809a2`;
191 unrelated cases filtered). Additional alias preparation remains in the real consumer. Both
newer accepted/nonempty Submitted generation and retained old glyph repaint keep their original
metrics/outlines/source checks; exact HWND/platform/OLE retirement remains. DirectWrite's platform
prewarm hook is the optional no-op, so these results establish no new native raster speedup.
Independent DirectWrite review confirms the correction retains descriptor-specific typography,
fallbacks and input IDs while the same physical face has one stable first-registered callback
ID. Shaped glyph face identity is not a promise that every alias/feature descriptor's distinct
input ID must be echoed. Layout applies the requested descriptor's features before the callback;
the correction does not merge those descriptor records. Installation, simulations and unknown
face recovery remain covered by the complete selected suite above. This prerequisite is committed
separately as `19a4ed3434a19e0cedbaadff5bbd63bac00e7441`.

The final all-feature Core/WGPU/Windows libraries plus both public text trajectories pass
1,268/1,268 (`c29f031c-e4b1-433a-b3b5-b13bee4deced`), with four existing interactive Windows
cases skipped: Core 989, WGPU 79, Windows 198 and two public trajectories. This includes the
complete native font-installation, retained-face, text, cache/input/rendering and native retirement
cohorts. Affected UI core/components/docking/Canvas pass 2,964/2,964 across 25 binaries with no
skipped cases (`6f52fee2-fc28-4058-aa96-f48338152a94`). The independently locked public fonts
and wrapping tests pass 5/5 without skips (`0138e675-b87d-48c5-bf91-5fc218382048`). They retain
Noop's empty inventory and actual Cosmic aliases, mixed-source shaping, installation, retained
old glyph bytes and final TextSystem/backend release. These consumers use software rasterization;
the Windows native trajectories above qualify their own backend. Final compiler/tooling
qualification now passes on Windows 11 with one build job and the shared target: locked workspace
all-target compilation, independently locked all-target/all-feature compilation, affected
Core/WGPU/Windows all-target/all-feature correctness Clippy and both explicit formatting checks
under Rust 1.95.0. Actual Rust 1.92.0 also passes affected Core/WGPU/Windows and independent
consumer all-target/all-feature checks. Existing nonblocking warnings remain. Import-boundary,
public API-tier, public-document-link and release-document scans pass. Independent implementation
and DirectWrite identity reviews leave no outstanding P1/P2 in this unit.

The final qualification includes cold reconstruction after the owner's build cleanup. The
workspace check rebuilds the existing aws-lc-sys native dependency before completing; this is
build evidence, not a runtime-cost measurement. No dependency or lockfile changes are needed.
Core's optional hook remains a no-op in other backends unless they override it. CoreText source,
Linux/browser execution, complete fallback priority, script/caret support, numeric frame/process
budgets and engine rendering integration retain their separate scope. This closes the font-work
unit, not the complete fixed-target refresh or canonical-docking replacement.

## Source Fragments, Text Layers And Underline Exclusions

The successor to `1d148d66dfdf89115046e08d6e01923b4b2e2165` reconciles fixed-target
`2936989f1b`, `f792c2d72d`, `bda9c0bd43` and `0dea8c63d9`. A deterministic visual Hebrew
glyph fixture reproduces the original split's index underflow before correction
(`11738462-2342-4139-be83-3f1d57bece89`, 0/1). `LineLayout` now owns the source-index partition;
ShapedLine delegates to it, retaining visual order and rebasing suffix indices/coordinates.
Ordered cursor traversal advances glyphs/decorations once, while reordered input scans its
remaining glyphs. Repeated and invalid UTF-8 boundaries, complete source reconstruction,
zero-length decorations and refusal recovery retain explicit tests. These fragments preserve
the existing shaping; they are not independently reshaped bidi/reflow/caret layouts.

TextLayout shares immutable measured lines through a local Rc snapshot and releases its internal
borrow before backend calls. Explicit background/foreground passes propagate errors; ordinary
painting keeps logging and finishes backgrounds before foregrounds. Both row passes use aligned
wrapped footprints, continued-decoration origins and exact final row endpoints. The zero-advance
background fix computes width after the existing half-cell origin adjustment. Underline callbacks
carry source ranges and resolved colors/styles; contiguous equal resolved styles merge ranges.

Window bounds and painting share the actual prepared underline, with existing snapped subtree
projection and opacity. Device-space exclusions normalize intervals and add visible-segment exact
clip rectangles to the existing clip stack. They retain full underline geometry, phase, transformed/
rounded ancestor clips, replay and sibling recovery. Five focused Window cases cover scales
1/1.25/1.5/2/3, fractional origins, nested transforms, invalid projection, empty geometry and
unordered/overlapping/adjacent/outside/covering exclusions.

The first focused library run passes 13/14 (`8950686e-fac7-462b-86c7-1314dc1f06df`). Its only
failure is a newly authored test using `Bounds::contains` on the exclusive bottom-right corner;
the test now compares enclosing right/bottom edges directly. All exact row background/underline
geometry assertions remain. The corrected RTL/cursor/callback and all five Window cases pass in
that run; the complete layer/public/native consumers follow.

Independent review exposes another owner defect: a positive-advance mixed RTL prefix can retain a
glyph beyond that advance, so an advance-only batch hides its footprint from a later opaque quad.
The actual raster-sprite/cover regression fails before correction
(`1cc9d8f6-46d6-42bb-9cc4-65ba90125b46`, 0/1). Foreground/background painting now bypasses the
whole-line batch for unordered, non-increasing-position or outlying glyphs and lets actual primitives participate in
ordering. Ordinary byte-ordered contained text retains aligned/wrapped batching. This corrects
the introduced fragment contract without guessed ink padding or additional raster preparation.

The first combined public/native focused run passes 18/19
(`ee46454a-b2f3-4915-9c29-90d74f46e6a5`); the native fixture's intersection recomputes a height
from 6.0000005 to 6, making exact Bounds structure equality an invalid containment oracle.
Direct left/right/top/bottom containment retains the intended geometry check without changing
the device-alignment, source, callback, submission or retirement requirements. A zero-advance
background/cover ordering assertion also covers its half-cell extension beyond the advance.

The corrected focused run passes 19/19 (`6442d6c2-a21a-4691-82b9-f15380999beb`; 1,266 unrelated
cases filtered). It includes actual primitive row/order/count checks, backend reentry, real
Cosmic source fragments/layers and the complete native text submission trajectory. The latter
retains every original source/tab/separator/font-feature/wrapping/Middle/notification/retirement
assertion and adds raw layout painting, source-verified RTL fragments and forwarded/excluded
underline callbacks. An unused integration-test import is subsequently removed; aggregate runs
use that same production source.

The final all-feature Core/WGPU/Windows libraries and all three public text trajectories pass
1,281/1,281 (`760c6f5e-f1d4-4d2e-9f89-aac97455a516`): Core 1,001, WGPU 79, Windows 198 and
public consumers 3, with four existing interactive Windows cases skipped. The complete native
text/cache/input/font/render-resource/color-pixel/retirement cases retain their existing scope.
Affected UI core/components/docking/Canvas pass 2,964/2,964 across 25 binaries without skips
(`dc0f85c4-38ce-436a-b6d2-dcad362d3dbc`). Independently locked fonts/wrapping/text-layer tests
pass 6/6 without skips (`e72b5ba0-3e73-404b-bfec-a7ebc68a8941`). The new independent test checks
the exact public method signatures and a custom Element's source construction; it does not launch
that element. Real layered drawing and native submission are established by their separate
consumers above.

Final locked workspace all-target and independently locked consumer all-target/all-feature
checks pass on Windows 11/Rust 1.95.0 with one build job and the shared target. Affected
Core/WGPU/Windows all-target/all-feature correctness Clippy and both explicit formatting checks
pass. Actual Rust 1.92.0 passes affected-framework and independent-consumer all-target/all-feature
checks. Nonblocking warnings remain, including existing project lints on test unwraps and argument
counts. No dependency or lockfile changes are required. Existing import-boundary, public API-tier,
document-link and release-document scans pass.
The split/cursor reviewer closes the original ordering P2 after the correction and actual
regressions; independent Window/TextLayout review finds no additional confirmed P1/P2. Neither
review runs Cargo independently. Their bounded source scopes and the executed main-thread runs
are distinct evidence. Existing general font-ink overhang is outside this fragment correction.
Actual primitive order
and software raster observations are separate from native Submitted frames, compositor pixels,
numeric costs, other platforms and general script/caret support.

This closes the scoped source-fragment/layer/underline unit. Window-local text ownership,
Taffy/intrinsic/root/grid/stack policy, image/API/platform work, canonical docking and the
consumer engine's View/Editor/game-runtime comparison remain separate incomplete work.

## Window-Local Text Ownership And Source Cache

This successor starts from Open-GPUI `55ddbcde39527629a28555b5eadb678cddfccd71` against fixed
Zed `635026fabc38440fce5e1767d125026a17b12012`. Official `772a8a9100` supplies Rc window-text
ownership, local RefCell state and a per-window font-run buffer, with final-target Cell take/set
from official follow-up `dde76a1dca`; `2a74c8dff6` removes unused
hash-only layout interfaces. The fork adopts these mechanisms without a hash compatibility
path. Normal content keys and full source remain the single cache interface. Global
TextSystem and immutable LineLayout results retain Arc ownership.

The fork separately owns coherent current/previous font generation and `abort_frame` recovery
after rejected atlas admission. Those existing contracts remain required by runtime font
installation and real rejected-candidate retry consumers. The fork also ends RefCell borrows before backend
shaping; publication rechecks generation and prefers a canonical result published by reentry.
No cross-thread window cache is introduced to retain an obsolete mutex-era test fixture.
Shared-global font installation still executes on a real worker thread, with range/cache
operations on the window owner.

The public Cosmic consumer uses two actual headless windows, exact shared global identity,
distinct local caches, plain/wrapped source reuse, notified width/font changes, retained old
glyph painting and close/reconstruct/release. The independent lockfile consumes the public
getter signature and directly executes two Noop WindowTextSystem owners; it does not launch
an App/Window. The existing Windows reconstruction trajectory adds real nonempty Submitted
frames, distinct rebuilt Rc caches and retained-handle lifetimes while preserving all original
wrapper, HWND/platform and backend release assertions.

The first stable-source focused run passes **29/29**, with 981 unrelated Core tests filtered:
`1362d402-0bb9-408a-aacf-1c33e178ff8a`. It includes the nine new window-entry reentry/unwind,
font-publication, canonical-key and abort/range scenarios, all five adapted generation cases,
the existing public font-installation tests and both actual rejected-candidate retry cases.
The source-preserving plain and ShapedLine paths share the same canonical layout, so dynamic
installation now requires exactly two backend shapes across the two generations rather than
retaining a duplicate hash-cache path.

The initial public/native cohort passes **5/5**, run
`85d8d47c-aa0b-4d8d-ac2b-15e48517495f`: the Cosmic two-window consumer, both existing native
font-installation trajectories, native Unicode text and three-cycle native App reconstruction.
Independent consumer review then identifies that forced refresh masks a notification failure.
Notified width and font updates now capture the prior paint count and only observe the
automatically accepted successor after update/dispatcher settlement. They issue no extra
refresh or draw; explicit refresh remains solely in the cross-frame reuse checks.

Independent cache review also identifies max_lines missing from the wrapped cache key in both
the starting fork and fixed official target. Run `6a7ae099-7364-44b3-828f-1ac61ee7aa5a` reproduces
the defect: after unlimited wrapping, clamp Some(1) reuses three boundaries while a fresh owner
returns none. The corrected notify-only public consumer passes in that same **1/2** red run.
The existing owned/borrowed content key now includes the wrapping limit, with None for plain
layouts. Both clamp orders, exact source, same-clamp reuse and committed/aborted frame cycles
use a fresh-owner oracle. This is an explicit fork correction to an inherited official defect,
separate from the official Rc/Cell/hash-interface migration.

The corrected complete all-feature Core/WGPU/Windows libraries plus the four public paragraph,
wrapping, layer and window-text consumers pass **1,292/1,292**, with four existing interactive
Windows cases skipped: `45531404-9e56-47a9-af74-d0decc341947`. This is Core 1,011, WGPU 79,
Windows 198 and four public trajectories across seven binaries. It includes the reproduced
clamp regression, notify-only successor observations and original native installation,
cached/deferred paint, source layout and exact reconstruction/teardown assertions.
Both independent reviews close their confirmed findings after bounded rereview; the reviews
are static and the serial executed qualification above is main-thread evidence.

Affected UI core/components/docking/Canvas libraries and integrations pass **2,964/2,964**
across 25 binaries without skips: `3eb9734b-720d-437c-9efa-d8222b1ca9c6`. Existing test-support
dead-code warnings remain outside the correctness claim.

The independently locked public fonts/wrapping/layer/window-text consumer passes **7/7**
without skips: `dbf2bdb7-113f-4623-a143-b68c709eb034`. Its new ownership case directly executes
Noop source layouts and exact release; the existing Cosmic case keeps real font installation,
old glyph bytes and backend lifetime. The layer case remains compile-only custom Element API
evidence. These independently resolved dependencies do not turn direct Noop owners into an
App/window or GPU qualification.

Locked workspace/all-target and independent all-target/all-feature checks pass on Windows 11
with Rust 1.95.0, as does affected Core/WGPU/Windows correctness Clippy with all targets/features.
Cargo uses one build job and the shared Nara target throughout. Existing warning categories
remain; these results do not claim a warning-free workspace.

Actual Rust 1.92.0 affected Core/WGPU/Windows and independently locked consumer
all-target/all-feature checks also pass on Windows. The fixed upstream workspace's newer
compiler pin is separate from this extracted-framework floor. Other OS compilation and
execution require their own qualification.

Root and independent workspace formatting checks pass under Rust 1.95.0. The existing built
import-boundary, document-link, release-document and public-API scans pass after documentation
closure, as does diff whitespace validation. No remote CI execution or publication occurs in
this unit.

Native Submitted, headless accepted paint and direct Noop API execution remain separate evidence. Removed locks
and fewer cache paths do not establish a CPU, allocation, frame-tail or process-memory result.
Other platforms, Taffy/API migration, canonical docking and engine adoption remain open.

## Taffy, Text Measurement And Root/Grid Sizing

This successor starts from Open-GPUI `1f01e811c101e5a81e52e955942f6a24a8974902` against fixed
Zed `635026fabc38440fce5e1767d125026a17b12012`. Official `91fdd55889`, `b05f40c554`,
`31ceaf7908`, `cdf33ac25e` and `b488c4e814` supply dependency/alignment/leaf-measurement
changes, truncated-cache invalidation, exact shaped-overflow checking and unknown-list-width
MaxContent probes. Official `b0da438545`, `5ccbbbd88f` and `41d72f6c1d` supply auto window/
prompt root sizing, shared row/column type and row methods, and opt-in stack growth. The
fork retains its pixel conversion, stroke rounding, Rc text snapshots, clips, font generations
and candidate/atlas custody. No new cache, layout or render owner is introduced.

Both lockfiles now select Taffy 0.15.0. The compatible library requirement initially lets
Cargo select 0.15.1; a second explicit update returns it to the recorded target version.
Only the required dependency closure changes. The independent application explicitly pins
0.15.0; its default graph drops the stack-growth packages and old Taffy parsing/grid closure.
The official stacker feature is available without becoming a default dependency.

The first focused compile finds a missing Position import in a new style test, corrected
without production changes. The next run executes **14/18**: four new fixture failures
expose incorrect test assumptions. Auto list-row roots need explicit full width for finite
constraints, Taffy 0.15 PerformLayout passes None known dimensions with adjusted content
available-space, and standalone element measurement must occur within the existing draw
lifecycle. The fixtures now declare those actual constraints/phases while retaining unknown
width source/materialization, logical content-box sizes, all root axes and shrink-wrap checks.
Corrected focused run **18/18** passes, with 1,010 unrelated Core cases filtered:
`79b4946b-1e88-4701-98b9-3a8b9a7b95a0`. The target does not supply a general word-based
MinContent text algorithm; this unit must not claim one.

The first real-Cosmic/Windows selected cohort passes **11/11**, with 276 unrelated cases
filtered: `fd54c3f9-c66d-4840-a977-d8bf8b31daeb`. The new public consumer proves actual
per-character width exceeds the shaped exact-fit width, then remeasures the same element
at tiny, MaxContent and final widths before painting all three truncation directions.
Its notify observation has no compensating refresh/draw. Actual Windows layout/paint,
nonempty Submitted generations and exact reconstruction/retirement pass alongside the
original native font, Unicode and wrapper reconstruction consumers. This precedes the
separately reviewed inherited untruncated-cache correction and is not the final full result.

Independent production review then identifies a second constraint-blind branch shared by the
starting fork and fixed official target: an ordinary wrapped result can answer a later
unconstrained query because the current wrap width is None. Run
`0844d941-1b7a-4915-9e4d-2f9f2bcaf8f1` reproduces **0/1**: a MaxContent request after 60px
wrapping returns **60x384** instead of the fresh owner's **672x24**. The same existing cache
now requires exact wrap-width equality in addition to both official truncation guards.
The regression retains full source and alternates 60px/MaxContent/120px/MaxContent on one
element. This is an explicit fork correction, distinct from the official migration.

Consumer review finds the initial Segoe UI descriptor does not exercise kerning on this
DirectWrite path, so its exact-fit result cannot detect removal of the shaped-width guard.
The native fixture now installs bundled IBM Plex Sans, explicitly enables kern and disables
liga/clig/calt. It requires isolated width to exceed the actual device-ceil shaped width and
the existing wrapper to alter all three exact-fit strings before actual StyledText paint
must retain the complete source. This stronger precondition executes successfully in the
first complete run; the original native notify, reconstruction and exact retirement checks
remain unchanged.

That first complete run, `1f8199e6-fb90-4782-8548-726e2a5800b2`, executes **1,311 passed /
1 failed** out of 1,312, with four existing interactive Windows cases skipped. The only
failure is an existing portal fixture whose absolute root's authored left/top are now
honored by Taffy 0.15. The backend's root-layout implementation explicitly defines absolute
roots through its out-of-flow positioning owner. The test now requires untransformed
(20,30,100,50) and transformed (120,80,200,200), rather than dropping the authored origin
and expecting (100,50,200,200). Original inert/hidden/linkage assertions remain. This adapts
an old expectation to the backend's official positioning contract, without changing portal
or transform production code.

The corrected complete all-feature Core/WGPU/Windows libraries plus five public text/layout
consumers pass **1,312/1,312**, with four existing interactive Windows cases skipped:
`daaf38c4-ab41-455a-9c6b-444697e207ad`. This is Core 1,029, WGPU 79, Windows 199 and five
public trajectories across eight binaries. It executes the inherited ordinary-wrap fix,
corrected portal geometry, the strengthened native kerning precondition and all original
font/cached/deferred/source/notification/reconstruction assertions. Production, consumer and
bounded portal-fixture reviews close their confirmed findings; their static scope remains
separate from main-thread execution.

The stack-growth-disabled Core selection passes **33/33**, with 995 unrelated/feature-specific
cases filtered: `5ecf7a95-3e27-42b4-ad73-bfa0b8a6574b`. It uses `--no-default-features
--features test-support` and includes all new owner/style/root tests plus the portal cohort.
The complete all-feature run above enables stacker. This qualifies both declared execution
configurations within these workloads, not unlimited-depth safety or a speed comparison.

The first affected component/docking/Canvas run, `3062c6f7-c3a5-4516-98fd-9b04ed53c152`,
executes **2,963 passed / 1 failed** out of 2,964 without skips. The only failure is the
Sidebar identity test: four h_full sidebars in one block root now each consume the viewport
height, so the second item is below the visible window. Its fixture now lays out the four
sidebars in a 2x2 grid and requires the two clicked item bounds to be inside the viewport
and disjoint. Encoded identity selectors, disabled duplicate behavior and exact handler
isolation remain unchanged. A missing px import in those new bounds assertions is corrected
before the rerun; no Sidebar production behavior is changed.

The corrected affected component/docking/Canvas cohort passes **2,964/2,964** across 25
binaries without skips: `b956374e-3098-44b8-a015-c56aa2d3ab73`. A bounded independent review
confirms the grid restores the original physical-input premise while preserving all identity
assertions; no additional P1/P2 remains in that adjustment.

The independently locked public fonts/wrapping/layers/window/grid consumer passes **9/9**
across five binaries without skips: `dce2c7df-2ac0-43f1-a2fa-23719840a7bb`. The new grid
cases check public row/column declarations and direct Noop source layout/overflow without
launching an App/Window. Existing Cosmic fonts and retained glyph-byte/lifetime tests keep
their real-font software scope. WGPU remains the application's independently selected
30.0.1 while Taffy is explicitly selected at 0.15.0.

Locked workspace/all-target and independently locked all-target/all-feature compilation pass
on Windows 11 with Rust 1.95.0. Affected Core/WGPU/Windows all-target/all-feature correctness
Clippy passes. Existing warning categories remain outside this correctness gate. All Cargo
commands are serial, use one build job and reuse the Nara target.

Actual Rust 1.92.0 affected Core/WGPU/Windows and independently locked consumer all-target/
all-feature checks also pass on Windows. The fixed official workspace compiler pin remains
separate from this executed extracted-framework floor. No Linux/macOS/browser compilation
or runtime qualification is inferred from Windows checks.

Final root and independent workspace formatting checks pass under Rust 1.95.0. Existing built
import-boundary, document-link, release-document and public-API scans pass after documentation
closure, as does whitespace validation. The final formatting pass changes spelling/layout
only; the qualified product and fixture behavior is unchanged. No remote CI or publication
occurs in this unit.

Public Cosmic headless paint, direct independent API checks and actual Windows nonempty
Submitted frames keep separate scope. The selected consumers exercise repeated constrained/
intrinsic/final measurement, all three overflow directions, actual kerning-sensitive exact
fit, narrower overflow, content grid geometry, automatic notified frames, root reconstruction
and exact release. Source inspection or dependency release notes do not establish numeric
CPU, allocation, P99 or process-retention improvement. Other platforms, canonical docking,
engine View integration and complete R11/R12 remain open.

## Shared Image Decoding And Animation

This unit starts from `11d9a96033ea8425b07f64fdf2bad6503f1720dc` and reconciles official
`9552acc2bc242d45342fa9b5a987d43868aee1ec` with fixed Zed target
`635026fabc38440fce5e1767d125026a17b12012`. Both original decode blocks are inherited
official implementation, content-identical to the imported anchor. The final target puts
static/GIF/WebP decoding under the existing platform owner; resource acquisition remains
in ImageAssetLoader. Static orientation precedes BGRA conversion, animated WebP uses a
transparent background, and animations preserve successful-frame delays while skipping bad
frames and refusing empty output. SVG keeps its existing renderer. Animated orientation is
outside the official mechanism.

The fork retains that implementation and records two local adaptations separately:

- The root `image` requirement is 0.25.4. The official EXIF message states this intent, but
  its actual manifest and final target still say 0.25.1. Inspection of the exact
  [0.25.4 archive](https://static.crates.io/crates/image/image-0.25.4.crate) confirms
  `ImageDecoder::orientation`, `DynamicImage::apply_orientation` and WebP background APIs.
  Both lockfiles remain on 0.25.10, without dependency churn. Archive inspection is not an
  executed minimum-image-version workspace matrix.
- Img uses the existing App executor clock for frame/loading elapsed time, matching its
  asynchronous loading timer. Native scheduling still supplies monotonic time; deterministic
  public consumers can advance the same clock without sleeping or writing frame indices.

The shared helper's contextual error conversion follows official behavior: resource decoder
failures now enter `ImageCacheError::Other`, while transport-specific branches retain their
own errors. The changelog records this observable migration. No new public decode/cache owner
or renderer contract is introduced.

On Windows 11, Rust 1.95.0, one build job and the shared Nara target, the unchanged decoder
first fails both official regressions: animated WebP returns one frame instead of two and
EXIF orientation yields the black first pixel instead of white. Nextest run
`b0912147-8e56-4c1b-bab6-40c09ae75461` reports 0/2, with 1,029 filtered tests. New tests
retain the exact upstream fixture bytes and add color/alpha, all-eight-orientation,
PNG/static-WebP, remaining static formats, GIF delay and malformed/frame-error controls.
The first focused build needs a test-only correction because image::Frame has no Debug
implementation for Result::unwrap_err; it is not a decoder failure.

The first runtime selection reports 14/15 (`a7d51802-48b9-4fca-84cf-d530f34f67c3`):
the generated ICO embeds an RGB PNG that the image decoder correctly refuses because ICO
requires RGBA. The fixture now encodes RGBA and retains all dimensions/pixel assertions.
The direct async loader test initially exposes opaque-future lifetime capture across the
App update closure; test-only static BoxFuture erasure preserves the public loader contract.
The corrected owner/loader selection passes 17/17
(`56dc8a8e-a659-4ee0-8f1e-a6f6afd5244d`).

Independent review identifies a strict loading-deadline defect under the unified clock.
An actual pending Custom Image has no placeholder at 199 ms; its one-shot timer notifies at
exactly 200 ms but the old `>` comparison prevents placeholder paint and schedules no new
notification. The public regression fails without another draw/refresh, alongside 20 other
passing owner/public image tests (`240d194e-afc0-49ac-90dd-945f173b3672`, 20/21).
The fork changes that comparison to `>=`. Review also catches the original official JPEG's
horizontal symmetry: it distinguishes only four of the eight transforms. The additional
matrix now encodes a two-axis-asymmetric JPEG, transplants the exact EXIF segment, compares
every pixel to independently decoded source coordinates and requires all eight results to
be distinct. The unmodified official fixture regression remains. An expanded WebP canvas
also checks transparent pixels outside both encoded 1x1 frames; the shared frame-policy
test preserves nonzero frame offsets. Synthetic frame-error coverage does not claim actual
codec recovery from every corrupted stream.

The corrected selected owner/public/native cohort passes 22/22
(`e2b28bb2-f4a1-41d7-9369-1ba9ac1d3edf`). Actual Windows Img instances independently
complete 0/1/0 frame sequences through encoded Image and embedded Resource. Each observation
binds accepted/nonempty Submitted, current rendered revision, submitted diagnostics and GPU
capture to one generation, checks the decoded delays and frame colors, and observes real
active/foreground status. Repeated frame zero reuses its atlas tile. Stable IDs then replace
the animated sources with EXIF JPEG, and a notified successor paints corrected dimensions and
corner pixels before unchanged-source atlas reuse. Closing and reconstruction verify exact
registered HWND generations, ordered retirement, root/window/App/asset/backend/platform
release and no native invariant failure. No forced draw, frame-index injection or fabricated
renderer receipt drives the native trajectory. Offscreen scene readback is not compositor
display evidence.

Consumer review then strengthens two missing negative controls. Image-only and path-only
headless roots must each receive completion notification and accept their actual paint without
another loader, clock advancement, a loading deadline or compensating draw. Static-image
timelines explicitly drain the frame callback queue, require no animation callback and retain
notification/revision before a separate atlas-reuse draw. The first stricter static check
reports 3/4 (`5e314022-fb4e-42fd-8325-7d24b6e703fe`) because its local revision variable
omits the immediately preceding explicit reuse draw; the fixture now tracks that exact final
revision. The corrected public selection passes 4/4 without skips
(`f1ace74c-e8ee-4558-9554-80ac517ee31b`). Atlas-hit controls remain separate from existing
view-cache replay/refused-candidate suites.

Final runtime qualification uses the same one-job shared target and locked dependencies:

| Executed gate | Result | Nextest run |
| --- | --- | --- |
| Core/WGPU/Windows libraries, all features, plus paragraph/layer/wrapping/window/intrinsic/image public consumers | 1,328/1,328; four existing interactive Windows cases skipped | `4341efab-d63e-4625-9f79-f4b4623b4a6c` |
| UI core/components/docking/Canvas libraries and integrations | 2,964/2,964; no skips | `b93a1d9a-8257-4ae6-a780-7ea257118b7e` |
| Independently locked public font/wrapping/layer/window/grid/image consumers | 12/12; no skips | `d447dd1d-8a64-498b-9edf-186a0715a0af` |

Both bounded production and consumer rereviews close confirmed P2 findings. Independent
image tests use public decoding without test-support; headless image tests use actual windows
and simulated scheduling with NoopTextSystem, not real-font or GPU evidence. Native evidence
comes from the separate Windows consumer above. Decoder frame counts alone establish neither
playback nor cache replay; atlas hits are not view-cache replay. Locked workspace all-target
and independent-consumer all-target/all-feature checks pass on Rust 1.95.0, as do affected
Core/WGPU/Windows correctness Clippy and both root/consumer formatting checks. Actual Windows
Rust 1.92.0 checks pass for those three framework packages and the independently locked
consumer, both all-target/all-feature. Existing import-boundary, public-API-tier, document-link
and release-document scans pass. No minimum-image-version build follows from these gates.
Other platform execution, numeric costs, the separate AVIF dependency-feature
policy, external loop, renderer-lifetime reconciliation, canonical docking and engine adoption
remain open.

## Externally Driven Application Lifetime

This unit starts from `995ac8576362e43f36364616b33cc45c9560fab7` and adopts official
`74798c68d5c63d31e2ccca5c8f5ec0a02c90679c` against fixed target
`635026fabc38440fce5e1767d125026a17b12012`. ApplicationHandle holds one strong AppCell;
to_async returns the existing weak AsyncApp. run_embedded uses the same run_platform path
as normal startup without adding the ordinary Wasm run's permanent retention. No custom
Drop, Clone, cross-thread entry, renderer, window or shutdown owner is introduced.

The fixed target leaves ApplicationHandle's definition/methods unchanged after introduction.
Its later launch path initializes display observation (`f495dd0f77`) and conditional
Subsecond hot reload (`670a779a5e`). The fork retains direct platform display/snapshot queries
and does not import those separate owners or hot-reload dependencies as a side effect of this
entry point. Full display/lifecycle notification and hot reload remain explicit closure work.
Official borrow-group/error diagnostics are also distinct from fork post-borrow convergence.

The deliberate fork adaptation is a complete App update transaction around handle callbacks,
matching AsyncApp::update. Copying official direct callback invocation reproduces both
defer-only effects stranded on return and nested entity updates escaping the host's outer
generation/batch: nextest `8d8a5f17-3cc1-46e9-9c93-0ea54414bc99` fails 0/2. The adapted
entry uses the existing update/unwind/effect owner and releases its AppCell borrow through
existing native callback/shutdown convergence. TestPlatform and VisualTestPlatform now invoke
launch and return instead of panicking; a test-only deferred TestPlatform launch uses its
existing foreground scheduler. VisualTestPlatform still delegates window/backend creation,
and its test host drives simulated tasks and native messages.

The first adapted owner cohort reports 6/8
(`3aff7fba-7925-46f4-93f1-5f2a7e007139`); two new fixtures assumed the wrong existing
lifecycle facts. Inside a host update pending_updates is one, not idle zero. Public nonterminal
shutdown restores the prior quitting state after its fence, as the existing shutdown-barrier
test already proves with a replacement window. The corrected tests require new-window refusal
during quit-observer preparation, completed window/root release and no residual barrier after
shutdown; they do not invent permanent terminal shutdown. Original production behavior remains.

The corrected owner selection passes 10/10 without skips
(`cdb0ce31-a22c-4f7d-bbb9-51bdb0589c52`). It checks synchronous and deferred launch,
native ordinary/returning and embedded owner lifetimes, whole-batch generation and defer-only effects,
reentry panic and subsequent update recovery, queued work after panic without an invented
rollback promise, native wake delivery after host borrow release, async timer updates, weak
AsyncApp release, shutdown observation and completed window/root release. A pending platform
launch callback independently retains its App after the returned handle is dropped; its
eventual one execution and final release are explicit controls. Bounded production review
finds no confirmed P1/P2 in the adapted entry or test-platform changes.
The ordinary-run release comparison is explicitly non-Wasm: ordinary Wasm run deliberately
retains its App, while run_embedded has the separate explicit lifetime. Wasm execution is
not qualified by the Windows cohort.

The first native compile finds test-only access to the private Pixels tuple field; public
f32 conversions retain the exact logical-to-device sampling oracle. The corrected selected
owner/native run passes 11/11 (`ae46cbfd-bf7f-44b3-8958-ddcd14563df5`). The consumer launches
once through Application::run_embedded over returning VisualTestPlatform and the real Windows
backend. Only ApplicationHandle/AsyncApp drive subsequent App work. Four actual model stages
produce matching-window/version accepted, nonempty Submitted, rendered/diagnostic/capture
generations, distinct quad colors and visible native text pixels. Source/run assertions verify
the shaped labels; the pixel oracle establishes visible text, not OCR of each exact label.

Notification and defer counters settle before any submission polling. A background await and
visual 100 ms timer drive the third state, followed by another host update. Busy-App native
input preserves its typed rejection, while frame callbacks wait for borrow release; direct
callback invocation and synthetic SendMessageW input remain test ingress, not OS-input fidelity.
Shutdown observes the exact registered HWND retirement sequence, model/root/window/global-text/
asset/platform release, and one OLE uninitialization. The retained AsyncApp cannot access the
released App. Its observed backend weak reference is the text backend, not proof that every GPU
allocation or native vsync thread has completed release. Bounded consumer review finds no
confirmed P1/P2; all five implementation/research/review subthreads are closed.

Complete all-feature Core/WGPU/Windows libraries plus six existing public text/layout/image
consumers pass 1,339/1,339, with four existing interactive Windows cases skipped
(`0a4a671d-31e4-495b-b3bc-a98008e5e56f`). Affected UI core/components/docking/Canvas
libraries and integrations pass 2,964/2,964 without skips
(`2daff788-2c5b-4501-a55c-6aa1813184ed`). Independently locked public consumers pass 14/14
without skips (`9b2a74d3-9f38-4126-ab02-52fd0bef95b7`); the two new embedded consumers
type-check ownership, generic returns and weak-context signatures without starting an App
or enabling test-support. Locked workspace all-target and independent-consumer all-target/
all-feature checks pass on Rust 1.95.0, as does affected Core/WGPU/Windows correctness Clippy.
Actual Windows Rust 1.92.0 affected-framework and independent-consumer all-target/all-feature
checks pass. Root/consumer formatting and existing import-boundary, public-API-tier,
document-link and release-document scans pass. Both lockfiles remain unchanged. The API does
not make a blocking platform return early. Stock
Windows still owns GetMessageW and process exit; the returning visual-test backend is distinct
from a production external Windows loop. Delayed launch may retain its callback/App after handle
drop, and AsyncApp weakness does not automatically cancel every future. Framework GPU capture
is not compositor display, numeric costs or shared engine rendering evidence. Other native
platforms, complete closure reconciliation, canonical docking, Nara integration and R11/R12
remain open.

## Owning Asset Loads And View Completion

This unit starts from `e4364085634906472d23dab0efe0b9305fab7a04` and reconciles official
`5896274168e06663568f42b5bae142162936e0a6` and its cleanup at fixed target
`635026fabc38440fce5e1767d125026a17b12012`. The official `CachedLoad` owns one background
load and one foreground publication task. A loading entry deduplicates interested entity IDs;
a loaded entry retains its result. App::fetch_asset now returns Option output without
subscribing, Window::use_asset registers each interested view and Window::get_asset remains
non-subscribing. Opaque ImageCacheItem new/get/use_image replaces ImageLoadingTask and the
public Loading/Loaded variants. Retain-all and the actual gallery LRU use that same owner;
existing image-disposal and decoder contracts remain.

The direct official publication body passes the initial multi-window notification and pending
window-load cancellation cases, but the active-App-borrow case panics with `RefCell already
borrowed`: nextest `b6e076ff-4dac-45e4-877e-dcc1d4e34188` reports 2/3. This is a fork-specific
modal-loop integration difference, not proof of the same failure in official GPUI. The adapted
foreground task uses the existing AppCell borrow-release waiters, checks entry identity only
inside the acquired App update and publishes/flushes notifications through the complete
transaction. It holds only weak App/entry references while waiting. Eviction drops the entry's
publication task, including a ready result waiting for a borrow, without retaining a replacement.
The new crate-private AsyncApp helper supplies that concrete App-only update; it creates no
cache, scheduler, native event or mutation owner. App release and terminal quitting stop the
publication rather than panicking or spinning.

The first adapted owner/image selection passes 15/16
(`e3e46165-d0bb-400c-9b9c-246dc8d4ba6d`). Only the new released-App fixture fails before
its release step: BackgroundExecutor::tick delegates to the complete test scheduler and can
already run foreground publication. The fixture now uses TestDispatcher::tick(true) to finish
only background work, retaining its pending-publication, App lifetime and exact result-release
assertions. No production behavior or acceptance assertion is weakened by that correction.

Corrected owner/image tests and the new actual Windows consumer pass 17/17 without skips
(`d263dbbe-b2db-4d35-a94a-14beb7ef755b`). The native consumer uses two real windows over
the returning visual host and one shared held asset. Both views register twice but each receives
one completion notification; both automatically publish accepted/nonempty Submitted image
frames without compensating notify, refresh or draw. Matching generation diagnostics and GPU
readback retain the exact image identity and color. Passive fetch/get, cancellation/replacement
and cached errors use a separate resource. Removing the active cache and atlas keys retains
the accepted native frame through its existing texture lease; both windows then retire through
the actual registered lifecycle and final App/text/platform owners release.

Bounded production review finds no confirmed correctness defects. Its added pressure cases
cover waited-update unwind and terminal shutdown before publication. Bounded consumer review
identifies an oracle gap: entity notification counts alone would miss an accidental global
refresh during passive completion. The native successor snapshots render/paint counts after
initial frame opportunities, processes further native frame opportunities after both passive
success and replacement failure, and requires those counts to remain unchanged. It also
checks active image release after native windows are gone while App is still alive. These
assertions retain the original notification, exact submission, pixels and retirement controls.
The native cancellation/replacement branch is passive; subscribed replacement is covered by
the owner tests, and existing candidate-refusal cohorts retain their distinct scope.

Source inspection also finds the gallery example dormant under core's `autoexamples = false`.
The migrated source is now an explicit `image_gallery` example of `open-gpui-platform`, with
the existing backend dependency and narrowly scoped dev dependencies. This avoids a core-to-
platform dependency cycle and does not reenable unrelated dormant examples. Its actual custom
LRU has capacity-one/two held-load cancellation, replacement and two-window animated-frame
disposal tests. Compile and execute that target explicitly:

```text
cargo nextest run --locked -p open-gpui-platform --all-features --example image_gallery --test-threads=1 --no-fail-fast --no-tests fail
```

An initial expanded compile catches one new fixture reading App's private quitting field. The
terminal-shutdown control instead reads the existing public test-support settlement fact,
preserving encapsulation and its no-publication, zero-notification and exact result-release
assertions. Root lockfile changes only add the gallery's existing dev dependency names to
the platform package; no dependency versions change. Full offline Cargo metadata requests a
missing non-host AccessKit package; locked metadata with `--no-deps` verifies the declared
example target without downloading unrelated platform source. This is not a product-test
failure or non-Windows compilation evidence.

The expanded selection passes 21/22 (`7e6a53d8-035d-43ec-be79-8207ed16164d`): all owner,
unwind/terminal, actual LRU and candidate-refusal cases pass. The strengthened native
no-redraw count detects one extra root render after passive success. Its baseline had not
established the initialization isolation already used by the native cache-dependency fixture.
The successor uses pointer/activation-disabled windows, verifies those actual native facts,
and establishes one explicitly refreshed accepted/nonempty Submitted initialization per window
while every result is still held. Only then does it snapshot passive counts. No notify,
refresh or draw is added after releasing a result, and all no-redraw equality, submission,
pixel, lease and retirement assertions remain. The candidate test separately requires exact
old scene/generation/dependencies on refusal and a notified accepted retry with the same result
identity; its retry is explicit rather than claimed as automatic recovery.

The corrected expanded selection passes 22/22 without skips
(`b99f01c8-d52c-4e2b-a7bc-7941ed30d15e`). This includes the strengthened native passive
no-redraw and pre-App-drop image-release controls, actual gallery LRU tests, candidate refusal/
notified retry and all owning cache/borrow/lifetime cases.

Bounded production and consumer reviews leave no confirmed P1/P2 outstanding; the consumer
review closes its original no-redraw oracle finding after source rereview and the 22/22 run.
All six implementation/review subthreads are closed. Native capture is offscreen renderer
evidence with observed native lifecycle, not compositor display or OS-input qualification;
these windows deliberately disable pointer/activation changes during passive measurements.
The independently locked asset consumers type-check public APIs without launching an App
or enabling test-support. Browser main-thread contention, non-Windows execution, numeric
CPU/allocation/P99/process retention and engine rendering are not established by this unit.

The complete all-feature Core/WGPU/Windows/platform libraries, six public text/layout/image
consumers and actual gallery example pass 1,361/1,361 across 11 binaries, with four existing
interactive Windows cases skipped (`a71a5ca9-380b-44fb-b369-4017115ac51d`). This reruns the
existing text/image, cached replay, input, atlas and native lifecycle consumers against the
final owning asset source rather than borrowing the preceding App unit's results.
Affected UI core/components/docking/Canvas libraries and integrations pass 2,964/2,964 across
25 binaries without skips (`a3b6f799-ee52-4e9c-bf6a-eb718c966486`).
Independently locked public consumers pass 16/16 across eight binaries without skips
(`a6590e6e-e6d5-468a-910e-33da27022346`). The two new asset consumers type-check Option
outputs and opaque new/get/use_image signatures; actual asynchronous/cancellation/native
behavior is established separately above.
Locked workspace all-target and independently locked consumer all-target/all-feature checks
pass on Rust 1.95.0, as does affected Core/WGPU/Windows/platform all-target/all-feature
correctness Clippy. Actual Windows Rust 1.92.0 checks pass for those four framework packages
and the independently locked consumer, both all-target/all-feature. The new gallery target
participates in framework compilation rather than remaining dormant source. Its first MSRV
build compiles the existing native TLS dependency closure serially; no compiler-floor increase
or dependency-version update is required. The independent lockfile remains unchanged.
Root and independent-consumer formatting checks pass, as do existing import-boundary,
document-link, release-document and public-API scans. The maintenance skill's official-baseline
clarification passes its existing validator. Verification uses serial one-job Cargo and Nara's
shared target; no remote publication or CI execution is part of this record. Full fixed-target
closure, canonical docking, Nara adoption and complete R11/R12 remain open.

## Connected Displays And Window Association

This successor starts from `1855a94c5eeb7e0a97e69479232211827efc3942` and adopts official
`f495dd0f77b69e287ed72c1168dc02965f08be7d` as retained by fixed target
`635026fabc38440fce5e1767d125026a17b12012`. App caches connected-display bounds and
refresh intervals, reports added/removed/changed properties and publishes the complete state
before observers. Window tracks its association through the dedicated display callback;
Window/Context observers read the cached interval. Both launch paths reread after platform
connection. The public interfaces follow the official mechanism.

Fork adaptations use the existing immutable PlatformDisplaySnapshot and ordered native App/
Window ingress and the existing SubscriberSet storage. Dedicated association reads retain the
official PlatformWindow::display authority; its event separately refreshes coherent committed
window facts, which continue to own geometry and placement receipts. Bounds updates no longer
consume the separate display association transition. Windows replaces
its message-only platform HWND with a hidden top-level tool window, removes the no-app-window
synchronous scan workaround and routes global broadcasts through the existing topology owner.
Refresh intervals participate in immutable topology publication. Native callbacks execute after
that publication and outside mutable topology borrows. Native creation still validates its exact
display generation.

The focused owner/provider cohort covers the official unknown-destination-before-App-callback
scenario, additions/removals/both change bits, spurious hints, ready complete App state,
displayless windows, subscriptions/entity release, busy-App FIFO, observer reentry/unwind,
closed windows and both initially empty launch paths. The first compile finds one missing trait
import in the extracted App owner and one test importing the public reexport instead of its
module-private Application constructor; both are corrected. The first runtime selection passes
6/16 (`3fae9e37-f4fa-4601-afc6-1cafc9ccdf55`). New provider helpers and adapted official
tests had assumed synchronous callbacks rather than the fork's ordered ingress. They now drain
through the existing TestAppContext convention, while busy-App tests retain their explicit
pending assertions. The dedicated display event also reads and commits one coherent platform
facts snapshot without consuming its separate association transition in bounds handling.

The expanded owner/provider/rate selection passes 19/21
(`7c045f72-b801-4470-82d6-dc7496e8330b`): all 16 App/Window scenarios pass. One new direct
provider call still needs its explicit drain. The other failure reproduces a defect in the
official shared Hz helper: a tiny positive finite rate has an unrepresentable reciprocal, and
Duration::from_secs_f64 panics. Duration::try_from_secs_f64 now returns None for that input while
preserving ordinary/unknown-rate behavior and representable rounded intervals. Those shared
helper tests are cross-platform rather than hidden inside the existing Linux-only layout tests.
An earlier narrow rate filter only reran the no-refresh owner case before that placement was
corrected; it supplied no Hz-helper result. Subsequent qualification is recorded below.

Bounded Core/Windows and foreign-backend production reviews find no confirmed additional P1/P2.
Their suggested two-phase retirement control is now an actual owner regression: an App display
observer closes the original window and opens a replacement before the second phase; the
original sequence settles once as stale/closed and neither window receives that callback.
The Windows compile corrects a new fixture's unsupported WINDOW_STYLE::intersects call to the
existing bit-mask idiom. The expanded selection passes 35/36
(`cfbf5d44-9b1f-421c-9f92-919081c999b5`), including all 22 Core and 12 topology cases.
The native zero-window fixture had sampled diagnostics before draining the deliberately queued
App callback. It now pumps the existing foreground/native loop outside its busy-App portions;
pending/FIFO/retirement assertions inside the active borrow remain unchanged.

The corrected expanded selection passes 36/36 without skips
(`e605a3ad-fb9d-4204-b8c6-5aafe621fae8`). The actual Windows trajectory checks the hidden
top-level platform HWND before creating any app window, exact current-monitor rereads, unchanged
publication generation for spurious broadcasts, one queued App notification, a real nonempty
accepted/Submitted frame, ordered busy-App sequences and exact native window retirement. A
detached Rust association fixture supplies a deterministic unknown ID without changing the OS
monitor; a separate TestAppContext fixture establishes changed properties and cached readiness.
The platform HWND and text/backend/OLE owners release after the final window. Bounded acceptance
review leaves no confirmed P1/P2 or false native-property claim. All seven implementation/review
subthreads are closed at that checkpoint.

The first complete runtime/public/gallery run passes 1,385/1,387 with four existing interactive
Windows skips (`aff9ea23-5a9f-4bcf-bf18-6787a1e6edea`). Existing nested native close and
event-driven geometry wake consumers fail; an unchanged focused rerun reproduces both failures
(`c44bc365-2141-43a8-b40f-0e7fc75b7975`, 0/2). The new official display hint can make a
native close asynchronous before its user callback executes. In the fork, that releases the
platform should-close slot before the user callback sends a nested WM_CLOSE. The nested query
then queues another user callback indefinitely. This is an exposed fork boundary defect, not
evidence against official display observation. The platform closure now reads the existing
WindowShouldCloseHandlerSlot's checked-out state before dispatch; both synchronous and deferred
paths share the same handler owner and unwind/generation cleanup. A Core pressure case requires
one denied nested query, one handler call and a later independent close remaining available.

The geometry fixture's initial one-shot foreground drain discarded posted wakes without
dispatching the new platform notification. It now settles startup using the existing real
message pump before establishing its initial baseline. All subsequent exact coalesced/rearmed
wake, committed latest geometry and no-polling assertions remain. Its helper visibility changes
are confined to the test module. A new bounded production review and corrected runs follow.

The corrected 18 App/Window scenarios, two display native/provider cases and both original
failing native consumers pass 22/22 (`2bae6661-5c87-412a-8e0c-07ec2beeea39`). The bounded
follow-up review finds no confirmed P1/P2 in the shared close-slot or startup-fixture changes.
All eight implementation/review subthreads are now closed. Complete consumer qualification
reruns against this owning correction rather than reusing the earlier partial full result.

The corrected complete all-feature Core/WGPU/Windows/platform libraries, six public text/layout/
image consumers and actual gallery example pass 1,388/1,388 across 11 binaries, with four
existing interactive Windows cases skipped (`722129ab-e723-40a9-a7a3-c6ff451f6239`). This
includes the new close-slot regression and reruns existing native input, cache, atlas, image,
font, geometry and retirement consumers against the final correction.

Affected UI core/components/docking/Canvas libraries and integrations pass 2,964/2,964 across
25 binaries without skips (`865d1baa-d538-4a8b-8256-e59f8deb0a48`). The independently
locked consumer then finds a manifest prerequisite hidden by the root feature unions: fork
Core still made bitflags optional under Wayland, while public DisplayChanges now needs it
unconditionally. The fixed target declares it as a normal dependency. Core now does the same,
retiring the implicit optional bitflags feature and its Wayland activation. This changes neither
resolved version nor the already-enabled dependency union of the completed runtime/component
cohorts. The independent consumer is rerun without enabling test-support; it must compile through
the ordinary dependency boundary rather than add a consumer feature workaround.

The corrected independently locked public consumers pass 18/18 across nine binaries without
skips (`75a75c4b-33f4-4fca-b03a-9d0dc1e4c378`). Both lockfiles and all dependency versions
remain unchanged. The two display consumers type-check owned App/Window/Context callbacks
without test-support and execute normal/fractional/invalid/unrepresentable Hz controls; they
do not launch an App or qualify OS display changes.

Locked workspace all-target and independently locked consumer all-target/all-feature checks
pass on Rust 1.95.0, as does affected Core/WGPU/Windows/platform all-target/all-feature
correctness Clippy. Actual Windows Rust 1.92.0 checks pass for those four framework packages
and the independent consumer, both all-target/all-feature. Existing warnings in legacy docking
and test support remain outside this unit; no warning-free-workspace claim follows. The
mandatory bitflags declaration does not increase the compiler floor or change either lockfile.

Root and independently locked formatting checks pass. Existing import-boundary, document-link,
release-document and public-API scans pass through the built xtask without another Cargo process.
All Cargo work is serial, one job, and reuses Nara's target; no clean or remote publication occurs.

The installed wasm32-unknown-unknown target supplies an additional compile-only check. Web's
test-target check fails in unchanged test-support dependency wait-timeout 0.2.1 because that
target has no imp module; it never reaches the new backend implementation. The pre-unit
manifests already enabled Core test-support/proptest through Web dev dependencies. The
separate production-library check then passes:

```text
cargo +1.95.0 check --locked -p open-gpui-web --target wasm32-unknown-unknown --lib
```

This qualifies production Core/WGPU/Web compilation, not browser execution or test-support.
The remaining dependency/platform audit must reconcile the fork's proptest 1.10.0 pin with
the fixed target's 1.12.0 and retain appropriate Windows/native test capabilities; a version
number alone does not establish a fix for the Web test dependency closure. No dependency
version is changed by this experiment. Native macOS/Linux and physical hotplug/refresh-mode
qualification remain open, alongside numeric costs and full framework/engine adoption.

Synthetic WM_DISPLAYCHANGE exercises real Windows ingress without changing the user's monitor
configuration. It cannot establish physical hotplug, changed OS refresh modes or frame pacing.
Non-Windows source migration, native platform execution and numerical costs are separate claims.
Official refresh-aware profiler/reporting successor `4f6d97b9de7d9f167da8b7832eeecea22c5a9efa`
remains queued; the display getter alone does not adopt it. Complete fixed-target reconciliation,
canonical docking, Nara adaptation and full R11/R12 remain open.

## Unified Profiler Foundation

The qualified local implementation is `3e5a907775e9f22d0a092a0f9f27c6f1696ade0e`, without
remote publication. The existing plan and candidate queue retain its fixed upstream target and
the remaining dependency-ordered work.

This prerequisite starts from `e5ad73a24a69274378264f792f4efb4d6d892ac6`, targeting the
official unified owner `a21007b7a948e46afbe719150f5e9968bfcd1078` together with profiler
feature gating `48511e0b9c` and last-reference collection safety `c16d19c94d`. The fixed
refresh target remains `635026fabc38440fce5e1767d125026a17b12012`. Scheduler action clocks,
draw-duration returns and dirty-to-present histograms use the corresponding final-source
follow-ups. This record does not claim the absent foreground journal, hang detector/monitor,
power/visibility filtering, native frame-signal timing or refresh-aware version-3 reports.

The official profiler feature gates task/action measurement and the single per-window
input/draw/presentation owner. Aggregate histograms remain active while the feature is enabled;
runtime tracing controls retained detailed records. Reference-counted internal scopes preserve
overlapping measurements and manual settings. Collection upgrades thread owners under the global
registry lock, then releases that lock before copying, locking or dropping those owners. The
existing stable cold marker preserves the qualified compiler floor. Trace cursors remain monotone
when buffers clear so independent collectors can continue across disable/re-enable.

Fork integration replaces InputLatencyTracker rather than creating another owner. Its actual
Submitted/Deferred/RepaintRequired/Rejected counts, pending input and exact refresh-callsite rows
move into WindowProfiler. Only accepted draws enter frame measurements; only actual Submitted
outcomes complete their covered input. Input arriving after an accepted draw remains pending
when that older scene is retried. Draw refusal/unwind restores the earlier pending measurement
and first invalidation; an accepted-generation unwind cannot publish an older scene's timing.
Platform-call start/end use the same scheduler clock and do not establish scanout.

The existing Criterion context and Canvas consumer retain their setup/update/teardown API.
Scoped tracing wraps the measured loop and collects once afterward, excluding setup and avoiding
per-iteration trace snapshot allocation. This adapter is distinct from the queued complete
upstream benchmark metrics/report evolution. Independent public consumers compile without
test-support. Instrumentation-disabled builds have separate no-collection checks.

The first focused compile succeeds. Its 18/28 result
(`f2e750e3-67f8-440d-9c63-021998c30134`) passes task/trace/lock/scope and actual Criterion
owner cases. All ten new Window cases stop at their common fixture assertion: TestAppContext's
input simulator drains scheduled work before the fixture's explicit draw/present controls. The
fixture next invokes production Window::dispatch_event without draining the scheduler. The
second result is 20/30 after including action regressions: the same ten Window cases still
stop at their common dirty assertion. App's test-dispatcher effect flush also draws dirty
windows when an update ends. The fixture now holds one real outer App update batch across
its manual input/draw/present sequence, preserving actual input handlers, dirty state,
accepted generations, renderer outcomes and sample assertions. It does not forge receipts or
reset candidate state.

That correction reaches 30/33 (`87828d4e-086b-4b7b-a5bb-767a7b4755d2`). The new reset
regression reproduces input metadata being restored from an older draw checkpoint after reset.
Actual Render unwind also reproduces retained fork draw-stack state blocking recovery. The
remaining refresh assertion omitted Div's unconditional active-state MouseDown refresh in
addition to the authored handler refresh. The Window repair shares the
existing refused-candidate rollback and restores the interrupted root drawing context; separate
feature-on/off cases exercise Render, prepaint and paint failures through a later real Submitted
test-platform frame. Further qualification follows against the owning corrections.

The next source checkpoint passes 32/36 (`3e6e52c2-6c7d-40c0-8459-92a20fb52eb5`). The
trace lifecycle matrix and reset rollback fix pass. Expanded acceptance reproduces stale
accepted-generation facts after a publication callback panics and omitted input completion
when keyboard modality dispatch draws synchronously before the input handler returns. The
new ungated draw fixture initially leases its own root through WindowHandle::update; it now
uses App::update_window so the actual draw can independently borrow/render that root. The
keyboard-only focus-visible recipe is confined to its concrete test.

The owning successor tracks the invalidation count at an accepted draw's start. An in-flight
input joins that pending draw only if its invalidations were covered and none occurred later;
later handler work remains pending for a newer draw. Reset's existing owner generation also
invalidates an in-flight pre-reset input. No extra scene identity is introduced. Root unwind
invalidates state already consumed by a by-value element callback rather than claiming it can
be restored after destruction. Ordinary atlas refusal retains strict state-transfer assertions.
Accepted publications settle subsequent accepted outputs and Window completion even after a
callback fails, preserve the new frame and presentation facts, then propagate the first panic.
The added context restoration and cleanup paths have functional rather than numeric cost scope.

The subsequent 35/37 checkpoint (`686c5374-ac36-4dce-af6a-39ed06ae95a6`) passes trace/reset,
actual root unwind/recovery, accepted-publication continuation and both synchronous keyboard
cases. The ungated color oracle omitted the authored 0.5 opacity; it now checks both color and
opacity. The refresh oracle initially attributes the automatic call to input modality. Moving
the final baseline inside the held update preserves exact control but does not remove that call:
36/37 (`c3c390b7-cc5c-4f17-bbec-fc9f694b5f9c`) still sees two refreshes. Following Div's
actual MouseDown listener identifies its unconditional active-state refresh. The final oracle
requires that exact non-coalesced Div call and the authored handler's coalesced call, two distinct
source rows and one completed input. The mouse-mode baseline does not change input modality.

The corrected focused gate passes 37/37 (`7bf3e09d-8409-4713-a811-586d50576cd2`), with
1,091 unrelated cases filtered out. Independent final production/acceptance review confirms
no remaining P1/P2 in the scoped source, including the native Draw/Present correlation. This
is not yet the complete runtime/native/consumer qualification. Trace lifecycle regression has
green execution and a source-confirmed race; only reset and the recorded Window defects have
executed red baselines. No trace-race red result is claimed.

The first native compile succeeds. Its executed trajectory (`cc58211f-245f-4753-8294-b1ab06e57c1d`)
stops at a two-input fixture expectation: MouseDown, MouseUp and KeyDown all invalidate this
stateful consumer. In particular, Div's capture-phase MouseUp listener clears the active state.
The corrected prefix oracle requires exactly three pending synthetic events. Its next execution
(`4d996539-6d78-4744-b62a-ac9435de4078`) reaches a later histogram with four covered inputs:
the actual message pump can deliver additional native pointer work before presentation. The final
native paint receipt therefore records the real pending batch at that candidate. The histogram
must match that exact batch, include at least the three already verified synthetic events, and
clear pending input on submission. The one histogram sample, authored pointer/key source, actual
accepted generation, nonempty Submitted receipt, timing bounds and teardown assertions remain.

That correction reaches the no-input redraw control in
`8b63b909-4ae4-427f-ac84-353001750954`. Post-submission native pointer work from the preceding
synthetic input can still complete another sample. The successor settles actual pending input
and frame measurements through the existing delayed message pump before taking the no-input
baseline. The controlled redraw must then leave both input histograms unchanged. It does not
clear pending work or fabricate an input-free frame; follow-up native work is outside that
isolated control and retained separately from the checked source submission.

The corrected native trajectory passes 1/1 (`7f8879f5-813b-4f53-a523-383d5fae16b7`), with
1,338 unrelated cases filtered out. It covers tracing disabled/enabled, actual synthetic-prefix
input, accepted/nonempty Submitted receipt correlation, no-input redraw after native follow-up
settles, exact window retirement, reconstruction and final HWND/App/backend/OLE release. This
does not establish OS input injection, scanout, physical display changes or numeric overhead.

The complete instrumentation-disabled Core library passes 1,095/1,095
(`939d12e3-591b-4569-865c-4ca973c73096`), without skips, using no defaults and the fork's
existing `test-support` feature. An initial command selects the upstream `bench-support` name,
which this retained Criterion consumer does not expose; it fails before compilation. The
corrected command does not add a feature alias or claim the queued complete upstream benchmark
API/report migration.

The independent lock update adds only hdrhistogram 7.6.0 and its crossbeam-channel 0.5.17
dependency; existing selections remain intact, distinct from root hdrhistogram 7.5.4. Its
all-target/all-feature check and 20/20 enabled public tests
(`a70993f7-9074-4ab4-8de1-019da2a62df6`) pass. Disabled public consumers pass 18/18
(`be7f02ed-b6ef-4886-b80a-e923f3246a81`), without skips. The private benchmark guard follows
the retained test-support/test boundary; profiler-only ordinary builds do not compile an unused
scope API. The profiler-only action-name lookup/import also follows its actual feature owner.
The four new Windows CI steps
separately execute enabled/disabled Core contracts and the ordinary public profiler consumer.
The workflow YAML parses locally; remote CI has not run.

The first complete enabled framework/public/gallery checkpoint passes 1,425/1,425
(`5ab25e94-1bed-48a9-90cc-0500fdae8b9e`), with the same four preexisting interactive Windows
skips. A final action-statistics review then identifies another defect shared by the starting
fork and fixed official target: a partial slow-action history raises its cutoff to the first
peak before filling its five slots. Descending qualifying samples retain only the first event.
The public ActionStatistics regression reproduces this in
`28537b96-e0a9-4d58-b228-15e4cc76444b` (2/3 pass). The successor keeps the original 100-microsecond
floor until the history is full, then uses its actual shortest retained runtime. It preserves
the separate full-history shortest-replacement regression. The pending component invocation is
interrupted during compilation before execution so final qualification can use this correction;
no component result is claimed from the interrupted invocation.

The final corrected framework/public/gallery gate passes 1,426/1,426
(`60f0b918-77a8-4113-9d94-1c003ac232fe`), including the new partial-history control and actual
native profiler trajectory. Four preexisting interactive Windows cases remain skipped.
The bounded final-delta review confirms the inherited action defect, take/reset behavior,
feature boundaries, native measurement scopes and four direct CI commands without another
confirmed P1/P2. All nine implementation/review subthreads are closed. Remaining broad consumer,
benchmark, compiler-floor and static checks are recorded below when complete.

The all-feature UI core/components/docking/Canvas cohort passes 2,966/2,966 across 25 binaries
(`cd2ae25d-e447-4748-917a-3b018ba55ca5`), without skips. The selected real Criterion consumer
`paint_frame_render_labels` then exposes inherited setup cost: its label conversion issues
20,000 UpdateNode commands, each cloning and validating the full document. The active debug
smoke is interrupted in that preparation after more than eight minutes; it yields no benchmark
result or numeric overhead comparison.

The fixture successor changes only preparation outside `b.iter`: update kind values in the
complete public snapshot, then reconstruct through CanvasDocument::from_snapshot. Full
20,000-node/19,900-edge counts and every label kind are asserted. Shapes, relations, metadata,
geometry, viewport, clipping and the measured render loop remain represented by that same
snapshot and public admission owner. A tenth independent review finds no P1/P2 in this bounded
fixture change; all ten subthreads are closed. This does not optimize runtime bulk mutation.
Both real Criterion test-mode invocations pass:

```text
cargo +1.95.0 test --locked -p open-gpui-canvas --bench large_canvas --features profiler -- paint_frame_render_labels --test
cargo +1.95.0 test --locked -p open-gpui-canvas --bench large_canvas --no-default-features -- paint_frame_render_labels --test
```

These are full-fixture execution controls in the debug test profile, not release measurements,
profiler overhead numbers or the queued upstream benchmark/report implementation.

Final feature-scoped checks against the corrected source pass: disabled Core 6/6
(`df4c4500-1442-4e95-9540-ebeed8b1767c`), enabled independent profiler 2/2
(`8337eccb-f660-4c24-b63a-dbfa6a91c239`) and disabled independent profiler 1/1
(`256eaf4f-3085-4caf-87b1-4b1e3cffecf6`). The earlier complete disabled/public cohorts retain
their recorded scope. Locked workspace all-target and independent all-target/all-feature
checks pass on Rust 1.95.0, as does correctness Clippy across Core/WGPU/Windows/Canvas with
all targets/features. This is a correctness gate, not a claim that existing non-fatal repository
warnings have been removed. Root and independent formatting checks pass.

Actual Windows Rust 1.92.0 all-target/all-feature checks pass for Core/WGPU/Windows/Canvas and
the independently locked consumer, including its separate hdrhistogram resolution. The existing
compiler-floor CI job now includes Canvas. Core/WGPU/Web production libraries compile for
wasm32-unknown-unknown both without and with the optional profiler. These compile results
establish neither native foreign-platform execution nor browser/test-support qualification;
the unchanged Web test-support dependency failure recorded by the display successor remains open.

Final import-boundary, public-document-link, release-document and public-API scans pass, together
with Git whitespace and workflow YAML checks. This closes the scoped profiler foundation;
multi-subscriber wake, sleep/window visibility, journal/hang detection/monitor, first frame-signal
provenance, refresh-aware measurements, power/cadence and complete upstream benchmark/debug
overlay dispositions remain queued. Complete fixed-target parity, foreign-platform execution,
numeric costs, canonical docking, Nara GPU/View adaptation and Editor/game selection remain open.

## Sleep, Wake And Presentation Visibility

This successor starts after profiler implementation `3e5a907775e9f22d0a092a0f9f27c6f1696ade0e`
and qualification `a536b1d93380cc9247a7cc9576ac6b81cdb65489`. It adopts official
`7a7c3e1d2f03195c5fa19bc890da330ad7f3abef` and
`3db02c2c77659ad29810594e6249287ea1e791f7`, using the final source at fixed target
`635026fabc38440fce5e1767d125026a17b12012`. App owns multiple sleep/wake subscriptions;
Window owns cached presentation visibility and Window/Context observers. Mandatory platform
signals retain their documented backend limits. Native shown/mapped state and accepted/Submitted
authority remain separate.

Fork integration keeps thread-safe native receipt and ordered App/Window post-borrow delivery.
Already emitted lifecycle boundaries are not coalesced. The obsolete Application singleton wake
slot is removed; its old singleton replacement test moves to the still-supported reopen handler,
while new sleep/wake cases require independent subscriptions and ordinary drop/detach behavior.
Visibility observation adds no implicit frame wake or telemetry/presentation authority.

Local implementation `8b283cb151b070d7d5851a531a86e6d6d67196cb` is qualified within the
scope below. Synthetic native power messages do not suspend the machine; source migration and
portable compilation cannot qualify foreign native power/occlusion or browser behavior. The
chronological failures and corrections below retain their original checkpoints.

The first all-feature Core production-library check passes. Initial focused test compilation
stops at four new fixture accesses to TestAppContext's private platform field; they now use its
existing accessor. Independent App/ingress review also catches two omitted official public test
entry points. TestAppContext::simulate_window_visibility_change and
VisualTestContext::simulate_visibility_change now forward through the existing simulation and
queue-drain convention. A separate public integration executes both interfaces and observer
lifetimes without core-private access. Bounded re-review closes that omission.

An initial headless instruction incorrectly assumes Visible. Exact official-source comparison
identifies Hidden in both introduction and fixed target; the final source and assertion follow
Hidden, preserving the existing shown/mapped false query. This is a corrected planning mistake,
not a retained fork behavior or an executed platform regression result.

The next test compile rejects a fixture capturing a borrowed TestPlatform inside a static
observer. It now captures the existing owned cross-thread ingress handle for that reentrant
notification. The first executed Core/public/native selection reaches 30/31
(`dece21c7-ffe8-4ddb-a1d5-589b71961761`), with 1,329 unrelated cases filtered out. App lifecycle,
public simulation, visibility lifetime/order and actual native power notifications pass. Native
visibility reaches a real minimize transition but Window's `is_minimized` query still returns
false. Subsequent source diagnosis finds the fixture created a permanently hidden window and
manually showed it without completing the normal initial-presentation owner. Pending initial
placement correctly projects its intended state. WM_SIZE already notifies WindowStateChanged
before its minimized early return. The corrected fixture uses normal first presentation and
registers its observer while the root is still hidden; minimize/restore assertions remain intact.
No WM_SIZE state-publication defect or visibility-getter facts workaround is claimed.

Independent Windows review additionally identifies fork-specific SetWindowPos show/hide commits
that need visibility sampling through WM_WINDOWPOSCHANGED. Its source path already documents
that WM_SHOWWINDOW/WM_SIZE are not guaranteed. The native successor adds that actual entry point.
Restore acceptance also needs a generation lower bound captured immediately before its explicit
refresh, so a submission from before minimization cannot satisfy the control. These findings
retain the official deferred native sampling mechanism and the fork's exact presentation custody.
Foreign-backend source review finds no confirmed P1/P2; it establishes no foreign-platform runtime
qualification.

The Windows wrapper exposes `contains` rather than bitflags' `intersects`; the first correction
compile rejects the latter. The owner now checks the two flags explicitly. Corrected native
power/visibility passes 2/2 (`78e95916-ba27-4185-89d0-d66af8fb3268`), with 1,356 unrelated
cases filtered out. It executes actual initial SetWindowPos reveal, explicit SetWindowPos
hide/show, ShowWindow minimize/restore, unchanged hints, busy-App order, exact retirement and
reconstruction. Restore requires a strictly newer nonempty Submitted receipt. Synthetic power
messages enter the real hidden platform HWND, with shared registration, multiple subscribers,
unsubscribe and exact shutdown release checks; no machine power state changes.

The complete profiling-disabled Core library and public lifecycle integration pass 1,113/1,113
(`602fd512-6845-49e5-ae30-f21c38530f81`), without skips. Core/WGPU/Web production libraries
compile for wasm32-unknown-unknown with profiling both off and on. This is compile evidence,
separate from browser event execution and the unchanged Web test-support dependency limitation.

Independent final source review confirms the complete App/Window/Context/native integration,
including the SetWindowPos adaptation and strict restore receipt, without an open P1/P2.
All nine implementation/review subthreads are closed. No numerical cost or native foreign-platform
result follows from the source review.

The complete enabled framework/public/gallery/native cohort passes 1,446/1,446
(`937a6285-4dc2-4bd2-9c8a-f0501087a2cd`), with four preexisting interactive Windows cases
skipped. It includes both public lifecycle integrations and 209 executed Windows cases. The
all-feature UI core/components/docking/Canvas cohort passes 2,966/2,966
(`8ff746a3-2341-43c3-b4da-f9cb7853e799`) across 25 binaries, without skips. Ordinary-consumer
all-feature tests pass 28/28 (`8e307102-6d48-40b0-ba6d-d1eb2da83abc`); the no-default-features
configuration passes 26/26 (`a528ca0d-c831-4f19-886c-3fbc4e491533`), both without skips. Their
independent lockfile and manifests are unchanged. The eight lifecycle tests distinguish ordinary
API compilation from standalone enum/subscription execution; actual App/Window delivery is
covered by the separate framework and public test-context consumers.

Locked workspace all-target and independent-consumer all-target/all-feature checks pass on Rust
1.95.0. Correctness Clippy passes for Core/WGPU/Windows/Canvas/Web all targets and features;
existing warnings are not a warning-free qualification. Actual Windows Rust 1.92.0 affected
Core/WGPU/Windows/Canvas and independently locked consumer checks pass, both all targets and
features. Root and independent formatting checks pass against the committed runtime source:

```text
cargo +1.95.0 check --locked --workspace --all-targets
cargo +1.95.0 check --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all-targets --all-features
cargo +1.95.0 clippy --locked -p open-gpui -p open-gpui-wgpu -p open-gpui-windows -p open-gpui-canvas -p open-gpui-web --all-targets --all-features -- -D clippy::correctness
cargo +1.92.0 check --locked -p open-gpui -p open-gpui-wgpu -p open-gpui-windows -p open-gpui-canvas --all-features --all-targets
cargo +1.92.0 check --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all-targets --all-features
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 fmt --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all -- --check
```

All Cargo runs are serial with one build job and the existing Nara target. Existing import,
public-API, document-link and release-document scans pass. Workflow YAML parses, and its lifecycle
public-test and independent feature commands retain the intended targets and serial settings.
No remote CI or publication is claimed. This closes the lifecycle observation prerequisite;
foreground journal/hang detection/monitor, frame-signal provenance, refresh-aware measurement,
power/cadence, complete benchmark/report/debug-overlay reconciliation, hot reload and remaining
fixed-target differences stay open. Native macOS/Linux, browser/test-support, physical suspend,
general Windows occlusion, numerical costs, canonical docking U4-U9 and Nara rendering/Editor
adoption retain their separate unqualified scopes.

## Foreground Journal And Hang Monitor

This successor starts after lifecycle implementation `8b283cb151b070d7d5851a531a86e6d6d67196cb`
and qualification `8554f9e765b4fe3369480fcba2d0580ec008bd18`. The fixed target remains
`635026fabc38440fce5e1767d125026a17b12012`. The complete official journal/detector/monitor
checkpoint is `d284cbbc87b55d659303106ced627c8612369827`: introduction `1861e58f98`, journal
consumer support `7316cf7745`, trigger rules `97b1e64a17`, lifecycle filtering `f0fb48c71a`,
unnotified suspend `7232ba04ef`, monitor `0f9c923e67` and corrections `4c0ead4a3e`/`d284cbbc87`.
Its measurement-v2 mechanism is distinct from the queued final-target signal/skip and
refresh-aware measurement-v3 successors.

Local implementation `8493e9eb1cad7c3336fa99b4b8749c5be1d6c7c5` is qualified within the
complete scope and limits below. It is a local commit without publication.

The official bounded ring, pending publisher, independent collectors, loss/discontinuity markers,
short-poll aggregation, explicit boundaries, frame-demand deadline, suspend clocks and detector
rules are imported through their existing profiler owners. All 45 official test entry points
remain; source preservation alone is not an executed result. App installs the foreground-thread
stream and optionally owns a monitor whose flush joins the existing bounded quit work. Actual
Window input, invalidation, visibility, accepted drawing and native submission feed that stream.
Ordinary consumers require no Nara document, ECS, history or private Host dependency.

Fork integration retains accepted/refused/unwinding candidates, exact Submitted correlation,
pending input reset and old-scene retries. Scoped task/action and Window activity guards close
the official bare begin/end unwind limitation while restoring enclosing task/action state. Native
wrappers and Web use the same owner. Official inherited foreground-idle APIs and Web idle
callbacks are reconciled; deterministic schedulers remain excluded from native runnable counting.
Current dirty demand is re-registered after a submission when newer dirty/presentation work
remains. No new render, scheduling, shutdown or telemetry owner is introduced.

The first locked production check stops before compilation because the root lockfile is missing
the Web idle helper's existing dependency edge. Core's target-specific libc edge and Web's
existing core-util helper edge are now explicit in the relevant lockfiles, with no package version
updates. The corrected all-feature Core production-library check passes in 38.43 seconds. Focused
execution, independent review and complete consumer/platform qualification remain in progress.
Results and corrections below must keep compilation, test-platform, native Windows and unexecuted
foreign/browser/physical-power/numerical-cost scope separate.

The first focused run executes 128 tests: 124 pass and four new public consumers fail. The
official module scenarios, profiler RAII/candidate tests, executor controls and existing lifecycle
tests pass at that checkpoint. The public fixture assumes native simulated input leaves drawing
pending, but the native input bridge can draw synchronously. It also constructs an inactive window,
so repeated immediate forced callbacks can enter inactive-frame throttling. The corrected public
fixture uses actual activation and direct public Window input for its independently controlled
input/draw/presentation phases. It retains the original event-order, collector and detector
assertions; corrected execution is still pending.

Independent review also identifies two fork integration gaps: re-registering failed presentation
retries can renew the same demand's deadline, and canceling focus-only work can leave stale journal
demand. Both need real Window controls and an owning correction. A separate inherited monitor
configuration issue admits zero or unrepresentable polling intervals; public admission now returns
InvalidInterval before spawning or occupying the App slot. These changes require their subsequent
tests and review closure, not just source confirmation.

The official publisher can retain a tail entry while a reader owns its target slot; it retries on
the next foreground publication. Monitor Flush polls finalized entries and waits for that callback,
without guaranteeing that every offered publisher entry has finalized. Public API documentation
keeps this limit explicit. The review's hypothetical contended-tail scenario was not executed and
is not an added complete-drain guarantee or a claim of lossless shutdown logging.

The corrected fixture first needs its AppContext trait import restored after switching away from
typed entity updates; drawing while a typed root update is checked out attempts to borrow that
same root again. A subsequent compile stops at the newly coordinated unqueued-task guard API
before that API lands. Neither compile failure is a behavior regression result. The final red
selection executes 15 tests (`2dcf5a91-9f4b-4a49-8ddf-f31afddebb7f`): 12 pass, and the
three intended demand regressions fail at their actual behavioral assertions. All seven public
consumers, both foreground queue controls and the three demand counter-controls pass.

The owning corrections preserve failed-attempt deadlines, clear only genuinely canceled
focus-only diagnostic demand and retain actual failed presentation work without successful
presentation boundaries or demand consumption. Journal writer/sealer/property controls and
Window/RAII cases cover those distinctions. The corrected focused cohort passes 144/144
(`edc3bbf2-715a-4750-8b23-4f1867bbea99`), with 1,086 unrelated tests filtered out. The Web
single-thread background/timer paths also need explicit unqueued task custody: such work still
records its full poll but must not consume another foreground task's queue count. The real
scheduler two-queued-task control passes, together with nested unqueued unwind recovery.

The first native selection passes power and visibility but fails the existing profiler trace at
the reconstructed window's initial draw (`a6cef8ee-375f-4596-92f8-bcb1b38a308e`, 2/3).
A diagnostic-only rerun (`34e05480-8ba4-4a43-a6c1-ee7849e3024b`, 0/1) confirms the exact
reconstruction callsite. That initial draw occurs while hidden, and the newly adopted official
visibility rules intentionally discard its dirty timestamp. Reconstruction now retains exact
initial Submitted/lifetime assertions and additionally traces an explicit visible redraw with
the complete dirty/draw/present timestamp oracle. Corrected native execution remains pending.

Final scheduler review identifies an inherited counting gap when a late foreground wake reaches
a closed native queue. Both ordinary and idle regressions use the real PlatformScheduler and
queue: a detached local task polls Pending, the receiver closes, a worker wakes it, and native-style
rejection forgets the !Send future without polling or destroying it on the worker. A new executor
on the same thread must still reach Idle. Both fail at that exact final assertion
(`dae7c78d-6e04-4d4c-9207-71ba16f49a08`, 0/2).

The existing dispatcher interface now needs explicit admission: only synchronous refusal settles
the scheduler's captured originating counter; a failed wake after successful insertion remains
admitted. Web mailbox-to-browser registration also retains its later rejection settlement on the
foreground thread. This is a fork library-lifetime correction to the official counting mechanism,
not a new execution queue. Accepted but never-polled backlog and complete platform queue
retirement remain separate from immediate refusal; the existing queue/native finalization does
not yet establish a complete drain guarantee. Those limits must remain visible after the
confirmed refusal regression is corrected.

The final scoped admission/RAII/demand/public selection passes 146/146
(`0f2f568a-3602-45c2-9864-bdc74d74f15f`), with 1,086 unrelated cases filtered out. Both
closed-queue red controls now pass without changing their assertions. Corrected native
profiler/power/visibility passes 3/3 (`28a22b05-4722-4f13-aabc-c0b0fff7afeb`), with 210
unrelated cases filtered out. The actual Submitted trajectory observes independent journal
cursors, precise input kinds, original paint/submission timestamps, visible reconstructed redraw,
shutdown-return callback completion, background thread identity and callback-capture release.
Core/WGPU/Web production libraries compile for wasm32-unknown-unknown with profiling off and on.
The added browser callback admission tests are source-only, not browser execution or a resolution
of the existing Web test-support dependency gap.

Final independent correction review leaves no confirmed open P1/P2. All ten implementation/review
subthreads are closed. Broad runtime/component/independent and final static/compiler-floor
qualification continue against frozen source. The turn crossed into November 7 local time;
earlier chronological results above retain their November 6 checkpoints.

The complete all-feature framework/public/gallery/native cohort passes 1,531/1,531
(`61f4dcc2-8233-43bb-92b4-cd1d4c4b3203`) across 13 binaries, with four preexisting interactive
Windows cases skipped. It includes all 45 original module scenarios, the strengthened writer
model, closed-queue/unqueued and Window demand controls, seven public journal consumers and
the actual native Submitted/monitor/reconstruction trajectory. Component/docking/Canvas,
independent feature configurations and final static/compiler-floor checks remain in progress.

The all-feature UI core/components/docking/Canvas cohort then passes 2,966/2,966
(`f37f6edc-426d-4f04-88c9-9d0cd8176299`) across 25 binaries, without skips. The next
independent-consumer step stops in locked metadata because its separate lockfile also needs the
Web helper's core-util edge. Adding that already resolved package reference changes no versions
or production source. Independent enabled/disabled and Core feature-off verification continue;
the successful runtime/component cohorts do not need repetition for that lock-only correction.

The independently locked all-feature consumer passes 32/32
(`1942c3b8-e139-47a5-b451-aa1ef1629997`); its no-default-features configuration passes 26/26
(`faecbc45-bc9a-4131-88a8-301ea8d650cc`), both without skips. The complete profiling-disabled
Core and public lifecycle cohort passes 1,114/1,114
(`7711f140-2ce9-4bc8-a1be-7bc1afdaa778`), without skips. The new disabled guard case confirms
that normal/unqueued task scopes remain zero-sized and do not register timing buffers, including
unwind. Package versions, sources and checksums are unchanged in both lockfiles; only existing
Core libc and Web core-util edges are added. Final locked compilation, correctness Clippy,
Windows compiler-floor and formatting checks continue against the same production source.

Locked workspace/independent compilation, affected Core/WGPU/Windows/Canvas/Web correctness
Clippy and both formatting checks now pass on Rust 1.95.0. Actual Windows Rust 1.92.0 affected
framework/Canvas and independently locked consumer all-target/all-feature checks pass. These
commands retain their distinct scopes:

```text
cargo +1.95.0 check --locked --workspace --all-targets
cargo +1.95.0 check --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all-targets --all-features
cargo +1.95.0 clippy --locked -p open-gpui -p open-gpui-wgpu -p open-gpui-windows -p open-gpui-canvas -p open-gpui-web --all-targets --all-features -- -D clippy::correctness
cargo +1.92.0 check --locked -p open-gpui -p open-gpui-wgpu -p open-gpui-windows -p open-gpui-canvas --all-features --all-targets
cargo +1.92.0 check --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all-targets --all-features
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 fmt --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all -- --check
```

An additional stable wasm `multithreaded` check stops in the unchanged wasm_thread 0.3.3
dependency's nightly-only `stdarch_wasm_atomic_wait` feature. The existing `hello_web` toolchain
and shared-memory/build-std configuration already require nightly. A separate production check
uses that exact configuration with the installed `rustc 1.98.0-nightly (57d06900f 2026-05-27)`;
it passes in 3m 24s. This compiles the multithreaded mailbox branch with its actual shared-memory
dependencies and does not change the normal compiler floor or imply browser execution:

```text
cargo +nightly --config crates/gpui_web/examples/hello_web/.cargo/config.toml check --locked -p open-gpui -p open-gpui-wgpu -p open-gpui-web --no-default-features --features open-gpui/profiler,open-gpui-web/multithreaded --target wasm32-unknown-unknown --lib
```

Existing import/document/release/public-API scans, workflow YAML/serial command checks, final
formatting and Git whitespace checks pass. No hosted CI or remote publication is claimed.
Foreground journal/detector/monitor and the restored idle prerequisite are complete within this
checkpoint's qualification. Platform frame-signal/completed-skip, refresh-aware v3, power/cadence,
complete benchmark/report/debug-overlay comparison, hot reload and remaining fixed-target
differences remain queued. Native macOS/Linux, browser test execution, physical suspend,
numerical costs, accepted-backlog retirement, complete publisher-tail drain, canonical docking
U4-U9 and Nara rendering/Editor/game adoption retain their own unqualified scopes. This does not
complete the fixed-target refresh or Nara R11/R12.

## Platform Frame Signals And Completed Skips

This successor starts at journal qualification
`84407f27ffacac0f2f5945c61220eb7567e7ac54` and adopts official
`cd4fc8de4ca8548cca2567352b87bcaaec13328f` with the nonempty-skip correction
`36b6d0951f`. The fixed target remains
`635026fabc38440fce5e1767d125026a17b12012`. Measurement v3, final-target power/cadence
and the later macOS MetalFrameSource retain separate owners.

The public options and shared atomic accumulator preserve the official captured time/source
pair. Disabled capture does not evaluate its closure; disabled accumulators have no timestamp
storage or atomic work. Journal raw signal/reentrant markers retain callback arrival metadata
without contributing work or sealing intervals. Completed no-render/inactive/thermal skips
seal only retained work, retain pending demand and do not flush folded-only polls at idle refreshes.
The shared atomic keeps official encoding semantics: timestamps before its origin saturate to
that origin, representable offsets are bounded, and equal encoded timestamps prefer native source.
The ingress merge separately retains the earlier request on equal or unavailable timestamps.
These paths preserve source/time custody without promising lossless arbitrary-clock ordering.

The Window callback records arrival before accessing App. The fork's existing native ingress
retains reentrant requests and merges force/presentation requirements, keeping the earliest known
timestamp and its matching source. Equal timestamps retain ingress order. Existing active arena
scope state identifies a draw-time arrival without another draw-state owner. Delayed delivery
does not create another raw signal. Completed skips follow existing native-frame completion;
failed/refused/unwound draw attempts are not relabeled as no-render skips. Recovery and exact
Submitted receipts remain authoritative. The inherited throttle helper is reconciled with official
required-presentation bypass and high-rate-input exemption from inactive throttling, while thermal
throttling remains available.

Windows associates the signal with the exact registered HWND/WindowId/generation. VSync records
after its actual compositor wait or fallback sleep and before redraw invalidation. Local requests
retain local provenance. Visibility, minimization and retirement drain stale captures. Foreign
backend source comparison and portable production compilation are separate from native execution.

The macOS source includes the official per-display registry prerequisite `96ce8f2a05`, host-clock
conversion, first-signal retention and stop cleanup, using the existing CoreVideo clock bindings.
The fork retains structured startup errors, queued activation timestamps, transaction depth and
native retirement. Wayland retains the official wrapping millisecond conversion, 60-second
plausibility bound and receipt-time fallback; its existing local executor request captures enqueue
time. The official demand-driven ping/retry and later cadence state machine remain queued rather
than being inferred from that timestamp support. X11's existing Expose path joins its timed refresh
and the delivered timer carries its calloop deadline/local source. Web RAF keeps the official
unavailable captured timestamp with native provenance; explicit local requests capture before their
post-borrow microtask. An initially drafted Web RAF clock conversion was removed to preserve the
official baseline without an independently qualified consumer. Headless has no fabricated native
signal. Five original foreign-clock regression bodies and a fork queued-activation regression are
source-preserved; they have not executed on this Windows host.

Initial integration compilation exposed two calls resolving to the raw `HasWindowHandle` trait
instead of the framework identity accessor. Both journal paths now use the existing exact Window
handle. The corrected locked all-feature Core library check passes on Rust 1.95.0. Behavioral,
native, independent-consumer and compiler-floor qualification are in progress; no broader result
is established by that compilation.

The first Core signal/ingress/journal/hang/frame/candidate selection passes 93/93
(`0bbda82b-30b9-4536-a696-26a0fa890432`), with 1,147 unrelated cases filtered out.
The Windows native/registry/VSync selection passes 6/6
(`2861eda8-53e1-4506-884f-d563ffb15ea8`), with 212 unrelated cases filtered out. It includes
real Submitted frames, hidden/minimized recovery, delayed retirement, released signal ownership,
stale registrations and an actual background VSync capture before foreground delivery.

An initial public command selected nonexistent integration-target names and stopped before
running tests. The corrected journal/lifecycle/frame-signals selection runs 13 tests, with 12
passing and one closure/Idle fixture failure (`8b8b2087-6e83-4e14-8af7-a6744071701d`). Its
no-op deterministic task does not invoke the real scheduler's profiler turn hooks. The fixture
now opens a second fully submitted control window before the pending-demand scenario and dispatches
real noninvalidating input after closing the first; exact Closed, released root, no-redraw and Idle
assertions remain. This is a public callback observation correction rather than a weaker close
or journal contract. Final public execution remains pending.

The corrected public frame-signal trajectory passes 4/4
(`1a07af7a-40e3-4e1d-8857-651dac1755da`), without skips. It retains both inactive required
presentation and activation/default-request recovery, exact accepted/submitted generations,
deferred-scene demand and close/retirement assertions. Final source review also corrects native/
Web local-request adapters to preserve an already captured timestamp/source pair instead of
retagging a known native time as local; only absent times receive a new local capture. The
existing backend request hook documents that custody. Independent Core, Windows and foreign
platform reviews leave no confirmed open P1/P2 in their scoped snapshots. All eight
implementation/review subthreads are closed. Complete affected qualification follows below.

The complete Core library plus journal/lifecycle/frame public targets pass 1,253/1,253
(`cbf7720c-08a4-40d4-ad33-2c4beed596d2`), without skips. The expanded WGPU/Windows/gallery
library-and-integration selection runs 442 tests: 441 pass and the unchanged global clipboard
round-trip fails while reading its final metadata-bearing value as `None`
(`b2fbfbcf-f24e-426a-8558-78953200085a`). The five preexisting skips are four interactive
Windows-authority cases and the gallery fixture-regeneration command. All executed frame-signal,
native Submitted/reconstruction/retirement and ordinary gallery interaction scenarios pass.
The clipboard failure's cause is not established; it remains recorded while an unchanged isolated
rerun checks reproducibility. The stopped serial sequence has not yet executed the subsequent
component or independent-consumer gates.

The isolated clipboard rerun passes 1/1 with source and assertions unchanged
(`12624f95-69c6-4ca7-b79d-96abb95e53aa`), filtering 217 unrelated cases. This establishes
that the failure did not reproduce in that rerun, not a diagnosed cause or a clean 442-test
single-run result. Keep the initial failure and the successful unchanged rerun together. The
remaining serial component, independent and feature-disabled gates resume without repeating
the already successful gallery/native scenarios.

The UI core/components/docking/Canvas cohort passes 2,966/2,966 across 25 binaries
(`27941d1b-a476-4d00-b14b-54ff9e203b4e`), without skips. Existing test-support unused-token
warnings remain unchanged. Before final qualification, the main integration also restores the
official and previous fork behavior of sampling high-rate input again after next-frame callbacks
for the presentation decision; a cached pre-callback sample must not stand in for current state.
The later final Core/native checks cover this small source follow-up. Independent enabled/
disabled consumers, feature-off Core, portable and compiler-floor checks continue serially.

The independently locked ordinary consumer passes 38/38 with all features
(`418ecbcf-de96-4267-a39e-92ed48e59c82`) and 29/29 with no default features
(`84903976-50f2-4f3f-826a-c7d2109a5d70`), both across 14 binaries without skips.
The new frame-signal target executes six enabled or three disabled API/value controls; its two
explicitly typed backend/journal closures remain unexecuted signature checks. Disabled capture
evaluates its closure zero times and the accumulator has zero timestamp-storage size. Existing
consumer dependencies, package versions and checksums are unchanged; both lockfiles add only
the existing scheduler package references required by Windows and macOS.

Profiling-disabled Core plus public lifecycle passes 1,122/1,122
(`163e56ee-955d-4c40-82f7-313bde5f26b2`), without skips, including the final source's
disabled timestamp-capture/storage controls. Final all-feature Core/Windows public integration,
workspace/consumer compilation, correctness Clippy, compiler-floor and portable checks follow.

The first final-source Core/Windows public run is interrupted after 1,401 of 1,482 cases,
without a final Summary (`open-gpui-frame-signals-core-native-qualified.log`). On continuation,
the owned session is unavailable and no Cargo/rustc process remains; the log's last write is
02:42 local. That partial output is not a completed qualification, and the subsequent queued
compilation/Clippy steps did not run. A fresh serial run reuses the existing build products and
keeps this interruption separate from test failures.

The resumed complete final-source Core/Windows library and public integration selection passes
1,482/1,482 across 12 binaries (`9f7394cf-ed21-4f9e-95b4-1d34111f056a`), with four preexisting
interactive Windows cases skipped. It includes all new Core, public and native signal controls,
the original journal/monitor/retirement cohorts, actual Submitted/reconstruction, the negative
public-surface compile cases and the unchanged clipboard round-trip. This completed run follows
the final high-rate-input sampling correction; the interrupted output above supplies no completion
claim. Earlier WGPU/gallery and component results retain their separately recorded scope.

Locked workspace/all-target and independent-consumer/all-target/all-feature checks pass on
Rust 1.95.0. Affected Core/WGPU/Windows/Canvas/Web all-target/all-feature Clippy passes with
`-D clippy::correctness`; preexisting style/unused warnings remain warnings. Both root and
independent-consumer `cargo fmt --all -- --check` pass. Existing import-boundary, document-link,
release-document and public-API scans pass, and the three frame-signal workflow consumer commands
parse as valid YAML with serial test execution. No hosted CI execution is claimed. Actual
Windows Rust 1.92.0 and portable production compilation remain the final checks below.

Actual Windows Rust 1.92.0 affected Core/WGPU/Windows/Canvas all-target/all-feature and
independently locked consumer all-target/all-feature checks pass. Stable Rust 1.95.0 Core/WGPU/
Web production libraries compile for wasm32-unknown-unknown with profiling disabled. The enabled
configuration and existing nightly shared-memory configuration continue separately; successful
Windows or portable compilation establishes neither Linux/macOS native execution nor browser
callback/timing qualification.

Stable production wasm also compiles with profiling enabled. The existing nightly shared-memory/
build-std configuration compiles its multithreaded mailbox branch, using the same commands and
toolchains already qualified by the preceding journal unit. These results are production compile
evidence, not browser/test-support execution. No normal compiler-floor, dependency version or
platform-release claim changes.

The scoped implementation is committed locally as
`5811543bebe10657b9f99b42bfadb214e7dad999`. Final Core/Windows is 1,482/1,482; the earlier
WGPU/gallery selection contributes 228 passing cases with one fixture-regeneration skip, separate
from its recorded clipboard failure and the later complete Windows success. Component/docking/
Canvas is 2,966/2,966, disabled Core/public 1,122/1,122, independent enabled/disabled 38/38 and
29/29. Compiler-floor, production wasm, locked compilation, correctness Clippy, formatting and
existing scans retain their exact scope above. All eight implementation/review subthreads are
closed. No push, hosted CI, branch movement, engine-default switch or performance measurement
is part of this qualification.

Continue with the complete official demand-driven platform-loop prerequisite `eb354c8d50` and
X11 Expose/recovery reconciliation, then refresh-aware measurement v3, power/cadence and later
macOS MetalFrameSource. Native Linux/macOS, browser execution, general occlusion/physical-power
and numerical-cost evidence remain unqualified. Complete publisher-tail drain and accepted queue
backlog retirement, benchmark/report/debug-overlay evolution, hot reload, canonical docking U4-U9
and Nara GPU/View adoption remain separate open scope. This unit does not complete fixed-target
parity or Nara R11/R12.

## Demand-Driven Platform Frame Loops

This successor starts at `c7fb5ccd8a81ee8221fb90711b02649516b4c4fa`; its qualified local
implementation is `b6506b48efca022c13256ef17d5ecfa26787b2ae`, without publication. It retains
fixed target `635026fabc38440fce5e1767d125026a17b12012`. Official Web wake/park and weak invalidator wakeups
come from `417d03305ac194470f5d85a4b584c34dcec8cf57`; Wayland's complete FrameLoop/ping/retry
mechanism and Core scheduling/async refresh come from `eb354c8d50`, with `cd4fc8de4c` signal
custody. X11 direct Expose and recovery reconstruction follow `ae99a867d7` and `f25434f3c5`.
Later refresh-aware measurement v3 and power/cadence retain their own owners.

Core uses the official `PlatformWindow::schedule_frame` and weak `frame_waker` through existing
WindowInvalidator and App effect-flush owners. Dirty state, accepted scenes needing presentation
and next-frame callbacks retain separate demand. The old completion-driven scheduling hook is
removed; Windows post-borrow native maintenance keeps its existing command path. Callback/frame
demand survives throttle, refusal and unsuccessful presentation. Wakeups execute outside the
invalidator borrow and cannot retain App/Window through a callback cycle. AsyncApp refresh now
uses the ordinary update/flush transaction, so a queued refresh effect reaches a parked source.

The first Core frame/signal/journal selection runs 20 tests, with 17 passing and three new
fixture failures (`ed54399f-f23d-4674-9904-4aa106a3eed4`). The fork's initial presentation can
consume startup callbacks before the source-only official fixture's assumed ticks; the corrected
fixture explicitly defers those initial requests. A callback asserting another Submitted scene
now creates actual refresh demand. The retained-waker test checks unchanged strong ownership,
closed-source refusal and final backend release after complete App teardown rather than assuming
that logical close instantly drops every test-context reference. Original parking, wake and
pending-presentation assertions remain in the following reruns.

Platform native, source-state, public/independent consumers and final enabled/disabled qualification
follow after production-stable worker checkpoints. Isolated source-extraction tests from a worker
are not package or browser execution; the Web and Wayland private production state modules are
made host-testable without copying their state machines. Windows continuous VSync is native
execution evidence, not proof of Wayland/browser parking. Native Linux/macOS/browser execution
and numerical cost qualification remain separate from portable compilation and state tests.

The second Core selection passes 27/29 (`aef20fb9-c096-4b61-b02e-dd511b94c74c`), with only
the two initial-compositor assumptions still failing. The fork's `add_window` alone does not
establish the fixture's accepted nonempty scene before those ticks. The corrected private fixture
now activates the test window and prepares that initial scene explicitly, matching the public
consumer's setup; presentation and parking still run through actual scheduled callbacks. The
retained-waker owner-release control passes after complete App teardown. Existing visibility,
signal, frame-demand and failed-retry controls pass unchanged.

Independent Core review identifies a real accepted-publication unwind gap: a candidate can become
accepted and clean with `needs_present` still true, then unwind before ordinary demand rescheduling.
The callback owner now catches that unwind, rearms remaining demand, restores rejected dirty/force
work where applicable, completes existing post-borrow native maintenance and resumes the original
panic. Recovery scheduling failure cannot replace the original panic or skip the separate native
completion attempt. New actual scheduled-callback controls require the same accepted generation
to submit without another draw, and a rejected render to preserve the old scene until a scheduled
retry succeeds. Neither recovery uses a manual `present` call. Their executed results appear
below; the review finding is source-confirmed, without a claimed executed red baseline for it.

The current-host all-feature Core/Windows/Web/Linux production-library check passes on Rust
1.95.0. Linux's cfg-excluded backend is not thereby typechecked for its native target. Wayland uses
the existing `CompleteNativeFrame` command after Core delivery/borrow release; its cfg/diagnostic
and test matches extend to Linux/FreeBSD, with an X11 no-op. `Stopped`, stale-deadline fencing,
explicit Submitted/error outcomes and post-borrow completion are fork adaptations to the official
loop. Source review finds no confirmed open P1/P2 in the frozen Wayland scope. X11/Web review
also finds no new P1/P2, and identifies the inherited official ResizeObserver forced-frame request
omission. That request is now restored after resize through the existing callback slot and Core
ingress, using mandatory presentation/force and unavailable native timestamp exactly as the fixed
target. No browser-before-paint execution claim follows from that source correction.

The corrected Core/public signal/scheduling/visibility/unwind selection passes 62/62 across five
binaries (`acf9a28d-1e6d-4b34-825b-d3860a6263ce`), with 1,203 unrelated cases filtered out.
Both accepted-publication and render-unwind controls execute through actual scheduled callbacks
and retain their original panic, scene/generation and parking assertions. The five public
scheduling workflows pass: notify/refresh of parked windows, callbacks queued mid-frame,
Deferred/Rejected old-scene retry, controlled atlas candidate refusal and held-native-terminal
close. Existing signal, journal and visibility controls remain green. A source sweep additionally
supplies the Linux headless no-op for the extended completion command; native Linux compilation
remains separate. Platform state/native and full affected qualification continue serially.

The host-testable actual Web/Wayland/pointer state selection passes 27/27 across two package
binaries (`2bdfe395-e53b-4851-b652-e07985ffcae6`), without skips, on Rust 1.95.0. It executes
the same private production state modules rather than an extracted copy. The worker's earlier
isolated Rust 1.99.1 Web source-extraction result is not part of this package qualification.
Ten Wayland and eight Web frame-state scenarios cover request coalescing, unconfigured/parked/
stopped behavior, retry after failure, outstanding callbacks, stale deadlines and independent
window demand; the remaining cases are the existing Web pointer controls. X11's six actual
callback-dispatch regressions remain source-only on this Windows host.

The affected native paint/power/visibility/profiler selection passes 5/5
(`293b470e-d7c1-452c-87b3-41afddaca029`), filtering 213 unrelated cases. It retains the original
actual Submitted/monitor/reconstruction assertions and extends the existing VSync trajectory:
public AsyncApp refresh and callbacks queued mid-frame reach newer exact nonempty Submitted
generations through the real message pump, each callback runs once, and pending callback captures
release during retirement. No private request hook, manual draw or fabricated receipt replaces
that added production path. The two-HWND reentrant paint control also retains native-command
ordering for IME maintenance. Windows VSync remains continuous and supplies no parking claim.

Final independent reliability review of the unwind correction leaves no confirmed open P1/P2;
the earlier Core finding is resolved within the callback owner and its executed regressions.
Web ResizeObserver source reconciliation is also independently rechecked without a new finding.
All nine implementation/review subthreads are closed. Complete runtime/public/gallery/state,
profiler-enabled component/docking/Canvas, independent configurations and disabled Core/public
qualification now run serially against the frozen source; final static/compiler checks follow.

The complete framework/public/gallery/state/native cohort runs 1,749 cases: 1,748 pass and the
unchanged native image decode/animation trajectory times out waiting for its nonempty Submitted
frame plus RGBA capture (`c89a3989-c0a0-49aa-b731-c3fc6c4fd3b3`). Its five preexisting skips
remain four interactive Windows cases and the gallery fixture-regeneration command. All Core,
public scheduling, state, gallery and other executed native cases pass. An unchanged isolated
rerun with the same package/feature union passes 1/1
(`1640f12f-3987-406d-8e08-04b351a1231d`), filtering 1,753 unrelated cases. The timeout did not
reproduce in that rerun; its cause is not established. Preserve both results rather than report
a clean single-run 1,749/1,749 result or widen the test's bounded wait.

The first component command uses the renamed package spelling for a dependency feature and
stops before compilation/tests. Its corrected `open_gpui/profiler` dependency-key spelling
preserves the intended profiling-enabled component cohort. This command correction changes no
manifest, dependency resolution or source. Component, independent and disabled gates continue.

The corrected component sequence is interrupted during compilation, before a nextest run ID or
test results. On continuation its owned session is unavailable, no Cargo/rustc process remains
and the log last changed at 11:06 local. The later independent/disabled steps have no logs and
did not run. A resumed serial sequence reuses the existing build products; the interrupted
compilation establishes no component result and is kept separate from the command spelling
correction and runtime failures above. The resumed profiling-enabled UI core/components/
docking/Canvas cohort passes 2,966/2,966 with no skips
(`a36aa07b-fd4d-42b6-a0e6-28a2fe12004f`). Independently locked consumers pass 40/40
with all features (`4ea21cb9-812c-4f0d-92d3-2088d2cbd8e7`) and 31/31 without default
features (`44adc352-858e-478b-9627-d7753b588cdb`), with no skips. Their two new scheduling
tests execute typed but uncalled closure construction and remain signature evidence; the five
crate-external TestPlatform workflows above supply actual scheduling behavior. The complete
instrumentation-disabled Core library plus lifecycle/scheduling public cohort passes
1,134/1,134 without skips (`f9e6607b-1115-46ac-9a6b-bdc6233dee08`). The two scheduled
unwind regressions and all five public scheduling workflows execute in this configuration too.

Locked workspace all-target and independently locked consumer all-target/all-feature checks
pass on Rust 1.95.0. Correctness Clippy passes for Core, WGPU, Windows, Canvas, Web and Linux
package targets on this host, and both root/independent formatting checks pass after the frozen
source is formatted. Host checking does not typecheck cfg-excluded native foreign backends.
No manifest or lockfile changes are required by this unit.

Actual Windows Rust 1.92.0 all-target/all-feature checks pass for Core, WGPU, Windows and
Canvas and for the independently locked consumer. Stable Rust 1.95.0 production wasm32
Core/WGPU/Web checks pass with profiling disabled and enabled. The installed nightly with the
existing hello_web shared-memory/build-std configuration also compiles the multithreaded
production branch. These are compilation results, not browser callback, pixel or timing
execution. Native Linux/macOS, browser/test-support and physical refresh/power qualification
retain their separate unexecuted scope.

The existing import-boundary, public-API, document-link and release-document scans pass, as do
YAML loading, the focused serial CI command check and Git whitespace validation. CI changes
are local configuration; no remote job, push or publication is performed. Continue with the
planned complete refresh-aware measurement-v3 owner, then power/cadence and MetalFrameSource.
Complete benchmark/report/debug-overlay comparison, publisher-tail/backlog retirement,
conditional hot reload, canonical docking U4-U9, numeric costs and engine GPU/View adoption
remain open. This scoped prerequisite does not complete the fixed-target refresh or select an
Editor/game-runtime default.

## Refresh-Aware Frame And Hang Measurement V3

This successor starts at `9901ed0bcc6272a647bccfb73016f3f22206a013`, after the complete
demand-driven platform-loop prerequisite. Its qualified local implementation is
`411895475e9e51a9f06954d5d676333d597b991f`, without publication. The fixed cycle target remains
`635026fabc38440fce5e1767d125026a17b12012`. The complete official measurement-v3 mechanism
comes from `4f6d97b9de7d9f167da8b7832eeecea22c5a9efa`, with the final profiler/journal cleanup
`dde76a1dca`. Later power/cadence and MetalFrameSource remain separate owners. Application-only
telemetry/UI consumers are comparison inputs, not framework dependencies.

Existing profiler, Window, view, journal, detector and monitor owners retain the mechanism.
Frame work starts at the later of signal arrival and invalidation and ends at platform-submission
start. Refresh grouping, real-interval missed refreshes, the official lateness floor/fallback and
input-response groups retain public report semantics. View type identity follows erased/weak
handles and each phase records self-time excluding nested views. Callback signal scope and
unwinding view scopes must not contaminate later measurements; accepted pending scenes retain
their original timing through failed submissions. Only actual Submitted completes the covered
frame/input measurement. Rejected attempts remain recorded work without fabricated completion.

The following record retains the implementation and public/native regression sequence, including
failed controls and their owner corrections. Existing synthetic ownership controls, TestPlatform
workflows, actual Windows message-pump/Submitted evidence and production compilation retain
distinct scope. Native foreign-platform/browser execution, physical refresh/power behavior,
numeric overhead and engine integration remain separate qualification.

The all-feature current-host Core/Windows production-library check passes on Rust 1.95.0.
The first focused Core/public cohort passes 184 of 187
(`e02f011b-2c24-40dc-b0bd-c6b2c9fa22d0`). Two failures are test migrations: the prior public
consumer still expects measurement version 2, and the new no-render control incorrectly expects
an empty idle callback to seal an interval. It now requires the exact raw signal and no boundary,
preserving the established completed-skip contract and the later direct draw's unmeasured state.
The third failure exposes a real inherited sealer defect: a nested view completed first fixes
the interval start too late, clipping an enclosing ten-millisecond draw to eight milliseconds.
The expected ten milliseconds is retained, with an explicit original-start assertion.

Independent journal/hang review also identifies two inherited official active-use defects:
poll-only consumers accumulate per-input history indefinitely, while fixed-time history pruning
can erase evidence needed by a still-open long interval. The owner red selection runs four tests:
one batched-boundary control passes and the two active-use plus nested-sealer regressions fail
(`d2b1406e-537d-4404-8a4e-737f228afe9c`). These are executed failures, separate from the earlier
source review. The correction keeps only the latest deliberate input, observes/seals entries in
recording order and leaves time consumption independent of attribution. The sealer retains its
previous-boundary floor and chooses the earliest retained work when sealing, so completion order
does not erase enclosing work. Its independent integer reference model follows the same timeline
contract; no original cap, discontinuity, demand or interval assertions are removed.

The actual Window/view controls now exercise RequestLayout, Prepaint and Paint nested self-time,
each phase's unwind and subsequent recovery, and erased/weak/failed-downcast attribution.
Independent source reviews leave no confirmed profiler or Window/view P1/P2. One additional
deterministic control combines a failed 60-Hz frame, another input and a replacing 240-Hz frame;
only the submitted replacement can classify work and complete both pending inputs. Corrected
focused and native/complete qualification follow before a final implementation claim.

The corrected focused Core/public cohort passes 191/191
(`60da1bfb-4c7a-421c-8f2b-65dae363bb0e`), filtering 1,117 unrelated cases. It includes all
three executed owner regressions, the sealer's completion-order/reference/occupancy properties,
the 60-to-240-Hz replacement control and all six public measurement workflows. The two migrated
fixture assertions retain their exact version/raw-signal and no-empty-boundary meaning.

The first native profiler/signal selection passes one of two
(`eb5d2ac1-6c88-467b-b8e7-b924014762ec`). Actual refresh-aware timing, raw callback association
and public report assertions execute, but the new one-shot render-delay marker is consumed by
an intermediate input render before the exact target scene. The corrected fixture binds that
control to the complete target input source until its exact Submitted frame is observed, then
releases it. It preserves original generations, receipts, dual collectors, reconstruction,
monitor/callback and retirement assertions. Corrected native execution passes 2/2
(`95fb8f79-a310-4d30-b3c7-e90fc65f7577`), filtering 216 unrelated cases. The observed native
sample is correctness evidence, not a numeric overhead or frame-budget qualification.

Further bounded review confirms a separate inherited duration defect: two deliberate inputs at
seconds 1 and 200, consumed at second 201, are interpolated as 200 active seconds instead of the
61-second union of their active windows. The explicit red regression reproduces 200 versus 61
(`88f4fc5f-aab0-495f-abda-cd2e390831dc`). A periodic short monitor cadence can hide this case;
the public API does not impose that cadence. Merely moving a cursor through completed input
events cannot preserve an enclosing input whose start precedes already completed nested events.

The complete runtime build is deliberately interrupted before nextest execution while this owner
correction is made; it supplies no broad test result. The existing input lifecycle now supplies
chronological begin/end activity diagnostics through the same journal, including nested and
unwinding input. Active time is settled from those state changes with bounded state, while
completed input events retain incident attribution and their original work/latency authority.
These raw controls are neither work nor interval completion. The following final-source
qualification supersedes the earlier 191 focused and two native passes for this correction.

The lifecycle correction's all-feature current-host production-library check passes. Final
focused Core/public execution passes 208/208
(`2fc7a635-8054-446f-a5f1-944941cd88b7`), filtering 1,117 unrelated cases. It retains the
61-second red expectation and covers one/multiple drains, overlapping windows, repeated/capped
consumption, synchronous nested input, guard unwind/retirement, suspend/loss and the independent
interval-union model. Actual profiler integration also requires outer input to remain active
after child completion and activity to close before profiler retirement or draw rollback.
Expanded native profiler/signal trajectories pass 2/2 against this source
(`a12e0e8a-3bf5-486f-ade7-b1aa808fa95d`), filtering 216 unrelated cases.

Final independent rereview closes all four confirmed defects and finds no remaining P1/P2 in
the bounded profiler/Window/view/journal/hang/input-lifecycle scope. Exact-journal weak guard
custody, raw-marker semantics, loss/suspend behavior and retained property operation spaces
remain explicit. All eight implementation/review subthreads are closed. Existing import-boundary,
public-API, document-link and release-document scans pass; workflow YAML loads and its three
public/independent measurement gates retain serial test execution. The corrected complete
framework/component/independent/disabled sequence is running; final build/compiler-floor and
portable checks follow. Pending publisher tails/backlog retirement, foreign native/browser
execution and numeric costs remain separate from this scoped qualification.

The corrected complete framework/public/gallery/production-state/native cohort passes
1,809/1,809 across 26 binaries (`869fc559-c396-42cd-b18c-c8fb108aebca`). Its five preexisting
skips remain four interactive Windows cases and the Gallery fixture-regeneration command.
The complete Windows image-animation trajectory passes in this run, as do both expanded
profiler/signal trajectories, native reconstruction/retirement, public measurement consumers
and actual production-state modules. No unchanged retry is needed for this cohort. Profiling-
enabled UI core/components/docking/Canvas passes 2,966/2,966 across 25 binaries without skips
(`cd6f2be8-2e61-4b4d-9298-d242ddd62462`). Independently locked feature configurations and
disabled Core/public qualification follow in the same serial sequence.

The independently locked all-feature consumer passes 46/46 without skips
(`9a9d4b0b-9c82-413d-8889-076b39890493`). Its six new enabled controls qualify ordinary
public frame arithmetic, histogram buckets, per-refresh accepted-draw snapshots, v3 view
conversion and raw input-marker non-work semantics. Typed unexecuted closures remain signature
evidence, separate from the executed public TestPlatform and native workflows above.

The independently locked no-default-feature consumer passes 32/32 without skips
(`8caf2adc-08b2-4b30-811a-4350171c6f48`), including the new disabled timing-scope/collector
control and existing zero-capture frame-signal behavior. Disabled Core/public qualification
continues separately.

The complete no-default-feature Core library plus lifecycle/scheduling public cohort passes
1,134/1,134 without skips (`201adfba-7258-4eb1-8f3b-cc65c363eb0f`). Feature-gated measurement
tests supply no disabled execution claim; the actual ordinary scheduling, input, presentation,
visibility and task-guard controls execute in this configuration. All runtime/consumer gates
now pass against the frozen lifecycle correction.

Locked workspace all-target and independently locked consumer all-target/all-feature checks pass
on Rust 1.95.0. Correctness Clippy passes for Core, WGPU, Windows, Canvas, Web and Linux package
targets on the current host. Both root and independent formatting checks pass. Actual Windows
Rust 1.92.0 all-target/all-feature checks pass for Core/WGPU/Windows/Canvas and the independently
locked consumer. Neither manifest nor lockfile changes are required by this unit.

Stable Rust 1.95.0 production wasm32 Core/WGPU/Web checks pass with profiling disabled and
enabled. The installed nightly with the existing hello_web shared-memory/build-std configuration
also compiles the multithreaded production branch. These are compilation results, not browser
callback/pixel/timing execution or cfg-excluded native Linux/macOS qualification. Existing import,
public-API, document-link and release-document scans, focused YAML/serial-command checks and Git
whitespace validation pass. All eight implementation/review subthreads close; no remote CI job,
push or publication is performed.

Continue at the complete official power/cadence successor and MetalFrameSource, then remaining
framework/interface/report differences, including generic View/ViewElement and conditional hot
reload. Complete benchmark/report/debug-overlay comparison, pending publisher-tail/backlog
retirement, canonical docking U4-U9, numeric costs and Nara GPU/View adoption remain open.
Measurement qualification changes no Editor/game-runtime default and does not complete the
fixed-target refresh or Nara R11/R12.

## Power Modes And Stable Frame Cadence

This successor starts at `110f9c88084227eb526515167c204af3555f22a8`, after complete
measurement-v3 qualification. The implementation is committed locally as
`2ceb09aa4590388b8e2d73ac1406caddc5b6c470`, with failure diagnostics at
`1a6d90bdaac5256c36064be9824459de74ea91bf`. The final results and native limits are below.
The fixed cycle target remains
`635026fabc38440fce5e1767d125026a17b12012`. Complete official power observation `fdcb198110`,
stable pacing `2580a00264`, configurable frame limits `7c8dbadcb5` and macOS MetalFrameSource
`0373366f43` map to the existing Platform/App, Window, process-info and backend frame owners.

Public power modes preserve unknown state, native callback ordering and observer lifetime.
Inactive, low-power and thermal intervals compose by the longest applicable value; mandatory
presentation and existing recovery authority bypass limits. Ordinary dirty work, pending scenes,
next-frame callbacks and high-rate-input presentation all constitute demand. Pure idle callbacks
must not consume cadence or wake a parked source. The official one-millisecond tolerance and
bounded quarter-interval correction preserve stable deadlines without a catch-up burst after a
long stall. Source/clock pairing, exact candidate/Submitted authority, deferred force and native
retirement remain intact. The local pacer handles unrepresentable public intervals without
overflow, retaining mandatory bypass.

Official provenance and extraction-specific decisions remain distinct:

- Official: PowerMode queries/observations and Linux modern/legacy providers; longest-limit
  composition, configurable intervals, stable cadence and all demand classes; macOS process-info,
  MetalFrameSource/CV fallback and offscreen screenshot behavior.
- Fork integration: the existing ordered cross-thread App ingress, shared notification-handler
  shape, immutable reader snapshots, exact candidate/Submitted and weak native-generation
  retirement. Apple's responsibilities map to the existing macOS package, without a second
  renderer or scheduler owner. Ordinary public consumers remain independent of Nara types.
- Owning corrections: unavailable Windows power queries remain Unknown; unrepresentable public
  intervals defer without overflow; native retirement retries no longer retain a detached task
  after their HWND transport is destroyed. The latter defect belongs to the fork, not Zed.

Implementation and owner/public/native qualification are in progress. No result, numeric power
saving or complete platform-execution claim follows from the prepared source. Windows synthetic
power messages must not change the host's physical settings. Actual host tests, native Windows
submission and cfg-excluded foreign source/production compilation retain distinct scopes.

The first focused owner/public run `aaf053b7-43e1-472b-a49c-acdab010905f` executes 73 tests:
69 pass and four fail. The foreign-thread App fixture wakes the deterministic scheduler from an
unregistered OS thread. Its correction queues the importer on the owning thread before the real
foreign callbacks, preserving their sequence and coalescing assertions; this does not qualify an
otherwise sleeping test scheduler's cross-thread wake. The scheduling fixture now explicitly
disables limits so its two-window immediate-submission promises remain about demand/wake rather
than inactive throttling. The signal fixture primes a real mandatory submission rather than an
idle callback, because idle callbacks no longer consume cadence. The new skip-reason fixture
dispatches real input before expecting a retained-work boundary; raw idle skips must not produce
empty intervals. Exact accepted/Submitted generations, pending input and deferred callbacks remain
asserted. Added controls cover high-rate-only presentation and expiry, mandatory cadence reset,
LowPower signal custody and power-mode changes without sleep/wake measurement interruption.
Two subsequent build-only attempts expose a missing public AppContext trait import in the migrated
scheduling fixture and an overlapping TestAppContext borrow in the new private signal test. Both
are corrected without production changes. The corrected focused run passes 76/76
(`38d81424-4371-452a-a975-380a0172de26`), filtering 1,273 unrelated cases. It includes all four
previously failing controls and the added cadence, input, signal and measurement regressions.

Initial independent Core, App/Linux, Windows and macOS reviews report no confirmed P1/P2 in their
bounded source scopes. Later native execution exposes the retirement defect below. Portable tests
include the actual production Linux profile parser and Metal frame
state, not native D-Bus or CAMetalDisplayLink execution. Linux service owner loss/reacquisition is
supported; complete system-bus disconnection currently ends observation with Unknown, matching the
fixed official implementation, and automatic bus reconnection is not claimed. macOS process-info,
Metal/CV window/renderer integration and the later offscreen screenshot correction `f816e1f926`
are adopted as source. Native macOS compilation, nonempty Apple/Intel screenshot pixels, display
migration and old-system CV fallback still need a macOS host.

The host-portable selection passes 3/3 (`f254585d-0eb6-426e-b3cd-4df4da2a1f25`), filtering
1,334 unrelated cases. It compiles the actual production Linux profile parser and Metal frame
state module through their package tests. This supersedes the earlier direct-rustc two-test
Metal state check, without adding native platform-execution claims.

The first native selection passes 4/8 (`7c8a5d75-566c-41e2-a64b-6c35536125c7`). The corrected
second selection passes 5/8 (`e82cde78-9ec0-46e5-ba51-97b7ba663052`). Registration sends an
asynchronous native power-status value; the fixture must observe that actual initial payload
before injecting synthetic preferences. Otherwise a late initial HighPower value can overwrite
the intended LowPower scenario. A test-only native probe now waits for the real payload. Pending
callback setup and closure share one App borrow so a legitimate successor callback cannot consume
the test capture before the requested retirement. These corrections retain exact source, native
submission and callback-lifetime assertions.

Additional retirement diagnostics reproduce an inherited fork P2: native HWND/OLE finalization is
complete, but a detached delayed retry still owns the platform Rc and its callbacks. A concurrent
foreground maintenance drain can complete finalization and destroy the retry's only HWND transport
before that delayed task resumes. Diagnostics show HWND absent, OLE released once, unregister and
destroy each attempted twice, no pending/finalization state, and one actual retained platform owner.
Independent diagnosis predicts the same values before reading them. The correction removes the
detached strong-Rc task from the existing retirement coordinator. HWND-bound timers use distinct
tokens, reject stale/previous-owner delivery and cancel before final HWND destruction; failed timer
admission falls back to the existing validated main-thread message. Finalization retains the
platform during retirement without a delayed-task owner. If both native transports refuse admission,
pending resources remain owned and
later maintenance can retry; successful progress under simultaneous transport failure is not
claimed. Win32's minimum timer period applies to shutdown retry backoff, not frame cadence.

The first post-refactor native run passes 8/11 (`e667243d-a833-4f6b-9456-bfc47234a9d8`). The
previously failing exact retirement now releases both weak owners, and all three new timer
controls pass: fallback admission, ordinary foreground maintenance completing before the valid
timer, and cancelled/previous-platform tokens. A build-only attempt first exposes two missing
test imports; explicit imports correct it. Independent rereview closes the retained-Rc defect at
the owning transport, retaining the simultaneous-admission-failure limit above.

Remaining native fixture corrections preserve their original contract: after both windows initialize,
read the latest actual source-0 Submitted generation rather than an earlier valid initialization
frame; inspect three staged power notifications while App is borrowed, then require the exact two
Coalesced links and final Delivered result after native import. Coalescing does not precede that
import. The power consumer dispatches actual native input before its frame callback so skip reasons
seal retained work rather than expect an empty interval. The profiler fixture responds only to its
`p` binding; unrelated desktop keystrokes do not panic the consumer, while the driver still requires
the injected `p` to update the source exactly once. Native input/frame/retirement assertions remain.

The next native run passes 10/11 (`534412f4-c575-4b2c-b1d3-336dd0e34413`). The remaining
power fixture attempted synchronous native input while holding App, correctly triggering the
existing busy-input boundary. It now dispatches outside the App borrow, through a rendered
mouse handler that invalidates the real view, before staging the native frame. Final focused
native execution passes 11/11 (`10cf828a-4e70-4535-abfb-3cd0a91e147c`), filtering 214 unrelated
cases. It retains actual hidden HWND notification registration, staged/coalesced delivery,
independent observers and App reconstruction, unavailable initial query recovery, exact failure/
retry retirement, two independently configured windows, deferred force/callback custody,
LowPower skip provenance, mandatory submission and mode-change recovery. Both preceding profiler/
signal and sleep/visibility trajectories execute unchanged guarantees. No physical power setting
or native receipt is synthesized as successful.

Existing import-boundary, public-API, document-link and release-document scans pass. The workflow
YAML loads with its existing two jobs and the five public/independent/disabled gates include the
new consumer. No remote workflow execution is claimed.

The complete framework/public/Gallery/production-state/native run executes 1,850 cases across
29 binaries (`7baff7f7-040b-42c6-909c-eda57f67f7d2`): 1,849 pass and one fails. Five
preexisting skips remain the four interactive Windows cases and the Gallery fixture-regeneration
command. All 1,629 non-Windows-package cases pass; the Windows package passes 220 of 221 executed
cases. Its sole failure is the two-HWND reentrant IME fixture, whose tight immediate message pump
does not wait for newly paced inactive dirty work. Its source/target windows now explicitly disable
all three frame limits so the test continues to isolate reentrant paint, exact coalesced callback,
post-borrow CompleteNativeFrame and restored text admission. All original assertions remain; the
dedicated public/native cadence workflows retain the actual limiting controls. No production
source changes follow this fixture correction. Complete Windows rerun and component/independent/
disabled qualification follow; the original broad failure is not reported as a clean full run.

The first complete Windows rerun executes 221 cases (`33fe8e01-b278-4cf1-9a32-29e2613860ef`):
220 pass, including the corrected two-HWND IME and complete power/signal/retirement trajectories.
Four preexisting interactive cases remain skipped. The nonactivating-first-show fixture fails its
global foreground equality, although its setup established only thread-local ActiveWindow. It
now acquires the existing foreground sentinel with the repository's native test helper and asserts
that actual global precondition before recording the baseline. Global foreground preservation,
application Z-order, native owner, click activation and programmatic activation assertions remain.
No activation policy or runtime source is changed. Complete Windows qualification follows against
this explicit native setup; external desktop focus is not inferred from thread-local activation.

The next complete Windows selection passes 220/221
(`6622f815-c33b-4ddf-a9c9-8cd92f93b78e`), retaining four interactive skips. Both corrected
fixtures and all new power/retirement workflows pass. The unchanged exact activation-terminal
case times out in its later success phase; it passed in both preceding broad selections. The
unchanged focused recheck passes all 14 affected/native/activation cases
(`0d58b084-cc5a-4c94-96b8-365f26c95aa4`), including that case.

An independent bounded activation review finds no confirmed production defect or dependency on
new frame pacing. The OS may reject foreground admission, and a successful ticket can also
coexist with later focus loss; the original timeout lacks facts to distinguish these from pending
readback. No cause is claimed from the successful rerun. The test retains all success and
first-terminal assertions; failure-only diagnostics now capture the exact ticket/generation,
pending/terminal boundary records and actual foreground/active/focus HWNDs. That diagnostic
successor still needs compilation/execution. A clean complete Windows run is not inferred from
the focused pass. All twelve implementation/review threads are closed.

Profiling-enabled UI core/components/docking/Canvas passes 2,966/2,966 across 25 binaries with
no skips (`c9b3b1f9-fa88-4133-802a-ae4e964b0b1e`). This includes actual Host interaction,
multiview scenarios, component composition and the existing virtualizer/cache controls. The
independently locked enabled/disabled consumers and disabled Core/public follow serially.

The independently locked all-feature consumer passes 51/51 without skips
(`6d220f39-2d6a-432b-8682-cbea92155a54`). Its five new power controls execute enum/default/
configuration and low-power reason assertions, while the explicitly unexecuted callback/App
closures remain signature-only evidence. Actual notification and window workflows retain the
separate public TestPlatform and native results above.

The independently locked no-default-feature consumer passes 36/36 without skips
(`6597c427-7844-4ac8-b3d9-9b02e40c5fa3`), including its four ordinary public power controls
and the existing zero-capture/empty-profiler accessors. Disabled Core/public qualification follows.

The complete no-default-feature Core library and public lifecycle/scheduling/power cohort passes
1,162/1,162 without skips (`1581e59e-6a63-446c-89d8-3657af1aea97`). Ordinary power/cadence
workflows execute without instrumentation, alongside existing input, rollback, visibility and
native-ingress controls. Profiling-only cases contribute no disabled execution claim. Component
and both independent gates now pass. Final Windows diagnostic execution and compiler/static
qualification remain separate from the completed host-portable gates.

The failure-diagnostic successor compiles and the complete Windows selection executes 221 cases
(`fd75df4d-26fe-474e-9fe2-fab5bf2db6ed`): 219 pass, two fail, and the same four interactive
cases remain skipped. All power/signal/retirement/IME/first-show controls pass. The activation case
fails earlier than the diagnostic wait: its target has no recorded activation/focus events, while
the foreground HWND changes to a third window rather than that target. This records loss of the
test's global foreground precondition, without proving the previous timeout's cause. The image
animation capture times out despite passing in the original broad and both preceding Windows
selections; bounded source diagnosis follows. No complete Windows pass is claimed and neither
oracle is weakened. Final build/static/compiler qualification proceeds separately.

Bounded image-source review finds no confirmed runtime defect or cadence starvation path. The
fixture follows each loader's own 0/1/0 cycle and captures the same exact submitted scene; its old
timeout does not identify the rejected prerequisite. The diagnostic successor at
`1a6d90bdaac5256c36064be9824459de74ea91bf` retains all nineteen acceptance conditions and
pixel oracles, records the calling phase and last unmet condition, and rethrows the original
failure. Independent review finds no P1/P2 in that or the activation diagnostic. All fourteen
implementation/review threads are closed.

The final complete Windows selection executes 221 cases
(`8700e766-e69f-4057-a23c-5afc1efc5b8e`): 220 pass, one fails, four interactive cases remain
skipped. Both diagnostic consumers, all power/signal/retirement/IME controls and corrected
first-show setup pass. The remaining failure is a different unchanged activation-restore fixture,
at its foreground-preparation wait. Its cause is not established by this result. One final bounded
recheck executes image capture, exact activation terminal and that activation-restore case:
3/3 pass (`6a4c2fd7-f7e8-4d31-a50b-edc683a5e0db`), filtering 1,852 cases. The historical
capture timeout and activation failures remain unresolved qualification limits; no clean complete
Windows gate is inferred from separate successful runs or weakened assertions.

Locked workspace all-target and independently locked all-target/all-feature checks pass on Rust
1.95.0. Final correctness Clippy passes for Core/WGPU/Windows/Canvas/Web/Linux/macOS package
targets on the current host after both diagnostic changes. Root and independent formatting checks
pass. Actual Windows Rust 1.92.0 all-target/all-feature checks pass for Core/WGPU/Windows/Canvas
and the independent consumer. Stable production wasm32 Core/WGPU/Web checks pass with profiling
off and on; the installed nightly and existing shared-memory/build-std configuration also pass.
These checks require no lockfile or package-version changes. Linux adds only a test dependency
edge to its existing Core dependency. Native foreign-platform and browser execution remain
unqualified by current-host cfg-excluded compilation.

This completes the implemented power/cadence unit's recorded owner, public, component, independent,
disabled and compiler checks, with the complete native-Windows gate explicitly still open. Preserve
that native qualification work while proceeding to the complete generic View/ViewElement, macro
and component-interface reconciliation. Physical power/frame-pacing costs, journal publisher-tail
and accepted-backlog retirement, full report/debug-overlay comparison, hot reload, canonical
docking U4-U9 and Nara GPU/View adoption remain open. No Editor/game-runtime default or complete
fixed-target refresh is selected by this implementation.

## Generic Views And Reusable Component Interfaces

This unit starts at `a89a7200c80fc40eb073dbad37789f17153ec7b8`, against fixed Zed
`635026fabc38440fce5e1767d125026a17b12012`, and is implemented locally at
`652122a4e118ff2d213ae0d9392c71f49c44553b`. Official `74b5207744ef95505fd75c56bf2c9d4736e931ee`
and final-target follow-ups supply View/ViewElement, typed/erased entity conversion, stateless
type-name scopes and the derive macro. Render and RenderOnce remain supported public traits.
The inherited Component and direct AnyView Element paths are removed without compatibility aliases.
One private phase implementation retains existing clip/transform/presentation/resource admission,
cached dependencies, opacity, reveal/portal aliases and scoped profiler unwind.

Initial production compilation exposes a missing Element trait import after moving phase code
into its private module. The corrected all-feature library check passes. The first disabled check
exposes a paint closure's mutable-call requirement; explicit one-shot closures preserve the same
phase execution through both configurations. The corrected profiler-disabled library check passes.
An independent source review identifies three conditional-cache fixtures and one profiler route
whose old AnyView return-type assumptions prevent test compilation. Their branches now finish
with into_any_element while all cache/opacity/subscription/type/unwind assertions remain. The
existing paint-only subscription trajectory also executes typed Entity::cached and erased cached
routes through the same complete assertions.

The first focused owner run passes 117/117
(`7d661b4b-143e-4bb1-ae06-42ef2c309be2`), filtering 1,207 unrelated cases. It preserves cache
and deferred dependency/opacity behavior, nested candidate rejection and unwind, exact atlas
admission and scene retries, view-presentation receipts, reveal/portal constraints and all selected
view profiler recovery/type-attribution cases. The paint-only subscription test executes both new
typed and existing erased cached routes. All-feature and profiler-disabled production-library
checks pass; actual new public/native consumers remain separately unexecuted at this checkpoint.

The new public TestPlatform consumer passes 5/5 without skips
(`d7ab944d-b04a-46f0-a6af-11692a32fd44`). It executes generic stateless pointer/keyboard/canvas
composition and type-local state, manual props with a non-Render backing entity across reorder,
removal and reconstruction, typed/erased/weak child and root conversion, and independent typed/
erased cached reuse, notification and exact release. At this initial checkpoint ordinary child
composition executes the macro's into_element override; the native trajectory below directly
executes its into_any_element override.
Independent consumer/derive/native-source review reports no confirmed P1/P2; native and ordinary
dependency execution still need their own results. Existing API-tier and document-link scans pass.

The first native build fails in the new fixture: terminal diagnostics contain
NativeInputDeliveryResult rather than DispatchEventResult. The corrected comparison copies both
propagate and default_prevented into the actual diagnostic value; input delivery and its terminal
assertion remain exact. An unused intermediate paint assignment is removed without skipping its
submitted-frame wait. The bounded core rereview closes the earlier fixture compilation findings
and confirms the typed/erased subscription trajectory retains all original assertions.

The corrected Windows native selection passes 4/4
(`b3629460-d6a6-4da4-ac03-81d302bbef3e`), filtering 1,546 unrelated cases. The new composition
executes props replacement, both entity cache routes, custom canvas, pointer/key callbacks outside
App borrows, exact newer nonempty Submitted generations, notification and refresh, retirement and
reconstruction. Backing, local-state, callback, root, global-theme and platform weak owners release;
stale native callbacks cannot enter the replacement window. Existing cached-dependency and both
profiler/signal native trajectories pass in the same selection. Inputs use production callbacks,
not OS injection; this establishes neither compositor scanout nor physical focus/timing costs.

The locked whole-workspace all-target check passes, including existing component/docking/Canvas
and example consumers. The independently locked ordinary dependency fixture passes 53/53 with
all features (`4bef5853-46d7-435a-b4fc-10f641a205ea`) and 38/38 without default features
(`c2ca7752-76ef-4a92-bc68-66702b1fcd66`), without skips. Its two new View tests construct the
generic/manual element and verify source callers; deliberately unexecuted composition/root
closures establish signatures only. Actual render/input/cache/lifetime behavior belongs to the
separate public and native trajectories above. No dependency or lockfile change is required.

The complete all-feature framework/library/public/Gallery/native cohort passes 1,856/1,856
(`2b8b0afc-5275-493e-b5fc-3e83ce40b23b`), with five existing skipped cases. It includes
Core 1,324, Windows 222, all five new public View workflows and the new native composition.
The current complete Windows selection passes without an isolated retry. This is a later complete
passing observation; it does not erase the preceding power unit's capture/activation and
foreground-preparation failures or establish their cause. Linux/macOS portable state tests on
Windows do not qualify foreign native backends, and Web host tests do not qualify a browser.

The complete affected UI core/components/docking/Canvas cohort passes 2,966/2,966 without skips
(`d347962d-bf7e-4de7-b289-d913bba188a3`). It preserves the existing component, overlay,
semantic, host recovery, viewport/payload and headless dependency-boundary assertions. No docking
production graph is replaced by this interface unit; canonical docking U4-U9 remains separate.

Profiler-disabled Core and the selected lifecycle/frame-scheduling/power/View public consumers
pass 1,167/1,167 without skips (`e6ed6761-bc35-4c0b-aea9-d1e21a06c9ca`). All five new public
View workflows execute again in this configuration, alongside the original cache, candidate,
subscription and lifecycle suite. Instrumentation is not required for their runtime behavior.

Locked whole-workspace and independent-consumer all-target checks, affected all-feature correctness
Clippy, root/independent formatting and actual Windows Rust 1.92.0 framework/Canvas/consumer
all-target/all-feature checks pass. Existing import-boundary, public-API, document-link and release
document scans pass. The local Verify workflow includes the new View consumer in public and both
independent configurations; its YAML parses. No remote workflow execution is claimed.

Stable production wasm checks pass with profiling disabled and enabled; the existing nightly
shared-memory/build-std configuration also compiles. These are production-library compilation
checks, not browser/test-support execution or foreign native-platform qualification.

Final independent review finds two P3 issues: entity conversion records a framework-internal
debug source location, and the initial public-test record overstates direct macro conversion
coverage. A source-location red control fails as expected (`91c687c3-c059-46b9-8410-b0a8af347def`).
Explicit Entity/AnyView conversions and erased cached embedding now propagate the caller through
the existing ViewElement constructor. This diagnostic-only fork adaptation extends the target's
internal-location behavior; render/cache ownership remains official. The original public props
trajectory directly exercises the derive's into_any_element override, while generic child
composition exercises into_element. Typed/erased/cached/upgraded-weak file/line checks accompany
the unchanged render/input/cache/release assertions. The earlier complete cohorts precede this
bounded metadata/test correction; its final affected checks are recorded separately below.

The corrected public workflows pass 5/5 with profiling
(`04070d1d-64e7-47dd-b350-726cb7ee7735`) and 5/5 without it
(`fd580e3b-e991-4cad-9644-e2e8e005a160`), without skips. The exact native composition passes
1/1 (`5b9c7daa-57d8-47bd-bfb6-1289786a7e0b`), filtering 225 unrelated cases. Source-location
checks retain their full render/input/cache/release trajectory. No complete-cohort rerun is
inferred from these affected checks, and no speed or allocation improvement is claimed.
Actual Rust 1.92.0 affected-framework/Canvas all-target/all-feature compilation, Core correctness
Clippy and root/independent formatting pass again after this correction. All seven implementation,
source-audit and review agents close; no Cargo process owned by this unit remains running.

The generic View owner/public/native execution is complete for the scoped current-host matrix. The
complete [accessibility diagnostic/synthetic-child successor](imports/zed-gpui-upstream-sync-candidates.md#accessibility-publication-diagnostics-and-synthetic-children)
is source-mapped separately to official `2268045a11` and the framework slice of `83d4847462`.
Node provenance is debug-only; tree capture and JSON access also exist in the target's release
builds. The fork's accepted publication/rollback and diagnostic-redaction contracts need their own
reconciliation and actual consumers. No unused substitute map is introduced. Conditional hot
reload retains its separate build/runtime owner. Native foreign-platform/browser execution and
measured-cost limits remain open. Earlier complete Windows failures retain their original scope
despite this later complete passing observation.

## Measured Text Queries, Rem Units And List Autoscroll

This unit starts at `18e5f46e87f919049628cd9fd1be782c0fbaf49a`, against unchanged fixed Zed
`635026fabc38440fce5e1767d125026a17b12012`. The UI implementation is committed locally at
`e58b8b7687aadc91408657b4e550a1b7d723c0a9`, following utility prerequisite
`787ab55a8f68daab1dc48e2c80d12f4109028bdb`. It reconciles framework slices previously hidden
by product-wide candidate dispositions: List `10628c3d2c`, TextLayout `c7ad65e468`, Rems/key
regressions `ccf4058b7a`, public util parsing `479bce0995` and shell/path regressions
`a923597341`/`362035d52a`. Product search, picker, telemetry, terminal and filesystem fallback
consumers remain outside the imported framework closure. The separate parameterized benchmark
owner retains its complete context/dispatcher/report prerequisites.

Complete official List normalization walks preceding rows, measures uncached rows and clamps at
the start. Fork adaptation retains PendingAutoscroll, source validity and admitted-frame commit;
it does not restore the target's immediate prepaint mutation. The official fixed-height scenario
and new variable-height/unmeasured/start-clamp controls first execute against the old logic.
An initial test build fails on a missing AppContext trait import, then the corrected red cohort
fails all three cases (`985ec1bb-a229-40f5-8633-63df26d686be`), exposing exact negative offsets
including -500px. Production normalization is added only after that reproduction. The public
frame-scheduling trajectory uses the existing actual atlas refusal to check public scroll and
presentation facts before acceptance, followed by exact Submitted recovery. Its first compilation
exposes an invalid access to the owner's crate-private direct revision. The corrected boundary
keeps revision assertions in the List owner, including a late-invalid overshoot source, while the
external consumer reads public facts only. No test-only public accessor is introduced.

TextLayout adopts complete source-order SmallVec/Arc queries and measurement-only indexed lookup;
the fork's Rc collection, text shaping, clip and candidate owners remain. Public software consumers
measure before prepaint, query original UTF-8/newline/wrapped identities and paint the same
StyledText, with notification, remeasurement, reconstruction and retained snapshots. Independently
locked closures explicitly establish signatures rather than execution. Rems values follow the
current Window rem size; layout and independent value/signature consumers are separate. Utility
source and all original regressions match the fixed target; public parser consumers introduce no
new environment policy or dependency. Independent geometry/utility and TextLayout source reviews
report no confirmed P1/P2. Runtime and compiler qualification is in progress; source review alone
does not qualify execution, native presentation, browser or numerical costs.

The first compiled focused owner/public run passes 51/52
(`5aa9edc0-45c5-4f8c-a3a9-4ebcfe914117`): all selected List, measured-text, key and Rems
controls pass, while the new public recovery fixture fails its generation premise. Independent
review traces the same sequence: the retry submits G+1; publishing the normalized scroll requests
refresh, and TestApp's effect flush accepts G+2 without submitting it. The corrected fixture
checks accepted G+2 versus exact Submitted/nonempty/latest-attempt G+1, then submits G+2 on the
following callback. Committed row bounds distinguish the normalized layout from the old source.
The generation equalities remain exact. Additional zero-height preceding-row controls and an
internal actual-atlas rejection/recovery check retain private revision custody.

The corrected focused owner/public cohort passes 54/54
(`46b74b02-853f-4a8c-9608-07479cc932f6`), filtering 1,289 unrelated cases. This includes all
selected List, measurement-only text, key dispatch and Rems controls, actual atlas refusal and
exact private revision recovery, mixed/all-zero preceding rows and public G+1/G+2/G+3 publication.
Committed row bounds place rows 0/1/2 at -10/10/30px after normalization. Utility owner/public
tests pass 4/4 (`cc061d70-c977-44a6-87b3-8b76aa634700`), filtering 111 unrelated cases. The
public parser case covers the official ID/version behavior including quotes and empty/whitespace
values. These controls do not claim product terminal or file-existence fallback behavior.

The first public measured-text build reaches linking but fails with MSVC LNK1140 while the drive
has about 45 MiB free. No test executes. Only that invocation's resolved failed PDB is removed;
the existing target and source remain. After free space recovers to about 6 GiB, the identical
nextest command links without profile/debug/linker changes. Its first execution fails an invalid
cross-frame unwrapped-Arc identity premise (`e4718bf2-943f-4c88-af5f-78bdc344584c`). Both the
fixed target and fork move wrapped-cache hits without retaining the separate raw cache entry in
that frame; a later width change may reshape while the retained old snapshot remains valid.
The public API does not promise that raw allocation identity across frames.

The corrected public trajectory retains same-width wrapped-cache identity and zero new shaping,
same-frame query/prepaint/paint identity, and exact source, font/run/glyph/position/metric equality
after width changes. Actual backend layout_line observations report unchanged=0 and reflow=3 in
this finite scenario. It passes 1/1 (`923189bf-3010-4bc1-9dff-90ea687a40df`), with real Cosmic/
Swash rasterization, notification, source replacement, refresh, reconstruction and retained
snapshots. The existing cache policy is preserved; this is a scoped work observation rather than
a general speedup, budget, native GPU or cross-App painting claim. Broader gates remain in progress.

Complete all-feature Core library and public integrations pass 1,390/1,390 without skips
(`c2afe594-850d-427d-9bf0-e3d0fb635226`), including the existing View, cached dependency,
candidate/unwind, input, accessibility and lifecycle owners. Complete utility library/public tests
pass 115/115 without skips (`323db97f-776d-4753-92e2-4eb733f03e82`). Existing import-boundary,
public-API, document-link and release-document scans pass; the updated Verify YAML parses. Runtime
WGPU/native and remaining consumer/compiler gates retain their separate in-progress scope.
The independently reviewable utility implementation is committed locally at
`787ab55a8f68daab1dc48e2c80d12f4109028bdb` after that complete utility result.

Complete WGPU library/public tests pass 89/89 without skips
(`6a487f8a-7028-45b6-a82a-cbbcfc0a633f`), including the new measured-text workflow and existing
paragraph/layer/wrapping/window-text, image and renderer/atlas controls. The affected Windows
native selection passes 5/5 (`fa960185-12e8-4bd8-b079-c43e9cf50c9b`), filtering 221 unrelated
cases. It preserves native Unicode layout/redraw, cached dependencies, frame-signal/profiler and
generic-view composition, exact submission and owner retirement. These are existing native
regressions alongside the new software query consumer, not a new native-query or complete
Windows-suite claim. Remaining component/independent/disabled/compiler gates are in progress.

Complete affected UI core/components/docking/Canvas tests pass 2,966/2,966 without skips
(`76c636c8-1b57-4613-b259-4882877c0873`). Existing virtualizer/list measurement, scroll,
semantic, overlay, host/payload/viewport recovery and dependency-boundary assertions remain.
This regression gate changes no canonical docking U4-U9 completion status.

The independently locked ordinary dependency fixture passes 56/56 with all features
(`605480f3-ed81-44c6-814c-12c591fe538d`) and 41/41 without default features
(`e7c514da-7e12-4b6d-b1e3-5e4c63f36f35`), without skips. The new value/style/query-signature
targets need no test-support or dependency changes. Their unexecuted Window/query closures remain
signature controls; actual lifecycle and paint results belong to the separate workflows above.

Profiler-disabled Core and the selected public lifecycle/frame-scheduling/power/View/Rems
consumers pass 1,181/1,181 without skips (`d03b5d5f-1cd4-4991-9c0c-de20677e7df4`). The
List source/atlas refusal and exact revision recovery, measurement-only queries and actual public
scroll/rem-size workflows execute in this configuration too. No timing feature is required for
the adopted behavior. All seven implementation/review agents close.

Locked whole-workspace and independent-consumer all-target checks pass. Affected Core/WGPU/
Windows/Canvas/utility all-feature correctness Clippy and actual Windows Rust 1.92.0 framework/
consumer all-target/all-feature checks pass. Stable production wasm compiles with profiling off/on;
the existing nightly shared-memory/build-std configuration also compiles. Root/independent fmt,
existing import/API/document/release scans and Verify YAML parsing pass. No dependency, lockfile,
compiler-floor or engine-default change is needed. No owned Cargo process remains active.

This closes the recorded query/List unit within its exact current-host/software/native-regression
scope. Foreign native/browser execution, numerical frame/memory budgets, complete framework
refresh, accessibility/benchmark/report/hot-reload owners, canonical docking and Nara GPU/View
adoption remain open. Source dispositions and earlier failures retain their original scope.

## Synthetic Accessibility And Explicit Published-Tree Inspection

The November 7 successor starts at `aef505a3ba9c1d2e6b4fcf079f2298aa1e239235` against
fixed Zed `635026fabc38440fce5e1767d125026a17b12012`. Official framework sources are
`83d4847462e57a6f72dc314abea05d57d3bdcfb6` (synthetic children) and
`2268045a119030735f762e5afaf59da0bda869f4` (complete raw JSON inspection and provenance).
The latter's serializer is unchanged at the fixed target. Zed Settings and editor-specific
dump actions are outside the framework closure; the Foundation Gallery supplies a real copy
consumer through the existing clipboard authority.

The implementation is committed locally at `751e563eb89eeb373dac41cf734bfe372b81d0e4`;
final qualification finishes on November 8 on the current Windows 11 host. The fixed target
and unknown exact original import checkout retain their earlier provenance boundaries.

`Element::a11y_synthetic_children`, the stateful Div callback and `A11ySubtreeBuilder` expose
post-prepaint semantic children and parent mutation. The fork uses its existing builder for
leaf admission, source validity, deferred parentage, modal filtering and final reference repair.
Prepaint rollback restores the complete current parent node because synthetic callbacks may
change more than its child count. Debug provenance is removed on rollback and filtered against
the final published node set. The common entity-backed View path records creating view names
for typed and erased views; this is a fork adaptation beyond the official AnyView writer.

The complete raw reader remains available in release builds. Capture happens at accepted
publication instead of speculative `end_frame`; metadata adds accepted `frame_generation`
and `activation_generation` without redefining the official monotonic capture `frame_number`.
Window title and active unique tab-stop count come from their existing owners. Activation
replacement, deactivation, interaction quiescence and close clear the retained snapshot.
[ADR0023](adr/0023-open-gpui-live-region-announcement-authority.md#explicit-published-tree-inspection)
distinguishes this content-bearing explicit reader/copy action from automatic redacted
announcement diagnostics and DevTools artifacts. Reading does not activate accessibility,
redraw, save or transmit content.

The initial Rust 1.95.0 all-feature Core library check passes. Its lock update adds only
`enumn` 0.1.14 and the AccessKit dependency edge, retaining AccessKit 0.24.1 and every existing
resolved dependency version. Runtime, public, native, independent and release qualification is
in progress; no platform consumption, numerical-cost or whole-refresh claim follows.

Initial focused serializer/existing A11y/atlas controls pass 48/48
(`8610d6f0-b3d1-4a81-98d2-203182cf73fe`). The first crate-external build rejects private
Pixels field access; the consumer is corrected to its public numeric conversion. Four actual
public workflows and the initial parent-rewrite control then pass 5/5
(`993148ed-d539-42ee-b05f-52179234fe82`). That ordinary append control does not exercise
the deferred-parent mutation journal and is not presented as a reproduced journal failure.

Independent review identifies irreversible-quiescence recapture and Div own-overflow validity
gaps. A stronger deferred-child control also exposes rollback attempting to remove a child
already deleted through arbitrary parent mutation. All three concrete regressions execute and
fail before their corrections (`f1504090-b0bb-4651-969d-2df74d0057b6`, 0/3). The owning fixes
keep inspection disabled after irreversible close/quiescence, attach the prepared clip's
validity to synthetic children without replacing inherited validity, and let the saved parent
snapshot restore its complete node while prior nonsnapshotted parents retain journal rollback.
The normal platform cleanup tree can still publish without recreating an inspection snapshot.

The corrected focused cohort passes 88/88
(`6ff300f6-8f41-4f0f-b21a-4de099876af5`), filtering 1,268 unrelated cases. It includes all new
owner/public workflows, existing clip/A11y/atlas controls, actual late paint invalidation,
candidate discard and atlas refusal/recovery, nested parent/provenance rollback, raw announcement
inspection versus metadata diagnostics, lifecycle clearing, view-cache controls and draw unwind.
Complete product/compiler/native qualification remains separate and in progress.

After the reserved-root correction and public numeric conversion cleanup, the complete
all-feature Core library and all public integration targets pass 1,412/1,412 without skips
(`762503b7-c3f7-4ac8-98fb-9c030878e190`). This includes the root collision control, all actual
synthetic/inspection workflows, prior candidate/unwind/focus/geometry/lifecycle/cache owners,
and the existing retired public-surface compile-fail controls. The earlier focused/native
results retain their pre-final-correction scope until their affected reruns below.

The actual Foundation Gallery selection passes 13/13
(`33c8a3ee-f213-49c4-9378-ebe13bb562f4`), filtering 123 unrelated cases. The two new click
workflows preserve the clipboard when no current tree exists, copy the accepted tree's complete
IDs/semantic values, include an explicit authored canary, preserve password masking, and leave
the automatic DevTools capture/history/export artifacts free of that canary. Existing field,
selection, live-region, busy/alert, repeated-text and inactive-drop workflows remain intact.
Independent bounded correctness re-review closes the reported failures without remaining
confirmed P1/P2, including Drawable attribution after nested View/deferred work. All eight
implementation/test/source-audit/review threads close; their static review is separate from the
executed test results. Native and compiler/consumer qualification is still in progress.

Qualification reuses Nara's ordinary target with one build job. Before further native/Core
linking, exact superseded Core/Gallery/Windows/independent/consumer test PDBs from this workstream
are removed in bounded batches after checking their resolved build-directory paths and that
their old test executables are not running. Only explicitly enumerated old symbol files are
removed while shared-disk free space fluctuates. Source, current binaries, dependency caches,
profiles and linker settings are retained; no Cargo clean or directory-wide removal is used.

The new Windows native trajectory passes with the affected selection, 6/6
(`2e852db6-da06-48df-9174-c82c920c6480`). It invokes the production installed activation
handler outside App borrows, observes accepted-tree JSON beside exact nonempty Submitted
receipts, notifies new Unicode content with stable synthetic IDs, checks passive repeated reads,
and verifies exact HWND/native/App/text/OLE release. Activation is synthetic; no real AT client,
UIA-event delivery, OS automation or compositor-consumption claim follows. Existing text,
cache, profiler/signal and scene-color native trajectories retain their assertions. This run
precedes the final reserved-root correction below; final affected checks follow separately.

A final public-builder boundary control exposes that the inherited builder did not reserve
the window root in its duplicate-ID set (`423b8813-ee5e-41d9-8fbb-f4467e189478`, 0/1).
The fix reserves that existing root at begin_frame, so a synthetic root collision uses the
same duplicate rejection as every other node. No second ID authority is introduced.

The exact serializer source is also compiled into a temporary module test harness with
`rustc +1.95.0 -C debug-assertions=no`, reusing the real framework geometry/shared-string and
resolved dependencies. All six non-debug serializer tests pass, including raw semantic export
without provenance, metadata, reference remapping and clear/counter behavior. Initial harness
attempts expose private Pixels access and then missing Cargo-supplied native library paths;
public numeric conversions and explicit existing native search paths correct the harness.
This is an executed non-debug serializer slice, not a release-profile whole App/native test,
compiler-floor result or performance measurement. Complete debug product gates follow.

The retained accepted tree is a complete snapshot while accessibility is active; complete parent
rollback can clone parent properties/children at a checkpoint. Neither this snapshot cost nor
the capture/provenance/JSON work has a numerical allocation, retention or tail-latency budget
from these correctness tests. Preserve that measurement scope for the benchmark/report owner.

Complete all-feature UI core/components/docking/Canvas library and integration tests pass
2,966/2,966 without skips (`159cd85d-bd1c-4d13-aba4-2fe24fd031a1`). Existing final-tree,
virtualized collection, text/field/selection, overlay, source/payload/viewport and recovery
contracts retain their assertions. This regression qualification does not complete canonical
docking U4-U9 or select a Nara Editor/game-runtime toolkit.

The final all-feature WGPU library/public and affected Windows selection passes 96/96
(`0b090e30-a879-4121-a9fe-40b17291e739`), filtering 1,632 unrelated cases. WGPU contributes
89 complete renderer/atlas/text/image/layout cases; seven native trajectories include exact
synthetic inspection, generic View composition, Unicode layout, cached notification, frame
signal/profiler and scene-color submission/retirement. This follows the reserved-root and
numeric-conversion corrections. It is an affected native selection, not the full Windows or
foreign-platform/browser gate, real screen-reader delivery or a numerical-cost qualification.

Profiler-disabled Core and public accessibility/lifecycle/frame-scheduling/power/View/Rems
consumers pass 1,203/1,203 without skips (`b4172ea9-b05e-4b33-8e09-8277f82f5cd2`). The new
synthetic/inspection paths, source attribution, rollback/filtering, atlas recovery and lifecycle
controls execute without requiring the profiler. Independent ordinary-consumer and compiler
qualification follow separately; this runtime gate does not borrow their coverage.

The independently locked ordinary consumer's all-target/all-feature check passes. Its two new
type-only API checks pass with profiling enabled (`1caa97ac-a5f0-4d41-abba-c6145eccf4ec`, 2/2)
and disabled (`2b952819-3d1f-42b1-b915-216ed21fe3f7`, 2/2), without test-support or an added
direct AccessKit dependency. Deliberately unexecuted Window/builder closures establish public
signatures and composition, not runtime publication. The full runtime evidence stays with the
separate workflows above. Both lockfiles add only enumn 0.1.14 and its AccessKit edge; existing
versions, including each lockfile's Syn patch, remain unchanged. Broader compiler checks follow.

Final locked whole-workspace and independent-consumer all-target checks pass; the independent
fixture compiles with all features and without default features. Core/WGPU/Windows/Gallery
all-target/all-feature correctness Clippy passes. Actual Windows Rust 1.92.0 checks pass for
Core/WGPU/Windows/Canvas and the independently locked all-target/all-feature consumer. Stable
production wasm compiles with profiling off/on; the existing nightly shared-memory/build-std
configuration also compiles. No RUSTC_BOOTSTRAP or compiler-floor increase is used. Root and
independent formatting, import/API/document/release scans and Verify YAML parsing pass. These
are local executed gates; no hosted CI run or remote publication follows.

This closes the recorded synthetic-child/explicit-inspection unit in its stated scope. The
non-debug serializer slice does not establish a release-profile whole App/native run. Initial
activation-tree delivery and official composite focus/keyboard/tab-group/property/configuration
mechanisms retain explicit dispositions in the candidate queue. Complete benchmark/report/hot
reload, foreign native/browser execution, real AT consumption, numerical allocation/retention/
tail-latency budgets, canonical docking U4-U9 and Nara GPU/View selection remain open. All eight
threads close and no owned Cargo process remains active. Documentation closure needs document
checks rather than another runtime cycle when no source changes follow.

## Composite Focus And Keyboard Continuity

The November 8 successor starts at `156d0b8c1835108223bcf9fdd1f7d7c39a396013`, against
unchanged fixed Zed `635026fabc38440fce5e1767d125026a17b12012`. Official framework
sources are `83d4847462e57a6f72dc314abea05d57d3bdcfb6` and
`2268045a119030735f762e5afaf59da0bda869f4`. Their composite-focus declaration,
real focused-ancestor gate, focus-generation keyboard arm, tab-group insertion and missing
focus-node diagnostics are the baseline. Settings/product migrations remain outside the
framework closure; property/configuration and synchronous initial activation-tree behavior
remain separate successors.

The implementation is committed locally at `53decdc64a2d182fad1fbf00ababcd2b55a906e7`.
Qualification reuses Nara's ordinary target serially with one build job. No dependency,
lockfile, declared compiler floor or engine default changes in this unit.

The fork resolves active descendants from the final surviving semantic topology, after source
validity, late geometry, modal filtering and deferred parentage. Its explicit accessibility-only
focus override does not grant real keyboard ancestry and retains priority when different from
the real focus. Inspection retains the original semantic focus separately from the selected
active descendant. Diagnostics retain only focus identity and static missing-ID/role reasons;
dedup/reset follows accepted publication.

The fork's candidate acceptance requires two adaptations to the official keyboard mechanism.
Actual input-time focus moves advance the Window-owned generation immediately, preserving ABA
invalidation between draws. Provisional candidate focus advances it only when accepted; claims
that reassert the same leaf do not count as moves. The existing key-event revision also rejects
an intervening dispatch that an interceptor or capture listener stopped before the element.
The official shared Enter/Space marker is retained, including adjacent Enter-down/Space-up.
This unit does not introduce a key-identity restriction or a second focus owner.

The initial all-feature production Core library check passes after rebuilding the user's
cleared target. The first concrete owner regression executes and fails before correction
(`dee1f37b-23cb-44e0-9df9-5c4ee4c61681`, 0/1): an enclosing failed prepaint transaction
resurrects focus after irreversible interaction quiescence. This is a local rollback authority
gap, not an official focus-generation behavior to remove. The actual atlas-refusal cohort then
passes its three accepted inert/removal/same-handle-rebinding controls but fails the refused
inert candidate (`083b5a61-36ca-480b-b3ee-abf0e3340722`, 3/4): eager suppression clears
the arm still owned by the previous accepted frame, so the visible retry loses its click.

Local rollback now preserves irreversible quiescence and its focus-claim cleanup. Window keeps
the actual leaf last counted by its generation, separately from provisional focus intent, so
direct or App-deferred quiescence counts the actual loss even after a candidate drops the old
binding. Provisional-only intent does not invent a committed transition, and the established
post-borrow cleanup schedule is preserved.
Keyboard-arm retirement uses existing accepted prepaint publications rather than mutating shared
accepted state while building. Complete public, native, ordinary-consumer and compiler
qualification are still in progress.

The first complete focused build exposes an unconstrained public callback event parameter;
the consumer now names `TargetedEvent<ClickEvent>` explicitly. Source comparison also corrects
the public consumer's overly strict cross-activation-key expectation to the official shared
marker, retaining the orphan, intervening-key, focus-ABA and one-shot assertions. The corrected
owner/public cohort passes 104/104 (`b7988642-48ab-4213-b3e0-d813edc2d02f`), filtering
1,291 unrelated cases. It includes direct/cached Enter/Space atlas refusal and accepted
retirement, App-deferred quiescence, candidate rollback/unwind, explicit override priority,
final semantic parentage, source notification, Tab groups and exact root release.

Bounded independent re-review closes the eager-arm-clear and deferred-quiescence counting
findings without remaining confirmed P1/P2. All seven implementation/test/audit/review threads
are closed. Static review and TestPlatform publication are separate from native Submitted,
real assistive technology, ordinary dependency features and numerical-cost qualification.

The new native Windows trajectory passes 1/1
(`27d9d68d-bb1e-4b4c-a48b-c3f3e22b0296`), filtering 227 other Windows cases. Synthetic
keys invoke the installed production native input callback outside App borrows; each records
one exact delivered input and key revision. Synthetic accessibility activation, Menu/ListBox
selection and source notification agree with the accepted tree/JSON beside nonempty actual
Submitted generations. The trajectory retains outsider/Tab/focus-ABA/stopped-key/close-on-down
controls, passive reads and exact HWND/native/root/App/TextSystem/OLE release. This is native
renderer and callback execution, not OS-generated input, a real AT client, UIA event receipt,
physical foreground focus or compositor scanout. Complete affected regression gates follow.

Complete all-feature Core/WGPU/Windows library and integration tests pass 1,770/1,770
(`1fa554ca-19dd-4d9a-b976-bcdc2e3fa02e`), with four existing interactive Windows cases
skipped. The retired-interface compile-fail control rebuilds its separate dependencies after
the user's target cleanup and passes without changing its assertion. All other Core/public,
renderer and native scenarios, including the new composite trajectory, retain their assertions.
This complete runtime cohort does not replace component, disabled, ordinary-consumer or
compiler qualification below.

Complete all-feature UI core/components/docking/Canvas library and integration tests pass
2,966/2,966 without skips (`fbb00dd8-694d-41e9-9a71-03fc0e5e7373`). Virtualized models,
overlay and field behavior, workspace movement and viewport lifecycle retain their original
assertions. This regression gate neither completes canonical docking U4-U9 nor establishes a
new Nara toolkit default or numerical-cost result.

The profiler-disabled affected owner/public cohort passes 111/111 without failures
(`1dbd6755-6af9-4a0d-8f9d-ef8d1e91ae85`), filtering 1,115 unrelated cases. All new
focus/keyboard controls, accessibility owners, existing atlas/unwind controls and both public
accessibility targets execute without profiling. This is an affected selection rather than a
repeat of the full disabled Core matrix; the preceding all-feature full result retains its
separate scope.

The independently locked ordinary consumer's accessibility/composite targets pass 4/4 with
all features (`a5b6be85-5dbe-4e24-8986-06e6e8d0a425`) and 4/4 without default features
(`5cfdb6a9-4e2e-4d28-a587-cfe5a6ee7803`). The two new composite checks deliberately leave
their Window/builder closures unexecuted and require neither test-support nor a direct AccessKit
dependency. They qualify public signatures and ordinary dependency features; they do not borrow
the real TestPlatform or Windows runtime coverage above. Both independent lock and package
versions remain unchanged.

The actual Gallery focus/accessibility selection passes 13/13
(`f62ddf60-b6fc-4e8a-8b33-b0e7ed0a8a5f`), filtering 123 unrelated cases. Existing text
value/selection, field help/error, live-region/busy/alert, repeated announcement, explicit tree
copy, password and redacted-artifact workflows keep their original assertions. This is the
affected Gallery selection, not a full Gallery or real AT gate. No runtime failure remains in
the executed cohorts; compiler and static gates are recorded below.

Locked Rust 1.95.0 whole-workspace all-target compilation and the independently locked
consumer's all-target/all-feature compilation pass. Correctness Clippy passes all targets and
all features for the affected Core/Windows owners with `-D clippy::correctness`. Existing style,
test unwrap and docking dead-code warnings remain; these are not warning-free checks.

Actual Windows Rust 1.92.0 all-target/all-feature checks pass for Core/Windows and the
independently locked consumer. The declared floor remains 1.92; the fixed source's newer
toolchain is not copied as a new requirement. These are Windows compilation gates, not
execution on Linux/macOS or a performance claim. Production wasm and final static checks
are recorded separately below.

Stable Rust 1.95.0 production Core/WGPU/Web libraries compile for
`wasm32-unknown-unknown` with profiling disabled and enabled. Browser/test-support execution and
real AT are not qualified by these checks. The optional nightly shared-memory configuration and
the non-debug whole App/native path are not rerun in this unit; earlier evidence retains its
original source scope. No scheduler, Web dispatcher or shared-memory implementation changes.
The rebuilt existing xtask passes import-boundary, public-API-tier, public-document-link and
release-document scans. The edited verification workflow also parses as YAML. No hosted CI,
remote publication or new scanner follows. Root and independently locked consumer format checks
pass. All owned Cargo invocations have terminated; no runtime re-execution is needed for the
following documentation-only qualification commit.

Active-descendant resolution creates final ID/parent maps only when claims and a real focused
node require them. Each claim's ancestor walk is bounded by the final node count. These
correctness tests establish neither allocation/retention budgets nor a latency improvement;
large composite-tree costs remain part of the separate benchmark/report owner. Non-debug
self/multiple-claim branches remain source-covered by conditional tests but are not executed
under a non-debug whole App configuration in this unit.

## Accessibility Properties, Configuration And Initial Trees

The November 8 successor starts at `37ceac021fec7efb5091334bc60877ba36ab75d6`, against
unchanged fixed Zed `635026fabc38440fce5e1767d125026a17b12012`. Official sources are
`83d4847462` (numeric step and initial root), `2268045a11` (shortcuts),
`9183ba4c5579c083b005d7f4c248d90c914fb46f` (enabled/active and test configuration), and
`cc053a4a6fa2fd0e8793201ed9099466af1be0b1` (author identifiers). The complete official
builders/writers use the existing grouped property owner. The official query names replace
the fork's `is_accessibility_active` without an alias; first-party consumers migrate together.
Local implementation is `d9608b1a3174f17df270d6b92894539ec32654a8`.

The initial callback returns the official immutable root-only creation-title snapshot before
the existing sequenced full refresh. No synchronous App borrow, draw, action publication,
inspection capture or successful frame receipt is inferred from that reply. A retained callback
reads the original Window interaction authority through its shared atomic admission bit and the
existing native-close fact. Exact unregistration invalidates admission; a native-close callback
sets its native fact without changing queued command settlement. The existing ingress shutdown fence
moves into its shared sequencer state so callback threads can refuse activation before native
termination, without reordering quit observers or captured-pointer release. Backend native and
provisional gates remain intact. Overlapping calls retain their actual admission point; no
new registry or globally atomic native-teardown/reply transaction is introduced.

The explicit raw inspector also exposes published `author_id`, an additive fork extension to
the fixed target's serializer. Automatic DevTools artifacts remain governed by existing
metadata allowlists. Actual public/native controls cover source-driven replacement/removal,
passive configuration, enabled-before-attachment subscriptions, force-disabled future windows,
root reconstruction, repeated activation/title updates and exact release. Their source alone
does not qualify execution. The initial all-feature production Core check passes; runtime,
native, ordinary-consumer and compiler qualification is in progress.

The first affected Core/public cohort executes 147 cases, passing 145 and failing two
(`93b39ccd-9438-4721-a714-e7bcbf950aec`). One old lifecycle history assertion omits the
new synchronous root. The other attempts to wake the deterministic TestScheduler from a real
OS thread, which that scheduler deliberately rejects. The corrected CPU case invokes the
original installed callback while App/Window are borrowed; the live worker-thread/borrowed-App
proof moves to the real Windows dispatcher. The deterministic scheduler contract is retained.
Reserved/rollback, native-close, retained-identity, quiescence and shutdown-fence controls pass
in the first cohort; all five new public workflows also pass. These partial results do not
replace the corrected complete affected run.

Source review also removes an unneeded property-writer divergence: absent properties retain
preexisting custom node content, matching the official writer. Normal fresh-node reconstruction
removes omitted source properties in the complete public workflow. Bounded independent review
finds no confirmed P1/P2 in the shared admission, retirement and shutdown-fence adaptations.
The initial four implementation/test/audit/review threads are closed. New synchronous replies and final
published trees retain separate native/AT/cost evidence requirements.

The corrected affected Core/public cohort passes 147/147
(`28531cc2-d267-41c5-ab80-6f4ce0df09ab`), filtering 1,271 unrelated cases. It retains
the original complete source-update/removal, action generation, geometry, atlas, announcement,
composite focus and lifecycle controls. A subsequent direct atlas-refusal/initial-root control
is added before complete regression qualification; its source is not counted as executed yet.

Both affected native trajectories pass 2/2
(`c08b9563-ab48-4c6b-b941-e549a524fcda`), filtering 226 unrelated Windows cases. The
extended original synthetic-accessibility trajectory checks passive enabled/active queries,
the exact synchronous creation-title root before full publication, complete authored property
update/removal, stable node identity and newer nonempty Submitted generations. Repeated
activation executes the original installed Send callback on a real OS worker while App and
Window are borrowed; it returns without rendering or publishing a frame, then the Windows
dispatcher delivers the complete live-title tree. Original provenance, source, raw inspection,
privacy and exact HWND/native/root/App/TextSystem/OLE retirement assertions remain. This is
synthetic activation and native callback/render execution, not a real AT/UIA client or OS input
and compositor-consumption qualification. The prior composite-keyboard trajectory also retains
all controls after migrating its initial-root expectation.

The first complete Core/public/WGPU/Windows run executes 1,787 cases, passing 1,782 and failing
five (`154e08d0-4a54-44fe-aac2-484c7c795f07`), with four preexisting interactive skips.
The new actual atlas-refusal/initial-root test passes. Four failures share an old public
inspection helper that equates platform tree count with accepted inspection capture count.
Its corrected fixture oracle counts its live-title publications, validates every creation-root
bootstrap separately and retains accepted empty-root reconstruction.

The fifth failure is an actual native regression: early generic admission revocation at
`on_close` changes an older queued activation's exact target-loss terminal from Closed to
Rejected. The fix restores original command timing and shares the existing `native_closed`
fact atomically with the Send activation callback instead. No new lifecycle owner is added.
Direct retained accessibility replies still refuse native-terminal windows, while the exact
queued platform command observes WindowClosed through its original dispatcher. Corrected
focused/native and complete reruns follow; the failed cohort is not reported as qualification.

The final-correction Core/public selection passes 13/13
(`32e6cc2c-835d-45d2-bae5-ad126dc7e848`), filtering 1,392 unrelated cases. It executes the
actual atlas refusal, native-close-before-logical-cleanup, original retained callback after
rollback/quiescence/replacement/shutdown, and all seven original public inspection workflows,
including accepted empty-root reconstruction. Bounded re-review finds no confirmed P1/P2 in
the native-close correction; all six implementation/test/review threads are closed. The exact
native terminal control and complete product/compiler matrices follow separately.

The corrected actual Windows activation/inspection and exact target-loss terminal controls
pass 2/2 (`08718bbf-037d-41a6-8076-13737f62ad9f`), filtering 226 unrelated cases. The
original queued command now observes Closed and settles WindowClosed exactly once, while the
retained accessibility callback still refuses a natively closed window. The production source
is stable before the final complete regression and compiler qualification.

The first full rerun on that stable production source passes 1,786/1,787
(`e7659da6-a517-4450-8ec3-6b6bcdd0b43b`), with four preexisting skips. Every corrected
inspection and activation-terminal scenario passes. One unchanged native mouse-leave fixture
receives four input/hover pairs where it expects three, while preserving pair ordering. Its
unchanged isolated rerun passes 1/1 (`32033417-c365-455c-bc48-0fcf4a2e3f37`). The cause of
the extra native transition is undiagnosed; neither assertion nor source is changed to suppress
it. A further complete rerun retains this earlier failure alongside its actual result.

The unchanged complete Core/public/WGPU/Windows rerun passes 1,787/1,787
(`e6f521d4-9aa9-4edd-903d-1dbc361bc1ed`), with four preexisting interactive skips. The
mouse-leave fixture, exact native activation terminal, all original public inspection workflows,
new property/configuration/initial-root consumers, actual atlas refusal and complete lifecycle
cohort pass together. The earlier extra-native-event cause remains undiagnosed, rather than
being converted into a claim that the rerun fixed it. No further product code change follows
this complete passing result; affected component/compiler/consumer gates remain separate.

The complete all-feature UI core/components/docking/Canvas library and integration cohort
passes 2,966/2,966 (`47011b6a-e4cd-40ca-a2e8-28f9a2eb0e3e`), with no skips. Existing
virtualized controls, final accessibility trees, text input, floating/cross-window docking,
retirement and Canvas consumers execute against the same stable Core source. Disabled,
ordinary-dependency, Gallery and compiler gates follow separately.

The profiler-disabled affected Core/public selection passes 182/182
(`288ba895-c674-4b40-ad97-13634308b33d`), filtering 1,061 unrelated cases. It executes
all three public accessibility configuration/tree/composite targets, together with A11y,
ordered native ingress, interaction authority, atlas refusal and shutdown controls. This
runtime selection uses `test-support`; ordinary dependencies remain a separate gate.

The independently locked ordinary-dependency configuration/tree/composite targets pass 6/6
with all features (`10a3dc2c-fd8d-408b-985c-c9ec0eec9677`) and 6/6 without default features
(`55638945-2e47-4f15-9809-04cda649cb8e`). These deliberately type-only consumers compile the
public builders, queries, subscriptions, custom elements, synthetic children and keyboard/focus
interfaces without test-support or a direct AccessKit dependency. Their closures are not runtime,
input, rendering or AT qualification; those scopes retain the separate public/native evidence above.

The actual Gallery focus/accessibility selection passes 13/13
(`2d7b7998-faed-4afe-9098-3cccc06f564d`), filtering 123 unrelated cases. It retains complete
field/tree relations, publication-driven announcements, password/privacy controls, explicit raw
tree copy and unavailable-tree clipboard preservation after the query-name migration.

Locked workspace all-target and independently locked consumer all-target/all-feature checks pass
on Rust 1.95.0. Affected Core/Windows correctness Clippy passes. Actual Windows Rust 1.92.0
Core/Windows and independently locked consumer all-target/all-feature checks pass, preserving the
declared compiler floor. Stable production wasm Core/WGPU/Web checks pass with profiling off
and on. Root/independent formatting, import-boundary, public-API-tier, public-document-link and
release-document checks pass; the changed workflow parses as YAML. No dependency, lockfile,
compiler-floor or remote-publication change follows. The workflow change is locally checked;
remote CI is not executed.

All six implementation/test/review threads and one subsequent read-only benchmark audit are
closed. That audit maps missing harness/report prerequisites in the existing candidate queue
and plan; it supplies source evidence only. The complete benchmark/report, conditional hot
reload, numerical budgets, canonical docking U4-U9 and Nara GPU/View adoption remain open.
This unit does not qualify real AT/UIA consumption, OS-generated input, foreign native/browser
execution or compositor scanout. Earlier failures and the undiagnosed extra mouse transition
remain recorded alongside the final passing results.

## Threaded Dispatcher And Queue Closure

The November 8 benchmark prerequisite starts at
`1670c373e34751ac520237b7d04a480b06ab7b4a`, against unchanged fixed Zed
`635026fabc38440fce5e1767d125026a17b12012`. Official `3135cdf8dc` supplies priority-queue
and Linux timer-loop closure; the concrete dispatcher follows `8886dcb0d4` and the fixed
target's cancellation/readiness/frame-service follow-ups. Existing PlatformDispatcher,
executors, priority queue and scoped profiler remain the owners. No benchmark context, font
injection, renderer submission or complete report behavior is qualified by this prerequisite.
Local implementation is `018fbdc5f708d91f0ae16a7fb43ac04280ebb680`.

The original blocked-receiver regression fails on the old implementation: 0/1
(`782e539e-c2b2-4df1-b947-36025a4061e4`). After the final sender is dropped, two waiting
receivers do not complete within the original five-second timeout. The official fix synchronizes
with the queue lock before waking all receivers and rechecks sender closure after every wake;
it preserves already queued values and existing priority. The corrected test and complete
dispatcher/public/runtime qualification follow separately.

The all-feature production Core check passes. The first affected test build stops at an
untyped disconnected-observer channel in the new panic test (`E0282`); annotating its unit
message type corrects the fixture without changing production code. The corrected focused
cohort follows. Existing import-boundary/public-API scans, document links and changed-workflow
YAML parsing pass at this checkpoint. Linux source and new test are independently reviewed,
without claiming active Linux compilation or execution.

Independent review identifies a new cancellation-test race before execution: a timer thread can
legitimately acquire the lock before a destructor's one-shot `try_lock`, whose assertion can
then abort through async-task's destructor handling. The owned build is stopped before execution.
The corrected fixture uses bounded lock acquisition and reports its result from the test body,
preserving rearm and the original 1/1/0 cancellation counts. Panic coverage now proves an actual
poll, and nested unwind retains an independently queued sentinel to detect premature Idle.
Production dispatcher code is unchanged by these corrections.

The corrected all-feature queue/dispatcher/executor/scheduler/profiler and public-runtime cohort
passes 182/182 (`10acfe9c-8ebd-4f9e-9876-29199e7205cb`), filtering 1,239 unrelated cases.
The original blocked-receiver regression and
all eight upstream dispatcher scenarios pass alongside two real crate-external execution
trajectories. Subsequent source review aligns queue reexports with the same support gate as
its module, including existing fork scheduler tests on macOS. Windows behavior is unchanged;
complete final-source and disabled/compiler qualification follow separately.

Bounded independent re-review closes the cancellation-fixture race and confirms the strengthened
panic/queued-sentinel controls. Queue/Linux source and core support gates have a separate
correctness review. All three implementation and two review threads are closed. Reviews remain
source evidence alongside the separately executed focused suite and later complete gates.

The complete final-source all-feature Core/public/WGPU/Windows cohort passes 1,810/1,810
(`ef3c4394-51a1-47c5-8ffe-f5edaaec5c25`), with four preexisting interactive skips. The
new threaded/public controls execute alongside original native submitted-frame, queued command,
input, cache, text, power and exact-retirement trajectories. This final-source cohort passes
without an isolated runtime retry. Component, disabled and ordinary-consumer gates follow.
After this result, the remaining serial sequence is adjusted at its stage boundary: the
independent fixture is a bin package and must select `--tests` without `--lib`. No fixture
command using the incorrect library selection is executed; no product source changes follow.

The complete all-feature UI core/components/docking/Canvas library and integration cohort
passes 2,966/2,966 (`228767d8-9fde-4bd3-973a-d8924f8129e5`), with no skips. Existing
component semantics, virtualized controls, final-tree publication, floating/cross-window
docking and Canvas workflows execute against the final shared Core source.

The profiler-disabled affected queue/dispatcher/executor/scheduler and public lifecycle cohort
passes 35/35 (`c3608cfb-ae75-4e1d-b9f7-0d87f68ad55c`), filtering 1,210 unrelated cases.
Both public execution trajectories pass without instrumentation. The disabled poll control
also verifies that main, worker and timer threads register no timing records and the scoped
guard remains zero-sized. This is the affected disabled selection, not an entire disabled
native or benchmark/report qualification.

The independently locked ordinary dependency tests pass 62/62 with all features
(`250f2812-5ac0-4ab9-b568-e0d7bec51e05`) and 47/47 without default features
(`26ef07ca-5692-4569-af92-ed666e4c3ff6`), with no skips. This unchanged fixture uses no
test-support or engine-private dependency. It includes executed value/layout controls and
explicitly type-only interfaces; those cases are not promoted to native, input or AT evidence.
The separate new test-support integration target supplies actual threaded handoff, timer,
cancellation, Task/Rc release and thread-affinity execution.

Locked workspace all-target and independently locked consumer all-target/all-feature checks
pass on Rust 1.95.0. Affected Core/Windows correctness Clippy passes. Actual Windows Rust
1.92.0 Core/Windows and independent consumer all-target/all-feature checks pass. Stable
production wasm Core/WGPU/Web checks pass with instrumentation off and on. The existing
nightly shared-memory configuration also passes using Rust 1.98.0-nightly
(`57d06900f`, May 27), build-std, atomics and the existing imported-memory flags; no
`RUSTC_BOOTSTRAP` workaround is used. These are production compilation results, not browser
or multithreaded runtime execution.

Root/independent formatting, import-boundary, public-API-tier, document-link and release-document
scans pass. The changed workflow parses as YAML and retains both feature modes; remote CI is
not executed. No dependency, lockfile or compiler-floor change follows. All five implementation/
review threads are closed, no owned Cargo remains active and no remote publication occurs.

The scoped prerequisite retains official process-lifetime timer reuse and existing admission/
backlog limits. Linux source and lifecycle test remain unqualified by active Linux compilation
or execution; macOS native and browser/test-support execution also retain their own gates.
Benchmark-feature separation, the old virtual context's replacement, real font/render injection,
per-routine platform isolation, measurement/report/allocator consumers and numerical costs remain
in the complete benchmark owner. Full refresh, hot reload, canonical docking U4-U9 and Nara
renderer/View adoption remain open. This unit changes no engine UI default or R11/R12 status.

## Complete Benchmark Context And Measurements

The November 8 successor continues from `916c7cb2d2fffcaa95199598655b5cef97fb9bda`
against fixed Zed `635026fabc38440fce5e1767d125026a17b12012`. It imports the complete
independent Apache-2.0 measurement owner, benchmark-only feature, threaded context/report,
parameterized macros, shared seeds and optional fallible renderer path. Official behavior is the
baseline; fork adaptation preserves accepted candidate and exact submission/resource authority.

The initial metrics/macro selection passes **27/27** without skips
(`6f7d00f6-7ea4-4f0b-b765-2037420139b5`). It includes all official measurement tests applicable
to this host, explicit Windows counter unavailability, allocator failure/reallocation accounting,
secondary normalization, macro validation and separate input/seed report expansion. A first command
was refused because `-j 1` and `--test-threads=1` both set nextest's test concurrency; the executed
command uses `CARGO_BUILD_JOBS=1` and only the latter nextest option.

Independent source review finds two inherited official defects. The new wrapper-visibility
regression fails against the imported macro (`84c6cf8c-6101-44d0-b0bd-8c70cc2898fc`); preserving
the original visibility corrects cross-module registration. A standalone Rust 1.95 test of the
actual shared seed source reproduces maximum-seed addition overflow. Count-based generation
allows a valid final `u64::MAX` and rejects an overflowing sequence before iteration. Both boundary
controls pass after correction. The complete metrics/macro selection then passes **28/28**
without skips (`07928f24-6f35-4588-990c-6d1e0749b2d7`); integrated Core seed controls and a
real module-qualified independent benchmark remain part of the full qualification below.

Actual Windows Rust 1.92.0 all-target/all-feature compilation passes for the new measurement
package and macros. This early prerequisite check does not replace full context/renderer/consumer
compiler-floor qualification.

The first integrated context/public selection compiles after correcting test-only return-type and
empty-array inference errors, then runs **40 tests: 36 pass and 4 fail**
(`58b67107-e12a-4bb8-9361-ca115d5d3592`). Public notification/resize/reconstruction, task/timer,
external callback and successive-routine consumers pass. New refusal and per-session close
scenarios expose remaining integration problems; their original assertions are retained.

Three additional independent-review findings reproduce in an isolated red selection
(`849ffe4e-ee5b-4fc6-9276-3099fd24983b`): App-destructor tasks are left queued when no timer
has yet been armed, a successful 100th cancellation still reports failure, and batched panic drops
input before closing its trace scope. The correction drains destructor work unconditionally,
checks remaining timers without cancelling a 101st round, and places the trace field before input.
The same selection retains the existing refresh-only renderer failure for diagnosis. These are
inherited official context edges, separately from the fork's terminal-generation integration.

Canvas now opts into deterministic window tests through an explicit `test-support` feature;
its dev-dependency no longer forces Core test support and leak detection into benchmarks. The
complete Canvas cohort and benchmark-only feature graph must both be checked. CI uses one global
build job and removes the invalid duplicate nextest concurrency options; no remote CI run is
implied by these local workflow edits.

The refresh-only renderer failure is a fork integration defect: the official helper assumes a
synchronous frame callback, while the fork enqueues it in ordered native ingress. Actual draw
previously occurred on a later App borrow release, outside measurement. A scoped App borrow
around each simulated callback now finishes that existing convergence path before returning.
The new regression reads the native submission count immediately inside measured work, before
any later App read, and compares complete draw samples. It does not bypass native ingress or
directly draw. Benchmark Apps use explicit quit mode across per-iteration window closes. The
existing terminal quit authority now preserves a dispatched state, accepts a late platform quit
after native terminal shutdown through post-borrow convergence, and avoids opening another fence.

Renderer source review also identifies the fork's infallible Metal intermediate-allocation helper.
It now adopts official fallible paired allocation and caller propagation, validates the owning
device's dimension limit, and rejects an invalid native resize until valid geometry recovers.
Metal regressions are source-only on this Windows host; no macOS execution or allocation-pressure
qualification is implied.

The initial injected-renderer selection runs **8 tests: 6 pass and 2 fail**
(`98cba361-074b-4016-8b85-625f480c5cb8`). Actual WGPU submission, padded-row readback, target
reuse, invalid-size recovery and atlas release pass. The public fixture needs explicit paintable
child bounds and an active, actually submitted startup baseline; its original pixel and exact
generation assertions remain intact. The combined successor runs **59 tests: 57 pass and 2 fail**
(`3f16050b-50c1-427e-b96a-02842714a3af`). Executed context/frame/unwind/timer corrections pass;
the remaining public fixtures require the startup frame delivery and a poll that explicitly exceeds
the public journal floor before asserting retained foreground work. Short-poll timing noise is not
a reliable positive-work oracle. These functional Criterion runs do not constitute matched
performance comparisons, regardless of incidental saved-baseline output.

The next 60-test run passes 59 and exposes the fixture's remaining BGRA input mismatch
(`a15e4dba-22e1-449c-b37b-efa1fb5475a8`). `RenderImage` explicitly stores BGRA bytes; the
fixture had passed RGBA red. Correcting the fixture's source bytes, while retaining the original
red screenshot assertion and the official renderer conversion, closes that failure. Final focused
context/public/renderer execution passes **60/60** without selected skips
(`1d8e78bc-94ce-4e92-8a8a-d2784b2b5c21`). This includes the late-terminal-quit unwind wake,
100/101 cancellation boundaries, refused-candidate recovery, actual WGPU pixels and exact
renderer/atlas release across reconstruction. Both selected benchmark feature graphs exclude Core
`test-support` and `leak-detection`; executable consumers and broad qualification follow.

The independently locked `maintained_harness` executable completes **9 benchmark cases** with
`--profile dev --features bench-support -- --noplot`. It executes native text shaping, two
parameterized renderer inputs, four input/seed task combinations, a single seeded task and the
module-qualified registration. Default counting-allocator reports contain actual allocation and
byte observations; no FakeCounter or test-support dependency is involved. The feature-graph check
also excludes Core leak detection. This is executed functional qualification in an unoptimized
profile; its incidental timings are not a release performance result. Windows retains the official
default unavailable renderer factory, so this binary's platform Submitted facts do not establish
GPU output; the separate injected WGPU consumer above supplies that evidence.

The maintained Canvas `large_canvas` executable passes all **5 smoke cases** under
`--profile dev --features bench-support -- --test`: index rebuild, visible query, paint-frame
culling, editor-model snapshot and label rendering. The 200-by-100 node grid, 39,900 indexed
records, labels, viewport and culling options remain intact. The label case emits actual
allocation, dirty-to-draw, draw and foreground-work reports, with unavailable Windows hardware/
resource counters reported explicitly. This one-iteration smoke is not a numeric budget or an
optimized renderer comparison. Complete component acceptance enables Canvas `test-support`
explicitly to retain its two window-driven input/clip regressions.

Complete all-feature Core/public/WGPU/Windows/platform/Gallery and metrics/macros execution passes
**2,040/2,040** (`ba398eb1-2d7d-4aab-b320-7c8b9943b9ea`). Five tests are skipped: four
pre-existing interactive Windows cases and the Gallery's fixture-regeneration maintenance case.
The full native trajectory needs no isolated runtime retry. UI core/components/docking/Canvas
passes **2,964/2,964** without skips (`b0be986b-39a7-43e1-bbd8-7cad3053c52f`), explicitly
enabling Canvas test support and retaining both window-driven input/clip cases. Comparing the
complete prior test-name set identifies two additional Docking tests gated on its own
`test-support` feature. Both pass in the supplemental run
(`a0e72175-78bc-45b7-afc8-1e0240959a0d`), with 1,473 unrelated Docking cases filtered out.
The union preserves the previous 2,966-test cohort without repeating already qualified cases.
Future complete cohort commands enable both `open-gpui-canvas/test-support` and
`open-gpui-docking/test-support` together; Core test support alone does not enable a consumer's
own feature-gated tests.

The instrumentation-disabled affected Core/public selection passes **112/112**
(`f4007354-2541-4c7b-a357-9133394aaf64`), with 1,155 unrelated cases filtered out. It retains
App/native-ingress/renderer lifecycle, seeded tests, foreground instrumentation, scheduled frames,
real threaded handoff/timers and public release controls. The independently locked consumer passes
**62/62** with all features (`2498f4cd-2564-44cc-b565-efa208c56337`) and **47/47** with
defaults disabled (`886f57b4-65db-402e-9b75-50cd59c1a6d4`), both without skips. These ordinary-dependency
value/layout and explicitly type-only controls remain separate from the executed benchmark
and injected-renderer consumers above; they do not claim to launch an independent native App.
Locked workspace all-target and independent all-target/all-feature checks pass on Rust 1.95.0.
Affected Core/WGPU/Windows/platform/Canvas/metrics/macros all-target/all-feature Clippy passes
with `-D clippy::correctness`. Actual Windows Rust 1.92.0 all-target/all-feature checks pass for
Core/WGPU/Windows/platform/Canvas/metrics/macros and the independently locked consumer. No compiler
floor increase is needed. Stable production wasm compilation passes with instrumentation off/on.
The existing optional shared-memory configuration passes on Rust 1.98.0-nightly (`57d06900f`,
May 27), using build-std and the checked-in atomics/imported-memory flags without
`RUSTC_BOOTSTRAP`. Root and independent formatting checks pass. These are production compilation
gates, not browser/test-support or foreign-native execution.

All ten implementation/test/review threads are closed. The final benchmark/context/renderer and Metal
source reviews report no unresolved P1/P2 findings in their reviewed scope. Existing import/API/
document/release scans, changed-workflow YAML/manifests and Git whitespace checks pass. Local CI
configuration changes do not imply remote CI execution or publication.

These functional gates do not establish numerical performance budgets. Linux/macOS hardware
counters and renderer execution require their owning platform evidence; Windows keeps the official
default unavailable renderer result and independently exercises an injected WGPU renderer. Official
`95cd535a5fad96d649513f96c5784ceefd379e47`'s additive test-support/leak-selection mechanism
remains a separate source-grounded successor. This unit excludes those features from benchmark
consumers without claiming that broader mechanism synchronized.

## Additive Test Support And Explicit Leak Tracking

The November 8 successor starts at `7c7a1411009a07afbcbd70340256adf55161491b` against
fixed Zed `635026fabc38440fce5e1767d125026a17b12012`. Official
`95cd535a5fad96d649513f96c5784ceefd379e47` separates test-support compilation from
entity-handle tracking and deterministic execution. The implementation follows existing
App/entity-map, scheduler/executor, util and Metal owners. The process-level test hint remains
distinct from each executor's identity; no new scheduler or lifetime owner is introduced.

An actual downstream util regression fails against the old implementation
(`ee1ab1af-5aa9-4bd4-8e67-a8cd77bba695`): enabling test support changes its home directory
to the fake own-test path. The official `cfg(test)` boundary corrects that behavior. Complete util
unit and external-consumer execution then passes **116/116** without skips
(`92487fa0-b33b-4504-b50c-e03ba8485a06`), including a separate own-test fixed-directory control.

Source review identifies an inherited tracked-mode release inconsistency. After complete entity
cleanup and destruction of the final refcount owner, an otherwise released typed/erased weak
handle still unwraps the missing owner. The actual regression fails with that unwrap
(`4fb17e78-f5fa-4886-ad71-fd1a3e1844c2`). The owning correction treats an absent owner as
released while retaining live strong and pending-cleanup refusal. Independent review also finds
the inherited consuming release diagnostic can hide still-live handles after a caught assertion.
The next focused run passes the owner-gone, AsyncApp and mixed-executor controls, but fails that
new preservation assertion: **3 pass, 1 fail** (`37972762-ceac-41b8-8b28-1beba84ee57b`).
The diagnostic now borrows its existing metadata and clones a backtrace only when resolving it;
the existing panic-safe destructor remains separate. These reproduced inherited findings are
separate from the official feature/runtime split.

The new `test-support-consumer` is independently locked and preserves actual public Test,
returning Application and real-time Bench workflows. The existing ordinary `dependency-consumer`
remains free of test support. Tracking selection uses a nonempty build-time
`GPUI_LEAK_DETECTION` or the explicit feature; nonempty `0` enables it too. Runtime
`GPUI_TEST_EXPECT_LEAK_DETECTION` is a test-only expectation, not a production switch or a cfg
inferred from a dependency. Feature-only snapshots are no-ops while weak release checks still
check actual ownership. Native View source preserves the complete input/notification/cache/
reconstruction/retirement trajectory and adds exact-generation DX11 pixels and mixed-context
identity controls. Its first build exposes two test-only private `Pixels` field accesses,
corrected to the public conversion. Actual default Windows execution then fails because the
backend's existing readback implementation is absent without its own test-support feature
(`feeeade4-c5ee-4d80-9312-1c760d48b6e2`). The native App already remains production-mode
after TestApp creation, and exact Submitted state precedes that failure. Enabling the existing
readback implementation/helpers for backend `cfg(test)` retains the complete pixel oracle and
passes the full original trajectory (`4001681e-82df-43ef-9ce2-94a149ec7d11`). It does not
enable readback for ordinary production builds.

Corrected focused Core/scheduler/public execution passes **130/130**
(`6fed4498-2fdc-4e65-833d-53c7706b52c6`), with 1,381 unrelated cases filtered out. It
includes original entity tests, all eleven new release controls, scheduler owners, mixed-executor
and AsyncApp identity, complete benchmark teardown and six actual public workflows. The same six
public workflows pass with test support and benchmark support compiled in while tracking is off.
Both independent source reviewers close their findings without remaining P1/P2 in scope; all
six implementation/review threads are closed. Independent-consumer matrix, tracked native
execution and compiler gates continue below.

The public/independent matrix passes with the following build-time settings. Every standalone
fixture case executes its ordinary binary; its Test and real App/Bench cases are selected by
their own features. No cfg from the dependency is presumed to propagate into the consumer.

| Build selection | Public result | Independently locked result |
| --- | --- | --- |
| Test + Bench, empty environment, tracking off | 6/6 (`affae813-95e6-4e31-9b64-72ad41fc8a4d`) | 4/4 (`e3896bac-165e-447d-8f4f-459d3063db77`) |
| No test or benchmark facilities | Ordinary signature control covered by both mode runs | 2/2 (`80e83309-e62f-4d3a-ba79-30f702013c91`) |
| Bench only, empty environment | Previous complete benchmark consumers retain separate scope | 3/3 (`cf88fe78-f807-40a9-a3b9-b6ce0915ccff`) |
| Test, `GPUI_LEAK_DETECTION=1`, no leak feature | 5/5 (`e5edab75-689f-464d-8398-cfa242319cc1`) | 3/3 (`ced0daff-9f76-48b4-8b00-b7565759f263`) |
| Test, nonempty `GPUI_LEAK_DETECTION=0` | Snapshot control 1/1 (`9e1c70e4-2b02-425c-9842-4bfe58007b73`), four unrelated cases filtered | 3/3 (`ad46f6b3-8de4-4c9f-9688-e19e1d8b5acf`) |
| Test + explicit leak feature, empty environment | 5/5 (`f09024ab-c3b6-496a-9e01-2b0117c43991`) | 3/3 (`155a6845-cc85-4671-aa55-1842649ebabe`) |
| All features, empty environment | Included in focused 130/130 above | 4/4 (`1f5445c3-7a9b-4199-85fa-bc3866b61674`) |

All selected matrix cases pass without skips except the explicitly filtered zero-value public
selection. The real-time cases execute worker-thread identity, actual timers and foreground entity
updates after deterministic context creation; they do not qualify native rendering or numeric
performance. Tracking-on/off both preserve live strong/weak and effect-cleanup release assertions.

The complete native View trajectory also passes with Windows test support/profiling enabled and
environment-selected handle tracking (`66614bd4-f94e-4f17-a07e-f59b59a0f070`). It retains
the same source-driven notified input/output, actual nonempty Submitted generation, DX11 scene
readback, cache/reconstruction and exact entity/HWND/platform/OLE retirement checks as the
untracked default run. Readback redraws the same captured framework scene; it is not proof of
OS input injection or compositor scanout. Foreign native execution remains unqualified.

The first complete Core/public/WGPU/Windows/platform/scheduler run executes **1,918 tests:
1,917 pass and 1 fails**, with four original interactive Windows cases skipped
(`2cf7a61f-642f-4dab-a26f-1e75aff238e1`). Its sole failure is the existing mouse-leave
fixture: four input/hover pairs appear where it expects three, matching an earlier recorded
symptom. All new mechanisms and the complete native View trajectory pass. This run does not
establish a clean complete native gate.

The fixture originally pumps arbitrary OS messages between the three deliberately sent pointer
transitions and the exact diagnostic snapshot. Moving that pump after the exact assertions
first reproduces the other boundary: the last queued hover has not yet reached Core delivery,
so the selected run passes View but fails mouse-leave with five events rather than six
(`05090d97-0630-4237-bd73-13eb96009915`). One existing App update/release now converges that
Core ingress before reading the original cursor. The six-event order, two exit counts/positions,
no captured physical frame and exact HWND teardown are unchanged. Later OS messages are pumped
separately with their own no-invariant-failure check. The correction changes only the fixture.

Both selected scenarios then pass (`e71e0cee-58a3-46b0-81f8-3ecac3740dd4`); its later OS
queue contains message IDs `[2a3, 403, 403, 2a3]`. That trace does not retrospectively identify
the initial extra pair's source. OS queue contamination remains a supported possible explanation,
not an established root cause. Independent fixture review closes the missing-convergence finding
with no remaining P1/P2; all seven implementation/review threads are closed. Complete Windows
and component qualification continue without weakening the exact oracle or suppressing input.

The subsequent complete Windows cohort passes **224/224**, with the same four interactive
skips (`b4329c4a-bf0a-4e4b-bff0-0e158094aaad`). Together with the unchanged 1,694 passing
non-Windows cases from the initial run, this qualifies the selected 1,918-case union after the
fixture correction; it is not reported as a single all-green 1,918-test run. Component and
ordinary-consumer qualification continue below.

The complete UI core/components/docking/Canvas cohort passes **2,966/2,966** without skips
(`c7b6da40-88d5-4d33-8c3b-774cbef2f0ce`), explicitly enabling both consumer test-support
features and environment-selected leak tracking. The ordinary independent consumer initially
refuses `--locked` after the official unconditional backtrace change. Offline resolution adds
only the required `backtrace` dependency edge to its existing Core package entry, without package
version changes. Its locked enabled/disabled runs continue against that reviewed one-line update.

Those ordinary independent runs pass **62/62** with all features
(`46816bf5-e157-41ac-aec2-989c8e6b20e3`) and **47/47** with defaults disabled
(`f90feed2-8aa1-42b6-8d80-70ecbc59959b`), both without skips. Their original value/layout
and explicitly type-only scope is unchanged; actual Test/native/Bench consumers remain separate.
The test-only tracking expectation now defaults to the forwarded feature and compile-time
environment when omitted, so ordinary test commands require no extra runtime variable. Its
default-use controls pass public **6/6** (`fbf2c751-0289-46aa-9302-98eaf3e9bac4`), independent
untracked **4/4** (`b2e13abb-5bcd-46a3-a73d-c266195ff72b`) and independent environment-tracked
**3/3** (`1702249a-d4f7-4337-b7f1-2ceb9284848c`). These controls still verify actual dependency
behavior rather than treating local cfg or expected mode as evidence of tracking.

Locked Rust 1.95 workspace and both independently locked consumer all-target/all-feature
compilation pass. Affected Core/WGPU/Windows/platform/scheduler/util correctness Clippy passes.
Actual Windows Rust 1.92 all-target/all-feature compilation passes for the affected framework and
both independent consumers. The environment-only tracking production build also passes with
no default features or test facilities. No compiler-floor increase is needed.

Stable production wasm passes with profiling off/on. The existing optional shared-memory
configuration passes on Rust 1.98.0-nightly (`57d06900f`, May 27), using its checked-in
build-std/atomics/imported-memory settings without `RUSTC_BOOTSTRAP`. These are compilation gates;
browser/test-support execution and foreign native platforms remain unqualified. Metal's
debug-only capture policy has source review, without macOS compilation or execution.

Actual feature-tree checks confirm the ordinary consumer's all-feature graph and the new
benchmark-only graph exclude Core test support and leak detection. Build-time tracking is
explicitly disabled for those runs; graph exclusion alone cannot prove an environment cfg is off.
Root, ordinary fixture and test-support fixture formatting checks pass. Existing import/API/
document/release scans, workflow YAML/manifests and Git whitespace checks pass; local CI edits
do not imply a remote run.

Implementation is local `c6ffb32bd1499c9e8fb46783a3d1a4de626707a0`, with the independently
reviewed mouse-leave fixture correction at `2daf8412170540f7e2e586687bc25d5eadf4f2fa`.
These results qualify the complete selected additive-test-support unit, retaining the original
failures and explicit scopes above. They do not establish measured build-time savings, per-frame
overhead, full fixed-target refresh, conditional hot reload, canonical docking or Nara GPU/View
adoption. No engine UI default follows.

The first fresh offline fixture resolution selected unrelated newer compatible packages.
Reusing the maintained dependency choices, then letting Cargo prune and independently resolve
the standalone fixture, retains its own lockfile with no changed shared package versions.
Unfiltered offline metadata additionally asks for an uncached foreign-platform package and
fails; locked Windows-filtered metadata passes without network access. This is a dependency
resolution record, not foreign-target compilation or execution.

## Conditional Native Hot Reload

The November 8 unit starts from `da1b1fdecbd5bb57a17e8898650cf5592958085b`, preserving
official `670a779a5ed9e7161c97aa4f998d395c207d5631` through the fixed target
`635026fabc38440fce5e1767d125026a17b12012`. Local feature implementation is
`ad70ac9230892ca4e4d47840b93215596b8c60e0`; the separate fork terminal correction is
`28eebf0e8dcd0c8a71f1064bf9f66052b5d3a55c`. Optional native debug build selection, per-App
task ownership, the bounded sixteen-table transport, build/process filtering, foreground patch
application, window refresh and both stored AnyView/common View render boundaries follow the
official mechanism. The fork applies through existing `AsyncApp::update_when_available` so a
native modal callback cannot replace code while its active App update still owns the borrow.
No additional reload manager, render loop or engine owner is introduced.

The initial locked enabled Core library check passes. Ten focused transport/owner/render
regressions pass in `2b82e9ee-6d70-422f-8a4e-c44dfe9b3529`, including the original filtering
and cached-state regressions, full-restart commands, malformed messages, refused handshakes,
disconnects, receiver retirement, App borrow waiting, task cancellation and failed native library
loads preserving state and refresh. These are not executable patch success evidence. Independent
core review finds no confirmed P1/P2 defect; its live-borrow successful-patch coverage observation
remains distinct from the actual compiler/runtime consumer.

The independently locked [native consumer](../tests/fixtures/hot-reload-consumer/README.md)
compiles using public native, rendering and entity APIs. It exercises a typed cached entity, a
stored erased view, and a component with persistent keyed state in a separate workspace member.
The isolated official Windows `dx` 0.7.10 executable reports `57d6794`; its published archive
SHA-256 is `45eb4f87b7f86fdba8508ee7e0ef1f9e13cf8f2ffcc6a2d5858d0e8ea7dd1531`.
The developer's installed 0.7.9 executable is unchanged. The first offline run fails before build
because the independent closure lacks a cached package; a locked fetch supplies that closure.
The next full dx build takes 412.90 seconds, but Windows loading fails with `0xc0000139`
before the patch connection. Import/debugger inspection identifies `TaskDialogIndirect` with
Common Controls 5.82: the fixture had disabled defaults without selecting `windows-manifest`.
The corrected consumer explicitly enables the existing feature. This is a consumer configuration
correction, not an official/fork hot-reload algorithm change. Actual edit/build/apply qualification
is still in progress at this checkpoint.

Independent acceptance review finds three P2 gaps in the initial fixture: state stays at its
constructor values, success precedes actual process exit, and an already-exited dx parent can
leave descendants alive. Corrections require an observed runtime state mutation before editing,
same-process nonce/state continuity, observed quit and process exit before success, and scoped
process custody through final cleanup. These findings do not weaken the original Submitted-frame,
pixel, entity or release assertions. Final executions and review closure belong below after they
actually complete.

The enabled Core/WGPU/Windows/gallery libraries and selected public interfaces subsequently
pass 1,823/1,823 with the four existing interactive skips
(`f6182b6d-bcaa-48f2-9e52-a63645a44132`). The corrected real dx run starts a visible DX11
window, observes constructor state at Submitted generation 2, then distinct runtime-mutated
state at Submitted generation 3 with the same entities and exact row pixels. dx compiles the
changed panel in 2,035 ms and GPUI reports patch application, but the immediate redraw exits
with `0xc00000fd` stack overflow. No PATCHED frame is accepted by the driver and no successful
result artifact is written. At that checkpoint the actual stack/codegen/jump-table cause remained
under investigation; compile and load success alone do not close the executable patch requirement.

Two debugger reproductions identify an external driver cause, preserving the original failure.
The symbolized stack reaches the patched root render through the correct stored AnyView/common
View boundaries and then faults in original `__chkstk`, without a repeated HotFn recursion cycle.
Disassembly shows the patched render loading the normal frame size `0x3810` into EAX before
calling dx's undefined-function stub. That stub replaces RAX with the original `__chkstk`
address and jumps through it, corrupting the stack-probe argument. This matches upstream
[Dioxus issue 5966](https://github.com/DioxusLabs/dioxus/issues/5966) and
[fix 5967](https://github.com/DioxusLabs/dioxus/pull/5967), commit
`14b4b767c8b661dffabe648bd07994e88ef57000`. The committed fixture backport adapts only that
register-transparent x64 jump to the published CLI 0.7.10 source; no GPUI boundary is removed
and no larger application stack is selected. A separately built driver and its binary hash
must identify any subsequent acceptance result. This correction is outside the fixed Zed GPUI
source comparison and is not represented as an unchanged released dx tool.

The separately built CLI uses unchanged published source except that one backport, with
`disable-telemetry`, Rust 1.95.0, development dependencies without debug information and
level-one optimization on the CLI entry crate. The entirely unoptimized entry first exhausts
its own main stack before launching the consumer; the entry optimization corrects that tool
configuration without changing the application's profile or stack reserve. The qualified binary
SHA-256 is `4fcb34ce830ffbb0c79556b846d873448664f260e56d8b01bd553d39172c2ae1`.

Actual edit/build/apply/readback passes with that identified driver. At scale 1.5, native DX11
observes constructor state, then root 41 and children 43/47/53 at accepted/Submitted generation 3.
The panel member's compatible render-body edit compiles in 2,252 ms. Generation 4 contains all
three updated colors/labels/recipe outputs, exact pixel values, the same process-start nonce,
window and entity identities, and unchanged mutated counters and one initialization each.
Normal retirement releases all three states once, records the quit callback and exits with code
zero before the driver reports success. A prior attempt already produces the correct patched
frame and exit zero but fails the stricter quit-record requirement: the fixture had omitted the
existing public `App::shutdown` before platform quit. The corrected fixture completes shutdown
explicitly. Observed edit-to-result is 3,419.234 ms including compilation, transport, redraw and
100 ms sampling; this single run is not a latency distribution or performance budget.

The same real base executable passes native wrong-build, wrong-PID, invalid-DLL, full-restart
and graceful-disconnect controls. Its actual handshake identifies the build, process and ASLR
anchor. All five live checkpoints retain generation 3, original pixels, entities and mutated
state without observer notification/redraw. The invalid DLL reports one failed load and no
successful application; the full-reload command reports restart required, and disconnect is
observed. Normal retirement, shutdown/quit and process exit zero pass. Wrong-target silence has
a protocol-pong plus 0.5-second settle bound; it is not an arbitrary-delay proof. The controlled
WebSocket server uses optional test-only `websockets==16.0`, not a new production dependency.
Those initial runs still emit the inherited native-terminal retirement diagnostic in both patched
and unpatched paths. The separate fork-owned correction below reproduces and resolves it; those
earlier runs alone do not qualify complete native custody.

The separate fork-owned lifecycle correction is reproduced by
`6b52efb8-5362-4385-8bd4-3e766eabc175`: two controls pass and the queued-Closed regression
fails with the exact missing-authority diagnostic. Windows reports one physical destruction
through both the retirement command result and the queued Closed event. The first successful
ticket acknowledgement removes the barrier; the later event previously treated that success
as a missing authority. Existing AppCell terminal bookkeeping now distinguishes Observed from
Settled, marking only successful acknowledgements as Settled. Repeated settled reports remain
idempotent while capture completion, dependency release and shutdown progression retain their
existing owners. First unknown reports and quiescence violations still diagnose errors.
All three controls pass in `3f96cf60-d90a-4da1-9583-771a0b1b8c4b`.

Independent lifecycle review finds a P2 in the new test logger: restoring a process-global log
level per TLS capture can race under ordinary parallel libtest. One-time logger/level selection
and TLS-only capture cleanup correct it; the separate reviewer closes that finding, and actual
ordinary `cargo test --test-threads=3` passes all three controls. Runtime review finds no P1/P2
in its bounded scope.

The real compiler/patch trajectory passes again after the lifecycle correction, now requiring
the missing-authority diagnostic to be absent. It retains the same generation 3-to-4, mutated
state, exact pixel, release/quit and exit-zero controls, with a 3,419.7 ms observed duration.
The subsequent native failure attempt fails its unchanged generation assertion at full restart:
root renders and accepted generation advance from 3 to 5 while model values and pixels remain
unchanged. The source of those additional refreshes is unproven. An unchanged rerun using the
same base executable and strict assertions passes all five failure checkpoints, retirement,
shutdown/quit and exit zero without the missing-authority diagnostic. Preserve both results;
the passing rerun does not explain the extra-refresh failure or establish isolation from
uncontrolled desktop events. Final success artifacts are `target/hot-reload-i_en9jka/result.json`
and `target/hot-reload-failures-eupmja5k/result.json`; these generated local observations are not
source-controlled fixtures.

After the terminal/logger correction, the affected lifecycle and complete Windows selection
`002c688f-6567-41b8-a329-632259fbb56b` runs 313 cases: 310 pass and three fail. Native image
animation and nonactivating first-show cases fail active/foreground assertions despite retaining
their other reported controls. An unchanged selected rerun
`3db9b830-603e-49df-b394-177c8f81fb30` passes both and still fails
`rejected_activation_restores_a_non_click_activating_gpui_foreground_window`. Read-only desktop
inspection identifies the external foreground HWND as the user's running Steam game. This
supports a desktop-state limitation, not a source-correctness proof. The remaining activation
gate is unqualified; no all-green final complete native run is claimed. The external process
is left untouched, and no activation assertion or product behavior is weakened.

The newly resolved dependency closure declares MIT, Apache-2.0 or Zlib licensing. Manifest
compiler-floor declarations alone do not establish the real Rust 1.92 build. Final affected
Core/WGPU/Windows all-target/all-feature compilation and the independently locked native consumer
all-target check both pass on actual Windows Rust 1.92.0 after the lifecycle correction. Root
lockfile comparison retains every existing package version and adds only seventeen packages.
The native fixture resolves its own compatible choices rather than copying the root lock;
its qualified temporary copy has the exact maintained fixture lockfile.

Final hot-reload-disabled View/cache/lifecycle controls pass 68/68
(`309e76d4-b5d0-447c-ad7d-de6f32f6edb5`). The ordinary independently locked consumer passes
62/62 enabled (`55757ebd-8913-4f83-b7aa-e9968a0c3aff`) and 47/47 disabled
(`c8d6d2f6-0993-4f83-a394-74b6cb0a2bc3`), preserving its separate value/type-only scopes.
The root locked workspace all-target check and affected Core/WGPU/Windows all-target/all-feature
correctness Clippy pass on Rust 1.95.0. Existing ordinary/test-support consumer checks and
components/docking/Canvas 2,966/2,966 passed before the small terminal correction; affected
owner, native consumer and compiler checks above cover that later change. Neither the earlier
1,823-case run nor those earlier component results are presented as one final all-green
complete acceptance run.

Feature-selected native Core also compiles with package-local development debug assertions
disabled. Actual compiler-artifact profile data confirms non-debug selection and the executed
build script omits `gpui_hot_reload`. This qualifies the conditional boundary without claiming
an optimized complete release application. Stable production Core/WGPU/Web compilation passes
with `hot-reload` requested and profiling both off and on; native-only transport dependencies
remain excluded. These are production compiler checks, not browser execution. Optional nightly
shared-memory compilation is not rerun for this unit.

All four workspace formatting checks pass: root, ordinary independent consumer, test-support
consumer and the new native consumer. Workflow YAML, both Python driver syntax checks, staged
whitespace and the existing import/public-API/document/release scans pass. All thirteen
implementation/review threads are closed; their bounded reviews close the confirmed
fixture/lifecycle P2 findings.
Remote CI is not executed. The one remaining native foreground-restoration failure and the
unexplained earlier extra-refresh observation retain the distinct scopes above.

Source inspection identifies dx 0.7.10 workspace replay limitations for
dev-dependency cycles, missing captures and foreign path dependencies. The fixture's separate
panel is a member of its independent workspace; its success does not qualify editing all
framework crates outside that workspace. Compatible render-body replacement does not provide
arbitrary struct-layout migration, custom Element phase replacement, browser transport, foreign
native execution, OS scanout, numerical latency budgets or a complete Nara Editor reload workflow.

## Shared Atlas Ownership And Retained Renderer Bindings

The November 8 successor starts from `480430f02aecc3bdcf78bed8126ef6b871974770` against
the unchanged fixed Zed target. Local implementation is committed at
`67e1360709ca48be74bf986f116b9c004ff5bd59`, with final scoped qualification on November 9.
Official `0ef92145ff87aab5f19f6b44c9c55a486f4f064f`
supplies owned insertion, Entry-based successful-only publication, shared AtlasState/AtlasBackend
and one HeadlessAtlas for TestWindow/Linux. The renderer prerequisite chain includes
`be8c6f9fb356dcd40a7ff06149568753e64ee171`,
`a434bb7ee275cefa348a2bfaf27dc30280bf38f7`,
`96e95edac45b0e0696179b8139aa521cf48db392` and
`e91b82c106817f2419207ebf81f1da766698ac95`. The
[successor plan](plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#shared-atlas-ownership-and-renderer-lifetime-successor)
owns complete backend/headless integration, frozen resource receipts and actual binding-cache
qualification. The execution history below distinguishes intermediate failures from final
affected-backend qualification and the remaining platform/native gates.

The shared owner retains the fork's concrete diagnostics and exact texture-instance leases.
Successful insertion consumes the caller's key without cloning it into another backend map.
Builder/backend failures leave the key unpublished. Builders now run under the same atlas lock
in every implementation and must not re-enter that atlas. Published region retention, atomic
set acquisition, old-epoch release refusal and checked monotonic identities remain backend
responsibilities. Test-only acquisition/release counters are absent from normal runtime builds.
The two official key-cache regressions and the original four TestAtlas lease scenarios move
to their shared owner without dropping their original assertions.

An initial Core library check passes, then the disabled focused owner/public cohort passes
55/55 (`7938f992-19b0-4e3a-8c3b-77287cb5a1c0`). That run precedes a diagnostic-source
correction: inserted diagnostics retain the builder's requested dimensions, while a cache hit
reports the existing tile dimensions. A backend-adjusted-size regression distinguishes those
facts. A separate actual lease/unwind regression catches a raster-builder panic, releases an
outer retained lease after lock unwind, refuses the retired instance and successfully retries
the unpublished key. It does not promise same-atlas reentry from a builder or its destructors.

The corrected all-feature focused owner/public selection passes 58/58
(`023cc0c4-a50e-433e-acdd-ee1d0ce007fe`). Complete Core library plus the public atlas,
frame-scheduling, View/component and lifecycle targets then pass 1,516/1,516 without skips
(`36a3f34d-1b2e-45ba-a181-deffee715398`). The ordinary public CPU-backend target also
passes 1/1 without optional features (`4a51802f-03bf-4f95-9fe4-99a22b0b255d`).
Actual Windows Rust 1.92.0 Core all-target/all-feature compilation passes. These results
qualify the stated Core/CPU paths, not GPU submission, native pixels, backend migration,
foreign execution, a cache speedup or complete fixed-target refresh.

The complete-Scene recording prerequisite is committed separately at
`186deca5a46f9e5ed48614b3e631a051b305e96a`. Its BoundsTree copy owns the nodes and starts
with empty pointer scratch; Scene copies preserve clip arenas, journals and original
geometry-validity associations without acquiring resource leases or presentation authority.
Fourteen BoundsTree/Scene tests pass
(`fbbb2933-219b-4faf-8da1-ce95a9e03a75`), followed by actual Windows Rust 1.92.0 Core
all-target/all-feature compilation. The bounded final review closes without P1/P2.
The WGPU fixture uses the actual Window scale factor two; fractional/dynamic DPI and DX
journal replay after fixture sprite replacement remain outside those backend controls.

The first WGPU all-target/all-feature check stops while writing its incremental dependency
graph with Windows disk-space error 112. The verified shared target contains approximately
219 GiB of compiler incremental cache. Automatic policy rejects the attempted cache-only
cleanup; no deletion occurs. Subsequent commands retain one build job and the same target,
with `CARGO_INCREMENTAL=0`. Existing dependencies, executables, driver and qualification
artifacts remain in place. This is a build-environment correction, not a source/compiler
failure or a changed runtime performance qualification.

The retried WGPU all-target/all-feature check passes. The first actual GPU selection
`66f175c0-d527-4897-b821-e3557c6c0826` runs 17 cases: eleven pass and all six new
cache/lifetime/growth cases fail their expected-color assertions with the cleared black pixel.
Independent source review identifies the shared fixture's missing clip arena and zero-count
default envelope; the production shader correctly rejects that envelope. Quad/path growth
fixtures have the same issue. These failures do not qualify later cache-count assertions.
The correction must use complete Window-generated clipping and retain all original pixel,
submission, creation/hit, generation and lifetime controls; shader rejection remains strict.

The same review identifies an inherited WGPU upload-admission gap: allocation and publication
can precede validation of a short source buffer, with failure deferred to queue upload. Both
the content import anchor and fixed official target retain that missing validation. The owning
backend must refuse malformed dimensions/bytes before allocation, queueing and key publication,
with retry and trailing-byte controls. This is a source-confirmed inherited correction rather
than a newly introduced fork regression.

The corrected WGPU selection `052e0606-dffb-4085-b7fa-8d9f8962b61f` runs 21 cases:
sixteen pass, including the new malformed-source, dimension/row-pitch, same-key retry and
resident-region/pending-upload controls, mixed stale-generation refusal and both existing
headless submission trajectories. Five cache/reset/growth pixel cases still fail. Their
Window fixture uses scale factor two, while the affected radius and pixel probes assume one;
the one-texel color fixture also samples atlas filtering boundaries. The next fixture revision
captures the public Window scale with the complete Scene, converts targets and probes to
device coordinates and uses uniform multi-texel colors. Expected colors, actual submission,
binding creation/hit and generation/lifetime assertions remain strict. A source correction
alone does not close those five execution failures.

The scale-corrected selection `6bfc3b98-afcd-4fd5-8cbf-0e74b96716cf` then passes
21/21 across the library and existing headless renderer integration target, with 78 tests
outside the selected scope. Actual submitted/readback controls now reach all cache-hit,
resize, eviction, mixed stale-instance refusal, leased-region retention, frozen receipt
and same-device reconstruction assertions. The mixed primitive path executes real Window
clips, nonidentity transforms, glyph/shadow/underline pixels and instance-buffer growth.
Same-device reset is not a replacement-Device qualification, and the monochrome glyph
fixture does not execute subpixel dual-source blending. No numeric speedup follows from
the observed creation/hit counters.

The independently locked ordinary consumer passes 48/48 with optional features disabled
(`b2e19246-24bb-4420-bd2a-7dd8f94a9523`), including its public headless lease scenario.
DirectX upload/admission passes 14/14
(`8147a03c-4b18-486b-9553-157bd33caa2b`). Its first complete-Scene renderer fixtures retain
one-texel filtering failures; replacing those uniform source images without changing the
pixel oracle yields 4/4 (`d46d371a-c537-4247-b202-3c1aac74ab5f`): the existing two-window
native asset consumer and three WARP controls. They exercise complete mixed-resource refusal
before target clearing, frozen original-region/SRV retention, renderer recovery and fresh
paint. These are executed Windows checkpoint results, not final workspace/component or
foreign-platform qualification.

Independent Core/shared-headless review closes without P1/P2 after the size/unwind correction.
The separate Metal atlas/render source review finds no confirmed P1/P2 in its bounded scope,
but its thirteen new tests have not run on macOS. The atlas-test helper now returns an explicit
missing-device error rather than reporting empty success. Native draw/capture, valid clip arenas,
CoreVideo and actual GPU-completion lifetime still require owning-platform execution; the
Windows Core pass count does not supply that evidence.

The first final all-feature framework/public/Gallery build stops before test execution
while linking the paragraph-text target: MSVC reports PDB `LNK1318 LIMIT (12)`, with only
97,546,240 bytes remaining on F:. The user frees space before the resumed serial build,
which keeps the same target and disables incremental compilation. The failed build is
not a test failure or a completed regression run.

After the user's cleanup, the resumed all-feature Core/WGPU/Windows/platform/public/Gallery
selection runs 1,935 cases: 1,934 pass, one fails and four original interactive cases remain
skipped (`a2eea953-d2dc-43e8-8e04-f1944c5dfbc1`). The only failure is the previously recorded
`rejected_activation_restores_a_non_click_activating_gpui_foreground_window`: foreground
restoration does not converge within 512 delayed pump attempts. This run supplies neither
an explanation of its current desktop cause nor a complete all-green native qualification.
The cold public negative-compilation check passes after its recorded slow build.
This selection precedes the additional prepared-receipt region-lifetime regression and any
subsequent WGPU correction; those require their own execution evidence.

That regression reproduces a real integration defect. Official region deallocation combined
with the fork's view-only prepared receipt permits removal/reinsertion to overwrite an old
resident region between preparation and submission. Actual readback is blue instead of the
prepared red (`2458e3f6-b0bc-4e82-b503-77d8d4eb1520`, one failing case). The owning
correction makes the private receipt retain the existing complete-set texture lease as well
as frozen views. It adds no second resource identity or owner. Error unwinding releases the
inner atlas lock before dropping the lease; old-epoch release cannot affect new residents.
The same-device frozen-view/reset behavior remains intact.

The corrected focused Atlas/renderer selection passes 20/20
(`ef58f584-f26c-40e8-a424-52580513e56a`), and all WGPU library/public integrations pass
110/110 without skips. Independent review closes the receipt correction without P1/P2.
The subsequent test revision moves receipt drop, region reuse and upload before first
readback to exercise same-Queue ordering after submission. It also adds atomic/deduplicated
acquisition and unsubmitted-receipt Drop controls; their results are recorded separately below.

The final Windows CI command passes 21/21 with 78 cases outside its focused selection
(`1ad1a803-9f52-4048-bf03-ef13083641d1`). The strengthened GPU control drops the receipt
immediately after actual submission, reuses/uploads the old region before first readback,
and still observes the submitted red target. The backend control proves no partial lease
on mixed valid/stale acquisition, one count for duplicate instances, and exact region
reclamation after an unsubmitted receipt drops. Bounded independent review closes both
test additions without P1/P2. Complete components/docking/Canvas passes 2,966/2,966 with
no skips (`86fe99ea-6441-4898-bf26-1577e2c3b0c4`). No production changes follow these
qualifications; independently locked consumers and final compiler/configuration gates follow.

The independently locked ordinary consumer then passes 63/63 with all features
(`4f4ebfbf-71a4-4d47-b76f-eaa3304a5f4a`) and 48/48 without optional features
(`49b05559-79db-43fb-9f7b-7ddd0b7d8870`), without skips. Both execute the public shared
headless lease consumer. These mode results use the fixture's own compatible dependency
choices and do not add test support to its normal graph.

Final November 9 Windows Rust 1.95 checks pass: locked workspace all-target compilation,
ordinary and test-support independent all-target/all-feature compilation, and affected
Core/WGPU/Windows all-target/all-feature Clippy with `-D clippy::correctness`. Actual
Rust 1.92.0 affected-framework and ordinary independently locked consumer all-target/
all-feature checks also pass. Stable production Core/WGPU/Web compiles for
wasm32-unknown-unknown with profiling disabled and enabled. These are compiler gates;
they establish neither browser execution nor a cross-platform native runtime result.

The existing optional shared-memory/multithreaded production wasm configuration also compiles
under the installed nightly, using its maintained hello-web configuration and profiler/
multithreaded features. It does not use stable feature bypasses or qualify browser execution.

All four formatting checks pass after three formatting-only wraps in the new ordinary
consumer test. Rebuilt existing import-boundary, public-API, document-link and release-doc
scans pass, as do workflow YAML and whitespace checks. The public/independent atlas and
focused Windows WGPU gates are added to the existing workflow without running remote CI.
No runtime code changes follow the recorded owner/consumer acceptance. Metal native tests,
the existing foreground-restoration failure, replacement-Device/fractional-DPI/subpixel
execution and numeric costs retain their stated limits. Full fixed-target refresh,
canonical docking and Nara GPU/View adoption remain open.

## Image Dependency Feature Policy

On November 9, the successor starts from Open-GPUI
`c3d31566ffd6cc55c6f2b98e5e33810812e27262` against the unchanged fixed Zed target
`635026fabc38440fce5e1767d125026a17b12012`. Official
`8e06c323ae063f3d245ea5c283d4bf373ad51db2` removes the unused default AVIF encoder
without changing the remaining image feature policy. The fork retains its qualified 0.25.4
API minimum and resolved image 0.25.10. No decoder, resource loader, extension inventory,
rendering owner or public GPUI format enum changes.
Local implementation is `b4355a36824f0709f1b73f7480ec88a884a489bc`; the independently
locked smoke consumer refresh is separately committed at
`015863c085dbcd845658cfb97c289c3ac4433aad`.

The root manifest disables defaults and explicitly enables bmp, dds, exr, ff, gif, hdr,
ico, jpeg, png, pnm, qoi, rayon, tga, tiff and webp. AVIF encoding uses `avif` and
ravif/rav1e; AVIF decoding separately needs `avif-native` and dav1d. Neither the previous
default graph nor this policy provides that decoder. The inherited `"avif"` extension
label remains an inventory entry, and TGA still lacks resource format guessing.

Actual before/after Core feature graphs are captured for Windows x64, Linux x64 and
macOS arm64. The after graphs include the entire workspace with all features.
Each removes exactly `avif`, `default` and `default-formats`, adding no other image feature.
Normal/build package graphs exclude ravif, rav1e and dav1d on all three targets.
These are resolved dependency graphs on Windows, not foreign compilation or runtime tests.
Optional packages can remain in Cargo.lock; their presence does not establish active builds.

The ordinary independently locked consumer owns optional
`image = { version = "0.25.4", default-features = false, features = ["avif"] }`,
selected by its own `avif-encoding = ["dep:image"]`. Actual feature/package graphs on the
same three targets exclude the encoder in default mode and include it under both explicit
`avif-encoding` and all-feature modes. No mode enables image defaults or the native decoder.
This adds no GPUI codec-selection API or feature. Initial offline foreign-graph inspection
encounters uncached ashpd 0.13.13; the targeted online retry uses the existing proxy and
preserves locked versions.

New Core resource controls await the actual ImageAssetLoader HTTP-resource path for DDS,
OpenEXR, Farbfeld, HDR and QOI, comparing all BGRA pixels against a deterministic red/blue
image. DDS uses a locally constructed DXT1 header/block rather than an external binary asset.
A separately named header-only AVIF control checks a recognized format reaching the shared
decoder and returning its exact Unsupported format error; it establishes neither valid AVIF
content nor decoding. The ordinary encoder control decodes public GPUI WebP pixels, converts
BGRA to RGBA and actually encodes one AVIF pixel with one thread. Format detection establishes
encoded output, not an AVIF round trip.

Root, ordinary-consumer, test-support-consumer and hot-reload-consumer lockfiles retain every
package version. The root/test-support/hot-reload image nodes only drop the direct `rgb` edge;
the ordinary fixture adds its application-owned image edge and retains encoding when selected.
Inspection also exposes a preexisting stale smoke_web lock: it still contains Taffy 0.10,
spin 0.10 and the retired stacksafe owner, and lacks current segmentation and accessibility
dependencies. Its separate refresh follows the existing manifests, including Taffy 0.15.1,
spin 0.12.3, ICU 2.4 and the required zero-copy closure. Unrelated compatible crossbeam-deque,
ctor, link-section and linktime-proc-macro selections are explicitly preserved. No manifest
version requirements are raised by that fixture maintenance.

Bounded independent source review confirms the exact official feature set, deterministic DDS
contents, actual awaited resource path, decoder error branch and ordinary encoder API use,
with no P1/P2 finding. Both implementation workers and the reviewer are closed. The existing
workflow gains the focused Core/public/independent image gates; remote CI is not executed.

The initial focused Core image run passes 20/20 without skips in its selected set
(`3e7a73fe-58cf-4009-b467-aba10db320ac`). After a test-variable naming cleanup, the final
Core/WGPU-public/Windows-native selection passes 25/25
(`6d6306ba-2a78-44d9-8976-3e007616d9db`): twenty Core cases, four public headless resource/
animation/loading consumers and the complete native encoded/embedded image trajectory.
The native consumer retains actual nonempty Submitted frames, decoded BGRA and framework
pixel readback, animation/replacement/reconstruction, atlas reuse and exact owner retirement.
It does not establish OS compositor presentation. The overlapping 20 and 25 selections are
not forty-five distinct regressions; 1,818 unrelated tests are filtered from the final run.

The ordinary independently locked image target passes 3/3 with encoding disabled
(`35b8cace-6222-49a0-82ca-75c9c36e27d2`) and 4/4 with only the application-owned
`avif-encoding` opt-in (`8f5452cf-51dc-4364-bf91-dda2c33e26c2`), without skips.
The enabled test writes nonempty AVIF bytes through the public encoder while existing GPUI
JPEG/WebP/error consumers keep passing. The normal dependency graph acquires no GPUI
test-support feature.

Final Windows Rust 1.95 locked workspace all-target and independently locked ordinary-consumer
all-target/all-feature checks pass. Affected Core/WGPU/Windows all-target/all-feature Clippy
passes with `-D clippy::correctness`. Actual Rust 1.92.0 checks pass for those three packages
and the ordinary independent consumer, both all-target/all-feature. These checks include the
application-owned encoder without raising the declared framework compiler floor.

Stable Rust 1.95 production Core/WGPU/Web compiles for wasm32-unknown-unknown with default
features disabled. Profiling and optional shared-memory variants retain their separately
recorded earlier qualification; they are not rerun for this dependency-only policy.
Import-boundary and public-API scans pass through the existing built xtask executable.
The refreshed independently locked smoke_web consumer also passes its stable Rust 1.95
production wasm32-unknown-unknown check. This is compilation rather than browser execution
or a Rust 1.92 qualification for that fixture.

Root and ordinary-consumer formatting checks, workflow YAML parsing, whitespace checks and
the existing document-link/release-doc scans pass. The earlier complete suites
are not repeated for this dependency policy; the affected image consumers above supply its
runtime evidence. Existing native foreground restoration, foreign execution, AVIF native
decoding, numeric cost distributions, complete fixed-target refresh, canonical docking and
Nara GPU/View adoption remain open. No build-time or memory speedup is claimed from the
upstream measurements.

## WGPU Dependency, Shaders And WebGL

The November 9 successor starts from maintained revision
`eebfbd2241e9da03125cc86353ee67b31de5131d` against the unchanged fixed Zed target.
Its complete implementation is committed at `38d3041acab90cc01cbc9984a8b738a8b2c3a194`.
Official WebGL `4601ead416`, feature cleanup `d544470d1d`, WGPU update `402675e805`
and reconfiguration follow-up `24a2823456` form the coherent renderer unit. The target
pins upstream gfx-rs WGPU and matching Naga to
`cddb2f5d80dc0009b2ef6767635140cacf90a007`. It contains Vulkan surface-texture teardown
`3b3506ea31154edc04de64debda0e780cba2639c` and the wasm wait-timeout correction at that pin.
Registry WGPU 30.0.0 in the framework and 30.0.1 in the ordinary consumer are distinct
dependency sources, not proof of those fixes.

The pinned dependency declares Rust 1.93. Current framework/consumer manifests and the
minimum-compiler CI job explicitly advance to that floor. The official toolchain action's
`1.93.0` ref is verified read-only at `48b64ba94bc1a6fe819fd9afc0799a7647efdc65`.
Earlier compiler-floor results retain their original source revisions; the new source still
requires its own actual compiler qualification. Nara already declares Rust 1.95, but its
separate registry WGPU 30.0.1 device identity remains an engine-integration gap.

Initial resolution stops at locked cfg_aliases 0.2.1 versus Naga's required 0.2.2, then at
the pinned dependency's minimum Web binding SDK. Targeted unlocks resolve the required
WGPU/Naga closure with cfg_aliases 0.2.2, bit-set/bit-vec, glow and the binding family.
Root/smoke bindings resolve js-sys/web-sys 0.3.107 and wasm-bindgen 0.2.130; the ordinary
consumer already had that binding family. The unrelated test-support and hot-reload
lockfiles retain their prior package selections. No general dependency update is used.

The new ordinary `vulkan_surface` target uses application-owned public WGPU types and an
owning Win32 HWND/class fixture. Both tests require a real Vulkan adapter and are explicitly
ignored in generic runs. One drops a surface while its acquired texture remains live; the
other submits a real GPU clear before surface Drop and waits afterward. It does not assume
a minimum GPU execution time or qualify physical device loss.

With the fixed Git dependency, both pass on Windows, NVIDIA GeForce RTX 4090, driver 621.14
(`333ecc6f-5354-4d4a-9991-90856154cc69`). Temporarily selecting the earlier application
registry pin reproduces both failures (`c81a5977-3546-4d7f-a4c3-1eda127817b8`): the first
panic is the exact `SwapchainAcquireSemaphore` still-in-use assertion in wgpu-hal 30.0.1.
A subsequent cleanup panic aborts each test process. Windows reports fast-fail code
0xc0000409; that generic exit description is not evidence of a native buffer overflow.
The tests do not catch or suppress either panic.

Restoring the declared fixed revision passes both unchanged tests again
(`fa8e2e62-dbd6-43cf-886f-451ea2d94435`), with actual Vulkan adapter output retained.
The framework pin stays unchanged throughout the application-source control. These runs
compile the ordinary native Core/Windows consumer and direct WGPU dependency; they do not
compile the still-in-progress GPUI WGPU renderer or Web backend and cannot qualify its shaders.
Validation scopes and uncaptured-error checks remain active; installed native Vulkan
validation-layer coverage has not been separately established.

The ordinary native Vulkan target then passes actual Rust 1.93.0 locked compilation.
Native production WGPU/Web libraries pass Rust 1.95 all-feature checking, and stable production
WGPU/Web/platform libraries pass wasm32-unknown-unknown checking with optional features disabled.
The complete renderer/runtime/test/browser and compiler-floor matrix still follows; these
production checks compile neither the new GPU regressions nor the complete browser consumer.

The independently locked ordinary renderer consumer passes its actual application-owned
device/queue, upload refusal/retry, atlas lease and final wrapper release trajectory
(`e7485737-1edd-4efe-ad39-e0c8bb230d35`, 1/1). Its initial test compilation used an unavailable
`InstanceDescriptor::default`; the correction uses the pinned dependency's public
`new_without_display_handle` constructor without changing the runtime assertions.

Focused matching-Naga and actual headless GPU controls pass 11/11
(`0578953d-80a0-47ad-9de5-e3f4508186a1`). These include full-image storage/packed equivalence,
five-texel instance rows with 184-byte Quads, two-texel clip rows with three-texel clip records,
out-of-bounds refusal before mutation and late path allocation refusal after a visible Quad.
The latter preserves previous pixels and accepted-draw count, then retries the same Scene
successfully after capacity restoration for both transports. Empty resource-write flushes and
readback copies are allowed; the assertions do not confuse those submissions with an accepted
draw. Running the packed shader on the native backend does not by itself qualify WebGL.

The browser driver's bounded policy/coordinate/PNG-reference tests pass 14/14
(`4ee24518-841a-4578-bafb-647a97d1fe49`). The expanded ordinary smoke consumer first requires
only workspace package-edge lock refresh: offline workspace update locks zero new versions.
Its first compilation then rejects private `Pixels.0` reads; conversion now uses public
`f32::from`. Actual browser execution and the corrected complete fixture compilation follow.

Trunk builds the complete fixture and installs its matching wasm-bindgen 0.2.130 without
replacing the global CLI. The first actual Edge run passes Auto/WebGPU input, activation and
both 800x600 and 960x720 PNG checks, but forced WebGL refuses the unchanged
`force_fallback_adapter=true` request: the pinned backend reports no GL fallback adapters.
The driver now makes GL success cases explicitly request `software=false` and separately
asserts that the original software-only request refuses without launching an App window or
retaining a DOM canvas. Production continues forwarding the requested adapter policy.

Actual GL then passes the input/capture/docking/activation trajectory but fails the unchanged
renderer samples and text coverage. Captured diagnostics identify the fork's `SurfaceParams`:
replacing the official 16-byte content mask with the 24-byte exact clip envelope made its
uniform size 40 bytes, violating WebGL's complete 16-byte binding requirement. This is a
fork integration defect, not evidence of a defect in the official 32-byte parameter layout.
Rust now reserves 48 bytes and WGSL reserves 32 bytes for the nested clip member. An initial
explicit trailing WGSL field also violates nested-uniform member spacing; matching-Naga
controls reproduce all three composed-shader failures
(`621ffedb-cdae-4a9f-83d0-d3eea9e8f22f`) before the member-size correction. The new Naga
reflection control checks all uniform struct spans and exact Rust/Surface shader size.
Native and browser qualification is repeated after this correction.

Matching-Naga, packed/native image equivalence, cross-row decoding and late refusal/retry
then pass 12/12 (`32a6e747-0dc9-4130-b7ae-256fc9e3b521`). The actual browser matrix also passes
with the corrected uniform: forced GL, Auto WebGPU, forced WebGPU and Auto GL under an explicit
`navigator.gpu` unavailability control. Each rendering case passes all 22 fixed samples at
800x600 and 960x720, including the last of 16,384 Quads, nested asymmetric clips, nonuniform
transforms, alpha layering, image and path. Text coverage measures 357 bright pixels for
WebGPU and 532 for GL, with the same 169x16 ink bounds; identical antialiasing bytes across
backends are not required. Input/focus/capture, docking and activation assertions stay intact.

The driver checks one real Dock primary and one unique connected DOM canvas, including removal
of Auto's failed detached candidate. It also verifies forced WebGPU refusal under that control
and the software-only GL refusal, rather than accepting another backend. Bounded independent
review identifies a negative-control gap: final candidate checks must also exclude attempts at
the other backend and validate the requested policy/control flags. Those checks are added and
source review closes the finding; the final driver rebuild and browser repeat follow.
This executes installed Edge 157.0.4341.42 at DPR 1 on Windows. It does not prove browser
context/device-loss recovery,
fractional/dynamic DPI, a native visible frame receipt, general GPU selection or performance.

The final complete WGPU library and all public integration binaries pass 123/123
(`2ad808a7-11d5-4143-812f-20849b48cf05`), including reflected surface member offsets, all shader
combinations, actual GPU atlas/clips/transforms/path/growth/refusal controls, image loading,
headless failure admission and text/layout reconstruction. No tests are skipped in that run.

Native configure-once source `e20d19d32f25734616243f15b7bfecefc633b8d7` is adapted through the
existing surface/context owner. The new independently locked ordinary public renderer
trajectory starts with a real HWND surface and zero requested drawable size, initializes its
context through adapter
configuration, resumes to a nonempty submitted Quad, shares that exact device/queue with a
second surface and suspends/resumes it. Both exact shutdown tickets quiesce before HWND
destruction, with error scopes/observers and bounded GPU polls. The HWND retains a nonzero
client area; the zero-size control targets the renderer request and its deferred state.
It passes alongside the two
dependency lifetime controls, 3/3 (`b43fa846-ec2d-43b6-b7cd-d941cdedf3ee`), on the same actual
RTX 4090/Vulkan adapter. It measures neither configuration-call count nor compositor pixels,
hybrid-GPU selection or physical device loss. It adds no native window owner to the framework.

The complete UI core/components/docking/Canvas cohort passes 2,966/2,966
(`9e44c7ee-86e4-491d-a747-da83227ad331`). The ordinary independently locked consumer passes
65/65 with all features (`57604b4d-852e-48a5-b0e2-e2c9211ed33c`), retaining the three explicitly
ignored native Vulkan cases executed separately above, and 48/48 with optional features
disabled (`383b9016-b480-4ced-a2e8-5ecdeac925ef`). Locked Rust 1.95 workspace all-target and
independent all-target/all-feature checks pass, as does affected Core/WGPU/Windows correctness
Clippy. Existing warning-level findings are retained.

Actual Rust 1.93.0 locked checks pass for the selected framework/native packages with all
targets/features, the ordinary consumer with all targets/features, the independently locked
test-support consumer with all targets/features and the hot-reload consumer with all targets.
The separate hot-reload panel also compiles. This establishes the declared dependency-driven
floor on the executed Windows target, rather than substituting a newer compiler or qualifying
foreign targets. Earlier Rust 1.92 records remain revision-scoped.

The rebuilt final driver passes its 14 policy/coordinate/reference tests
(`1a7250d5-38fe-483e-ac0c-25e6e632d04a`) and the complete real browser matrix again, including
the strengthened forced-backend attempt/policy/control checks. All four rendering cases pass
22/22 samples at both sizes, with text coverage and all original interaction controls.
The required run uses no `--allow-unavailable` skip. Browser and static-server processes are
retired by the existing driver.

Stable Rust 1.95 production wasm checks pass with profiling disabled and enabled. The existing
nightly/build-std shared-memory configuration also compiles the new WGPU/Web path with profiling
enabled; this is compilation, not a multithreaded browser execution claim. Native and production
wasm correctness Clippy pass. Root, ordinary-consumer and smoke-consumer formatting checks pass.

Resolved Windows x64, Linux x64, macOS arm64 and wasm graphs use the one exact Git WGPU source.
Linux/FreeBSD dependency declarations retain the official explicit backend set; other targets
retain defaults and wasm adds WebGL. These feature graphs do not establish foreign compilation
or native execution. The WGPU crate has no direct image edge with optional support disabled;
enabling test support restores that optional edge. Both WGPU and dev Naga resolve the same
source revision, while unrelated test-support/hot-reload lock selections remain unchanged.

Workflow YAML, whitespace and the existing import-boundary, public-API, document-link and
release-document gates pass. Bounded independent source review closes the driver finding and
finds no remaining confirmed P1/P2 in the native configuration/sharing/retirement scope.
All six implementation/review threads are closed. This completes the scoped renderer unit,
not the fixed-target refresh. Other native/browser environments, physical device/context loss,
fractional/dynamic DPI, numeric costs, the existing foreground-restoration gap, canonical
docking and Nara shared-GPU/View adoption remain open. No engine execution owner or UI default
is selected by these framework results.

## Frame-Time Debug Overlay

The November 9 continuation starts from `7cd732eda7f8c5c80e0d08b737504f720f6fa6f5`
against fixed Zed `635026fabc38440fce5e1767d125026a17b12012`. Official `07a0bd1220` introduces
the Apache-2.0 frame-time overlay. The maintained profiler prerequisite already measures draws,
but the public mode enum, Window controls and complete bitmap Scene consumer were absent.
Implementation `73472ca230bac117f8cb417d0cd6b0c1364c980e` imports all final-target recipes
and six official regressions, preserving
the bounded 1,000-sample history, Hidden/Minimal/Full behavior, duration percentiles, reset and
lifetime frame count.

The fork adapts ContentMask to the existing exact ClipStack/primitive owner, with identity
primitive transforms and viewport clipping. Scene geometry errors propagate to the existing
candidate rollback, keeping the old accepted scene and history until valid retry. Painting
occurs after root views and outside cache ranges; successful accepted draw checkpoints supply
the sample duration. Refused and unwinding attempts, old-scene presentation and idle callbacks
do not add samples. No overlay timer, font shaping, layout invalidation or alternate statistics
authority is introduced. These are draw-duration readouts, not GPU/compositor latency or
measured overhead.

Production all-feature Core library checking passes. First focused official/geometry/Window
owner tests pass 19/19 (`6d7d508d-e2f1-4d21-9bb4-419027f1f5ee`). This includes unchanged official
percentile/glyph/reset/count behavior, the full rolling window, scaled/exact clips, existing
scene isolation, cached view replay, no self-refresh loop, old-scene retries, atlas refusal,
render/accepted-cleanup unwind and actual geometry-refusal recovery.

The first ordinary CPU Scene consumer passes all mode/glyph/history/idle assertions, then exposes
retained headless resources after close: the historical TestWindow state still owns its renderer
and atlas (`58e98ed1-d01f-4ca3-bfbe-cb8808f3800e`, 1/2). The consumer's immediate view/resource/
App lifetime assertions remain intact. The existing prepared-quiescence operation now releases
the configured renderer and atlas outside its lock before acknowledgement, while a blocked
retry retains both. An owning regression checks the exact ticket, blocked/retry progression,
lock-free destructor reentry and single release while diagnostic state remains owned.

The first native overlay trajectory compiles but stops at an existing input-helper assertion:
the new fixture disabled trace while requiring three pending input samples
(`c106d71f-841d-4906-9c09-1a13bcf5de96`, 0/1). Enabling trace still reproduces the failure
(`f65e5ecf-3e93-4582-8aa5-318ccb6706de`), rejecting that initial diagnosis: input measurement
does not depend on trace. The fixture now uses the existing native settling operation between
capture/control activity and the reset/measured input batch, preserving the original three-input
assertion. The unchanged native mode/reset/pixels/resize/input/focus/retirement trajectory then
passes 1/1 (`e947d79d-e701-4e25-9882-a174123f393b`). Scoped trace restoration remains, and the
internal no-self-refresh control explicitly disables trace.

With resource quiescence corrected, both actual public CPU consumers pass 2/2
(`7e3b6055-025e-452c-a28f-91aa0e85e6a1`). They decode all bitmap readouts from complete Scenes,
exercise mode/reset/history, idle parking, two viewport sizes/reconstruction, platform resize,
root replacement and exact view/renderer/atlas/App release. Configured CPU renderer outcomes
are kept distinct from the separate native GPU evidence.

Source review classifies resource retention as a fork retirement-adapter gap. Official
TestWindow already shares renderer/atlas ownership and its TestPlatform retains an active
window. The fork adds exact retirement and last-created/diagnostic retention, but had not
released those resources at its new boundary. The correction preserves the official data
structure and existing shutdown owner rather than attributing a nonexistent historical-map
protocol to the official implementation. The owning resource correction and public-reentry
regression are committed separately at `d928c5459a8ed174d1a6835abd096b3e7a996b5a`.

The final resource regression also reenters public `AsyncApp::update`/`App::shutdown` from
renderer destruction, checking that the exact ticket remains unquiesced and nonterminal
throughout destruction. Its first compilation fails because the test calls `expect` on
`AsyncApp::update`'s unit return; the assertion is corrected to use the actual public signature.
The complete owner selection then passes 20/20
(`73889f22-f9a6-4ad6-a081-4580ca3f2db9`), including all six original official tests, seven
geometry/history controls, six Window/cache/refusal/unwind controls and the resource owner.
Bounded independent reviews close the overlay integration and resource correction, including
the corrected public shutdown reentry regression. All five implementation/review threads close.
Private direct recursive invocation of prepared shutdown is not qualified by the public App
flow; the existing native-drain phase serializes that flow.

Complete Core/WGPU/Windows library and public integrations pass 1,970/1,970
(`f315ff03-8b6e-4850-a6af-beb351322295`): Core/public 1,617, WGPU/public 123 and Windows 230.
The five skips comprise the four existing interactive cases and the explicitly ignored new
overlay trajectory. The previously failing foreground-restoration test passes unchanged in
this run; its earlier environmental failures remain recorded and their cause is not proven
resolved.

The separate ignored overlay rerun fails its Minimal panel-pixel assertion
(`18d5d4ea-bdbc-4e66-83cd-b6f64eb6b52f`). Source inspection finds that its new controls use
the last captured generation as their lower bound, although intervening settling can advance
the window with the same root source. The fixture now snapshots the current accepted generation
immediately before each mode/reset/placement operation and requires a newer actual Submitted
frame. The initial Hidden observation explicitly requests a fresh frame. No pixel, input,
resize, capture, focus or retirement assertion is weakened. The complete ignored trajectory
passes 1/1 (`39087646-5beb-4a7d-8ad6-ce4b4224467f`), followed by all five actual native
overlay/profiler/frame-signal/power/visibility trajectories passing unchanged
(`17389df3-289a-45be-8bb4-9dd1cbb8e380`). This corrects a definite fixture observation gap;
the recorded pixel failure alone does not establish a renderer defect.

Complete UI core/components/docking/Canvas consumers pass 2,966/2,966
(`01b26816-5c52-4aaa-8a94-17ac4c3bf589`), preserving all existing interaction, cache,
overlay, virtualizer and retained-state assertions.

The ordinary independently locked consumer passes 67/67 with optional features enabled
(`3d9de14a-ccf7-4ace-86f4-36fc24006bc9`, three existing explicitly ignored Vulkan controls)
and 48/48 with them disabled (`079997c3-8abf-46e8-a6f6-2d15d6bbc8a5`).
Its new two-case overlay target executes mode values and separately checks unexecuted Window
signatures; it does not claim a downstream native runtime. Complete profiling-disabled Core
library and public integrations pass 1,350/1,350
(`f07e4137-a3a7-4de9-aacb-a07860d1219a`), including the resource-retirement owner and
public reconstruction. The overlay module, controls and per-window state remain feature-gated.
The independently locked additive test-support consumer also passes 4/4 with all its features
(`36187e19-c2f4-4505-86da-cfba1c8ae2d8`), executing actual TestApp window cleanup, handle/
snapshot ownership, the ordinary utility binary and returning real application/benchmark
contexts. This is one feature selection, not a rerun of its earlier complete instrumentation
environment matrix.

A sixth bounded read-only pass confirms the native generation correction with no definite
P1/P2, then closes. Strict capture assertions can still expose an extra draw between waiting
and readback, or an extra post-reset draw; this potential false-negative timing scope is not
silently accepted as another frame's pixels. The same pass identifies source-confirmed official
animation scheduling, stateful spring integration and system notifications as remaining Core
mechanisms. Their explicit queue/plan entries are source evidence only, not implementation or
execution. Relocated display, missing-glyph and pointer-capture APIs are not classified as absent.

Locked Rust 1.95 workspace all-target compilation and both independently locked ordinary/
additive consumer all-target/all-feature checks pass. Affected Core/WGPU/Windows all-target/
all-feature correctness Clippy passes with existing warnings retained. Actual Windows Rust
1.93.0 checks pass for those framework packages and both independent consumers with all targets
and features; this does not qualify the compiler floor on another OS. Root, ordinary-consumer
and additive-consumer formatting completes without unrelated tracked changes.

Stable production wasm Core/WGPU/Web compilation passes with profiling disabled and enabled.
The existing nightly/build-std/shared-memory production configuration also passes with profiling
and multithreading selected. This is compilation evidence; this unit does not execute the overlay
in a browser, on macOS or on Linux. Prior renderer/browser qualification retains its own scope.
Existing import-boundary, public API, release-document, document-link and workflow YAML gates
pass. Remote CI is not executed.

This completes the scoped overlay implementation and qualification, with the recorded red runs,
fixture corrections and resource-owner provenance preserved. Draw duration is not compositor
latency, and no numeric overhead or frame budget is established. The bounded residual inventory
does not exhaust the fixed-target delta. Remaining animation/spring/notification/platform work,
historical foreground-test variability, canonical docking, full refresh and Nara GPU/View and
Editor/game-runtime adoption remain open; the parent goal stays active.

## Generic Animation Controls And Clock

The November 9 successor starts at `968f836f48a097ef49e692c62f5d1734c657bb75` against fixed
Zed `635026fabc38440fce5e1767d125026a17b12012`. Its local implementation is
`4ed461f2b591d6e44d84abc742094740fbd607af`. Official
`d06ebb7c033365a427b5cc8a3e2a2651bcb0c267`,
`4ed3738c4068495e12d4d78044b005e5ed9d3b8c`,
`dd04a229dd22700ff43815b8514453e197c333b7` and
`99e70ee4a02d8dcf1b72f487f81aaf0d3be74f76` supply App reduced motion, synchronized repeats,
per-animation frame limits and executor time. The complete non-Spring mechanism includes
ordinary builders, ParentElement composition, the shared App epoch and modulo before f32
conversion, chained static endpoints and image reduced-motion behavior. Stateful Spring and
the fork motion crate remain the separately planned successor. The App preference is explicitly
application-controlled here; automatic OS preference propagation and component motion-policy
reconciliation are not established by the generic element and image controls.

Retained fork adaptations use existing owners. A delayed animation notification waits through
App borrows and clears its pending flag inside the admitted closure before notifying, allowing
synchronous redraw to renew its next timer. FPS values whose reciprocal cannot form a finite,
nonzero, representable timer and deadline use ordinary frame scheduling; this adds no arbitrary
FPS ceiling. A weak reference to the exact AnimationState flag prevents a timer from keeping a
rejected first-mount state alive or notifying an accepted replacement view.

The source-only review finding for that last adaptation is reproduced before correction.
An accepted static view first mounts a 10 FPS animation in a candidate refused by an actual atlas
lease. A successful retry 50 ms later creates its own timer. At 101 ms the rejected candidate's
detached timer incorrectly renders the accepted view: four renders instead of three. Nextest
`5980708f-54ae-4885-ab27-7809bd91e1b5` fails that assertion. The correction retains existing
accepted-state timers while refusing notification after the rejected state's release, including
a static accepted retry. This is a fork candidate-transaction adaptation of the official detached
timer, not a change to the official shared clock or a second scheduling authority.

The first test build fails on a fixture treating the private bool-returning primitive insertion
as Result. The subsequent run `8fc9a3c6-7e29-41ef-bafc-bfedaa5d206b` passes 20/24: two fixtures
close the final window and inadvertently trigger default App shutdown before reopening; another
loads a view-subscribed image outside a rendered phase; the refusal fixture incorrectly asserts
that its deliberately rejected primitive is admitted. Explicit fixture quit policy, observation
inside the rendered view and the existing refusal outcome correct those fixtures without
weakening scene, cache, clock or lifetime assertions.

The corrected focused Core/public run `cfa2d915-df34-4688-901e-1dfd70b871a2` passes 26/26.
It includes all five official non-Spring tests, twelve further animation regressions, three image
policy/cache/loading cases, three actual candidate-refusal cases and three crate-external
workflows. The public CPU Scene renderer proves accepted/submitted generations, ordinary child
composition, two-window shared phase, cached views, reconstruction and exact renderer/atlas/App
release; it does not execute a GPU. The ordinary independently locked consumer distinguishes
executed builder/value/easing checks from explicitly unexecuted App/Window signature closures.

Run the focused owners, public consumer and actual Windows consumer serially:

```sh
cargo nextest run --locked -p open-gpui --all-features --lib --test animation -E 'test(elements::animation::) | test(elements::img::tests::reduced_motion::) | test(window::animation_tests::) | binary(animation)' --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --test animation --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked -p open-gpui-windows --all-features --lib -E 'test(=platform::native_test_support::native_animation_tests::native_generic_animations_progress_reduce_restore_and_retire_pending_frames)' --run-ignored only --test-threads=1 --no-fail-fast --no-tests fail
```

The actual Windows trajectory passes 1/1 at
`6c0250b7-3a8e-43a0-951c-7cc7c3df0261`, after correcting fixture-only access to Pixels' private
tuple field to use its public conversion. It observes real nonempty Submitted generations and
matching GPU pixels, input/focus, static reduced-motion endpoints and restoration. Its pending-close wait uses a FIFO
foreground barrier after animation tasks have registered their timers, so the deadline upper
bound cannot predate actual timer registration. Deterministic Core tests own exact deadline and
expired-notification assertions; native deadline observation supplies no hard OS callback-latency
or frame-rate budget.

The production-stable complete Core/WGPU/Windows/Gallery library/public cohort passes 2,138/2,138,
with seven declared skips (`97b52caf-890a-4c9b-88b6-5e44c617b040`). The new ignored native
animation trajectory is executed separately above. Historical foreground restoration passes
unchanged in this cohort; this does not resolve its recorded variability.

Independent rereview identifies a second fixture-only image observation issue: calling cache
load while still pending could subscribe on behalf of the tested Img and mask its missing
completion notification. The corrected observer first reads the current ImgState, calls load
only after loading has ended and then enters the cache's already-Loaded branch. Before reading
the observed Arc, the workflow requires an actual render increase, cleared loading state and one
image paint after HTTP completion. No production image code changes. The aggregate above uses
the earlier fixture; final focused execution of all three strengthened image cases passes 3/3
at `1f455552-d565-4309-b8ed-1698f86974e1` after formatting.
Both bounded independent reviews close without remaining P1/P2; the third source-analysis lane
records the Spring successor's explicit ownership and migration semantics in the plan. All six
implementation/review/analysis threads are closed.

The complete UI core/components/docking/Canvas cohort passes 2,966/2,966 without skips
(`77129fd1-8424-4d82-b734-394cb2ccc106`). The first independently locked build finds a fixture
annotation mismatch: adding `id` creates `Stateful<Div>`, not Div. Its explicit
`AnimationElement<Stateful<Div>>` annotation now matches the public type without weakening the
executed parent/child composition. No production interface changes for that correction.

Final independently locked consumers pass 68/68 with `profiler` selected
(`15f9725d-819b-4889-8fe2-75be7d87a5ff`) and 52/52 without optional features
(`33da70d7-295b-4ea8-a2f4-3b8aca289eb1`), without skips. The four new ordinary animation cases
execute builder/value/easing and parent construction while retaining explicitly unexecuted
App/Window signature probes. These selections do not enable the fixture's optional WGPU
renderer or Vulkan runtime controls; their earlier renderer qualification keeps its own scope.
The complete profiling-disabled Core library plus public animation target passes 1,310/1,310
without skips (`8a18e862-04a3-46c2-b76d-02f6a3697e50`). This includes the strengthened image
fixture and all generic animation owners without the profiler feature. Locked Rust 1.95 workspace
all-target and independently locked all-target/all-feature checks pass. Core/WGPU/Windows
all-target/all-feature correctness Clippy passes with existing warnings retained. Actual Windows
Rust 1.93 framework and independently locked consumer all-target/all-feature checks pass; no
compiler-floor or dependency change is introduced by this unit.

Stable production wasm Core/WGPU/Web checks pass with profiler disabled and enabled. The existing
nightly/build-std/shared-memory production configuration passes with profiler and multithreading.
These are compilation gates, not animation runtime/pixel execution in a browser or another OS.
Root and ordinary-consumer cargo fmt checks pass without unrelated source changes. Existing
import-boundary, public API, release-document and document-link scans and workflow YAML parsing
pass. Local CI commands are recorded; no remote CI execution or publication is performed.

This completes the scoped generic-animation implementation and qualification. Complete stateful
Spring/motion reconciliation and notifications remain queued, with source-backed ownership and
semantic migration in the plan. Foreign execution, numerical costs, full fixed-target refresh,
canonical docking and the real Nara GPU/View/Editor/game-runtime boundary remain open. The owner
clarification requires a qualified fixed baseline and modular engine seam before the later
separate framework-maintenance session; this unit does not claim that handoff complete.

## Official Spring And Shared Domain Sampling

The November 9 successor starts from `af682e37f36f8fc5de575ef485c1d247509a9d22` against
fixed Zed `635026fabc38440fce5e1767d125026a17b12012`. Local implementation is
`ecbbfa361ae4489b2de4121c5f0ca8b2a1d7aebb`. Official
`8b1497dbd22fb06f5838a7c0b84a1e54fafa71bc` and the final executor-clock correction
`99e70ee4a02d8dcf1b72f487f81aaf0d3be74f76` supply the full mathematical, typed-target,
playback and Element-ID Spring mechanism. ADR 0018 refines ADR 0016/0017 before implementation:
one pure solver lives in the existing motion crate, and GPUI exposes the same solver types while
owning official targets/interpolation, builders and element lifecycle. Domain sampling and policy
remain renderer-neutral, with no scheduler or engine-private dependency.

The initial import preserves the official solver and private helper bodies exactly, along with
all eight math tests and six Spring element tests. Fork domain options replace duplicate damping
formulas and the independent rest-speed threshold with the shared step and natural-frequency
settling rule. Physical configuration and review metadata are separate. Domain freeze-at-cancel
remains distinct from official Cancelled restoring the initial value; normalized domain progress
clamping is distinct from raw Spring overshoot. Initial motion qualification passes 89/89
(`ddbb2009-d8cf-4be4-b728-7b7d5650040a`) and three doctests.

Independent review exposes an inherited numeric defect in the official canonical conversion.
Common scaling of valid finite stiffness, damping and mass by `1e-30` or `1e30` underflows or
overflows their f32 product, corrupting the damping ratio. The original body fails three actual
scaled-configuration regressions (`b6bea980-1211-4665-9b57-996c4bdd277e`): infinity, an infinite
settling duration and NaN even in the zero-time propagator. Computing only the canonical ratios
and products with f64 intermediates corrects this reproduced scale failure. The public values
and subsequent solver remain f32, with no parameter clamp or universal numeric-range guarantee.
The complete corrected motion cohort passes 92/92
(`578cf645-2021-498c-8ded-6b4aa430edb3`). Canonical f64 intermediates may change final-bit rounding;
no bitwise parity with the previous arithmetic or cross-platform simulation guarantee follows.

The fork's candidate rejection also needs a concrete adaptation. The direct official mutable
element state advances to a target or playback requested by a candidate even when a later atlas
lease rejects that frame. Retrying with the original target then follows the rejected trajectory.
Actual owner tests `bd243d95-8ec3-49d3-bc99-edc8264e1df2` fail four of five cases: retarget,
Completed, Cancelled and paint unwind; fresh first-mount rejection already restarts correctly.
Two earlier fixture builds require explicit TestAppContext and AtlasTextureKind imports.

Spring now computes a candidate-local sample from its last accepted state and records publication
through the existing prepaint frame transaction. Acceptance commits the exact sample and its
view's next-frame demand; a refused or unwinding candidate commits neither. Returning the stable
state owner to the element map before invoking user animation/layout also preserves it through
callback unwind. Cached replay uses the same publication identity. This is a Spring-local
adaptation of the existing acceptance owner, not a generic cloned-state transaction mechanism.

Independent review also catches an unconditional current-view lookup in this adaptation that
would panic when directly drawing a settled Spring outside a rendered view. Only an active
sample now captures its view. A real VisualTestContext::draw regression covers that static path.
The first focused/public run passes 47/49 (`b349eb73-7a48-4197-a4e9-b440f596bd72`), including
all official and candidate controls. Two new public fixture assertions confuse exact color
interpolation with f32 arithmetic and unrounded authored width with Taffy's rounded paint width.
Channel-tolerance comparison and the actual rounded-width oracle correct those fixtures without
changing production interpolation, geometry or refusal behavior.

The corrected focused/public cohort passes 50/50
(`e72e9630-fc34-4369-9d42-a79292a1558e`), including the static draw regression, all official
Spring controls, five candidate cases, public typed/cache/playback/reconstruction and the
App-floor reveal workflow. Real CPU Scenes retain accepted/submitted generation and exact
view/renderer/atlas/TextSystem/App release assertions; they are not GPU execution evidence.

The initial domain-focused cohort passes 28/31
(`22d283ee-5257-4c96-aedc-2d0b4778adff`). Three new fixture failures do not justify changing
production geometry or weakening animation admission. Dock panel content excludes the existing
one-pixel border on each side, giving 198/398 pixels for half/full panes. Inserting a third
Splitter panel preserves its minimum-fraction constraints rather than dividing the authored
fractions by their sum: the left width is 63 pixels, and the extra panel uses its constrained
8/17 fraction of the 388-pixel panel extent, with both edges rounded at the actual device
scale. The active visual-affordance probe must run in a real rendered canvas prepaint;
calling it from an ordinary Window update has no current view for frame scheduling. The
corrected probes retain final geometry, unchanged-target completion, Idle frame demand,
unchanged source preferences and restoration assertions.
The first corrected run passes 30/31 (`2b037e20-96b5-4945-90be-16ece9526ef2`); its remaining
extra-panel assertion incorrectly rounds to whole logical pixels, while the test window's
scale permits half-pixel logical edges. The final oracle rounds each edge at the actual scale,
preserving the exact fraction and finite layout extent rather than broadening its tolerance.
The final domain-focused cohort passes 31/31
(`45aa36eb-f958-4a9c-96a2-b0cb5e220179`). A bounded independent rereview finds no remaining
P1/P2 in the adapter behavior, final geometry or real prepaint probes; it is source review,
separate from these executed controls.

The actual Windows trajectory passes 1/1
(`d8eefc2c-24b6-4b16-abaf-85cce3f124c2`). It uses the production Application/executor,
native HWND registration, input callbacks and DirectX renderer. Every observed control must
reach a newer nonempty Submitted generation, with matching paint source/revision, diagnostics
and GPU readback pixels. The trajectory retains outward velocity through retarget and pause,
observes reversal, and distinguishes a zero-velocity stop by resuming at the held position
through a real executor barrier. It checks Completed/Cancelled endpoints, active and new-target
reduced-motion behavior, restoration, logical focus and pointer/key input. Closing from actual
pending frame demand requires exact quiesce/destroy/terminal tickets, registration removal,
no further frame callbacks and view/native/renderer/App/platform release.

Native transient trajectories permit at most three fresh element epochs to observe their
intermediate phases, each with a captured static initial frame. Missed phases do not count
as success; source, generation, pixel or lifecycle assertion failures are not retried. The
executed test passes in 16.791 seconds. This does not establish a numeric frame-time budget,
physical foreground input, compositor scanout or display latency.

The complete Core/WGPU/Windows/Gallery library and public integration cohort passes
2,171/2,171 (`a5142c08-8fdd-48cc-8a5f-25a2c82e2645`) with eight declared skips. These retain
the previous seven skips plus the newly added, separately executed native Spring case. The
existing foreground-activation regression passes unchanged in this run; its historical
variability remains recorded rather than being considered repaired by the Spring unit.
The complete UI core/components/docking/Canvas library and public integration cohort passes
2,974/2,974 without skips (`c3357409-fa3e-45a7-8f1c-de2d6af51653`), retaining existing
layout, drag, selection, focus, virtualizer, overlay, source and retirement assertions.

The first independently locked enabled run passes 72/74
(`a1c1465c-ffe3-47b4-9bde-704e7a2f9ee3`). A new color endpoint assertion repeats the exact
f32 comparison mistake and now compares individual channels with its existing tolerance.
The other failure is a real inherited solver boundary: damping 40, target -4 and epsilon
0.001 produce a nominal conservative duration of 3.008299828 seconds, but the sampled
position is -3.9989998, just outside tolerance after f32 target translation. A focused
five-of-six diagnostic rerun (`a9fd0bf5-a97b-42e6-9d31-ce3f1429c9bd`) records those exact
values. An owning regression independently reproduces it before the fix
(`4e603177-3e57-42dc-848c-d37ebd3f45de`).

`settle_time` now moves rounded target-plus/minus-epsilon endpoints inward when their actual
distance exceeds epsilon, then uses the smaller representable distance for its existing
position envelope. Step, propagator, velocity threshold and all three decay formulas remain
unchanged. If epsilon is smaller than the target's representable spacing, a zero position
envelope is conservative and may take longer. This corrects the observed duration contract
without relaxing `is_settled`, padding test time or claiming arbitrary-range accuracy.
The full updated motion cohort passes 93/93
(`19b469c1-8b17-4154-8492-1649d8e5d401`) and three doctests. Its regression checks positive,
negative and large translations and multiple samples at/after the returned duration.
The complete framework/component cohorts above precede this final duration-only correction;
only pure duration/easing consumers use it, while active domain and element stepping use the
unchanged step/is_settled path. Final affected public/independent qualification follows below.

The corrected independently locked consumers pass 74/74 with profiling
(`450deb00-39b7-4e2e-9515-682a1ac40530`) and 58/58 without it
(`c8fd9768-6b2a-4879-99c3-ec24035555f2`). A subsequent independent numeric review finds a
second boundary missed by the first fix: with configuration (100,40,1), initial (5,-40) and
target 4, the opposing position contributions are about 0.0139272 and -0.0149271. The original
`(target + x) + y` rounds before cancellation, producing 3.9989998 at the returned duration.
The mirrored owning regression fails before correction
(`b194e2d1-bab0-4063-ba9e-7cda4b30fa52`).

Only fixed-target position accumulation now evaluates the target and both matrix contributions
in f64 before converting the final value to f32. The official matrix/decay formulas, velocity,
ramp path and public types remain unchanged. Together with the representable endpoint threshold,
this corrects the reproduced cancellation without loosening tolerances. The complete motion
cohort passes 94/94 (`220184cd-3dfb-4dc8-bee9-33c98e607053`); the final affected
public/domain/native paths and both feature selections
are rerun below. Earlier complete cohorts and the 74/58 consumer results precede this final
position-accumulation correction. The in-progress disabled Core build is deliberately interrupted
before execution to avoid qualifying the superseded arithmetic; no unrelated process is stopped.
No arbitrary-range, bitwise-parity or measured performance guarantee follows from these fixes.

The final affected Core/public Spring, candidate/refusal, domain and reduced-motion paths pass
43/43 (`6816c51f-9684-4d93-87f7-edebee2dcb0d`). The complete actual native Spring trajectory
passes again, 1/1 (`759b4b18-46fe-4210-aa4f-52bc272d81f0`), with all source, pixel, input,
velocity-carry, playback and retirement assertions retained. The bounded final numeric review
closes without remaining scoped P1/P2 and independently reproduces corrected mirrored values.
All thirteen implementation/review/inventory threads for this unit are closed.
Final independently locked Spring consumers pass 6/6 with profiler
(`ff663e35-5d78-413a-9c8b-6498196e3111`) and 6/6 without optional features
(`b975133e-bf08-406a-a330-c2dfc7b89693`), without skips. Their executed solver, typed value,
easing and parent-construction cases retain the separately named unexecuted signature probe;
the ordinary consumer does not enable GPUI test-support or claim native rendering.
The final profiling-disabled complete Core library and public Spring target pass 1,340/1,340
without skips (`63315640-40c5-46da-a37e-dd4f3a6a429c`), including the original generic
animation owners, candidate/resource lifetimes and all official/added Spring regressions.

Locked Rust 1.95 workspace all-target and independently locked all-target/all-feature checks
pass. Expanding correctness Clippy to the affected UI components exposes a preexisting
June 30 test literal, `4..1`, under `reversed_empty_ranges`. That test intentionally supplies
reversed offsets as data to normalization. An explicit `Range { start: 4, end: 1 }` retains
the exact same input/assertion without disabling the lint or changing production conversion.
The first expanded Clippy run fails only on that existing test; the corrected gate and its
owning test are rerun below. No dependency, lockfile or compiler-floor change follows.
Final all-target/all-feature correctness Clippy passes for Core/WGPU/Windows, motion, UI core,
components and docking, retaining existing warnings. The original reversed-range normalization
test passes with its unchanged assertion (`65a8bce3-08f4-4a60-89f8-4a30de39fa39`).
Actual Windows Rust 1.93.0 all-target/all-feature checks pass for Core/WGPU/Windows, Canvas,
platform, benchmark metrics, macros, motion, UI core, components and docking, plus the
independently locked consumer. The CI minimum-compiler job now includes the affected motion
and domain packages; this local execution is separate from unexecuted remote CI and foreign
native compiler/runtime qualification.

Stable production wasm Core/WGPU/Web compilation passes with profiling disabled and enabled.
The existing nightly/build-std/shared-memory production configuration also passes with profiler
and multithreading selected. These are compilation gates; Spring runtime/pixels in a browser,
macOS or Linux are not executed by this unit. Final motion doctests pass 3/3. Root and independent
consumer formatting checks pass. Existing import-boundary, public API, document-link and release
document scans and workflow YAML parsing pass, without new dependencies or policy scanners.
All work is local; no remote CI or publication is performed.

The App reduced-motion preference is a floor for actual Splitter, keyed-list, docking and
bring-into-view adapters, including active transitions. Domain preferences and captured layout
sources are not rewritten. The domain adapters' existing wall-clock implementation remains
separate from the official element's executor clock; this unit makes no general deterministic
component-timing or velocity-continuity claim. This completes the scoped official Spring and
shared-domain implementation/qualification. Full refresh, notifications and the bounded Core
residual inventory, canonical docking, measured costs and actual Nara engine/Editor/game-runtime
integration remain open. The maintenance handoff follows its separately qualified framework
baseline and concrete engine consumer; the full foundation goal remains active.

## Focused Product Gates

The U11 authority-convergence focused gate is:

```powershell
cargo nextest run --locked -p xtask --lib verify_plan_covers_u11_targets_features_and_process_scoped_environment --no-fail-fast
$env:CARGO_BUILD_JOBS = '1'
cargo nextest run --locked -p open-gpui-devtools --all-features --test framework_adapters --test resolved_semantic_redaction --test table_redaction --no-fail-fast
cargo test --locked -p open-gpui-devtools --all-features --doc
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery u11_gallery_convergence_smoke_composes_real_authorities_in_one_window --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery devtools_gallery --no-fail-fast
cargo run --locked -p xtask -- scan-theme-schema
cargo run --locked -p xtask -- scan-doc-links
```

`table_canaries_never_cross_any_devtools_artifact_boundary` is the Table privacy gate. It injects
unique Table id/label, column id/label, business row id, explicit instance id, grouped value, cell
value, identity diagnostic, and debug-selector forms, then proves none crosses live capture,
history, diff, Inspector detail/copy, session export, headless capture/export/report artifacts,
JSON/Markdown report, or the checked-in fixture. `table_opaque_ids_are_stable_only_inside_their_own_session`
proves that adapter-assigned ordinals are stable for one session without becoming reversible or
globally stable. The resolved-semantic target separately keeps form values/errors, accessible
text, labels, and input values out of the same downstream surfaces. The Gallery Inspector smoke
places a unique value on the real GPUI clipboard route, proves navigation neither ingests nor
replaces it, and then verifies the copied Inspector JSON remains redacted.
`devtools_gallery_headless_fixtures_match_producer_output` applies the shared canary absence check
to the generated Gallery capture, session, and report fixtures.

`framework_focus_snapshot_reads_the_rendered_window_authority` proves DevTools obtains focused
element presence and opaque claim/frame revisions from the real rendered window. Scope and handle
counts remain `None` when no complete producer registry can prove them; unknown counts must never
be serialized as guessed zeroes.

`u11_gallery_convergence_smoke_composes_real_authorities_in_one_window` is intentionally one real
window flow, not a catalog or hand-built snapshot assertion. It composes nested Popover -> Menu ->
Dialog topmost dismissal and LIFO focus restoration, sibling scoped themes with a deferred
surface, a real async-validating `FormStore`, final AccessKit value/busy/relations/action facts,
exactly-once Sidebar semantic activation, and fake-clock Tree typeahead reset. The focused command
is followed by the complete package gates in `xtask verify`; per-domain tests remain the owning
diagnostics when this convergence sentinel fails.

The full `open-gpui` nextest target is the unified gate for pointer-session, focus, input-dispatch,
and window-lifecycle regressions. Focused filters documented below provide faster implementation
feedback but do not replace that package-wide gate in `xtask verify` or CI.

The GitHub Actions `Verify` workflow also runs stable wasm surface checks on the Linux matrix:

```sh
cargo check -p open-gpui-web --target wasm32-unknown-unknown --tests --locked -j 1
cargo check -p open-gpui-web --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-platform --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-wgpu --target wasm32-unknown-unknown --locked -j 1
```

These are stable, single-threaded wasm gates. The Linux matrix also runs the stable browser smoke:

```sh
cargo run -p xtask -- web-smoke
```

`xtask web-smoke` builds `crates/gpui_web/examples/smoke_web` with Trunk, serves the generated files with cross-origin isolation headers, opens a headless Chrome/Chromium/Edge browser, and verifies app readiness, DOM/canvas initialization, focus/input delivery, a single-window shell interaction, the explicit unsupported platform-viewport capability on web, and a `DockSurface` viewport readiness/open probe that returns typed `backend_unsupported` without creating a window or runtime registration. The command fails when WebGPU is unavailable, so CI cannot pass without executing those behavior assertions. A developer diagnosing a machine without WebGPU may opt into `cargo run -p xtask -- web-smoke --allow-unavailable`; that explicit local skip is not a verification result. The smoke intentionally avoids the nightly shared-memory example so CI proves the default stable browser path.

Nightly shared-memory/atomics checks for `hello_web` remain optional verification, not CI requirements.

Release and public-documentation gates are split into two focused commands:

```sh
cargo run -p xtask -- verify-release-docs
cargo run -p xtask -- scan-doc-links
```

Before publishing a prepared release tag, generate the GitHub Release notes from the target version section:

```sh
cargo run -p xtask -- verify-release-docs --version <version> --notes-output target/release/release-notes.md
```

`verify-release-docs` checks the target changelog section, rejects manually wrapped changelog bullets and paragraphs, validates user-facing README dependency versions, and requires crate-local README metadata for public entry crates. Daily verification checks `docs/release/breaking-changes.md` against `CHANGELOG.md` `[Unreleased]`; release-note generation checks the same inventory against the selected version section because that is the text published to GitHub Releases. `scan-doc-links` checks strict user-facing relative links in root docs, release docs, verification docs, the ADR index, every engineering decision record, and public crate READMEs. Historical plans and progress logs remain outside the strict link gate until they are archived or indexed.

For the 2026-07 DevTools live runtime workbench slice, the focused local gates are:

```powershell
cargo fmt -p open-gpui-devtools -p open-gpui-ui-foundation-gallery -p open-gpui-docking-native
cargo check -p open-gpui-devtools --no-default-features --tests --locked
cargo check -p open-gpui-devtools --features gpui --tests --locked
cargo check -p open-gpui-devtools --features docking --tests --locked
$env:CARGO_BUILD_JOBS = '1'; cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --all-features --test diff_contracts --test session_contracts --test inspector_contracts --no-fail-fast --locked
cargo check -p open-gpui-ui-foundation-gallery --tests --locked
cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked
cargo check -p open-gpui-docking-native --tests --locked
cargo nextest run -p open-gpui-docking-native --no-fail-fast --locked runtime_status_panel
rg "fn (theme_snapshot|form_snapshot|resource_snapshot|docking_snapshot)" examples/ui-foundation-gallery/src/pages/devtools.rs
$sequencePattern = 'select_event\(0\)|select_event\(sequence|devtools-inspector:event:\{sequence\}'
rg -n $sequencePattern crates examples docs/knowledge/engineering
$debugIdentityPattern = 'format!\(".*DevtoolsEventIdentity|Debug.*DevtoolsEventIdentity'
rg -n $debugIdentityPattern crates examples docs/knowledge/engineering
cargo run -p xtask -- scan-doc-links
cargo run -p xtask -- scan-public-api --check
git diff --check
```

The `rg` static-builder and sequence/debug-selector guards should return no matches. DevTools
session replay is local/offline only: imports validate schema, protocol, bounded history, JSON
size, and event counts before recomputing diffs, and they do not introduce remote transport or
mutation APIs. Event selection is identity-first through `DevtoolsEventIdentity`; append sequence is
display metadata only. GPUI runtime metadata is intentionally narrow: app/window/focus/input/frame
and scroll counters and geometry are allowed, while raw user input, clipboard payloads, editable
text values, unredacted window titles, and accessibility labels remain outside the capture
contract. Docking runtime facts must come from public `DockViewportRuntimeStatus` records; missing
private facts are not inferred.

For the 2026-07 DevTools headless artifact pipeline, the focused local gates are:

```powershell
cargo fmt -p open-gpui-devtools -p open-gpui-ui-foundation-gallery -p open-gpui-docking-native -p xtask --check
cargo check -p open-gpui-devtools --tests --locked
cargo check -p open-gpui-devtools --all-features --tests --locked
cargo nextest run -p open-gpui-devtools --all-features --test artifact_contracts --test report_contracts --test docking_runtime_contracts --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked
cargo check -p xtask --locked
cargo nextest run -p xtask --test devtools_cli_contracts --no-fail-fast --locked
cargo run -p xtask -- devtools --help
cargo check -p open-gpui-ui-foundation-gallery --all-targets --locked
cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked
cargo check -p open-gpui-docking-native --all-targets --locked
cargo nextest run -p open-gpui-docking-native devtools --no-fail-fast --locked
cargo run -p xtask -- scan-public-api --check
cargo run -p xtask -- scan-doc-links
cargo run -p xtask -- verify-release-docs
git diff --check
```

The checked-in artifacts under `crates/devtools/tests/fixtures/` are the fixture owner for CLI
smoke: they cover report, diagnose, diff, stream, query, assert, follow, and bounded wait behavior
without launching Gallery or docking-native. `scan-public-api` includes the `open-gpui-devtools`
root export allowlist so artifact writer types remain intentional and report-rule internals remain
private. The headless pipeline remains local/offline only: no CDP bridge, remote transport, runtime
mutation API, screenshot baseline store, or persistent trace database is part of this gate.

For the 2026-07 scroll viewport and wheel-input intent slice, the core contract is that tracked
scroll surfaces emit committed post-layout viewport facts, typed wheel intent controls default
scrolling and propagation, and focus-on-wheel is opt-in rather than implicit. The focused local
gates are:

Runtime tests should collect final viewport facts through `ScrollViewportChangedEvent` or
`ScrollHandle::committed_viewport_snapshot`, assert simulated-input side effects through
`TestInputDispatchSnapshot` and `VisualTestContext::last_dispatch_event_result`, and assert focus
ownership through `VisualTestContext::debug_selector_is_focused` or
`VisualTestContext::focused_debug_selector`. These probes expose committed correctness facts only;
component render plans, transient dispatch flags, and broader P2 performance telemetry stay private.

```sh
cargo test -p open-gpui scroll_handle_committed_viewport_events --locked
cargo test -p open-gpui scroll_handle_programmatic_reveal_uses_named_source --locked
cargo test -p open-gpui scroll_lifecycle_capture --locked
cargo test -p open-gpui scroll_wheel_intent --locked
cargo test -p open-gpui test_input_dispatch_snapshot --locked
cargo test -p open-gpui plain_scroll_wheel_preserves_focus_without_opt_in --locked
cargo test -p open-gpui scroll_wheel_focus_intent_moves_focus_deterministically --locked
cargo test -p open-gpui test_child_wheel_handler_prevents_parent_list_scroll --locked
cargo test -p open-gpui-ui-components table --locked
```

Dependency health is enforced through:

```sh
cargo run -p xtask -- dependency-health
```

The command checks that every workspace package declares the workspace MSRV, that the declared MSRV is at least the maximum `rust-version` in the resolved dependency graph, that duplicate registry crate versions are explicitly allowlisted, and that `cargo audit --json` reports no unignored vulnerabilities. The current declared MSRV is Rust 1.93 for the pinned upstream WGPU/Naga correction; the prior Linux platform floor was Rust 1.92 through `oo7 0.6.0`. The [WGPU qualification](#wgpu-dependency-shaders-and-webgl) distinguishes completed compiler checks from the ongoing renderer/backend matrix. The dependency health workflow runs this command on Linux, and the release workflow requires a successful `dependency-health.yml` run for the release commit before publishing.

For the 2026-07 runtime UI hardening slice, the Web dispatcher exposes a typed
`WebDispatcherMode` through `WebDispatcher::mode()` and `WebPlatform::dispatcher_mode()`. Stable
web builds report `SingleThreaded { reason: BuiltWithoutMultithreadedFeature }`; multithreaded
shared-memory mode remains feature-, browser-capability-, and worker-startup gated. If workers
cannot be started after capability checks pass, the dispatcher reports
`SingleThreaded { reason: WorkerStartupFailed }` instead of panicking. The focused local gates for
this slice were:

```sh
cargo fmt --all
cargo fmt --all --check
cargo check -p open-gpui-web --target wasm32-unknown-unknown --tests --locked -j 1
cargo check -p open-gpui-web --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-platform --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-wgpu --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-windows --all-features --locked
cargo check --workspace --locked
cargo run -p xtask -- scan-ui-contract
cargo run -p xtask -- scan-import-boundary
git diff --check
(cd crates/gpui_web/examples/hello_web && cargo check --target wasm32-unknown-unknown -j 1)
```

The `hello_web` command uses the example's nightly/shared-memory wasm toolchain configuration and
is expected to emit Rust's `-Ctarget-feature=+atomics` warning.

The follow-up runtime/docking hardening pass keeps the same public capability posture and removes
several panic-oriented internal paths. Windows device-lost recovery errors are now logged and
retried instead of panicking, renderer refresh failures re-mark device invalidation, and optional
custom clipboard metadata/image formats are skipped when Windows format registration is
unavailable. The focused local gates that completed for this pass were:

```sh
CARGO_TARGET_DIR=/tmp/open-gpui-u1-check cargo check -p open-gpui-windows --all-features --locked
cargo test -p xtask public_api_snapshot --locked
cargo run -p xtask -- scan-public-api --check
cargo check -p open-gpui-web --target wasm32-unknown-unknown --tests --locked -j 1
cargo check -p open-gpui-docking --tests --locked
cargo fmt --all --check
git diff --check
```

Local `open-gpui` package checks, docking nextest runs, and repeated `xtask` scans can stall in
build-script or test-runner startup on the macOS workstation after prior cargo work has completed.
When that happens, stop the hung local command and use CI as the owner for full-workspace
confirmation rather than treating the incomplete local run as a pass.

The Windows platform backend no longer uses `unimplemented!()` for `hide_other_apps` or
`unhide_other_apps`; unsupported hide-other-apps behavior is a debug diagnostic/no-op on Windows.
The macOS host check proves the crate configuration still compiles locally, while the Windows CI
`open-gpui-windows --all-features` gate remains the final owner for Windows API coverage.

For the 2026-07 Zed GPUI upstream-sync utility slice, the focused U7 verification on the Windows
host was:

```powershell
cargo nextest run -p open-gpui-util
cargo check -p open-gpui-util --locked
cargo check -p open-gpui-util --target x86_64-apple-darwin --locked
cargo run -p xtask -- scan-import-boundary
git diff --check
```

The Windows Job Object process-tree tests ran on the host. The Darwin custom-`PATH` command tests
compiled for `x86_64-apple-darwin`; runtime execution still requires a macOS runner.

For the 2026-07 Zed GPUI upstream-sync SVG renderer slice, the focused U8 verification on the
Windows host was:

```powershell
cargo nextest run -p open-gpui -E "test(text_with_split_glyph_clusters_in_mixed_fonts_does_not_panic) or test(svg_renderer)"
cargo check -p open-gpui --locked
cargo run -p xtask -- renderer-smoke
cargo run -p xtask -- scan-import-boundary
cargo audit
git diff --check
```

The follow-up dependency remediation updated the actionable advisories: `quinn-proto` now resolves
to `0.11.16`, `anyhow` to `1.0.103`, `memmap2` to `0.9.11`, `async-tar` to `0.6.1` with the Tokio
runtime, `futures-lite` to `2.6.1`, `stacksafe` to `1.0.2`, `reqwest` to `0.13.4`, and `crossbeam-epoch` to `0.9.20`. `cargo audit`
now exits successfully. `.cargo/audit.toml` temporarily ignores the two `quick-xml 0.39.4`
advisories because the currently published `wayland-scanner 0.31.10` and `zbus_xml 5.1.1` releases
still pin `quick-xml = "0.39"`, and both reach this workspace through proc-macro/code-generation
paths rather than runtime XML parsing. Remove those ignores once upstream releases accept
`quick-xml >= 0.41`.

At that dependency audit, warning-class advisories are not denied by the local gate:
`paste 1.0.15` comes from `image`'s then-default AVIF encoding chain, and
`ttf-parser 0.25.1` comes through the font/SVG stack.
The AVIF encoder feature does not provide decoding; decoding needs the separately selected
`avif-native` feature, absent from this qualified graph. Official `8e06c323ae` removes the unused
default encoding chain while retaining the other default format features; the later
[dependency-policy qualification](#image-dependency-feature-policy) records that adoption.
Optional encoder packages can remain locked, so active-feature removal is not an advisory
resolution claim. Replacing the second requires a renderer/font-stack
migration. The updated SVG stack crates reviewed in this slice (`resvg 0.46.0`, `usvg 0.46.0`,
`imagesize 0.14.0`, `kurbo 0.13.1`, `polycool 0.4.0`, `roxmltree 0.21.1`, `svgtypes 0.16.1`) are
crates.io packages with MIT and/or Apache-2.0 licenses.

For focused `open-gpui-canvas` work, run:

```sh
cargo fmt -p open-gpui-canvas
cargo check -p open-gpui-canvas --benches
cargo nextest run -p open-gpui-canvas
cargo check -p open-gpui-smoke-native
```

When changing Canvas runtime query or cache ownership, also run:

```sh
cargo nextest run -p open-gpui-canvas spatial_cache runtime_query runtime --no-fail-fast
```

When changing Canvas document, tool, GPUI adapter, or root facade internals, also run:

```sh
cargo nextest run -p open-gpui-canvas document tool gpui public_surface_tests --no-fail-fast
```

The canvas crate also has a large-canvas Criterion baseline:

```sh
cargo bench -p open-gpui-canvas --features bench-support --bench large_canvas
```

Use the benchmark to compare spatial-index, visible-query, and paint-frame culling changes. It is
not part of the default CI gate because benchmark timing is runner-dependent.

For focused `open-gpui-ui-core`, `open-gpui-ui-components`, or UI foundation gallery work, run:

```sh
cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
cargo check -p open-gpui-ui-core
cargo check -p open-gpui-ui-components
cargo check -p open-gpui-ui-foundation-gallery
cargo nextest run -p open-gpui-ui-core
cargo nextest run -p open-gpui-ui-components
cargo nextest run -p open-gpui-ui-foundation-gallery
```

For focused `open-gpui-form`, `open-gpui-resource`, or `open-gpui-devtools` ecosystem work, run:

```sh
cargo fmt -p open-gpui-form -p open-gpui-resource -p open-gpui-devtools -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
cargo check -p open-gpui-form -p open-gpui-resource -p open-gpui-devtools --tests --locked
cargo check -p open-gpui-devtools --features command --tests --locked
cargo check -p open-gpui-devtools --features motion --tests --locked
cargo check -p open-gpui-devtools --features gpui --tests --locked
cargo check -p open-gpui-devtools --all-features --tests --locked
cargo check -p open-gpui-devtools --features form,resource --tests --locked
cargo check -p open-gpui-devtools --features gpui,motion,docking --tests --locked
cargo check -p open-gpui-devtools --no-default-features --features form --tests --locked
cargo check -p open-gpui-devtools --no-default-features --features resource --tests --locked
cargo check -p open-gpui-devtools --no-default-features --features motion --tests --locked
cargo check -p open-gpui-devtools --no-default-features --features docking --tests --locked
cargo check -p open-gpui-devtools --no-default-features --features gpui --tests --locked
cargo check -p open-gpui-devtools --no-default-features --features ui-components --tests --locked
cargo check -p open-gpui-ui-components --tests --locked
cargo check -p open-gpui-ui-foundation-gallery --tests --locked
cargo nextest run -p open-gpui-form -p open-gpui-resource -p open-gpui-devtools --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --test inspector_contracts --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features command --test command_adapters --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features motion timeline --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features gpui layout --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features form,resource form_resource_adapters --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features gpui,motion,docking framework_adapters --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-components form_adapter resource_adapter --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-foundation-gallery form resource devtools --no-fail-fast --locked
```

`open-gpui-form` and `open-gpui-resource` are renderer-neutral crates; their tests must not require
a live GPUI window. `open-gpui-resource` remains protocol-agnostic, so resource tests should drive
query and mutation generations around fake app-owned fetch results instead of introducing HTTP
policy. Devtools is read-only: tests should assert snapshot collection, filtering, diagnostics,
selection, target/domain/event capture, selected-detail JSON export, and redaction summaries
without mutating app state. On Windows, if `open-gpui-devtools --all-features` nextest hits
`link.exe` LNK1102 while linking test binaries, rerun the same gate with `CARGO_BUILD_JOBS=1`; do
not treat the first linker out-of-memory as a code pass.

The Components gallery has an `ecosystem-adapters` section for `FormFieldProjection`,
`ResourceCollectionProjection`, and `ResourceMutationProjection`. DevTools consumes those same
redacted form/resource snapshots through feature-gated first-party adapters, and framework facts
come from the `ui-components`, `gpui`, `motion`, and `docking` adapter modules when public
read-only snapshots exist. The DevTools gallery page is a separate registry-backed and
capture-backed dogfood page:

```sh
cargo run -p open-gpui-ui-foundation-gallery -- --page devtools
```

For the current `VirtualizedList` and `open-gpui-motion` foundation, run the focused gates below
before relying on the full workspace gate:

```sh
cargo metadata --no-deps --format-version 1
cargo test -p open-gpui-ui-core --locked virtualizer
cargo test -p open-gpui-ui-components --locked --lib virtualized_list
cargo test -p open-gpui-ui-components --locked --test public_surface
cargo test -p open-gpui-ui-components --locked --test layout virtualized
cargo check -p open-gpui-ui-foundation-gallery --tests --locked
cargo test -p open-gpui-ui-foundation-gallery --locked --test foundation_gallery component_catalog_contracts
cargo test -p open-gpui-ui-foundation-gallery --locked --test foundation_gallery component_smoke_tree_virtualized
cargo check -p open-gpui-motion --tests --locked
cargo nextest run -p open-gpui-motion --no-fail-fast --status-level fail --locked
cargo test -p open-gpui-motion --doc --locked
```

These gates cover key-based virtualized state, typed and custom-rendered rows, measured-row
snapshots, typeahead, replacement-style range selection, sticky-section snapshot metadata,
active-indicator motion demand/reduced-motion/offscreen behavior, typed public export ownership,
and gallery scroll/keyboard containment.

For current crate discovery, the normal-checkout user entry points are:

```sh
cargo run -p open-gpui-ui-foundation-gallery
cargo run -p open-gpui-docking-minimal
cargo run -p open-gpui-docking-native
cargo run -p open-gpui-canvas-notes
```

The minimal docking example is the common API entry point. The native docking example is the
dogfood surface for viewport runtime diagnostics and multi-window capability gates. The component,
motion, docking, web, and platform crate guides live at `crates/ui_components/README.md`,
`crates/motion/README.md`, `crates/gpui_docking/README.md`, `crates/gpui_web/README.md`, and
`crates/gpui_platform/README.md`. Public package metadata should point at crate-local README files.

The gallery package includes Components-page runtime smoke coverage for regressions that state-only
tests can miss: short-viewport page scrolling and navigation reset, navigation rail scrolling,
Select popup outside dismissal, nested ScrollArea wheel scrolling, vertical Tabs rail scrolling,
horizontal plus vertical Splitter pointer dragging, Table column resize dragging, and long Sidebar
internal navigation scrolling. Run the gallery package tests before relying on manual dogfood for
those paths.
Overlay gallery smoke coverage now also includes menu submenu hover-open and sibling branch
switching on the rich-items sample, so submenu branch visibility, local hover retention, and
old-branch dismissal are verified through the real gallery shell instead of only through
component-state tests.
Component-state coverage also includes `MenuSubmenuSurface` and `MenuSafeHoverCorridor`, which
prove renderer-neutral submenu placement inputs and safe-hover transition bounds for the floating
submenu panels.
The Components-page ScrollArea regressions also cover release-queue wheel isolation so scroll
gestures on the sample card chrome do not leak to the page shell.
Because the Components page now carries more depth samples, the longer-section smokes also rely on
catalog directory jumps and page-scroll handle alignment instead of only raw page wheel motion;
that keeps the focused inspection paths stable even as the page grows.
The Components page has two inspection modes: the full all-components conformance page, and a
catalog-driven focused component-family view. Directory chips remain pure anchor jumps. Focused
mode is entered from catalog cards and restored through the explicit `All components` control. The
focused-view proof includes a catalog-driven matrix that opens every focusable official or
state-contract catalog entry, plus focused runtime smokes for scroll reset and nested scroll
containment:

```powershell
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focuses_every_focusable_catalog_entry
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focuses_catalog_family_and_restores_all_mode
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_table_scroll_stays_inside_sample
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_mode_resets_page_on_family_change
```

For focused docking split, preview, motion, zoom, divider, and accessibility primitive work, keep
the gates aligned to the shared primitive boundary:

```sh
cargo fmt --all -- --check
cargo nextest run -p open-gpui-motion --no-fail-fast
cargo check -p open-gpui-ui-core --tests
cargo nextest run -p open-gpui-ui-components splitter --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests --no-fail-fast
cargo check -p open-gpui-docking-native
git diff --check
```

For docking render-authority convergence work, prove deterministic geometry through
`DockPresentationScene` parity rather than screenshot or pixel-level styling parity:

```sh
cargo fmt --all -- --check
cargo nextest run -p open-gpui-docking host_render_tests host_render_geometry_parity_tests host_presentation_scene_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_viewport_preview_tests host_viewport_preview_visual_tests host_viewport_route_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_divider_hit_map_tests host_accessibility_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_interaction_tests --no-fail-fast
cargo check -p open-gpui-docking
cargo check -p open-gpui-docking-native
git diff --check
```

This gate locks root, leaf, tab-bar, empty-space, floating-title/content, split child, splitter,
zoom, divider hit map, and accessibility rectangles to the same deterministic scene/layout
authority. The remaining render-measured probe is intentionally named
`render_tab_label_drop_scene_fact_probe` and may only publish tab-label facts whose final bounds
depend on GPUI text shaping, intrinsic title layout, or close-button layout.

Use narrower checks while iterating:

```sh
cargo nextest run -p open-gpui-ui-core split --no-fail-fast
cargo nextest run -p open-gpui-ui-components splitter gpui_adapter_maps_splitter_role --no-fail-fast
cargo nextest run -p open-gpui-docking geometry host_accessibility_tests host_divider_hit_map_tests workspace_resize_policy_tests graph_split_tests interaction --no-fail-fast
cargo nextest run -p open-gpui-docking spatial_navigation_tests host_zoom_focus_tests::host_focus_neighbor_command_uses_spatial_navigation host_accessibility_tests::accessibility_splitter_actions_resize_through_transaction_path host_accessibility_tests::accessibility_vertical_splitter_actions_target_vertical_axis host_divider_hit_map_tests host_interaction_tests::horizontal_splitter_drag_updates_width_fractions host_interaction_tests::vertical_splitter_drag_updates_height_fractions host_interaction_tests::splitter_drag_clamps_to_minimum_pane_size host_interaction_tests::corner_splitter_drag_updates_both_axes_through_rendered_events interaction::tests::corner_splitter_drag_produces_two_axis_resize_request interaction::tests::corner_splitter_drag_clamps_one_axis_without_corrupting_other_axis render::tests::divider_affordance_states_have_distinct_feedback_colors --no-fail-fast
cargo nextest run -p open-gpui-docking transition_plan_from_route_affordance_describes_source_marker source_hover_over_known_viewport_renders_target_drop_preview routed_preview_replacement_clears_old_target_overlay_without_stale_payload escape_clears_routed_marker_target_overlay_and_active_drag viewport_runtime_begin_payload_drag_clears_previous_routed_preview viewport_runtime_revalidates_routed_preview_release_against_current_policy --no-fail-fast
cargo nextest run -p open-gpui-docking transition_pane_clip_mounts_real_pane_content host_unzoom_command_retargets_from_active_zoom_sample dragging_tab_to_other_stack_center_moves_panel transition_plan_from_overlay_scene_uses_current_bounds_for_matching_layers transition_plan_keeps_preview_layers_at_current_target_bounds overlay_replacement_keeps_preview_layers_at_current_target_bounds --no-fail-fast
```

These checks prove capability alignment instead of pixel parity: tab insertion previews remain tab
previews, nested edge targets stay scoped to the pane that owns the guide, cross-window route
markers stay separate from target previews, zoom/focus produce deterministic descriptors, divider
and corner hits derive from the shared split hit map, and accessibility descriptors expose roles,
bounds, orientation, selected state, disabled state, and actions.

The shared motion runtime checks additionally prove that `open_gpui_motion` owns deterministic
timeline sampling, spring sampling, scalar values, model-neutral scalar samples, frame-demand
reasons, explicit model/preset resolution, layout projection data, motion policy validation,
terminal state, reduced-motion completion, stable-identity retarget matching, and renderer-neutral
projection clips. The public motion surface is the controller/model/policy/projection layer;
`MotionValue` remains a private implementation detail behind `MotionScalarTrack`, and
`open_gpui_ui_core` must not re-export motion contracts.
`SplitterLayoutTransition::sample` now exposes final-content bounds plus visible
clip bounds for insert, remove, resize, collapse, and expand transition descriptors. The GPUI
Splitter adapter consumes those samples for programmatic identity/count/collapse/expand changes via
an overlay path; leaving panel content is retained when callers use view-backed
`SplitterPanel::view` panels, while one-shot element-backed panels keep their existing render
limitations.
`ui_components::Splitter` uses the scalar controller and explicit committed-layout model for
programmatic fraction changes while keeping pointer drags immediate and policy-tested. Docking uses
the same scalar motion model for transition progress, keeps explicit custom timeline specs intact,
renders move/resize panes through renderer-neutral projection visual bounds plus final-size
clip/occlusion layers, and keeps pane, divider, visual-affordance, zoom, focus, tab, route, and
viewport semantics local. Transition pane clips mount real final-size pane content behind an
occlusion mask rather than generic placeholder rectangles, visual-affordance preview geometry stays
pinned to the current semantic target, adapter-owned transition executors still request GPUI
frames, and interrupted zoom/unzoom starts from the current sampled geometry. The native runtime
panel exposes this as
`motion proof: shared-runtime+run-state+scalar-value+scalar-sample+explicit-models+policy-gates+layout-projection+projection-clips+sampled-progress+retargeted-identity+reduced-motion-final-state+high-frequency-bypass`.
The remaining render-measured drop-scene probe is intentionally limited to tab-label facts whose
bounds depend on text shaping; presentation-scene facts own root, pane, tab bar, empty, and
floating-title targets.

Cross-window preview cleanup is part of the same semantic contract. A routed hover may leave a
source-window route marker and a target-window preview at the same time, but those are distinct
overlay layers: source route markers become `RouteMarker` transition descriptors, target previews
own payload tab previews, and replacing the route target must clear the old target overlay before a
release can commit. Escape cancellation during a real GPUI docking drag clears the active drag,
source marker, target preview, and runtime session together without making the docking host steal
ordinary panel focus when no drag is active.

Shared split primitive coverage now owns generic fraction normalization, one-fill-child share
resolution, and pixel-delta adjacent resize helpers in `open_gpui_ui_core`. Docking consumes those
helpers for graph normalization, render flex shares, presentation-scene split layout, and splitter
drag resize transactions. Docking-local geometry should remain limited to docking-specific
drop-guide boxes, central-region target policy adapters, and GPUI `Bounds<Pixels>` conversion.
Docking-private spatial navigation now resolves nearest pane focus targets from the current
presentation scene using direction filtering, perpendicular overlap priority, and distance
tie-breaking. The direction enum is a docking command input, while the rectangle-neighbor resolver
remains private to docking because it depends on docking pane semantics and rendered presentation
facts.

Current docking accessibility output maps supported descriptor data into GPUI element state:
stable IDs, roles, labels, selected/disabled state, orientation, numeric splitter values, tab
focus/select actions, and splitter increment/decrement actions. Docking keeps hint strings and
drop affordance descriptors in its renderer-neutral scene, but GPUI currently has no generic
element API for an accessibility hint/description field or a platform drop action callback. Active
drop, drag-source, and rejected-target affordance nodes are therefore exposed as labeled group
descriptors without inventing unsupported platform actions, and focused tests assert that those
nodes disappear when the visual affordance scene is empty.

Docking visual affordance runtime work should use `DockVisualAffordanceScene` as the visual
feedback authority for drop guides, tab insertion, route markers, divider/corner affordances,
focus rings, zoom egress, accessibility, visual-affordance motion identity, and native diagnostics.
Target previews, route markers, accessibility descriptors, transition plans, and runtime
diagnostics now consume visual affordance descriptors directly; no `DockOverlayScene` semantic
adapter remains. The native docking runtime panel reads runtime-owned visual affordance status and
shows one compact affordance line per viewport with layer count, active layer, scope/state, target
node, zone, payload index, frame generation, and visual-affordance motion state.

Focused visual affordance runtime gates:

```sh
cargo fmt --all -- --check
cargo nextest run -p open-gpui-docking host_viewport_preview_tests host_viewport_preview_visual_tests host_viewport_route_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_render_tests host_transition_tests host_render_geometry_parity_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_accessibility_tests host_divider_hit_map_tests host_debug --no-fail-fast
cargo nextest run -p open-gpui-docking host_interaction_tests native_captured host_viewport_drop --no-fail-fast
cargo check -p open-gpui-docking
cargo check -p open-gpui-docking-native
git diff --check
```

Native dogfood command:

```sh
RUST_LOG=info,open_gpui_docking=debug,open_gpui=info RUST_BACKTRACE=1 cargo run -p open-gpui-docking-native --bin open-gpui-docking-native 2>&1 | tee /tmp/open-gpui-docking-native.log
```

Table gallery gates now follow the same split: `open-gpui-ui-core` tests prove row-model,
manual row-model stages, manual expansion, child-load metadata, virtualizer, column sizing,
column-window, row pinning, and resize-math contracts without rendering, including grouped row ids,
expansion lookup behavior, expandable unloaded branches, built-in group-row aggregate cells,
pinned-column region splitting, center-column virtual windows, top/center/bottom row regions,
keep-pinned versus page-only policies, manual filtering/sorting/pagination cache keys, pagination
row/page totals, per-column facet metadata, manual facet payload cache keys, and on-end/on-change
resize deltas. The same core gate proves exact source/group identity through every row-model stage,
snapshot-scoped occurrence invalidation, explicit duplicate instances across source reorder, and
canonical completion of partial column order. `open-gpui-ui-components` tests prove adapter exports,
state metadata, manual row-model render-plan metadata, faceting render-plan metadata, row-pinning
render-plan metadata,
expansion payload metadata, resize callback wiring, center-window header/body mounting, fixed
row-pinned bands, exact edit outcomes, duplicate NodeId and measurement separation, virtual focus
proxy behavior, and scroll ownership; gallery smokes prove long table scroll input stays inside
the table viewport, `release-resize` column dragging updates the controlled sample without moving
the outer Components page, wide center lanes scroll independently from fixed left/right pinned
lanes, `row-pinning` keeps top/bottom row bands fixed while the center body scrolls, `server-paged`
renders an app-owned page snapshot with total counts plus caller-provided facet summaries,
`filter-board` exposes client-derived status counts and score ranges, and `server-tree` renders
app-owned manual child loading. The focused proofs are:

`components_gallery_smoke_grouped_table_pinned_center_scroll_stays_inside_sample` is the focused
sticky-pinned Table proof: it enters the Table family view, scrolls the `release-rollup` center
lane horizontally, and asserts left/right pinned lanes plus the outer Components page stay fixed.

`components_gallery_smoke_matrix_table_center_column_window_stays_inside_sample` is the focused
center-column virtualization proof: it enters the Table family view, scrolls the `release-matrix`
center lane horizontally, verifies far center metric cells are unmounted before scrolling and
mounted after scrolling, and asserts left/right pinned lanes plus the outer Components page stay
fixed.

`components_gallery_smoke_row_pinning_table_scroll_stays_inside_sample` is the focused row-pinning
proof: it enters the Table family view, aligns the `row-pinning` sample to an interactive center
cell, wheels inside the center body, and asserts the sample, top pinned band, bottom pinned band,
and left-pinned cells stay fixed while the center row window changes.

`components_gallery_smoke_table_server_tree_loads_children_from_expansion_request` is the focused
manual-expansion proof: it enters the Table family view, starts with `server-api` absent from the
app-supplied source snapshot, clicks the unloaded `server-workspace` disclosure, verifies the
expansion payload carries zero loaded children plus idle child-load state, and then confirms the
new child row renders after the gallery runtime supplies the loaded snapshot.

`components_gallery_smoke_faceted_filter_updates_table_rows` is the focused faceted-filter proof:
it enters the Table family view, opens the `filter-board` status popover, verifies wheel input on
the popup content stays local, selects the exact `Done` token, checks the controlled change payload
and filtered row counts, then toggles the token off and confirms the original row window returns.

`components_gallery_smoke_range_filter_updates_table_rows` is the focused numeric range proof:
it enters the Table family view, opens the `filter-board` score popover, verifies wheel input on
the popup content stays local, types a minimum score, checks the controlled
`TableRangeFilterChange` payload and filtered row counts against the same `TableState` contract,
and confirms a lower-score row leaves the rendered window.

`components_gallery_smoke_predicate_filter_updates_table_rows` is the focused predicate-filter
proof: it enters the Table family view, types into the `filter-board` name predicate control,
checks the controlled `TablePredicateFilterChange` payload and sample-owned predicate override,
verifies filtered/final row counts against the resolved `TableState`, and confirms the rendered
row window changes without moving the outer Components page.

`components_gallery_smoke_editable_table_cell_updates_sample_rows` is the focused text-cell editing
proof: it enters the Table family view, targets the `editable-release` sample, edits a rendered
`name` cell through the nested `TextInput`, verifies `TableCellEditChange` targets the exact
`(TableRowIdentity, TableColumnId)` pair, confirms the gallery applies the change to its app-owned
`TableState`, and proves a read-only `status` cell does not mount an editor.

`components_gallery_smoke_checkbox_table_cell_updates_sample_rows` is the focused checkbox editing
proof: it enters the Table family view, targets the `toggle-release` sample, toggles a rendered
`enabled` cell through the nested `Checkbox`, verifies `TableCellEditChange` targets the exact
`(TableRowIdentity, TableColumnId)` pair, confirms the gallery applies the bool change to its
app-owned `TableState`, and proves the checkbox cell does not mount a text editor.

`components_gallery_smoke_multiline_table_cell_updates_sample_rows` is the focused multiline
cell-editing proof: it enters the Table family view, targets the `multiline-release` sample, edits
a rendered `notes` cell through the nested `Textarea`, verifies the same exact identity pair and
newline-preserving `TableCellEditChange` payload, confirms the gallery applies the change to its
app-owned `TableState`, and proves non-multiline/read-only cells do not mount the wrong editor.

`components_gallery_smoke_content_fit_table_cell_edit_widens_name_column` is the focused
content-fit proof: it enters the Table family view, targets the `content-fit-release` sample,
edits the visible `name` cell, verifies the sample keeps the fixed `score` lane anchored, and
proves the adapter-measured `name` column widens while header and body stay aligned.

`table_runtime_measured_row_height_reflows_after_paint` is the focused measured-row proof: it
renders a measured `Table` with wrapped body content, verifies the first row grows beyond the
fallback row height, and confirms the second row is laid out below the expanded row after the
measurement cache settles.

`components_gallery_smoke_select_table_cell_updates_sample_rows` is the focused select-edit
proof: it enters the Table family view, targets the `select-release` sample, opens a fixed-option
`Select` editor, picks `blocked`, verifies `TableCellEditChange` targets the exact
`(TableRowIdentity, TableColumnId)` pair, confirms the gallery applies the text change to its
app-owned `TableState`, and proves the select cell does not activate or select the row.

`open-gpui-ui-components` table tests also cover the select editor adapter path directly:
`table_behavior_snapshot_exposes_editable_leaf_cell_kinds_for_leaf_cells_only`,
`table_runtime_select_cell_edit_emits_change_without_row_interaction`, and the other table cell
edit gates prove the fixed-option `Select` editor stays a leaf-cell recipe rather than a new row
interaction path.
The focused U5 identity gates are
`occurrence_identity_is_scoped_to_the_source_snapshot`,
`explicit_duplicate_identity_survives_every_row_model_stage`,
`column_order_normalizes_duplicate_unknown_and_partial_ids`,
`partial_column_order_reorder_normalizes_source_order_before_visibility_and_pinning`,
`ambiguous_business_id_edit_is_an_inspectable_no_op`,
`stale_occurrence_edit_cannot_retarget_a_reordered_duplicate`,
`duplicate_measurements_follow_explicit_identity_across_source_reorder`,
`duplicate_source_rows_publish_distinct_stable_accessibility_nodes`,
`table_virtual_focus_proxy_preserves_keyboard_claim_without_stealing_focus`, and
`table_focus_falls_back_only_when_identity_leaves_the_final_model`. Together they prove that
occurrence identity cannot retarget a newer snapshot, explicit instances survive reorder, partial
order retains unlisted source columns, duplicate edit/measurement/node paths remain disjoint, and
offscreen logical focus stays keyboard-actionable without reclaiming focus moved outside Table.
The Table modules are now verified by ownership layer. `open-gpui-ui-core` owns renderer-neutral
row-model, column, header, filtering, faceting, aggregation, sizing, selection, and virtualizer
contracts. `open-gpui-ui-components` owns the `Table` facade, behavior snapshots, crate-private
render-plan resolution, keyed runtime, header/body/cell/editor/resize element assembly, callback
payloads, and typed public export declarations.
`open-gpui-ui-foundation-gallery` owns the end-to-end samples and scroll containment proofs. For a
Table-only change, prefer the focused commands below before the full `xtask` gate; keep the public
surface and Gallery metadata commands when moving public paths so owner drift is detected early.

```powershell
cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
cargo check -p open-gpui-ui-core --tests
cargo check -p open-gpui-ui-components --tests
cargo nextest run -p open-gpui-ui-core table
cargo nextest run -p open-gpui-ui-components --test table --no-fail-fast
cargo test -p open-gpui-ui-core -p open-gpui-ui-components --doc --locked
cargo nextest run -p open-gpui-ui-foundation-gallery table
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata gallery_contract_metadata_matches_component_rows components_gallery_smoke_focused_table_scroll_stays_inside_sample components_gallery_smoke_grouped_table_pinned_center_scroll_stays_inside_sample components_gallery_smoke_matrix_table_center_column_window_stays_inside_sample components_gallery_smoke_row_pinning_table_scroll_stays_inside_sample
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_faceted_filter_updates_table_rows components_gallery_smoke_range_filter_updates_table_rows components_gallery_smoke_predicate_filter_updates_table_rows components_gallery_smoke_column_visibility_updates_release_matrix components_gallery_smoke_resizable_table_resize_updates_sample components_gallery_smoke_grouped_table_column_reorder_updates_sample
cargo nextest run -p open-gpui-ui-core numeric_range_filters_match_finite_number_cells_inclusively numeric_range_filters_normalize_open_and_reversed_bounds categorical_filters_match_exact_tokens_and_multiple_values
cargo nextest run -p open-gpui-ui-components --test table --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test text_input --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata gallery_contract_metadata_matches_component_rows components_gallery_smoke_focuses_catalog_family_and_restores_all_mode components_gallery_smoke_editable_table_cell_updates_sample_rows components_gallery_smoke_checkbox_table_cell_updates_sample_rows components_gallery_smoke_select_table_cell_updates_sample_rows components_gallery_smoke_multiline_table_cell_updates_sample_rows
```

Key sentinels inside those binaries include
`row_model_pipeline_executes_each_stage_before_final_pinning_partition`,
`official_components_match_typed_public_exports`,
`common_extended_diagnostic_and_adapter_paths_compile`,
`gallery_contract_metadata_matches_component_rows`,
`table_range_filter_state_resolves_bounds_and_popover_contract`,
`table_range_filter_change_updates_filters_and_resets_pagination`,
`table_behavior_snapshot_exposes_faceting_metadata`,
`table_behavior_snapshot_exposes_editable_leaf_cell_kinds_for_leaf_cells_only`, and
`controlled_text_input_on_change_accepts_input_without_supplied_controller`.

`VirtualizedList` follows the same split at component scale: `open-gpui-ui-components` tests prove
render-plan rows, scroll-target math, PageDown reveal, and Enter/Space activation payloads, while
the gallery metadata and smoke tests prove the official catalog entry, 10k-item rendered sample,
and inner scroll containment inside the overflowing Components page. The focused proof is:

```powershell
cargo nextest run -p open-gpui-ui-components virtualized_list
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_virtualized_list_scroll_stays_inside_sample
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_virtualized_list_card_wheel_does_not_leak_to_page
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_virtualized_list_keyboard_reveals_and_activates
```

The components package includes runtime smoke coverage for Switch, TextInput, Textarea, RadioGroup,
Listbox, Select, Combobox, Command, Tabs, and Toolbar keyboard navigation. The focused Switch test
renders a controlled switch, clicks its real root selector, verifies `on_change` receives the next
checked value, and confirms disabled switches do not emit changes. The focused TextInput tests
render a standalone controller-backed input, click its real root, accept simulated platform text,
sanitize single-line input, verify the controller caret ends at the inserted text, and assert
password display mode masks one glyph per grapheme while preserving the stored value. The focused
Textarea checks prove newline-preserving controlled payloads in component tests and inner viewport
wheel containment in the Components gallery. The focused
RadioGroup test renders real radio
items, rejects disabled clicks, skips disabled items with arrow navigation, verifies default
selection seeding, click and arrow-selection payloads, and confirms Space on an already selected radio does not emit a duplicate
selection change. The focused Listbox test renders real standalone, separator, and grouped options,
rejects disabled clicks, keeps arrow navigation selection-free, skips disabled/separator rows, and
verifies Enter and Space dispatch both option-level and listbox-level selection callbacks. The
focused Select test opens the real trigger, rejects disabled popup option clicks, verifies click and
keyboard selection payloads, closes after selection, and confirms popup Listbox arrow navigation
skips disabled rows. The focused Combobox tests click the controller-backed text input, type a
query, open the filtered popup by trigger and keyboard paths, verify filtered Listbox options, and
select filtered options with ordered select/open callbacks. The focused Command tests cover
renderer-neutral ranking, controlled and default query ownership, stable-value selection across
descriptor reorder, multi-select selected chips, virtualized result render plans, app-owned index
snapshots, core `CommandDescriptor` projection into Command/Menu/ContextMenu surfaces, inline and
dialog command filtering, keyboard activation, shortcut payloads, non-dialog content persistence,
and dialog Escape/outside press dismissal. Command ownership is split across
`command/descriptor.rs`, `command/model.rs`, `command/style.rs`, `command/render_plan.rs`, and
`command/runtime.rs`, while `command/mod.rs` remains the public builder facade. Menu,
ContextMenu, Tree, and Table behavior snapshots now follow the same source-owner discipline:
`menu/` owns descriptor/model/render-plan/runtime/style plus the facade, `context_menu/` owns the
point-anchor facade and neutral state, `tree/` owns descriptor/model/movement/render-plan/runtime
boundaries, and `table/behavior/` owns counts, columns, header, rows, and tree summary snapshots.
The focused gallery Command smoke renders ranked, multi-select, virtualized, and indexed/loading samples in focused family mode,
verifies selected chips, stable selected values, and snapshot metadata are inspectable, and
confirms wheel input on the virtualized sample does not move the surrounding card.
The Components-page command contracts also cover the `registry-dispatch` sample for
`CommandCenter` shortcut/dispatch projection plus empty shortcut diagnostics, and the
`provider-search` sample for
`CommandProviderSource` refresh into a rendered `CommandIndexSnapshot`, including provider request
id, query metadata, projected shortcuts, and empty shortcut diagnostics. The `context-stack`
sample proves that `CommandContextStack` scopes command descriptors and projects the GPUI keymap
binding active for the focused key context. The command crate provider lifecycle tests cover
center-issued request ids, bound responses, stale async responses being ignored without mutating
registry sources, explicit `CommandSourceHandle`/`CommandProviderHandle` unregister behavior, and
the `CommandProviderRefreshController` query/loading/response/snapshot pipeline. The command crate
also covers `CommandKeyBindingRegistry`, which lets app/plugin sources contribute command-id keyed
shortcut dictionaries, projects valid entries into concrete GPUI `KeyBinding` values, preserves
GPUI chord and key-context predicate semantics, reports missing-action or parse diagnostics without
panicking, reports same-context command shortcut conflicts, and returns an install report when
app shells append projected bindings into a GPUI keymap. Conflict coverage includes global
no-context bindings that overlap concrete context bindings under GPUI runtime precedence rules.
The UI component command tests now also cover `CommandPaletteProjection`, which adapts a
`CommandCenter` query/keymap projection into a `PreFiltered` `CommandIndexSnapshot`, provider
statuses, shortcut diagnostics, and UI-ready status rows for failed providers plus shortcut drift;
the Command runtime navigation layer, which supports Home/End, configurable Up/Down loop
navigation, Vim-style control aliases, PageUp/PageDown, and Alt+Up/Alt+Down group jumps;
`CommandPaletteController`, which coordinates palette query
changes across provider refresh controllers, refreshes registered synchronous providers, exposes
pending provider requests for app-owned async tasks, keeps compatibility missing-provider ids,
ignores stale async responses through the existing provider request guard, and wraps command-center
query-history navigation so up/down history keys can reuse the current query as a prefix and
restore the draft query at the newest boundary; plus
`CommandProviderPaletteProjection`, which adapts a provider refresh projection into a `PreFiltered`
`CommandIndexSnapshot`, carries loading provider status into `CommandLoadingState`, and lets
`Command::provider_refresh_projection` bind query and snapshot metadata without app-owned snapshot
glue. The Command gallery includes a diagnostics/empty sample that renders provider failure,
shortcut/action drift, and an empty list inside the component-owned command surface.
Run the focused proof with:

```powershell
cargo nextest run -p open-gpui-ui-components command
cargo nextest run -p open-gpui-ui-components command::runtime::tests --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_palette_projection_builds_status_items_from_provider_failures_and_diagnostics command_state_accepts_explicit_status_items --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_palette_controller --no-fail-fast
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-command center_reports_command_key_binding_conflicts_and_install_report center_reports_global_key_binding_context_conflicts --no-fail-fast
cargo nextest run -p open-gpui-command center_projects_command_key_bindings_into_gpui_keymap center_reports_command_key_binding_projection_diagnostics --no-fail-fast
cargo nextest run -p open-gpui-command center_exposes_query_history_navigation memory_history_promotes_duplicate_queries memory_history_navigates_recent_queries_with_prefix --no-fail-fast
cargo nextest run -p open-gpui-command context_stack keymap_shortcut_projection_can_respect_context_stack center_context_stack_drives_scopes_keymap_and_provider_requests --no-fail-fast
cargo nextest run -p open-gpui-command source_and_provider_handles_unregister_their_runtime_state --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_descriptors --no-fail-fast
cargo nextest run -p open-gpui-ui-components command menu --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery command
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
```

For the reusable command ecosystem, also keep `docs/ui/command-ecosystem.md` current. It records
the split between GPUI `Action`/`Keymap` execution, the app-owned
`open_gpui_command::CommandCenter` facade, scoped source registration/unregistration, availability
guards, shortcut projection, dynamic provider responses, fuzzy search/history ranking, menu
projection, and command-id dispatch.

The focused Tabs test renders real tabs,
preserves the `default_selected` seed on the first frame, rejects disabled tab clicks, keeps manual
arrow navigation as focus-only, and activates focused tabs with Enter and Space. The focused
Toolbar test renders real toolbar items, moves roving focus with arrow/Home keys, skips disabled and
separator items, and activates the focused item with Enter.

The components package also includes low-state primitive coverage for Separator, Kbd, Progress,
Skeleton, Avatar, AvatarGroup, and AvatarGroupCount. Those tests verify resolved state branches,
explicit root/common/prelude exports, theme color intents, stable rendered debug selectors, decorative
separator semantics, progress clamping, indeterminate progress, Avatar fallback initials,
explicit accessible labels, size metrics, `Role::Image`, group visible/hidden counts, overflow
label state, and source metadata staying outside image-loading ownership. The gallery metadata and
short-viewport smoke tests also verify those primitives are listed as official catalog entries and
render visible samples with stable debug selectors.
The public component gate is federated. `component_contract/rows/catalog.rs` owns only the 48
official ids, revisions, families, and required scenario ids. `public_api/common.rs`,
`public_api/default.rs`, and `table/mod.rs` generate typed export facts from their own `pub use`
declarations. Components/Overlay Gallery catalogs own selectors and Story probes, DevTools owns its
immutable metadata projection, and each native target owns a sibling `*.scenarios.toml` artifact.
No shared test manifest or method/source inventory mirrors those owners.

The focused compile/runtime proof is:

```sh
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo test --locked -p open-gpui-ui-components --doc
```

`official_components_match_typed_public_exports` checks every product id against common/default
exports, keeps typed diagnostic facts disjoint from those surfaces, and anchors
`TableBehaviorSnapshot` to the explicit Table diagnostic owner while preserving
`TableVirtualizerSnapshot` as a default restoration input. The compile witness checks root, common,
prelude, explicit Table diagnostic, and GPUI adapter paths. Same-declaration compile-fail doctests
check every diagnostic export against the actual root, common, and prelude facades; a hand-written
leak makes the corresponding doctest compile unexpectedly and fail. `scan-ui-contract` validates
typed export owner/tier/duplicate facts, docs projection drift, invalid/duplicate scenario bindings,
filter-expression injection, and removed central-authority residue, then runs every registered exact
nextest coordinate.

The v0.3 public API freeze also has a workspace-level tier gate:

```sh
cargo run -p xtask -- scan-public-api --check
```

This gate checks root, prelude, common, default, advanced, model, runtime, adapter, and persistence ownership for docking, motion, UI components, UI core, and canvas. Canvas coverage includes the root common facade plus the explicit `adapter`, `persistence`, and `advanced` tiers. It is a deterministic tier scan today and is the integration point for a future rustdoc-json or cargo-public-api snapshot backend.

Accessibility contract coverage now has its own semantic gate. `ComponentA11yContract` validates
role/name/value/action facts without a live platform backend, while the existing GPUI adapter tests
continue to prove role, orientation, toggled-state, and action mapping into GPUI. Run:

```powershell
cargo nextest run -p open-gpui-ui-components a11y --no-fail-fast
```

That gate keeps the renderer-neutral validation vocabulary covered. Official semantic producers
are additionally asserted in the final GPUI `TreeUpdate` with real Focus, Click, Increment,
Decrement, SetValue, selection, and text dispatch; Table has its own multi-node final-tree target.
Static accessibility evidence rows, Gallery claims, and their consumers are absent. They must not
be recreated in place of final-tree and action tests.
The authority and privacy boundary is recorded in
[Semantic accessibility and final-tree authority](knowledge/engineering/decisions/semantic-accessibility-final-tree-authority.md).

Theme portability is guarded by the theme focused gate:

```powershell
cargo nextest run -p open-gpui-ui-components --test theme --test theme_scope --test public_surface --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-foundation-gallery -E 'test(tokens_gallery_renders_sibling_theme_scopes_and_a_deferred_overlay) | test(devtools_gallery_workbench_refreshes_from_shell_live_facts) | test(devtools_gallery_initial_frame_uses_the_window_effective_theme)' --no-fail-fast --locked
cargo run -p xtask -- scan-theme-drift
cargo run -p xtask -- scan-theme-schema
```

That gate keeps app/window/subtree precedence, exact-window refresh, cached child invalidation,
early-return and panic-safe scope restoration, complete-payload deferred opening-generation
capture, delayed native tooltip capture, Gallery sibling scopes, window-effective initial DevTools
capture, runtime `ThemeContext` rendering, complete code-built `ThemeDefinition` registration, and
the JSON loader facade working: `THEME_JSON_SCHEMA_VERSION`, `theme_json_schema`,
`theme_json_string`, `theme_definition_from_json_str`, `theme_definition_from_json_file`,
`register_theme_json_str`, and `register_theme_json_file`. Production component render paths resolve
color intents from
`ThemeResolver::current(window, cx)` or an explicit snapshot. The app-only resolver and direct
default-light `ThemeResolver::resolve` path are absent. Direct GPUI tooltip attachment uses
`Tooltip::scoped`; official components capture delayed builders automatically. Focus-ring painting
follows the same rule: production render paths use `focus_ring_shadow_with_theme(...)` with an
explicit render-time theme context.
Button and TextInput consume the shared typography, spacing, radius, and density recipes; official
overlays and Tooltip consume elevation; Splitter and VirtualizedList consume strict motion policy.
Tests cover explicit-size precedence, reduced-motion safety, source-versus-effective revision,
metadata-only no-ops, complete schema round-trip, atomic invalid replacement, non-color cached-child
refresh, and opening-generation density/motion capture. The committed schema is generated, while
`scan-theme-drift` enforces two real production recipe consumers for every public design token.
The ownership and detached-render boundary are recorded in
[Theme scope resolution and deferred capture](knowledge/engineering/decisions/theme-scope-resolution.md).
Loader failures are structured as `ThemeLoadError` / `ThemeFileField` for unsupported schema
versions, missing identity or nested design facts, unsupported mode/density/motion/token/state
names, invalid elevation values, duplicate or incomplete token/state coverage, and invalid RGB
values. The old color-only shape and `fallback_mode` are rejection cases, not fallback paths.

Collection typeahead has a separate deterministic runtime gate:

```powershell
cargo nextest run -p open-gpui-ui-components --lib typeahead --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-components --test typeahead_runtime --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-components --test layout --test choice --test overlay --no-fail-fast --locked
cargo nextest run -p open-gpui-ui-foundation-gallery --no-fail-fast --locked
```

The private session owns accepted character normalization, the 700ms executor-clock deadline,
repeated-character cycling, and instance lifecycle. Tree, VirtualizedList, Menu, ContextMenu, and
standalone Listbox consume it; Select consumes it only through an open popup Listbox. Tests cover
the exact timeout boundary, no-redraw cycling, IME/modifier propagation, disabled and structural
rows, reveal without selection, reorder/remove by stable key, remount, same-window instances,
equal IDs in different windows, close/reopen generations, and closed Select triggers. Combobox and
Command runtime tests remain the negative gate for editor-owned query input.

The foundation component family gate covers the shipped disclosure, numeric, navigation, display,
action, and feedback additions: Accordion, Collapsible, Slider, NumberInput, ToggleGroup, Link,
Breadcrumb, Tag, and ToastStack. These tests keep one canonical API per family, explicit
root/common/prelude exports, ownership vocabulary, resolved-state purity, official catalog metadata, and
focused Components-page rendering aligned:

```powershell
cargo nextest run -p open-gpui-ui-components --test public_surface --test form --test navigation --test primitives --test theme --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery --test foundation_gallery -E 'test(gallery_contract_metadata_matches_component_rows) | test(gallery_story_contracts_derive_selectors_and_runtime_probes_from_gallery_owners) | test(components_gallery_smoke_focuses_every_focusable_catalog_entry) | test(components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation)'
```

The choice and runtime seams are guarded separately: `choice.rs` owns stable-value matching for
choice models; `collection_typeahead.rs` privately owns collection input timing and acceptance;
`roving_focus.rs` owns shared enabled-item navigation targets; and `menu/runtime.rs` owns
instance-local menu typeahead alongside submenu hover timing and local scroll state. Editable
Command and Combobox queries remain owned by their text-input controllers.
Feedback coverage now promotes `StatusCue` and `EmptyState` as official rendered Components
catalog entries. The focused component tests verify root/common/prelude exports, feedback intent labels,
resolved roles, metrics, and theme color intents. The gallery metadata tests require their
component/state `SIGNALS` entries and stable `gallery:component-status-cue-sample:{id}` /
`gallery:component-empty-state-sample:{id}` selectors, while the short-viewport smoke verifies the
real `status-cue:*:root` and `empty-state:*:root` debug selectors render.
`official_components_match_typed_public_exports` is the public-owner gate for canonical component
rows and their root/common/prelude exports. `gallery_contract_metadata_matches_component_rows` is
the Gallery metadata gate: official Components and Overlay rows must project the exact canonical
id, revision, and family, while Gallery-local adapter/anatomy/state rows carry no product metadata.
`gallery_story_contracts_derive_selectors_and_runtime_probes_from_gallery_owners` is the
story-probe contract gate. It requires official component samples, renderer-neutral state readouts,
and overlay samples to expose a reusable `StoryContract` with truthful public selectors and
user-observable probe operations:
open, dismiss, select, edit, scroll, focus, activate, and read-public-payload. Gallery smokes should
prefer `component_story_contract_for(name)` and `component_story_contracts_for_focus(mode)` before
falling back to raw debug selectors for adapter-internal details. The official sample selector
pairs and state readout selector pairs are derived from those story contracts so focused catalog
traversal and selector metadata stay aligned.
`gallery_story_contracts_derive_selectors_and_runtime_probes_from_gallery_owners` also guards the
pre-renderer state-contract boundary. Gallery-local `state-contract` entries declare a
`state_contract_selector`, do not declare an official `sample_selector`, and stay disjoint from
`official_sample_selector_pairs`.
The current state contracts are `TreeState` and `VirtualizedListState`; their signals cover state,
descriptor, action/result, helper, and payload types. `TreeState` remains a reusable hierarchy
contract even though `Tree` is now an official rendered component, matching the
`VirtualizedListState` / `VirtualizedList` split. The Components page smoke also verifies every
`state_contract_readout_pairs()` selector is visible.
The official Table gate requires `Table`, `TableState`, `VirtualizerState`,
`TableFacetedFilter`, `TableRangeFilter`, `TablePredicateFilter`, `TableColumnVisibility`, role
signals for table rows and cells, and at least one `gallery:component-table-sample:{id}` selector.
Table smokes and state tests assert that rendered row selectors stay bounded by the virtualizer's
visible rows plus overscan, scroll input stays inside the table viewport, sortable header actions
emit state-update payloads, controlled column resize callbacks carry stable sizing payloads,
categorical faceted filters emit controlled exact-token updates, numeric range filters emit
controlled finite-bound updates, predicate filters emit controlled operator/value updates, column
visibility emits controlled hide/show payloads, editable text cells emit controlled exact
`(TableRowIdentity, TableColumnId)` change payloads without triggering row interaction callbacks;
row activation and expansion request payloads stay controlled; source-tree row models keep nested
descendants addressable through exact `TableRowIdentity` lookup; manual source-tree snapshots expose
unloaded/loading/failed child metadata, row-pinning regions split top/center/bottom rows with
keep-pinned and page-only policies, and grouped / expanded row models keep collapsed descendants
addressable through `TableRowModel::row(&TableRowIdentity)`. `TableRowModel::source_rows` and
`unique_source_row` provide explicitly non-exact business-`TableRowId` lookup. Exact source
identities distinguish unique, explicit-instance, and occurrence rows; occurrence identity is
source-snapshot-local, while a caller-owned instance identity survives source replacement and
reorder. Ambiguous or stale edits are inspectable no-ops; controlled
column order changes normalize the complete source order before moving a listed or unlisted column
without taking ownership of visibility or pinning. Virtual focus tests prove the Table-root proxy,
real keyboard continuation, no-steal remounting, and first-row or empty-model fallback. The
Components gallery now carries `release-rollup`, a grouped Table sample that mixes expanded and
collapsed team groups,
exposes aggregate count and score cells, pins the identifier and status columns, and has its own
sticky-header plus inner-scroll smoke. It also carries
`server-paged`, a manual filtering/sorting/pagination sample that renders only the current
app-supplied page snapshot while exposing server-known total row and page counts through the
gallery summary and `TableBehaviorSnapshot`. It also carries `release-resize`, a controlled
column-sizing sample whose resize smoke drags the `name` handle, records the app-owned committed
width, and verifies header and first-row cell widths stay aligned. `filter-board` is also the
faceted-filter proof: it renders a `status` `TableFacetedFilter`, records
`TableFacetedFilterChange` payloads in the sample runtime log, proves selecting `Done` changes the
rendered row window, proves clearing restores it, and confirms popup wheel input does not move the
outer table sample. It also renders a score `TableRangeFilter`, records
`TableRangeFilterChange` payloads in the same runtime log, applies the range to a sample-owned
`TableState` override, proves filtered/final row counts match the core contract, and confirms
popup wheel input stays local. It also renders a name `TablePredicateFilter`, records
`TablePredicateFilterChange` payloads in the same runtime log, applies the operator/value
predicate to a sample-owned `TableState` override, and proves the rendered row window follows the
core filtered row model. `release-matrix` also renders a `TableColumnVisibility` toolbar
control, records `TableColumnVisibilityChange` payloads in the sample runtime log, applies
visibility overrides to the sample-owned `TableState`, proves hiding a metric column removes its
header and cells, proves show-all restores the column, and confirms popup wheel input stays local.
`release-rollup` now also proves controlled column-order changes: the sample runtime log records
`TableColumnOrderChange` payloads, applies the app-owned override to the sample `TableState`, and
shows the score column re-rendering before team while the sample card stays anchored. The focused
component partial-order gate separately starts from an incomplete order, moves an initially
unlisted column under visibility and pinning, and asserts the resulting full source-column order.
`components_gallery_smoke_grouped_table_scroll_stays_inside_sample` is the focused vertical
sticky-header proof: it enters the Table family view, wheels the `release-rollup` body, and
asserts the header band stays fixed while the body row window advances.
`editable-release` is the text-cell editing proof: it renders editable `name` and `team` columns,
keeps `status` read-only, records each `TableCellEditChange` exact logical-row identity, column id,
and apply outcome in the sample runtime log, applies the updated sample-owned `TableState` override,
and proves the changed row text re-renders through the normal Table pipeline.
`release-matrix` is the wide center-column virtualization and column-visibility sample: it pins the
identity and status lanes, exposes fourteen center metrics, locks identity/status visibility, and
has focused smokes that prove off-window center columns unmount/remount, hide/show visibility
changes update rendered headers/cells, and horizontal / popup wheel input remains inside the
sample. `row-pinning` is the row-region sample: it pins top and bottom review rows around a paged center body, exposes
top/center/bottom readouts, and proves center-body wheel input changes the center row window
without moving the fixed row bands or outer sample. The Table adapter keeps the row and column
virtualizers separate internally; public tests assert the resulting two-axis behavior through
`TableBehaviorSnapshot` plus gallery runtime probes. `dependency-tree`
is the source-hierarchy
sample: it proves nested `TableRow` children resolve to visible tree rows,
keeps collapsed descendants addressable by stable id, exposes tree-depth and tree-branch summary
metadata, and drives controlled expansion plus row activation through the gallery runtime log.
`server-tree` is the manual-expansion sample: it preserves the app-supplied source snapshot,
renders unloaded, loading, and failed branch affordances, records loaded-child and load-state
metadata in expansion payloads, and proves that child rows appear only after the gallery runtime
supplies the loaded snapshot.
Core table tests also assert that `TableAggregation` exposes stable built-in aggregate labels,
resolves count, sum, min, max, and average cells for grouped rows without hiding the grouping
column value, and lets `TableState::with_aggregation_fn` resolve named custom aggregate callbacks
with safe empty fallback for unknown names. Core and component tests assert that
`TableColumnPinning` splits visible columns into
left, center, and right regions after visibility/order resolution, ignores unknown or invisible
pinned ids, removes moved columns from their previous pinned side, and exposes matching
header/body region metadata and debug selectors. They also assert `TableRowPinning` deduplicates
each caller-owned target list, preserves target order, resolves exact source/group identities,
expands explicit business-id bulk targets in model order, gives top logical identities precedence
over bottom overlaps, ignores unknown/filtered/collapsed rows, preserves pinned rows outside the
current page by default, supports page-only behavior, feeds only center rows into the vertical
virtualizer, and renders fixed row-pinned bands around the scrollable center body. Final AccessKit
tests additionally prove row, cell, and header `NodeId` stability through column and row pinning,
duplicate source-instance separation, old-node action dispatch, and identity restoration after a
virtual row or column leaves and re-enters the window.
The official Tree gate requires `Tree`, `TreeState`, `TreeMetrics`, tree/tree-item role signals,
and at least one `gallery:component-tree-sample:{id}` selector. Component runtime tests verify
expansion, reveal, and selection payloads; gallery smokes verify keyboard expansion/selection
through the sample runtime log and prove Tree wheel input stays inside the sample viewport.
`TreeChildrenLoadState` adds the lazy-branch gate: unit tests prove expanded unloaded/loading/failed
branches do not synthesize fake child rows, toggle payloads carry loaded-child and load-state
metadata, and loading branches do not repeat toggle requests. The `remote-workspace` gallery sample
proves unloaded, loading, loaded, and failed branch affordances plus runtime payload metadata.
Tree typeahead is covered by a pure state test and a runtime adapter test: the pure helper matches
visible, focusable row labels with wraparound and skips disabled/collapsed rows, while the rendered
adapter reads the shared private session and moves focus without selecting. The `document-outline`
gallery smoke now verifies typing `n o` focuses the visible Notes row after the expand/select path.
Tree and virtualized-list state-contract samples are verified through
`components_page_samples_expose_component_metadata`: Tree readouts assert visible flattening,
disabled-row position skipping, navigation skipping, toggle payloads, and Enter/Space selection
actions; virtualized-list state-contract readouts assert active/selected indices, PageUp/PageDown
clamping, activation payloads, viewport item count, overscan, and semantic scroll strategy labels.
The same metadata test now also checks the official `Tree` sample's role metadata and keyboard
toggle payload, the official `remote-workspace` Tree sample's child-load metadata, plus the
official `VirtualizedList` sample's 10k item count, listbox roles, active/selected state, visible
range, and overscan summary.

The focused Tree proof is:

```powershell
cargo nextest run -p open-gpui-ui-components tree_state_resolves_lazy_branch_load_metadata_without_synthetic_children tree_toggle_payload_includes_child_load_state_and_blocks_loading feedback_tree_and_virtualized_list_public_exports_remain_explicit
cargo nextest run -p open-gpui-ui-components tree_typeahead_targets_visible_focusable_items_from_current_focus tree_runtime_typeahead_focuses_visible_matching_row
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata components_gallery_smoke_tree_expands_and_selects components_gallery_smoke_tree_lazy_branches_emit_load_metadata
```

The gallery package also includes a compact-shell runtime smoke that switches the gallery to the
compact viewport policy, verifies the derived mobile shell and compact density, scrolls the left
navigation rail to deep pages, and confirms switching away and back resets the page scroll position.

The gallery package also includes Overlay-page runtime smoke coverage for popover, modal dialog,
alert dialog, non-modal sheet, menu, and ContextMenu right-click hotspot opening plus Escape
dismissal. Popover and Dialog smokes open the real component trigger, assert Dialog initial focus,
and assert focus restoration to the trigger after outside press, modal barrier dismissal, and
Escape dismissal. The AlertDialog smoke opens the real trigger, confirms the cancel action gets the
default focus, verifies the primary action closes the dialog, and confirms Escape dismissal
restores focus to the trigger. The Overlay gallery intentionally keeps default-open contract
samples visually closed at page load so modal barriers and floating layers do not block page
scrolling; the metadata rows still report each sample's resolved default-open contract.

The U3/U4 window overlay fleet has a joint completion gate. It covers renderer-neutral focus and
overlay policy, GPUI pointer-session and focus-claim lifecycle, the window focus-scope runtime,
every official overlay adapter, Gallery runtime snapshots, and the redacted DevTools projection:

```powershell
cargo nextest run --locked -p open-gpui-ui-core -E 'test(/^(focus|overlay)::tests::/)' --no-fail-fast
cargo nextest run --locked -p open-gpui -E 'test(/app::test_context::pointer_session_tests::/)' --no-fail-fast
cargo nextest run --locked -p open-gpui dropping_the_focused_handle_does_not_advance_the_explicit_claim_revision --no-fail-fast
cargo check --locked -p open-gpui-ui-components --tests
cargo nextest run --locked -p open-gpui-ui-components -E 'test(/overlay::focus_scope::tests::/)' --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-components --test window_overlay_runtime --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-components --test overlay --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-components --test choice --no-fail-fast
cargo check --locked -p open-gpui-ui-foundation-gallery --tests
$overlayGalleryTests = @(
    'overlay_gallery_smoke_renders_catalog_entries_and_official_samples'
    'overlay_gallery_smoke_dismisses_popover_from_outside_press'
    'overlay_gallery_smoke_opens_tooltip_from_hover_focus_and_ignores_disabled'
    'overlay_gallery_smoke_renders_manual_tooltip_from_state'
    'overlay_gallery_smoke_keeps_hover_card_open_on_outside_press_and_dismisses_on_escape'
    'overlay_gallery_smoke_toggles_hover_card_from_control_surface'
    'overlay_gallery_smoke_closes_dialog_from_modal_barrier_and_escape'
    'overlay_gallery_smoke_controlled_dialog_refusal_keeps_modal_authority'
    'overlay_gallery_smoke_closes_alert_dialog_from_action_and_escape'
    'overlay_gallery_smoke_closes_non_modal_sheet_from_outside_press'
    'overlay_gallery_smoke_closes_menu_from_escape_and_outside_press'
    'overlay_gallery_smoke_opens_menu_submenu_from_hover'
    'overlay_gallery_smoke_nested_popover_menu_dialog_restores_focus_lifo'
    'overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses'
)
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery @overlayGalleryTests --no-fail-fast
cargo check --locked -p open-gpui-devtools --features ui-components
cargo nextest run --locked -p open-gpui-devtools --features ui-components --test framework_adapters framework_adapters_project_window_overlay_runtime_without_raw_layer_ids --no-fail-fast
git diff --check
```

The `open-gpui` lifecycle gates prove stable pointer capture across redraws, routed event-target
versus physical-hover semantics, explicit and automatic release, cancellation on deactivation or
window removal, cross-window isolation, and focus-claim stability when a focused handle is dropped.
The `open-gpui-ui-core` gate owns the canonical focus resolver and overlay policy states, while
the `open-gpui-ui-components` `focus_scope_tests.rs` module (registered as
`overlay::focus_scope::tests`) proves that the GPUI runtime applies those policies to live handles.

`window_overlay_runtime` proves the four runtime phases, parent topology, topmost input arbitration,
modal barriers, controlled refusal, callback reentrancy, exit/reopen, owner release/remount, focus
loops and restoration, and window isolation. The `overlay` and `choice` targets prove the same
authority through all official families, including Popover -> Menu -> controlled Dialog LIFO Escape
and focus restoration, choice/search editor preservation, and passive Tooltip/HoverCard layers.
Gallery must obtain `WindowOverlayRuntime::snapshot()` from the window that rendered the actual
component adapters; a hand-built layer registration is not Gallery evidence. Its nested smoke
checks kind, phase, parent topology, and each restore target, while the refusal smoke keeps Dialog
in `CloseRequested` with modal, keyboard, and focus authority intact.

DevTools consumes `WindowOverlaySnapshot` through the allowlist entry point
`open_gpui_devtools::ui_components::window_overlay_probe_snapshot`. Layer and parent relationships
are snapshot-local opaque ordinals. The projection includes only layer count, kind, phase, presence,
pending open/reason, keyboard eligibility, modal pointer barrier, and focus active/entered facts;
window identity, lifecycle generations, and intent revisions are omitted. Raw `OverlayLayerId`
strings must never enter a DevTools node, property, JSON payload, or serialized capture. The adapter
test uses unique parent and child canary IDs and must fail if either appears anywhere in the
projection or serialization.

The fleet gate requires a visible runtime registration path for Dialog, Sheet, AlertDialog,
Popover, Menu, ContextMenu, Select, Combobox, Command overlay mode, HoverCard, and Tooltip. It also
requires an absence scan for `OverlayLayerHost`, the removed request forwarding helpers, and
component-owned Escape/outside/focus tails. Component state tests alone do not demonstrate window
runtime ownership.

The focused Overlay catalog gates are:

```powershell
cargo nextest run -p open-gpui-ui-foundation-gallery overlay_page_catalog_entries_have_signals_and_sample_selectors
cargo nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke_renders_catalog_entries_and_official_samples
```

The `open-gpui-ui-core` overlay tests are the renderer-neutral policy gate for layer kind, presence,
outside-press policy, Escape policy, focus restore intent, initial focus intent, and
`resolve_overlay_placement` side/alignment/fit/trace behavior. The per-window production runtime
consumes those resolvers; the window-free tests are not themselves proof of live registration,
input interception, or focus ownership.
The `open-gpui-ui-components` overlay helper tests cover GPUI mapping for deferred priority, snap
margin, anchor conversion, and placement resolution. Escape and outside-press decisions are
verified through the `ui_core` resolvers and the real per-window runtime rather than request
forwarding helpers.
The U3/U4 `window_overlay_runtime`, `overlay`, and `choice` integration targets above are the
production-authority proof for migrated families. Trigger-anchored components that do not provide measured
trigger/content bounds should not be documented as owning safe-bounds flip/shift at render time
until live placement measurement is wired into that placement path.
For GPUI runtime focus assertions, `VisualTestContext::debug_selector_is_focused` and
`VisualTestContext::focused_debug_selector` are the preferred test hooks. They use test-only
debug-selector-to-focus-handle data and keep focus checks independent from component internals.
The federated component contract keeps adapter-only, renderer-neutral state, primitive, Gallery,
and docs ownership explicit without copying those facts into one row. Table, Tree,
VirtualizedList, and Command expose behavior snapshots or state readouts; renderer assembly plans
stay crate-private unless a future component deliberately promotes a narrower state contract.
For the UI architecture deepening refactor, keep the focused gates below close to the code that
changes them. They cover the component contract rows, public export map, removed primitive
aliases, overlay runtime policy, choice/search behavior, the Command ownership split, the Table
behavior-snapshot and internal render-plan boundary, shared row-window projection, theme registry,
and gallery catalog/conformance/runtime/sample/render module split:

For the non-overlay choice/search, default-surface, motion, and gallery story-contract refactor,
run this focused subset before broader workspace gates:

```sh
cargo fmt --all -- --check
cargo check -p open-gpui-ui-components --tests
cargo nextest run -p open-gpui-ui-components --test choice --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-motion --no-fail-fast
cargo test -p open-gpui-motion --doc
cargo check -p open-gpui-ui-core --tests
cargo nextest run -p open-gpui-ui-core split --no-fail-fast
cargo nextest run -p open-gpui-ui-core --test headless_contracts --no-fail-fast
cargo check -p open-gpui-ui-foundation-gallery --tests
cargo nextest run -p open-gpui-ui-foundation-gallery component --no-fail-fast
cargo run -p xtask -- scan-ui-contract
git diff --check
```

These commands prove that Select and Combobox keep behavior after their
model/style/render-plan/runtime splits, `choice.rs` remains an internal behavior seam,
`open_gpui_ui_components` no longer exposes broad command/core infrastructure through the curated
default surface, `MotionValue` stays private behind consumed motion controller APIs, and
Listbox/Select/Combobox/Command gallery stories expose state readout selectors for public-payload
assertions.

The window-runtime and choice focus tests should keep the `scroll_surface`, `choice`, `overlay`,
`layout`, and `table` focused gates green. The `choice` gate proves Select, Combobox, and dialog
Command register with the per-window authority and preserve focus plus open-change callback order
after selection, Escape dismissal, outside-press dismissal, and controlled refusal.

For the deep UI framework module refactor, run the focused ownership gates below before the full
workspace gate. They cover runtime theme context, ephemeral semantic projection with final-tree and
action evidence, removed registry history, shared overlay placement,
`open_gpui_ui_core::grid_viewport::RowWindow`, gallery story-contract projection, and
`open_gpui_command::CommandDescriptor` projection:

```powershell
cargo fmt --all
cargo check -p open-gpui-ui-core --tests
cargo check -p open-gpui-ui-components --tests
cargo check -p open-gpui-ui-foundation-gallery --tests
cargo nextest run -p open-gpui-ui-core overlay grid_viewport --no-fail-fast
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components --lib scroll_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test choice --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test overlay --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test layout --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test table --no-fail-fast
cargo nextest run -p open-gpui-ui-components theme_scope theme a11y menu context_menu command --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_descriptors --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery component --no-fail-fast
cargo run -p xtask -- scan-theme-drift
cargo run -p xtask -- scan-theme-schema
cargo run -p xtask -- scan-ui-contract
rg -n "ThemeResolver::resolve\(" crates/ui_components/src -g "*.rs"; if ($LASTEXITCODE -eq 0) { exit 1 } elseif ($LASTEXITCODE -ne 1) { exit $LASTEXITCODE } else { exit 0 }
rg -n "ThemeRuntime|ThemeResolver::current\(cx\)" crates/ui_components/src examples/ui-foundation-gallery/src -g "*.rs"; if ($LASTEXITCODE -eq 0) { exit 1 } elseif ($LASTEXITCODE -ne 1) { exit $LASTEXITCODE } else { exit 0 }
git diff --check
```

For public export, Gallery metadata, native scenario, or Table diagnostic owner changes, run:

```powershell
cargo fmt --all -- --check
cargo check --locked -p open-gpui-ui-components -p xtask --tests
cargo nextest run --locked -p xtask --lib
cargo nextest run --locked -p open-gpui-ui-components --test public_surface --test table --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery component_catalog_contracts::gallery_contract_metadata_matches_component_rows --no-fail-fast
cargo run --locked -p xtask -- scan-ui-contract
```

For component contract, a11y, gallery conformance, and theme productization work, start from the
reusable UI contract audit before dropping to focused behavior tests:

```powershell
cargo run --locked -p xtask -- scan-ui-contract
cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
cargo check -p open-gpui-ui-components --tests
cargo check -p open-gpui-ui-foundation-gallery --tests
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-components a11y --no-fail-fast
cargo nextest run -p open-gpui-ui-components theme --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery component_catalog_contracts::gallery_contract_metadata_matches_component_rows --no-fail-fast
```

`scan-ui-contract` checks narrow component rows, same-declaration public export facts, the exact docs
projection, Gallery canonical metadata, test-owned scenario artifacts, removed central-authority
residue, and retired subtree-presentation entry points across GPUI, macros, components, and Gallery
production source. It executes every registered exact test coordinate, including the final-tree and
real AccessKit action scenarios that verify semantic assembly instead of inferring it from static
claim rows.
Use the narrower
`scan-theme-schema`, `scan-theme-drift`, and focused nextest commands when investigating a specific
failure.

## Interactive Subtree Transform Gate

Changes to `SubtreeTransform`, `ElementGeometry`, targeted input, frame-journal geometry, scene
primitives, renderer conversion, or Motion projection must run the focused U12 gate before broad
workspace verification:

```powershell
$env:CARGO_BUILD_JOBS = '1'
cargo fmt --all -- --check
cargo nextest run --locked -p open-gpui --lib transform --no-fail-fast
cargo nextest run --locked -p open-gpui --lib measured tooltip --no-fail-fast
cargo nextest run --locked -p open-gpui --all-features --test presentation_surface --no-fail-fast
cargo nextest run --locked -p open-gpui-motion --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-components motion_adapter --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery presentation --no-fail-fast
cargo check --locked -p open-gpui-wgpu -p open-gpui-windows --tests
git diff --check
```

The GPUI tests cover checked numeric construction, nested composition, every scene primitive,
transactional late failure, hit testing, explicit local/window event coordinates, pixel and line
wheel semantics, pointer capture rebinding, drag/drop, IME, final AccessKit bounds, deferred and
portal behavior, cache replay, tooltip lifecycle, committed measurement, and displayed debug
geometry. `presentation_surface` also rejects the removed `Transformation`,
`TransformationMatrix`, and `with_transformation` names and guards the opaque renderer ABI.

The Gallery `presentation` smoke uses nested non-uniform transforms with a real Button, delayed
Tooltip owner, TextInput, ScrollArea, drag/drop target, accessibility group, Motion projection, and
committed geometry readout. Static labels or a quad-only sample are not sufficient evidence.

Native CI remains responsible for each renderer on its owning platform. WGPU, DirectX, and Metal
must compile the shared primitive and run their ABI/conversion tests; capable runners additionally
run the designated transformed-pixel and clip smoke. A Windows local run cannot replace Metal or
Linux/native backend evidence.

## Layout-Preserving Subtree Presentation Gate

Changes to `SubtreePresentation`, low-level `Window` registration gates, focus/IME reconciliation,
pointer cancellation, accessibility membership, deferred/cache journals, overlay presentation, or
official component presentation behavior must run the focused U13 gate:

```powershell
$env:CARGO_BUILD_JOBS = '1'
cargo fmt --all -- --check
cargo nextest run --locked -p open-gpui --lib presentation --no-fail-fast
cargo nextest run --locked -p open-gpui --lib pointer_session accessibility focus text_input tooltip scroll deferred --no-fail-fast
cargo nextest run --locked -p open-gpui --test presentation_surface --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-components --lib presentation --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-components --test semantic_activation --test overlay presentation --no-fail-fast
cargo nextest run --locked -p open-gpui-docking dock_host_presentation --no-fail-fast
cargo nextest run --locked -p open-gpui-devtools --features ui-components framework_adapters --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery presentation --no-fail-fast
cargo run --locked -p xtask -- scan-ui-contract
cargo run --locked -p xtask -- scan-public-api --check
cargo run --locked -p xtask -- scan-doc-links
cargo run --locked -p xtask -- verify-release-docs
git diff --check
```

The GPUI runtime tests assert the exact `Visible`/`Inert`/`Hidden` channel matrix, ancestor
dominance, custom-element fail-closed behavior, same-frame focus/capture/IME and final AccessKit
cleanup, one terminal pointer cancellation, and no stale replay on restoration. Deferred, cache,
portal, and nested-transform tests must toggle only the ancestor presentation where possible so a
child notification cannot hide a stale-journal defect. Every desktop CI host runs the
all-features `presentation_surface` target. Its compiler-checked negative fixtures reject the
retired `Visibility` export, style fields, and fluent methods through the final public export graph;
they do not infer Rust visibility or re-export semantics from source text.

The Gallery Presentation flow keeps equivalent content and layout metrics while switching all
three states at identity and transformed geometry. It exercises real controls, scrolling,
drag/drop, tooltip/deferred content, overlay intent, AccessKit, and Inspector paths. Static enum
labels or an outer fixed-size wrapper alone are not layout evidence. `presentation_surface` and
`scan-ui-contract` reject `Style::visibility` and its generated
Serde/schema `StyleRefinement::visibility` field, `Visibility::{Visible, Hidden}`,
`visibility_style_methods!` and its generated `.visible()` / `.invisible()` (`fn visible` /
`fn invisible`) methods, `a11y_hidden`, `aria_hidden`, and compatibility aliases for the retired
surface. `scan-ui-contract` retains only exact internal residue tokens and does not duplicate the
compiler's public-surface authority. Review must separately confirm that no second ancestor
presentation authority exists outside `SubtreePresentation`.

## Committed Live Region And Announcement Gate

Changes to `SemanticDescriptor` live facts, AccessKit live mapping, `StatusCue`/Toast/Field/
Command/Tree/VirtualizedList status consumers, or `Window::announce` must run the U14 gate:

```powershell
$env:CARGO_BUILD_JOBS = '1'
cargo fmt --all -- --check
cargo check --locked -p open-gpui -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-devtools -p open-gpui-ui-foundation-gallery --all-features --tests
cargo nextest run --locked -p open-gpui --features test-support,inspector --lib --no-fail-fast -E 'test(announcement) | test(live_region)'
cargo nextest run --locked -p open-gpui-ui-components --test a11y --no-fail-fast
cargo nextest run --locked -p open-gpui-devtools --test resolved_semantic_redaction --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery --no-fail-fast -E 'test(focus_a11y)'
cargo run --locked -p xtask -- scan-ui-contract
cargo run --locked -p xtask -- scan-public-api --check
cargo run --locked -p xtask -- scan-doc-links
cargo run --locked -p xtask -- verify-release-docs
git diff --check
```

The GPUI tests must observe accepted announcements in a final `TreeUpdate`, one-generation
retention and later removal, repeated equal text with distinct identities, activation-generation
replacement, inactive/close drops, two-window isolation, and unchanged focus. Declarative tests
must cover role defaults, explicit `Live::Off`, atomicity, busy transitions, rollback, deferred/cache
replay, and `Visible`/`Inert`/`Hidden` membership. Component tests must prove rendering or remounting
does not submit the transient queue. The Gallery drives the real status, busy, alert, same-text, and
inactive controls. A unique runtime canary must occur in the test platform's final tree and be absent
from typed diagnostics, DevTools capture/history/diff/export/inspector/report/artifact/fixture output.
The platform promise is committed-tree publication; native adapters own whether and when a screen
reader delivers the event.

## Window-Owned Bring Into View Gate

Changes to reveal targets, scroll-container capture, focus or AccessKit reveal entry, direct-scroll
cancellation, virtual collection materialization, or component reveal adapters must run the U16
gate:

```powershell
$env:CARGO_BUILD_JOBS = '1'
cargo fmt --all -- --check
cargo nextest run --locked -p open-gpui bring_into_view --no-fail-fast
cargo nextest run --locked -p open-gpui --test bring_into_view_surface --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-components --test layout --test choice --test a11y --test public_surface --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-components --no-fail-fast
cargo nextest run --locked -p open-gpui-ui-foundation-gallery --test foundation_gallery presentation_bring_into_view_unifies_all_entry_paths_and_virtual_materialization --no-fail-fast
cargo run --locked -p xtask -- scan-ui-contract
cargo run --locked -p xtask -- scan-public-api --check
cargo run --locked -p xtask -- scan-doc-links
cargo run --locked -p xtask -- verify-release-docs
git diff --check
```

Core evidence must cover physical horizontal and vertical policies, margins, nested committed
inner-to-outer traversal, saturated-container continuation, disjoint and overlapping chains,
non-uniform transforms, device-pixel convergence, portals, cached/deferred bindings, suppression,
unmount, close, user override, Motion and reduced motion, winning-focus arbitration, and real
AccessKit actions. `bring_into_view_surface` keeps target, sequence, chain, and geometry identity
opaque and rejects generic node or premature logical-axis APIs.

Virtual collection evidence must prove stable-key materialization precedes physical reveal across
fixed and measured rows, unavailable data, filtering, reorder, stale completion, and ABA
replacement. The Gallery drives application, keyboard-focus, AccessKit, animation, wheel
cancellation, and virtual materialization through one nested two-axis transformed scenario. A
component-calculated final offset, index-based GPUI target, implicit portal ancestry, or focus-owned
scrolling runtime fails this gate even if a single viewport appears to move correctly.
The gate also covers direct-scroll interruption before next-frame physical submission and a
same-frame geometry replacement plus user interruption after submission: neither may reissue a
reveal or move a losing virtual Tree focus target.
Virtual Tree materialization must observe the focus revision after all ordinary prepaint commits;
the terminal focus-stable prepaint phase rejects focus and blur mutations so a callback cannot
invalidate the claim after the Tree has moved its virtual viewport. Fenced focus handoffs settle
their logical focus normally but must not perform an implicit reveal after input has interrupted
their `ScrollChainFence`.

Run the full component and gallery package gates only after broad contract-table, theme, or gallery
changes:

```powershell
cargo nextest run -p open-gpui-ui-components --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery --no-fail-fast
```

The Components gallery root keeps `runtime`, `samples`, and render ownership private. Stable
Gallery API names are re-exported explicitly from `components.rs`; sample families live under
`examples/ui-foundation-gallery/src/pages/components/samples/`, runtime probes under
`examples/ui-foundation-gallery/src/pages/components/runtime/`, and render orchestration/readouts
under `examples/ui-foundation-gallery/src/pages/components/render/`. Runtime catalog/Story tests
verify canonical metadata, selectors, probes, and focused-section behavior without parsing Rust
source text.

```powershell
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-core overlay
cargo nextest run -p open-gpui-ui-components --test overlay --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test choice --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test table --no-fail-fast
cargo nextest run -p open-gpui-ui-core virtualizer
cargo nextest run -p open-gpui-ui-components --test layout --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test theme --no-fail-fast
cargo run -p xtask -- scan-theme-drift
cargo nextest run -p open-gpui-ui-foundation-gallery --no-fail-fast
```

The binary-level gates above include these focused sentinels:
`official_components_match_typed_public_exports`,
`common_extended_diagnostic_and_adapter_paths_compile`,
`scenario_declarations_reject_duplicate_contracts_and_filter_expressions`,
`scenario_validator_reports_missing_duplicate_unknown_and_owner_drift`,
`scenario_validator_rejects_reused_executable_coordinates`,
`overlay_open_change_helpers_match_core_policies`,
`dialog_runtime_respects_escape_policy_and_restores_trigger_focus`,
`choice_surfaces_share_stable_value_resolution_and_query_normalization`,
`row_model_pipeline_executes_each_stage_before_final_pinning_partition`, `row_window`,
`virtualized_list_behavior_snapshot_uses_item_descriptors_and_virtualizer_contracts`,
`virtualized_list_behavior_snapshot_applies_builder_metrics`,
`table_behavior_snapshot_exposes_center_column_summary_without_window_internals`,
`table_behavior_snapshot_exposes_pinned_column_regions`, `theme_registry`, `theme_resolver`,
`theme_snapshots`, `gallery_contract_metadata_matches_component_rows`,
`gallery_story_contracts_derive_selectors_and_runtime_probes_from_gallery_owners`,
`choice_search_story_contracts_expose_state_readouts_and_product_metadata`,
`components_gallery_smoke_focused_choice_search_state_readouts_render`,
`components_gallery_smoke_focuses_catalog_family_and_restores_all_mode`, and
`components_gallery_smoke_focuses_every_focusable_catalog_entry`.

The theme drift scan is the focused gate for component color recipes and built-in theme token
coverage. It requires all `ThemeResolver::*_colors` component calls to be implemented and listed
in `crates/ui_components/src/theme/recipes.rs`, rejects component-local `impl ThemeResolver`
extensions, and checks that light, dark, and high-contrast palettes expose the same token/state
shape. Add or move recipes in the theme module first, then update the catalog entry in the same
patch.

The `open-gpui-ui-components` public contract tests should also keep
`public_resolved_state_contracts_avoid_gpui_runtime_types` passing. That test is the hard
headless-readiness guard for public resolved-state structs: it prevents `Window`, `App`,
`Context`, `RenderOnce`, `IntoElement`, `ElementId`, `Entity`, focus handles, scroll handles, and
callback storage from entering state contracts. The companion extraction-blocker inventory tests in
`open-gpui-ui-components` and `open-gpui-ui-core` pin the extraction gate deliberately. Component
public-state blockers are currently empty: resolved overlay contracts expose `OverlayResolvedState`, while
`GpuiOverlayState` stays in the GPUI adapter helper surface for deferred priority and snap margin.
Public component metrics and accessibility state now use neutral UI-core vocabulary; adding public
GPUI `Pixels`, `Bounds`, `Point`, or `Size` aliases to resolved-state contracts should fail the
guard inventory. `open-gpui-ui-core` is now renderer-neutral: it has no `open_gpui` dependency,
no UI-core source references to `open_gpui`, and no `UiPx` conversion impls for GPUI style types.
Adaptive policies accept neutral `UiPx` thresholds and inputs instead of GPUI `Pixels`; GPUI
callers should convert their concrete window or viewport width at the adapter boundary before
invoking UI-core adaptive helpers. The companion strict-boundary inventory must stay empty.
The public-surface compile witness keeps the intentionally public GPUI helper paths usable:
`TextInputController`, `FieldControl`, `UiA11yElementExt`, `VirtualizedListGpuiExt`, externally
supplied `ScrollHandle`, `focus_ring_shadow_with_theme`, `GpuiOverlayState`,
accessibility/geometry conversions, and adapter scheduling helpers remain supported under
`open_gpui_ui_components::gpui_adapter`. The typed common-tier gate rejects registering its named
adapter-only surfaces as common exports; it does not infer helper ownership from Rust source text.
Any deliberate root or prelude projection is therefore an explicit public-API change that must move
into the same-declaration tier authority and its compile gates.
`FocusRing` itself uses neutral `UiPx`; only the GPUI focus-ring shadow helper returns `BoxShadow`,
and production render paths should use the explicit-theme helper.

When changing GPUI accessibility repair or component metadata that creates explicit cross-node
relationships, also run:

```sh
cargo check -p open-gpui
cargo nextest run -p open-gpui --lib window::a11y::tests::repair_tree_update
```

Manual UI foundation dogfood should use the dedicated gallery after the automated checks pass:

```sh
cargo run -p open-gpui-ui-foundation-gallery
cargo run -p open-gpui-ui-foundation-gallery -- --page components
```

1. Open `Tokens` and confirm the semantic token registry shows surface, text, accent, focus ring,
   destructive, overlay, and modal overlay keys without introducing a styled component layer.
2. Open `Sizing & Density`, switch between compact and desktop from the summary panel, and confirm
   the highlighted density and default size change with the foundation policies.
3. Open `Adaptive`, use the same compact/desktop switch, and confirm device samples show mobile /
   desktop shell mode, compact / regular / expanded class, and panel samples show compact / medium /
   wide classes.
4. Open `Focus & A11y`, tab through the focusable controls, confirm the focus-visible outline is
   visible, click the counter and reset controls, and toggle the switch. The visible counter and
   switch state should match the accessible role/state vocabulary shown by the page.
5. Open `Overlay`, click `open overlay`, confirm the anchored popover appears from the trigger, then
   close it from the popover or press Escape. The geometry readout should keep anchor, layout,
   visual, preferred, and safe-window rectangles visible. The behavior contract matrix should show
   distinct tooltip, popover, dialog, and menu policies for presence, outside press, Escape, focus,
   underlay blocking, and GPUI adapter fields such as deferred priority and snap margin. In the
   Tooltip samples, hover `Hover or focus`, tab to `Focus only`, and confirm each reveals
   descriptive tooltip content while `Disabled` remains unfocusable and closed; `Manual delayed`
   should stay visible and report its custom delay policy. In the HoverCard samples, confirm
   `Profile preview` reports its default-open interactive contract without visually blocking the
   page at load, `Focus preview` opens only from keyboard focus, and `Manual card` opens and closes
   from its gallery control with pass-through or consume outside-press metadata shown in the state
   row. In the Popover samples, confirm `Default open` reports the default-open contract
  without visually blocking the page at load, `Controlled` opens and closes from its gallery
  control, Escape closes the controlled popover, outside press closes the visible popovers, and the
  `Consume outside` sample reports a consuming outside-press policy while `Disabled` remains
  closed. In the Dialog samples, open and close `Controlled modal`, confirm Escape and the modal
  barrier can close it without activating underlay controls, confirm `Default open` reports a
  blocking modal layer without visually blocking the page at load, confirm `Outside ignored`
  reports the sticky outside policy, and confirm `Disabled` stays closed. In the AlertDialog
  samples, open `Delete project`, confirm the destructive action is explicit, cancel receives the
  default focus, outside press is consumed without dismissing, Escape closes it, and focus returns
  to the trigger; confirm the safe cancel sample reports its default-open and modal-underlay
  contract without visually blocking the page at load. In the Sheet samples, confirm the left modal
  sheet reports blocking underlay input, the right non-modal sheet opens from its gallery control
  and reports pass-through outside behavior without a blocking modal barrier, and the bottom sticky
  sheet reports bottom-edge attachment, hidden close affordance, and ignored outside press. In the
  Menu samples, confirm arrow keys move roving focus over enabled
   action items while skipping separators and disabled items, Enter/Space activates the focused
   action and closes the menu, Escape closes the controlled menu, and `Outside ignored` keeps its
   explicit outside policy. In the ContextMenu samples, right-click the hotspot and confirm the
   menu opens from the pointer point, snaps inside the window near edges, and closes on outside
   press or Escape.
6. Open `Components`, or start there directly with
   `cargo run -p open-gpui-ui-foundation-gallery -- --page components`, and confirm Button, Badge,
   Accordion, Collapsible, Slider, NumberInput, ToggleGroup, Link, Breadcrumb, Tag, ToastStack,
   IconButton, Separator, Kbd, Progress, Skeleton, Avatar, ScrollArea, Splitter, Switch, Checkbox,
   RadioGroup, Toggle, Label, TextInput, Textarea, Field, Tabs, Toolbar, Sidebar, Listbox, Select,
   Combobox, Command, Table, and VirtualizedList samples render with enabled, disabled, selected, checked, unchecked,
   indeterminate, pressed, invalid, required, read-only, placeholder, value, help, error,
   decorative, semantic, indeterminate-progress, fallback-initial,
   source-metadata, roving-focus, popup, overflow-axis, scroll-reset, resize-constraint, row-model,
   and virtualized-viewport states. The Badge, Kbd, Skeleton, and non-removable Tag samples should
   remain display-only.
   The Accordion and Collapsible samples should expose stable disclosure values, disabled rows, and
   open-state readouts. Slider and NumberInput samples should expose clamped min/max/step metadata,
   disabled/read-only or invalid states, and keyboard or step payload semantics. ToggleGroup should
   expose single and multiple stable-value selection with disabled-item skipping. Link and
   Breadcrumb should expose accessible navigation labels and activation metadata. Tag should expose
   removable and disabled-remove metadata. ToastStack should expose visible stack ordering,
   overflow, timeout pruning, dismiss reasons, and action metadata without owning timers.
   Use a few catalog cards, such as Table, Tree, and VirtualizedList, to enter focused
   component-family mode; confirm unrelated samples are hidden, the section directory stays
   available, nested sample scrolling still stays inside the sample, and `All components` restores
   the full conformance page with the page scroll reset. The Separator samples should distinguish semantic and
   decorative roles. The Progress samples should cover determinate and indeterminate values, with
   indeterminate progress rendering as a short non-percentage segment rather than a fixed 33% fill.
   The Avatar samples should show derived fallback initials, explicit fallback text, explicit
   accessible labels, and source metadata without owning image loading. The IconButton samples
   should be square controls with visible focus and explicit accessible labels. The ScrollArea samples should cover vertical overflow, horizontal overflow,
   and two-axis overflow; wheel or trackpad scrolling should stay inside each constrained viewport
   while the state readout reports the expected axis and reset policy. Scroll each constrained
   ScrollArea once, then continue scrolling the same viewport after the content has moved; it should
   keep moving instead of snapping back to the origin after the redraw caused by the first scroll.
   The gallery navigation rail should also scroll independently inside its own viewport so deep
   sections remain reachable on compact windows. The vertical Tabs sample should keep its tab rail
   scrollable inside the constrained gallery card, and the focused component smoke now verifies the
   shared `ScrollArea` viewport directly through `tabs_vertical_tablist_scrolls_when_constrained`.
   The Splitter samples should
   show horizontal and vertical panel groups, stable handle affordances, min/max fraction readouts,
   collapsed-panel metadata, and pointer-drag resizing without changing surrounding layout. Drag the
   vertical collapsed sample far enough to restore the collapsed panel, then confirm subsequent
   dragging resizes it normally. The RadioGroup samples should
   cover vertical required selection and horizontal navigation that skips disabled items. The Toggle
   samples should expose button-like pressed state without behaving like a checkbox. The Tabs
   samples should cover horizontal automatic activation and vertical manual activation; use arrow
   keys, Home/End, Enter, and Space to confirm focus movement and activation behavior. The vertical
    sample should keep its tab rail scrollable inside the constrained gallery card. The Toolbar
    samples should expose horizontal and vertical command groups; use arrow keys plus Home/End to
    confirm roving focus skips disabled items and separators. Use Enter or Space to activate action
    items, and Space only to activate toggle items. The component runtime smoke now verifies the rendered Toolbar keyboard path
    for disabled-item/separator skipping and activation payloads. The Sidebar samples should expose
   expanded, icon-collapsed, and long scrollable navigation; icon collapse should hide visible labels
   while keeping item labels explicit, disabled and duplicate values should fail closed, and the long
   sidebar should scroll inside its sample frame. Semantic activation coverage verifies pointer,
   Enter/Space key-up, AccessKit, and programmatic parity, item-handler override, and focus repair
   without stealing external focus. The component smoke verifies the shared `ScrollArea` viewport through
   `sidebar_long_navigation_scrolls_inside_shared_scroll_area`, and the gallery smoke verifies the
   long sidebar's internal viewport moves relative to its sample card. The Listbox samples should
   expose
   grouped options, disabled option skipping, selected and active descendant metadata, empty-state
   behavior, and keyboard navigation/activation with Up/Down/Home/End plus Enter/Space. The
   component runtime smoke now verifies rendered Listbox disabled clicks, selection-free arrow
   navigation, disabled/separator skipping, and option/listbox callback parity for keyboard
   activation. The Select
   samples should expose closed, controlled-open, and disabled states; confirm the trigger label
   reflects the selected option, the open sample uses a non-modal dismissible listbox popup with a
   scrollable long option set, Escape/outside press dismisses it, disabled empty select remains
   closed, and the state readout keeps trigger-selected value distinct from popup listbox selection.
   The component runtime smoke now verifies rendered Select trigger opening, disabled popup
   option rejection, click selection, keyboard popup selection that skips disabled rows, selection
   payloads, and ordered popup close callbacks. The Combobox samples should expose editable
   filtering, selected value metadata that does not disappear when the query hides the selected
   option, an empty filtered state, disabled input/popup suppression, and visible query metadata.
   The component runtime
   smoke now verifies real Combobox text-input editing, filtered popup options, filtered option
   click selection, and close callbacks. The Command samples should expose ranked search results,
   selected chips for multi-select, stable selected values independent of result order, a 10k-item
   virtualized command result window, app-owned
   indexed/loading metadata, shortcut labels, inline and dialog-backed presentation, and modal
   dialog outside/Escape dismissal while preserving the Components page scrollability. The component
   runtime smoke now verifies real Command text-input editing, inline filtering, keyboard
   activation, shortcut payloads, non-dialog content persistence, multi-select toggling, virtualized
   scrolling/reveal behavior, and app-owned index snapshot state. The default TextInput
   sample should accept real text editing through the controller-backed path, and the password
   sample should show masked display metadata while preserving the underlying value contract. The
   Textarea samples should expose placeholder, filled, overflowing, and invalid states; wheel input
   inside the overflowing textarea should scroll its multiline content without moving the sample
   card or outer Components page. The
   gallery remains scrollable and keeps focus visible when the page overflows. The Table samples
   should expose the `release-queue` 10k-row virtualized window,
    the filtered/sorted/paginated `filter-board` model with working status `TableFacetedFilter`,
    score `TableRangeFilter`, and name `TablePredicateFilter` controls,
    the controlled `release-resize` sizing
    sample, the grouped and sticky pinned `release-rollup` model with left/right fixed lanes and a
    horizontally scrollable center lane, the wide `release-matrix` center-column window with a
    working `TableColumnVisibility` control, the source-tree `dependency-tree` sample with nested
    rows and controlled expansion, stable selected row ids, the editable `editable-release`
    text-cell sample with app-owned row updates, the `multiline-release` textarea-cell sample with
    newline-preserving app-owned row updates, table/row/cell accessibility metadata, sortable header
    metadata, resize handle metadata, row activation, expansion, column-visibility, and cell-edit
    log entries, and internal body viewports that scroll
    without moving the outer Components page.
    The Tree sample should expose `document-outline`,
    tree/tree-item accessibility metadata, expandable `Paper` children, a state readout, an inner
    viewport that scrolls without moving the outer Components page, and selection/toggle events
    through the gallery sample runtime log. The VirtualizedList sample should expose the
    `release-navigation` 10k-item window, listbox/listbox-option roles, active/selected
    metadata, visible/overscan readouts, an internal viewport that scrolls without moving the
    outer Components page, card-chrome wheel containment, and PageDown plus Enter/Space activation
    through the gallery sample runtime log. The app should stay open after opening `Components`;
    an `accesskit_consumer`
   panic during that navigation is a
   regression in the accessibility repair gate. The Components page also serves as a conformance
   surface: confirm the visible component catalog distinguishes official components from
    adapter-only helpers and internal anatomy, and confirms Separator, Kbd, Progress, Skeleton, and
    Avatar are official entries with state types, then confirm the visible gate cards for explicit
    crate exports, gallery metadata, ScrollArea redraw persistence, Splitter runtime constraints,
    Tabs overflow, `table-virtualization`, `tree-renderer`, `virtualized-list-renderer`, and
    explicit accessible metadata on icon-only and label-association samples.
   The Overlay Menu and ContextMenu samples should expose action, checkbox, radio, separator,
   disabled, submenu, typeahead, controlled-open, outside-policy, and point-anchor variants. Use
   `cargo nextest run -p open-gpui-ui-components menu` and `cargo nextest run -p
   open-gpui-ui-components context_menu` to verify rich item payloads, pure typeahead,
   visible-submenu keyboard navigation, submenu hover delay / close timing, local menu scrollability,
   context-menu reuse, and long-menu wheel containment through
   `context_menu_runtime_long_menu_scroll_stays_inside_surface`. Use `cargo nextest run -p
   open-gpui-ui-components
   menu_runtime_hover_opens_submenu_and_preserves_child_focus
   menu_runtime_hover_switches_between_submenu_branches` together with the menu family command to
   cover the hover-delay runtime. Use
   `cargo nextest run -p open-gpui-ui-foundation-gallery
   overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts
   overlay_page_context_menu_samples_expose_point_anchor_contracts
   overlay_page_catalog_entries_have_signals_and_sample_selectors
   overlay_gallery_smoke_closes_menu_from_escape_and_outside_press
   overlay_gallery_smoke_opens_menu_submenu_from_hover
   overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses` plus `cargo check -p
   open-gpui-ui-foundation-gallery --tests` after changing the overlay menu family.
7. Re-run `cargo nextest run -p open-gpui-ui-components` and `cargo nextest run -p
   open-gpui-ui-foundation-gallery` if a manual check exposes a component or gallery regression.

For UI component productization checkpoint work, additionally review
`docs/adr/0008-open-gpui-ui-component-productization-roadmap.md` after the automated component
tests pass. If a future task explicitly reopens extraction, also review
`docs/adr/0006-open-gpui-ui-headless-extraction-checkpoint.md` and
`docs/adr/0007-open-gpui-ui-headless-boundary-design.md`. The checkpoint should continue to
identify which behavior is neutral, which behavior remains GPUI adapter-owned, and why the current
crates remain the active product boundary.

CI runs a three-platform matrix for pushes to `master` / `main`, pull requests, and manual workflow
dispatches:

- Windows runs the same local gate, `cargo nextest run -p xtask`,
  `cargo nextest run -p open-gpui-docking-native --no-fail-fast`,
  `cargo check -p open-gpui-windows --all-features --locked`, and
  `cargo nextest run -p open-gpui-windows --all-features --locked --no-fail-fast`.
- Linux runs `cargo check -p open-gpui-linux --all-features --locked` after installing the system
  headers needed for Wayland, X11, fontconfig, freetype, and pkg-config, then runs
  `cargo nextest run -p open-gpui-linux --all-features --locked --no-fail-fast`.
- macOS runs `cargo check -p open-gpui-macos --features font-kit --locked` and
  `cargo nextest run -p open-gpui-macos --features font-kit --locked --no-fail-fast`.
- All three platforms run `cargo check -p open-gpui-wgpu --features font-kit --locked`.

Run the complete native WGPU package test suite explicitly with:

```sh
cargo run -p xtask -- renderer-smoke
```

That command runs every `open-gpui-wgpu` package test with the `font-kit` feature, including the
real-adapter renderer pipeline smoke and the atlas, lease, shutdown, and renderer contract tests.
Windows CI owns this complete package gate. It is not part of the default `verify` command because
the real-adapter case depends on GPU, driver, and interactive-session availability.

## Renderer Submission And Per-Window Surface Lifecycle Gate

Changes to provisional destination semantics, `PlatformWindowPresentOutcome`, renderer submission
tracking, WGPU surface resize/recreation, or presentation shutdown must run the U31 gate serially:

```powershell
$env:CARGO_BUILD_JOBS = '1'
cargo fmt --all -- --check
cargo nextest run --locked -p open-gpui --test-threads 1 --no-fail-fast
cargo nextest run --locked -p open-gpui-docking --test-threads 1 --no-fail-fast
cargo nextest run --locked -p open-gpui-wgpu --test-threads 1 --no-fail-fast
cargo check --locked -p open-gpui-windows --features test-support
git diff --check
```

The GPUI gate proves that accepted destination semantics remain non-interactive until the exact
accepted frame receives renderer `Submitted`, and that `Deferred`, `RepaintRequired`, `Rejected`,
wrong-frame, wrong-placement, and native-terminal outcomes cannot manufacture submission. Docking
proves that a post-boundary renderer rejection enters committed-destination recovery without
rolling back committed topology or using elapsed time as a semantic verdict. WGPU proves zero-size
suspension, resize-intent coalescing, stale configuration rejection, usable suboptimal
render-then-reconfigure, generation-bound surface recreation, per-window terminal rejection, and
exact presentation-shutdown ownership.

These package tests are not native multi-window release evidence. Every renderer/backend claiming
released multi-window support must additionally run an owning-platform scenario with two real
native windows: submit non-empty frames to both, resize and suspend/resume one, retire the secondary
only after renderer quiescence, then close the primary with an empty renderer/surface/native census.

Run the docking smoke surface explicitly after changing `open-gpui-docking`:

```sh
cargo fmt --all -- --check
cargo check --tests -p open-gpui-docking
cargo check -p open-gpui-docking-minimal --locked
cargo nextest run -p open-gpui-docking
cargo nextest run -p open-gpui-docking-native --no-fail-fast
cargo check -p open-gpui-docking-native
cargo run -p open-gpui-docking-minimal
cargo run -p open-gpui-docking-native
```

When changing docking drop routing, delivery, or public surface boundaries, also run:

```sh
cargo nextest run -p open-gpui-docking drop_target viewport_drop_route viewport_drop_delivery public_surface_tests --no-fail-fast
```

For dockspace scenario-testkit, multiview parity, or benchmark-shape work, run:

```sh
cargo nextest run -p open-gpui-docking dockspace_testkit --no-fail-fast
cargo nextest run -p open-gpui-docking dockspace_scenario_fixture_tests dock_op_fixture_tests --no-fail-fast
cargo nextest run -p open-gpui-docking workspace_move workspace_drop workspace_action host_multiview_scenario_tests host_viewport_model_tests host_viewport_matrix_tests host_interaction_tests public_surface_tests --no-fail-fast
cargo nextest run -p open-gpui-docking --features test-support native_dockspace_projection --no-fail-fast
cargo nextest run -p open-gpui-docking-native native_interactive_scenario_registry_matches_cases worker_subcase_matrix_is_behavior_specific --no-fail-fast
cargo nextest run --locked -p xtask --lib manifest native_gate --no-fail-fast
cargo bench -p open-gpui-docking --bench tab_drag_latency --features test-support
python -m py_compile tools/benchmark_docking_drag.py
python tools/benchmark_docking_drag.py --json
```

The dockspace scenario testkit is internal verification infrastructure, not a public headless
docking crate. The doc-hidden `native_scenario_support` module is compiled only for tests or the
`test-support` feature and projects immutable fixture data into the workspace native adapter; graph
mutation still runs through production `DockSurface` and workspace transactions. Scenario rows
should identify typed oracle domains and ImGui reference categories without copying ImGui internals
or replacing `DockSurface` as the application facade.

For docking presentation/preview/motion work, the focused semantic gates are:

```sh
cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests --no-fail-fast
cargo nextest run -p open-gpui-docking-native runtime_status_panel_formats_platform_capabilities --no-fail-fast
```

For Dock visual-style authority changes, also run:

```sh
cargo nextest run --locked -p open-gpui-docking -E 'test(/visual_style/) or test(/viewport_payload_drag::tests/)' --no-fail-fast
cargo nextest run --locked -p open-gpui-docking-native theme_adapter_produces_distinct_complete_dock_styles --no-fail-fast
cargo run --locked -p xtask -- scan-ui-contract
```

The style gate requires one complete `DockVisualStyle` for every render path, a read-only
per-surface or explicit-host resolver, unchanged graph and surface revisions for style-only updates,
window and subtree isolation, source-opening versus live-target drag semantics, payload identity
stability, retired-session cleanup, and the structural `DockDropGuideMetrics` name. The source scan
rejects production UI Components dependencies, retired guide-style names, competing theme lookups,
`Default` implementations for complete Dock visual inputs, and color or shadow literals outside
the exact built-in style definition scopes. The native example maps light, dark, and high-contrast
snapshots in the application layer; Docking itself remains theme-independent.

For source-owned captured native drag transport changes, also run:

```sh
cargo nextest run -p open-gpui native_captured --no-fail-fast
cargo nextest run -p open-gpui-docking native_captured --no-fail-fast
cargo nextest run -p open-gpui-windows hit_stack pointer_capture --no-fail-fast
```

These gates inject movement and release into the source window while the target receives no raw
pointer event. They require atomic activation of the exact prepared generation, preservation of the
original ingress sequence across the AppCell outbox boundary, a callback-scoped physical point and
coherent hit-stack geometry, and exactly-once terminal cleanup for release, cancellation, capture
loss, source close, shutdown, replacement, and panic. `Unavailable`, a cross-point or incomplete
observation, an opaque top-level, a stale host scene, and a foreign Dock surface must never fall
through to a hidden host or reuse the last valid preview.

Release locking runs before source-window mouse interceptors and listeners. The post-borrow Dock
consumer receives the exact candidate and host scene reserved at `MouseUp`. It may roll the frame
proof forward only across a semantically identical redraw with the same registration, binding,
runtime context, geometry, and routing facts; it cannot re-hit-test into a replacement candidate.
Resolver panic is carried to post-borrow delivery so the exact route is claimed and cleaned before
the panic resumes.
Platform capture release is likewise prepared once before delayed dispatch, and retries
reuse its frozen pointer-session identity. Shutdown may retire the logical registry while waiting
for native capture terminal, and a rejected native destroy retains its platform-window owner until
retry succeeds. Automated owners include
`native_captured_release_does_not_retarget_to_a_scene_created_by_mouse_up_listener`,
`capture_release_is_prepared_before_a_budget_delayed_first_dispatch`,
`shutdown_uses_native_window_terminal_after_capture_release_retries_are_saturated`,
`stale_framework_release_cannot_target_a_newer_native_capture`, and
`failed_destroy_during_platform_window_retirement_retries_without_losing_owner`.

## Windows Native Interactive Runner Gate

Claims about system-routed cross-HWND pointer input, actual Win32 capture ownership, or a live
provisional viewport require the self-hosted runner label
`open-gpui-windows-native-interactive-ephemeral`. The runner is owned by the Windows backend CODEOWNER in
`.github/CODEOWNERS`; runner outage or missing capacity is an infrastructure failure that blocks
the gate. Do not skip the job, move it to `windows-latest`, or replace system input with
`SendMessageW` / `PostMessageW`. Escalate an unavailable or incapable runner through a repository
infrastructure issue assigned to `@Latias94`.

The self-hosted workflow is not triggered by `pull_request`. It accepts only merge-queue commits,
trusted pushes to the primary branch, or a maintainer `workflow_dispatch` with a full `commit_sha`;
checkout is rejected unless HEAD matches that object exactly. Pull-request candidates receive this
evidence through the merge queue or a maintainer-owned trusted-branch reproduction, never by running
their head revision directly on the self-hosted desktop. Every third-party action in this workflow is
pinned to an immutable full Git object id. The runner must be a separate immutable one-job Windows
host registered with `--ephemeral`, with no repository or organization secrets, no reusable
credentials, and no access to privileged internal resources. Its machine or VM must be destroyed or
returned to a known-clean image after the job. A persistent self-hosted desktop must never carry the
ephemeral label or execute this workflow. Runner logs must be forwarded before the disposable runner
is destroyed.

The runner must provide all of the following:

- an unlocked interactive user desktop rather than a service session;
- compatible process integrity and UIPI boundaries for `SendInput`;
- one serial owner of the global cursor and pointer buttons;
- a non-empty virtual screen containing the test window;
- a functioning native renderer/GPU path;
- permission to foreground the sentinel HWND without human interaction;
- one-job ephemeral registration and a clean disposable host; and
- no credentials or unrestricted network reachability that candidate code could retain or abuse.

Before checkout, the workflow reads the following machine-level attestations directly from the
runner host and fails closed if any are absent or inconsistent:

| Machine environment variable | Required evidence |
| --- | --- |
| `OPEN_GPUI_NATIVE_RUNNER_EPHEMERAL` | exact value `1` |
| `OPEN_GPUI_NATIVE_RUNNER_ONE_JOB` | exact value `1` |
| `OPEN_GPUI_NATIVE_RUNNER_CLEAN_IMAGE_ID` | immutable image/build identifier for the disposable host |
| `OPEN_GPUI_NATIVE_RUNNER_ACCOUNT_SID` | exact SID of the dedicated low-privilege interactive account |
| `OPEN_GPUI_NATIVE_RUNNER_NETWORK_POLICY` | exact value `deny-default-v1` |

The preflight also requires `Environment.UserInteractive`, a non-zero Windows session id, and a
current process SID matching the attested account. These facts are provisioned in the machine image,
not through repository variables, workflow inputs, or secrets. The workflow grants only read-only
repository contents and references no repository or organization secret. Infrastructure owns image
destruction and network-policy enforcement; the repository gate verifies their machine-stamped
identities before any candidate checkout or Cargo execution.

The preflight opens a rendered GPUI window, waits for a non-empty framework frame to receive a
`Submitted` renderer outcome, records and later restores the foreground window and system cursor,
injects mouse move/down/up through `SendInput` without naming a receiver, observes the actual WndProc
recipient, and verifies `GetCapture()` acquisition and release. A five-second deadline waits for
the exact canary-tagged HWND message together with its GPUI observer/capture effect, so stale input
cannot satisfy the preflight. Stack unwinding releases a matching test capture and restores
the original cursor; if failure occurs after injected button-down, cleanup first injects a
best-effort button-up so the desktop cannot remain in a global drag. The input guard is destroyed
before application/platform teardown, which continues to own every test HWND. The workflow
serializes all runs through the same concurrency group and never cancels an in-progress cursor
owner.

Run the sentinel only on that dedicated runner:

```powershell
$env:CARGO_BUILD_JOBS = '1'
$env:OPEN_GPUI_NATIVE_INTERACTIVE = '1'
cargo nextest run -p open-gpui-windows --locked --run-ignored only -E 'test(=platform::native_test_support::native_interactive_runner_sentinel_proves_system_pointer_delivery_and_capture)' --no-fail-fast
```

The sentinel is the phase-zero runner proof. The native Dock manifest
`examples/docking-native/tests/native_windows_interactive.native-scenarios.toml` binds fifteen stable
scenario ids to one exact package/test coordinate, one U27-U29 requirement owner, one behavior
dispatch, an explicit ignored policy, and the typed native observation domains that worker must
cross:

| Scenario id | Owner | Required observation domains | Exact native worker responsibility |
| --- | --- | --- | --- |
| `native.u27.source-capture` | U27 | `system-input`, `wnd-proc`, `capture`, `point-stack`, `presentation` | source-owned `GetCapture`, OS-routed move/up, target preview, and committed host drop |
| `native.u29.complex-dockspace-workflow` | U29 | `system-input`, `wnd-proc`, `capture`, `point-stack`, `presentation`, `lifetime` | three distinct HWNDs execute the complete `three_viewport_rehome` fixture in order, with renderer-submitted preview, exact headless layout checkpoint, target focus/selection, captured route lineage, and live-window checks after every step |
| `native.u27.opaque-occlusion` | U27 | `system-input`, `wnd-proc`, `capture`, `point-stack`, `presentation` | ordinary opaque HWND barrier, point-scoped Z-order, and no hidden-host targeting |
| `native.u27.opaque-hit-transparent-prefix` | U27 | `point-stack`, `lifetime` | clean-desktop Windows proof that an unregistered visually opaque HWND remains an opaque routing barrier even when its native hit test passes through to a lower HWND |
| `native.u27.surface-shutdown` | U27 | `system-input`, `wnd-proc`, `capture`, `presentation`, `lifetime` | capture cancellation plus dependent-window and ticket convergence after anchor close |
| `native.u29.provisional-same-hwnd-promotion` | U29 | `system-input`, `wnd-proc`, `capture`, `point-stack`, `presentation` | one continuous proof of visible non-empty provisional presentation, pre-release interaction rejection with no replay, and durable same-HWND promotion |
| `native.u29.live-route-and-release-lock` | U29 | `system-input`, `wnd-proc`, `capture`, `point-stack`, `presentation` | same visible HWND follows A-to-B-to-C route placements while the source owns capture, then `MouseUp` locks the final sample against immediate later cursor motion |
| `native.u29.committed-loss-recovery` | U29 | `system-input`, `wnd-proc`, `capture`, `point-stack`, `presentation`, `lifetime` | committed promoted-window loss, one visible recovery entry, real native activation of its `Restore` control, exact topology/revision recovery, and HWND retirement |
| `native.u28.activation-terminal` | U28 | `wnd-proc`, `lifetime` | ordinary detached creation does not steal activation, request generations settle first-terminal-wins from real User32 facts, later native focus cannot revive rejection, close settles `WindowClosed`, and no keyboard messages are synthesized |
| `native.u28.provisional-activation-z-order` | U28 | `wnd-proc`, `point-stack`, `presentation`, `lifetime` | clean-desktop real-HWND proof that same-window promotion retains, invalidates, reconstructs, and consumes point-scoped opaque-barrier authority across physical placement and native activation without accepting stale HWND incarnations |
| `native.u28.client-geometry-reconciliation` | U28 | `wnd-proc`, `presentation` | real HWND initial presentation rejects injected custom-titlebar client drift, restores the hidden placement, and accepts only a bounded exact retry |
| `native.u28.event-driven-geometry-wake` | U28 | `wnd-proc`, `presentation` | two external native geometry callbacks coalesce behind one posted wake and publish the latest coherent physical geometry without polling the foreground executor |
| `native.u28.process-convergence` | U28 | `system-input`, `wnd-proc`, `capture`, `presentation`, `lifetime` | active captured surface shutdown, live pre-exit HWND census, and whole worker process-tree convergence under a kill-on-close Job Object |
| `native.u28.no-input-pass-through` | U28 | `system-input`, `wnd-proc`, `capture`, `point-stack`, `presentation` | two consecutive no-input GPUI HWNDs pass through to a lower GPUI HWND, stop at a lower external opaque HWND, and reject a generation-changing sample |
| `native.u28.mixed-dpi-client-bounds` | U28 | `system-input`, `wnd-proc`, `capture`, `point-stack`, `presentation`, `lifetime` | captured desktop tear-off across the same real distinct-DPI display pair in both lower-to-higher and higher-to-lower directions, with at least one negative physical display origin, same-HWND provisional promotion, and exact target-side physical client bounds; missing dual-DPI or negative-origin capacity is a gate failure |

The schema 6 manifest is the native-scenario execution inventory and selector-metadata authority: its
rows supply the scenario id, requirement owner, package/test coordinate, ignored policy, typed
observation domains, behavior, and optional dockspace fixture id used to construct each exact
`nextest` invocation. A complex-workflow row does not duplicate fixture steps or declare a synthetic
single step: the worker loads and executes the complete checked-in fixture in order. `xtask` rejects
an unknown scenario, a missing native adapter/oracle, or a singular step on a complete workflow.
`xtask`
maintains only the closed `NativeScenarioBehavior` vocabulary needed to dispatch typed receipts; it
does not duplicate package, exact-test, or ignored metadata. `native_manifest_failures` requires one
row for every supported behavior and rejects duplicate ids, behaviors, or package/test coordinates
without attempting to parse Rust source. The non-ignored example registry test reloads the same
manifest and checks row identity plus evidence basics rather than maintaining a second ownership
table. A Dock worker receives the selected scenario id through `OPEN_GPUI_NATIVE_SCENARIO_ID`, reloads
that manifest row, and dispatches it through the matching behavior-specific wrapper. The workflow
invokes one typed runner entry, and the runner builds one exact `nextest` process per manifest row in
either `open-gpui-docking-native` or `open-gpui-windows`. The workflow does not contain a second list
of selectors, so an alias worker cannot satisfy two scenario ids. The CI-negative fixtures exercise
the manifest's fail-closed schema, unique row/behavior/coordinate rules, behavior receipts, and
workflow-entry checks before the interactive workers run.

The provisional activation Z-order and opaque hit-transparent prefix rows are intentionally ignored
in the ordinary Windows package suite. They mutate or inspect global foreground and top-level
Z-order state and therefore require the named ephemeral interactive runner's clean desktop and
capability preflight. Moving them behind the typed manifest prevents unrelated user windows or
integrity boundaries from becoming false product failures while retaining the exact real-HWND
proofs as release gates.

The dedicated workflow entry is:

```powershell
$env:CARGO_BUILD_JOBS = '1'
$env:OPEN_GPUI_NATIVE_INTERACTIVE = '1'
cargo run --locked -p xtask -- native-windows-interactive
```

The typed runner first executes the Windows system-input sentinel, the Windows backend baseline, and
the non-ignored native scenario registry. It then runs each manifest row independently with its exact
package and `test(=...)` selector, `--test-threads=1`, and `--no-tests fail`; rows whose manifest
metadata sets `ignored = true` add `--run-ignored only`, regardless of whether the owning package is
Dock or the Windows backend. The rendered native Dock baseline runs afterward. A scenario failure
therefore cannot be hidden by a broad regex, an alias worker, a package mismatch, or an accidentally
skipped ignored test.

Each worker uses a two-phase, nonce-bound pipe protocol. The parent creates the child with piped
stdin/stdout, assigns it to a Windows Job Object configured with
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, and only then sends `START <nonce>`. After the scenario shuts
down its surface, the worker waits for the application/platform event loop to return, writes exactly
one `OPEN_GPUI_NATIVE_DOCK_REPORT <nonce> <json>` line to stdout, and remains alive waiting for
`RELEASE <nonce>` on stdin. The typed pre-exit census includes the logical window registry, active
drag, surface session/runtime terminal state, capture ownership, every observed native generation
and its terminal acknowledgement, and the worker's ordinary and message-only HWND sets. The parent
independently enumerates the same PID while the worker is still alive, compares both census views
with the pre-application baseline, and verifies that the Job still owns exactly one active process.
Only then does it send the release acknowledgement. A timeout or unwind terminates the entire Job,
and successful exit must leave its active-process count at zero. No temporary file carries start,
report, release, or liveness authority. The former post-exit `EnumWindows` assertion is removed
because process termination would make it pass without proving framework-owned retirement.

The `native.u27.source-capture` row opens two distinct rendered Dock HWNDs, derives source and target
points from their committed debug-selector bounds and actual client-to-screen scale, and injects an
unaddressed system drag. It requires the source HWND to acquire and retain `GetCapture()` while the
system cursor crosses the target HWND, requires the source framework observer to receive the captured
move/up while the target observer receives no mouse event, waits for a non-empty target drop-preview
frame, and then requires one newer durable surface revision in which the payload moved from the source
space to the target space. The sentinel separately proves the canary-tagged WndProc boundary. The
test restores the cursor, foreground window, button state, and matching capture during normal
completion and unwinding.

This two-HWND scenario proves captured routing, preview, and committed-host drop. It does not by
itself claim desktop live-undock, provisional-content reveal, or same-HWND promotion completion;
those remain separate manifest rows rather than being inferred from this gate.

`native.u29.complex-dockspace-workflow` is the multi-operation Dockspace gate. It opens the fixture's
primary space plus two dependent viewport spaces as three non-overlapping native windows, keeps a
resident tab in each dependent space so normal viewport-empty closure never invalidates the next
operation, and performs four real captured drags across changing source/target ownership. Each step
derives its source and target from committed debug bounds, resolves the target point through
production drop-guide geometry, and waits for a newer renderer-submitted frame whose active guide
matches that exact zone and its production device-pixel-snapped bounds. It then releases through the
source-owned WndProc capture route and accepts completion only when a strictly newer submitted frame
has cleared the active guide and the exported layout matches the corresponding production headless
checkpoint. The worker then requires all three HWNDs to remain live until normal application
shutdown, whose process census and native terminal events remain the lifetime authority.

`native_interactive_provisional_gate_presents_and_promotes_same_hwnd` moves a captured payload to a
runner-selected point outside both test HWND rectangles and keeps the
physical button down. It requires a third, runtime-opening, natively visible HWND with a submitted
non-empty frame containing the payload before release, while the provisional remains gated for
pointer input, activation, and focus. It then releases the locked point and requires the same HWND
(not a replacement) to become the committed viewport, with exactly one durable surface revision
and one graph ownership transfer. A framework mouse observer is installed as soon as the exact
provisional window appears and proves that the captured move/down/up stream is not replayed into
that window.

`native_interactive_live_route_reuses_same_hwnd_and_locks_release` starts from the same admitted
provisional but owns a distinct movement contract. While the source retains native capture, the
visible provisional must follow route samples A, B, and C through generation-bound physical client
placement without another show, hide, HWND, graph revision, or payload transfer. The worker releases
at C and immediately moves the system cursor back to B; the final placement, route destination, and
same-HWND promotion must remain bound to the locked C sample.

`real_hwnd_activation_terminal_is_exact_and_first_terminal_wins` runs in the owning Windows package
without system-input injection. It first proves that presenting an ordinary detached HWND does not
steal foreground, thread activation, or keyboard focus. Two programmatic requests then prove exact
generation supersession and an observed `Activated` or OS `Rejected` terminal. A policy-rejected
ticket remains terminal after a later real User32 foreground/focus transition, and destroying the
target HWND before its queued command dispatches settles that exact ticket as `WindowClosed` once.
The WndProc trace must contain no framework-synthesized keyboard message.

`real_hwnd_provisional_reveal_is_nonactivating_hit_transparent_and_promotes_in_place` additionally
forces a real foreground activation after destination semantics are submitted but before the
provisional interaction gate opens. Windows must restore the prior foreground HWND, suppress a
positive framework activation observation, reconstruct the point-scoped opaque-barrier proof, and
retain that authority for the later formal activation. After admission, the same scenario minimizes
the promoted HWND and requires a non-activating native restore plus a fresh barrier verification
before the backend activation command may settle.

`real_hwnd_windowed_initial_presentation_rejects_client_drift_before_retry` injects client-geometry
drift into a real custom-titlebar HWND during initial presentation. The first transaction must stay
hidden, reject the stale presentation, restore the retained native placement, and preserve the
bounded retry authority. Only an exact post-show client-geometry readback may publish the successful
presentation facts.

`event_driven_geometry_wake_commits_latest_native_geometry_without_polling` applies two external
native resizes before GPUI consumes either callback. Committed window facts must remain unchanged
until the platform HWND receives its posted foreground wake. Dispatching that one wake must publish
the latest coherent physical geometry, proving callback-driven convergence without manually polling
the foreground executor.

`native_interactive_no_input_prefix_passes_through_and_fails_closed_on_generation_drift` creates two
real GPUI top-level windows above the lower Dock host and commits `accepts_pointer_input = false` on
both. The owning Windows hit stack must traverse both `HTTRANSPARENT` windows and publish a target
preview for the lower GPUI HWND. A one-shot typed backend hook advances one exact no-input observation
generation between the first and verifying samples; that event must publish no preview. The same
prefix is then placed above an ordinary external opaque HWND, which must terminate routing before the
lower GPUI host.

`native_interactive_mixed_dpi_final_client_bounds_are_exact` enumerates physical monitor facts through
the Windows test-support module and freezes one real display pair with distinct effective DPI and at
least one negative physical origin. Separate lower-to-higher and higher-to-lower worker subcases use
that same pair. Each begins one real captured payload drag on the selected source monitor, opens and
presents the provisional on that source DPI, moves the still-captured system pointer to an open
desktop point on the target monitor, and releases there. The selected negative-display probe must be
the actual reveal or release point, not metadata-only evidence. The same provisional HWND must
converge to the target display identity, scale, independently calculated placement, and direct native
physical client bounds before release, promote without replacement, and retain those facts through a
subsequent accepted frame. A runner without two distinct-DPI displays and a negative-origin member
fails capability preconditions; the scenario never converts missing hardware into a skip.

`native_interactive_opaque_occlusion_blocks_underlay_and_preserves_same_hwnd` adds an ordinary
opaque top-level HWND over the final point. It requires independent frontmost verification to keep
that barrier above the visible provisional while final placement and same-HWND promotion remain
exact-session scoped. Immediately before system `LEFTUP`, the release point must still resolve to
that exact barrier HWND. After the source WndProc receives the captured `LEFTUP`, the worker also
requires the exact generation-locked callback observation to carry the delivered physical point and
the typed `OpaqueBarrier` terminus. Unlike ordinary desktop release rows, this scenario intentionally
does not claim an open-shell-desktop release point.

Ordinary desktop-release rows treat their pre-`SendInput` open-desktop check only as an environment
precondition. After the source WndProc receives the captured `LEFTUP`, the worker also reads the
exact generation-locked release observation recorded inside GPUI's callback and requires its sampled
physical point to equal the delivered release point and its typed terminus to be `OpenDesktop`.
`OpaqueBarrier` and registered-window termini cannot receive open-desktop evidence even when they
would otherwise select the same high-level desktop tear-off route.

`native_interactive_committed_destination_loss_retires_runtime_authority` arms one generation-bound
test seam before `MouseUp`. The production promotion executor first crosses its durable graph
boundary, then that seam terminates the exact same-HWND destination before destination-semantics
acknowledgement. The worker waits until its registration, runtime ownership, capture, GPUI window,
and HWND retire. It then requires exactly one visible `LostViewportRecovery` entry in a surviving
window, locates that entry's committed `Restore` control, and activates it through real system mouse
move/down/up. The test accepts success only after the entry disappears, the payload moves into the
anchor recovery space, the lost space no longer owns it, and the surface publishes exactly one loss
revision followed by exactly one recovery revision. A later ordinary close after interaction
admission is intentionally not credited as committed-loss authority.
Deterministic AccessKit tests separately own focus and action-tree semantics; this native row does
not claim operating-system assistive-technology E2E.

`native_interactive_anchor_close_releases_capture_and_retires_dependent_hwnds` begins a real captured
drag, posts `WM_CLOSE` to the primary anchor, and proves exact capture cleanup followed by dependent
close dispatch and finally ordinary anchor close dispatch. The process-convergence row adds two
independent-surface subcases. The last-window subcase closes the primary surface, proves the survivor
remains isolated, and then closes the final anchor under the application quit policy. The terminal
App-shutdown subcase enters shutdown from a still-checked-out provisional builder while another
provisional is visibly gated and the source owns capture. Capture cleanup must precede both dependent
close dispatches; the primary anchor is then retired by App registry clear rather than an ordinary
anchor close dispatch. Both subcases require the surface sessions to reach `Closed`, the viewport
runtime and GPUI window registry to empty, native capture to clear, active drag to disappear, all
terminal tickets to settle without failure, and every owned HWND to be destroyed. The typed native
observation stream proves presentation quiescence before destruction, `WM_NCDESTROY` before native
terminal acknowledgement for each window, and dependent `WM_NCDESTROY` before anchor destruction
begins. The test restores the physical button state through an unwind-safe guard; a separate
60-second parent watchdog sends a best-effort button-up, terminates the worker Job, and reports its
captured artifacts instead of leaving the runner in a global drag or a descendant process alive
until the workflow timeout.

These scenarios are separate assertions, not a claim that one worker proves every U28 matrix cell.
Two-surface isolation, renderer fault injection, and terminal-order permutations continue to have
deterministic or subprocess coverage listed below and must not be inferred from a different native
scenario id. Mixed-DPI placement receives native credit only from its dedicated dual-DPI manifest
row.

The docking native example exercises the public multi-window setup: applications build one
`DockController`, wrap it in a `DockViewportRuntimeHandle`, register window-close cleanup, and open
controller-backed primary and secondary `DockHost` viewports. The runtime panel reports both the
last route target and the route selection source, so dogfood runs can distinguish captured native
hit-stack routes, trusted hovered-window routes, window-stack fallback routes, focus-stamp fallback
routes, and current-facts rejections. It also reports the current platform viewport capability
snapshot, including `window_hit_stack` independently from placement facts, so platform-boundary
regressions are visible during native dogfood. The
placement restore line reports matched and missing restored windows, and the tear-off status line
reports whether a viewport opened from suggested bounds or drag-source geometry, so placement
authority regressions are visible in the same panel.
The primary, secondary, and central hosts deliberately use light, dark, and high-contrast contexts.
During a routed drag, the source-owned deferred visual must keep the source opening-generation
style, while target guides and previews must immediately use the destination host style. Closing or
cancelling and reopening the same payload must capture a new source generation.

Docking target previews are scene-owned. During dogfood, every target-window preview should be
explainable from the same capability model: `DockPresentationScene` resolves panes, tab bars,
tab labels, splitters, floating containers, focus regions, and overlay anchors; `DockPreviewScene`
describes allowed/rejected target facts; `DockVisualAffordanceScene` gives stable layer identity for
preview bodies, guide boxes, tab insertion, payload tabs, route markers, and rejected state;
`DockTransitionPlan` describes motion/reduced-motion semantics; divider hit maps, zoom/focus state,
accessibility descriptors, and runtime diagnostics consume the same descriptor path. Rendering must
not recreate guide availability independently from those descriptors. Debug selectors reflect that
contract:
target-stack guides use `dock:<space>:drop-guide:inner:<tabs>:<zone>`, root/host guides use
`dock:<space>:drop-guide:outer:<zone>`, the split body is exposed separately from the full target
preview container as `dock:<space>:drop-preview:body`, and center/tab docking exposes a
`dock:<space>:drop-preview:tab-insertion` affordance before payload tab previews.

Manual native docking dogfood should use the same example after the automated checks pass:

1. Launch `cargo run -p open-gpui-docking-native` and confirm the app opens `Docking demo`,
   `Docking preview`, and `Empty central dogfood` windows.
2. Drag a primary-class tab from `Docking demo` into another primary-compatible target; the preview
   must appear in the destination window and release must select the moved item there.
3. Drag the `Preview` / `Diff` secondary-class stack from `Docking preview` back into `Docking demo`;
   item order and the active tab must be preserved.
4. Drag `Preview` / `Diff` over `Empty central dogfood`; the route must render as rejected and
   release must not mutate the graph because the central space only accepts central-class panels.
5. Use `Restore central note` from the runtime status panel; the `Central note` panel must reopen in
   the empty central window and recover the central-region identity instead of becoming ordinary
   root-only content.
6. Drag a tab or stack outside every docking window; a new runtime-backed viewport must open before
   the graph moves the source payload.
7. Dock the torn-off viewport content back into an existing window; the destination window must
   activate and the moved item must become the selected tab.
8. Move runtime-opened windows across displays, choose `Save placement`, then use `Reopen closed
   demo viewports`; restored placement should use saved bounds only as placement input while live
   Windows captured-drag routing continues to use the point-scoped physical hit observation and its
   embedded target geometry. Model-level fallback routes use current viewport facts, never saved
   placement. On macOS, windows on a secondary display should keep non-overlapping desktop-space
   bounds for the model-level route paths that consume those facts.
9. Exercise the runtime panel close-policy controls for prevent, retain, and merge-back behavior;
   closing a viewport must match the selected policy without losing descriptor-backed panel restore
   or leaving a stale cross-window route preview in another viewport.
10. Start a cross-window drag, hover a valid target, then move to an area of the same viewport with
   no current dock target before releasing; the previous preview must not commit from stale target
   state.
11. Drag over the empty central dogfood window; empty central-space preview, rejection, and
   passthrough behavior must match the visible policy state.
12. While dragging over a valid tabs target, confirm the guide affordance is target-owned and
    box-shaped rather than a floating five-button cluster. Center hover should show a center box,
    side hover should highlight only the corresponding side box, and inactive boxes should remain
    visibly weaker than the active one.
13. Hover the center of a compatible target and confirm the destination window renders one dock
    preview plus one contained payload tab preview. Hover any split edge and confirm the preview
    becomes an edge band and the payload tab preview disappears.
14. Reproduce a nested-target case by docking into a child region, then dragging another tab into
    the remaining nested leaf. Confirm hovering the left or right side of that nested leaf resolves
    inside the nested leaf itself rather than snapping to the neighboring region or to the root
    edge.
15. Drag a tab or stack outside every valid host. Confirm the route marker reads as tear-off or
    rejection only; no fake blue dock target or payload tooltip should appear at the source.
16. Drag the two-tab `Preview` / `Diff` stack over a compatible target stack center. The
    destination preview must show a shared preview body plus two selected-tab-like payload tab
    previews in payload order; the previews should clip to the target tab bar instead of becoming a
    single dark rectangle.
17. Repeat the same two-tab stack drag across windows. The target window must render the same
    payload tab preview structure as a local hover, while the source window shows only route-marker
    feedback when applicable.
18. Hover a side drop box for the root central leaf and a side drop box for a nested child leaf.
    The root central leaf should use outer split semantics; the nested child leaf should keep
    inner split semantics.
19. Hover rejected center and route targets. The target preview must use rejected tokens, suppress
    payload tab previews, and leave the graph unchanged on release.
20. Start a routed cross-window hover, then hover a different compatible viewport before release.
    The old target window must lose both its target preview and payload tab previews, while the new
    target owns the current preview.
21. Press Escape during a routed drag after a target preview appears. The source route marker,
    target preview, runtime drag session, and GPUI active drag must all clear in one frame.

Current docking multi-viewport capability states:

- Platform viewport windows require two gates. `DockPolicy::allow_platform_viewports` is the
  workspace/app opt-in, while `PlatformViewportCapabilities::platform_viewport_windows` is the
  backend fact that independent application viewport windows exist. The capability defaults to
  false; macOS, Windows, X11, and the test platform opt in, while Web and Wayland fail closed.
  When policy allows tear-off but the backend capability is false, outside-viewport route preview
  records `PlatformViewportWindowsUnsupported`, and explicit `open_viewport` / tear-off runtime
  opens return `Unsupported` before creating a GPUI window. Single-window docking and in-window
  split/merge/floating interactions remain available on web. Automated owners:
  `viewport_runtime_handle_drop_route_fails_closed_when_platform_viewport_windows_unsupported`,
  `viewport_runtime_open_viewport_fails_closed_when_platform_viewport_windows_unsupported`, and
  `viewport_runtime_tear_off_fails_closed_when_platform_viewport_windows_unsupported`.
- Captured native cross-window routing has its own capability gate.
  `PlatformViewportCapabilities::window_hit_stack` means the backend can classify native top-level
  coverage at one physical point through the first opaque terminal, preserve full registered
  `WindowId` values, and attach coherent client geometry to each registered hit. Windows and the
  test platform currently implement that contract. Other backends return `Unavailable`; that
  permits only a provable current source-local target and never cross-window fallthrough, desktop
  inference, or last-preview reuse. Automated owners include
  `native_captured_mouse_up_is_locked_before_user_cleanup_and_delivered_post_borrow`,
  `runtime_native_captured_foreign_surface_projects_rejection_without_delivery`,
  `runtime_native_captured_target_close_clears_preview_without_retiring_source_route`, and the
  `hit_stack_stabilization_*` Windows tests.
- Coordinate facts are explicit runtime state. `DockViewportCoordinateStatusRecord` reports whether
  each registered viewport is using shared global-screen bounds or receiver-local window bounds, and
  the runtime panel exposes that generation next to the route selection source. Mixed-DPI and
  display-ambiguous backends should fail closed or degrade to local-only routing until the platform
  backend can publish stronger facts. Automated owners: `host_viewport_route_tests` and
  `viewport_lifecycle_record_reports_window_local_coordinate_status`.
- Docking policy scenes, divider hit maps, floating-model deltas, and local drop facts remain in
  absolute layout coordinates. Captured cross-window routing starts with one immutable
  `PlatformNativePointerPhysicalFrame`; only after the point-scoped hit stack selects a registered
  target may Dock convert through that entry's `PlatformWindowPhysicalGeometry`. Tear-off placement
  derives from the same callback frame and source geometry rather than mixing drag-start window
  bounds with a release-time point. The viewport snapshot retains GPUI's opaque committed
  `ElementGeometry` for host-local conversion; a transform-only frame advances route facts so a
  proof captured under old displayed geometry cannot authorize a later release.
- Viewport creation and reused-window placement are capability-gated platform operations.
  `Window::window_capabilities()` is the sole profile: its `creation` half reports first-appearance,
  typed-owner, and first-presentation ordering support, while its `mutations` half reports
  placement, pointer input, coherent activation policy, alpha, topmost, taskbar visibility, and
  coordinate space. There is no Dock-local flag capability mirror. Runtime diagnostics resolve
  `App::window_profile` for every registered viewport, preserving its actual `WindowKind` and
  target-display-resolved profile instead of projecting the backend's normal-window or
  primary-display answer. Readiness uses the requested viewport display before opening, so X11
  alpha support reflects that screen's transparent visual; an unavailable saved display id falls
  back to the current default before readiness, projection, and window creation. A queued request
  is not an applied native fact. Initial presentation records whether submission occurs before
  visibility, after visibility, or establishes visibility as part of the first buffer commit;
  Wayland uses the third state rather than claiming either impossible ordering.
  Terminal observations retain the typed request and committed facts, and terminal failures are
  not retried each frame until the target or relevant committed facts change, including reused
  windows that remain pending or retain an adjusted/rejected/unsupported terminal barrier. The
  native runtime panel reports per-window capability profiles, last request/dispatch, and terminal
  observations separately. Detached viewport options request no activation on first appearance,
  retain normal lifetime activation and click focus, accept pointer input, and bind a typed current
  surface owner when an exact anchor handle is available. Reusing that window never replays the
  one-shot appearance request as programmatic activation.
  Owning-platform CI also proves the native boundary: Windows compares creation facts with
  independent Win32 readback, exercises each advertised live domain, and injects a partial
  pointer- and activation-policy failures to verify native rollback and terminal rejection. macOS,
  X11, and Wayland
  test kind-specific creation projections that are shared with their production constructors,
  including Wayland's XDG-versus-LayerShell split. A backend may only promote a property to `Live`
  when its owning runner covers dispatch, native failure, and terminal observation.
  Automated owners:
  `host_viewport_platform_capability_tests`,
  `reused_viewport_does_not_repeat_pending_or_terminal_window_mutations`,
  `runtime_status_projects_each_viewports_actual_window_kind_profile`,
  `viewport_runtime_syncs_supported_options_when_reusing_window`, and
  `empty_central_passthrough_syncs_window_pointer_input`.
- Preview proof is semantic rather than pixel-perfect. `DockPreviewVisualDescriptor` records the
  allowed/rejected decision, active layer, active zone, tab insertion descriptor, payload tab
  previews, and route-preview marker shape, while debug selectors continue to anchor rendered
  dogfood checks. Presentation, overlay, transition, zoom/focus, divider hit map, and accessibility
  descriptors are covered by focused tests. The native runtime panel exposes preview capability as
  `preview proof: presentation-scene+real-content-reveal+overlay-motion+tab-insertion+retargeting+splitter-motion+zoom-focus+divider-hit-map+corner-drag+a11y+route-cleanup+reduced-motion`
  and motion runtime capability as
  `motion proof: shared-runtime+run-state+scalar-value+scalar-sample+explicit-models+policy-gates+layout-projection+projection-clips+sampled-progress+retargeted-identity+reduced-motion-final-state+high-frequency-bypass`.
  The transition executor currently productizes sampled pane, divider, visual-affordance, zoom, and
  focus motion on top of explicit timeline or spring scalar models.
  Overlay-scene-to-transition conversion for tab insertion, payload ghosts, route markers, and
  rejected state is descriptor proof, not an every-frame drag-preview animation guarantee.
  Automated owners: `host_presentation_scene_tests`, `host_viewport_preview_visual_tests`,
  `host_transition_tests`, `host_zoom_focus_tests`, `host_divider_hit_map_tests`, and
  `host_accessibility_tests`. Transparent payload-window rendering, platform accessibility mapping,
  and screenshot or pixel-regression baselines remain explicitly deferred follow-up work.
- Routed overlay and terminal cleanup are fail-closed. Source-window route markers and
  target-window previews are separately renderable. Release locks an immutable candidate before
  source listeners run, then validates its scene lineage instead of trusting cached preview state
  or re-running hit testing after deferred redraw. A current frame may replace the frozen renderer
  token only when its complete native-routing content is identical. Each route is bound to its source
  generation, ingress sequence, runtime session, surface lease, host binding, and scene
  registration. Terminal claim removes that exact route before any commit effect;
  source close or surface shutdown retires it synchronously, target close removes only that target's
  preview, and panic cleanup still admits the next generation. Runtime diagnostics record the
  terminal route or typed unavailable/rejected outcome instead of retaining an unrelated prior
  success. Automated owners: `host_viewport_preview_tests`, `host_transition_tests`,
  `runtime_native_captured_source_close_retires_route_and_foreign_preview`,
  `native_captured_release_does_not_retarget_to_a_scene_created_by_mouse_up_listener`,
  `runtime_native_captured_resolver_panic_retires_g1_and_accepts_g2`, and
  `native_captured_desktop_release_matrix_tears_off_payloads_from_source_mouse_up`.
- Test ownership is split by concern. Route, lifecycle, placement, close, preview, platform
  capability, and visual-proof assertions live in focused `host_viewport_*_tests` modules; the old
  monolithic runtime test files have been deleted. Rendered native dogfood tests remain
  end-to-end integration coverage.
- `DockViewportRuntimeHandle` remains the application-facing facade. Platform sync and pointer-input
  requests now live behind `viewport_platform_sync`, window effects live behind
  `viewport_runtime_effects`, route/scene/close handle methods are split into
  `viewport_runtime_handle::{route_ops,scene_ops,close_ops}`, and coordinate facts live with the
  viewport registry/status model. New tests should target those owning modules instead of adding
  crate-private pass-throughs to the handle.

Before publishing a crate, confirm that the packaged archive carries the expected attribution files:

```sh
cargo package -p open-gpui --list --allow-dirty
```

For the canvas crate specifically, run:

```sh
cargo package -p open-gpui-canvas --list --allow-dirty
cargo publish -p open-gpui-canvas --dry-run --allow-dirty
```

Every published Open GPUI crate should include `README.md`, `LICENSE-APACHE`, and `NOTICE`. Cargo
does not package files outside a crate root through `include`, so each publishable crate root keeps
its own `NOTICE` copy.

The import-boundary scan rejects dependency files that reintroduce Zed's GPL tracing stack
(`ztracing`, `ztracing_macro`, `zlog`), the old `zed-sum-tree` dependency, the Zed monorepo as a
Cargo git dependency, retired Zed Git fork sources that have already been migrated, or the removed
Zed `perf` crate dependency. The retired `zed-scap` package and `zed-industries/scap` Git source
are also rejected now that screen capture resolves through the Open GPUI-owned
`open-gpui-scap` fork. The old crates.io `zed-font-kit` package is retired and should not be
reintroduced; font-kit resolves through the Open GPUI-owned fork configured in the crate manifests.
