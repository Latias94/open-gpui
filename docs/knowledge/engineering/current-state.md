---
type: Current State
title: open-gpui component renderer implementation state
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
git_commit: 500045e
verified_by:
  - cargo fmt -p open-gpui-ui-components
  - cargo nextest run -p open-gpui-ui-components table component_api_inventory
  - git diff --check
  - python $HOME\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
  - cargo fmt -p open-gpui-ui-core
  - cargo nextest run -p open-gpui-ui-core virtualizer table
  - git diff --check
  - python $HOME\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
  - cargo nextest run -p open-gpui-ui-components table_runtime_pinned_body_scrolls_without_moving_parent
  - cargo nextest run -p open-gpui-ui-components table
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_grouped_table_scroll_stays_inside_sample
  - cargo nextest run -p open-gpui-ui-foundation-gallery table
  - cargo fmt --all -- crates/ui_components/src/table.rs crates/ui_components/tests/components.rs examples/ui-foundation-gallery/src/pages/components.rs examples/ui-foundation-gallery/src/pages/components/render.rs examples/ui-foundation-gallery/tests/foundation_gallery.rs
  - git diff --check
  - python $HOME\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
  - cargo check -p open-gpui-ui-core --tests
  - cargo check -p open-gpui-ui-components --tests
  - cargo nextest run -p open-gpui-ui-core table
  - cargo nextest run -p open-gpui-ui-components table
  - cargo nextest run -p open-gpui-ui-foundation-gallery table
  - git diff --check
  - python $HOME\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
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
  - cargo nextest run -p open-gpui-ui-core -p open-gpui-ui-components
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_page_table_samples_expose_virtualized_row_model_contract components_gallery_smoke_focused_table_scroll_stays_inside_sample components_gallery_smoke_table_scroll_stays_inside_sample components_gallery_smoke_grouped_table_scroll_stays_inside_sample components_gallery_smoke_resizable_table_resize_updates_sample
  - cargo nextest run -p open-gpui-ui-components table component_api_inventory
  - cargo nextest run -p open-gpui-ui-foundation-gallery table
  - cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
  - cargo nextest run -p open-gpui-ui-core table
  - cargo nextest run -p open-gpui-ui-components table component_api_inventory
  - cargo nextest run -p open-gpui-ui-foundation-gallery table
  - git diff --check
  - python $HOME\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
---

# Current State

- Goal: Execute `docs/plans/2026-06-23-008-feat-ui-table-faceting-filter-metadata-plan.md`.
- Branch: `main`
- Last verified: 2026-06-23, focused Table faceting metadata gates passed:
  `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table component_api_inventory crate_root_and_prelude_exports_remain_explicit`, and
  `cargo nextest run -p open-gpui-ui-foundation-gallery table`.
- Done: Implemented U1-U5 of `docs/plans/2026-06-23-005-feat-ui-table-tree-data-plan.md` in the
  working tree. `TableRow` now carries nested children, `TableState` resolves source-tree rows
  with depth/parent/branch/descendant metadata, source-tree expansion reuses
  `TableExpansionState`, and collapsed descendants stay addressable through row lookup metadata.
  The GPUI `Table` adapter now exposes row focus, click/double-click/keyboard activation payloads,
  controlled source-tree expansion request payloads, tree disclosure affordances, and a runtime
  expansion override so disclosure clicks update the current table element immediately.
- Done: Added the focused Components gallery `dependency-tree` Table sample with pinned identity
  and status lanes, tree-depth state summary metadata, controlled expansion/activation runtime
  logging, and `components_gallery_smoke_tree_table_expands_and_activates`.
- Done: Wrote `docs/plans/2026-06-23-006-feat-ui-table-manual-expansion-plan.md` for the next
  Table slice. The plan keeps manual expansion separate from the client-expanded path, adds
  expandable unloaded branches plus child-load metadata, and keeps real async fetching owned by the
  application.
- Done: Implemented the manual expansion / async child metadata slice in the working tree.
  `TableRowChildrenLoadState` records idle/loading/failed child metadata, `TableRow` can be
  expandable without loaded children, and `TableExpansionMode::Manual` preserves app-supplied
  ungrouped source snapshots while the existing client-expanded path stays intact. The
  `ui_components::Table` adapter exports the new contract types, renders loaded/unloaded/loading/
  failed tree disclosure states, and includes loaded-child and child-load metadata in expansion
  request payloads.
