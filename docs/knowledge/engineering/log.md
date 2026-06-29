---
type: Work Progress
title: Session log
status: active
---

# Log

- 2026-06-30: Started the P2/P3 fearless component refactor goal on
  `refactor/component-p2-p3-cleanup` from `main` / `origin/main`. Read-only subagent lanes are
  `contracts`, `large_modules`, `runtime_patterns`, and `exports_tests`; the first main-thread
  target is the `ui_components` component contract source-scanning blind spot for split component
  directories.
- 2026-06-30: Added a root/prelude re-export alignment contract in
  `crates/ui_components/tests/components.rs`. The test parses explicit `pub use` token sets from
  `src/lib.rs` and `src/prelude.rs`, keeps adapter-only root exports out of the default surface, and
  records the intentional prelude-only convenience tokens as an allowlist.
- 2026-06-30: Hardened component contract source scanning for split component directories.
  `component_source_inputs` can now name either `.rs` files or directories, directory inputs expand
  recursively to source paths, and public method scanning requires the target impl to exist across
  the expanded set instead of every mapped file. Verified with focused component contract nextest
  runs and `cargo check -p open-gpui-ui-components --tests`.
- 2026-06-30: Added the crate-private `consume_overlay_event` helper in
  `crates/ui_components/src/overlay.rs` and replaced repeated overlay-handled event-consumption
  pairs across Dialog, Popover, HoverCard, Select, Combobox, Menu, ContextMenu, and Sheet close/barrier
  paths without changing each component's open/close runtime decisions. Verified with focused
  overlay/component nextest coverage and `cargo check -p open-gpui-ui-components --tests`.
- 2026-06-30: Kept `consume_overlay_event` scoped to overlay open/close and barrier handling:
  Select and Combobox trigger open/close paths use the helper, while Menu and ContextMenu submenu
  navigation/focus/activation still consume keyboard events locally. Verified with focused
  select/combobox/menu/context-menu nextest coverage and
  `cargo check -p open-gpui-ui-components --tests`.
- 2026-06-30: Split the broad component API inventory contract test into focused checks for
  row uniqueness/classification, public method surface drift, ownership vocabulary, and stateful
  regression sentinels. Verified the four focused inventory tests and
  `cargo check -p open-gpui-ui-components --tests`.
- 2026-06-30: Split the Tree render-plan API into
  `crates/ui_components/src/tree/render_plan.rs`, preserving `tree` module exports and updating the
  component contract source mapping. Verified with `cargo check -p open-gpui-ui-components --tests`
  and focused tree/export/contract nextest coverage.
- 2026-06-30: Split Tree movement payloads and `apply_tree_move` into
  `crates/ui_components/src/tree/movement.rs`, preserving `tree` module exports and updating the
  component contract source mapping. Verified with `cargo check -p open-gpui-ui-components --tests`
  and focused tree movement/export/contract nextest coverage.
- 2026-06-29: Continued `docs/plans/2026-06-29-002-refactor-table-depth-second-stage-plan.md`
  on `refactor/table-depth-second-stage`. The Table adapter is now split into concern-owned
  modules for header rendering, resize/reorder affordances, body rows, cells, editors, runtime,
  resolution, render plans, filtering recipes, column visibility, toolbar, layout, metrics, and
  virtualization. Added source-owner and gallery conformance assertions so the old
  `ui_components/src/table.rs` adapter path cannot reappear silently. Verified the U6/U7 focused
  surface with `cargo fmt -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-components --tests`, `cargo nextest run -p open-gpui-ui-components table`, and the
  public inventory focused nextest commands before the final full verification pass.
- 2026-06-28: Completed the choice-surface refactor implementation for
  `docs/plans/2026-06-28-001-refactor-ui-choice-surface-plan.md` in the working tree.
  `ui_components::choice` now centralizes stable-value projection, query normalization, and
  multi-select dedupe for `Command`, `Combobox`, and `Select`; `roving_focus.rs` owns shared
  vertical, paged, and typeahead target helpers; and `menu_runtime.rs` owns Menu / ContextMenu
  runtime state, submenu hover timers, branch switching, and local submenu scroll handles. The
  Components gallery now exposes a `choice-surfaces` conformance gate plus state readouts for
  stable values, selected chips, query/typeahead metadata, and shared navigation behavior. Verified
  with focused component/gallery gates, `git diff --check`, engineering wiki validation, and
  `cargo run -p xtask -- verify`.
- 2026-06-28: Completed the measured-row Table slice in the working tree. `ui_components::Table` now exposes `row_measure_mode`, the GPUI adapter can measure rendered body row heights and feed them back into the row virtualizer cache, and the render path keeps fixed-height rows unchanged when the mode stays at `Fixed`. Focused component and gallery verification passed after syncing the component API inventory for the new `Table` builder and the existing `Select::full_width` baseline drift.
- 2026-06-27: Completed the Table select editor slice in the working tree. `TableCellEditor::Select` and `TableSelectOption` are now public in core and components, the GPUI Table adapter renders fixed-option `Select` editors for leaf cells, the row-click path now respects prevented events so embedded editors do not wake row activation, the Components gallery gained a `select-release` sample plus a dedicated select smoke, and the component contract / verification notes now call out the new inline-edit recipe. Verified with targeted `cargo test -p open-gpui-ui-components` runs for the select cell regression, the standalone select runtime smoke, and the explicit root / prelude export gate. Next action is to pick the next Table maturity boundary, with sticky headers currently the clearest follow-up.
- 2026-06-27: Refreshed the engineering memory after confirming the
  `feat/scroll-surface-containment` branch is clean and the scroll-surface / context-menu / sidebar
  / tabs work is fully shipped. The current durable state now points at the next Table follow-up
  boundary instead of the finished containment slice, and the repo-local memory bundle stays
  aligned with the clean checkout.
- 2026-06-27: Completed the Table column-order slice on `feat/scroll-surface-containment`.
  `ui_components::Table` now emits controlled `TableColumnOrderChange` payloads through
  `on_column_order_change`, the gallery runtime log stores per-sample column-order overrides, and
  the `release-rollup` sample re-renders the reordered center columns without disturbing the rest
  of `TableState`. Verified with `cargo fmt --all`, the targeted `open-gpui-ui-components` Table
  component gates, and `cargo nextest run -p open-gpui-ui-foundation-gallery
  components_gallery_smoke_grouped_table_column_reorder_updates_sample`.
- 2026-06-27: Completed the Table sticky-header slice on `feat/scroll-surface-containment`. The
  GPUI Table adapter now renders the header band as an absolute overlay at the top of the table
  root, pads the body by the same band height, and keeps vertical wheel input inside the table
  body while the header stays fixed. Updated the runtime and gallery proofs so the pinned body
  scroll test and the `release-rollup` smoke both assert stable header bounds across vertical
  scroll. Verified with focused table adapter formatting and gallery smoke coverage,
  `cargo nextest run -p open-gpui-ui-components table`,
  `cargo nextest run -p open-gpui-ui-foundation-gallery table`, and `git diff --check`.
- 2026-06-27: Added adapter-owned submenu hover timers and close timing on
  `feat/scroll-surface-containment`. `MenuRuntime` now tracks submenu hover epoch, a task handle,
  hovered path, and submenu surface hover state; submenu trigger and submenu surface hover events
  schedule delayed open/close transitions without moving timer state into resolved contract types.
  Updated the component and gallery tests to advance the test clock before asserting submenu
  visibility, and refreshed the component contract, verification notes, and current-state memory.
  Verified with `cargo fmt --all`, `cargo check -p open-gpui-ui-components --tests`,
  `cargo check -p open-gpui-ui-foundation-gallery --tests`,
  `cargo nextest run -p open-gpui-ui-components menu_runtime_hover_opens_submenu_and_preserves_child_focus menu_runtime_hover_switches_between_submenu_branches`,
  and `cargo nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke_opens_menu_submenu_from_hover`.
- 2026-06-27: Completed the Menu floating submenu panel slice on
  `feat/scroll-surface-containment`. `Menu` now renders branch-local menu content inside a
  focusable shell, keeps the current branch scrollable, records trigger bounds during paint, and
  resolves open child branches into deferred floating submenu panels to the right using the
  renderer-neutral submenu surface contract. Added focused component coverage proving submenu
  panels open to the right instead of stacking under the trigger, and kept the existing hover-open
  / sibling-branch-switching proofs green. Verified with `cargo fmt -p open-gpui-ui-components`,
  `cargo check -p open-gpui-ui-components --tests`,
  `cargo nextest run -p open-gpui-ui-components menu_runtime_keyboard_submenu_opens_and_selects_child menu_runtime_hover_opens_submenu_and_preserves_child_focus menu_runtime_hover_switches_between_submenu_branches menu_state_resolves_submenu_surface_and_safe_hover_contract menu_submenu_surface_resolves_left_placement_without_renderer_state`, and
  `git diff --check`.
