---
type: Work Progress
title: Table numeric range filter controls
status: completed
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
---

# Table numeric range filter controls

- 2026-06-24: Completed the core numeric range slice in `ui_core`. `TableFilterKind` now carries
  inclusive finite numeric bounds through `TableNumericFilterBound`, reversed endpoints normalize,
  and non-finite or blank endpoints stay out of the contract.
- 2026-06-24: Productized `TableRangeFilter` in `ui_components`. The recipe composes `Popover`,
  `TextInput`, and `Button` primitives, keeps partially typed min/max text in adapter runtime,
  and emits controlled `TableRangeFilterChange` payloads that preserve unrelated filters while
  resetting pagination.
- 2026-06-24: Added the focused `filter-board` gallery proof. The sample now renders a score range
  control, records change payloads in `TableSampleRuntimeLog`, and proves popup wheel input stays
  local while the table row window narrows.
- 2026-06-24: Updated `docs/ui/component-contract.md` and `docs/verification.md` so numeric range
  filtering is documented as a shipped Table recipe alongside the categorical faceted filter.
- 2026-06-24: Verified the slice with targeted `cargo fmt`, focused `cargo nextest run` commands
  for `ui_core`, `ui_components`, and the gallery smoke, gallery contract checks, engineering wiki
  validation, and `git diff --check`.
- 2026-06-24: Next action is selecting the next Table boundary.

# Citations

[1] [Plan](../../../plans/2026-06-24-006-feat-ui-table-numeric-range-filter-controls-plan.md)
[2] [Component contract](../../../ui/component-contract.md)
[3] [Verification](../../../verification.md)
