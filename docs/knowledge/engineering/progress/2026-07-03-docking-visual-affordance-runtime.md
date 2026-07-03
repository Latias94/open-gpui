---
type: Work Progress
title: Docking visual affordance runtime
status: active
timestamp: 2026-07-03T23:59:00+08:00
git_branch: refactor/docking-visual-affordance-runtime
related_plan: docs/plans/2026-07-03-001-refactor-docking-visual-affordance-runtime-plan.md
verified_by:
  - CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-docking host_viewport_preview_visual_tests host_presentation_scene_tests host_divider_hit_map_tests --no-fail-fast
  - CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-docking host_transition_tests host_render_tests host_viewport_preview_visual_tests --no-fail-fast
  - CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-docking host_accessibility_tests host_divider_hit_map_tests host_debug --no-fail-fast
  - CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-docking host_viewport_preview_visual_tests host_transition_tests host_render_tests --no-fail-fast
  - CARGO_BUILD_JOBS=1 cargo check -p open-gpui-docking
  - CARGO_BUILD_JOBS=1 cargo check -p open-gpui-docking-native
  - CARGO_BUILD_JOBS=1 cargo check -p open-gpui-docking-native --tests
---

# Summary

`refactor/docking-visual-affordance-runtime` implements the plan to make docking visual feedback
capability-aligned instead of split across preview, overlay, motion, accessibility, and native
diagnostic helpers.

# Implemented

- Added `DockVisualAffordanceScene` as the crate-private visual feedback authority for drop target
  bodies, guide boxes, tab insertion slots, payload tab and ghost previews, route markers, rejected
  targets, divider handles and corners, focus rings, and zoom egress.
- Migrated overlay transition planning and render overlay transition identity to consume visual
  affordance layers and stable `motion_key` values.
- Migrated overlay accessibility descriptors to `DockVisualAffordanceScene` and deleted the old
  overlay-only accessibility mapping.
- Added public debug summary types and `DockHost::visual_affordance_debug_summary()` so the native
  runtime panel can show active affordance id, kind, scope, state, target node, zone, payload index,
  frame generation, and overlay motion state without log spam.
- Removed the route-marker overlay adapter. Route markers now go directly from route preview to
  `DockVisualAffordanceScene`; `DockOverlayScene` remains only as a render adapter for concrete
  drop-preview drawing and measured payload-tab layout.

# Commits

- `3622d23 test(docking): characterize visual affordance preview layers`
- `3b29827 refactor(docking): introduce visual affordance scene`
- `52f059e refactor(docking): route overlay motion through affordance scene`
- `8fb0fa3 refactor(docking): expose affordance diagnostics`
- `f7009a4 refactor(docking): remove route overlay adapter`

# Notes

- The remaining `DockOverlayScene` is intentionally not a semantic authority. New accessibility,
  motion, diagnostics, route marker, divider, focus, or zoom behavior should go through
  `DockVisualAffordanceScene`.
- Native runtime panel reads compact summaries from opened `DockHost` windows. If dogfood exposes a
  same-window render borrowing issue, change the flow so `DockHost` publishes summaries into
  runtime status rather than having the panel read host windows during render.
- A few broad tests still use overlay fixtures because they exercise measured payload-tab drawing
  and the remaining render adapter boundary.

# Citations

- [Plan](../../../plans/2026-07-03-001-refactor-docking-visual-affordance-runtime-plan.md)
- [Verification](../../verification.md)
