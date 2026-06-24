---
type: Work Progress
title: Table column groups and nested headers
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
base_commit: 0a2d767
related_plan: docs/plans/2026-06-24-010-feat-ui-table-column-groups-nested-headers-plan.md
---

# Summary

- The next Table maturity boundary is column groups and nested headers.
- The plan is written at `docs/plans/2026-06-24-010-feat-ui-table-column-groups-nested-headers-plan.md`.
- The chosen design keeps `TableColumn` as the behavioral leaf descriptor and adds a separate
  group/tree contract for header structure. Sorting, filtering, faceting, editing, visibility,
  pinning, and sizing remain leaf-column behavior.

# Verified State

- Current `ui_core::TableState` stores flat `Vec<TableColumn>` descriptors and resolves visible
  leaf columns through visibility, ordering, pinning, and sizing.
- Current `ui_components::TableRenderPlan` renders one fixed header row from
  `TableColumnRenderPlan` and does not expose header-group rows.
- Local references are available:
  `repo-ref/tanstack-table/packages/table-core/src/core/headers/buildHeaderGroups.ts` and
  `repo-ref/fret/ecosystem/fret-ui-headless/src/table/headers.rs`.

# Open Threads

- U1 still needs the core column tree descriptors and normalized leaf projection.
- U2 still needs renderer-neutral header group resolution.
- U3/U4 still need render-plan and GPUI multi-row header work.
- U5/U6 still need gallery proof, contract docs, verification docs, and memory completion.

# Next Action

Implement U1: add column tree descriptors while keeping the existing flat `with_columns` API and
leaf `columns()` projection stable.

# Citations

[1] [Plan](../../../plans/2026-06-24-010-feat-ui-table-column-groups-nested-headers-plan.md)
[2] Commit `0a2d767` - `docs(knowledge): record table predicate filters`
