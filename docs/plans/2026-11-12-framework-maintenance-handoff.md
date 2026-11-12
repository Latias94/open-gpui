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

## Repository And Published Baseline

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
and concurrent work before editing. Keep `repo-ref/` read-only. Coordinate Cargo with the Nara
session: one invocation at a time, `CARGO_BUILD_JOBS=1`, Rust `1.95.0`, shared
`F:/SourceCodes/Rust/nara/target`, no clean. The documented framework compiler floor and foreign
platform claims keep their own verification.

## Upstream Maintenance

The fixed comparison target is Zed `635026fabc38440fce5e1767d125026a17b12012`.
Do not silently replace that pin with a moving remote HEAD or claim monorepo/API parity.
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

1. Verify the current checkout/remote pin and read the current-state/verification sources.
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
