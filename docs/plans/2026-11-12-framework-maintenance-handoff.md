# Open-GPUI Framework Maintenance Handoff

Prepared November 12, 2026 for the independent Open-GPUI maintenance session. This document owns
handoff navigation and suggested next work. Accepted ADRs remain architecture authority; existing
plans and qualification records retain their exact scope.

The engine development path is recorded separately in Nara:
`docs/plans/2026-11-12-nara-ui-integration-handoff.md`.

## Scope And Next Deliverable

Independent framework maintenance can begin from the published baseline and qualified engine
consumer. Completing Nara's Editor or game UI selection is not a prerequisite for that session.
This roadmap covers reusable UI runtime, platform, rendering, control behavior and performance
work. Nara's domain tools and Jellyflow's graph-document interface remain in their own owners.

The first deliverable should address a concrete framework reproduction from those consumers,
or complete public behavior/skin reuse with an ordinary custom control. Preserve ordinary
standalone applications alongside hosted roots. Return the exact revision, migration notes,
changed feature/platform scope and a focused consumer command. A compositor, new package
hierarchy or Signals rewrite needs a measured workload and its own selected design first.

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
| P0 | Stabilize hosted-root, custom native content and presentation lifecycle. | Capture/text/IME/popups, geometry/scale changes, accepted/refused frames, cancellation and close work through the original Window and submission owner. |
| P1 | Complete public behavior/skin reuse under a real custom-control consumer. | Two skins share pointer/keyboard/a11y/programmatic activation, focus, disabled/read-only behavior, publication and retirement. Do not expose internal state solely to make an export list look complete. |
| P1 | Continue canonical docking U4–U9 against its owning plan. | Preserve stable panel identity, factories, multiple View lifetimes and native ownership. Do not infer docking completion from finite graph panels. |
| P1 | Improve projection/state ergonomics and quantify layout cost. | Shared models, independent dirty drafts and equal-value/new-operation-basis cases remain correct; attribute notification, render, layout/prepaint and subscription release. |
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
