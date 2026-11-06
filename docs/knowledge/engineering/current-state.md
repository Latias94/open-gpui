---
type: Current State
title: Open GPUI maintained-framework development state
status: active
timestamp: 2026-11-06T18:52:00+08:00
git_branch: optimize/docking-graph-transaction
related_plan:
  - ../../plans/2026-08-21-1349-refactor-docking-kernel-parity-plan.md
  - ../../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md
  - ../../plans/2026-07-09-002-refactor-v030-product-surface-hardening-plan.md
  - ../../plans/2026-07-09-003-refactor-devtools-runtime-ecosystem-plan.md
  - ../../plans/2026-07-08-003-refactor-runtime-docking-core-hardening-plan.md
verified_by:
  - docs/knowledge/engineering/verification/2026-07-09-devtools-inspector-click-dogfood.md
  - docs/knowledge/engineering/verification/2026-07-09-devtools-runtime-ecosystem-provider-controller.md
  - docs/verification.md
---

# Current State

- Authoritative development workspace: `F:/SourceCodes/Rust/open-gpui`, on
  `optimize/docking-graph-transaction`. The inspected starting revision was
  `89cb76ba1d59904d5c670f770aa8e30fe0346178`, with a clean initial worktree.
- The older Nara `repo-ref/open-gpui` snapshot is 63 commits behind that baseline. It is reference
  evidence, not the development workstream.
- Canonical docking remains in progress under the August 21 plan. New prepared close/floating
  operations do not establish the U4 production cutover; the legacy DockWorkspace/graph path still
  exists. U4-U9 and native multiview acceptance remain open.
- Maintenance now includes an independently locked framework/platform/component/docking consumer,
  compatible dependency patch choices and replacement of withdrawn `spin` 0.10.0. Its README and
  the verification guide define the compile-only scope.
- The new upstream comparison baseline is Zed `635026fabc38440fce5e1767d125026a17b12012`, observed
  on November 5. The arena successor now has separate allocation, nested cleanup, App-bound clear
  tokens and post-borrow Windows frame maintenance. The verification guide records 2,984 library
  tests, 162 Windows cases (four interactive cases skipped), both allocator Miri models and the
  outstanding DirectManipulation timing comparison. The TextSystem successor now pools independent
  width caches, preserving reuse without retaining the owning text system. Its 841 Core cases,
  598 component integration cases and final 164 Windows cases pass; the verification guide retains
  an initial activation timeout and unchanged successful reruns. Public headless/native
  reconstruction also exposed and corrected stale WinRT settings factories after OLE apartment
  recreation (`432d079675`). The cached-dependency successor now isolates nested draw/callback
  reads, retains paint/deferred subscriptions and fixes deferred journal positions and inherited
  opacity. Final Core 873, affected UI/component/docking/Canvas 2,964 and selected Windows 165
  pass; four existing interactive cases remain skipped. The native cache matrix proves actual
  submitted frames and exact retirement. The verification guide retains the initial failures,
  activation-fixture corrections and notification/performance limits. The text-layout successor
  now preserves native tabs, single-baseline Unicode separators, default/explicit font features
  and wrapped hit-test error semantics. Its final Core 877, affected consumers 2,964 and Windows
  173 pass, with the same four interactive cases skipped. A new native text trajectory checks
  full source, actual shape/paint, a notified newer Submitted frame and exact release. The
  native-resource successor now validates atlas sources before allocation and destinations
  before upload, preserving both insertion APIs and exact leased regions. Color staging reads
  release their map on return/error/unwind, and blended glyph targets begin transparent. Its
  verification record owns the WARP/debug-layer and actual emoji pixel evidence. The
  pending-input successor now shares recognized modifier interception/observation and singleton
  timeout, settles accepted focus/blur/drop transitions once, and preserves refused-candidate
  state and deadlines. Active pending/replay checks retain the original focus/context, including
  callback changes, while completed live-window actions retain their observation. Final Core
  901/901 and affected consumers 2,964/2,964 pass, including 24 focused scenarios. The native
  two-handler consumer passes production callbacks, leased IME preference, WM_CHAR text and
  exact HWND/platform teardown.
  Final Windows 187/187 passes with four existing interactive cases skipped. A cache-fixture
  correction isolates replay from observed native-hover refresh and establishes one exact
  initialized submission before its baseline; original cache/model assertions remain. The
  verification guide owns the earlier failures, actual commands and limits. These five
  initial units do not complete the text/layout/API/platform refresh or engine adaptation.
