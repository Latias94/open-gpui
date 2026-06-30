---
type: Work Progress
title: Table global filtering and faceting planning
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
---

# Table global filtering and faceting planning

- 2026-06-24: Wrote `docs/plans/2026-06-24-007-feat-ui-table-global-filtering-faceting-plan.md`
  as the next Table boundary. The plan keeps global filtering state separate from column filters,
  derives global facet metadata from the pre-global-query row basis, and scopes the first
  component recipe to a controlled search input.
- 2026-06-24: The plan deliberately defers fuzzy ranking, operator menus, nested predicate
  builders, async search, and server-side query orchestration so the first slice stays aligned
  with the current component/productization boundary.
- 2026-06-24: Next action is to decide whether to start `ce-work` on this plan or deepen it
  further before implementation.

# Citations

[1] [Plan](../../../plans/2026-06-24-007-feat-ui-table-global-filtering-faceting-plan.md)
