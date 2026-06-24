---
type: Work Progress
title: Table filter operators
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
git_commit: ae798e7
related_plan: docs/plans/2026-06-24-009-feat-ui-table-filter-operators-plan.md
---

# Summary

- The next Table maturity boundary is richer built-in filter operators over the existing
  column-filter pipeline.
- The plan is already written and committed as `f4a0af7`.
- The core U1/U2 slice is already implemented and committed as `ae798e7`.

# Verified State

- `TableFilterKind` now includes explicit text and numeric comparison operator variants alongside
  the existing contains / one-of / range forms.
- `ui_core` and `ui_components` export the new operator types through crate roots and preludes.
- Focused `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components` passed.
- Focused `cargo nextest run -p open-gpui-ui-core table` passed.
- Focused `cargo nextest run -p open-gpui-ui-components crate_root_and_prelude_exports_remain_explicit table_public_exports_include_core_table_and_virtualizer_contracts component_api_inventory` passed.

# Open Threads

- U3 still needs the controlled `TablePredicateFilter` recipe and payload.
- U4 still needs the Components gallery proof.
- U5 still needs contract, verification, and memory updates for the predicate slice.

# Next Action

Implement U3: add the controlled predicate-filter recipe and payload.

# Citations

[1] [Plan](../../../plans/2026-06-24-009-feat-ui-table-filter-operators-plan.md)
[2] Commit `f4a0af7` - `docs(plan): add table filter operators plan`
[3] Commit `ae798e7` - `feat(ui-core): add table filter operators`