- The GPU-record/color-transfer successor now uses fully initialized PaddedBool32 fields and
  direct shared-record DirectX upload, preserving clip/tile/transform ABI. Windows transfer
  directions are corrected under the existing gamma-2.2 approximation. Safe field-width and
  actual old-shader pixel regressions precede the fixes. Complete Core/WGPU 941, affected
  consumers 2,964 and Windows 188 pass; four existing interactive Windows cases remain skipped.
  The expanded native trajectory checks gradients, image flags/opacity, decorations, transformed
  rounded clips, exact cached/deferred replay and native/platform release. Both debug runtime
  shaders and non-debug FXC embedded bytes execute the pixel oracle. The latter uses unoptimized
  Rust with FXC `/O3`, not a Rust performance qualification. Draw-time cache mode is bound to
  the accepted generation, preserving accessibility/inspector bypass. The verification guide
  owns exact runs, scale 1.5 evidence and remaining macOS/header/Metal, WGPU scene-pixel,
  Linux/browser, native accessibility/mixed-DPI and measured-cost limits.
- The font-installation successor now invalidates descriptor, line-layout and character-width
  state after successful installation. Current/previous caches share one generation authority;
  stale ranges, delayed wrapped results and rejected hash candidates retain their qualified
  recovery rules. Public test/headless accessors share the App's exact TextSystem. App owns
  bounded, replaceable missing-grapheme delivery and waits for borrow release before dispatch.
  Cosmic preserves loaded face identity and fallback-only fonts, with retained source storage.
  Final Core/WGPU 999, affected consumers 2,964 and Windows 189 pass; four existing interactive
  Windows cases remain skipped. The verification guide owns the original failures, real Noto
  fallback/software raster, new-family native installation and exact release evidence. Native
  existing-family reselection, Windows/CoreText missing reporting, the later Web Canvas layer,
  other platforms, compiler-floor execution and measured costs remain open.
- A game-engine Editor is the selecting consumer. Framework maintenance and rendering-adapter
  refactors are allowed; Nara document, ECS and history types do not enter the core framework.
- The native-font successor now reselects already matched DirectWrite weights and named private
  fallbacks after publication. Unchanged faces retain their loaded IDs; old simulated faces
  remain valid, including through an unknown native callback. Native identity-cache entries
  retain the faces they index. The original/corrected red baselines, intermediate 194/195
  result, final focused 9/9 and complete Windows 197/197 are recorded in the verification
  guide; four existing interactive cases remain skipped. CoreText source and deterministic
  shaping/raster regressions are prepared; macOS compilation and execution remain unqualified.
  This correction addresses gaps also present in the fixed Zed target.
- The Cosmic paragraph successor now shapes B-class segments independently, preserving complete
  source/glyph geometry and FontRun configuration with a monotone private cursor. Missing indices
  aggregate against the original source so CRLF stays one report. Actual licensed Hebrew bases/
  marks raster at two scales. The public headless shape/paint consumer exposes and retains a
  final-glyph closest-hit regression; the owning Core comparison is corrected. The verification
  guide owns all red/intermediate/final results and the automatic-fallback, platform and cost limits.
  Final all-feature Core/WGPU/Windows plus public paragraph consumer passes 1,207/1,207:
  Core 943, WGPU 66, public consumer 1 and Windows 197, with four existing interactive cases
  skipped. The actual Cosmic paragraph consumer remains headless/software raster evidence.
  Affected UI core/components/docking/Canvas passes 2,964/2,964 across 25 binaries with no skips.

