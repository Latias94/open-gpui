---
type: Work Progress
title: Docking affordance authority cleanup plan
status: completed
timestamp: 2026-07-03T00:00:00+08:00
git_branch: refactor/docking-visual-affordance-runtime
related_plan: docs/plans/2026-07-03-002-refactor-docking-affordance-authority-cleanup-plan.md
tags:
  - docking
  - visual-affordance
  - motion
  - split
  - diagnostics
---

# Summary

Created the follow-up implementation-ready plan for the remaining fearless docking cleanup after
`DockVisualAffordanceScene` became the visual feedback descriptor.

# Current Progress

- U1 shipped in commit `85f6196` (`refactor(docking): make affordance scene own target previews`).
- U2 shipped in commit `db1ce27` (`refactor(docking): rename overlay motion to affordance motion`).
  Transition/motion APIs now use visual-affordance names, and the old `overlay_scene` test bridge
  has been deleted.
- U3 shipped in commit `ea73bdd` (`refactor(docking): publish affordance diagnostics via runtime status`).
  Visual affordance diagnostics are now published through
  `DockViewportRuntimeStatus` and consumed by the native status panel without retaining `DockHost`
  window handles.
- U4 shipped in commit `14d61fd` (`refactor(docking): share split and motion geometry primitives`).
  Renderer-neutral rect motion helpers now live in `ui_core`, docking
  transition sampling consumes those helpers, split/divider `UiRect` conversion is centralized, and
  graph layout now reuses the same split layout scene path as presentation/render.
- U5 completed locally: ADRs, verification docs, current-state memory, progress memory, and
  engineering log now point at the visual-affordance, runtime-status, and shared split/motion
  primitive paths.
- Target drop-preview rendering now builds and renders from `DockVisualAffordanceScene`.
- Payload tab measurement layout is neutral `DockPayloadTabPreviewLayout` data owned by the visual
  affordance scene, not the old overlay adapter.
- The implementation plan is complete; no source path should direct future work through
  `DockOverlayScene`, `DockOverlayTransition`, `DockOverlaySample`, or `split_pane_bounds`.

# Verification

- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo check -p open-gpui-docking`
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-docking host_viewport_preview_visual_tests host_render_tests host_accessibility_tests --no-fail-fast`
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-docking host_transition_tests host_zoom_focus_tests host_interaction_tests host_render_tests host_accessibility_tests host_viewport_preview_tests --no-fail-fast`
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo check -p open-gpui-docking -p open-gpui-docking-native`
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-docking dragging_tab_to_edge_renders_drop_preview visual_affordance_records_update_and_clear_with_window_references --no-fail-fast`
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-docking-native --no-fail-fast`
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo check -p open-gpui-ui-core -p open-gpui-docking`
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-ui-core motion_runtime split --no-fail-fast`
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-docking graph_split_tests host_presentation_scene_tests host_render_geometry_parity_tests host_divider_hit_map_tests host_transition_tests --no-fail-fast`
- `cargo fmt --all -- --check`
- `git diff --check`
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo check -p open-gpui-ui-core -p open-gpui-docking -p open-gpui-docking-native`
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-docking --no-fail-fast` (886 passed)

# Subagent Findings

- Overlay/affordance audit: resolved for target preview rendering and test fixtures; visual
  affordance descriptors are now the single semantic preview path.
- Transition API audit: resolved for crate-local motion types, executor samples, host facade
  methods, debug regions, and focused tests.
- Native diagnostics audit: resolved by adding runtime-owned visual affordance records and removing
  status-panel `DockHost` handle retention.
- Split/animation audit: resolved for the confirmed duplicate layer. `ui_core` now owns rect edge
  selection, offscreen source rects, reveal rects, and rect interpolation; docking keeps only
  semantic wrappers and divider-specific sampling.

# Planned Work

The new plan has five implementation units:

- U1: make target preview consume visual affordance descriptors. Done in `85f6196`.
- U2: rename overlay motion API to visual-affordance motion API. Done in `db1ce27`.
- U3: move visual diagnostics into runtime status. Done in `ea73bdd`.
- U4: collapse split and motion geometry duplication around existing `ui_core` primitives. Done in
  `14d61fd`.
- U5: update ADR/progress/docs and run focused-to-broad verification. Done.

# Next Action

Commit this U5 documentation tail, then merge/push according to the user's branch workflow. The next
product-level docking direction is not cleanup; it is choosing the next runtime capability slice
such as stronger animation observability, accessibility dogfood, or native visual polish.

# Citations

- [Plan](../../../plans/2026-07-03-002-refactor-docking-affordance-authority-cleanup-plan.md)
- [Previous visual affordance progress](2026-07-03-docking-visual-affordance-runtime.md)
