---
type: Work Progress
title: Docking flat motion runtime framework planning
status: planned
timestamp: 2026-07-02T12:45:00+08:00
git_branch: refactor/docking-flat-motion-runtime
related_plan: docs/plans/2026-07-02-002-refactor-docking-flat-motion-runtime-plan.md
related_adr:
  - docs/adr/0010-docking-presentation-scene-motion-model.md
  - docs/adr/0011-docking-split-motion-primitive-boundary.md
  - docs/adr/0012-docking-runtime-capability-alignment.md
tags:
  - docking
  - motion
  - animation
  - split
  - ui-ux
---

# Summary

Created the next docking UI/UX plan after the descriptor-first and runtime-capability passes.
The new plan treats ADR 0010, ADR 0011, and ADR 0012 as the accepted boundary and focuses on the remaining runtime-quality gap: flat render authority, real pane-content reveal instead of placeholder transition rectangles, interruptible transition retargeting, stronger shared motion vocabulary, overlay stability, programmatic split motion, and zoom/focus polish.

# Current Baseline

- Commit `4b238ac` fixed root-edge hover guide affordances by keeping passive inner side guides visible under the active outer root-edge target.
- `open_gpui_ui_core` already has split and motion primitives.
- `open_gpui_docking` already has presentation, overlay, transition, zoom/focus, divider hit-map, and accessibility descriptor modules.
- The next plan does not repeat descriptor extraction; it turns those descriptors into runtime render and motion authority.

# Key Planning Decisions

- New plan rather than updating the June 30 plans, because the older plans are now baseline/history and this one supersedes them for the runtime animation layer.
- No new ADR yet. Add ADR 0013 only if implementation changes the accepted primitive/executor boundary from ADR 0011 or ADR 0012.
- Capability parity remains the target, not pixel parity with ImGui, BonSplit, SuperSplit, or macOS.
- Pointer drag remains immediate; committed and programmatic layout changes may animate.

# Next Action

Run implementation with `ce-work` or goal mode against the new plan after the user chooses execution.

# Citations

- [Plan](../../../plans/2026-07-02-002-refactor-docking-flat-motion-runtime-plan.md)
- [ADR 0010](../../../adr/0010-docking-presentation-scene-motion-model.md)
- [ADR 0011](../../../adr/0011-docking-split-motion-primitive-boundary.md)
- [ADR 0012](../../../adr/0012-docking-runtime-capability-alignment.md)
- [Docking presentation scene plan](../../../plans/2026-06-30-002-refactor-docking-presentation-scene-motion-plan.md)
- [Docking split motion primitive plan](../../../plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md)