- The Unicode wrapping successor reconciles UAX line
  opportunities, explicit continuation indentation, complete source/visual graphemes and public
  Middle truncation while preserving font generations and candidate/cache custody. Start/End
  share exact wrapper/paragraph probes; public style and unclamped-shaping controls retain text,
  run coverage, reflow and final release. The verification guide owns the reproduced baseline and
  review defects, the expanded native Submitted trajectory and the final reruns. Compiler-floor
  execution exposed an existing unstable slice API in TypeIdHasher; independent correction
  `8d35827da9a0bd335c3e85201ee82dbd77135224` passes both hasher tests under Rust 1.92.0.
  Stable profiler hints and explicit Windows button-message dispatch also remove unsupported
  source constructs. Affected framework and independent-consumer all-target/all-feature checks
  now pass on actual Rust 1.92.0 for Windows. Final all-feature Core/WGPU/Windows plus both
  public text consumers pass 1,250/1,250 with four existing interactive Windows cases skipped.
  Affected UI core/components/docking/Canvas pass 2,964/2,964 with no skips; the independently
  locked public API consumer passes 3/3. The verification record retains earlier review failures
  and the MSVC/PDB disk-space retry. Locked workspace/independent compilation, correctness
  Clippy, dependency-policy tests and explicitly rerun formatting checks pass. Complete refresh,
  other platforms, engine integration and measured costs remain open.

- The font-work successor is qualified after Unicode commit `b466037e87e`. Explicit public
  prewarming shares normal Cosmic primary/fallback attributes, while available-name suggestions
  reflect only backend-reported families. Measured glyph images have a one-use handoff bounded to
  256 entries and 8 MiB of image-buffer capacity; eviction regenerates normally. The two inventory
  regressions fail on baseline. Cold exact bytes, actual generation/match identity, capacity
  pressure and old-face retention pass. Additional native alias preparation exposes an existing
  DirectWrite callback identity defect; separate correction `19a4ed3434a` preserves the first
  physical face mapping without merging input descriptors. Final all-feature Core/WGPU/Windows
  plus public text trajectories pass 1,268/1,268 with four existing interactive Windows cases
  skipped; affected UI core/components/docking/Canvas pass 2,964/2,964 with no skips. Independently
  locked public fonts/wrapping tests pass 5/5. Locked workspace and independent-consumer checks
  pass on Rust 1.95.0, as do affected correctness Clippy and both formatting checks. Actual Rust
  1.92.0 affected Core/WGPU/Windows and independently locked consumer all-target/all-feature
  checks pass on Windows. Existing import/document/release/API scans pass. The verification
  record retains the red/compile corrections, independent reviews and exact native/software
  scopes. Other platforms, complete fallback/script support, engine integration and measured
  frame/process costs remain open.

- The text fragment/layer/underline successor is qualified after `1d148d66dfd`. Raw
  layouts own source-index glyph partition; the forward cursor preserves existing shaping and
  explicitly separates ordered traversal from reordered scans. The old RTL underflow and an
  advance-only fragment/opaque-cover ordering defect are reproduced before correction. TextLayout
  releases its borrow before painting shared measured snapshots; backgrounds/highlights/foregrounds
  have public split passes with actual aligned/wrapped geometry. Underline callbacks expose source
  ranges/resolved styles; Window bounds/exclusions share existing snapped projection and exact
  ancestor clips. The corrected focused suite passes 19/19, including actual emitted primitives,
  Cosmic public paint/reconstruction and native source-fragment/underline Submitted/retirement.
  The verification record retains the half-open-boundary and floating-intersection fixture
  corrections. Final Core/WGPU/Windows plus three public text trajectories pass 1,281/1,281
  with four existing interactive Windows cases skipped; affected components/docking/Canvas pass
  2,964/2,964 and independent fonts/wrapping/layer API tests pass 6/6 without skips. Locked
  workspace/consumer compilation, correctness Clippy, both formatting checks and existing
  import/document/release/API scans pass on Rust 1.95.0. Actual Rust 1.92.0 affected-framework and
  independent-consumer all-target/all-feature checks pass on Windows. Both bounded independent
  reviews leave no outstanding confirmed P1/P2. This is not independent bidi reflow, general
  caret support, full ink-overhang qualification, a numeric cost result or complete upstream parity.

- The window-text ownership successor adopts official Rc window handles, RefCell cache state,
  final-target Cell scratch and deletion of unused hash APIs. Global TextSystem and immutable
  layouts retain Arc ownership. Fork generation coherence, actual refused-candidate recovery
  and borrow-free backend reentry/publication remain explicit local adaptations. Independent
  review reproduces a wrapped-key clamp omission present in the starting fork and official
  target; the existing content key now includes max_lines. The notify consumer now observes
  automatic accepted successors without compensating refresh/draw. Corrected Core/WGPU/Windows
  plus four public text consumers pass 1,292/1,292 with four existing interactive Windows cases
  skipped. Affected components/docking/Canvas pass 2,964/2,964 and independently locked public
  text tests pass 7/7 without skips. Both bounded reviews close their findings. Locked
  workspace/consumer compilation and affected correctness Clippy pass on Rust 1.95.0; actual
  Windows Rust 1.92.0 framework/consumer all-target/all-feature checks pass. The verification
  guide owns exact scope, final checks and red history; numeric costs and other platforms
  remain unqualified. Both formatting checks and existing import/document/release/API scans pass.