- 2026-06-27: Added the renderer-neutral Menu submenu floating-surface contract on
  `feat/scroll-surface-containment`. `MenuSubmenuSurface` resolves placement input, preferred
  submenu bounds, and `MenuSafeHoverCorridor` from trigger bounds plus content size;
  `MenuState::submenu_surface_for_trigger` exposes the plan for submenu triggers without changing
  the current inline GPUI submenu renderer. Root/prelude exports and docs now record the contract.
  Verified with `cargo fmt -p open-gpui-ui-components`,
  `cargo check -p open-gpui-ui-components --tests`,
  `cargo nextest run -p open-gpui-ui-components menu_state_resolves_submenu_surface_and_safe_hover_contract menu_submenu_surface_resolves_left_placement_without_renderer_state`,
  and `cargo nextest run -p open-gpui-ui-components crate_root_and_prelude_exports_remain_explicit component_api_inventory_uses_stable_ownership_vocabulary public_resolved_state_contracts_avoid_gpui_runtime_types`.
- 2026-06-27: Deepened the Overlay Menu hover-submenu proof on
  `feat/scroll-surface-containment`. The `rich-items` Menu gallery sample now has two non-empty
  sibling submenus, and component plus gallery smokes verify hover-open, sibling branch switching,
  old-branch dismissal, and closing the active branch from a plain root item. Verified with
  `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`,
  `cargo nextest run -p open-gpui-ui-components menu_runtime_hover_switches_between_submenu_branches menu_runtime_hover_opens_submenu_and_preserves_child_focus`,
  and `cargo nextest run -p open-gpui-ui-foundation-gallery overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts overlay_gallery_smoke_opens_menu_submenu_from_hover`.
- 2026-06-26: Started the Avatar family slice on `feat/scroll-surface-containment`. Added
  `AvatarGroup` and `AvatarGroupCount` to `open-gpui-ui-components`, wired theme colors and root /
  prelude exports, added focused component tests for visible/hidden counts and group count state,
  and promoted the family into the Components gallery catalog plus smoke coverage. Verified with
  `cargo check -p open-gpui-ui-components --tests`,
  `cargo check -p open-gpui-ui-foundation-gallery`,
  `cargo nextest run -p open-gpui-ui-components --tests avatar --no-fail-fast`, and
  `cargo nextest run -p open-gpui-ui-foundation-gallery --tests foundation_gallery --no-fail-fast`.
- 2026-06-26: Closed the Avatar family slice on `feat/scroll-surface-containment`. Added a real
  overlapping `AvatarGroup` surface, a dedicated `AvatarGroupCount` overflow bubble, public exports
  and theme intents, focused API/state tests, gallery catalog proof, and a dedicated smoke selector.
  Verified with `cargo nextest run -p open-gpui-ui-components avatar_group_state_tracks_visible_and_hidden_counts controlled_text_input_on_change_marks_input_controller_driven --no-fail-fast`,
  `cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation --no-fail-fast`,
  `git diff --check`, and the existing `cargo check` gates.
- 2026-06-26: Finished the scroll-surface local containment slice on
  `feat/scroll-surface-containment`. Vertical `Tabs` now route their rail through the shared
  `ScrollArea` primitive, long `Sidebar` navigation keeps the same shared scroll ownership, and
  the focused component/gallery smokes now assert the local viewport directly. Verification
  passed with `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`,
  `cargo nextest run -p open-gpui-ui-components tabs_vertical_tablist_scrolls_when_constrained sidebar_long_navigation_scrolls_inside_shared_scroll_area scroll_area_nested_scroll_keeps_parent_static`,
  `cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_vertical_tabs_scroll_inside_sample components_gallery_smoke_sidebar_long_navigation_scrolls_inside_sample`,
  `git diff --check`, and
  `python $HOME\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering`.
- 2026-06-26: Synced engineering memory after the Table content-fit slice and the text input
  editor family both shipped. The stale autosize and textarea next-action markers in
  `current-state.md` now point at the next open component-depth boundary instead of a finished
  slice.
- 2026-06-26: Wrote `docs/plans/2026-06-26-005-feat-ui-scroll-surface-local-containment-plan.md`
  as the next scroll-surface boundary. The plan tightens the shared `ScrollArea` contract and
  routes constrained vertical `Tabs` and long `Sidebar` surfaces through the same local scroll
  ownership model so wheel input stays inside the sample viewport.
- 2026-06-26: Completed the first Tree drag-and-drop hierarchy slice from
  `docs/plans/2026-06-26-004-feat-ui-tree-drag-drop-hierarchy-plan.md`. The Tree contract now
  exposes controlled move payloads and pure move application helpers, the GPUI adapter owns the
  pointer drag sensor plus drop-zone preview, and the Components gallery proves a visible
  `editable-outline` reorder. The final smoke had to scroll the `child` row itself into view before
  starting the drag; that kept the test stable inside the nested gallery viewport. Focused
  `cargo nextest run` checks passed for `open-gpui-ui-components` Tree move contracts and the
  gallery Tree samples.
- 2026-06-26: Wrote `docs/plans/2026-06-26-004-feat-ui-tree-drag-drop-hierarchy-plan.md` for
  the next Tree boundary. The plan keeps Tree move ownership controlled, splits pure move
  resolution from adapter-owned pointer drag state, and defers cross-tree dragging, auto-expand on
  hover, and a shared DnD crate until the first slice proves the contract. Next action is U1:
  add the pure Tree move target contract.
- 2026-06-26: Completed
  `docs/plans/2026-06-26-003-feat-ui-tree-virtualized-window-plan.md` in the working tree.
  `TreeRenderPlan` / `TreeRowRenderPlan` now resolve a fixed-row overscan window from `TreeState`
  and `VirtualizerState`, the GPUI `Tree` adapter exposes opt-in virtualized rendering with
  viewport and overscan controls, and the Components gallery now includes a large
  `release-outline` Tree sample plus focused verification for render-plan exports and gallery
  metadata. A deeper far-row gallery scroll proof was attempted and then trimmed back to the
  stable slice boundary; the remaining Tree follow-ups are drag-and-drop hierarchy editing and any
  later scroll-proof hardening the runtime still needs.
- 2026-06-26: Completed
  `docs/plans/2026-06-26-002-feat-ui-tree-typeahead-plan.md` in the working tree.
  `TreeState::typeahead_target` now performs renderer-neutral prefix matching over visible,
  focusable rows, and the GPUI Tree adapter owns the printable-key buffer/reset policy. Component
  runtime coverage proves typeahead moves focus without selecting; the existing Tree gallery smoke
  now also verifies typing `n o` focuses the visible Notes row. Deferred Tree work remains
  drag-and-drop hierarchy editing and virtualized tree data.
- 2026-06-26: Completed
  `docs/plans/2026-06-26-001-feat-ui-tree-lazy-loading-plan.md` in the working tree.
  `TreeChildrenLoadState` now models loaded, unloaded, loading, and failed child states; Tree
  descriptors, resolved item state, and toggle payloads expose loaded-child counts and load-state
  metadata while keeping async loading caller-owned. The Components gallery adds the
  `remote-workspace` Tree sample with unloaded/loading/failed/loaded branches and a focused smoke
  proving unloaded and failed branches emit load metadata while loading branches do not repeat
  toggle requests. Next Tree follow-ups are typeahead, drag-and-drop hierarchy editing, and
  virtualized tree data.
- 2026-06-26: Completed U4 of
  `docs/plans/2026-06-25-002-feat-ui-text-input-editor-family-plan.md` in the working tree.
  `TableCellEditor::MultilineText { rows }` extends the renderer-neutral Table editor contract,
  the GPUI adapter maps it to fixed-height `Textarea` cells, and `TableCellEditChange` continues
  to carry app-owned row/column edit payloads. The Components gallery now has `multiline-release`
  plus a focused smoke proving newline-preserving textarea edits update sample-owned table rows.
- 2026-06-26: Completed U3 of
  `docs/plans/2026-06-25-002-feat-ui-text-input-editor-family-plan.md` in the working tree.
  `Textarea` is now a separate controlled multiline component with newline-preserving
  `on_change`, renderer-neutral `TextareaState`, root/prelude exports, component API inventory
  coverage, Components gallery samples, Field+Textarea composition, and a focused gallery smoke
  proving textarea wheel input stays inside the sample instead of moving the page. Next action is
  to decide whether U4 Table multiline cell editor composition is needed before switching to the
  next component-depth boundary.
- 2026-06-25: Completed U1/U2 of
  `docs/plans/2026-06-25-002-feat-ui-text-input-editor-family-plan.md` in the working tree.
  `TextInput` now has internal value/display projection helpers, public `TextInputDisplayMode`
  root/prelude exports, password display that masks one glyph per stored grapheme, controller
  hit-testing / IME geometry mapping between displayed mask offsets and stored value offsets, and a
  Components gallery password sample. Verified focused TextInput, gallery metadata, and public API
  / contract guard nextest checks. Next action is U3: add a separate controlled `Textarea`.
