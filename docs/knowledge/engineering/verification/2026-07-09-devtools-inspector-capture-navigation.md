---
type: "Verification Evidence"
title: "DevTools inspector capture navigation"
timestamp: 2026-07-09T04:54:13Z
status: "passed"
related_plan: "docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md"
git_branch: "main"
git_commit: "89587821"
verified_by: "CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked"
---

# Summary

Committed the U6 inspector navigation slice for the DevTools target/domain/runtime plan.

# Verified State

- `DevtoolsInspectorState` now has explicit target, domain, and event selection APIs.
- Filtering by an event can make the owning target and domain visible and selected.
- Selected detail priority is domain snapshot first, selected event second, then legacy snapshot fallback.
- Empty captures produce no selected detail and a model-level `NoSelectedDetail` error.
- GPUI inspector rendering exposes stable debug selectors for target list, domain list, event list, selected detail, diagnostics, and legacy snapshot rows.
- Copy/export labels and success feedback are exposed in the selected-detail DTO for model-level assertions.

# Commands

- `cargo fmt -p open-gpui-devtools`
- `cargo check -p open-gpui-devtools --no-default-features --tests --locked`
- `cargo nextest run -p open-gpui-devtools --test inspector_contracts --no-fail-fast --locked`
- `cargo check -p open-gpui-devtools --features gpui --tests --locked`
- `cargo check -p open-gpui-devtools --all-features --tests --locked`
- `CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked`

# Result

All listed gates passed locally. The first all-features nextest attempt hit Windows `link.exe` LNK1102 out-of-memory while linking test binaries; rerunning the same test gate with `CARGO_BUILD_JOBS=1` passed 53 tests with 53 passing.

# Next Action

Start U7 by wiring gallery dogfood to capture-derived inspector state, updating docs, and running the final verification contract.

# Citations

- `docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md`
- `crates/devtools/src/inspector.rs`
- `crates/devtools/src/gpui.rs`
- `crates/devtools/tests/inspector_contracts.rs`
