---
type: Current State
title: Docking runtime capability follow-up state
status: active
timestamp: 2026-07-01T00:00:00+08:00
git_branch: main
related_plan:
  - docs/plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md
  - docs/plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md
related_adr:
  - docs/adr/0010-docking-presentation-scene-motion-model.md
  - docs/adr/0011-docking-split-motion-primitive-boundary.md
verified_by:
  - cargo check -p open-gpui-docking --tests
  - cargo nextest run -p open-gpui-docking host_transition_tests host_render_tests --no-fail-fast
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

- Goal: Execute `docs/plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md`
  with fearless docking runtime capability refactors.
- Branch: `main`, ahead of `origin/main` by local docking commits.
- Done: `docs/plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md` is implemented
  and committed through `3497a85`.
- Done: Shared split and motion primitives live in `open_gpui_ui_core`; `ui_components::Splitter`
  renders through core state; docking presentation, overlay, transition descriptors, zoom/focus,
  divider hit maps, accessibility descriptors, and resize transactions consume those primitives.
- Done: Read-only subagent research synthesized the next capability gaps: real transition timeline
  sampling, sampled overlay/clip/divider render integration, precise tab insertion, payload ghost
  cleanup, zoom/focus user surfaces, GPUI accessibility mapping, corner-drag proof, and continued
  split primitive cleanup.
- Done: Phase A / P0 for the runtime capability plan is implemented locally. `DockTransitionExecutor`
  now samples progress with deterministic test timing, reduced motion exposes a final sample once,
  entering panes keep final-size content bounds while reveal clips animate, and render consumes
  sampled overlay/clip/divider output as a root transition layer over the final semantic layout.
- In progress: Phase B is next: precise tab insertion / payload ghost lifecycle and routed overlay
  cleanup from U4 and U9.
- Last verified: Phase A focused gates passed locally; see the runtime capability verification
  evidence file.
- Blocked: None.
- Next action: commit Phase A, then continue with U4/U9 payload tab insertion and routed overlay
  cleanup.

# Citations

- [Split primitive plan](../../plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md)
- [Runtime capability follow-up plan](../../plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md)
- [ADR 0010](../../adr/0010-docking-presentation-scene-motion-model.md)
- [ADR 0011](../../adr/0011-docking-split-motion-primitive-boundary.md)
- [Progress note](progress/2026-06-30-docking-split-motion-primitives.md)
- [Verification evidence](verification/docking-split-motion-primitives-20260630.md)
- [Runtime capability verification evidence](verification/docking-runtime-capability-alignment-20260701.md)
- [Follow-up subagent synthesis](subagents/docking-runtime-capability-followup-20260630.md)
