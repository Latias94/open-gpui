---
type: Work Progress
title: UI motion spring foundation
status: verified
timestamp: 2026-07-03T23:59:00+08:00
git_branch: feat/ui-motion-spring-foundation
related_plan: docs/plans/2026-07-03-004-refactor-ui-motion-spring-foundation-plan.md
related_adr:
  - docs/adr/0016-ui-motion-spring-foundation.md
git_commits:
  - 25a4e11 docs(ui): record motion spring foundation boundary
  - 2215895 feat(ui-core): add deterministic spring motion sampler
  - 8667464 feat(ui-core): add scalar motion model sampling
  - dea4942 feat(ui-core): add layout projection primitives
  - 0703a34 feat(ui-core): add scalar motion controller
  - 08d5369 feat(ui-core): add motion policy validation
  - 0c96d2a refactor(docking): use spring projection motion primitives
tags:
  - ui-core
  - motion
  - spring
  - projection
  - splitter
  - docking
---

# Summary

Implemented the UI motion spring foundation plan on `feat/ui-motion-spring-foundation`.

# Shipped Capability

- `open_gpui_ui_core` now exposes deterministic spring sampling, scalar motion model sampling,
  layout projection data, scalar controller frame-demand contracts, and motion policy validation.
- Existing timeline sampling remains available and explicit custom timeline specs stay honored.
- `ui_components::Splitter` uses the shared scalar controller and default layout spring for
  programmatic changes while keeping pointer drag direct.
- `gpui_docking::DockTransitionExecutor` uses a shared scalar track for transition progress and
  projection-derived pane/divider move/resize geometry.
- Motion policy tests lock high-frequency bypass, duration budget, bounce budget, reduced-motion
  final semantics, and unrelated preview target behavior.
- The native runtime panel reports spring, projection, scalar controller, motion policy, and
  high-frequency bypass capability in the motion proof summary.

# Boundaries

- No native compositor backend, CoreAnimation bridge, public animation builder, DOM measurement,
  CSS strings, keyframes, or decorative animation framework was added.
- Motion samples remain presentation evidence. Docking release authority still comes from current
  drop facts.
- Same-identity retargeting may animate. Unrelated target preview geometry remains pinned to the
  current semantic target.

# Verification

See [verification evidence](../verification/ui-motion-spring-foundation-20260703.md). The focused
and broad plan gates passed on the feature branch.

# Citations

- [Plan](../../../plans/2026-07-03-004-refactor-ui-motion-spring-foundation-plan.md)
- [ADR 0016](../../../adr/0016-ui-motion-spring-foundation.md)
