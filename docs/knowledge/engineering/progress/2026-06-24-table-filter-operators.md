---
type: Work Progress
title: Table filter operators
status: complete
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
git_commit: ecc5f45
related_plan: docs/plans/2026-06-24-009-feat-ui-table-filter-operators-plan.md
---

# Summary

- The Table filter-operators plan is complete through core operators, component recipe, gallery
  proof, contract docs, verification docs, and engineering memory.
- The plan was committed as `f4a0af7`, core U1/U2 landed as `ae798e7`, the component recipe landed
  as `82997fe`, and the gallery proof landed as `ecc5f45`.
- `TablePredicateFilter` is now the official one-column operator/value recipe for text and numeric
  leaf predicates. Nested AND/OR predicate builders, saved views, server query compilation, and
  custom callback registries remain deferred.

# Verified State

- `TableFilterKind` now includes explicit text and numeric comparison operator variants alongside
  the existing contains / one-of / range forms.
- `ui_core` and `ui_components` export the new operator types through crate roots and preludes.
- `TablePredicateFilter`, `TablePredicateFilterState`, `TablePredicateFilterOperator`,
  `TablePredicateFilterOperatorOptionState`, and `TablePredicateFilterChange` are exported through
  the component crate root and prelude.
- The Components gallery `filter-board` sample now renders a name predicate filter, records
  controlled predicate payloads in `TableSampleRuntimeLog`, applies them to sample-owned
  `TableState`, and proves the rendered row window follows the core filtered row model.
- Focused `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components` passed.
- Focused `cargo nextest run -p open-gpui-ui-core table` passed.
- Focused `cargo nextest run -p open-gpui-ui-components crate_root_and_prelude_exports_remain_explicit table_public_exports_include_core_table_and_virtualizer_contracts component_api_inventory` passed.
- Focused `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery` passed.
- Focused `cargo nextest run -p open-gpui-ui-components table_predicate_filter` passed.
- Focused `cargo nextest run -p open-gpui-ui-components component_api_inventory crate_root_and_prelude_exports_remain_explicit table_public_exports_include_core_table_and_virtualizer_contracts public_resolved_state_contracts_avoid_gpui_runtime_types` passed.
- Focused `cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_predicate_filter_updates_table_rows` passed.
- Focused `cargo nextest run -p open-gpui-ui-foundation-gallery official_component_catalog_entries_have_signals_and_sample_selectors state_contract_catalog_entries_have_signals_and_readout_selectors components_page_table_samples_expose_virtualized_row_model_contract` passed.

# Open Threads

- Nested AND/OR predicate builders, query-builder UI, saved views, URL persistence, server query
  compilation, fuzzy ranking/highlighting, and date/time-specific operators remain future work.
- The next Table maturity boundary should be selected separately; likely candidates are
  column-group / nested-header polish, richer editor families, or a data-source ergonomics layer.

# Next Action

Select and plan the next Table maturity slice.

# Citations

[1] [Plan](../../../plans/2026-06-24-009-feat-ui-table-filter-operators-plan.md)
[2] Commit `f4a0af7` - `docs(plan): add table filter operators plan`
[3] Commit `ae798e7` - `feat(ui-core): add table filter operators`
[4] Commit `82997fe` - `feat(ui-components): add table predicate filter recipe`
[5] Commit `ecc5f45` - `feat(gallery): prove table predicate filters`
