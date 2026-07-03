---
type: Work Progress
title: UI motion value foundation plan
status: active
timestamp: 2026-07-04T02:50:03+08:00
git_branch: feat/ui-motion-value-foundation
related_plan: docs/plans/2026-07-04-001-refactor-ui-motion-value-foundation-plan.md
related_adr:
  - docs/adr/0017-ui-motion-value-foundation.md
git_commits:
  - 6d89aac docs(ui): plan motion value foundation
tags:
  - ui-core
  - motion
  - value
  - policy
  - splitter
  - docking
---

# Summary

Started the UI motion value foundation implementation on `feat/ui-motion-value-foundation`.

# Plan Boundary

- The plan compares Open GPUI motion against `repo-ref/motion` and accepts only the native Rust UI
  pieces that fit current consumers: explicit model resolution, scalar value/run state, frame-demand
  reasons, policy gates, and projection honesty.
- Keyframes, repeat, pause/seek/speed, grouped playback, public subscribers, dependent values,
  React hooks, DOM measurement, CSS/WAAPI behavior, and compositor-backed execution are deferred.
- New value/run APIs must have Splitter or docking consumer proof before being exported publicly.

# Current State

- `main` was merged and pushed through the previous UI motion spring foundation closeout before this
  branch started.
- Plan commit: `6d89aac docs(ui): plan motion value foundation`.
- ADR 0017 records the accepted value/model/run/policy boundary for this implementation round.

# Next Action

Implement U2 from the plan: normalize motion model/state/preset semantics, characterize projection
consumption, and stop hidden `MotionSpec` to spring conversion in Splitter and docking paths.

# Citations

- [Plan](../../../plans/2026-07-04-001-refactor-ui-motion-value-foundation-plan.md)
- [ADR 0017](../../../adr/0017-ui-motion-value-foundation.md)
- [Previous spring progress](2026-07-03-ui-motion-spring-foundation.md)