- The Taffy/text/root/grid successor now adopts official 0.15 leaf/alignment APIs, truncated
  measurement isolation, actual shaped-overflow checking, MaxContent list probes, auto window/
  prompt roots and row-sizing APIs. Stack growth is opt-in across both owners. A reproduced
  ordinary-wrap cache defect shared with the official target now requires exact wrap-width
  equality. One portal fixture now verifies the authored absolute root inset and existing
  local-transform result rather than assuming a zero root origin. Corrected Core/WGPU/Windows
  plus five public trajectories pass 1,312/1,312, with four existing interactive Windows cases
  skipped; the stacker-disabled owner/portal cohort passes 33/33. Native bundled-font kerning,
  actual Submitted successors and exact reconstruction/release retain their source scope.
  Affected components/docking/Canvas pass 2,964/2,964 and independently locked public text/grid
  tests pass 9/9 without skips. A full-height multi-Sidebar test now declares visible grid
  allocation while retaining physical-input and identity isolation. Independent reviews close
  production, native-oracle, portal and Sidebar-fixture findings. Locked workspace/consumer
  compilation and correctness Clippy pass on Rust 1.95.0; actual Windows Rust 1.92.0 affected
  framework/consumer all-target/all-feature checks also pass. The verification guide retains
  initial failures and scope. No general word-based MinContent, numeric cost improvement,
  other native platform or engine integration follows. Both formatting checks and existing
  import/document/release/API scans pass.

- The shared image successor adopts official EXIF orientation and final-target shared
  static/GIF/WebP decoders. The removed duplicate paths were inherited upstream code, not a
  fork-specific mechanism. Encoded Image and resource Img retain successful animation frames,
  delays, transparent canvas and contextual decode errors. Fork adaptations declare the
  source-confirmed image 0.25.4 API minimum without changing either 0.25.10 lock entry, and
  align Img with the existing executor clock. Review reproduces and fixes the exact loading
  deadline; the asymmetric orientation oracle distinguishes all eight transforms. Public
  consumers isolate each loader's completion notification, deterministic 99/100 ms timelines
  and static idle callbacks. Actual Windows two-source Img submits 0/1/0, binds GPU readback
  to exact generations, replaces EXIF sources and reconstructs/releases all owners. Both
  bounded independent reviews close confirmed findings; all six subthreads are closed.
  Complete Core/WGPU/Windows and six public consumer binaries pass 1,328/1,328 with four
  existing interactive Windows cases skipped. Affected components/docking/Canvas pass
  2,964/2,964 and independently locked public consumers pass 12/12 without skips. Locked
  workspace/consumer checks, affected correctness Clippy, both formatting checks and existing
  import/document/release/API scans pass on Rust 1.95.0. Actual Windows Rust 1.92.0 framework/
  independent-consumer all-target/all-feature checks also pass. The verification guide retains
  red regressions and fixture corrections. Numeric costs, other platforms, image dependency
  features, external-loop/API/renderer-lifetime reconciliation, canonical docking and engine
  adoption remain open.

- The externally driven App successor adopts official run_embedded/ApplicationHandle, using
  the existing shared launch path and strong/weak ownership. A reproduced 0/2 direct-callback
  baseline justifies the fork's complete update transaction for defer-only effects and nested
  batching. Corrected owner tests pass 10/10; the actual Windows visual-host selection passes
  11/11. Returning Test/VisualTest platforms support the launch lifecycle without changing
  stock Windows' blocking process loop. Four native model stages bind accepted/nonempty
  Submitted and offscreen captures, with notification/defer settled before polling, background
  await, a visual timer and exact observed native/App/text/platform release. Delayed-launch
  callback custody, nonterminal public shutdown and weak AsyncApp limits remain explicit.
  Both bounded independent reviews find no confirmed P1/P2; all five subthreads are closed.
  Complete Core/WGPU/Windows and six public text/layout/image consumers pass 1,339/1,339,
  with four existing interactive Windows cases skipped. Affected components/docking/Canvas
  pass 2,964/2,964 and independently locked consumers pass 14/14 without skips. Locked
  workspace/consumer checks, correctness Clippy and actual Windows Rust 1.92.0 affected-framework/
  consumer all-target/all-feature checks pass, together with root/consumer formatting and
  existing import/document/release/API scans. No production
  external Windows loop, shared GPU, full renderer release, OS-input/timing or performance
  claim follows. The queue now separately records display observation, conditional hot reload
  and the target's owning asset-load migration; they are not implied by this entry point.

