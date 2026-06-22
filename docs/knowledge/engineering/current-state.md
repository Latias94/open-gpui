---
type: Current State
title: open-gpui table and virtualizer implementation state
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
git_commit: 8b4237b
verified_by:
  - cargo nextest run -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
  - cargo nextest run -p open-gpui-ui-core virtualizer table
  - cargo nextest run -p open-gpui-ui-components table feedback tree virtualized_list
  - cargo nextest run -p open-gpui-ui-components feedback_tree_and_virtualized_list_public_exports_remain_explicit crate_root_and_prelude_exports_remain_explicit default_theme_resolves_all_current_component_color_intents public_resolved_state_contracts_avoid_gpui_runtime_types feedback tree virtualized_list
  - cargo nextest run -p open-gpui-ui-foundation-gallery official_component_catalog_entries_have_signals_and_sample_selectors state_contract_catalog_entries_have_signals_and_readout_selectors components_page_samples_expose_component_metadata components_page_conformance_gates_reference_core_and_gallery_contracts components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_page_table_samples_expose_virtualized_row_model_contract
  - cargo nextest run -p open-gpui-ui-foundation-gallery table
  - cargo check -p open-gpui-ui-foundation-gallery --tests
  - cargo check -p open-gpui-ui-components --tests
  - cargo check -p open-gpui-ui-core --tests
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_navigation_rail_scrolls_inside_shell components_gallery_smoke_vertical_tabs_scroll_inside_sample components_gallery_smoke_scroll_area_samples_scroll_inside_page components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation
  - cargo nextest run -p open-gpui-ui-components scroll_area_default_handle_survives_reconstructed_component_values scroll_area_reset_key_resets_default_runtime_handle scroll_area_runtime_scrolls_horizontal_and_two_axis_content tabs_vertical_tablist_scrolls_when_constrained
  - cargo nextest run -p open-gpui-ui-components alert_dialog_state_records_required_actions_and_destructive_intent alert_dialog_state_blocks_underlay_and_restores_focus_to_trigger
  - cargo nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke_dismisses_popover_from_outside_press overlay_gallery_smoke_opens_hover_card_from_real_trigger_and_dismisses overlay_gallery_smoke_closes_dialog_from_modal_barrier_and_escape overlay_gallery_smoke_closes_non_modal_sheet_from_outside_press overlay_gallery_smoke_closes_menu_from_escape_and_outside_press overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses components_gallery_smoke_closes_select_popup_from_outside_press
  - cargo nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke_closes_alert_dialog_from_action_and_escape
  - cargo nextest run -p open-gpui-ui-components overlay_adapter_config_defaults_follow_overlay_kind_policy overlay_open_change_helpers_match_core_policies splitter_runtime_drag_resizes_horizontal_and_vertical_panels splitter_state_normalizes_panel_fractions_and_constraints splitter_resize_delta_clamps_to_adjacent_min_max splitter_runtime_fraction_overrides_still_use_resize_constraints splitter_collapsed_panel_uses_collapsed_fraction
  - cargo nextest run -p open-gpui-ui-components splitter_runtime_drag_resizes_horizontal_and_vertical_panels splitter_state_normalizes_panel_fractions_and_constraints splitter_resize_delta_clamps_to_adjacent_min_max splitter_runtime_fraction_overrides_still_use_resize_constraints splitter_collapsed_panel_uses_collapsed_fraction
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_tabs_and_splitter_interactions_survive_full_page_composition overlay_gallery_smoke_closes_alert_dialog_from_action_and_escape
  - cargo nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke_dismisses_popover_from_outside_press overlay_gallery_smoke_opens_hover_card_from_real_trigger_and_dismisses overlay_gallery_smoke_closes_dialog_from_modal_barrier_and_escape overlay_gallery_smoke_closes_non_modal_sheet_from_outside_press overlay_gallery_smoke_closes_menu_from_escape_and_outside_press overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses components_gallery_smoke_closes_select_popup_from_outside_press
  - cargo nextest run -p open-gpui-ui-components overlay_adapter_config_defaults_follow_overlay_kind_policy overlay_open_change_helpers_match_core_policies
---

# Current State

