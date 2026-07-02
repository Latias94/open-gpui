---
type: Current State
title: Open GPUI UI motion runtime foundation state
status: active
timestamp: 2026-07-02T22:46:36+08:00
git_branch: refactor/docking-flat-motion-runtime
related_plan: docs/plans/2026-07-02-003-refactor-ui-motion-runtime-foundation-plan.md
related_adr:
  - docs/adr/0010-docking-presentation-scene-motion-model.md
  - docs/adr/0011-docking-split-motion-primitive-boundary.md
  - docs/adr/0012-docking-runtime-capability-alignment.md
  - docs/adr/0013-ui-motion-runtime-foundation.md
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
  - python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering
---

# Current State

- Branch: `refactor/docking-flat-motion-runtime`.
- Goal: finish `docs/plans/2026-07-02-003-refactor-ui-motion-runtime-foundation-plan.md`.
- Done: `open_gpui_ui_core` owns `MotionTimeline`, deterministic sampled progress,
  immediate/active/completed/cancelled timeline state, reduced-motion final semantics, and
  stable-identity retarget helpers.
- Done: `ui_components::Splitter` consumes `MotionTimeline` for programmatic fraction animation and
  keeps pointer drag immediate.
- Done: `gpui_docking::DockTransitionExecutor` consumes `MotionTimeline` and
  `retarget_motion_snapshots` while keeping pane, divider, overlay, zoom, focus, tab, route,
  viewport, and release semantics local.
- Done: Native dogfood status panel exposes a separate `motion proof` line for shared runtime,
  sampled progress, retarget identity, and reduced-motion final state.
- Done: ADR 0013 records the generalized shared motion runtime boundary.
- In progress: final broad verification, shipping review, and final memory closeout.
- Blocked: None.
- Next action: run final verification gates, run the ce-work shipping tail, then mark the goal
  complete if no actionable findings remain.

# Citations

- [Motion runtime foundation plan](../../plans/2026-07-02-003-refactor-ui-motion-runtime-foundation-plan.md)
- [ADR 0013](../../adr/0013-ui-motion-runtime-foundation.md)
- [Progress memory](progress/2026-07-02-ui-motion-runtime-foundation.md)
- [Verification](../../verification.md)
