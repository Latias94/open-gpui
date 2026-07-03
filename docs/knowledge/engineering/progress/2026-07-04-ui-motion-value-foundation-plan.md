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
  - 9a6e119 docs(ui): record motion value boundary
  - 1e52b12 refactor(ui): make motion model resolution explicit
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
- U2 is implemented: `MotionRunState` is the shared state name with `MotionTimelineState` retained
  as a compatibility alias; `MotionPreset` resolves explicit default spring presets; Splitter and
  docking custom `MotionSpec` paths remain timeline-backed; default Splitter and docking spring
  behavior now uses explicit preset/model entry points.
- U2 verification so far: `cargo nextest run -p open-gpui-ui-core motion --no-fail-fast` passed;
  focused `cargo test --lib -- --exact` checks passed for Splitter custom timeline/reduced motion
  and docking custom timeline/default preset tests; `cargo check -p open-gpui-ui-core -p
  open-gpui-ui-components -p open-gpui-docking` passed. Wider nextest filters for
  `open-gpui-ui-components splitter` and `open-gpui-docking host_transition_tests` were interrupted
  after hanging without failure output and should be retried later or replaced with narrower gates.

# Next Action

Implement U3 from the plan: add proof-gated scalar value/run state only if Splitter or docking has a
real consumer path; otherwise keep it private or delete it.

# Citations

- [Plan](../../../plans/2026-07-04-001-refactor-ui-motion-value-foundation-plan.md)
- [ADR 0017](../../../adr/0017-ui-motion-value-foundation.md)
- [Previous spring progress](2026-07-03-ui-motion-spring-foundation.md)
