---
type: Current State
title: open-gpui component renderer implementation state
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
git_commit: d383026
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
  - cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
  - cargo fmt -p open-gpui-ui-components
  - cargo nextest run -p open-gpui-ui-components component_api_inventory
  - cargo nextest run -p open-gpui-ui-components command
  - cargo nextest run -p open-gpui-ui-components crate_root_and_prelude_exports_remain_explicit
  - cargo nextest run -p open-gpui-ui-components public_resolved_state_contracts_avoid_gpui_runtime_types
  - cargo nextest run -p open-gpui-ui-foundation-gallery command
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata
  - cargo check -p open-gpui-ui-foundation-gallery
  - cargo nextest run -p open-gpui-ui-components switch_runtime_click_emits_on_change_with_next_checked
  - cargo nextest run -p open-gpui-ui-components
  - cargo nextest run -p open-gpui-ui-foundation-gallery
  - cargo fmt -p open-gpui-ui-foundation-gallery
  - cargo nextest run -p open-gpui-ui-foundation-gallery overlay_page_catalog_entries_have_signals_and_sample_selectors overlay_gallery_smoke_renders_catalog_entries_and_official_samples
  - cargo nextest run -p open-gpui-ui-foundation-gallery overlay
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focuses_every_focusable_catalog_entry
  - cargo nextest run -p open-gpui-ui-foundation-gallery
  - git diff --check
  - python $HOME\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
---

# Current State