- The owning asset-load successor adopts official CachedLoad, foreground publication and
  deduplicated interested views. App fetch returns Option output; opaque image entries replace
  Shared handles and per-use completion tasks across Window, retain-all and the actual gallery
  LRU. The direct official publication reproduces a fork-specific busy-App panic; waiting on
  existing AppCell releases before the complete App transaction preserves entry identity and
  cancellation. Owner/image/LRU/candidate/native focused cases pass 22/22 after recorded
  fixture/oracle corrections. Actual native consumers submit both windows automatically from
  one load, retain passive no-redraw assertions and release the image before App shutdown.
  The previously dormant gallery now has an explicit platform example with actual capacity-one/
  two cancellation and two-window animated-disposal tests. Bounded production/consumer reviews
  leave no confirmed P1/P2; all six subthreads are closed. Complete affected runtime/public/
  gallery passes 1,361/1,361 with four existing interactive Windows cases skipped; components/
  docking/Canvas pass 2,964/2,964 and independently locked public consumers pass 16/16 without
  skips. Locked workspace/consumer checks and affected correctness Clippy pass on Rust 1.95.0;
  actual Windows Rust 1.92.0 affected-framework/consumer all-target/all-feature checks also pass.
  The independent lock and every dependency version remain unchanged; root lock changes only
  declare existing gallery dev dependencies. The verification record owns
  exact results and the separate native initialization, explicit candidate retry and platform/
  performance limits. Root/consumer formatting and existing import/document/release/API scans
  pass, as does the maintenance skill validator. That skill's local `bf3e0d2f7c6` clarification
  makes official mechanisms the default baseline and requires evidence for retained differences.

- The connected-display successor adopts official App/Window observation, refresh intervals
  and both launch initializations after owning-assets `1855a94c5ee`. It reuses the fork's
  immutable topology, committed facts and ordered native ingress. Windows has a hidden top-level
  broadcast recipient with no app windows. The verification guide retains the initial fixture
  failures, official Hz reciprocal-overflow correction and reproduced deferred-close reentry
  defect in the fork. Existing Window close-handler custody now protects queued native queries;
  the geometry fixture establishes its baseline after actual startup delivery. Corrected focused
  follow-up passes 22/22; full runtime/public/gallery passes 1,388/1,388 with four existing
  interactive Windows skips. Components/docking/Canvas pass 2,964/2,964 and independently locked
  public consumers pass 18/18 without skips. The latter exposes and qualifies promoting bitflags
  from optional Wayland activation to the official required Core dependency. Versions and both
  lockfiles remain unchanged. Locked compilation, affected correctness Clippy, formatting and
  actual Windows Rust 1.92.0 framework/consumer all-target/all-feature checks pass. All eight
  implementation/review subthreads are closed. Foreign-backend source, synthetic WM ingress,
  detached association/property fixtures and actual Submitted frames retain their separate
  scopes; physical mode/hotplug changes, native non-Windows execution and numerical costs remain
  unqualified. Production Core/WGPU/Web compile on wasm32-unknown-unknown; Web test-target
  compilation stops in unchanged wait-timeout test-support dependency. The verification record
  retains that gap and the proptest pin comparison. Refresh-aware telemetry remains queued
  rather than implied by the getter.

# Historical Main Integration

The previous snapshot recorded `df6ae2f` on July 9. The following integration summary retains that
historical scope and is not current branch or CI qualification.

