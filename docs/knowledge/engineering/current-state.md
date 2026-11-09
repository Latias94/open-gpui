---
type: Current State
title: Open GPUI maintained-framework development state
status: active
timestamp: 2026-11-10T04:57:30+08:00
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

- Windows desktop clipboard normalization is implemented and qualified at
  `a7e47b311aa6a721c5dd0991c36893694db13043`. The official NUL/metadata regression fails before
  the correction; focused/native 3, complete Windows 244, public clipboard 7 and ordinary
  independent clipboard 5 pass. The complete typed Mac pasteboard source is prepared at
  `fe5bb458f2ef23c0def7f8e19aa2327e404504b4`, retaining six official and six added tests.
  Its native compilation/execution remains pending: the actual foreign-host Darwin attempt stops
  earlier at missing generated media bindings. Host checks cannot qualify cfg-excluded Mac code.
  Independent review finds no confirmed defect. Three maintained locks add only existing bitflags
  edges; locked workspace/consumer checks, Windows correctness Clippy/Rust 1.93, independent wasm,
  formatting and existing static gates pass. All three threads close with no owned helper left.
  The [record](../../verification.md#desktop-clipboard-normalization-and-owned-pasteboard)
  preserves the distinct platform scopes. Continue the credential successor and Mac native gates;
  fixed-target closure, numerical budgets and the real engine seam still precede maintenance handoff.

- The complete asynchronous clipboard/Web paste successor is implemented and qualified at
  `5209f4e82c7cef3c095807fcf13cd16fe860b5c6`, after `770463485ac9`. Focused 34, complete framework/
  public/Gallery 2,028, components/docking/Canvas 2,985, complete Windows 243, ordinary consumers
  96/69 and disabled Core/public clipboard 1,379 pass. Actual Windows ready-snapshot/text/
  Submitted/reconstruction and real browser permission/text/image/trusted-paste workflows execute.
  Fixed handler identities, accepted fences and per-delivery PNG digests close review gaps;
  original fixture compilation/effect-flush/navigation failures remain in the
  [record](../../verification.md#asynchronous-clipboard-and-web-paste). Locked checks, correctness
  Clippy, actual Windows Rust 1.93, production wasm profiler off/on and nightly shared memory,
  formatting and existing static/YAML gates pass. No lock or compiler requirement changes.
  All nine threads close and no owned helper remains. Desktop normalization/pasteboard ownership,
  remaining fixed-target differences and a real Nara seam consumer still precede maintenance handoff.

- The complete official randomized-tree/platform-tree successor is implemented and qualified at
  `b42295d0e535e4105470e80873a6ad7832e81863`, after `f6f3d98d156`. Focused Core/benchmark/
  AppCell/ingress and tree/presentation/
  keyboard/atlas passes 181/181 (`3bad5b99-e03a-4112-9294-283ce6c1624b`). The unchanged cached
  mixed workload exposes a redundant fork wake feedback loop; tracked App-borrow/convergence
  custody corrects ordinary wake creation, with finite animation red-to-green evidence.
  Complete dev/stacker and release/non-stacker platform smokes pass 115/115 each; the independently
  locked benchmark passes 64/64 after restoring activation between reference comparisons.
  The initial 14/17 run reproduces refused-Hidden mouse-state loss; Div now publishes interaction
  membership through its existing accepted-frame transaction/discard owner. Two scale/effect-flush
  fixture corrections preserve all assertions. Production/platform compilation passes, including
  seventeen groups/five families. Official proptest 1.12 and path-expression support replace the
  incompatible 1.10 pin; value/property consumers pass 5/5 and 2/2 at their recorded checkpoints.
  Complete framework/public/Gallery passes 2,014 and components/docking/Canvas 2,974. Final
  eight-node/Wide native tree and animation pass 2/2 after correcting the geometric sampler's
  transparent-border/raster-snapping assumptions, retaining geometry-first pixel selection.
  Ordinary enabled/disabled consumers pass 91/67, additive API/property consumers 6 and disabled
  Core/public 1,383 pass. Two initial Windows aggregates remain 241/242 with nine skips; explicit
  keyboard-only and pure-redraw pointer isolation preserve all assertions. Final Windows passes
  242/242 with nine skips. Separate `1756e8f79aa`, `515e4d77c16` and `36e2567b16c` own wake and
  fixture corrections. Locked workspace/independent checks, correctness Clippy, actual Windows
  Rust 1.93 framework/three-consumer checks, production wasm profiling off/on and nightly shared
  memory, exact bench-only feature graphs, formatting and existing static/YAML gates pass.
  Final independent smoke also verifies the exact Activated ticket. All eighteen implementation,
  review and next-unit analysis threads close. No owned Cargo/native helper remains active.
  ADR 0022 records why tree visibility binds to the existing cross-channel presentation scope.
  The [record](../../verification.md#randomized-trees-and-platform-tree-workloads) retains source,
  actual failures and final qualification. Async clipboard/Web paste, remaining fixed-target
  dispositions, numeric budgets, canonical docking and the real Nara consumer remain open.
- The official Send foreground/Web Fetch successor is implemented at
  `ff4b3f4b95d48410c7c1d3820e2b2a98444aacab`. Send handles retain the originating scheduler,
  dispatch and counter owners. Fetch uses the official bounded main-thread response pump,
  cancellation and explicit credentials; stored JS Window/unsafe Send+Sync are removed.
  Actual worker-stream timeouts identify the missing official std queue prerequisite, now
  committed at `72676731163`. Final-source HTTP passes eight 32,768-chunk responses, both
  configured modes, six body-drop cases and waitAsync fallback. Queue stress passes 8/16
  workers, 524,288 items/128 rounds per mode and every asynchronous join. Exact async-task and
  wasm_thread sources are recorded separately from that owning queue correction.
  The complete runtime/public cohort passes 2,233/2,235 with nine declared skips; both failures
  refuse native window creation during display-topology changes. Those unchanged cases and
  both Send native workflows subsequently pass 4/4, including exact Submitted pixels and the
  ignored GPU trajectory. The original aggregate remains failed. Ordinary independently
  locked consumers pass 83/67 with profiler on/off, disabled Core/public Send 1,351 and UI
  core/components/docking/Canvas 2,974 pass. Locked Rust 1.95 checks, expanded correctness
  Clippy, actual Windows Rust 1.93 framework/three-consumer checks, stable production wasm
  profiler off/on, wasm Clippy and nightly shared-memory/profiler production compilation pass.
  Stable browser smoke passes actual WebGL/WebGPU/Auto,
  both PNG sizes and original interaction/fallback/refusal controls after disk/cache failures.
  Source and locks are preserved. The [verification record](../../verification.md#sendable-foreground-submission-and-web-fetch)
  owns failures, exact runs and remaining scope. All ten threads and both local fixture servers
  are closed; full refresh and the real engine consumer still precede maintenance handoff.
- The complete official system-notification successor is implemented at
  `26639a9715aff09ffbc61b92af45014728f02f94`, after `b14967342f2`. Core identity/show/dismiss/response,
  TestPlatform observations, native Windows/Linux/macOS transports and the ordinary example
  retain fixed-target behavior. Responses reuse ordered App updates and generation-aware
  handler replacement; initialization stays lazy. Windows transport retirement uses existing
  native finalization. A real helper borrow failure is reproduced and corrected without changing
  recoverable versus terminal shutdown. Corrected Core/public 12, Windows 11 including actual
  Submitted-pixel/retirement workflow, complete framework/public/Gallery 2,194, ordinary
  independent enabled 77 and disabled notification 3 pass. Disabled Core/public notification
  paths also pass 12. The [record](../../verification.md#system-notifications-and-ordered-app-responses)
  retains exact failures, source limits and synthetic notification activation scope. All eight
  implementation/review/next-unit-analysis lanes close. Three lock graphs add only the required
  notification closure, changing no existing versions. Locked checks, expanded correctness
  Clippy, actual Windows Rust 1.93 framework/consumer checks, stable production wasm profiling
  off/on, existing nightly shared-memory and independently locked wasm smoke compilation,
  formatting and existing import/API/document/release/YAML checks pass. Actual OS notification
  delivery, foreign native execution, numeric costs, complete refresh and Nara integration remain open.
- The official stateful Spring successor is implemented at
  `ecbbfa361ae4489b2de4121c5f0ca8b2a1d7aebb`, after `af682e37f36`. The existing motion
  crate owns one pure official solver;
  GPUI reexports its exact types and owns typed targets/playback/Element-ID lifecycle. Duplicate
  domain physics is removed with explicit options, epsilon and separate policy hints. Numeric
  common-scaling regressions justify canonical f64 intermediates; refused-candidate trajectories
  justify Spring-local publication through the existing accepted-frame owner. Translated
  settling/cancellation failures justify representable endpoints and final position accumulation.
  Complete framework/components 2,171/2,974 pass at their recorded checkpoint; final motion 94,
  affected public/domain 43, actual Windows Submitted/pixels/input/retirement 1, disabled
  Core/public 1,340 and independent Spring 6/6 in both profiler selections pass. Earlier broader
  ordinary consumers pass 74/58. App reduced motion finishes active domain transitions without
  rewriting preferences.
  The [record](../../verification.md#official-spring-and-shared-domain-sampling) preserves actual
  red runs, source checkpoints and fixture corrections. Locked checks, expanded correctness
  Clippy, actual Windows Rust 1.93 framework/consumer checks, production wasm profiler off/on,
  existing nightly shared-memory compilation, doctests, formatting and static gates pass.
  All thirteen current lanes close without remaining scoped P1/P2; no complete refresh,
  numeric cost or engine adoption follows.
  A separate bounded source pass records five further Core mechanisms after notifications:
  gesture/axis locking, randomized element trees, sendable foreground submission, async clipboard
  and atomic credential insertion/Apple errors. These are queued source evidence, not parity.
- The official non-Spring animation successor is implemented at
  `4ed461f2b591d6e44d84abc742094740fbd607af`, after
  `968f836f48a097ef49e692c62f5d1734c657bb75`. Generic controls,
  executor/shared-phase clocks, animated parent composition and image reduced motion preserve
  the fixed target's complete mechanism. A reproduced first-mount candidate refusal requires
  a weak exact-state timer; App-borrow admission and invalid FPS intervals use existing owners.
  Focused 26, actual native Submitted/pixels/retirement 1, complete framework/public/Gallery
  2,138 with seven declared skips and components/docking/Canvas 2,966 pass. Final strengthened
  image cases pass 3, independent enabled/disabled consumers pass 68/52 and profiling-disabled
  Core/public animation passes 1,310. Locked checks, correctness Clippy, actual Windows Rust 1.93
  framework/independent checks, stable production wasm profiling off/on, existing nightly
  shared-memory compilation, formatting and static gates pass. The
  [record](../../verification.md#generic-animation-controls-and-clock) preserves initial failures,
  source provenance and the exact qualification scope.
  All six implementation/review/analysis lanes close without remaining P1/P2. The Spring
  successor's plan retains complete official UI state/playback and one shared pure solver,
  with explicit fork policy/geometry migration rather than duplicate physics. The owner's
  later separate maintenance session follows the fixed-target refresh and real modular
  engine-integration assessment; immutable consumed revisions and consumer evidence connect
  the two repositories. Full refresh, native timing/foreign execution, measured costs,
  canonical docking and Nara adoption remain open.
- The complete official frame-time debug overlay is implemented at
  `73472ca230bac117f8cb417d0cd6b0c1364c980e`, after
  `7cd732eda7f8c5c80e0d08b737504f720f6fa6f5`. Complete mode/history/bitmap recipes and all six
  official tests reuse the existing candidate/clip owner with accepted-draw-only sampling.
  The separate fork TestPlatform resource-retirement correction is
  `d928c5459a8ed174d1a6835abd096b3e7a996b5a`. Focused owners pass 20; complete Core/WGPU/
  Windows library/public consumers pass 1,970 with five skips, including the separately executed
  new overlay case. Final actual native overlay/profiler/signal/power/visibility passes 5/5,
  components/docking/Canvas passes 2,966, disabled Core/public passes 1,350, ordinary enabled/
  disabled consumers pass 67/48 and additive consumers pass 4. The historical foreground case
  passes unchanged this run without proving its earlier variability resolved. Locked checks,
  correctness Clippy, actual Windows Rust 1.93 framework/both-consumer checks, stable production
  wasm profiling off/on, existing nightly shared-memory compilation, formatting and static gates
  pass. All six implementation/review/inventory threads close. The
  [record](../../verification.md#frame-time-debug-overlay) preserves actual failures, corrected
  native frame observation, resource provenance and strict-readback timing limits.
  A bounded source pass identifies official animation scheduling, stateful spring integration
  and system notifications as remaining Core mechanisms; these are queued source evidence,
  not implementations or an exhaustive inventory. Numeric costs, foreign overlay execution,
  full refresh, canonical docking and Nara GPU/View/Editor adoption remain open.
- The complete WGPU dependency/shader/WebGL successor is implemented at
  `38d3041acab90cc01cbc9984a8b738a8b2c3a194`, using official WGPU/Naga
  `cddb2f5d80dc0009b2ef6767635140cacf90a007` and an explicitly qualified Windows Rust 1.93
  floor. Registry Vulkan controls reproduce both semaphore assertion/cleanup aborts; the
  restored fixed source passes unchanged. The fork's 40-byte exact-clip surface uniform is
  corrected to 48 bytes without changing the shared 24-byte clip ABI. Complete WGPU/public
  123, actual native Vulkan/renderer 3, components/docking/Canvas 2,966 and independent
  enabled/disabled 65/48 pass. Final actual Edge WebGPU/WebGL/Auto fallback pass both PNG
  sizes, 22 fixed samples each, text, resize and original input/capture/docking/activation.
  Forced backend and software-policy failures remain explicit. Locked checks, native/wasm
  correctness Clippy, actual minimum-compiler framework/three-consumer checks, production
  wasm profiling off/on, existing nightly shared-memory compilation, four-target graphs,
  formatting and static gates pass. Both bounded reviews close, all six threads close and
  the [record](../../verification.md#wgpu-dependency-shaders-and-webgl) retains actual runs,
  corrections and scopes. Foreign execution, physical loss, general DPI, numeric costs,
  existing native foreground restoration, full refresh and engine adoption remain open.
- The official image dependency policy is implemented at
  `b4355a36824f0709f1b73f7480ec88a884a489bc`. Fourteen formats and Rayon remain enabled;
  the default AVIF encoder is excluded, and ordinary applications can restore it through
  their own image dependency. Three-platform before/after and downstream graphs confirm
  the exact active feature boundary. Final Core/public/native image consumers pass 25/25,
  and independent encoding off/on passes 3/3 and 4/4. Locked workspace/consumer compilation,
  correctness Clippy, actual Windows Rust 1.92 framework/consumer checks and production wasm
  pass. The inherited stale smoke lock is separately refreshed at
  `015863c085dbcd845658cfb97c289c3ac4433aad` and passes its own locked wasm check.
  Root/ordinary formatting and source review pass; all three threads close.
  The [qualification](../../verification.md#image-dependency-feature-policy) retains
  source provenance, decoder/guessing limitations and actual execution scope. Full refresh,
  native foreground restoration, foreign execution, numerical costs and engine adoption
  remain open.
- Shared atlas/renderer integration is implemented at
  `67e1360709ca48be74bf986f116b9c004ff5bd59`, from
  `480430f02aecc3bdcf78bed8126ef6b871974770` against the unchanged fixed Zed target.
  Official owned insertion, shared AtlasState/AtlasBackend and shared HeadlessAtlas replace
  duplicate key ownership; WGPU adopts instance preupload and separate cached texture
  bindings. Exact instance leases and complete-frame refusal remain explicit fork contracts.
  Core/public checkpoints pass 1,516/1,516 and the ordinary independently locked disabled
  consumer passes 48/48. DirectX upload/admission passes 14/14; the corrected existing
  two-window native asset trajectory and three WARP renderer controls pass 4/4. WGPU's
  scale-corrected GPU/headless selection passes 21/21 after the recorded 16/21 failure.
  Real pixels, binding reuse and exact resource/refusal assertions remain intact.
  The complete resumed framework/public/Gallery selection passes 1,934/1,935, preserving
  four interactive skips; the previously recorded foreground-restoration gate still fails.
  A red GPU regression exposes the local view-only receipt/region-recycling integration
  defect; the existing complete-set lease now protects the prepared regions. Complete WGPU
  library/public passes 110/110, the strengthened CI owner/renderer selection passes 21/21,
  components/docking/Canvas passes 2,966/2,966, and independent enabled/disabled modes pass
  63/63 and 48/48. Locked workspace/both-consumer compilation, correctness Clippy, actual
  Windows Rust 1.92 framework/consumer checks, stable production wasm profiling off/on,
  existing nightly shared-memory wasm, four formatting and existing static gates pass.
  Complete Scene recording is independently committed at `186deca5a46`, with fourteen
  owning regressions and bounded source review.
  Metal has bounded source review, with thirteen native tests
  still unexecuted on macOS. Remaining native foreground, foreign execution, replacement-Device/
  fractional-DPI/subpixel scope, full refresh, measured costs and engine adoption stay open. The
  [qualification](../../verification.md#shared-atlas-ownership-and-retained-renderer-bindings)
  preserves failures and exact scopes.
- Official conditional native hot reload is implemented at
  `ad70ac9230892ca4e4d47840b93215596b8c60e0` through existing App/View owners.
  The independently locked real compiler/patch consumer changes typed cached, startup-stored
  erased and stateful component render bodies while retaining runtime-mutated state, identities
  and initialization counts. Final DX11 generation 3-to-4 pixels and normal retirement/quit/exit
  pass, with one 3,419.7 ms observation. All five rejection/disconnect controls pass an unchanged
  rerun; the preceding unexplained extra-refresh failure remains recorded. Successful execution
  uses an identified upstream dx x64 code-generation backport, separate from GPUI provenance.
  A reproduced fork-only duplicate terminal diagnostic has an independently reviewed AppCell
  correction `28eebf0e8dcd0c8a71f1064bf9f66052b5d3a55c`; three owner tests and ordinary
  parallel libtest pass. Final affected lifecycle/Windows runs 310/313, then two of the three
  failures pass unchanged. One foreground-restoration
  gate still fails with an external game foreground; no final complete all-green native run is
  claimed. Workspace all-target compilation, affected correctness Clippy, hot-reload-disabled
  owner/public 68, independent enabled/disabled 62/47 and actual Windows Rust 1.92 checks pass.
  Feature-selected non-debug Core excludes the hot-reload cfg; stable production wasm compiles
  with profiling off/on. All four workspace formatting checks and existing import/API/document/
  release scans pass; all thirteen implementation/review threads are closed. See the
  [qualification](../../verification.md#conditional-native-hot-reload) for actual scope.
- The official additive test-support successor is implemented at
  `c6ffb32bd1499c9e8fb46783a3d1a4de626707a0`, after
  `7c7a1411009a07afbcbd70340256adf55161491b`. Runtime queries follow each executor, while
  build-time environment/feature selection independently controls entity-handle tracking.
  Complete util passes 116/116 after the old downstream home-directory regression fails.
  Two reproduced inherited release-diagnostic defects have owning corrections; the default
  and explicitly tracked Windows View trajectories pass actual mixed-context identity, pixels
  and exact release. Focused Core/scheduler/public 130 and the complete independent mode matrix
  pass. Complete framework runs 1,917/1,918; the sole existing mouse-leave fixture failure has
  separately reviewed convergence/observation correction `2daf8412170540f7e2e586687bc25d5eadf4f2fa`.
  Selected native 2/2 and complete Windows 224/224 then pass, preserving four interactive skips.
  The initial extra-event source remains unproven. Components/docking/Canvas pass 2,966/2,966
  with tracking enabled. Ordinary enabled/disabled 62/47, locked framework/both-consumer checks,
  correctness Clippy, actual Windows Rust 1.92, production native/wasm and all three format
  checks pass. Existing static gates pass. All seven implementation/review threads are closed.
  Conditional hot reload follows in the separate successor above. Numeric costs, full refresh
  and engine adoption remain open.
- The complete benchmark context/measurement successor is implemented at
  `5f3a99fad9284c17a8eaac508ab8b4f47018be44`, after
  `916c7cb2d2fffcaa95199598655b5cef97fb9bda`. It retains the official independent measurement
  package, parameterized macros, real-time threaded contexts and fallible optional headless
  renderers. Corrected metrics/macros pass 28/28; focused context/public/actual WGPU execution
  passes 60/60. The independently locked benchmark executes all nine cases, and the maintained
  Canvas benchmark passes five smoke cases. Both benchmark feature graphs exclude Core test support
  and leak detection. Complete runtime/public/Gallery/native/metrics/macros passes 2,040/2,040
  with five documented skips; components/docking/Canvas passes 2,964/2,964 plus both separately
  feature-gated Docking controls, preserving the prior 2,966-test set. Disabled affected Core/public
  passes 112/112; independent enabled/disabled passes 62/47. Locked checks, correctness Clippy,
  actual Windows Rust 1.92, stable/nightly production wasm and both formatting checks pass. Existing owning records
  distinguish reproduced inherited corrections from fork candidate/submission and terminal-quit
  adaptations. This does not complete the fixed-target refresh or select an engine UI default.
  The additive test-support/leak-selection mechanism is qualified by the separate successor
  above; excluding test instrumentation from benchmarks alone did not implement it.
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

- The unified-profiler foundation at `3e5a907775e9f22d0a092a0f9f27c6f1696ade0e` adopts
  official feature gating, safe thread-owner collection
  and the common WindowProfiler before the queued journal/hang/refresh-aware chain. It removes
  InputLatencyTracker and its feature name, preserves concrete submission/callsite diagnostics,
  and migrates the ordinary consumer and real Criterion Canvas workflow. Fork adaptations keep
  input tied to the accepted draw that covers it, including synchronous keyboard draws, refused
  candidates, exact scene retries, reset and unwind. The existing Window rollback now handles
  interrupted Render/prepaint/paint, and accepted-publication failures finish Window completion
  before propagating the first panic. Trace lifecycle serialization and corrected slow-history
  ranking repair source-confirmed inherited defects; the verification guide owns executed red
  baselines and distinct scopes. Corrected focused 37, full runtime/public/gallery 1,426 and
  UI/component/docking/Canvas 2,966 pass, with four existing interactive Windows skips in the
  full runtime cohort. Disabled Core 1,095 and independent enabled 20/disabled 18 pass, with
  final scoped feature checks against the latest source. The real 20,000-node/19,900-edge Canvas
  benchmark passes both feature configurations after replacing inherited quadratic fixture
  preparation with complete public snapshot construction. No runtime mutation or numeric cost
  improvement follows from that setup correction. All ten implementation/review subthreads
  are closed. Locked workspace/independent checks, correctness Clippy, formatting and actual
  Windows Rust 1.92.0 affected-framework/Canvas/consumer checks pass. Production wasm compilation
  is separate from browser execution and the unchanged Web test-support dependency gap.

- The sleep/wake and presentation-visibility successor is qualified at
  `8b283cb151b070d7d5851a531a86e6d6d67196cb`. Official multi-subscriber App power and
  Window/Context visibility observations replace the singleton wake path. All backend sources
  adopt mandatory platform methods; fork adaptations retain native cross-thread/post-borrow
  ordering, exact retirement and SetWindowPos sampling. Public test-context consumers exercise
  the supported simulation APIs. Complete runtime/public/gallery/native passes 1,446/1,446,
  with four preexisting interactive Windows skips; UI/component/docking/Canvas passes
  2,966/2,966. Disabled Core/public 1,113 and independent enabled/disabled 28/26 pass without
  skips. The native trajectory executes real HWND reveal/hide/show/minimize/restore, requires a
  newer nonempty Submitted receipt and retires exact callbacks/platform resources. Synthetic
  power messages do not suspend the machine. The verification guide retains compile/fixture
  corrections, the headless Hidden planning correction and source-only foreign backend scope.
  Locked checks, correctness Clippy, both formatting checks and actual Windows Rust 1.92.0
  affected-framework/Canvas/consumer checks pass. Production wasm compiles with profiling off
  and on; browser/test-support, native macOS/Linux and numeric costs stay unqualified. All nine
  implementation/review subthreads are closed. Journal/hang/monitor, frame-signal, refresh-aware
  and power/cadence owners remain queued.

- The foreground journal/detector/monitor successor is qualified at
  `8493e9eb1cad7c3336fa99b4b8749c5be1d6c7c5`, through official checkpoint `d284cbbc87`.
  All 45 original tests remain, alongside public consumers, writer model, scoped unwind and
  exact accepted/Submitted controls. Fork corrections preserve canceled demand, failed submission
  work, queued/unqueued task custody and late closed-queue refusal settlement. Public monitor
  configuration rejects invalid intervals; shutdown Flush waits for a finalized-stream poll and
  callback within existing bounded quit work. Complete runtime/public/gallery/native passes
  1,531/1,531, with four preexisting interactive Windows skips. Components/docking/Canvas
  passes 2,966/2,966; disabled Core/public passes 1,114/1,114; independent enabled/disabled
  passes 32/32 and 26/26 without skips. Actual native Submitted, two collector cursors, visible
  reconstruction and monitor off-thread Flush/callback release execute. Locked checks,
  correctness Clippy, both formatting checks and actual Windows Rust 1.92.0 framework/Canvas/
  consumer checks pass. Stable wasm profiling on/off compiles; optional multithreaded wasm uses
  the existing nightly/shared-memory configuration and also compiles. The verification record
  retains initial failures, real red regressions, exact corrections and ordinary-versus-nightly
  scope. All ten implementation/review subthreads are closed. Browser/native foreign-platform
  execution, physical suspend, numeric costs, accepted-but-unpolled backlog, pending publisher
  tail drain, full fixed-target refresh, docking U4-U9 and engine adoption remain open.

- The platform-signal/completed-skip successor is qualified locally at
  `5811543bebe10657b9f99b42bfadb214e7dad999` against official `cd4fc8de4c` and `36b6d0951f`.
  The final Core/Windows library/public cohort passes 1,482/1,482 with four existing interactive
  skips. The earlier WGPU/gallery selection contributes 228 passing cases and one fixture skip;
  the verification guide retains its initial clipboard failure, unchanged isolated pass and later
  complete Windows success, plus the interrupted first final run. Components/docking/Canvas
  pass 2,966/2,966; disabled Core/public 1,122/1,122 and independent enabled/disabled 38/38 and
  29/29 pass without skips. Locked checks, correctness Clippy, formatting, actual Windows Rust
  1.92.0 and stable wasm on/off plus existing nightly shared-memory compilation pass.
  The source audit identifies inherited official demand-driven prerequisite `eb354c8d50` and X11
  direct Expose/recovery work; these have a complete owning successor before cross-platform
  refresh-aware measurement. Signal capture does not approve a permanent rejection of those
  official mechanisms. All eight implementation/review subthreads are closed. Native foreign
  platforms, browser execution, numerical costs and complete fixed-target refresh remain open;
  no engine default changes.

- The demand-driven platform-loop successor is qualified locally at
  `b6506b48efca022c13256ef17d5ecfa26787b2ae`, after `c7fb5ccd8a8`.
  Official `417d03305a` weak wakeups/Web RAF parking, `eb354c8d50` Wayland scheduling and X11
  Expose/recovery now use existing owners. The completion-driven hook is removed; Wayland uses
  the existing native completion command after App borrow release. A source-confirmed accepted-
  publication unwind gap has an owning retry/completion correction and real scheduled-callback
  regressions. Corrected focused Core/public passes 62/62, actual Web/Wayland/pointer state
  modules pass 27/27 on the host and the affected native selection passes 5/5. Complete runtime/
  public/gallery/state/native runs 1,749 cases, with 1,748 passing and one native image-animation
  timeout; its unchanged isolated rerun passes 1/1, with the cause undiagnosed. Five preexisting
  skips remain explicit. Components/docking/Canvas pass 2,966/2,966, independently locked enabled/
  disabled consumers pass 40/40 and 31/31, and disabled Core/public passes 1,134/1,134, without
  skips. Locked workspace/consumer checks, correctness Clippy, formatting, actual Windows Rust
  1.92.0 and stable/nightly production wasm compilation pass. Linux/browser state tests do not
  qualify native execution or numerical costs. All nine implementation/review subthreads close.
  The verification guide preserves initial fixture failures, interrupted compilation, independent
  reviews and source limits. No engine default or full-refresh completion changes.

- Refresh-aware measurement v3 is qualified locally at
  `411895475e9e51a9f06954d5d676333d597b991f`, after `9901ed0bcc6272a647bccfb73016f3f22206a013`.
  Official `4f6d97b9de` and relevant cleanup
  supply signal/dirty-to-submission timing, refresh groups, view self-time and the three hang
  rules. Exact candidate/Submitted authority remains intact. Executed red controls expose four
  inherited sealer/active-use defects; bounded attribution, deferred sealer start and existing
  input-lifecycle controls correct completion order, long-open attribution and inactive gaps.
  The final lifecycle-focused Core/public cohort passes 208/208 and both expanded native
  profiler/signal trajectories pass 2/2. A former full build is stopped before test execution
  after the last defect is confirmed. The corrected complete framework/public/Gallery/state/
  native cohort passes 1,809/1,809 with five preexisting skips, including the native image-animation
  trajectory without an isolated retry. Components/docking/Canvas passes 2,966/2,966, independent
  enabled/disabled consumers pass 46/46 and 32/32, and disabled Core/public passes 1,134/1,134,
  without skips. Locked workspace/consumer checks, correctness Clippy, formatting, actual Windows
  Rust 1.92.0 and stable/nightly production wasm configurations pass. Existing import/API/document/
  release scans pass. All eight implementation/review threads close. The verification guide
  preserves earlier failures and scopes. No dependency or engine-default
  changes follow. Publisher-tail/backlog retirement, numeric costs and native foreign-platform/
  browser execution remain separate. The then-queued official generic View/ViewElement API has
  its implemented successor and scoped qualification below.

- The power/cadence successor is implemented locally at
  `2ceb09aa4590388b8e2d73ac1406caddc5b6c470`, adopting official `fdcb198110`, `2580a00264`,
  `7c8dbadcb5` and macOS `0373366f43` through existing Platform/App, Window and backend owners.
  Longest enabled limits cover dirty, pending-presentation, callback and high-rate-input demand;
  idle callbacks consume no cadence slot. Exact mandatory recovery and deferred force survive.
  Corrected focused Core/public passes 76/76; actual production Linux profile/Metal state passes
  3/3 on the host. The final focused Windows selection passes 11/11, including public power
  observations, LowPower-to-Submitted recovery, signal custody and exact native retirement.
  Native execution exposes and independently diagnoses an inherited fork detached-retry Rc leak;
  HWND-bound retries replace that transport within the existing coordinator. Its ordinary-
  maintenance, admission-fallback and stale-token controls pass with exact weak-owner release.
  All fourteen implementation/review threads close. Complete framework qualification executes
  1,850 cases: 1,849 pass and a reentrant IME fixture fails; explicit unlimited window options
  correct its timing premise. Windows reruns each pass 220/221, exposing first an uncontrolled
  foreground setup, then an unchanged activation-terminal timeout. The corrected first-show
  setup and all 14 focused power/retirement/IME/activation cases pass. The activation timeout's
  cause remains unproven; no production source change follows. Image/activation failure diagnostics
  are committed at `1a6d90bdaac5256c36064be9824459de74ea91bf`. The final Windows selection
  passes 220/221, with a different foreground-preparation failure; all new power/retirement and
  both diagnostic consumers pass. Its final bounded image/activation recheck passes 3/3. The
  complete Windows gate remains open, including the earlier capture timeout and activation
  failures. Components/docking/Canvas 2,966, independent enabled/disabled 51/36 and disabled
  Core/public 1,162 pass without skips. Locked workspace/consumer checks, correctness Clippy,
  root/independent formatting, actual Windows Rust 1.92.0 and stable/nightly production wasm
  checks pass. No clean complete Windows run is inferred from the focused rechecks. The verification
  guide retains earlier failures and corrections. macOS Metal/CV/process-info and Linux D-Bus
  remain native source-only, distinct from portable tests; physical timing/energy costs remain
  unmeasured. No engine default or complete fixed-target-refresh claim follows.

- The generic View/component successor is implemented locally at
  `652122a4e118ff2d213ae0d9392c71f49c44553b`, starting at
  `a89a7200c80fc40eb073dbad37789f17153ec7b8` against the unchanged fixed Zed target.
  Official View/ViewElement replaces Component and direct AnyView Element embedding; Render
  and RenderOnce remain public. Shared phase helpers preserve existing cache/candidate/resource
  admission, subscriptions and scoped profiler unwind. Focused owners pass 117/117, new public
  consumers 5/5 and native composition/cache/profiler/signal trajectories 4/4. The complete
  framework/public/Gallery/native cohort passes 1,856/1,856 with five existing skips; affected
  components/docking/Canvas passes 2,966/2,966; profiler-disabled Core/public passes 1,167/1,167;
  independently locked consumers pass 53/53 and 38/38. Whole-workspace all-target compilation,
  correctness Clippy, formatting, static scans, affected Rust 1.92 and stable/nightly production
  wasm checks pass. Final review reproduces and corrects explicit conversion source locations;
  public workflows pass 5/5 in each profiler configuration and the native composition passes 1/1
  after this metadata/test correction, followed by affected Rust 1.92, Clippy and formatting.
  Earlier complete cohorts retain their pre-correction scope. All seven threads close.
  The complete accessibility synthetic-child/diagnostic successor
  is source-mapped to official `83d4847462` and `2268045a11`, with accepted-publication and ADR
  0023 redaction reconciliation still queued. Conditional hot reload and complete fixed-target
  refresh remain separate.

- The framework query/List successor is implemented locally at
  `e58b8b7687aadc91408657b4e550a1b7d723c0a9`, starting at
  `18e5f46e87f919049628cd9fd1be782c0fbaf49a`. Complete official measured TextLayout queries,
  Rems helpers, List upward normalization and utility/shell/path/key framework slices are
  implemented with public and independently locked consumers and scoped qualification.
  Three List red controls reproduce negative offsets before the owning correction. The first
  focused Core/public run passes 51/52, exposing a new fixture's incorrect one-frame recovery
  premise; the corrected focused run passes 54/54 with exact G+1/G+2/G+3 facts and zero-height/
  private atlas revision controls. Complete Core/public passes 1,390/1,390 and utility 115/115.
  The utility prerequisite is committed at `787ab55a8f68daab1dc48e2c80d12f4109028bdb`.
  The actual measured-text workflow passes 1/1 after correcting an unsupported cross-frame raw
  Arc identity premise; observed unchanged/reflow shaping is 0/3 with the official cache policy
  preserved. Complete WGPU passes 89/89, components/docking/Canvas 2,966/2,966, disabled
  Core/public 1,181/1,181, independent enabled/disabled 56/56 and 41/41, and affected Windows
  native five pass. Locked workspace/consumer checks, correctness Clippy, actual Windows Rust
  1.92.0, stable/nightly production wasm, formatting and existing static scans pass. All seven
  threads close; no owned Cargo remains active. See the guide for run IDs, failures and limits.
  No engine default, numerical-cost budget or complete fixed-target refresh follows.

- The synthetic-accessibility/explicit-inspection successor is implemented locally at
  `751e563eb89eeb373dac41cf734bfe372b81d0e4`. Official Element/Div/builder and complete raw
  inspection use accepted publication with frame/activation metadata; automatic artifacts retain
  ADR0023 redaction. Executed red controls precede deferred-parent rollback, own-clip validity,
  irreversible-quiescence and reserved-root corrections. Final Core/public 1,412, components/
  docking/Canvas 2,966, WGPU/affected native 96, disabled Core/public 1,203 and Gallery 13 pass.
  Independent type-only checks pass 2/2 in each feature mode; the non-debug serializer slice
  passes six, without claiming a release-profile whole App run. Locked workspace/consumer checks,
  correctness Clippy, actual Windows Rust 1.92, production wasm, formatting and static scans pass.
  All eight threads close; no owned Cargo remains active. The candidate queue separately retains
  official active-descendant focus, keyboard continuity, tab-group ordering, missing diagnostics/
  properties, initial-tree delivery and enabled/author-ID differences. Full refresh, AT/foreign
  native/browser execution, numerical costs, hot reload, docking and Nara adoption remain open.

- The composite-focus/keyboard successor is implemented locally at
  `53decdc64a2d182fad1fbf00ababcd2b55a906e7`, after
  `156d0b8c1835108223bcf9fdd1f7d7c39a396013`, against the unchanged fixed target.
  Complete official descendant focus, keyboard activation, Tab-group and missing-node mechanisms
  reuse existing final-tree, focus and accepted-publication owners. Concrete local rollback and
  refused-arm regressions precede the fixes. Focused owner/public 104, profiler-disabled affected
  111, complete Core/public/WGPU/Windows 1,770 and UI core/components/docking/Canvas 2,966 pass;
  four existing interactive Windows cases remain skipped. Actual native composite callback,
  Submitted and exact release evidence stays distinct from OS/AT consumption. Seven threads are
  closed. Independently locked type-only controls pass 4/4 in each feature mode, and the actual
  Gallery selection passes 13/13. Locked workspace/consumer checks, affected correctness Clippy,
  actual Windows Rust 1.92 and stable production wasm compilation in both profiling modes pass.
  Root/independent formatting, existing static scans and workflow YAML parsing pass. No owned
  Cargo remains active; no remote publication occurs.
  The next coherent owner completes remaining property/configuration/initial-tree mechanisms,
  followed by benchmark/report and hot reload. Full refresh, numerical budgets, canonical docking
  U4-U9 and Nara renderer/View adoption remain open.

- The accessibility property/configuration/initial-tree successor is implemented locally at
  `d9608b1a3174f17df270d6b92894539ec32654a8`, after
  `37ceac021fec7efb5091334bc60877ba36ab75d6`, against the unchanged fixed target.
  Official shortcut/numeric-step/author-ID builders, enabled/active queries, test configuration
  and synchronous creation-title roots reuse the existing property, A11y and ordered-ingress
  owners. Shared original admission and native-close facts protect retained Send callbacks;
  the existing sequencer owns the shutdown fence. An executed native regression corrected
  premature generic admission revocation while preserving exact queued WindowClosed settlement.
  Corrected complete Core/public/WGPU/Windows passes 1,787/1,787 with four existing interactive
  skips. An earlier unchanged mouse-leave extra-event failure remains undiagnosed despite its
  passing isolated and complete reruns. Six implementation/test/review threads are closed.
  Complete UI core/components/docking/Canvas passes 2,966/2,966 and profiler-disabled affected
  Core/public passes 182/182. Independently locked ordinary type-only controls pass 6/6 in each
  feature mode; the actual Gallery selection passes 13/13. Locked workspace/consumer checks,
  affected correctness Clippy, actual Windows Rust 1.92, stable production wasm in both profiling
  modes, root/independent formatting and existing static scans pass. Workflow YAML parsing passes.
  Initial roots, accepted full trees, native Submitted frames and real AT consumption retain
  separate evidence requirements. One additional read-only benchmark audit is closed; its source
  findings queue the complete harness/report prerequisites without runtime qualification. Full
  refresh, hot reload, numerical budgets, canonical docking U4-U9 and Nara adoption remain open.
  No owned Cargo remains active and no remote publication occurs.

- The complete benchmark owner's threaded-dispatcher/queue-closure prerequisite is implemented
  locally at `018fbdc5f708d91f0ae16a7fb43ac04280ebb680`, after
  `1670c373e34751ac520237b7d04a480b06ab7b4a`. The complete official
  concrete dispatcher uses existing platform/executor/queue/profiler owners; fork changes retain
  admission results, scoped timing and thread-affine refusal. The original blocked-receiver test
  fails before the official closure fix. Corrected queue/dispatcher/executor/profiler and actual
  public consumers pass 182/182 after an untyped-channel correction and reviewed fixture-race
  fixes. Complete Core/public/WGPU/Windows 1,810, components/docking/Canvas 2,966, disabled
  affected 35 and independent enabled/disabled 62/47 pass; four original interactive skips remain.
  Locked checks, affected correctness Clippy, actual Windows Rust 1.92, stable production wasm
  in both instrumentation modes and existing nightly shared-memory compilation pass. Root/
  independent formatting, existing static scans and workflow YAML parsing pass. All five
  implementation/review threads are closed. Linux source is reviewed but active Linux
  compilation/runtime remains unqualified. Current exposure uses test-support;
  benchmark-feature separation, context replacement, real text/render injection, report and
  numerical budgets remain in the complete successor. No owned Cargo remains active and no
  remote publication occurs. Full refresh, hot reload, docking and Nara adoption remain open.

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

- Continue the [credential storage and add-only Keychain successor](../../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#credential-storage-and-add-only-keychain-successor),
  preserving complete official status, cancellation, backend I/O and ownership behavior. The
  framework add-only API is macOS-only; do not invent a cross-platform atomic fallback.
  Keep the [Mac pasteboard native gates](../../verification.md#desktop-clipboard-normalization-and-owned-pasteboard)
  open until actual Apple compilation and all twelve tests execute. Full fixed-target closure
  and the real Nara consumer still precede framework-maintenance handoff.
- Continue after qualified
  [system notifications](../../verification.md#system-notifications-and-ordered-app-responses),
  [Spring/motion](../../verification.md#official-spring-and-shared-domain-sampling) and the
  complete Send/Fetch successor. Preserve official regressions and ordinary independent
  consumption, keeping Core/native and browser execution scope separate. Actual OS notification
  delivery remains unqualified. Continue the
  [bounded residual inventory](../../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#bounded-core-residual-inventory)
  through complete official mechanisms; it is not an exhaustive semantic audit.
- Continue remaining renderer/API/platform reconciliation after the complete
  [WGPU dependency/shader/WebGL qualification](../../verification.md#wgpu-dependency-shaders-and-webgl)
  and [image feature-policy qualification](../../verification.md#image-dependency-feature-policy).
  Preserve ordinary downstream extension, retained formats, the declared compiler floor
  and independently locked consumers through existing owners.
  Keep historical native foreground-restoration failures and the current unchanged pass visible;
  its real stateful edit/build/apply/output consumer has passed. Preserve ordinary/release feature exclusion,
  the declared compiler floor and the distinct GPUI/fork/external-driver source provenance.
  Successful body patching does not qualify engine adoption.
- Apply the maintenance skill's official-baseline rule through existing owners. Preserve
  qualified fork admission, completed-action observation, source/geometry validity, exact native
  leases, accepted cache publication and App/post-borrow command authority while reconciling
  the remaining mechanisms. Keep the recorded notification and publisher/backlog limits visible;
  avoid replacing the fork or reapplying completed units.
- Full fixed-target refresh, historical native focus variability, numerical budgets and remaining
  renderer/API/platform differences stay open. Qualify prepared CoreText selection/cascade
  on macOS, other native/browser environments and touchpad scroll/inertia/pinch/response timing
  on their actual platforms. Existing getters, HWND/IME regressions and source inspection do not
  supply that execution or performance evidence. DirectWrite existing-family and installed-
  fallback results retain their separate recorded scope.
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
