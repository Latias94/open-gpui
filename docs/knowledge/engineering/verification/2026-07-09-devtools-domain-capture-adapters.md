---
type: "Verification Evidence"
title: "DevTools domain capture adapters"
timestamp: 2026-07-09T04:40:55Z
status: "passed"
related_plan: "docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md"
git_branch: "main"
git_commit: "07fc0f81"
verified_by: "cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked"
---

# Summary

Committed the U5 adapter-domain slice for the DevTools target/domain/runtime plan.

# Verified State

- Command registry, keybinding projection, and keymap resolution adapters now expose first-party `DevtoolsCapture` helpers.
- Form and resource adapters now expose data-domain captures with stable count summaries and preserved legacy snapshots.
- Layout and timeline snapshots now expose `capture()` methods plus module-level capture helpers.
- Existing `*_probe_snapshot`, `*_snapshot_envelope`, and closure-backed probe APIs remain intact.
- All new captures use runtime targets and preserve redaction-first snapshot output.

# Commands

- `cargo fmt -p open-gpui-devtools`
- `cargo check -p open-gpui-devtools --no-default-features --tests --locked`
- `cargo nextest run -p open-gpui-devtools --no-default-features --test layout_adapters --test timeline_adapters --no-fail-fast --locked`
- `cargo check -p open-gpui-devtools --all-features --tests --locked`
- `cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked`

# Result

All listed gates passed locally. The full all-features DevTools nextest run executed 51 tests with 51 passing.

# Next Action

Start U6 by deepening `DevtoolsInspectorState::from_capture()` navigation around targets, domains, events, diagnostics, and legacy snapshots.

# Citations

- `docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md`
- `crates/devtools/src/command.rs`
- `crates/devtools/src/form.rs`
- `crates/devtools/src/layout.rs`
- `crates/devtools/src/resource.rs`
- `crates/devtools/src/timeline.rs`