- 2026-06-25: Wrote `docs/plans/2026-06-25-002-feat-ui-text-input-editor-family-plan.md`
  as the next component-depth boundary after Table content-fit. The plan keeps the current-crate
  product boundary, starts by splitting stored text from displayed text, then adds password display,
  a separate controlled `Textarea`, and finally optional Table multiline editor composition after
  the primitive is stable. References include the existing TextInput controller research,
  `gpui-component` masked offset handling, Fret's textarea/editor boundaries, and TanStack's
  app-owned editable table examples. Next action is U1: add text value/display projection helpers.
- 2026-06-25: Wrote `docs/plans/2026-06-25-001-feat-ui-table-autosize-by-content-plan.md`
  as the next Table maturity boundary. The plan adds a renderer-neutral content-fit column policy
  plus adapter-owned visible-width measurement and keeps manual sizing authoritative. Sticky
  headers, column drag reorder, dataset-wide exact autosizing, multiline editor measurement, and
  headless extraction remain deferred. Next action is U1: add the core content-fit column policy.
- 2026-06-25: Completed the first content-fit slice in the working tree. `TableColumn`
  width-policy exposure is in place, `TableRenderPlan` now reports measured content-fit widths, the
  Components gallery has a `content-fit-release` proof sample, and focused tests prove visible
  edits widen the name column while the fixed score column stays anchored. Next action is to commit
  the slice and choose the next Table boundary.
- 2026-06-25: Completed U1 of
  `docs/plans/2026-06-25-001-feat-ui-table-autosize-by-content-plan.md` in the working tree.
  `ui_core::TableColumn` now carries a renderer-neutral width policy with `Fixed` and
  `ContentFit` modes; the policy participates in table cache keys; the GPUI table render plan
  exposes the policy for each resolved column; and focused core/components/table nextest checks
  passed. Next action is U2: adapter-owned visible-width measurement and overlay.
- 2026-06-25: Completed U4 of
  `docs/plans/2026-06-24-010-feat-ui-table-column-groups-nested-headers-plan.md` in the working
  tree. `ui_components::Table` now renders nested header groups with multi-row, region-aware GPUI
  header lanes while preserving leaf sort and resize behavior; focused core and component nextest
  checks passed; and the next action is U5: add gallery proof for the nested headers slice.
- 2026-06-25: Completed U5 of
  `docs/plans/2026-06-24-010-feat-ui-table-column-groups-nested-headers-plan.md` in the working
  tree. `release-matrix` now carries the nested-header gallery proof with a grouped column tree, a
  header-summary readout, and a focused center-lane scroll smoke that keeps pinned header families
  mounted while the far center window enters and exits view. Focused gallery nextest checks passed,
  wiki validation passed, and the next action is U6: update memory and decide whether this slice is
  ready to commit.
- 2026-06-25: Completed U2 of
  `docs/plans/2026-06-24-010-feat-ui-table-column-groups-nested-headers-plan.md` in the working
  tree. `TableState::resolve` now emits region-split nested header groups using
  `TableResolvedHeaderKind`, `TableResolvedHeaderCell`, `TableResolvedHeaderGroup`, and
  `TableResolvedHeaderGroupRegions`; the new contract is exported from `ui_core` and
  `ui_components`; and focused `cargo nextest run` checks passed for the core nested-header cases
  plus the component public-export smoke. Next action is U3: expose nested headers in
  `TableRenderPlan`.
- 2026-06-24: Completed U1 of
  `docs/plans/2026-06-24-010-feat-ui-table-column-groups-nested-headers-plan.md` in the working
  tree. `TableColumnGroupId`, `TableColumnNode`, and `TableColumnGroup` now expose the nested
  column-tree contract in `ui_core`; `TableState` keeps a normalized tree plus leaf projection,
  prunes duplicate leaf ids deterministically, and includes tree shape in the cache key; and
  `ui_components` root / prelude exports plus focused nextest checks now cover the new types.
  Next action is U2: renderer-neutral header group resolution.
- 2026-06-24: Created
  `docs/plans/2026-06-24-010-feat-ui-table-column-groups-nested-headers-plan.md` as the next Table
  maturity boundary. The plan keeps `TableColumn` as the behavioral leaf descriptor, adds a
  separate group/tree header contract, uses TanStack and Fret header-group references, and starts
  with U1 core column tree descriptors plus normalized leaf projection.
- 2026-06-24: Created `docs/plans/2026-06-24-009-feat-ui-table-filter-operators-plan.md` as the
  next Table boundary and committed it as `f4a0af7`. The plan uses TanStack filter-function
  references and Fret parity tests to shape a closed built-in predicate family instead of a
  callback registry.
- 2026-06-24: Completed U1/U2 of the filter-operators plan and committed the core slice as
  `ae798e7`. `TableFilterKind` now carries explicit text and numeric comparison operators,
  case-sensitivity metadata, cache-key participation, and focused tests proving richer predicates
  compose with facets and the global filter.
- 2026-06-24: Completed U3/U4 of the filter-operators plan and committed the component/gallery
  slice as `82997fe` and `ecc5f45`. `TablePredicateFilter` now productizes the controlled
  operator/value recipe, `TablePredicateFilterChange` preserves unrelated `TableState` slices,
  and the Components gallery proves the predicate filter against the `filter-board` sample with
  runtime logs and row-window changes.
- 2026-06-24: Refreshed the Table memory bundle so `docs/ui/component-contract.md`,
  `docs/verification.md`, and `docs/knowledge/engineering/current-state.md` now record the
  predicate-filter slice as shipped behavior and keep nested AND/OR predicate builders deferred.
- 2026-06-24: Completed U3/U4 of
  `docs/plans/2026-06-24-008-feat-ui-table-column-visibility-controls-plan.md` and committed the
  slice as `d8abeaa`. `release-matrix` now renders a `TableColumnVisibility` toolbar control over the wide
  metric table, locks the pinned identity/status columns, records controlled visibility payloads in
  `TableSampleRuntimeLog`, applies app-owned visibility overrides through
  `table_sample_state_with_runtime`, and has focused smoke coverage for hiding/restoring a metric
  column plus popup wheel containment. `docs/ui/component-contract.md` and `docs/verification.md`
  now record column visibility as shipped Table behavior.
- 2026-06-24: Completed U2 of
  `docs/plans/2026-06-24-008-feat-ui-table-column-visibility-controls-plan.md` in the current
  branch. `TableColumnVisibility` now productizes the toolbar recipe with controlled open/visibility
  ownership, item metadata, show-all / reset payloads, and `apply_to` helpers that preserve
  unrelated `TableState` slices. The components test suite now covers state resolution, payload
  semantics, public exports, and the API inventory contract for the new recipe family.
- 2026-06-24: The active Table maturity goal remains open. The current next boundary is
  column visibility, and the next actions after that slice are gallery proof, contract updates,
  and the next maturity gap in the table roadmap.
- 2026-06-24: Completed U1 of
  `docs/plans/2026-06-24-008-feat-ui-table-column-visibility-controls-plan.md` in the working
  tree. `TableColumnVisibilityOverrides` now provides sparse runtime visibility overrides,
  `TableColumn::with_hideable` protects default-visible identity columns from stale hidden
  overrides, `TableState` includes visibility in equality/cache keys, and visible-column
  resolution now uses runtime overrides before existing order / pinning / sizing consumers.
  Verified with `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components`,
  `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table_public_exports_include_core_table_and_virtualizer_contracts crate_root_and_prelude_exports_remain_explicit`,
  and `git diff --check`. Next action is U2: component recipe and payload.
- 2026-06-24: Confirmed the active long-running Table maturity goal for the current session:
  continue planning, fearless refactoring, implementation, verification, memory updates, and
  commits until the Table API, interaction behavior, performance / virtualization, gallery proof,
  and docs contract are mature enough to serve as the official component-library baseline. The
  current next boundary is
  `docs/plans/2026-06-24-008-feat-ui-table-column-visibility-controls-plan.md`, committed as
  `55c7970`; next action is U1 core runtime column visibility state.
- 2026-06-24: Set a long-running Table maturity goal for the current session: continue planning,
  fearless refactoring, implementation, verification, memory updates, and commits until the Table
  component is mature enough to be an official component-library baseline. The latest shipped slice
  is `8fc540f feat(table): add toolbar recipe for filter compositions`, which adds
  `TableToolbar` / `TableToolbarState`, exports and inventories the recipe, and moves the gallery
  `filter-board` controls into the toolbar. Verified with focused component and gallery nextest
  runs plus `git diff --check`. Next planning boundary is likely column visibility or another
  shell-level table composition helper.
- 2026-06-24: Wrote `docs/plans/2026-06-24-007-feat-ui-table-global-filtering-faceting-plan.md`
  as the next Table boundary. The plan keeps global filter state separate from column filters,
  adds a global facet summary derived from the pre-global-query row basis, and scopes the first
  `TableGlobalFilter` recipe to a controlled search input. Fuzzy ranking, operator menus, and
  nested predicate builders are deferred.
