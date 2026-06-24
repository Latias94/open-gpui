---
type: Work Progress
title: Table column visibility controls
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
---

# Table Column Visibility Controls

- 2026-06-24: Completed U1 of the column visibility controls plan in the working tree.
  `TableColumnVisibility` now stores caller-owned sparse runtime overrides, `TableColumn` exposes
  `hideable` / `with_hideable`, `TableState` carries visibility through equality and cache keys,
  and the existing visible-column pipeline resolves effective visibility before order, pinning,
  sizing, and render-plan consumers.
- 2026-06-24: The U1 tests cover descriptor default overrides, showing a default-hidden column,
  hiding a default-visible column, retaining unknown ids as caller-owned state, protecting
  non-hideable default-visible columns from stale hidden overrides, preserving unrelated table
  state, and public export coverage through `ui_core`, `ui_components`, and both preludes.
- 2026-06-24: U1 verification passed with `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components`,
  `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table_public_exports_include_core_table_and_virtualizer_contracts crate_root_and_prelude_exports_remain_explicit`,
  and `git diff --check`.
- Next action: Implement U2 by adding the `TableColumnVisibility` component recipe, item metadata,
  visible / hidden counts, show-all / reset changes, and `apply_to` helpers for app-owned
  `TableState` updates.

# Citations

[1] [Plan](../../../plans/2026-06-24-008-feat-ui-table-column-visibility-controls-plan.md)
[2] TanStack reference `repo-ref/tanstack-table/packages/table-core/tests/unit/features/column-visibility/columnVisibilityFeature.utils.test.ts`
[3] Fret reference `repo-ref/fret/ecosystem/fret-ui-kit/src/imui/table_column_visibility/state/mutation.rs`
