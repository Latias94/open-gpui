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

The command checks that every workspace package declares the workspace MSRV, that the declared MSRV is at least the maximum `rust-version` in the resolved dependency graph, that duplicate registry crate versions are explicitly allowlisted, and that `cargo audit --json` reports no unignored vulnerabilities. The current MSRV is Rust 1.92 because the Linux platform dependency chain reaches `oo7 0.6.0`; `wgpu 30`, `naga 30`, `resvg 0.46`, and `usvg 0.46` currently require Rust 1.87. The dependency health workflow runs this command on Linux, and the release workflow requires a successful `dependency-health.yml` run for the release commit before publishing.

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

The remaining warning-class advisories are not denied by the local audit gate: `paste 1.0.15` comes
from `image`'s AVIF codec chain, and `ttf-parser 0.25.1` comes through the font/SVG stack. Removing
the first would drop AVIF image support; replacing the second requires a renderer/font-stack
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
cargo bench -p open-gpui-canvas --bench large_canvas
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
