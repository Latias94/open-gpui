---
type: Work Progress
title: Table column visibility controls
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
---

# Table Column Visibility Controls

- 2026-06-24: Completed U1 of the column visibility controls plan in the working tree.
  `TableColumnVisibilityOverrides` now stores caller-owned sparse runtime overrides, `TableColumn` exposes
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
- 2026-06-24: Completed U2 of the column visibility controls plan in the current branch.
  `TableColumnVisibility` now resolves item metadata, visible / hidden counts, show-all / reset
  actions, controlled open / visibility ownership, and `TableColumnVisibilityChange::apply_to`
  helpers while preserving unrelated `TableState` slices.
- 2026-06-24: U2 verification passed with
  `cargo nextest run -p open-gpui-ui-components table_column_visibility_state_resolves_items_counts_and_popover_contract table_column_visibility_change_updates_visibility_and_preserves_table_state table_public_exports_include_core_table_and_virtualizer_contracts crate_root_and_prelude_exports_remain_explicit component_api_inventory_uses_stable_ownership_vocabulary public_resolved_state_contracts_avoid_gpui_runtime_types`,
  `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table component_api_inventory crate_root_and_prelude_exports_remain_explicit public_resolved_state_contracts_avoid_gpui_runtime_types`,
  and `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components`.
- 2026-06-24: Completed U3/U4 in the current branch. `release-matrix` now renders a
  `TableColumnVisibility` toolbar control, keeps pinned identity/status columns locked, records
  app-owned visibility changes in `TableSampleRuntimeLog`, applies visibility overrides through the
  shared sample-runtime state path, and has focused smoke coverage for hiding/restoring a metric
  column plus popup wheel containment. `docs/ui/component-contract.md` and `docs/verification.md`
  now document the shipped contract.
- 2026-06-24: U3/U4 verification passed with
  `cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_column_visibility_updates_release_matrix components_page_samples_expose_component_metadata components_page_conformance_gates_reference_core_and_gallery_contracts components_page_table_samples_expose_virtualized_row_model_contract`,
  `cargo nextest run -p open-gpui-ui-components table_column_visibility component_api_inventory crate_root_and_prelude_exports_remain_explicit public_resolved_state_contracts_avoid_gpui_runtime_types`,
  `cargo nextest run -p open-gpui-ui-foundation-gallery table`,
  and `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`.
- Next action: Commit U3/U4, then choose the next Table maturity gap from the roadmap.

# Citations

[1] [Plan](../../../plans/2026-06-24-008-feat-ui-table-column-visibility-controls-plan.md)
[2] TanStack reference `repo-ref/tanstack-table/packages/table-core/tests/unit/features/column-visibility/columnVisibilityFeature.utils.test.ts`
[3] Fret reference `repo-ref/fret/ecosystem/fret-ui-kit/src/imui/table_column_visibility/state/mutation.rs`
