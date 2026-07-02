---
type: Verification Evidence
title: Docking flat motion runtime verification
status: verified
timestamp: 2026-07-02T20:56:30+08:00
git_branch: refactor/docking-flat-motion-runtime
related_plan: docs/plans/2026-07-02-002-refactor-docking-flat-motion-runtime-plan.md
related_progress: docs/knowledge/engineering/progress/2026-07-02-docking-flat-motion-runtime-plan.md
---

# Verified

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo check -p open-gpui-docking`
- `cargo nextest run -p open-gpui-ui-core motion split --no-fail-fast`
- `cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast`
- `cargo nextest run -p open-gpui-docking host_transition_tests host_render_tests host_presentation_scene_tests host_zoom_focus_tests --no-fail-fast`
- `cargo nextest run -p open-gpui-docking host_viewport_preview_visual_tests host_viewport_preview_tests root_edge_hover_keeps_target_leaf_side_guides_visible dragging_tab_to_other_stack_center_moves_panel --no-fail-fast`
- `cargo nextest run -p open-gpui-docking host_interaction_tests::tab_bar_preview_positions_payload_tab_at_leading_and_middle_slots host_viewport_close_tests::runtime_suite::viewport_runtime_cancel_retain_should_close_restores_current_route_facts host_viewport_route_tests::runtime_suite::viewport_runtime_requires_backend_route_selection_for_drop transition_plan_from_overlay_scene_uses_current_bounds_for_matching_layers transition_plan_keeps_preview_layers_at_current_target_bounds overlay_replacement_keeps_preview_layers_at_current_target_bounds host_viewport_preview_visual_tests host_viewport_preview_tests root_edge_hover_keeps_target_leaf_side_guides_visible dragging_tab_to_other_stack_center_moves_panel --no-fail-fast`
- `cargo nextest run -p open-gpui-docking --no-fail-fast`
- `cargo nextest run -p open-gpui-docking-native runtime_status_panel_formats_platform_capabilities --no-fail-fast`
- `cargo check -p open-gpui-docking-native`
- `python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering`

# Evidence Scope

- Real pane reveal: `transition_pane_clip_mounts_real_pane_content` now proves final-size content,
  clip bounds, and occlusion bounds.
- Overlay runtime: center hover schedules body, insertion, payload tab, and payload ghost samples;
  routed preview tests prove source route marker and target overlay separation. Overlay/drop-preview
  geometry now stays pinned to the current semantic target instead of interpolating from previous
  preview bounds; the overlay timeline only drives opacity/lifecycle.
- Retargeting: transition executor retarget tests cover sampled layout geometry; zoom/unzoom tests
  cover active zoom sample retargeting.
- Splitter adapter: `splitter` tests prove programmatic fraction animation and immediate pointer
  drag.

# Final Closeout

- Full docking nextest passed 860/860 after fixing the tab-preview hover retarget regression and
  replacing two viewport route coordinate equality checks with pixel-tolerant point assertions.
- Native example check passed after the runtime proof summary update.
