---
type: Subagent Finding
title: Docking runtime capability follow-up synthesis
timestamp: 2026-06-30T23:45:34+08:00
git_branch: main
related_plan: docs/plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md
source_agents:
  - split_animation_a11y
  - split_docking_gap
  - split_core_fit
  - bonsplit_reference
  - docking_docs_followup
---

# Finding

The split/motion primitive refactor has landed the right descriptor model, but runtime capability alignment is not complete.
The next work should make descriptors drive visible behavior: transition sampling, sampled overlay/clip/divider/focus/payload geometry, precise tab insertion, payload ghost cleanup, zoom/focus presentation, GPUI accessibility mapping, corner-drag productization, and native dogfood proof.

# Evidence

- `split_animation_a11y` found that `DockTransitionExecutor` still stores a plan and requests a frame but lacks start time, progress, easing sampling, repeated scheduling, cancellation, and sampled overlay/clip/divider output.
- `split_docking_gap` found that center tab insertion has `DockPreviewTabInsertion.slot_bounds` but still needs exact before/middle/append slot computation, payload ghost rendering, and routed cleanup.
- `split_core_fit` found that shared split primitives are strong enough for docking's next step, while generic fill-child policy and pixel resize helpers can still move from docking-local geometry into `open_gpui_ui_core`.
- `bonsplit_reference` found portable ideas in BonSplit and ImGui: controller transaction flow, layout versus tree snapshots, rectangle-neighbor focus navigation, data-first preview objects, explicit inner/outer targets, and queued commit requests.
- `docking_docs_followup` found documentation drift: local `main` is now at `3497a85`, but previous memory still described the feature branch and an unfinished U10 state.

# Recommendation

Use `docs/plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md` as the next implementation artifact.
Prioritize the runtime executor and sampled overlay/clip/divider render path first, because zoom/unzoom, focus animation, payload ghosts, and cross-window overlay animation all depend on that foundation.
Keep `DockGraph`, current facts, drop commits, central region, floating containers, and viewport routing in docking; move only generic split geometry, fill, resize, and navigation primitives into `open_gpui_ui_core`.

# Disposition

Captured in the `004` plan as implementation-ready follow-up work.
No code was changed by the subagents.
