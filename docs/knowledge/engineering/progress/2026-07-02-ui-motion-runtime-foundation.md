---
type: Work Progress
title: UI motion runtime foundation
status: active
timestamp: 2026-07-02T22:46:36+08:00
git_branch: refactor/docking-flat-motion-runtime
related_plan: docs/plans/2026-07-02-003-refactor-ui-motion-runtime-foundation-plan.md
related_adr:
  - docs/adr/0011-docking-split-motion-primitive-boundary.md
  - docs/adr/0012-docking-runtime-capability-alignment.md
  - docs/adr/0013-ui-motion-runtime-foundation.md
git_commits:
  - 0b5ff55 docs(ui): plan motion runtime foundation
  - 182ce03 feat(ui-core): add shared motion runtime primitives
  - 0bb5897 refactor(ui-components): use shared motion timeline for splitter
  - d028e31 refactor(docking): use shared motion runtime for transitions
  - 55628ca test(docking): expose motion runtime proof summary
verified_by:
  - cargo fmt --all -- --check
  - git diff --check
  - cargo nextest run -p open-gpui-ui-core motion --no-fail-fast
  - cargo test -p open-gpui-ui-components runtime_animates_programmatic_fraction_changes --lib -- --nocapture
  - cargo test -p open-gpui-ui-components runtime_retargets_from_sampled_fraction_and_drag_syncs_immediately --lib -- --nocapture
  - cargo test -p open-gpui-ui-components runtime_reduced_motion_completes_without_transition --lib -- --nocapture
  - cargo test -p open-gpui-ui-components --test public_surface component_api_inventory_tracks_public_method_surface -- --nocapture
  - cargo nextest run -p open-gpui-docking transition_executor_samples_timeline_and_reveal_geometry transition_executor_replaces_active_execution_and_completes_reduced_motion_immediately overlay_retarget_keeps_tab_preview_layers_at_current_target_bounds host_unzoom_command_retargets_from_active_zoom_sample public_focus_command_uses_immediate_overlay_only_feedback --no-fail-fast
  - cargo nextest run -p open-gpui-docking-native runtime_status_panel_formats_platform_capabilities --no-fail-fast
tags:
  - ui-core
  - motion
  - animation
  - splitter
  - docking
---

# Summary

Implemented the shared UI motion runtime foundation from
`docs/plans/2026-07-02-003-refactor-ui-motion-runtime-foundation-plan.md` on
`refactor/docking-flat-motion-runtime`.

# Shipped Capability

- `open_gpui_ui_core` now owns `MotionTimeline`, `MotionTimelineSample`,
  `MotionTimelineState`, `MotionSnapshot`, `MotionRetargetItem`, `MotionRetargetSet`, and
  `retarget_motion_snapshots`.
- The runtime primitive is renderer-neutral: it samples deterministic timeline progress, reports
  immediate/active/completed/cancelled states, preserves reduced-motion final semantics, and matches
  interrupted samples to new targets by stable identity.
- `ui_components::Splitter` uses `MotionTimeline` for programmatic fraction changes and keeps
  pointer drags immediate.
- `gpui_docking::DockTransitionExecutor` uses `MotionTimeline` for progress/completion and
  `retarget_motion_snapshots` for pane, divider, and overlay retarget matching while keeping
  docking sample semantics local.
- The docking native status panel now exposes `motion proof:
  shared-runtime+timeline-state+sampled-progress+retargeted-identity+reduced-motion-final-state`.

# Boundaries

- `ui_core` owns timing and identity matching only.
- Adapters own frame scheduling, rendering, semantic interpolation, and enter/leave policies.
- Docking owns graph, tab, route, viewport, pane, divider, overlay, zoom, focus, and release
  semantics.
- No compositor, spring, keyframe, or broad public animation framework was introduced.

# Current Verification State

Focused gates have passed for `ui_core`, splitter runtime, docking transition retargeting, zoom
retargeting, focus immediacy, and the native proof summary. Final broad package gates and
engineering wiki validation still need to run before marking this progress record verified.

# Citations

- [Plan](../../../plans/2026-07-02-003-refactor-ui-motion-runtime-foundation-plan.md)
- [ADR 0013](../../../adr/0013-ui-motion-runtime-foundation.md)
- [Verification](../../../verification.md)
