---
type: Work Progress
title: Table autosize by content planning
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: feat/table-nested-headers
related_plan: docs/plans/2026-06-25-001-feat-ui-table-autosize-by-content-plan.md
---

# Summary

- The next Table maturity boundary is autosize-by-content.
- The plan treats content-fit sizing as a renderer-neutral column policy with adapter-owned
  visible-width measurement.
- Manual sizing stays authoritative, and the first slice keeps wrapped or rich editor measurement
  out of scope.
- U1 is complete in the working tree: the core width policy and render-plan policy exposure are
  in place, the Components gallery now proves visible edits widen the `content-fit-release`
  sample, and the focused core/components/table checks passed.
- The long-running Table maturity goal remains active.

# Next Action

- Commit the content-fit slice and decide the next Table maturity boundary.

# Citations

[1] [Plan](../../../plans/2026-06-25-001-feat-ui-table-autosize-by-content-plan.md)
