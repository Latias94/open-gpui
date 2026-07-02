---
type: Work Progress
title: Docking render authority convergence planning
status: planned
timestamp: 2026-07-02T23:58:00+08:00
git_branch: main
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

Created the follow-up plan for docking render authority convergence after the flat motion runtime
and shared motion runtime work were merged to `main`.

# Decision

The next docking pass should not add another animation primitive.
It should reduce geometry drift by making `DockPresentationScene` the reference point for
deterministic render, drop-fact, divider, floating, tab-bar, zoom, and accessibility geometry.

Render probes remain acceptable only where GPUI text shaping or intrinsic tab-label measurement
is the real authority.
The plan makes that exception explicit so probe-only geometry does not spread back into root,
leaf, splitter, floating-title, or empty-space regions.

# Next Action

Start from U1 of the plan: add scene/render geometry parity tests for root split, nested pane,
floating container, empty central region, and zoomed scene.
Then migrate deterministic drop facts and splitter/chrome geometry in dependency order.

# Citations

- [Render authority convergence plan](../../../plans/2026-07-02-004-refactor-docking-render-authority-convergence-plan.md)
- [Docking flat motion runtime progress](2026-07-02-docking-flat-motion-runtime-plan.md)
- [UI motion runtime foundation progress](2026-07-02-ui-motion-runtime-foundation.md)
