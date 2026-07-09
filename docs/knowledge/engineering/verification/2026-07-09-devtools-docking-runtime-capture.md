---
type: "Verification Evidence"
title: "DevTools docking runtime capture"
timestamp: 2026-07-09T04:32:02Z
status: "passed"
related_plan: "docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md"
git_branch: "main"
git_commit: "d7c92768"
verified_by: "cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked"
---

# Summary

Committed the U4 docking runtime capture slice for the DevTools target/domain/runtime plan.

# Verified State

- `docking_runtime_capture(status)` projects `DockViewportRuntimeStatus` into a runtime target, a docking domain, sanitized event records, and the legacy docking snapshot.
- `docking_runtime_probe_snapshot(status)` remains available as a compatibility wrapper over the new capture path.
- Docking lifecycle and visual-affordance records now appear as structured DevTools targets/events without reaching into private runtime state.
- The adapter keeps DevTools local, read-only, renderer-neutral, and redaction-first; private `should_close` outcome details remain opaque because docking does not publicly export their concrete types.

# Commands

- `cargo fmt -p open-gpui-devtools`
- `cargo check -p open-gpui-devtools --features docking --tests --locked`
- `cargo nextest run -p open-gpui-devtools --features docking --test framework_adapters --no-fail-fast --locked`
- `cargo check -p open-gpui-devtools --all-features --tests --locked`
- `cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked`

# Result

All listed gates passed locally. The full all-features DevTools nextest run executed 51 tests with 51 passing.

# Next Action

Start U5 by replacing command/layout/timeline/form/resource flat wrappers with first-party domain capture helpers while preserving legacy snapshot adapters for compatibility.

# Citations

- `docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md`
- `crates/devtools/src/docking.rs`
- `crates/devtools/src/target.rs`
- `crates/devtools/tests/framework_adapters.rs`
