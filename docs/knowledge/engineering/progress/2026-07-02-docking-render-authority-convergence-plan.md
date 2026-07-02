---
type: Work Progress
title: Docking render authority convergence implementation
status: active
timestamp: 2026-07-03T01:42:37+08:00
git_branch: refactor/docking-render-authority-convergence
related_plan: docs/plans/2026-07-02-004-refactor-docking-render-authority-convergence-plan.md
related_adr:
  - docs/adr/0010-docking-presentation-scene-motion-model.md
  - docs/adr/0015-ui-motion-runtime-foundation.md
tags:
  - docking
  - render
  - geometry
  - ui-ux
---

# Summary

Implemented the docking render authority convergence plan through U5 on
`refactor/docking-render-authority-convergence`.

The pass is intentionally about geometry authority, not pixel-level style parity or new animation
primitives. `DockPresentationScene` is now the reference for deterministic root, pane, split,
splitter, tab-bar, floating, empty-central, and zoom geometry. Render still measures tab-label
bounds because final label hit targets depend on GPUI text shaping, intrinsic title layout, and the
close-button element.

# Shipped Boundary

- U1 added render/scene parity tests for root split children, nested panes, splitter handles,
  three-child splits, floating frame/title/content, empty central regions, and zoomed panes.
- U2 made deterministic viewport host scene frames scene-owned for root, leaf, tab-bar,
  empty-space, and floating-title facts while keeping runtime facts as release authority.
- U3 introduced `split_geometry` so presentation scene and render share split share/handle
  planning, with divider hit map and accessibility tests locking the same rectangles.
- U4 introduced `chrome_geometry` for tab-bar height, floating-title height, floating content
  bounds, and scene tab-label estimates. The failed characterization showed the old scene tab-bar
  policy was 28px while render was 36px; the shared policy now uses the rendered 36px tab strip.
- U5 removed the generic `render_viewport_drop_scene_fact_probe` name and replaced it with
  `render_tab_label_drop_scene_fact_probe`, making the remaining probe tab-label-specific by API.

# Verification

Focused gates observed during implementation:

- `cargo nextest run -p open-gpui-docking host_render_tests host_presentation_scene_tests host_interaction_tests --no-fail-fast` passed 117/117.
- `cargo nextest run -p open-gpui-docking render_tab_bar_bounds_match_presentation_scene_tab_bar render_floating_bounds_match_presentation_scene_container render_tiny_floating_handle_clamps_to_presentation_title_bar render_measured_tab_label_fact_overrides_scene_equal_slot_estimate runtime_nested_tab_tear_off_uses_leaf_size_not_tab_label --no-fail-fast` passed 5/5.
- `cargo nextest run -p open-gpui-docking render_measured_tab_label_fact_overrides_scene_equal_slot_estimate rendered_host_scene_frame_seeds_deterministic_facts_from_presentation_scene --no-fail-fast` passed 2/2.
- `cargo check -p open-gpui-docking` passed.
- `git diff --check` passed after U4.

# Next Action

Run the plan's final verification contract, validate engineering memory, then perform the
simplification/review tail. After that, merge back to local `main` and push if the user asks to
land the branch.

# Citations

- [Render authority convergence plan](../../../plans/2026-07-02-004-refactor-docking-render-authority-convergence-plan.md)
- [Docking flat motion runtime progress](2026-07-02-docking-flat-motion-runtime-plan.md)
- [UI motion runtime foundation progress](2026-07-02-ui-motion-runtime-foundation.md)