- Runtime/docking/core hardening from `docs/plans/2026-07-08-003-refactor-runtime-docking-core-hardening-plan.md` is present on `main`.
- v0.3 public-surface hardening from `docs/plans/2026-07-09-002-refactor-v030-product-surface-hardening-plan.md` is present on `main`.
- DevTools runtime ecosystem work from `docs/plans/2026-07-09-003-refactor-devtools-runtime-ecosystem-plan.md` is present on `main`.
- `open-gpui-devtools` exposes capture providers, scoped event recorders, target/domain/event capture DTOs, `DevtoolsInspectorState`, static `DevtoolsInspector`, and stateful `DevtoolsInspectorController`.
- The UI foundation gallery DevTools page collects through provider-backed capture and has a real click smoke for inspector rows/actions.
- The `CHANGELOG.md` `[Unreleased]` section is accumulating v0.3 user-facing migration notes while workspace crates remain at `0.2.0`.

# Current Entry Points

- Canonical docking plan: `../../plans/2026-08-21-1349-refactor-docking-kernel-parity-plan.md`.
- Upstream successor: `../../imports/zed-gpui-upstream-sync-candidates.md#november-5-maintained-framework-successor`.
- Dependency consumer: `../../../tests/fixtures/dependency-consumer/README.md`.
- DevTools runtime plan: `../../plans/2026-07-09-003-refactor-devtools-runtime-ecosystem-plan.md`.
- DevTools runtime verification: `verification/2026-07-09-devtools-runtime-ecosystem-provider-controller.md`.
- DevTools inspector click dogfood: `verification/2026-07-09-devtools-inspector-click-dogfood.md`.
- v0.3 public surface plan: `../../plans/2026-07-09-002-refactor-v030-product-surface-hardening-plan.md`.
- Verification matrix: `../../verification.md`.

# Next Action

- Continue the coherent framework refresh beyond the five initial correctness units. The candidate
  queue retains font/text/layout/API, image decoding, external-run-loop and platform differences.
  Font prewarming, available-family names and glyph-raster qualification are complete within the
  recorded Windows/software scope, as are source-fragment/layer/underline and window-local
  text ownership. The Taffy/intrinsic/root/grid/stack successor is qualified within its
  recorded scope, together with shared-image decoding and its real public/native consumers.
  The externally driven App, owning asset-load and display-observation units are qualified within
  their recorded scope. Reconcile the official refresh-aware telemetry owner next; display getters
  do not qualify frame pacing. Apply the maintenance skill's official-baseline rule while
  reconciling remaining image-feature/API/platform/hot-reload differences. Qualify the
  prepared CoreText selection/cascade work on macOS when that platform is available. The
  DirectWrite successor now
  has separate existing-family and installed-fallback evidence rather than borrowing the
  missing-family installation proof.
  Preserve the implemented font generations, App-owned reporting, GPU flag representation and
  Windows transfer correction. Qualify the
  remaining compiler/dependency requirements
  before broad engine adaptation; do not overwrite the fork or reapply already-covered fixes.
  Preserve pending-input admission, context/recipient
  validity and completed-action observation, together with the native upload successor's
  source/destination admission and exact-instance leases.
  Preserve the cache successor's admitted read/opacity publication and bounded convergence,
  including the explicit non-view/external-group notification limits. Preserve the arena successor's
  AppCell native delivery, candidate-frame rules and explicit post-borrow command authority. Qualify touchpad
  scroll/inertia/pinch and response timing separately; source inspection or the new HWND/IME
  regression does not establish that input/performance parity.
- Preserve and continue the canonical-docking plan; assess the actual renderer/Host seam through
  the engine consumer. Complete Editor/game-runtime selection remains consumer work rather than
  a premise that two permanent UI engines are required.

# Citations

- [Canonical docking plan](../../plans/2026-08-21-1349-refactor-docking-kernel-parity-plan.md)
- [Upstream sync successor](../../imports/zed-gpui-upstream-sync-candidates.md#november-5-maintained-framework-successor)
- [Independent dependency consumer](../../../tests/fixtures/dependency-consumer/README.md)
- [Runtime/docking/core hardening plan](../../plans/2026-07-08-003-refactor-runtime-docking-core-hardening-plan.md)
- [v0.3 public surface hardening plan](../../plans/2026-07-09-002-refactor-v030-product-surface-hardening-plan.md)
- [DevTools runtime ecosystem plan](../../plans/2026-07-09-003-refactor-devtools-runtime-ecosystem-plan.md)
- [DevTools runtime provider/controller verification](verification/2026-07-09-devtools-runtime-ecosystem-provider-controller.md)
- [DevTools inspector click dogfood verification](verification/2026-07-09-devtools-inspector-click-dogfood.md)
