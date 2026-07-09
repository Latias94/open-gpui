---
type: Current State
title: Open GPUI main integrated state
status: active
timestamp: 2026-07-09T16:05:45+08:00
git_branch: main
related_plan:
  - ../../plans/2026-07-09-002-refactor-v030-product-surface-hardening-plan.md
  - ../../plans/2026-07-09-003-refactor-devtools-runtime-ecosystem-plan.md
  - ../../plans/2026-07-08-003-refactor-runtime-docking-core-hardening-plan.md
verified_by:
  - docs/knowledge/engineering/verification/2026-07-09-devtools-inspector-click-dogfood.md
  - docs/knowledge/engineering/verification/2026-07-09-devtools-runtime-ecosystem-provider-controller.md
  - docs/verification.md
---

# Current State

- Snapshot timestamp: 2026-07-09T16:05:45+08:00.
- Branch: `main`.
- Current head: `df6ae2f` on local `main` and `origin/main`.
- Current work: v0.3 public-surface hardening, DevTools target/domain/event provider runtime, stateful GPUI DevTools inspector controller, gallery DevTools dogfood, and CI web-smoke hardening are integrated on `main`.
- Blocked: none. Latest GitHub Actions for `df6ae2f` are green for `Verify`, `Publish Check`, and `Dependency Health`.

# Integrated Summary

- Runtime/docking/core hardening from `docs/plans/2026-07-08-003-refactor-runtime-docking-core-hardening-plan.md` is present on `main`.
- v0.3 public-surface hardening from `docs/plans/2026-07-09-002-refactor-v030-product-surface-hardening-plan.md` is present on `main`.
- DevTools runtime ecosystem work from `docs/plans/2026-07-09-003-refactor-devtools-runtime-ecosystem-plan.md` is present on `main`.
- `open-gpui-devtools` exposes capture providers, scoped event recorders, target/domain/event capture DTOs, `DevtoolsInspectorState`, static `DevtoolsInspector`, and stateful `DevtoolsInspectorController`.
- The UI foundation gallery DevTools page collects through provider-backed capture and has a real click smoke for inspector rows/actions.
- The `CHANGELOG.md` `[Unreleased]` section is accumulating v0.3 user-facing migration notes while workspace crates remain at `0.2.0`.

# Current Entry Points

- DevTools runtime plan: `../../plans/2026-07-09-003-refactor-devtools-runtime-ecosystem-plan.md`.
- DevTools runtime verification: `verification/2026-07-09-devtools-runtime-ecosystem-provider-controller.md`.
- DevTools inspector click dogfood: `verification/2026-07-09-devtools-inspector-click-dogfood.md`.
- v0.3 public surface plan: `../../plans/2026-07-09-002-refactor-v030-product-surface-hardening-plan.md`.
- Verification matrix: `../../verification.md`.

# Next Action

- For implementation work, prefer v0.3 release readiness and public documentation/API drift gates before adding another large feature surface.
- For DevTools work, the next useful step is broader dogfood polish or browser-visible gallery inspection, not another provider abstraction layer.

# Citations

- [Runtime/docking/core hardening plan](../../plans/2026-07-08-003-refactor-runtime-docking-core-hardening-plan.md)
- [v0.3 public surface hardening plan](../../plans/2026-07-09-002-refactor-v030-product-surface-hardening-plan.md)
- [DevTools runtime ecosystem plan](../../plans/2026-07-09-003-refactor-devtools-runtime-ecosystem-plan.md)
- [DevTools runtime provider/controller verification](verification/2026-07-09-devtools-runtime-ecosystem-provider-controller.md)
- [DevTools inspector click dogfood verification](verification/2026-07-09-devtools-inspector-click-dogfood.md)
