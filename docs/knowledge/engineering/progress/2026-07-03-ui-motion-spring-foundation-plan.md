---
type: Work Progress
title: UI motion spring foundation planning
status: active
timestamp: 2026-07-03T23:40:00+08:00
git_branch: feat/ui-motion-spring-foundation
related_plan: docs/plans/2026-07-03-004-refactor-ui-motion-spring-foundation-plan.md
related_adr:
  - docs/adr/0015-ui-motion-runtime-foundation.md
  - docs/adr/0016-ui-motion-spring-foundation.md
tags:
  - ui-core
  - motion
  - spring
  - projection
  - docking
  - splitter
---

# Summary

`docs/plans/2026-07-03-004-refactor-ui-motion-spring-foundation-plan.md` is the active
implementation plan for the next Open GPUI motion layer. The plan extends the ADR 0015 timeline
runtime with a narrow renderer-neutral spring/projection/controller/policy foundation.

# Boundary

- `open_gpui_ui_core` owns deterministic motion math, projection data, frame-demand vocabulary, and
  policy validation.
- GPUI adapters own frame requests, render-layer application, live measurement, domain
  interpolation, and product semantics.
- `open_gpui_ui_components::Splitter` keeps pointer drag immediate and may use shared motion only
  for programmatic changes.
- `open_gpui_docking` keeps release authority in current facts; motion remains presentation
  evidence only.
- Docking preview geometry must stay pinned to the current semantic target when identities change.

# Implementation Notes

The work should land in dependency order:

1. Record the ADR 0016 boundary.
2. Add deterministic spring sampling and a shared motion contract in `ui_core`.
3. Add layout projection primitives in `ui_core`.
4. Add grouped motion/controller frame-demand vocabulary.
5. Add policy validators before broad adapter migration.
6. Migrate Splitter and docking without regressing pointer immediacy or pinned previews.
7. Update native proof, verification memory, and cleanup duplicate helper scaffolding.

# Deferred

This plan still does not introduce native compositor backends, public animation builders, keyframes,
decorative motion, DOM measurement, CSS strings, or pixel-perfect reference matching.

# Citations

- [Plan](../../../plans/2026-07-03-004-refactor-ui-motion-spring-foundation-plan.md)
- [ADR 0015](../../../adr/0015-ui-motion-runtime-foundation.md)
- [ADR 0016](../../../adr/0016-ui-motion-spring-foundation.md)
