---
type: Work Progress
title: Docking affordance authority cleanup plan
status: planned
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

# Subagent Findings

- Overlay/affordance audit: `DockVisualAffordanceScene` is already used for transition and
  accessibility, but target drop-preview rendering still starts from `DockOverlayScene`; payload-tab
  layout is still overlay-named.
- Transition API audit: motion input is already `DockVisualAffordanceScene`, but transition plans,
  executor samples, host facade methods, debug regions, and tests still use overlay naming.
- Native diagnostics audit: the native runtime panel reads `DockHost` handles directly for visual
  affordance summaries instead of consuming a runtime status snapshot.
- Split/animation audit: `ui_core` split and motion primitives exist, but docking still has duplicate
  split render, divider hit-map identity, reveal, and bounds interpolation helpers.

# Planned Work

The new plan has five implementation units:

- U1: make target preview consume visual affordance descriptors.
- U2: rename overlay motion API to affordance motion API.
- U3: move visual diagnostics into runtime status.
- U4: collapse split and motion geometry duplication around existing `ui_core` primitives.
- U5: update ADR/progress/docs and run focused-to-broad verification.

# Next Action

Execute the plan via goal/`ce-work` on the current branch. Rust changes must follow local Rust
best-practice instructions, use `cargo fmt`, and prefer `cargo nextest` for tests.

# Citations

- [Plan](../../../plans/2026-07-03-002-refactor-docking-affordance-authority-cleanup-plan.md)
- [Previous visual affordance progress](2026-07-03-docking-visual-affordance-runtime.md)
