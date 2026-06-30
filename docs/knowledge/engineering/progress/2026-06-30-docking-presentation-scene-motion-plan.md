---
type: Work Progress
title: Docking presentation scene and motion model implementation
status: verified
timestamp: 2026-06-30T16:45:00+08:00
related_plan: docs/plans/2026-06-30-002-refactor-docking-presentation-scene-motion-plan.md
related_adr: docs/adr/0010-docking-presentation-scene-motion-model.md
git_branch: refactor/docking-platform-hardening
source_session: 019ef563-9e5e-78f2-a6c7-23bf52b8993e
tags:
  - docking
  - presentation-scene
  - motion
  - ui-ux
---

# Summary

The docking presentation scene and motion model plan has a descriptor-first implementation in the
working tree. It targets capability alignment rather than pixel parity: flat presentation geometry,
explicit root overlay layers, semantic tab insertion preview, transition descriptors, zoom/focus
presentation state, divider/corner hit maps, accessibility descriptors, reduced-motion behavior, and
native dogfood proof text.

# Key Decisions

- `DockGraph` remains the persistent semantic authority.
- `DockPresentationScene` becomes the shared absolute geometry contract for render, hit testing, preview, motion, focus, and accessibility descriptors.
- Current-facts drop delivery remains authoritative; scenes and previews explain feedback but do not authorize releases.
- Motion is planned from previous and next scenes before per-frame animation polish.
- Zoom/unzoom is presentation state, not graph mutation.
- BonSplit and SuperSplit are capability references, not implementation templates.

# Implemented Surface

- `DockPresentationScene` resolves root/floating/empty-central panes, tab bars, tab labels,
  splitters, focus regions, and overlay anchors from `DockHostRenderSession`.
- `DockOverlayScene` gives stable layer identity for target body, guide boxes, tab insertion,
  payload tabs, route markers, and rejected state; target guide rendering now consumes overlay
  guide layers.
- Center/tab docking preview carries `DockPreviewTabInsertion` and renders
  `DropTabInsertionPreview`; edge/root previews suppress tab insertion.
- `DockTransitionPlan` describes pane enter/leave/move/resize, divider transitions, overlay
  tab-insertion/payload/rejected descriptors, and reduced-motion immediate behavior.
- `DockZoomState` and `DockZoomScene` model zoom/unzoom as presentation-only state with sibling
  egress edge descriptors and focus-region preservation.
- Zoomed presentation scenes now remap target tab bar, tab label, focus-region, and overlay-anchor
  geometry to the zoomed scene bounds, reusing the shared touching-edge preference rule used by
  transition geometry.
- `DockDividerHitMap` derives single-axis splitter targets and corner two-axis targets from the
  presentation scene.
- `DockAccessibilityScene` derives internal pane/tab/tab-panel/splitter/floating/drop/drag role
  descriptors from presentation and overlay scenes.
- The native runtime panel proof string now advertises
  `presentation-scene+overlay-layers+tab-insertion+motion+zoom+divider-hit-map+a11y+reduced-motion`.

# Verified State

- `cargo fmt --all -- --check` passed.
- `git diff --check` passed.
- `python3 /Users/frankorz/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering` passed.
- `cargo check -p open-gpui` passed.
- `cargo check --tests -p open-gpui-docking` passed.
- `cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests --no-fail-fast` passed: 24/24.
- `cargo nextest run -p open-gpui-docking host_zoom_focus_tests host_transition_tests --no-fail-fast` passed after the zoom geometry consistency fix: 7/7.
- `cargo nextest run -p open-gpui-docking --no-fail-fast` passed after the final code fix: 812/812.
- `cargo check -p open-gpui-docking-native` passed.
- `cargo nextest run -p open-gpui-docking-native --no-fail-fast` passed: 17/17.

# Current Repo State

- Branch: `refactor/docking-platform-hardening`.
- The working tree still contains prior docking/platform hardening changes and this implementation.
- New implementation files are intentionally crate-private descriptor models; platform animation and
  accessibility adapters remain follow-up work behind the same model.

# Next Action

Commit carefully after choosing the intended commit scope. The working tree still mixes prior
platform hardening changes with this presentation/motion implementation.

# Citations

- [Plan](../../plans/2026-06-30-002-refactor-docking-presentation-scene-motion-plan.md)
- [ADR](../../adr/0010-docking-presentation-scene-motion-model.md)
- [Prior preview authority plan](../../plans/2026-06-29-003-refactor-docking-preview-scene-authority-plan.md)
- [Platform hardening plan](../../plans/2026-06-30-001-refactor-docking-platform-hardening-plan.md)
- [Nested inner-edge verification](../verification/docking-nested-inner-edge-20260628.md)
- [Multi-viewport authority finding](../subagents/docking-multiviewport-authority-20260619.md)