- 2026-06-24: Completed the Table numeric range filter controls slice from
  `docs/plans/2026-06-24-006-feat-ui-table-numeric-range-filter-controls-plan.md`. `ui_core`
  now supports inclusive finite numeric range filters through `TableNumericFilterBound`,
  `ui_components` exposes the `TableRangeFilter` recipe and `TableRangeFilterChange` payload, and
  the Components gallery adds a focused `filter-board` score range proof with runtime logs.
  Updated `docs/ui/component-contract.md` and `docs/verification.md` so numeric range filtering is
  recorded as a shipped Table recipe. Targeted `cargo fmt` and `cargo nextest` commands passed for
  the core/components/gallery slice, and engineering wiki validation plus `git diff --check`
  passed. Next action is selecting the next Table boundary.
- 2026-06-24: Completed the Table cell editing slice from
  `docs/plans/2026-06-24-005-feat-ui-table-cell-editing-plan.md` in the working tree.
  `TableColumn::text_editable`, `TableCellEditor::Text`, `TableCellEditChange`, and
  `Table::on_cell_edit_change` now provide opt-in text-cell editors over app-owned row state.
  The `editable-release` gallery sample records controlled edit payloads, applies changes to a
  sample-owned `TableState` override, re-renders changed row text, and keeps read-only cells
  display-only. Updated `docs/ui/component-contract.md`, `docs/verification.md`, and
  `docs/knowledge/engineering/progress/2026-06-24-table-cell-editing-plan.md`; verified with
  focused component and gallery nextest runs, memory validation, and `git diff --check`. Next
  action is commit.
- 2026-06-24: Completed U3/U4 of
  `docs/plans/2026-06-24-004-feat-ui-table-faceted-filter-controls-plan.md` as `1298177`.
  `filter-board` now renders a status `TableFacetedFilter`, records app-owned filter overrides and
  `TableFacetedFilterChange` payloads in the gallery runtime log, recomputes summary rows from the
  controlled state, and has focused smoke coverage for popup wheel containment, selecting `Done`,
  filtered/final row counts, and clearing back to the original row window. `docs/ui/component-contract.md`,
  `docs/verification.md`, and engineering memory now record the recipe as shipped while keeping
  global faceting, async option search, and fetch/cache orchestration as follow-up work. Verified
  with `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`,
  `cargo nextest run -p open-gpui-ui-components table component_api_inventory crate_root_and_prelude_exports_remain_explicit`,
  `cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata components_page_conformance_gates_reference_core_and_gallery_contracts components_gallery_smoke_faceted_filter_updates_table_rows table`,
  and `git diff --check`. Next action is selecting the next Table boundary.
- 2026-06-24: Completed U2 of
  `docs/plans/2026-06-24-004-feat-ui-table-faceted-filter-controls-plan.md` as `cfcab3a`.
  `TableFacetedFilter` now productizes the categorical faceted filter recipe with search query
  control, popover policy control, checkbox facet rows, a controlled `TableFacetedFilterChange`
  payload that resets pagination to the first page, and crate-root / prelude exports. Verified
  with `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components`,
  `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table`, and `git diff --check`. Next action is U3:
  extend the gallery proof and contract evidence for the faceted filter recipe.
- 2026-06-24: Completed U1 of
  `docs/plans/2026-06-24-004-feat-ui-table-faceted-filter-controls-plan.md` as `a52751f`.
  `TableFilter` now carries `TableFilterKind`, preserving `contains` while adding exact
  categorical `one_of` / `exact` token filters with order-independent selected value sets.
  `TableFilterKind` is exported through `ui_core`, `ui_components`, and both preludes. Verified
  with `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components`,
  `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table`, and `git diff --check`. Next action is U2:
  add the faceted filter recipe in `ui_components`.
- 2026-06-24: Wrote `docs/plans/2026-06-24-004-feat-ui-table-faceted-filter-controls-plan.md`
  as the next Table follow-up boundary. The plan keeps scope to single-column categorical faceted
  filter controls, adds exact categorical filter semantics to `TableFilter`, reuses existing
  Popover + command-palette primitives for the UI recipe, and defers global faceting, numeric
  range sliders, async facet loading, and standalone headless extraction. Next action is U1:
  extend core `TableFilter` semantics.
- 2026-06-24: Completed the Table row-selection variants slice as `ea3785f`. `ui_core::TableState`
  now carries explicit selection policy knobs for single vs multiple selection, explicit-control
  vs row-click activation, and descendant propagation, plus renderer-neutral selection summaries
  for full-model and current-page scopes. `ui_components::Table` emits controlled
  `TableRowSelectionChange` payloads and keeps row-click selection distinct from activation when
  the policy is explicit-control. The Components test suite now covers row-click selection,
  explicit-control row clicks, and API inventory / export baselines for the new row-selection
  callback. Verified with `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table`, `cargo nextest run -p open-gpui-ui-foundation-gallery table`,
  and `git diff --check`.
- 2026-06-24: Completed the Table custom aggregation callbacks slice as `dded73b`. `TableState`
  now stores named custom aggregation callbacks, grouped rows resolve named custom aggregates
  through the renderer-neutral pipeline, `TableRenderPlan` exposes the callback count, and the
  Components gallery includes `grouped-custom-aggregation`. Verified with
  `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table`, `cargo nextest run -p open-gpui-ui-foundation-gallery table`,
  and `git diff --check`.
- 2026-06-24: Chose the next Table planning boundary as
  `docs/plans/2026-06-24-003-feat-ui-table-row-selection-variants-plan.md`. The slice covers
  checkbox, radio, and list-like row selection recipes on top of the existing stable selected-row
  id state, keeps selection semantics renderer-neutral in `ui_core`, keeps selection chrome and
  gestures in `ui_components`, and defers cell editing, server-synced selection persistence, and a
  general feature plugin system.
- 2026-06-24: Started the Table custom aggregation callbacks slice from
  `docs/plans/2026-06-24-002-feat-ui-table-custom-aggregation-callbacks-plan.md`. `TableState`
  now stores named aggregation callbacks, grouped rows resolve named custom aggregates through the
  same renderer-neutral pipeline as built-ins, `TableRenderPlan` exposes the registered callback
  count, and the Components gallery includes a focused `grouped-custom-aggregation` sample. Core
  checks passed, `cargo check -p open-gpui-ui-foundation-gallery` surfaced a `Send + Sync` bound
  gap on the stored callback type, and the next step is to tighten the callback wrapper and rerun
  validation.
- 2026-06-24: Completed the Table row-pinning / two-axis viewport slice as `725f859` on `main`.
  The later `docs(knowledge): sync row-pinning completion state` commit `bbc6633` refreshed the
  memory bundle after the code landed. The working tree was clean, `git diff --check` passed, and
  the next Table follow-up could start from the updated `main` line.
- 2026-06-24: Started the Table two-axis viewport follow-up from
  `docs/plans/2026-06-24-001-feat-ui-table-two-axis-virtualization-plan.md`. `ui_core` now has a
  renderer-neutral `GridViewport2D` plus `resolve_grid_viewport_2d`, and `ui_components::Table`
  exposes the combined row/center-column viewport when both axes are available without merging the
  underlying row and column virtualizer contracts. Focused core tests cover empty axes, clamped
  offsets, stable keys, and overscan behavior; focused gallery tests now prove the `row-pinning`
  sample still keeps wheel containment while surfacing the combined viewport contract. Next action
  is to finish the remaining docs / verification refresh and commit the slice.
- 2026-06-23: Implemented the Table row-pinning slice from
  `docs/plans/2026-06-23-009-feat-ui-table-row-pinning-plan.md`. `ui_core::TableState` now carries
  `TableRowPinning`, keep-pinned/page-only policy, and resolved `TableRowRegions`; `ui_components`
  exposes row-region render metadata, uses the center region as the only vertical virtualizer
  input, and renders fixed top/bottom pinned bands around the center scroll body. The Components
  gallery now has a `row-pinning` Table sample with row-region readouts and
  `components_gallery_smoke_row_pinning_table_scroll_stays_inside_sample`. Verified
  `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table`, and
  `cargo nextest run -p open-gpui-ui-foundation-gallery table`. Next action is docs/memory
  validation and commit.
- 2026-06-23: Wrote `docs/plans/2026-06-23-009-feat-ui-table-row-pinning-plan.md` as the next
  Table follow-up. The plan uses TanStack Table's `top` / `center` / `bottom` row pinning model and
  Fret's Rust row-pinning helpers as the main references. Scope is limited to core row pinning
  state, duplicate-free pinned row regions, keep-pinned versus page-only policy, center-only
  vertical virtualization, fixed top/bottom pinned bands in the GPUI adapter, focused Components
  gallery proof, and contract / verification memory updates. Row-selection controls, cell editing,
  synthetic summary rows, data fetching, standalone headless extraction, and full two-axis grid
  virtualization remain deferred. Next action is U1: add the core row-pinning state and visibility
  policy in `crates/ui_core/src/table.rs`.
