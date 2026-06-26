---
type: Work Progress
title: Table autosize by content completion
status: complete
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: feat/table-nested-headers
related_plan: docs/plans/2026-06-25-001-feat-ui-table-autosize-by-content-plan.md
---

# Summary

- The Table autosize-by-content boundary is complete and shipped as commit `10ec7b3`
  (`feat(table): support content-fit width growth`).
- The plan treats content-fit sizing as a renderer-neutral column policy with adapter-owned
  visible-width measurement.
- Manual sizing stays authoritative, and the first slice keeps wrapped or rich editor measurement
  out of scope.
- The core width policy, render-plan policy exposure, adapter-owned measured widths, Components
  gallery `content-fit-release` sample, and focused component/gallery verification are complete.
- The next component-depth boundary is the text input editor family plan, because richer Table
  editors should compose official editor primitives instead of growing a Table-owned editor engine.

# Next Action

- Start U1 of
  `docs/plans/2026-06-25-002-feat-ui-text-input-editor-family-plan.md`: value/display projection
  helpers for single-line text input.

# Citations

[1] [Plan](../../../plans/2026-06-25-001-feat-ui-table-autosize-by-content-plan.md)
[2] Commit `10ec7b3` - `feat(table): support content-fit width growth`
[3] [Text input editor family plan](../../../plans/2026-06-25-002-feat-ui-text-input-editor-family-plan.md)
