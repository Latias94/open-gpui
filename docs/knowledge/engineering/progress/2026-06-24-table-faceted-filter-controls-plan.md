---
type: Work Progress
title: Table faceted filter controls planning
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
git_commit: cfcab3a
---

# Table faceted filter controls planning

- 2026-06-24: Completed U2 of the faceted filter controls plan as `cfcab3a`.
  `TableFacetedFilter` now composes the official Popover, TextInput, Checkbox, Button, and
  ScrollArea primitives into a reusable single-column categorical filter recipe. The recipe exposes
  controlled/default open and query ownership, selected value inputs, popover placement / dismiss
  policy knobs, empty and clear labels, and `TableFacetedFilterChange` payloads that preserve
  unrelated filters while resetting pagination to the first page.
- 2026-06-24: U2 verification passed with
  `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components`,
  `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table`, and `git diff --check`.
- 2026-06-24: Completed U1 of the faceted filter controls plan as `a52751f`.
  `TableFilter` now supports kind-based filters with exact categorical token sets, while the
  existing `contains` behavior stays intact.
- 2026-06-24: Wrote `docs/plans/2026-06-24-004-feat-ui-table-faceted-filter-controls-plan.md`
  as the next Table follow-up boundary. The plan keeps scope to single-column categorical faceted
  filter controls, exact categorical `TableFilter` semantics, Popover + command-palette UI
  composition, and a Components gallery proof.
- 2026-06-24: Deferred global faceting, numeric range sliders, async facet loading, a general
  predicate builder, and standalone headless extraction.
- Next action: Execute U3 by adding a focused gallery proof and contract/verification evidence for
  the faceted filter recipe.

# Citations

[1] [Plan](../../plans/2026-06-24-004-feat-ui-table-faceted-filter-controls-plan.md)
[2] [Faceting metadata plan](../../plans/2026-06-23-008-feat-ui-table-faceting-filter-metadata-plan.md)
