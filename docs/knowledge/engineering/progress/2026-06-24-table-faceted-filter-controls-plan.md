---
type: Work Progress
title: Table faceted filter controls planning
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
git_commit: a52751f
---

# Table faceted filter controls planning

- 2026-06-24: Completed U1 of the faceted filter controls plan as `a52751f`.
  `TableFilter` now supports kind-based filters with exact categorical token sets, while the
  existing `contains` behavior stays intact.
- 2026-06-24: Wrote `docs/plans/2026-06-24-004-feat-ui-table-faceted-filter-controls-plan.md`
  as the next Table follow-up boundary. The plan keeps scope to single-column categorical faceted
  filter controls, exact categorical `TableFilter` semantics, Popover + command-palette UI
  composition, and a Components gallery proof.
- 2026-06-24: Deferred global faceting, numeric range sliders, async facet loading, a general
  predicate builder, and standalone headless extraction.
- Next action: Execute U2 by adding the faceted filter recipe in `ui_components`.

# Citations

[1] [Plan](../../plans/2026-06-24-004-feat-ui-table-faceted-filter-controls-plan.md)
[2] [Faceting metadata plan](../../plans/2026-06-23-008-feat-ui-table-faceting-filter-metadata-plan.md)
