---
type: Current State
title: Docking runtime capability follow-up state
status: active
timestamp: 2026-06-30T23:45:34+08:00
git_branch: main
related_plan:
  - docs/plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md
  - docs/plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md
related_adr:
  - docs/adr/0010-docking-presentation-scene-motion-model.md
  - docs/adr/0011-docking-split-motion-primitive-boundary.md
verified_by:
  - cargo check -p open-gpui-docking --tests
  - cargo nextest run -p open-gpui-docking geometry host_accessibility_tests --no-fail-fast
  - cargo fmt --all -- --check
  - cargo nextest run -p open-gpui-ui-core --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests --no-fail-fast
  - cargo check -p open-gpui-docking-native
  - git diff --check
  - python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering
---

# Current State

- Goal: Local `main` now includes the completed split/motion primitive refactor and has a new
  implementation-ready follow-up plan for docking runtime capability alignment.
- Branch: `main`, fast-forwarded to `3497a85` (`docs(docking): record split motion primitive
  boundary`), ahead of `origin/main` by 12 local commits.
- Done: `docs/plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md` is implemented
  and committed through `3497a85`.
- Done: Shared split and motion primitives live in `open_gpui_ui_core`; `ui_components::Splitter`
  renders through core state; docking presentation, overlay, transition descriptors, zoom/focus,
  divider hit maps, accessibility descriptors, and resize transactions consume those primitives.
- Done: Read-only subagent research synthesized the next capability gaps: real transition timeline
  sampling, sampled overlay/clip/divider render integration, precise tab insertion, payload ghost
  cleanup, zoom/focus user surfaces, GPUI accessibility mapping, corner-drag proof, and continued
  split primitive cleanup.
- In progress: Planning only. `docs/plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md`
  is the next executable plan; no implementation has started for it in this state record.
- Last verified: previous focused gates for split/motion primitives passed locally. This planning
  update did not rerun cargo tests.
- Blocked: None.
- Next action: run `compound-engineering:ce-work` or a goal against the `004` plan when ready; push
  local `main` separately if remote publication is desired.

# Citations

- [Split primitive plan](../../plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md)
- [Runtime capability follow-up plan](../../plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md)
- [ADR 0010](../../adr/0010-docking-presentation-scene-motion-model.md)
- [ADR 0011](../../adr/0011-docking-split-motion-primitive-boundary.md)
- [Progress note](progress/2026-06-30-docking-split-motion-primitives.md)
- [Verification evidence](verification/docking-split-motion-primitives-20260630.md)
- [Follow-up subagent synthesis](subagents/docking-runtime-capability-followup-20260630.md)
