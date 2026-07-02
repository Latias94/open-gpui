---
type: Current State
title: Open GPUI docking flat motion runtime state
status: complete
timestamp: 2026-07-02T20:56:30+08:00
git_branch: refactor/docking-flat-motion-runtime
related_plan: docs/plans/2026-07-02-002-refactor-docking-flat-motion-runtime-plan.md
related_adr:
  - docs/adr/0010-docking-presentation-scene-motion-model.md
  - docs/adr/0011-docking-split-motion-primitive-boundary.md
  - docs/adr/0012-docking-runtime-capability-alignment.md
verified_by:
  - cargo fmt --all -- --check
  - git diff --check
  - cargo check -p open-gpui-docking
  - cargo nextest run -p open-gpui-ui-core motion split --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_transition_tests host_render_tests host_presentation_scene_tests host_zoom_focus_tests --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_viewport_preview_visual_tests host_viewport_preview_tests root_edge_hover_keeps_target_leaf_side_guides_visible dragging_tab_to_other_stack_center_moves_panel --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_interaction_tests::tab_bar_preview_positions_payload_tab_at_leading_and_middle_slots host_viewport_close_tests::runtime_suite::viewport_runtime_cancel_retain_should_close_restores_current_route_facts host_viewport_route_tests::runtime_suite::viewport_runtime_requires_backend_route_selection_for_drop transition_plan_between_overlay_scenes_keeps_previous_bounds_for_matching_layers host_viewport_preview_visual_tests host_viewport_preview_tests root_edge_hover_keeps_target_leaf_side_guides_visible dragging_tab_to_other_stack_center_moves_panel --no-fail-fast
  - cargo nextest run -p open-gpui-docking --no-fail-fast
  - cargo nextest run -p open-gpui-docking-native runtime_status_panel_formats_platform_capabilities --no-fail-fast
  - cargo check -p open-gpui-docking-native
  - python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering
---

# Current State

- Branch: `refactor/docking-flat-motion-runtime`.
- Goal: completed `docs/plans/2026-07-02-002-refactor-docking-flat-motion-runtime-plan.md`.
- Done: Shared motion sampling tokens, committed-layout/continuity/affordance specs, split
  transition descriptors, and `ui_components::Splitter` programmatic fraction animation are in
  place.
- Done: Docking transition execution retargets from current sampled geometry; pane reveal renders
  real final-size content behind a clip and an occlusion mask.
- Done: Viewport host frames are seeded from `DockPresentationScene` facts. The remaining
  render-measured fact probe is intentionally limited to text-shaped tab labels.
- Done: Overlay preview feedback has an independent transition executor and schedules body,
  guide, tab insertion, payload tab, payload ghost, route marker, and rejected descriptors without
  changing release authority.
- Done: Zoom/unzoom/focus use the real-content transition path; public focus commands stay
  immediate, and interrupted unzoom retargets from the active zoom sample.
- Done: Tab insertion preview layers keep current target bounds during hover retargets while guide
  layers can still use affordance motion. Full docking nextest passed 860/860.
- In progress: None for this plan.
- Blocked: None.
- Next action: merge or push the completed branch according to the normal branch flow.

# Citations

- [Flat motion runtime plan](../../plans/2026-07-02-002-refactor-docking-flat-motion-runtime-plan.md)
- [Progress memory](progress/2026-07-02-docking-flat-motion-runtime-plan.md)
- [Verification](../../verification.md)
