# Open-GPUI Framework Maintenance Handoff

Prepared November 12, 2026 for the independent Open-GPUI maintenance session. This document owns
handoff navigation and suggested next work. Accepted ADRs remain architecture authority; existing
plans and qualification records retain their exact scope.

The engine development path is recorded separately in Nara:
`docs/plans/2026-11-12-nara-ui-integration-handoff.md`.

## Scope And Next Deliverable

The subsequent [November 13 closure completion](#november-13-framework-closure-completion)
is the current framework-maintenance continuation. Its fixed source ledger and platform
matrix supersede the older recent-only queue and preserve every earlier qualification's
revision scope. Read that continuation before choosing the next consumer pin or task.

Independent framework maintenance can begin from the published baseline and qualified engine
consumer. Completing Nara's Editor or game UI selection is not a prerequisite for that session.
This roadmap covers reusable UI runtime, platform, rendering, control behavior and performance
work. Nara's domain tools and Jellyflow's graph-document interface remain in their own owners.

The first deliverable should address a concrete framework reproduction from those consumers,
or complete public behavior/skin reuse with an ordinary custom control. Preserve ordinary
standalone applications alongside hosted roots. Return the exact revision, migration notes,
changed feature/platform scope and a focused consumer command. A compositor, new package
hierarchy or Signals rewrite needs a measured workload and its own selected design first.

The subsequent [Editor Framework Evolution plan](2026-11-13-1145-refactor-editor-framework-evolution-plan.md)
records the owner's revised development direction: wrappers and focused extensions around official
GPUI, mature control references, evaluation of the shared `dockspace` implementation, and complete
Nara editing pages. Keep the existing state model; broader external-host expansion is deferred.
The owner subsequently authorized breaking changes and implementation of the replacement.
[ADR 0034](../adr/0034-shared-dockspace-session-integration.md) selects the shared semantic session
and supersedes ADR 0033's binary-kernel choice. Dependency, multiview, persistence and native
lifecycle qualification still precede retirement of their incumbent production owners. Current
implementation and executed limits are recorded in the
[November 13 evidence](../knowledge/engineering/verification/editor-framework-evolution-20261113.md).

The current local code checkpoint is `104f5dca`
on `refactor/editor-framework-evolution`; this session has not pushed it. Numeric,
Tree and asynchronous choice workflows, multiline keyboard editing and the shared
docking adapter have local commits and explicit compiler/source receipts. The
Core/Mac and complete old/new docking libraries at `68d52b00` pass 3,626 cases in two
serial runs; 236 affected component/input cases also pass. The final real Mac
smoke creates and submits a child, preserves View identity and local/shared state
through relocation, and retires the exact native group while another workspace
survives. Review findings, corrections and exact run IDs belong in the evidence.

The continuation independently rechecks the full 564-SHA fixed upstream set,
149 root/toolchain entries and actual final iOS/Apple/Tokio/EGL/renderer owners;
no unexplained applicable source omission remains. `6561969d` adds the original
cross-window Submitted/candidate overlap regression; all 46 shared adapter tests
pass. `b51c2163` restores official deferred Mac activation callback timing while
retaining original native and ingress identities. Its final native matrix is
14/14, independent public runtime is 8/8, and the complete Core/Mac run's 2,105
other cases plus corrected focused regression pass. Rust 1.93 and correctness
Clippy remain qualified. `104f5dca` fixes probe cleanup and identity reuse;
final execution leaves zero probe processes or Dock entries. Historical failures
and actual source scopes remain in the evidence rather than being overwritten.

Rust 1.100 is the local development compiler. Core/components remain at Rust
1.93; docking and its selected consumers declare 1.95. Cargo 1.100 intermediate
paths require JSON artifact discovery, and the shared older nextest 0.9.115 has a
known environment-reconstruction warning; the task used an isolated 0.9.151
runner without changing that shared installation. New allocator and DropGuard
APIs do not justify silently raising package floors or replacing typed owners.

Continue with the actual remaining integration boundaries:

1. Run Nara's canonical Dialogue Session/file and complete native trajectory on
   Windows DX12/Vulkan with its exact selected source. Its canonical 1.95 Windows
   SDK compile and 83 library cases pass; Mac Session Open remains unsupported by
   NaraFS's exact guarantees. The local source is not remotely fetchable until
   its objects are supplied or a later publication is authorized.
2. Complete the adapter's independently observed OS capture, native hit-stack,
   stationary pointer and mixed-DPI placement facts on their owning backends.
   The Mac window smoke does not establish those physical input capabilities.
3. After that declared qualification, migrate facade/callers together and remove
   the displaced graph/controller/interaction owners under U7. Retain the one-way
   durable migration and original GPUI physical lifecycle contracts.
4. Attribute C0's actual engine/UI workload before selecting a retained-layout,
   upload or raster experiment. Preserve Entity and the original engine queue;
   there is no selected Signals rewrite or wholesale compositor import.

## Recorded Windows Publication

```text
checkout: F:/SourceCodes/Rust/open-gpui
remote: https://github.com/Latias94/open-gpui.git
branch: main
published code baseline: f75c10b2a9bf7b4dcb2c8c2aa446203997601369
published main including the original handoff: 2cc8ff10304ba905ccf0d664595f3bc93c34b170
previous remote main: 689db5b49b33a5b2e64ba332bff72c98e70092bb
former maintenance branch: optimize/docking-graph-transaction
```

The user explicitly requested merging the maintenance branch into `main` and pushing it. The
merge was a fast-forward, with no conflict or new source delta. Remote `main` was queried after
the push and matches `f75c10b...`. Core/WGPU/Windows library checks pass on that exact source.
The former maintenance branch was also published. This is not new full-platform release
qualification and does not complete canonical docking U4–U9.

Do not assume this one publication authorizes later pushes. Inspect live Git status, instructions
and concurrent work before editing. Keep `repo-ref/` read-only. In the recorded Windows/Nara
environment, coordinate Cargo with that session: one invocation at a time,
`CARGO_BUILD_JOBS=1`, Rust `1.95.0`, shared `F:/SourceCodes/Rust/nara/target`, no clean.
Those paths and that toolchain describe the Windows qualification environment. Reuse the active
checkout's configured target on other hosts. The documented framework compiler floor and
foreign platform claims keep their own verification.

## November 12 macOS Continuation

The subsequent Mac/Apple maintenance runs in `/Users/frankorz/codes/rust/open-gpui` on `main`.
The owner authorizes local fixes and Conventional Commits; this continuation does not publish
them. Its native host is Apple Silicon, macOS 27.0, Rust 1.99.1 and SDK 26.5. Cargo runs serially
with `CARGO_BUILD_JOBS=2`, one nextest test thread and the existing checkout target. A separate
checkout can replace an example executable in that shared target: copy a completed native
probe to a task-owned temporary path immediately after building and run that copy.

The [Mac source reconciliation](../imports/zed-gpui-upstream-sync-candidates.md#november-12-macos-reconciliation)
fixes Zed `78f5d7446c55f6c1aa9b66e8111f6b3f4625ebb0`, includes inherited omissions, and
records retained atlas, candidate, font and explicit native-owner contracts. The original
ports are `e820a593` (fallback font weight/style), `49c4b64b` (text/Div allocation) and
`3691fb44` (window controls, active-display placement, diagnostics, nullable pasteboards and
modal Dialog sheets), with source/qualification records at `bb343e49`.

Consumers using complete `WindowOptions` or `WindowParams` literals must initialize
`app_owns_titlebar_drag`; `false` preserves native titlebar dragging. The existing two-argument
`WindowBounds::centered` remains available; `centered_on_display` adds explicit display selection.
The `debug_state` JSON is diagnostic and has no stable application-data schema.

The [native qualification](../verification.md#macos-source-reconciliation-on-november-12)
records 1806/1806 Core/Apple/Mac library cases, the independently locked consumer, actual Metal
shader compilation and repeated real AppKit sheet runs. Read its combined-worktree and
forced-close limits with the
[follow-up correctness and retirement record](../verification.md#macos-follow-up-correctness-and-native-retirement)
before claiming a committed baseline. Follow-up commits `353971ac` and `158d558d` respectively
restore BOM source indices and complete renderer-before-native-close ordering, including the
previously pending native layer fixes. The complete Apple/Mac libraries then pass 175/175 with
no skips, and real native close/reentrant close, modal sheets and submitted-resize retirement
pass without the former shutdown-protocol diagnostic. The isolated selection-assertion cleanup
is `3d764fa9`, followed by the three related fixtures at `7a412b38`; they preserve their public
text-input, pointer/text and list regressions while enabling stricter Clippy. Darwin PATH and
native errno correction is `9dc411c4`, with its complete 35/35 native process cohort. Final
Core/Apple/Mac/util all-target correctness and suspicious Clippy, independently locked consumer,
formatting and document gates pass. All four continuation workers are finished and closed.

The resulting local code checkpoint is `9dc411c483dba3ac4ac05d4e02fa4b1d1c5d4d91`.
The remote main separately observed during this continuation is
`625173c944fb32555293446405f346db7c8b17cf`; the local tracking ref was
`2cc8ff10304ba905ccf0d664595f3bc93c34b170`. These are distinct observations, not a fetched
or merged reconciliation. This continuation has not pushed its local commits. Inspect the live
remote and coordinate any later publication before choosing a consumer's next source pin.

Physical input, fullscreen pixels, older macOS, actual M5 frame timing, energy and persistent
Keychain behavior retain their separate qualification. A macOS pass does not replace Windows,
Linux, browser, engine-consumer or compiler-floor evidence.

## November 12 Rendering Continuation

The owner subsequently authorizes the six-commit rendering successor, starting at `986dbcbf`
and selecting Zed `6ceb6fee8738274589b59139df27efc191ec2231`. Its
[fixed inventory](../imports/zed-gpui-upstream-sync-candidates.md#november-12-rendering-successor)
owns source provenance, dependencies and implemented dispositions. The rendering qualification
is recorded [separately](../verification.md#november-12-rendering-successor); the earlier Mac
qualification does not establish the new shaders or path representation.

The resulting local implementation/API checkpoint is
`0ed315daa37e94d6f80b357339ab8eff6b9b5afc`. Core/Apple/Mac passes 2077/2077, and the
final Apple/Mac/WGPU package cohort passes 296/296, without skips. Workspace and independently
locked consumer compilation, wasm, precompiled/runtime Metal and strict correctness/suspicious
Clippy pass. Six implementation/review workers are finished and closed. This checkpoint is
committed locally and has not been pushed.

The public path migration follows the official analytic winding implementation:

- Replace `Path<Pixels>` fields and return types with `Path`.
- Build contours with `PathBuilder`, then pass the result to `Window::paint_path`. Direct
  triangulated `Path::new`, `PathVertex`, `PathId` and `PathBuilder::build_path` consumers migrate
  to contours rather than constructing the retired GPU representation.
- Initialize framework `StrokeOptions` fields (`line_width`, `line_cap`, `line_join`,
  `miter_limit`) and `FillOptions::fill_rule`; the old Lyon tessellation option builders are
  no longer the public option types.
- Renderer consumers receive `Primitive::Path(PaintedPath)` and use `tile_count` / `append_to`
  for paint, curve and tile records. Exact clip indices remain frame-local. Keep local paint
  coordinates distinct from projected coverage geometry. `PaintedPath::new` accepts mask bounds
  directly and returns a `Result` for invalid device conversions; the Window paint path records
  these failures through its existing candidate-validity owner.

The successor preserves the original host's WGPU device, encoder, queue and actual submission.
Private depth and path recording resources follow that submission's lifetime. Native Metal
shutdown still requires exact surface quiescence before AppKit terminal close. Do not infer
quantitative performance gains from the official commit descriptions or existing functional
benchmark completion.

External WGPU devices must enable at least four storage buffers per shader stage (including
vertex and fragment limits) and six combined buffers per stage on native targets. The recorder
validates the enabled device rather than the adapter's advertised maximum. Update an explicit
host limit request before constructing `WgpuSceneRenderer`; the shader data now includes path
tiles, curves, paint and exact clip records.

Next, run the existing native close, sheet and submitted-resize retirement controls in an active
macOS GUI session, then run the Windows renderer compiler/shader/WARP gates on Windows. The
current host can execute headless Metal/WGPU pixels but CoreGraphics reports no active displays;
its MSVC cross-check stops in native compression dependencies before DirectX source checking.
After those gates, measure a real consumer's frame/upload/overdraw costs before choosing another
renderer architecture change. Browser execution, Rust 1.93 and older-macOS qualification remain
explicitly separate.

The subsequent [Windows completeness audit](../imports/zed-gpui-upstream-sync-candidates.md#november-12-windows-completeness-audit)
uses the actual fixed target tree and records 12 adopted, 1 equivalent, 1 partial and 11 queued
commits after the historical whole-framework target. The WGPU ordinary-image fast path is
partial; Windows COLR layer bounds, List padding, Cosmic mark offsets and Wayland callback
timing have their own queued successors. Current Windows compiler/shader/GPU results and
the test-only migration corrections are owned by the
[Windows qualification](../verification.md#windows-follow-up-qualification). Read that record
before treating the earlier cross-compilation limit as the current Windows result. AppKit GUI
close/sheet/resize and forced-close contention still need their owning Mac runner.

## November 13 Incremental Sync and Native Follow-up

The [incremental ledger](../imports/zed-gpui-incremental-sync-2026-11-13.md)
continues from local `d1ffee6315e72dd81f3ef690486860680a696e3a` across ten
official commits, fixing the new endpoint at
`27bbbc1cc480a7500cec122f120e796d1fb8500c`. The earlier 564-commit closure
remains complete at its original target. Three newly missing contributions are
adopted; the fork's LinearSrgb specialization follows the official alpha-uniform
compatibility mechanism.

Use the [current execution record](../verification.md#november-13-incremental-sync-and-native-follow-up)
for newer native and browser results. On this macOS 27 runner, bare CLI processes
cannot establish the same AppKit activation state as app bundles launched through
LaunchServices. AppKit also refuses some windowless activation requests; test
explicit activation after ordering a real native window. Keep the exact submitted
frame, callback, ownership and renderer-retirement assertions. A probe assertion
must print its error and exit unsuccessfully before unwinding through AppKit.

The final code checkpoint is `ccd5070dfe502ef413b0259c359e66e69f63b1f8`.
The new ten-row source range is complete. Real execution also fixes WebGPU
non-uniform sprite sampling, macOS CDP editing commands and a mirror snapshot
that could span two accepted input-handler identities. The complete Chromium
workflow exits 0 in 222.274 seconds, including model/DOM selection agreement
through twelve additional field-focus round trips, fonts, clipboard and touch.
The incremental ledger maps every new source and qualification owner to its
full local SHA; original failing attempts remain in the verification record.

An earlier final AppKit attempt stops after CoreGraphics changes from one to
zero active displays. After one display returns, the complete strict matrix
passes **14/14** in 24.949 seconds, with actual normal OS exit 0 for every case
and a newer submitted generation in both resize variants. Build the examples once,
then repeat the required modes through
`python3 scripts/run_macos_native_probe.py target/debug/examples/native_windowing -- sheet`
and the corresponding native-close/layer-shutdown commands. This runner uses
an isolated app bundle and requires the explicit receipt plus the app's actual
normal exit 0. Earlier individual native success markers retain their narrower
scope; the final combined receipt is recorded separately. Windows/Linux native
drivers/input, Safari, physical iOS and numerical
benchmark comparisons still belong to the original named platform runners.

## November 13 Framework Closure Completion

The owner expands maintenance to all applicable GPUI behavior at fixed Zed
`6ceb6fee8738274589b59139df27efc191ec2231`, starting locally from
`a0e5a818e0a3628114e37362bc57a0bc240e66c0`. The
[canonical ledger](../imports/zed-gpui-commit-reconciliation-2026-11-12.md) records
564 unique full upstream commits reconstructed from the content-verified import
anchor, including root/toolchain history, inherited omissions and the later
renderer patches. Each SHA has one row with source evidence and disposition.
Its [platform matrix](../imports/zed-gpui-commit-reconciliation-2026-11-12.md#platform-and-functional-unit-verification-matrix)
maps official groups to local implementation commits, executed gates and exact
other-platform follow-up. A baseline source checkpoint is identified as a
checkpoint, rather than presented as a fabricated original adoption commit.

The source successor completes List padding/borders and its cache/autoscroll
seams, batched tab navigation, lazy text wrappers/shared identity, Cosmic mark
offsets/sorting, the WGPU ordinary-image fragment fast path and Windows CPU COLR.
The broader audit also reconciles keymap NoAction/continuation precedence,
Div drag-over/debug selectors, Window pointer/test/cache/scene/menu APIs,
mobile touch/lifecycle vocabulary, hidden Mac activation, Windows disabled
caption/shared-IME/ordinary PopUp behavior, Job activity queries, full benchmark
workloads/tooling and the EGL error-handling dependency correction. Two additional
List vertical-inset defects found in both trees are repaired with a real
failing-before-fix rendering/reveal regression.

Complete Apache `open-gpui-ios` and optional `open-gpui-tokio` owners are selected.
iOS reuses shared Apple rendering, ordered native ingress, callback/input leases,
public activation requests and exact renderer-before-native shutdown tickets.
Tokio preserves upstream owned/borrowed runtime, task-drop cancellation and
JoinError behavior. These owners and the OHOS explicit-injection guard extend
framework scope without importing a Zed product or engine domain owner. Application
roots selecting native GLES must carry the documented official EGL patch because
library dependencies cannot export root Cargo patches.

The local code checkpoint is `138c6ef22c4ce8bdacad800bd1ae2cebd2cf0c44`.
Its reviewed implementation commits are:

| Local commit | Scope |
| --- | --- |
| `1660391ab1fbdd99a21336ef5e74bb9191ee80a6` | SharedString identity equality |
| `300c05d28a7b50ec90c296850aad01224e7bb0a4` | Core APIs, blanket elements, List/tab/keymap/text and public migrations |
| `4f8299386f8143cd0ff8fc3f5f1e21eef555f74a` | Windows CPU COLR, caption/IME/PopUp and Job activity |
| `d99e96a50714022d9dfb25b04763cf249654a714` | WGPU sprite fast path and Cosmic offsets/sorting/corpus |
| `69b5338ad1c2488e5d8052a0d6487df584b15e80` | Wayland callback time, hidden AppKit activation and Windows resources |
| `ca5f3abf148c1c9234ef4f09876da55b0956bd86` | Complete UIKit/Tokio owners, consumers, OHOS routing and EGL pin |
| `10878654b465c4f2f4e4fd4781f712952439b79e` | Complete benchmark workloads, ChildText and serial comparison tool |
| `138c6ef22c4ce8bdacad800bd1ae2cebd2cf0c44` | Ordinary runnable examples and factory guidance |

The [current verification](../verification.md#november-13-framework-closure-completion)
records final **3615/3615** affected tests, three existing ignored AppKit main-thread
cases, **8/8** independent public controls, **4/4** test-support consumer controls,
feature-off compilation, workspace/independent all-target checks, real-sysroot
Windows cross-compilation, production/a11y wasm, real Rust 1.93 host/simulator and
strict host/iOS Clippy. The iPhone 17/iOS 26.3 native probe prints its required
success marker after real pixel, activation, two composition-cleanup, resized
submission, idle-guard and exact-retirement checks. Native UIKit protocol smoke
also passes. Initial failures and repairs remain recorded, including key ownership
surviving resignKeyWindow and CAMetalLayer ignoring a zero size request. Both
require actual native semantics rather than a synthetic cached fact.

All five benchmark targets complete bounded Criterion test smoke with production
native factories. The unoptimized 500-frame layout-churn diagnostic completes in
93.465 seconds after an initial 55-second timeout; this is workload completion,
not a baseline/candidate timing result. Exact-shape/shared resource regressions,
source review and existing format/import/API/docs/release/dependency-health gates
pass. Full upstream/local SHA and platform mappings live in the ledger; the
hand-off does not replace that evidence or grant foreign native qualification.

Next development should first close the platform matrix on owning runners:
AppKit hidden activation/close/sheet/submitted resize with an active display;
Windows native DirectWrite COLR, caption/IME/PopUp, Job and embedded/runtime-shader
WARP controls; Linux X11/Wayland/GLES/profiler switching; browser WebGPU/WebGL and
physical input; iOS device lifecycle/rotation/IME/Universal Links and GPU retirement.
Keep compile, headless GPU, simulator, real native window and physical-input
results distinct. A later performance task should use the complete benchmark
workloads with one fixed baseline/candidate pair, the same font/device/feature
selection and recorded measurement environment before choosing a renderer rewrite.

The [migration guide](../ui/migration-v0.3.md#blanket-element-conversion), release
inventory and new package READMEs cover blanket Element conversion, complete
GraphicalEnvironment literals/lowercase headless, exhaustive touch/diagnostic
matches and optional platform/runtime adoption. Preserve the fork's accepted
candidate, exact clip, resource lease and native retirement contracts when comparing
future official changes. This continuation authorizes local Conventional Commits;
it does not push. Inspect live local/remote state before a later pull, merge or
publication. Do not move the fixed target just because upstream HEAD advanced.

## Upstream Maintenance

The earlier whole-framework comparison target is Zed
`635026fabc38440fce5e1767d125026a17b12012`; the later Mac/Apple cycle explicitly selects
`78f5d7446c55f6c1aa9b66e8111f6b3f4625ebb0`. Keep each cycle's source and executed scope
distinct. Do not silently replace either pin with a moving remote HEAD or claim monorepo/API parity.
Read these existing owners before beginning another refresh:

- [Upstream sync skill](../../.agents/skills/open-gpui-upstream-sync/SKILL.md).
- [Import provenance](../imports/zed-gpui-import.md).
- [Candidate dispositions](../imports/zed-gpui-upstream-sync-candidates.md).
- [Refresh plan](2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md).
- [Verification evidence](../verification.md).
- [Current engineering state](../knowledge/engineering/current-state.md).

Use complete official mechanisms as the baseline, then refactor fork-owned changes for supported
consumers. A retained difference needs a concrete contract, source evidence and focused tests.
Trust unchanged official mechanisms; spend qualification on changed integration and ownership.
The extracted repository has its own history: a guessed cross-repository merge-base is not
provenance.

The owner requires regular comparison with official Zed GPUI during ongoing development, not
only during a dedicated sync cycle. Before changing a framework owner, read the corresponding
official implementation and relevant fix/follow-up history at the recorded pin. Before committing
the unit, compare its final behavior and regression scenarios again. Reuse complete official
mechanisms, then adapt the fork's concrete candidate, resource and native-lifecycle contracts;
record a material difference and its reason in the existing owner/evidence documents. Observe a
new upstream revision separately and select a new comparison pin explicitly when starting its
adoption cycle.

## Consumers And Interface Pressure

Open-GPUI remains independent of Nara and Jellyflow. Native applications keep ordinary platform
and renderer choices. An engine may supply its own device/queue/target/encoder and own actual
submission through the existing hosting/render interfaces.

Nara's independently locked consumer is at
`F:/SourceCodes/Rust/nara/tests/fixtures/gpui-engine-consumer`. Its Scene, property/hierarchy,
typed graph, continuous gestures and independent image View trajectories qualify concrete
engine integration. They are not a complete Editor or game UI decision. The optional
`jellyflow-trial` consumer tests two native graph Views through the same original engine Host.

Jellyflow's previous registry `open-gpui = "=0.2.0"` is not source-compatible with this maintained
revision despite the same package version. Its migration uses the exact published Git revision.
The resulting pressure includes typed pointer-capture handles, targeted input/scroll intents,
committed measurements, modifier context and accepted publication/retirement. Keep these as
ordinary framework interfaces rather than add engine- or node-specific methods to Window.

Consumer integration failures should provide an operation, original window/view identity,
expected outcome, source pin and minimal reproduction. Framework fixes belong here; graph
interaction/measurement policy belongs in Jellyflow; documents/assets/history/compiler/RenderHost
policy belongs in Nara.

Nara's subsequent Dialogue prerequisite is `ddcc880893c0081e6ceb76be31d995e3ce7ee37b`:
an original Session-owned domain, a bounded codec, stable dynamic choice ports and immutable
Jellyflow facts. Its CPU/public workflow passes 75/75, including existing controls. The planned
native external-document adapter is still missing. Its currently confirmed blockers are in
Jellyflow's combined read-side/mutation runtime and native intent/completion path, not an
identified Open-GPUI runtime defect. Keep framework work separate until that consumer supplies
a concrete framework reproduction.

## Dependency And Ownership Direction

Share appropriate mechanisms and complete unstyled behavior. Separate live roots, product state,
skins and adapters. Existing `ui-core` has only neutral motion as a production dependency;
components/canvas/docking require the GPUI runtime. Public resolved state is not a complete
unstyled control interface while activation/focus/publication behavior remains private.

Extract a module when an actual independently consuming workflow benefits from its dependency
or lifecycle seam. Avoid a crate per algorithm and avoid a universal UI tree or renderer wrapper.
Do not import engine ECS, document history, shader graphs, project services or a second scheduler.
Game UI reuse must preserve no-native-window roots and suitable input/resource/time adapters;
Editor success cannot establish gamepad, split-screen or world-space behavior.

## Recommended Roadmap

| Priority | Work | Acceptance and limits |
|---|---|---|
| P0 | Keep published source and direct consumers buildable. | One framework source identity; focused checks and native integration for changed contracts. Preserve compiler/feature/platform support explicitly. |
| P0 | Preserve existing hosted-root and presentation correctness. | Fix concrete capture/text/IME/popups, geometry, candidate and close regressions through the original owners; broader host expansion is not the current development priority. |
| P1 | Use the delivered scalar, Tree and searchable-choice workflows in complete consumers. | The November 13 component/Gallery receipt and resolved independent findings are recorded in the evolution evidence. Preserve private semantic activation, text/focus and publication owners; physical capture/IME still needs the owning platform. |
| P1 | Complete physical qualification and caller cutover for the selected shared dockspace architecture. | ADR 0034 supersedes the binary-kernel choice. Logical multiview, exact output correlation, persistence and group retirement have deterministic coverage. Native capture/hit-stack and active-display execution precede incumbent removal; do not introduce two semantic writers in a workspace. |
| P1 | Deliver a coherent Nara editing page above the reusable controls and selected docking proof. | Preserve original document/history/Workspace and RenderHost ownership. Qualify the exact source pin; a fixture or design mockup does not establish an Editor-default migration. |
| P2 | Retain the current state model and measure concrete update/layout costs. | Preserve existing projections, independent drafts and operation-basis correctness. A different state architecture requires a separately selected consumer-backed design. |
| P2 | Optimize repeated upload or raster work when measurements justify it. | Correct baseline versus static properties, scrolling collection, caret/text, node canvas and unrelated animated View; report allocations, bytes/uploads, CPU/GPU and latency separately. |
| P2 | Refine runtime/platform/render/component package seams from consumers. | Standalone UI, engine hosting and appropriate game roots each demonstrate their dependency/lifecycle need. Preserve ordinary applications and avoid Nara-specific public interfaces. |

## Compositor And Performance Direction

A compositor is a research direction, not a required next rewrite. Distinguish:

- Cached View/dependency reuse, which can skip internal description/layout work.
- CPU Scene/prepaint/paint replay, which still remaps/copies/orders retained primitives.
- Retained GPU geometry, which can avoid uploads but still execute raster shaders.
- Raster layers and scroll tiles, which trade textures/rebuild/memory for sampling.
- Damage tracking and final composition; partial offscreen work does not prove partial swapchain
  presentation.
- A render thread, which needs an owned immutable recording/resource lifetime and cannot take
  arbitrary live UI state.

Measure the actual bottleneck first. Cache validity must include geometry, inherited style/text,
fonts/fallback, scale, clips, opacity, transforms, resource generations and presentation. Define
eviction, memory budgets and device/target retirement. Preserve the host's single queue and
actual submission authority. Do not add another GPU Scene owner merely because capacity reuse
does not skip unchanged uploads.

Signals can narrow state authoring/dependency work but do not make layout/paint/upload
incremental by themselves. Entity-wide `notify`, tracked reads, explicit cached islands and their
non-View projection limitation need precise treatment. Reproduce the actual sparse-update
workload before replacing the runtime or adopting persistent layout.

Nara's source-grounded research is at:

- `docs/research/2026-11-11-open-gpui-engine-integration-layers.md`,
  sections `Compositor And Cache Research Direction` and
  `GPUI Critique And State Invalidation Research`.
- `docs/research/2026-10-27-custom-drawn-editor-ui-boundaries.md`.
- `docs/research/2026-11-12-jellyflow-node-editor-audit.md`.

Those are paths relative to the sibling Nara checkout. They distinguish implemented mechanisms
from proposals and record the supplied compositor document and critique article. The supplied
files are background inputs; current source and matching native measurements are evidence.

## First Maintenance Session

1. Verify the current checkout, local commits and remote pin; read this handoff's Mac continuation
   with the current-state/verification sources. Choose the actual host's serialized Cargo setup.
2. Obtain the final Nara/Jellyflow integration result and any concrete framework reproductions.
3. Choose one complete consumer-driven interface or measured cost. Keep scope independent from
   engine document/compiler work and unfinished node-library defects.
4. Implement in its existing owner, review independently, run focused serial qualification and
   update the owning guide/plan/evidence.
5. Return the exact commit, API migration, supported source/compiler/platform scope and minimal
   consumer command to the Nara/Jellyflow sessions.

The original handoff was published in the later `2cc8ff1` documentation commit. Code consumers
retain `f75c10b` because that change adds no runtime behavior. Subsequent local handoff updates
do not authorize another push or constitute another upstream sync or runtime qualification.