- 2026-06-23: Implemented `docs/plans/2026-06-23-008-feat-ui-table-faceting-filter-metadata-plan.md`
  in the working tree. `ui_core::TableState` now resolves per-column facet metadata, deterministic
  unique value counts, numeric ranges, and manual/server facet payloads with cache-key coverage.
  `ui_components::TableRenderPlan` exposes faceting ownership and column facets through the public
  contract, and the Components gallery readouts prove `filter-board` client facets plus
  `server-paged` manual facet payloads for a 64-row server set. Updated the component contract and
  verification docs. Simplification review removed hot-path facet payload copies from core
  resolution and `TableRenderPlan`; focused code review found and fixed the NaN equality edge case
  for manual facet payload/cache-key comparisons. Verified `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`,
  `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table component_api_inventory crate_root_and_prelude_exports_remain_explicit`,
  `cargo nextest run -p open-gpui-ui-foundation-gallery table`, `git diff --check`, and
  engineering wiki validation. The slice is committed in the current change; next action is the
  next Table follow-up boundary.
- 2026-06-23: Wrote `docs/plans/2026-06-23-008-feat-ui-table-faceting-filter-metadata-plan.md`
  as the next Table follow-up. The plan uses TanStack Table's per-column faceted row model,
  unique-value count, and min/max references plus Fret's Rust-native faceting parity helpers. Scope
  is limited to per-column faceting metadata, deterministic unique value counts, numeric ranges,
  manual/server facet payloads, render-plan exposure, and gallery readouts. Global faceting,
  concrete faceted filter toolbar UI, async fetching, row pinning, selection variants, editing, and
  two-axis grid virtualization remain deferred. Next action is U1: add core facet value/count/range
  types and stable facet keying in `crates/ui_core/src/table.rs`.
- 2026-06-23: Completed `docs/plans/2026-06-23-007-feat-ui-table-manual-row-model-controls-plan.md`
  as `d6e5c0d`. `ui_core::TableState` now supports independent manual filtering and sorting via
  `TableStageMode`, `TablePagination::manual` carries server row-count/page-count metadata, and
  manual row-model stages preserve app-supplied snapshots while keeping row ids, selection, grouping,
  expansion, lookup, and cache keys stable. `ui_components::TableRenderPlan` exposes the manual
  stage modes and pagination totals, root/prelude exports include `TableStageMode`, and the
  Components gallery adds `server-paged` to prove an app-owned page snapshot plus total counts.
  Verified `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery -- --check`,
  `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table component_api_inventory`,
  `cargo nextest run -p open-gpui-ui-foundation-gallery table`, and `git diff --check`.
- 2026-06-23: Wrote `docs/plans/2026-06-23-007-feat-ui-table-manual-row-model-controls-plan.md`
  as the next Table follow-up after `bfa91df`. The plan narrows the server/data-source direction to
  TanStack-style manual filtering, sorting, and pagination controls plus row-count/page-count
  metadata, with real fetch/cache orchestration, full faceting payloads, row pinning, editing, and
  headless extraction deferred.
- 2026-06-23: Implemented the Table manual expansion / async children metadata slice in the working
  tree. `TableRowChildrenLoadState` now carries idle/loading/failed child metadata, source rows can
  be expandable before children load, `TableExpansionMode::Manual` preserves app-supplied
  ungrouped source snapshots, and the GPUI Table expansion payload includes loaded-child and
  child-load metadata. The Components gallery now has `server-tree`, a focused sample that starts
  with unloaded/loading/failed branches and simulates app-owned child loading after the
  `server-workspace` disclosure request. Updated the Table contract and verification docs, then
  verified `cargo nextest run -p open-gpui-ui-core table`,
  `cargo nextest run -p open-gpui-ui-components table component_api_inventory`, and
  `cargo nextest run -p open-gpui-ui-foundation-gallery table`. Next action is docs validation and
  commit.
