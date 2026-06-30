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

- U4 is complete in the working tree.
- U5 is complete in the working tree.
- `ui_core` now has `TableColumnGroupId`, `TableColumnNode`, and `TableColumnGroup` plus a
  normalized column-tree `TableState` contract, and `TableRenderPlan` exposes nested header-group
  render metadata.
- Duplicate leaf ids are pruned deterministically during tree normalization, the cache key includes
  the normalized tree shape, and the GPUI table adapter now renders multi-row nested headers while
  preserving leaf sort and resize behavior.
- `release-matrix` now serves as the gallery proof: the sample uses a grouped column tree, the
  table summary reports header rows / visible groups / leaf counts, and the focused gallery smoke
  proves the center lane scrolls while the pinned header families stay mounted.
- `ui_components` root and prelude exports include the new types, and focused core / component
  nextest checks pass.
- The next Table maturity boundary is the commit decision for this slice, then the next Table
  header maturity slice.
- The plan is written at `docs/plans/2026-06-24-010-feat-ui-table-column-groups-nested-headers-plan.md`.
- The chosen design keeps `TableColumn` as the behavioral leaf descriptor and adds a separate
  group/tree contract for header structure. Sorting, filtering, faceting, editing, visibility,
  pinning, and sizing remain leaf-column behavior.

# Verified State

- Current `ui_core::TableState` stores flat `Vec<TableColumn>` descriptors and resolves visible
  leaf columns through visibility, ordering, pinning, and sizing.
- Current `ui_components::TableRenderPlan` exposes nested header-group rows and the GPUI adapter
  renders them in multi-row region-aware lanes.
- Local references are available:
  `repo-ref/tanstack-table/packages/table-core/src/core/headers/buildHeaderGroups.ts` and
  `repo-ref/fret/ecosystem/fret-ui-headless/src/table/headers.rs`.

# Open Threads

- The remaining question is whether to commit this slice now or keep batching it with the next
  Table header follow-up.

# Next Action

Decide whether to commit this slice now or keep batching it with the next Table header follow-up.

# Citations

[1] [Plan](../../../plans/2026-06-24-010-feat-ui-table-column-groups-nested-headers-plan.md)
[2] Commit `0a2d767` - `docs(knowledge): record table predicate filters`
