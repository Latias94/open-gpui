---
type: Current State
title: Open GPUI maintained-framework development state
status: active
timestamp: 2026-11-06T04:41:15+08:00
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
  Reconcile native selection after adding weights/styles to an already matched family through
  its existing font records; the missing-family installation proof does not cover that workflow.
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