- 2026-06-23: Wrote `docs/plans/2026-06-23-006-feat-ui-table-manual-expansion-plan.md` for the next Table slice. The plan narrows the follow-up to manual expansion and async child metadata: source rows can be expandable before children load, the core resolver keeps the current client-expanded path intact, and the gallery proof will simulate app-owned child loading instead of introducing a real fetch layer.
- 2026-06-23: Completed U5 of `docs/plans/2026-06-23-005-feat-ui-table-tree-data-plan.md` in the working tree. The Components gallery now includes `dependency-tree`, a nested source-tree Table sample with pinned identity/status lanes, controlled expansion, row activation logging, tree-depth summary metadata, and a focused smoke that proves disclosure expansion and keyboard activation work on the rendered sample. Focused Table tests now pass in `open-gpui-ui-components` and `open-gpui-ui-foundation-gallery`. Next action is U6: update the component contract, verification docs, and engineering memory, then run the final broad verification set and commit.
- 2026-06-23: Wrote `docs/plans/2026-06-23-005-feat-ui-table-tree-data-plan.md` for the next Table slice. The plan keeps source tree rows separate from synthetic grouped rows, reuses `TableExpansionState` for nested source hierarchy, adds row focus / activation / expansion payloads, and scopes the first gallery proof to a focused tree-data Table sample. Next action is U1: add source-row hierarchy to the core Table contract.
- 2026-06-23: Completed U5/U6 of `docs/plans/2026-06-23-004-feat-ui-table-column-virtualization-plan.md` as `25875d0`. The Components gallery now has `release-matrix`, a wide pinned Table sample with fourteen center metrics and a focused runtime smoke proving off-window center columns unmount/remount while left/right pinned lanes and the outer page stay fixed. The Table state row was reduced to capability-specific readouts so simple samples no longer show grouped/pinned zero-noise, and `docs/ui/component-contract.md` plus `docs/verification.md` now describe `TableCenterColumnWindowPlan` as shipped adapter behavior. Verified `cargo fmt -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata components_page_table_samples_expose_virtualized_row_model_contract components_gallery_smoke_focuses_catalog_family_and_restores_all_mode components_gallery_smoke_matrix_table_center_column_window_stays_inside_sample`, and `git diff --check`. Next action is to pick the next Table follow-up boundary: full two-axis grid virtualization, tree-data tables, or server-style table flows.
- 2026-06-23: Completed U4 of `docs/plans/2026-06-23-004-feat-ui-table-column-virtualization-plan.md` as `5d67277`. Added coverage that virtualized center columns keep full accessibility column indexes, that a center header entering the rendered window after horizontal scroll still emits the real `TableHeaderAction`, and that committed center sizing changes recompute the virtual center window geometry without renumbering rendered column identity. Existing pinned-center resize runtime coverage remains the drag callback proof. Verified `cargo fmt -p open-gpui-ui-components`, `cargo nextest run -p open-gpui-ui-components table component_api_inventory`, and `git diff --check`. Next action is U5: add a wide Table gallery proof with inspectable center-window state and focused runtime scroll gates.
- 2026-06-23: Completed U3 of `docs/plans/2026-06-23-004-feat-ui-table-column-virtualization-plan.md` as `3819ac6`. `ui_components::Table` now mounts center headers and body cells from the resolved `TableCenterColumnWindowPlan`, uses leading/trailing spacers to keep the center scroll geometry stable, and leaves left/right pinned lanes fully rendered. Runtime tests prove far-right center header/cell selectors are absent before horizontal scroll, appear after center-lane scroll, pinned x positions stay fixed, and row virtualization still advances independently after horizontal scrolling. Verified `cargo fmt -p open-gpui-ui-components`, `cargo nextest run -p open-gpui-ui-components table component_api_inventory`, and `git diff --check`. Next action is U4: preserve sorting, resize handles, accessibility indexes, and stable selector identity across virtualized center columns.
- 2026-06-23: Completed U2 of `docs/plans/2026-06-23-004-feat-ui-table-column-virtualization-plan.md` as `df27aa4`. `ui_components::TableRenderPlan` now exposes `TableCenterColumnWindowPlan` metadata from current center columns plus adapter-owned horizontal scroll input: total center width, visible/overscan ranges, leading/trailing spacer widths, rendered center columns, and whether the center lane is actively virtualized. Crate root and prelude exports include the new plan type. Verified `cargo fmt -p open-gpui-ui-components`, `cargo nextest run -p open-gpui-ui-components table component_api_inventory`, and `git diff --check`. Next action is U3: render virtualized center headers and body cells from the shared window.
- 2026-06-23: Completed U1 of `docs/plans/2026-06-23-004-feat-ui-table-column-virtualization-plan.md` as `94fdd59`. `VirtualizerState::resolve_known_size_window` now accepts exact item keys and sizes, computes exact total geometry plus visible/overscan ranges, and materializes only the rendered measurement window. Verified `cargo fmt -p open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-core virtualizer table`, and `git diff --check`. Next action is U2: add Table center-column window metadata from the shared horizontal scroll handle.
- 2026-06-23: Created `docs/plans/2026-06-23-004-feat-ui-table-column-virtualization-plan.md` as the next Table slice. The plan narrows the next two-dimensional virtualization step to center-column virtualization first: reuse the one-dimensional virtualizer with exact per-column widths, keep left/right pinned lanes fully rendered, render only the visible plus overscan center columns, and preserve the current row virtualizer. TanStack Table / TanStack Virtual provide the table-state-vs-virtualizer boundary and column spacer model; Fret provides the render-plan row-group split precedent. Next action is U1: add exact-size virtualizer window support.
- 2026-06-23: Finished the sticky pinned Table slice on top of `3273c1a`. `ui_components::Table` now keeps vertical wheel input inside pinned table bodies through the adapter-owned scroll handle, `release-rollup` carries explicit sticky pinned lane widths, and focused gallery Table mode proves that horizontally scrolling the center lane keeps left/right pinned cells and the outer Components page fixed. Verified `cargo nextest run -p open-gpui-ui-components table_runtime_pinned_body_scrolls_without_moving_parent`, `cargo nextest run -p open-gpui-ui-components table`, `cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_grouped_table_scroll_stays_inside_sample`, `cargo nextest run -p open-gpui-ui-foundation-gallery table`, `git diff --check`, and engineering wiki validation. Next action is to commit the slice.
- 2026-06-23: Created `docs/plans/2026-06-23-003-feat-ui-table-sticky-pinned-columns-plan.md` as the next Table slice after column sizing / resize. The plan follows TanStack's left/center/right column-family model and Fret's split row-group approach: left/right pinned lanes stay fixed, center header/body lanes share one horizontal scroll source, and existing vertical row virtualization remains one-dimensional. Next action is U1: derive adapter sticky-layout metadata from the existing `TableRenderPlan` region widths.
- 2026-06-23: Completed U5 from `docs/plans/2026-06-23-002-feat-ui-table-column-sizing-plan.md` by refreshing `current-state.md` and this session log with the completed Table sizing / resize slice. Engineering wiki validation and `git diff --check` passed for the docs-only memory update. Next Table follow-up should pick between sticky horizontal pinned-column layout and two-dimensional grid virtualization, with tree-data tables and server-style table flows still behind those foundations.
- 2026-06-23: Completed U4 from `docs/plans/2026-06-23-002-feat-ui-table-column-sizing-plan.md` as `3273c1a`. The Components gallery now includes a `release-resize` Table sample with controlled `TableColumnSizing`, app-owned resize change logging, visible total-width / resizable-column metadata, and selector-aligned smoke coverage proving a real resize drag updates the sample without leaking scroll work to the page shell. Verified focused gallery table smokes, `git diff --check`, and prepared the memory refresh as the final U5 step.
- 2026-06-23: Completed U3 from `docs/plans/2026-06-23-002-feat-ui-table-column-sizing-plan.md` as `426742a`. `ui_core` now has pure column resize state/update helpers with `OnChange` / `OnEnd` modes and LTR / RTL delta handling; `ui_components::Table` exposes callback-backed resize handles, controlled `TableColumnSizingChange` payloads, and public exports. Verified `cargo nextest run -p open-gpui-ui-core -p open-gpui-ui-components` passing 271/271 plus `git diff --check`; the handle gating also preserved header-click sort behavior for non-resizable samples.
- 2026-06-23: Created `docs/plans/2026-06-23-002-feat-ui-table-column-sizing-plan.md` as the next Table follow-up after grouped rows, aggregation, and pinned semantic regions. The plan explicitly uses local TanStack Table references for committed `columnSizing`, transient `columnResizing`, total-size helpers, and resize modes, plus Fret's TanStack parity tests and table resize handle adapter as implementation references. Scope is limited to renderer-neutral column sizing state, resolved widths / offsets, GPUI resize handles, controlled sizing callbacks, and focused gallery proof; sticky pinned-column layout, horizontal pinned scroll sync, autosize, column reorder, and two-dimensional virtualization remain deferred. Next action is U1: add core column sizing descriptors and caller-owned sizing state.
- 2026-06-23: Completed U6 of `docs/plans/2026-06-23-001-feat-ui-table-depth-plan.md`. Full verification now passes after hardening the Components gallery automation for the longer page: `cargo fmt --all`, `cargo fmt --all --check`, `cargo check -p open-gpui-ui-core --tests`, `cargo check -p open-gpui-ui-components --tests`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p open-gpui-ui-core table`, `cargo nextest run -p open-gpui-ui-components table`, `cargo nextest run -p open-gpui-ui-foundation-gallery table`, `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery` all passed. During that pass, the stale Command catalog selector was corrected to `gallery:component-command-sample:ranked-search`, the gallery page `ScrollHandle` was exposed through `GalleryShell`, and the long Components-page smokes now use directory jumps plus scroll-handle alignment for sections and interactive targets. The full gallery suite is green again at 74/74; the full components suite is green at 209/209. Next action is to start the next Table follow-up plan, most likely column sizing / resize semantics.
- 2026-06-23: Implemented U5 from `docs/plans/2026-06-23-001-feat-ui-table-depth-plan.md`. The Components gallery now includes `release-rollup`, a 320-row Table sample grouped by `team` with a mixed expansion state (`group:team=UI` and `group:team=Platform` expanded), built-in aggregate cells (`count(name)` and `sum(score)`), left-pinned `name`, and right-pinned `status`. `TableSampleStateSummary` and the visible state row now expose grouped/expanded row counts, group/leaf counts, aggregate count, explicit expansion input count, and pinned left/center/right column counts. Gallery tests assert aggregate cell values, collapsed descendant lookup by stable row id, pinned render-region order, focused Table mode rendering both `release-queue` and `release-rollup`, and grouped-table inner scroll containment via `components_gallery_smoke_grouped_table_scroll_stays_inside_sample`. Verified `cargo fmt --all --check`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, `cargo nextest run -p open-gpui-ui-foundation-gallery table`, `cargo nextest run -p open-gpui-ui-components table`, focused foundation-gallery metadata/conformance/catalog smoke tests, `git diff --check`, and engineering wiki validation. Next action is U6: commit/push U5, then pick the next Table follow-up boundary.
- 2026-06-23: Implemented U4 from `docs/plans/2026-06-23-001-feat-ui-table-depth-plan.md`. `TableColumnPinning` now owns left/right pinned column ids in `ui_core`, `TableState` includes pinning in equality/cache keys, and `TableResolvedState` exposes `TableColumnRegions` so renderers can consume left/center/right lanes after visibility and explicit ordering. The GPUI `TableRenderPlan` now carries `TableColumnRegionRenderPlan` metadata, cell/header regions, and stable header/body region debug selectors. Tests cover unknown/invisible pinned ids, moving columns between sides without duplicates, header/body region order, public exports, and runtime selectors. Verified `cargo fmt --all --check`, `cargo check -p open-gpui-ui-core --tests`, `cargo check -p open-gpui-ui-components --tests`, `cargo nextest run -p open-gpui-ui-core table`, `cargo nextest run -p open-gpui-ui-components table`, `cargo nextest run -p open-gpui-ui-foundation-gallery table`, `git diff --check`, and engineering wiki validation. Next action is U5: add grouped/aggregated/pinned Table gallery samples and focused gallery proofs.
- 2026-06-23: Implemented U3 from `docs/plans/2026-06-23-001-feat-ui-table-depth-plan.md`. `TableState` now owns `TableAggregation` specs keyed by column id, includes them in the cache key, and resolves built-in `count`, `sum`, `min`, `max`, and `average` aggregate cells for grouped rows. Group rows keep the grouping column value visible even when that same column has an aggregate spec. `ui_core::prelude` plus `ui_components` crate-root/prelude exports now include the group-row, expansion, and aggregation contract types. Updated Table contract and verification docs. Verified `cargo fmt --all --check`, `cargo check -p open-gpui-ui-core --tests`, `cargo check -p open-gpui-ui-components --tests`, `cargo nextest run -p open-gpui-ui-core table`, `cargo nextest run -p open-gpui-ui-components table`, and `cargo nextest run -p open-gpui-ui-foundation-gallery table`. Next action is U4: pinned left/center/right visible column regions and adapter proof.
- 2026-06-27: Added `context_menu_runtime_long_menu_scroll_stays_inside_surface` in `open-gpui-ui-components` to prove a 12-item default-open ContextMenu keeps wheel input inside its local `ScrollArea` while the surface stays fixed. The overlay gallery still verifies the point-anchor ContextMenu smoke, and the long-menu containment gate now lives at the component level because default-open overlay samples remain visually non-blocking. Verified `cargo fmt --all --check`, focused `cargo nextest run -p open-gpui-ui-components context_menu_runtime_long_menu_scroll_stays_inside_surface`, focused `cargo nextest run -p open-gpui-ui-components context_menu`, focused `cargo nextest run -p open-gpui-ui-foundation-gallery overlay_page_context_menu_samples_expose_point_anchor_contracts overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses`, and `git diff --check`.
- 2026-06-23: Implemented U1/U2 from `docs/plans/2026-06-23-001-feat-ui-table-depth-plan.md`. `TableResolvedRow` now supports leaf and group row kinds; `TableGroupRow` records grouping column/value, depth, parent id, first leaf id, and leaf count; `TableState` owns grouping and expansion inputs and resolves core -> filtered -> grouped -> sorted -> expanded -> paginated -> final. Expansion is keyed by stable row id and collapsed descendants remain addressable through lookup metadata. The GPUI Table adapter now renders from resolved row cells instead of requiring every row to have a source row, so group rows participate in the same one-axis virtualized row stream. Updated component contract and verification docs. Verified `cargo check -p open-gpui-ui-core --tests`, `cargo check -p open-gpui-ui-components --tests`, `cargo nextest run -p open-gpui-ui-core table`, `cargo nextest run -p open-gpui-ui-components table`, `cargo nextest run -p open-gpui-ui-foundation-gallery table`, engineering wiki validation, and `git diff --check`. Next action is U3: built-in aggregate metadata for group rows.
- 2026-06-23: Created `docs/plans/2026-06-23-001-feat-ui-table-depth-plan.md` as the next component-depth slice after Menu / ContextMenu. The plan deepens the existing official Table instead of repeating the v0 table / virtualizer work: implement grouped and expanded row-model stages, expose group rows and aggregate metadata in `ui_core`, split visible columns into pinned left/center/right regions for the GPUI adapter, and add focused Components gallery samples that prove grouped/pinned table behavior and nested scroll containment. Next action is U1: extend the core resolved row contract so one final row stream can contain both leaf rows and group rows.
- 2026-06-22: Started and implemented the Menu / ContextMenu depth slice from `docs/plans/2026-06-22-006-feat-ui-menu-context-menu-depth-plan.md`. `MenuState` now resolves action, checkbox, radio, separator, and submenu rows with stable tree paths, visible submenu rows, caller-owned checked metadata, activation payload kind/path/checked state, pure typeahead, keyboard submenu open/close targets, validated runtime submenu paths, and local scrollability. `ContextMenuState` reuses the same menu model with point-anchor placement and visible-surface placement sizing. The Overlay gallery now exposes rich item, typeahead, and long-scroll Menu samples plus rich and edge-long ContextMenu samples. Review follow-up fixed keyboard activation so item-level handlers fire before component-level handlers, matching pointer activation. Verified `cargo check -p open-gpui-ui-components --tests`, `cargo check -p open-gpui-ui-foundation-gallery --tests`, focused `cargo nextest run -p open-gpui-ui-components menu`, `cargo nextest run -p open-gpui-ui-components context_menu`, focused submenu/runtime gates, focused Overlay gallery menu/context gates, engineering wiki validation, and `git diff --check`. Hover corridor, menubar, OS menu bridge, app-menu registry, and global command dispatch remain deferred.
- 2026-06-22: Completed the Command component-depth plan through U5 as commits `a29ba1e`, `eb68aac`, `726115b`, `41c719a`, and `d383026`. `CommandState` now covers deterministic ranked results, controlled/default query ownership, optional multi-select selected chips, virtualized long result render plans, and app-owned `CommandIndexSnapshot` input with local-ranked, pre-ranked-filtered, and pre-filtered modes; global registries, dispatch buses, keybinding resolution, enablement engines, and async indexing remain app-owned. Focused gallery Command samples now cover ranked search, multi-select, a 10k-item virtualized command index, and indexed/loading metadata. Verified `cargo nextest run -p open-gpui-ui-components command`, `cargo nextest run -p open-gpui-ui-components component_api_inventory`, export/runtime-state guards, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-foundation-gallery command`, `components_page_samples_expose_component_metadata`, and `git diff --check`.
- 2026-06-22: Created `docs/plans/2026-06-22-005-feat-ui-command-depth-plan.md` from the component-depth roadmap. The plan keeps the next slice focused on `Command`: renderer-neutral ranked results, controlled query ergonomics, optional multi-selection, virtualized long result sets, app-owned index snapshot hooks, focused gallery samples, and contract / verification updates. Global command registries, dispatch buses, keybinding engines, and async indexing owned by the component crate remain deferred. Next action is to run `ce-work` on the plan.
- 2026-06-22: Recorded the next UI component-depth roadmap in `docs/knowledge/engineering/decisions/open-gpui-ui-component-depth-roadmap.md`. The product direction is to deepen existing complex families instead of adding more shallow primitives: start with `Command` fuzzy ranking / multi-select / virtualized results, then `Menu` and `ContextMenu` submenu and menu-item semantics, then advanced `Table` and `Tree` behavior, with focused polish on `Sidebar`, `ScrollArea`, `TextInput`, `Avatar`, and `Overlay` as usage exposes gaps. Updated `current-state.md` so the next action is a `Command` depth `ce-plan`.
- 2026-06-22: Started the gallery automation regression hardening slice. Wrote `docs/plans/2026-06-22-004-test-ui-gallery-automation-regression-plan.md`, added `components_gallery_smoke_focuses_every_focusable_catalog_entry` as a catalog-driven focused-mode matrix smoke, refactored the focused-table helper to reuse the catalog-driven focus path, and fixed the gallery structure so `radio-group` is a sibling section instead of living under the hidden `checkbox` block. Updated `docs/verification.md`, verified the new matrix gate plus the full `cargo nextest run -p open-gpui-ui-foundation-gallery` pass (72/72), and confirmed the matrix now covers every focusable official or state-contract catalog entry while still restoring `All components`.
- 2026-06-22: Completed U5 from `docs/plans/2026-06-22-003-feat-ui-api-overlay-productization-plan.md` as `029826e`. Components now support all-components and catalog-driven focused component-family viewing. Catalog cards enter focused mode for official families, `All components` restores the full conformance page, directory chips stay pure anchor jumps, and the page scroll reset key includes the focused family so family changes reset scroll. Added focused gallery smokes for catalog focus/restoration, focused Table nested scroll containment, and family-change scroll reset; hardened deep scroll smokes to use directory jumps where they still test full-page composition. Tree and VirtualizedList roots now focus their current keyboard target when clicked, and the gallery Tree smoke enters focused Tree mode before clicking the concrete `paper` row. Verified `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, full `cargo nextest run -p open-gpui-ui-components` passing 183/183, full `cargo nextest run -p open-gpui-ui-foundation-gallery` passing 71/71, and `git diff --check`. Subagent review `u5_tree_focus_review` accepted the Tree root click-to-focus behavior and recommended the focused-mode Tree smoke path.
- 2026-06-22: Completed U4 from `docs/plans/2026-06-22-003-feat-ui-api-overlay-productization-plan.md` as `f320da1`. The Overlay page now exposes an official `OVERLAY_CATALOG` with status, family, resolved-state type, coverage, sample selector, catalog selector, and behavior-gate metadata for Tooltip, HoverCard, Popover, Dialog, AlertDialog, Sheet, Menu, and ContextMenu. The page renders visible Overlay catalog cards without merging overlay samples into the Components catalog, and the contract / verification docs now name the overlay catalog gate. Verified `cargo fmt -p open-gpui-ui-foundation-gallery`, focused `cargo nextest run -p open-gpui-ui-foundation-gallery overlay_page_catalog_entries_have_signals_and_sample_selectors overlay_gallery_smoke_renders_catalog_entries_and_official_samples`, focused `cargo nextest run -p open-gpui-ui-foundation-gallery overlay`, full `cargo nextest run -p open-gpui-ui-foundation-gallery`, and `git diff --check`. Next action: start U5 focused component-family viewing.
- 2026-06-22: Completed U3 from `docs/plans/2026-06-22-003-feat-ui-api-overlay-productization-plan.md` as `acc6e91`. `Switch::on_click` was removed from the public builder API and replaced by `Switch::on_change`, keeping Switch aligned with the scalar value-change callback vocabulary. The API inventory, public-method baseline, component contract, and verification docs now classify Switch with `checked` plus `on_change`; the new runtime test clicks the real Switch root, verifies controlled feedback updates the checked value, and confirms disabled switches do not emit changes. Verified `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components component_api_inventory`, `cargo nextest run -p open-gpui-ui-components switch_runtime_click_emits_on_change_with_next_checked`, full `cargo nextest run -p open-gpui-ui-components`, full `cargo nextest run -p open-gpui-ui-foundation-gallery`, and `git diff --check`. Read-only review `u3_callback_review_light` found no blocking issues. Next action: start U4 Overlay catalog metadata and gate cleanup.
- 2026-06-22: Completed the U2 query-seed follow-up as `b40cb08`. `Combobox::query` and `Command::query` were renamed to `default_query`, their state contracts still expose current `query()` getters, the Components gallery shell now seeds those adapters through `default_query`, and the API inventory records `default_query -> query` for both components. Verified `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components component_api_inventory`, full `cargo nextest run -p open-gpui-ui-components`, full `cargo nextest run -p open-gpui-ui-foundation-gallery`, and `git diff --check`.
- 2026-06-22: Completed U2 from `docs/plans/2026-06-22-003-feat-ui-api-overlay-productization-plan.md` as `e4c46b9`. Renamed seed-shaped builders to `default_*` for `Tabs`, `RadioGroup`, `Toolbar`, `Tree`, `VirtualizedList`, `Menu`, and `ContextMenu`; kept `Sidebar::selected` as controlled app state while moving its focus seed to `default_focused`; updated the gallery sample builders, contract docs, and API inventory baselines accordingly. Verified `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components component_api_inventory`, full `cargo nextest run -p open-gpui-ui-components`, full `cargo nextest run -p open-gpui-ui-foundation-gallery`, and `git diff --check`.
- 2026-06-22: Completed U1 from `docs/plans/2026-06-22-003-feat-ui-api-overlay-productization-plan.md` as `293ec0d`. `crates/ui_components/tests/components.rs` now has a public API inventory covering official Components catalog entries plus overlay families, separates render inputs from controlled runtime inputs, records `default_*` seeds, policy hints, callbacks, and the `Tabs::selected` legacy seed exception, and compares top-level component public methods against a source-derived baseline so builder additions/removals require inventory updates. Verified `cargo fmt -p open-gpui-ui-components`, focused `cargo nextest run -p open-gpui-ui-components component_api_inventory`, full `cargo nextest run -p open-gpui-ui-components` passing 182/182, `git diff --check`, and engineering wiki validation.
- 2026-06-22: Wrote `docs/plans/2026-06-22-003-feat-ui-api-overlay-productization-plan.md` for the next UI slice. The plan starts with a public API inventory and guard tests, then normalizes controlled/default/policy builder semantics, callback naming, Overlay catalog metadata, and focused Components gallery inspection. The next execution step is U1 API inventory before broad component renames.
- 2026-06-22: Promoted `Tree` to the official Components surface. `open_gpui_ui_components::Tree` now composes `TreeState` with keyed GPUI runtime state, focus handles, expansion overrides, selection/toggle callbacks, and an inner `ScrollArea`; `TreeState` remains the renderer-neutral hierarchy contract and state-contract readout. The gallery now includes a `document-outline` Tree sample, `tree-renderer` conformance gate, runtime interaction log, keyboard expand/select smoke, nested scroll containment smoke, and compact-shell directory jump coverage. Verified `cargo nextest run -p open-gpui-ui-components` passed 180/180 and `cargo nextest run -p open-gpui-ui-foundation-gallery` passed 66/66.
- 2026-06-22: Promoted `VirtualizedList` to the official Components surface after committing the concrete renderer as `b05f856`, the gallery/docs slice as `08958cf`, and the shared-slice API follow-up as `b349f4b`. The gallery now has an official `VirtualizedList` catalog entry, 10k-item `release-navigation` sample, `virtualized-list-renderer` conformance gate, inner viewport wheel smoke, card-chrome wheel containment smoke, and a full-page PageDown plus Enter/Space activation smoke backed by a gallery runtime log. `VirtualizedList::from_shared_items` now accepts `Arc<[VirtualizedListItemDescriptor]>` instead of exposing `Vec` storage details. `VirtualizedListState` remains visible as a state-contract readout for keyboard/navigation semantics. Full `open-gpui-ui-components` passed 179/179 and full `open-gpui-ui-foundation-gallery` passed 64/64.
- 2026-06-22: Added controlled `TextInput::value(...).on_change(...)` ergonomics. The public builder now creates a keyed adapter controller when `on_change` is present, dispatches sanitized single-line values from GPUI text input mutations, and leaves `TextInputState` renderer-neutral. Focused `open-gpui-ui-components text_input` nextest passed.
- 2026-06-22: Pushed the feedback/tree/virtualized-list productization slice to `origin/main` as `474ac18` after rebasing onto remote `45d3199`. Created the follow-up plan for a real `VirtualizedList` GPUI renderer. The next slice should keep `VirtualizedListState` as the keyboard/navigation contract, compose it with `open_gpui_ui_core::VirtualizerState` for rendered windows, and promote `VirtualizedList` from `state-contract` to `official` only after gallery runtime scroll and keyboard gates exist.
- 2026-06-22: Productized the pulled feedback/tree/virtualized-list primitives in the Components gallery. `StatusCue` and `EmptyState` are now official rendered feedback components with catalog/signals/gallery/root-selector/theme/export coverage. `TreeState` and `VirtualizedListState` are now explicit `state-contract` catalog entries with separate readout selectors, state/action/helper/payload signals, and gallery readouts that do not imply completed renderers. Verified focused component and gallery gates, including the short-viewport Components smoke after raising the scroll helper budget for the longer page.
- 2026-06-22: Completed the Table / Virtualizer performance follow-up on top of pulled HEAD `f85e91a`: `TableState` now uses shared row storage plus a conservative cache key, `TableRuntime` caches resolved row models across scroll redraws, `VirtualizerState::resolve_fixed_window` materializes only the rendered fixed-height window, and Components gallery table samples are lazy static data with precomputed state summaries. Also reviewed the pulled `feedback`, `tree`, and `virtualized_list` primitives as suitable groundwork, with gallery/productization follow-up still needed. Verified `cargo nextest run -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery` passing 273/273.
- 2026-06-22: Completed Table adapter review follow-up: added `TableHeaderAction` / `Table::on_sort_requested`, kept virtualizer snapshot restoration measurement-only with live scroll offset precedence, disambiguated duplicate row ids for render and virtualizer keys, aligned header/body column minimum widths, updated Table verification docs, and verified `cargo nextest run -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery` passing 257/257. Remaining performance follow-up is row-model/virtualizer caching for the 10k gallery sample.
- 2026-06-22: Added the official `open_gpui_ui_components::Table` adapter, promoted Table into the Components gallery catalog/signals/page directory/conformance gates, added 10k-row and filtered table samples, verified focused gallery Table coverage with `cargo nextest run -p open-gpui-ui-foundation-gallery table`, then verified the full related suite with `cargo nextest run -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery` passing 253/253.
- 2026-06-22: Implemented `open-gpui-ui-core::table` and `open-gpui-ui-core::virtualizer`, then verified the core contract with `cargo nextest run -p open-gpui-ui-core` passing 39/39.
- 2026-06-22: Added ADR 0009 and extended `docs/ui/component-contract.md` plus `docs/verification.md` with the table / virtualizer product boundary, official Table gate, and split between pure `ui_core` contracts and GPUI adapter scroll ownership.
- 2026-06-21: Wrote `docs/plans/2026-06-21-001-feat-ui-table-virtualizer-roadmap-plan.md` and tightened the scope around table-core v0, virtualizer metrics/range v0, GPUI adapter recipe, gallery conformance, and official component gates. The execution order is U1 -> U2 -> U6 -> U3 -> U4 -> U5.
- 2026-06-21: Framed the next series around table / virtualizer design instead of a new headless crate. The planning context now uses `repo-ref/fret`, `repo-ref/tanstack-table`, and `repo-ref/tanstack-virtual` as the primary references.
- 2026-06-21: Stabilized the Components page by moving the directory into its own fixed strip above the page scroll area; replaced the flaky data-grid wheel-motion regression with a stable state-level contract assertion and kept the release queue horizontal scroll smoke as the runtime proof.
- 2026-06-21: Added gallery-level wheel isolation on the ScrollArea sample card so wheel gestures on the release-queue chrome stay local and do not leak to the page shell; kept the release queue runtime scroll proof intact.
- 2026-06-21: Rechecked the splitter and overlay contract surface at `a7f0b96`; focused splitter, overlay, and gallery composition nextest runs remained green, and no new behavior gaps were found.
- 2026-06-21: Added an Overlay gallery smoke for `AlertDialog` on the real trigger, cancel default focus, primary action close, and Escape dismissal; this filled the remaining overlay contract gap without changing the component implementation.
- 2026-06-21: Refreshed `docs/knowledge/engineering/current-state.md` and `docs/ui/component-contract.md` so the gallery scroll regression gate points at commit `14efadc` and the next action stays focused on the remaining overlay / splitter review; later AlertDialog work advanced `main` to `d64f5d6`, then the memory refresh advanced `main` to `a7f0b96`.
- 2026-06-21: Added gallery scroll hardening in `examples/ui-foundation-gallery/src/pages/components/render.rs` and added smoke coverage for navigation rail scrolling, constrained vertical Tabs scrolling, and ScrollArea wheel scrolling in `examples/ui-foundation-gallery/tests/foundation_gallery.rs`.
- 2026-06-21: Focused gallery and component nextest runs passed, including the existing overlay and splitter runtime gates.
- 2026-06-21: Updated `docs/verification.md` to record the new Components-page regression gates.
- 2026-06-20: `f5e5d3a` pushed to `origin/main` for close-recovery test source alignment.
- 2026-06-20: `54304fc` pushed to `origin/main` for the final close-recovery test fix.
- 2026-06-20: Remaining dirty files are confined to `crates/gpui_docking/*`; current pass treats them as likely formatting /整理 noise until proven otherwise.
- 2026-06-20: Focused docking verification completed with `cargo nextest run -p open-gpui-docking --tests` passing 597/597. The current `crates/gpui_docking/*` diff still reads as formatting / import-reorder churn rather than a confirmed behavior change.
- 2026-06-20: `repo-ref/fret` research points to a thin facade + deep helper split for diagnostics: `fretboard` only forwards CLI entry points, `fret-diag` owns the real tooling contract, and scroll/virtual-list logic separates `ScrollHandle` state from `visible_range`/`window_range` policy. That pattern is a better fit for our gallery than extracting a headless crate right now.
- 2026-06-20: Current repo state was refreshed after verification: working tree is clean, `main` matches `origin/main`, and the next meaningful plan should start fresh around scroll / popup / splitter rather than re-opening the old headless discussion.