- Goal: Finish the Command component-depth slice and move to the next component-depth target.
- Branch: `main`
- Last verified: 2026-06-22, Command-focused component and gallery gates passed after commits `41c719a` and `d383026`: `cargo nextest run -p open-gpui-ui-components command`, `cargo nextest run -p open-gpui-ui-foundation-gallery command`, `cargo check -p open-gpui-ui-foundation-gallery`, export / API inventory guards, and `git diff --check` with only Windows LF/CRLF warnings.
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
- Done: Productized the pulled `feedback`, `tree`, and `virtualized_list` primitives in the Components gallery. `StatusCue` and `EmptyState` are now official rendered feedback components with catalog entries, signals, gallery samples, root selector smoke coverage, export tests, and theme-intent coverage. `TreeState` remains an explicit `state-contract` catalog entry, while `VirtualizedListState` now sits beside the official `VirtualizedList` renderer as the keyboard/navigation contract and gallery readout surface.
- Done: Pushed the productization slice to `origin/main` as commit `474ac18` after rebasing onto remote commit `45d3199`.
- Done: Wrote the follow-up plan for a real `VirtualizedList` GPUI renderer that composes `VirtualizedListState` with `open_gpui_ui_core::VirtualizerState` instead of treating the state contract as a rendered component, then implemented the concrete adapter and gallery promotion.
- Done: Promoted `VirtualizedList` into the official Components catalog and page directory with a 10k-item `release-navigation` sample, stable sample selectors, a `virtualized-list-renderer` gate, nested scroll containment smoke, and a full-page PageDown plus Enter/Space activation smoke backed by the gallery runtime log.
- Done: Tightened `VirtualizedList::from_shared_items` to accept `Arc<[VirtualizedListItemDescriptor]>`, so shared large-list storage exposes a slice contract instead of leaking `Vec` storage details.
- Done: Added standard controlled TextInput ergonomics with `TextInput::value(...).on_change(...)`. The adapter now creates a keyed `TextInputController` when `on_change` is supplied, emits sanitized single-line values, and keeps callbacks out of `TextInputState`.
- Done: Promoted `Tree` into the official Components surface. The adapter composes `TreeState` with keyed GPUI runtime state, focus handles, expansion overrides, selection/toggle callbacks, and an inner `ScrollArea`. The gallery now has a `document-outline` Tree sample, `tree-renderer` conformance gate, runtime selection/toggle log, keyboard expand/select smoke, and nested scroll containment smoke. `TreeState` remains visible as the renderer-neutral hierarchy readout beside the official component.
- Done: Wrote `docs/plans/2026-06-22-003-feat-ui-api-overlay-productization-plan.md` for the next slice. It starts with a public API inventory, then normalizes controlled/default/policy builders, callback names, Overlay catalog metadata, and focused Components gallery inspection.
- Done: Completed U2 from `docs/plans/2026-06-22-003-feat-ui-api-overlay-productization-plan.md` as commits `e4c46b9` and `b40cb08`. Seed-shaped builders on `Tabs`, `RadioGroup`, `Toolbar`, `Tree`, `VirtualizedList`, `Combobox`, `Command`, `Menu`, and `ContextMenu` now use `default_*` names, `Sidebar::default_focused` covers the adapter-owned focus seed while `Sidebar::selected` stays controlled, state getters such as `ComboboxState::query()` and `CommandState::query()` still expose the current value, the gallery build paths now call the renamed builders, and the contract docs and inventory guard reflect the new ownership vocabulary.
- Done: Completed U3 from `docs/plans/2026-06-22-003-feat-ui-api-overlay-productization-plan.md` as `acc6e91`. `Switch::on_click` was removed from the public builder surface in favor of `Switch::on_change`, the API inventory and public-method baseline now classify `Switch::checked` plus `on_change` with the scalar value-change vocabulary, the contract docs no longer list Switch as a callback exception, and a real GPUI runtime test verifies enabled clicks emit the next checked value while disabled switches do not emit changes. Read-only review `u3_callback_review_light` found no blocking issues.
- Done: Completed U4 from `docs/plans/2026-06-22-003-feat-ui-api-overlay-productization-plan.md` as `f320da1`. The Overlay page now has `OVERLAY_CATALOG`, `OverlayCatalogEntry`, `OverlayCatalogStatus`, and `overlay_sample_selector_pairs()` covering Tooltip, HoverCard, Popover, Dialog, AlertDialog, Sheet, Menu, and ContextMenu. The gallery renders visible Overlay catalog cards, docs describe the overlay catalog contract, and focused tests guard catalog signals plus rendered sample selectors.
- Done: Completed U5 from `docs/plans/2026-06-22-003-feat-ui-api-overlay-productization-plan.md` as `029826e`. The Components page now has `ComponentFocusMode::All` and catalog-driven focused component-family viewing, with an explicit `All components` control, directory chips kept as pure anchor jumps, focus-mode page scroll reset keys, and smoke coverage for all-mode restoration, family switching reset, and focused Table nested scroll containment.
- Done: Hardened composite-widget focus ergonomics by letting `Tree` and `VirtualizedList` roots focus their current keyboard target when clicked. Tree row clicks remain the explicit gallery interaction path; subagent review confirmed the Tree smoke should enter focused Tree mode via the catalog before clicking the concrete `paper` row.
- Done: Wrote `docs/plans/2026-06-22-004-test-ui-gallery-automation-regression-plan.md`, added a catalog-driven focused-mode matrix smoke for every focusable Components catalog entry, fixed the RadioGroup nesting bug so its focused selector renders in `radio-group` mode, and updated `docs/verification.md` with the new gate.
- Done: Recorded the component-depth roadmap in `docs/knowledge/engineering/decisions/open-gpui-ui-component-depth-roadmap.md`: deepen `Command`, then `Menu` / `ContextMenu`, then advanced `Table` and `Tree` behavior before adding more shallow primitives.
- Done: Wrote `docs/plans/2026-06-22-005-feat-ui-command-depth-plan.md` for the next `Command` depth slice. The plan covers renderer-neutral ranking, controlled query ergonomics, optional multi-selection, virtualized long results, app-owned index snapshots, focused gallery samples, and contract / verification memory updates.
- Done: Completed U1-U5 of the Command depth plan. `CommandState` now resolves deterministic ranked results, controlled/default query ownership, optional multi-select selected chips, virtualized render plans for long result sets, and caller-owned `CommandIndexSnapshot` sources with `LocalRanked`, `PreRankedFilter`, and `PreFiltered` modes. The component crate still does not own global command registries, dispatch buses, keybinding resolution, enablement engines, or async indexing.
- Done: Promoted focused Command gallery samples for ranked search, multi-select, a 10k-item virtualized command index, and app-owned indexed/loading metadata. Gallery smokes prove focused Command mode renders all samples, selected chips are inspectable, and virtualized sample wheel input stays inside the sample.
- Follow-up: Keep the full all-components page as the integration stress test; focused mode is a product inspection path, not a replacement for full-page scroll and conformance gates.
- Blocked: None.
- Next action: Plan the next component-depth slice around `Menu` / `ContextMenu` submenu and menu-item semantics unless a new user-facing regression needs priority first.

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
[15] Commit `474ac18` - `feat(ui-components): productize feedback and state contracts`
[16] Plan `docs/plans/2026-06-22-002-feat-ui-virtualized-list-renderer-plan.md`
[17] Verification evidence `docs/knowledge/engineering/verification/tree-renderer-productization-20260622.md`
[18] Plan `docs/plans/2026-06-22-003-feat-ui-api-overlay-productization-plan.md`
[19] Commit `293ec0d` - `test(ui-components): add API inventory guard`
[20] Commit `e4c46b9` - `feat(ui-components): normalize default seed builders`
[21] Commit `b40cb08` - `feat(ui-components): rename query seed builders`
[22] Commit `acc6e91` - `feat(ui-components): rename switch callback to on_change`
[23] Read-only review agent `u3_callback_review_light`
[24] Commit `f320da1` - `feat(gallery): add overlay catalog gates`
[25] Commit `029826e` - `feat(gallery): add focused component view`
[26] Subagent finding `docs/knowledge/engineering/subagents/u5-focused-components-tree-smoke-review.md`
[27] Plan `docs/plans/2026-06-22-004-test-ui-gallery-automation-regression-plan.md`
[28] Verification command `cargo nextest run -p open-gpui-ui-foundation-gallery`
[29] Decision `docs/knowledge/engineering/decisions/open-gpui-ui-component-depth-roadmap.md`
[30] Plan `docs/plans/2026-06-22-005-feat-ui-command-depth-plan.md`
[31] Commit `41c719a` - `feat(ui-components): add command index snapshots`
[32] Commit `d383026` - `feat(ui-gallery): deepen command samples`