- Done: Added the focused Components gallery `server-tree` Table sample. It uses manual expansion,
  starts with unloaded/loading/failed top-level branches, records expansion payload metadata in the
  runtime log, and simulates app-owned child loading by swapping in a loaded source snapshot after
  the `server-workspace` disclosure request.
- Done: Updated the Table contract and verification docs so manual source-tree expansion,
  expandable unloaded branches, and child-load metadata are documented as shipped component
  behavior. Real fetch/cache/data-source orchestration remains app-owned follow-up work.
- Done: Committed the manual expansion slice as `bfa91df`.
- Done: Wrote `docs/plans/2026-06-23-007-feat-ui-table-manual-row-model-controls-plan.md` as the
  next Table slice. The plan follows TanStack-style manual filtering/sorting/pagination controls,
  keeps real fetching and cache ownership in the app, and scopes the first implementation proof to
  server pagination totals plus app-supplied row snapshots.
- Done: Completed the manual row-model controls slice as `d6e5c0d`. `TableStageMode` now lets
  filtering and sorting become manual independently, `TablePagination::manual` carries
  row-count/page-count metadata, manual stages preserve caller-supplied snapshots, and the row-model
  cache key includes the new stage ownership and pagination-total inputs.
- Done: `ui_components::TableRenderPlan` now exposes filtering/sorting/pagination ownership modes
  plus pagination row/page totals, and the core/components crate roots and preludes export
  `TableStageMode`.
- Done: The Components gallery now includes `server-paged`, a manual filter/sort/page Table sample
  that renders only the app-supplied page snapshot while exposing total row and page counts in the
  state readout. Contract docs and verification docs describe manual row-model controls as shipped
  behavior.
- Done: Wrote `docs/plans/2026-06-23-008-feat-ui-table-faceting-filter-metadata-plan.md` as the next
  Table slice. The plan follows TanStack and Fret faceting references, narrows the first pass to
  per-column facet metadata, unique value counts, numeric ranges, and manual/server facet payloads,
  and defers global faceting plus concrete faceted filter toolbar UI.
- Done: Implemented the Table faceting/filter metadata slice in the working tree. `ui_core::TableState`
  now resolves deterministic per-column facet summaries with unique value counts and numeric
  ranges, excludes the target column's own local filter for client facets, accepts explicit
  manual/server facet payloads, and includes faceting inputs in state equality/cache keys.
  `ui_components::TableRenderPlan` exposes faceting ownership plus column facet metadata, with
  crate-root/prelude exports covering the new contract types.
- Done: The Components gallery now proves both client and server facet metadata. `filter-board`
  exposes status unique counts and score ranges derived before pagination, while `server-paged`
  supplies manual status counts and score range metadata for the full 64-row server set even though
  the sample renders only the current 8-row page snapshot. `docs/ui/component-contract.md` and
  `docs/verification.md` record per-column faceting metadata as shipped while keeping global
  faceting, rich filter controls, async option search, and fetch/cache orchestration deferred.
- Done: Simplification review removed the remaining hot-path facet metadata copies: core facet
  resolution uses a recursive visitor instead of intermediate filtered/flattened row vectors, and
  `TableRenderPlan` delegates facet access to its shared resolved table state instead of cloning the
  payload. Focused code review also found and fixed the NaN equality edge case for
  `TableFacetValueCount`, so manual facet payloads with NaN values no longer make cache-key
  comparisons non-reflexive. Final scoped gates, `git diff --check`, and engineering wiki
  validation passed.
