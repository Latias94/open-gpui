---
type: Work Progress
title: Docking flat motion runtime framework implementation
status: verified
timestamp: 2026-07-02T20:56:30+08:00
git_branch: refactor/docking-flat-motion-runtime
related_plan: docs/plans/2026-07-02-002-refactor-docking-flat-motion-runtime-plan.md
related_adr:
  - docs/adr/0010-docking-presentation-scene-motion-model.md
  - docs/adr/0011-docking-split-motion-primitive-boundary.md
  - docs/adr/0012-docking-runtime-capability-alignment.md
git_commits:
  - 3015c9f docs(docking): plan flat motion runtime framework
  - 7f2b958 feat(ui-core): add shared motion sampling tokens
  - 85024d1 fix(docking): retarget transitions from sampled progress
  - fc9ab40 fix(docking): retarget transitions from sampled geometry
  - 97f4449 feat(ui-core): describe splitter layout transitions
  - c8b0e70 feat(docking): render real pane content in transition clips
  - 91b33f5 feat(docking): seed viewport drops from presentation scene
  - 40840cb feat(ui-components): animate splitter fraction changes
  - 2686682 fix(docking): measure tab label drop facts from render
  - 908c3ec feat(docking): animate overlay preview transitions
  - fa433a7 fix(docking): occlude base panes during reveal transitions
  - b7a8290 fix(docking): drive transition occlusion from samples
verified_by:
  - cargo fmt --all -- --check
  - git diff --check
  - cargo check -p open-gpui-docking
  - cargo nextest run -p open-gpui-ui-core motion split --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_transition_tests host_render_tests host_presentation_scene_tests host_zoom_focus_tests --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_viewport_preview_visual_tests host_viewport_preview_tests root_edge_hover_keeps_target_leaf_side_guides_visible dragging_tab_to_other_stack_center_moves_panel --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_interaction_tests::tab_bar_preview_positions_payload_tab_at_leading_and_middle_slots host_viewport_close_tests::runtime_suite::viewport_runtime_cancel_retain_should_close_restores_current_route_facts host_viewport_route_tests::runtime_suite::viewport_runtime_requires_backend_route_selection_for_drop transition_plan_from_overlay_scene_uses_current_bounds_for_matching_layers transition_plan_keeps_preview_layers_at_current_target_bounds overlay_replacement_keeps_preview_layers_at_current_target_bounds host_viewport_preview_visual_tests host_viewport_preview_tests root_edge_hover_keeps_target_leaf_side_guides_visible dragging_tab_to_other_stack_center_moves_panel --no-fail-fast
  - cargo nextest run -p open-gpui-docking --no-fail-fast
  - cargo nextest run -p open-gpui-docking-native runtime_status_panel_formats_platform_capabilities --no-fail-fast
  - cargo check -p open-gpui-docking-native
  - python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering
tags:
  - docking
  - motion
  - animation
  - split
  - ui-ux
---

# Summary

Implemented the flat motion runtime pass from the July 2 docking plan on
`refactor/docking-flat-motion-runtime`. The implementation keeps `DockGraph` as mutation authority
and current drop facts as release authority, while moving runtime motion toward flat
presentation-scene and overlay-scene samples.

# Shipped Capability

- `MotionSpec` now exposes sampled progress and named tokens for committed layout, continuity, and
  short affordance motion.
- `DockTransitionExecutor` retargets scheduled transitions from the current sampled pane, divider,
  and overlay geometry instead of restarting from zero.
- Transition pane reveal renders real pane content at final size through clip wrappers. A
  transition occlusion mask covers the base final scene behind the reveal so new/returning panes do
  not show through early.
- `DockPresentationScene` seeds viewport drop facts for root, empty, panes, tab bars, and floating
  title bars. The remaining `render_viewport_drop_scene_fact_probe` is intentionally limited to
  tab-label facts whose bounds depend on GPUI text shaping.
- Overlay preview feedback has its own adapter-owned transition executor. Local and routed previews
  schedule root-level overlay transition plans for target body, guide boxes, tab insertion slots,
  payload tabs, payload ghosts, route markers, and rejected feedback. Precise tab insertion layers
  keep current target bounds during hover retargets so the drop preview does not lag behind the
  resolved target slot.
- `ui_components::Splitter` keeps pointer dragging immediate and animates programmatic fraction
  changes with shared committed-layout motion.
- Zoom/unzoom/focus use the same real-content transition samples. Public focus commands remain
  immediate for high-frequency keyboard use, while explicit focus-region proofs can still pass a
  motion spec.

# Boundaries

- No new ADR was needed for that docking-only closeout. ADR 0011 and ADR 0012 still held at that
  time: UI-core described renderer-neutral primitives; adapters owned scheduling and rendering;
  docking release authority stayed in current drop facts. ADR 0013 later records the generalized
  shared motion runtime boundary after `ui_components::Splitter` and `gpui_docking` both moved to
  the same renderer-neutral timeline and retarget primitive.
- This is capability parity, not pixel parity or native compositor parity. The shipped claim is
  GPUI-native real-content reveal, retargeting, overlay affordance motion, reduced-motion
  semantics, and descriptor-backed proof.

# Closeout

- Final full docking sweep passed with `cargo nextest run -p open-gpui-docking --no-fail-fast`
  (860/860).
- Native example check passed with `cargo check -p open-gpui-docking-native`.
- Native manual dogfood should still inspect nested edge hover, center tab insertion preview,
  cross-window route marker separation, zoom/unzoom continuity, and reduced-motion final semantics.

# Citations

- [Plan](../../../plans/2026-07-02-002-refactor-docking-flat-motion-runtime-plan.md)
- [ADR 0010](../../../adr/0010-docking-presentation-scene-motion-model.md)
- [ADR 0011](../../../adr/0011-docking-split-motion-primitive-boundary.md)
- [ADR 0012](../../../adr/0012-docking-runtime-capability-alignment.md)
- [Verification](../../../verification.md)
