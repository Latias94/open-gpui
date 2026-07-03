---
type: Work Progress
title: Docking affordance authority cleanup plan
status: in-progress
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
- U2 completed locally: transition/motion APIs now use visual-affordance names, and the old
  `overlay_scene` test bridge has been deleted.
- Target drop-preview rendering now builds and renders from `DockVisualAffordanceScene`.
- Payload tab measurement layout is neutral `DockPayloadTabPreviewLayout` data owned by the visual
  affordance scene, not the old overlay adapter.
- Remaining implementation units are U3 diagnostics, U4 split/motion primitive cleanup, and U5 docs
  plus broad verification.

# Verification

- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo check -p open-gpui-docking`
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-docking host_viewport_preview_visual_tests host_render_tests host_accessibility_tests --no-fail-fast`
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-docking host_transition_tests host_zoom_focus_tests host_interaction_tests host_render_tests host_accessibility_tests host_viewport_preview_tests --no-fail-fast`

# Subagent Findings

- Overlay/affordance audit: resolved for target preview rendering and test fixtures; visual
  affordance descriptors are now the single semantic preview path.
- Transition API audit: resolved for crate-local motion types, executor samples, host facade
  methods, debug regions, and focused tests.
- Native diagnostics audit: the native runtime panel reads `DockHost` handles directly for visual
  affordance summaries instead of consuming a runtime status snapshot.
- Split/animation audit: `ui_core` split and motion primitives exist, but docking still has duplicate
  split render, divider hit-map identity, reveal, and bounds interpolation helpers.

# Planned Work

The new plan has five implementation units:

- U1: make target preview consume visual affordance descriptors. Done in `85f6196`.
- U2: rename overlay motion API to visual-affordance motion API. Done locally, pending commit.
- U3: move visual diagnostics into runtime status.
- U4: collapse split and motion geometry duplication around existing `ui_core` primitives.
- U5: update ADR/progress/docs and run focused-to-broad verification.

# Next Action

Commit U2, then implement U3 by moving native visual-affordance diagnostics to runtime status and
removing direct native status-panel ownership of `DockHost` handles.

# Citations

- [Plan](../../../plans/2026-07-03-002-refactor-docking-affordance-authority-cleanup-plan.md)
- [Previous visual affordance progress](2026-07-03-docking-visual-affordance-runtime.md)
