---
type: Work Progress
title: Docking split motion primitive refactor
status: active
timestamp: 2026-06-30T19:30:00+08:00
source_session: 019f09c8-a122-7d42-b250-053c40f9c513
git_branch: feat/docking-split-motion-primitives
related_plan: docs/plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md
related_adr:
  - docs/adr/0010-docking-presentation-scene-motion-model.md
  - docs/adr/0011-docking-split-motion-primitive-boundary.md
tags:
  - docking
  - split
  - motion
  - accessibility
  - ui-core
---

# Summary

The docking split motion primitive refactor is in final U10 cleanup. The implementation keeps
domain state, primitive layout, GPUI adapters, and docking presentation state separated:

- `open_gpui_ui_core` owns split layout scenes, splitter hit maps, motion descriptors, and
  renderer-neutral accessibility vocabulary.
- `open_gpui_ui_components` adapts those primitives to GPUI rendering and accessibility helpers.
- `open_gpui_docking` owns `DockGraph` semantics, presentation scenes, overlay scenes, transition
  descriptors, zoom/focus presentation, divider/corner runtime, and graph-validated resize
  transactions.

# Completed Commits

- `448c910` - plan split motion primitive refactor.
- `d23f360` - harden splitter state contracts.
- `fc5333c` - extract split primitives into core.
- `1a21692` - add shared motion descriptors.
- `1aa06fc` - resolve docking presentation splits through core primitives.
- `c85aaab` - drive tab previews from overlay scene.
- `27fd30f` - add presentation zoom and motion execution.
- `8373b99` - unify divider resize transactions.

# Current U10 Work

- Accessibility descriptors now carry orientation, selected state, disabled state, and action
  descriptors for panes, tabs, tab bars, splitters, drop targets, drag sources, payloads, and
  rejected targets.
- `open_gpui_ui_core::Role::Splitter` and the GPUI role adapter are covered by focused tests.
- The old docking-local split handle-center and handle-hit geometry helpers were removed. Graph
  layout keeps only pane-bound calculation; divider hit testing is owned by presentation-scene
  splitters plus `SplitterHitMap`.
- ADR 0011 records the primitive boundary between `ui_core`, `ui_components`, and docking.
- Component and verification docs now describe docking split/motion primitive gates.

# Next Action

Run the plan's required gates, perform the required `ce-code-review` pass, apply eligible findings,
record final verification evidence, and commit the U10 docs/code cleanup.

# Citations

- [Plan](../../../plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md)
- [ADR 0010](../../../adr/0010-docking-presentation-scene-motion-model.md)
- [ADR 0011](../../../adr/0011-docking-split-motion-primitive-boundary.md)
- [Verification evidence](../verification/docking-split-motion-primitives-20260630.md)
