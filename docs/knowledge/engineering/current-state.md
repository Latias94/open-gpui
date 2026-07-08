---
type: Current State
title: Open GPUI main integrated state
status: active
timestamp: 2026-07-08T19:15:00+08:00
git_branch: main
related_plan:
  - ../../plans/2026-07-08-003-refactor-runtime-docking-core-hardening-plan.md
  - ../../plans/2026-07-08-002-feat-devtools-form-resource-ecosystem-plan.md
  - ../../plans/2026-07-07-002-refactor-post-v020-stabilization-plan.md
verified_by:
  - docs/knowledge/engineering/verification/open-gpui-devtools-form-resource-ecosystem-20260708.md
  - docs/verification.md
---

# Current State

- Snapshot timestamp: 2026-07-08T19:15:00+08:00.
- Branch: `main`.
- Current work: runtime/docking/core hardening and devtools/form/resource ecosystem work are pushed to `origin/main`; a follow-up cleanup batch is narrowing UI form-control ownership, adding a closure-backed DevTools probe adapter, and sharing gallery ecosystem runtime logs.
- Blocked: none. Broad local full-workspace checks can still stall on this workstation after heavy cargo work, so CI remains the owner for full-workspace confirmation.

# Integrated Summary

- Runtime/docking/core hardening from `docs/plans/2026-07-08-003-refactor-runtime-docking-core-hardening-plan.md` is present on `main`.
- DevTools/form/resource ecosystem work from `docs/plans/2026-07-08-002-feat-devtools-form-resource-ecosystem-plan.md` is present on `main`.
- `open-gpui-devtools`, `open-gpui-form`, and `open-gpui-resource` are first-party workspace crates.
- UI components now include the first-party `FormControlState` helper through root/prelude/primitives imports and ecosystem `form_adapter` / `resource_adapter` helpers.
- The Components gallery has deterministic form/resource adapter samples, and the DevTools page demonstrates redacted read-only inspection.

# Current Entry Points

- Ecosystem progress: `progress/2026-07-08-open-gpui-devtools-form-resource-ecosystem-final.md`.
- Ecosystem verification: `verification/open-gpui-devtools-form-resource-ecosystem-20260708.md`.
- Verification matrix: `../../verification.md`.

# Citations

- [Runtime/docking/core hardening plan](../../plans/2026-07-08-003-refactor-runtime-docking-core-hardening-plan.md)
- [Devtools/form/resource ecosystem plan](../../plans/2026-07-08-002-feat-devtools-form-resource-ecosystem-plan.md)
- [Final ecosystem progress](progress/2026-07-08-open-gpui-devtools-form-resource-ecosystem-final.md)
- [Final ecosystem verification](verification/open-gpui-devtools-form-resource-ecosystem-20260708.md)
