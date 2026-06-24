---
type: Work Progress
title: Table custom aggregation callbacks completion
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
git_commit: dded73b
---

# Table custom aggregation callbacks completion

- 2026-06-24: Completed the Table custom aggregation callbacks slice as `dded73b`. `TableState`
  now stores named custom aggregation callbacks, grouped rows resolve named custom aggregates
  through the renderer-neutral pipeline, `TableRenderPlan` exposes the callback count, and the
  Components gallery includes a focused `grouped-custom-aggregation` sample.
- 2026-06-24: Verified the slice with `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table`, `cargo nextest run -p open-gpui-ui-foundation-gallery table`,
  and `git diff --check`.

# Citations

[1] [Plan](../../plans/2026-06-24-002-feat-ui-table-custom-aggregation-callbacks-plan.md)
[2] Commit `dded73b` - `feat(table): add named custom aggregation callbacks`
