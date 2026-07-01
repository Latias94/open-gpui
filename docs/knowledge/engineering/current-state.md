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
  - cargo nextest run -p open-gpui-docking host_viewport_preview_visual_tests host_transition_tests host_interaction_tests::tab_bar_preview_positions_payload_tab_at_leading_and_middle_slots host_interaction_tests::tab_bar_append_preview_shifts_payload_tab_right_of_existing_tab host_interaction_tests::dragging_tab_to_target_tab_bar_empty_area_appends drop_runtime::tests::reorder_target_updates_insert_index_within_same_tab_stack drop_runtime::tests::tabs_drop_preserves_reorder_target_while_pointer_stays_inside_tab host_viewport_preview_tests::handle_suite::source_hover_over_known_viewport_renders_target_drop_preview --no-fail-fast
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
- Done: Phase B / P0 local and routed preview capability is implemented locally. Center/tab preview
  now has explicit `PayloadTab` and `PayloadGhost` overlay layers, precise insertion slot geometry can
  be applied to overlay descriptors, rendered tab-bar hover resolves label-specific insert indexes
  before falling back to append, and same-stack reorder hold no longer freezes a changed insert slot.
  Routed target previews now expose the same payload tab/ghost layer contract while source route
  markers remain separate.
- In progress: Phase C is next: zoom/focus user surfaces, GPUI accessibility mapping, split primitive
  cleanup, and visible corner-drag proof from U5-U8.
- Last verified: Phase A and Phase B focused gates passed locally; see the runtime capability verification
  evidence file.
- Blocked: None.
- Next action: commit Phase B, then continue with U5/U6/U7/U8 capability work.

# Citations

- [Split primitive plan](../../plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md)
- [Runtime capability follow-up plan](../../plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md)
- [ADR 0010](../../adr/0010-docking-presentation-scene-motion-model.md)
- [ADR 0011](../../adr/0011-docking-split-motion-primitive-boundary.md)
- [Progress note](progress/2026-06-30-docking-split-motion-primitives.md)
- [Verification evidence](verification/docking-split-motion-primitives-20260630.md)
- [Runtime capability verification evidence](verification/docking-runtime-capability-alignment-20260701.md)
- [Follow-up subagent synthesis](subagents/docking-runtime-capability-followup-20260630.md)