- Goal: Complete the Table / Virtualizer performance follow-up after `docs/plans/2026-06-21-001-feat-ui-table-virtualizer-roadmap-plan.md`.
- Branch: `main`
- Last verified: 2026-06-22, full `open-gpui-ui-core` + `open-gpui-ui-components` + `open-gpui-ui-foundation-gallery` nextest passed 273/273 before commit `8b4237b`, including the new feedback/tree/virtualized_list primitives and the Table runtime/performance regression gates.
- Done: Moved the Components section directory into its own fixed strip above the page scroll area.
- Done: Kept the Components-page scroll smoke passing while preserving the directory jump contract and page scroll reset behavior.
- Done: Replaced the unstable `data-grid` wheel-motion expectation with a stable state-level contract assertion and kept the release queue horizontal scroll smoke as the runtime proof.
- Done: Added gallery-level wheel isolation on the ScrollArea sample card so release-queue chrome does not leak scroll input to the page shell.
- Done: Added gallery smoke coverage for AlertDialog trigger -> action -> Escape dismissal and focus restoration.
- Done: Confirmed existing overlay and splitter runtime regression gates remain green.
- Done: Rechecked the splitter and overlay contract surface at `d64f5d6`; no new behavior gaps were found in the current codebase.
- Done: Pulled the local `repo-ref/fret`, `repo-ref/tanstack-table`, and `repo-ref/tanstack-virtual` references into the planning context.
- Done: Wrote the table / virtualizer roadmap plan and tightened it around table-core v0, virtualizer v0, gallery conformance, and official component gates.
- Done: Added ADR 0009 and extended the component contract / verification docs with the table and virtualizer product boundary.
- Done: Implemented `open-gpui-ui-core::table` and `open-gpui-ui-core::virtualizer` with passing core contract tests.
- Done: Added `open_gpui_ui_components::Table` as a thin GPUI recipe over `TableState` and `VirtualizerState`; concrete `ScrollHandle` ownership stays in the adapter layer.
- Done: Promoted Table into the Components gallery catalog, signals, page directory, conformance gates, and rendered samples (`release-queue` with 10k rows plus `filter-board` with filter/sort/pagination).
- Done: Added Table gallery contract and runtime smoke coverage proving row-model metadata, a11y roles, virtualized render windows, and nested scroll containment.
- Done: Added `TableHeaderAction` and `Table::on_sort_requested` so sortable headers emit state-update payloads without moving table state ownership into render code.
- Done: Hardened the Table adapter after review: live scroll offsets win after virtualizer measurement snapshots, duplicate row ids get unique render/virtualizer keys, and header/body column minimum widths match.
- Done: Completed the Table performance follow-up: `TableState` row storage is cheap to clone and exposes a conservative cache key, the GPUI `TableRuntime` caches resolved row models across scroll redraws, `VirtualizerState::resolve_fixed_window` materializes only the visible + overscan window for fixed-height tables, and the Components gallery precomputes table state summaries from lazy static samples instead of rebuilding 10k rows during page render.
- Done: Productized the pulled `feedback`, `tree`, and `virtualized_list` primitives in the Components gallery. `StatusCue` and `EmptyState` are now official rendered feedback components with catalog entries, signals, gallery samples, root selector smoke coverage, export tests, and theme-intent coverage. `TreeState` and `VirtualizedListState` are now explicit `state-contract` catalog entries with separate readout selectors, signal gates, and gallery readouts.
- Follow-up: Design real `Tree` and `VirtualizedList` GPUI renderers separately. Keep `VirtualizedListState` as the keyboard/navigation contract unless renderer work proves it should compose more directly with `open_gpui_ui_core::VirtualizerState`, which remains the rendered range engine.
- Blocked: None.
- Next action: Run the broader component/gallery nextest suite after final doc review, commit the feedback/tree/virtualized-list productization slice, then plan renderer follow-up work for Tree or VirtualizedList.

# Citations

[1] Plan `docs/plans/2026-06-20-001-refactor-ui-gallery-interaction-hardening-plan.md`
[2] Commit `a7f0b96` - `docs(knowledge): sync current state to latest gallery gate`
[3] Commit `d64f5d6` - `fix(gallery): cover alert dialog dismissal path`
[4] Commit `14efadc` - `fix(gallery): harden components page scroll surfaces`
[5] AlertDialog gallery gate added on 2026-06-21
[6] Session `019ec6c8-5566-7062-8458-21ebe1360573`
[7] Progress note `docs/knowledge/engineering/progress/2026-06-21-gallery-components-directory-fixed-and-scroll-regressions-stabilized.md`
[8] Plan `docs/plans/2026-06-21-001-feat-ui-table-virtualizer-roadmap-plan.md`
[9] ADR `docs/adr/0009-open-gpui-table-and-virtualizer-product-shape.md`
[10] Verification command `cargo nextest run -p open-gpui-ui-core`
[11] Verification command `cargo nextest run -p open-gpui-ui-foundation-gallery table`
[12] Verification command `cargo nextest run -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`
[13] Pull head `f85e91a` - `fix(ui): keep virtualized list test helper import scoped`
[14] Commit `8b4237b` - `perf(ui-components): cache table virtual windows`
