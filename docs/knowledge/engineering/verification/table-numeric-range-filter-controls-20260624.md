---
type: Verification Evidence
title: Table numeric range filter controls verification
status: completed
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
---

# Table numeric range filter controls verification

- `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`
- `cargo nextest run -p open-gpui-ui-core numeric_range_filters_match_finite_number_cells_inclusively numeric_range_filters_normalize_open_and_reversed_bounds categorical_filters_match_exact_tokens_and_multiple_values`
- `cargo nextest run -p open-gpui-ui-components table_range_filter_state_resolves_bounds_and_popover_contract table_range_filter_change_updates_filters_and_resets_pagination table_render_plan_exposes_faceting_metadata table_public_exports_include_core_table_and_virtualizer_contracts component_api_inventory_uses_stable_ownership_vocabulary`
- `cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata components_page_conformance_gates_reference_core_and_gallery_contracts components_gallery_smoke_faceted_filter_updates_table_rows components_gallery_smoke_range_filter_updates_table_rows components_gallery_smoke_focuses_catalog_family_and_restores_all_mode`
- `cargo fmt -p open-gpui-ui-foundation-gallery`
- `cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_range_filter_updates_table_rows`
- `python $HOME\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering`
- `git diff --check`

# Notes

The gallery smoke verifies wheel containment on the popup, controlled range payload recording,
row-model narrowing, and stable override propagation through the sample-owned `TableState`.