- Done: Wrote `docs/plans/2026-06-23-005-feat-ui-table-tree-data-plan.md` as the next Table slice. The plan keeps tree-data rows separate from synthetic grouping, reuses `TableExpansionState` for source hierarchy, adds row interaction payloads and focus semantics, and scopes the first gallery proof to a focused tree-data Table sample with runtime expansion and activation coverage.
- Done: Completed U5/U6 of `docs/plans/2026-06-23-004-feat-ui-table-column-virtualization-plan.md` as `25875d0`. The Components gallery now includes `release-matrix`, a wide pinned Table sample with fourteen center metrics, and a focused smoke that proves far center columns stay unmounted before scroll, mount after horizontal scroll, and keep the outer Components page plus fixed lanes stationary. The gallery state row is also less noisy for non-grouped tables, and the Table contract / verification docs now describe the center-column window as a first-class adapter behavior.
- Done: Completed the sticky pinned Table slice on top of `3273c1a`: the GPUI Table adapter keeps vertical wheel input inside pinned table bodies, `release-rollup` exposes explicit left/center/right lane widths, and the focused gallery smoke proves horizontal center-lane scrolling leaves left/right pinned lanes plus the outer Components page fixed.
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
- Done: Wrote `docs/plans/2026-06-22-006-feat-ui-menu-context-menu-depth-plan.md` and implemented the Menu / ContextMenu depth slice through rich item semantics, stable tree paths, caller-owned checkbox/radio checked state, pure typeahead, keyboard submenu open/close targets, local menu scroll carriers, ContextMenu reuse, and expanded Overlay gallery samples.
- Done: Verified the new Menu / ContextMenu core gates with `cargo check -p open-gpui-ui-components --tests`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, focused `cargo nextest run -p open-gpui-ui-components menu`, `cargo nextest run -p open-gpui-ui-components context_menu`, focused submenu/runtime tests, focused Overlay gallery menu/context samples, engineering wiki validation, and `git diff --check`. Review follow-up fixed keyboard item-level selection handler parity and ContextMenu placement sizing for long visible menus. Hover corridor submenu opening, menubar, OS menu bridge, app-menu registry, and global command dispatch remain deferred.
- Done: Committed and pushed the Menu / ContextMenu depth slice as `697f762`.
- Done: Wrote `docs/plans/2026-06-23-001-feat-ui-table-depth-plan.md` for the next Table depth slice. The plan focuses on making grouped and expanded row-model stages real, adding built-in aggregate metadata, splitting visible columns into pinned left/center/right regions, and proving the behavior in focused Components gallery Table samples.
- Done: Completed U1/U2 of the Table depth plan in `ui_core`: `TableResolvedRow` now represents leaf and group rows, `TableGroupRow` records grouping column/value, depth, parent, first leaf, and leaf count, and `TableState` resolves core -> filtered -> grouped -> sorted -> expanded -> paginated -> final. Expansion is caller-owned by stable row id and collapsed descendants remain addressable through row lookup metadata.
- Done: Updated the GPUI Table adapter to render from resolved row cells instead of assuming every row has a source row. Group rows now share the existing one-axis virtualized row stream and get distinct row chrome without changing scroll ownership.
- Done: Updated the Table contract and verification docs so grouped and expanded row-model behavior is no longer documented as deferred.
- Done: Completed U3 of the Table depth plan in `ui_core`: `TableAggregation` and `TableAggregateKind` now define built-in `count`, `sum`, `min`, `max`, and `average` aggregate cells for group rows. Aggregate specs are part of the `TableState` cache key, group row cells expose aggregate values, the grouping column still displays the grouping value, and `ui_components` crate-root/prelude exports cover the new contract types.
- Done: Updated the Table contract and verification docs so built-in aggregation metadata is no longer documented as deferred; custom aggregate callbacks remain app/future work.
- Done: Completed U4 of the Table depth plan in `ui_core` and `ui_components`: `TableColumnPinning` now splits resolved visible columns into left, center, and right `TableColumnRegions` after visibility and ordering; unknown/invisible pinned ids are ignored; moving a column between sides removes duplicates; pinning participates in the `TableState` cache key; and the GPUI `TableRenderPlan` exposes matching `TableColumnRegionRenderPlan` metadata plus header/body region debug selectors.
- Done: Updated the Table contract and verification docs so pinned semantic render lanes are no longer documented as deferred. Sticky pinned-column scrolling, column resizing, and two-dimensional grid virtualization remain follow-up work.
- Done: Completed U5 of the Table depth plan in the Components gallery. `table_samples()` now includes `release-rollup`, a grouped and pinned Table sample with 320 release rows grouped by `team`, explicit expansion for `group:team=UI` and `group:team=Platform`, aggregate count and score cells, left-pinned `name`, right-pinned `status`, and precomputed readout metadata for grouped/expanded rows, aggregate count, and pinned column regions.
- Done: Added focused gallery proof for the new Table behavior: state tests assert aggregate cells, hidden collapsed descendants staying addressable by stable row id, pinned column region order, and focused Table mode rendering both `release-queue` and `release-rollup`; runtime smoke `components_gallery_smoke_grouped_table_scroll_stays_inside_sample` proves the grouped sample scrolls inside the table viewport without moving the outer Components page.
- Done: Completed U6 of the Table depth plan with full focused and broad verification. The full `open-gpui-ui-components` suite passes 209/209 and the full `open-gpui-ui-foundation-gallery` suite passes 74/74 after stabilizing long Components-page automation.
- Done: Hardened the Components gallery smokes discovered during U6: the Command catalog entry now points at the real `ranked-search` sample selector, and long-section smokes use catalog directory jumps plus the gallery page `ScrollHandle` to align concrete interactive targets before clicking, dragging, or scrolling nested controls.
- Done: Created `docs/plans/2026-06-23-002-feat-ui-table-column-sizing-plan.md` to start the next Table slice. The new plan uses TanStack Table's committed sizing / transient resizing split and Fret's parity fixtures as the main references, and it deliberately stops before sticky pinned-column layout or two-dimensional virtualization.
- Done: Completed U1/U2 of `docs/plans/2026-06-23-002-feat-ui-table-column-sizing-plan.md` as commits `9264682` and `513f13c`. `TableColumnSizing` now resolves controlled widths and total size, the GPUI Table adapter consumes column sizing offsets, and the components crate exports the sizing contract through its public surface. Verified the focused core/components table gates before moving on to resize interaction work.
- Done: Completed U3 of `docs/plans/2026-06-23-002-feat-ui-table-column-sizing-plan.md` as `426742a`. `TableColumnResizeMode`, `TableColumnResizeDirection`, and resize state/update helpers now drive committed/transient resize behavior, the GPUI adapter exposes callback-backed drag handles with controlled sizing change events, and tests cover LTR/RTL drag semantics plus runtime header-click parity.
- Done: Completed U4 of `docs/plans/2026-06-23-002-feat-ui-table-column-sizing-plan.md` as `3273c1a`. The Components gallery now has a `release-resize` Table sample, a runtime sizing log, visible width / resizable-column summaries, selector-aligned resize smoke coverage, and docs / verification entries for the new gate.
- Done: Created `docs/plans/2026-06-23-003-feat-ui-table-sticky-pinned-columns-plan.md` as the next Table slice. The new plan keeps the existing semantic pinned regions, turns the center lane into a shared horizontal scroll surface, and keeps vertical row virtualization one-dimensional.
- Done: Completed the sticky pinned Table implementation as `f0b7e62` and recorded its contract / verification evidence as `7f8b986`.
- Done: Created `docs/plans/2026-06-23-004-feat-ui-table-column-virtualization-plan.md` for the next Table slice. The plan narrows two-dimensional virtualization to center-column virtualization first: pinned lanes stay fully rendered, center lanes render only the visible plus overscan column window, and row virtualization remains one-dimensional.
- Done: Completed U1 of `docs/plans/2026-06-23-004-feat-ui-table-column-virtualization-plan.md` as `94fdd59`. `VirtualizerState` now resolves exact-size windows for known item widths, materializes only visible plus overscan measurements, and keeps the fixed-size path unchanged.
- Done: Completed U2 of `docs/plans/2026-06-23-004-feat-ui-table-column-virtualization-plan.md` as `df27aa4`. `ui_components::TableRenderPlan` now exposes `TableCenterColumnWindowPlan` metadata for the center lane, including total center width, visible and overscan ranges, leading/trailing spacer widths, rendered center columns, and virtualization activity from adapter-owned horizontal scroll input.
- Done: Completed U3 of `docs/plans/2026-06-23-004-feat-ui-table-column-virtualization-plan.md` as `3819ac6`. The GPUI `Table` adapter now renders center headers and body cells from the shared `TableCenterColumnWindowPlan`, inserts leading/trailing spacers to preserve full center-lane scroll geometry, keeps left/right pinned lanes fully mounted, and tests prove off-window center selectors unmount/remount while row virtualization remains independent after horizontal scroll.
- Done: Completed U4 of `docs/plans/2026-06-23-004-feat-ui-table-column-virtualization-plan.md` as `5d67277`. Added plan-level accessibility coverage for virtualized center columns, runtime sort coverage for a rendered center header after horizontal scroll, and resize-geometry coverage proving the virtual center window recomputes from committed sizing while preserving the rendered column identity set.
- Follow-up: Keep the full all-components page as the integration stress test; focused mode is a product inspection path, not a replacement for full-page scroll and conformance gates.
- Follow-up: The column sizing / resize, sticky pinned-column, center-column virtualization,
  tree-data, row-interaction, manual-expansion, manual row-model control, and per-column faceting
  metadata slices are complete. Remaining later Table follow-ups are two-dimensional grid
  virtualization, custom aggregation callbacks, row pinning, row selection variants, cell editing,
  global faceting, concrete faceted filter UI, and standalone headless extraction if cross-framework
  pressure appears.
