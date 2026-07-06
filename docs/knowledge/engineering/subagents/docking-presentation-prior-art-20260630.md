---
type: Subagent Finding
title: Docking presentation prior art synthesis
status: complete
timestamp: 2026-06-30T16:00:00+08:00
subagent_names:
  - supersplit_model
  - bonsplit_research
  - docking_ux_audit
tags:
  - docking
  - supersplit
  - bonsplit
  - presentation-scene
---

# Finding

The durable prior-art lesson is a three-layer split:

- semantic layout stays tree/graph shaped;
- presentation geometry is resolved into a flat absolute scene;
- overlays and motion operate above that scene rather than inside individual panes.

SuperSplit adds the stronger motion and overlay model: root-level drop-zone overlays, previous/next scene transition planning, final-size split insertion, occlusion masks, focus presentation, zoom egress edges, corner drag, cross-window drag/drop, and accessibility integration.
BonSplit adds the tab/split interaction vocabulary closest to Open GPUI: controller commands, layout snapshots, tree snapshots, tab bar insertion indicators, pane focus navigation, zoom state, divider animation, and geometry notifications.

# Evidence

- User-provided SuperSplit notes from Mitchell Hashimoto's posts.
- `repo-ref/bonsplit/README.md`
- `repo-ref/bonsplit/Sources/Bonsplit/Public/BonsplitController.swift`
- `repo-ref/bonsplit/Sources/Bonsplit/Public/BonsplitDelegate.swift`
- `repo-ref/bonsplit/Sources/Bonsplit/Public/Types/LayoutSnapshot.swift`
- `repo-ref/bonsplit/Sources/Bonsplit/Internal/Views/TabBarView.swift`
- `repo-ref/bonsplit/Sources/Bonsplit/Internal/Views/PaneContainerView.swift`
- `repo-ref/bonsplit/Sources/Bonsplit/Internal/Utilities/SplitAnimator.swift`
- `crates/gpui_docking/src/drop_preview.rs`
- `crates/gpui_docking/src/drop_target.rs`
- `crates/gpui_docking/src/graph_layout.rs`
- `crates/gpui_docking/src/render.rs`
- `crates/gpui_docking/src/render_split.rs`

# Recommendation

Adopt the capability model, not the platform implementation.
Open GPUI should keep `DockGraph`, dock spaces, floating containers, current-facts release authority, and GPUI-native rendering, while adding a derived `DockPresentationScene`, root overlay layers, `DockTransitionPlan`, tab insertion preview, zoom/focus presentation state, scene-derived divider hit maps, and accessibility/reduced-motion descriptors.

# Disposition

Applied to `docs/plans/2026-06-30-002-refactor-docking-presentation-scene-motion-plan.md` and `docs/adr/0010-docking-presentation-scene-motion-model.md`.
