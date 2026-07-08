---
type: Current State
title: Open GPUI devtools form resource ecosystem state
status: active
timestamp: 2026-07-08T18:24:00+08:00
git_branch: feat/devtools-form-resource-ecosystem
related_plan:
  - ../../plans/2026-07-08-002-feat-devtools-form-resource-ecosystem-plan.md
verified_by:
  - cargo fmt -p open-gpui-form -p open-gpui-resource -p open-gpui-devtools -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery -p xtask --check
  - cargo check -p open-gpui-form -p open-gpui-resource -p open-gpui-devtools -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --tests --locked
  - cargo check -p open-gpui-devtools --features gpui --tests --locked
  - cargo nextest run -p open-gpui-form --no-fail-fast --locked -j 1
  - cargo nextest run -p open-gpui-resource --no-fail-fast --locked -j 1
  - cargo nextest run -p open-gpui-devtools --no-fail-fast --locked -j 1
  - cargo nextest run -p open-gpui-ui-components --no-fail-fast --locked -j 1
  - cargo nextest run -p open-gpui-ui-foundation-gallery --no-fail-fast --locked -j 1
  - cargo run -p xtask -- scan-ui-contract
  - cargo run -p xtask -- scan-doc-links
  - cargo run -p xtask -- verify-release-docs
  - git diff --check
---

# Current State

- Snapshot timestamp: 2026-07-08T18:24:00+08:00.
- Goal: finish `docs/plans/2026-07-08-002-feat-devtools-form-resource-ecosystem-plan.md`.
- Branch: `feat/devtools-form-resource-ecosystem`.
- Last verified implementation commit: `d70afc5c feat(ecosystem): add adoption docs and contract gates`.
- Current work: U1-U8 are implemented, documented, reviewed, and locally verified. No implementation work remains on the plan.
- Blocked: none.

# Integrated Summary

- Done: `open-gpui-devtools`, `open-gpui-form`, and `open-gpui-resource` are first-party workspace crates with crate READMEs, focused tests, and release-doc metadata.
- Done: DevTools owns read-only serializable snapshot DTOs, probe collection, diagnostics, redaction summaries, JSON export, and an optional GPUI inspector behind the `gpui` feature.
- Done: Form core owns renderer-neutral field identity, typed lenses, dirty/touched/visited meta, validation generations, debounce queues, submit/reset lifecycle, dynamic JSON values, and redacted snapshots.
- Done: Resource core owns renderer-neutral query keys, observers, generation-aware fetch state, retry policy, invalidation/refetch outcomes, pagination snapshots, mutation lifecycle, and redacted diagnostics.
- Done: UI components expose root-level form/resource adapter helpers while keeping adapter-only surfaces out of the common prelude.
- Done: The Components gallery has deterministic form/resource adapter samples, and the DevTools page demonstrates redacted read-only inspection.
- Done: `xtask verify` and `docs/verification.md` include the first-party ecosystem crate test gate.

# Current Entry Points

- Ecosystem plan: `docs/plans/2026-07-08-002-feat-devtools-form-resource-ecosystem-plan.md`.
- Form crate: `crates/form/README.md`.
- Resource crate: `crates/resource/README.md`.
- DevTools crate: `crates/devtools/README.md`.
- UI adapters: `crates/ui_components/README.md` and `docs/ui/component-contract.md`.
- Gallery samples: `cargo run -p open-gpui-ui-foundation-gallery` and `cargo run -p open-gpui-ui-foundation-gallery -- --page devtools`.
- Verification matrix: `docs/verification.md`.

# Historical Navigation

Older command, component, docking, motion, native UI framework, and post-v0.2.0 stabilization work remains available through `index.md`, `progress/`, `verification/`, `sessions/`, `subagents/`, and ADR links. Treat those files as historical evidence unless the current plan, README, changelog, workflow, or crate source confirms the same state.

# Citations

- [Devtools/form/resource ecosystem plan](../../plans/2026-07-08-002-feat-devtools-form-resource-ecosystem-plan.md)
- [Final ecosystem progress](progress/2026-07-08-open-gpui-devtools-form-resource-ecosystem-final.md)
- [Final ecosystem verification](verification/open-gpui-devtools-form-resource-ecosystem-20260708.md)
- [Verification guide](../../verification.md)
- [Root README](../../../README.md)
