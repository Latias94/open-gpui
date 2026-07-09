---
type: "Verification Evidence"
title: "DevTools target-domain capture core slice"
timestamp: 2026-07-09T04:08:30Z
status: "passed"
related_plan: "docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md"
git_branch: "main"
git_commit: "37eab2e5"
verified_by: "cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked"
---

# Summary

Committed and pushed the U1-U3 foundation slice for the DevTools target/domain/runtime plan.

# Verified State

- `DevtoolsCapture`, target/domain DTOs, legacy probe projection, capture-level identity diagnostics, and `DevtoolsRegistry::collect_capture()` are present.
- `DevtoolsEventRecorder` retains the latest bounded events with append-time sequence ordering and omitted count.
- `TimelineSnapshot::from_event_batch()` projects retained events with timestamp, duration, target/domain metadata, retained count, omitted count, and recorder limit.
- The active plan was revised after headless doc review to include the MVP checkpoint, registry semantics, feature gates, event ordering, docking accessor gate, and inspector navigation/accessibility requirements.

# Commands

- `cargo check -p open-gpui-devtools --no-default-features --tests --locked`
- `cargo nextest run -p open-gpui-devtools --test target_domain_contracts --test event_recorder_contracts --test snapshot_contracts --no-fail-fast --locked`
- `cargo check -p open-gpui-devtools --all-features --tests --locked`
- `cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked`

# Result

All listed gates passed locally. The slice was pushed to `origin/main` at `37eab2e5`.

# Next Action

Start U4 by restructuring `crates/devtools/src/docking.rs` around target/domain capture output while keeping `docking_runtime_probe_snapshot(status)` as the compatibility wrapper.

# Citations

- `docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md`
- `crates/devtools/src/domain.rs`
- `crates/devtools/src/event.rs`
- `crates/devtools/src/target.rs`