- Blocked: None.
- Next action: Run final quality checks for
  `docs/plans/2026-06-23-008-feat-ui-table-faceting-filter-metadata-plan.md`, validate engineering
  memory, then commit the Table faceting metadata slice.

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
[33] Commit `697f762` - `feat(ui-components): deepen menu and context menu semantics`
[34] Plan `docs/plans/2026-06-23-001-feat-ui-table-depth-plan.md`
[35] Commit `85a6edf` - `docs(ui): add tanstack table references`
[36] Commit `5280468` - `feat(ui-core): add grouped table row models`
[37] Commit `dd525ab` - `feat(ui-core): add table group aggregations`
[38] Verification command `cargo nextest run -p open-gpui-ui-core table`
[39] Verification command `cargo nextest run -p open-gpui-ui-components table`
[40] Verification command `cargo nextest run -p open-gpui-ui-foundation-gallery table`
[41] Plan `docs/plans/2026-06-23-002-feat-ui-table-column-sizing-plan.md`
[42] Commit `9264682` - `feat(ui-core): add table column sizing state`
[43] Commit `513f13c` - `feat(ui-components): expose table column sizing contract`
[44] Commit `426742a` - `feat(ui): add table column resizing interactions`
[45] Commit `3273c1a` - `feat(gallery): add table column resize sample`
[46] Plan `docs/plans/2026-06-23-003-feat-ui-table-sticky-pinned-columns-plan.md`
[47] Verification command `cargo nextest run -p open-gpui-ui-core -p open-gpui-ui-components`
[48] Verification command `cargo nextest run -p open-gpui-ui-foundation-gallery components_page_table_samples_expose_virtualized_row_model_contract components_gallery_smoke_focused_table_scroll_stays_inside_sample components_gallery_smoke_table_scroll_stays_inside_sample components_gallery_smoke_grouped_table_scroll_stays_inside_sample components_gallery_smoke_resizable_table_resize_updates_sample`
[49] Verification evidence `docs/knowledge/engineering/verification/table-sticky-pinned-columns-20260623.md`
[50] Plan `docs/plans/2026-06-23-004-feat-ui-table-column-virtualization-plan.md`
[51] Verification evidence `docs/knowledge/engineering/verification/table-exact-size-virtualizer-window-20260623.md`
[52] Commit `df27aa4` - `feat(ui-components): add table center column window plan`
[53] Commit `3819ac6` - `feat(ui-components): virtualize table center columns`
[54] Commit `5d67277` - `test(ui-components): cover virtualized table center interactions`
[55] Commit `234b0cc` - `feat(ui): add table tree rows and row interactions`
[56] Plan `docs/plans/2026-06-23-006-feat-ui-table-manual-expansion-plan.md`
[57] Commit `bfa91df` - `feat(ui): add table manual expansion state`
[58] Plan `docs/plans/2026-06-23-007-feat-ui-table-manual-row-model-controls-plan.md`
[59] Commit `d6e5c0d` - `feat(ui): add table manual row-model controls`
[60] Verification command `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery -- --check`
[61] Verification command `cargo nextest run -p open-gpui-ui-core table`
[62] Verification command `cargo nextest run -p open-gpui-ui-components table component_api_inventory`
[63] Verification command `cargo nextest run -p open-gpui-ui-foundation-gallery table`
