---
type: "Work Progress"
title: "DevTools inspector category projections"
description: "U1 completed for DevTools ecosystem deepening."
timestamp: 2026-07-09T01:53:53Z
tags: ["devtools", "inspector", "ce-work"]
related_plan: "docs/plans/2026-07-09-001-feat-devtools-ecosystem-deepening-plan.md"
git_branch: "feat/devtools-ecosystem-deepening"
git_commit: "139cb14a"
verified_by: "cargo nextest run -p open-gpui-devtools inspector --no-fail-fast --locked; cargo check -p open-gpui-devtools --tests --locked; cargo check -p open-gpui-devtools --all-features --tests --locked"
---

# Summary

- Completed U1 inspector category projection.
- Added `DevtoolsSnapshotCategory`, `DevtoolsSnapshotCategorySummary`, category labels on `DevtoolsSnapshotRow`, and `category_summaries()`.
- Added first-class `SnapshotKind::Timeline` and `SnapshotKind::Layout` so later adapters can share the same projection surface.

# Details

- Category filtering now matches probe id, kind label, category label, and snapshot tree nodes.
- Diagnostics participate in category summaries through the `diagnostic` category when they match the active filter.
- Tests cover data summaries, command/timeline/layout/custom classification, and selection movement when filtering by category.

# Next Action

- Start U2: register command snapshots in the gallery through existing `open_gpui_devtools::command` adapters and update gallery contract tests.

# Citations

- Commit: `139cb14a`
- Plan: `docs/plans/2026-07-09-001-feat-devtools-ecosystem-deepening-plan.md`
- Files: `crates/devtools/src/inspector.rs`, `crates/devtools/src/snapshot.rs`, `crates/devtools/tests/inspector_contracts.rs`
