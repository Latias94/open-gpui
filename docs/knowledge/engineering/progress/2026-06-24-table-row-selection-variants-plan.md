---
type: Work Progress
title: Table row selection variants planning
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
---

# Table row selection variants planning

- 2026-06-24: Chose the next Table follow-up boundary as row selection variants instead of continuing with two-axis virtualization. The new plan is `docs/plans/2026-06-24-003-feat-ui-table-row-selection-variants-plan.md`.
- 2026-06-24: Scoped the slice to checkbox, radio, and list-like selection recipes over the existing stable selected-row id state.
- 2026-06-24: Kept the contract renderer-neutral in `ui_core`, kept gestures and selection chrome in `ui_components`, and deferred cell editing, server-synced selection persistence, and a general feature plugin system.

# Citations

[1] [Plan](../../plans/2026-06-24-003-feat-ui-table-row-selection-variants-plan.md)
[2] Session `019ec6c8-5566-7062-8458-21ebe1360573`
