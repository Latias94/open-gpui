# Verification

Run the local Open GPUI gate with:

```sh
cargo run -p xtask -- verify
```

The gate runs:

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo check -p open-gpui-smoke-native`
- `cargo nextest run -p open-gpui-ui-core`
- `cargo nextest run -p open-gpui-ui-components`
- `cargo nextest run -p open-gpui-ui-foundation-gallery`
- `cargo run -p xtask -- scan-theme-drift`
- `cargo run -p xtask -- scan-import-boundary`
- `cargo run -p xtask -- scan-ui-contract`

For focused `open-gpui-canvas` work, run:

```sh
cargo fmt -p open-gpui-canvas
cargo check -p open-gpui-canvas --benches
cargo nextest run -p open-gpui-canvas
cargo check -p open-gpui-smoke-native
```

The canvas crate also has a large-canvas Criterion baseline:

```sh
cargo bench -p open-gpui-canvas --bench large_canvas
```

Use the benchmark to compare spatial-index, visible-query, and paint-frame culling changes. It is
not part of the default CI gate because benchmark timing is runner-dependent.

For focused `open-gpui-ui-core`, `open-gpui-ui-components`, or UI foundation gallery work, run:

```sh
cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
cargo check -p open-gpui-ui-core
cargo check -p open-gpui-ui-components
cargo check -p open-gpui-ui-foundation-gallery
cargo nextest run -p open-gpui-ui-core
cargo nextest run -p open-gpui-ui-components
cargo nextest run -p open-gpui-ui-foundation-gallery
```

The gallery package includes Components-page runtime smoke coverage for regressions that state-only
tests can miss: short-viewport page scrolling and navigation reset, navigation rail scrolling,
Select popup outside dismissal, nested ScrollArea wheel scrolling, vertical Tabs rail scrolling,
horizontal plus vertical Splitter pointer dragging, Table column resize dragging, and long Sidebar
internal navigation scrolling. Run the gallery package tests before relying on manual dogfood for
those paths.
Overlay gallery smoke coverage now also includes menu submenu hover-open and sibling branch
switching on the rich-items sample, so submenu branch visibility, local hover retention, and
old-branch dismissal are verified through the real gallery shell instead of only through
component-state tests.
Component-state coverage also includes `MenuSubmenuSurface` and `MenuSafeHoverCorridor`, which
prove renderer-neutral submenu placement inputs and safe-hover transition bounds for the floating
submenu panels.
The Components-page ScrollArea regressions also cover release-queue wheel isolation so scroll
gestures on the sample card chrome do not leak to the page shell.
Because the Components page now carries more depth samples, the longer-section smokes also rely on
catalog directory jumps and page-scroll handle alignment instead of only raw page wheel motion;
that keeps the focused inspection paths stable even as the page grows.
The Components page has two inspection modes: the full all-components conformance page, and a
catalog-driven focused component-family view. Directory chips remain pure anchor jumps. Focused
mode is entered from catalog cards and restored through the explicit `All components` control. The
focused-view proof includes a catalog-driven matrix that opens every focusable official or
state-contract catalog entry, plus focused runtime smokes for scroll reset and nested scroll
containment:

```powershell
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focuses_every_focusable_catalog_entry
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focuses_catalog_family_and_restores_all_mode
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_table_scroll_stays_inside_sample
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_mode_resets_page_on_family_change
```

For focused docking split, preview, motion, zoom, divider, and accessibility primitive work, keep
the gates aligned to the shared primitive boundary:

```sh
cargo fmt --all -- --check
cargo nextest run -p open-gpui-ui-core motion spring projection policy --no-fail-fast
cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast
cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests --no-fail-fast
cargo check -p open-gpui-docking-native
git diff --check
```

For docking render-authority convergence work, prove deterministic geometry through
`DockPresentationScene` parity rather than screenshot or pixel-level styling parity:

```sh
cargo fmt --all -- --check
cargo nextest run -p open-gpui-docking host_render_tests host_render_geometry_parity_tests host_presentation_scene_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_viewport_preview_tests host_viewport_preview_visual_tests host_viewport_route_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_divider_hit_map_tests host_accessibility_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_interaction_tests --no-fail-fast
cargo check -p open-gpui-docking
cargo check -p open-gpui-docking-native
git diff --check
```

This gate locks root, leaf, tab-bar, empty-space, floating-title/content, split child, splitter,
zoom, divider hit map, and accessibility rectangles to the same deterministic scene/layout
authority. The remaining render-measured probe is intentionally named
`render_tab_label_drop_scene_fact_probe` and may only publish tab-label facts whose final bounds
depend on GPUI text shaping, intrinsic title layout, or close-button layout.

Use narrower checks while iterating:

```sh
cargo nextest run -p open-gpui-ui-core split --no-fail-fast
cargo nextest run -p open-gpui-ui-components splitter gpui_adapter_maps_splitter_role --no-fail-fast
cargo nextest run -p open-gpui-docking geometry host_accessibility_tests host_divider_hit_map_tests workspace_resize_policy_tests graph_split_tests interaction --no-fail-fast
cargo nextest run -p open-gpui-docking spatial_navigation_tests host_zoom_focus_tests::host_focus_neighbor_command_uses_spatial_navigation host_accessibility_tests::accessibility_splitter_actions_resize_through_transaction_path host_accessibility_tests::accessibility_vertical_splitter_actions_target_vertical_axis host_divider_hit_map_tests host_interaction_tests::horizontal_splitter_drag_updates_width_fractions host_interaction_tests::vertical_splitter_drag_updates_height_fractions host_interaction_tests::splitter_drag_clamps_to_minimum_pane_size host_interaction_tests::corner_splitter_drag_updates_both_axes_through_rendered_events interaction::tests::corner_splitter_drag_produces_two_axis_resize_request interaction::tests::corner_splitter_drag_clamps_one_axis_without_corrupting_other_axis render::tests::divider_affordance_states_have_distinct_feedback_colors --no-fail-fast
cargo nextest run -p open-gpui-docking transition_plan_from_route_affordance_describes_source_marker source_hover_over_known_viewport_renders_target_drop_preview routed_preview_replacement_clears_old_target_overlay_without_stale_payload escape_clears_routed_marker_target_overlay_and_active_drag viewport_runtime_begin_payload_drag_clears_previous_routed_preview viewport_runtime_revalidates_routed_preview_release_against_current_policy --no-fail-fast
cargo nextest run -p open-gpui-docking transition_pane_clip_mounts_real_pane_content host_unzoom_command_retargets_from_active_zoom_sample dragging_tab_to_other_stack_center_moves_panel transition_plan_from_overlay_scene_uses_current_bounds_for_matching_layers transition_plan_keeps_preview_layers_at_current_target_bounds overlay_replacement_keeps_preview_layers_at_current_target_bounds --no-fail-fast
```

These checks prove capability alignment instead of pixel parity: tab insertion previews remain tab
previews, nested edge targets stay scoped to the pane that owns the guide, cross-window route
markers stay separate from target previews, zoom/focus produce deterministic descriptors, divider
and corner hits derive from the shared split hit map, and accessibility descriptors expose roles,
bounds, orientation, selected state, disabled state, and actions.

The shared motion runtime checks additionally prove that `open_gpui_ui_core` owns deterministic
timeline sampling, spring sampling, scalar values, model-neutral scalar samples, frame-demand
reasons, explicit model/preset resolution, layout projection data, motion policy validation,
terminal state, reduced-motion completion, and stable-identity retarget matching.
`ui_components::Splitter` uses the scalar controller and explicit committed-layout model for
programmatic fraction changes while keeping pointer drags immediate and policy-tested. Docking uses
the same scalar motion model for transition progress, keeps explicit custom timeline specs intact,
renders move/resize panes through renderer-neutral projection visual bounds plus final-size
clip/occlusion layers, and keeps pane, divider, visual-affordance, zoom, focus, tab, route, and
viewport semantics local. Transition pane clips mount real final-size pane content behind an
occlusion mask rather than generic placeholder rectangles, visual-affordance preview geometry stays
pinned to the current semantic target, adapter-owned transition executors still request GPUI
frames, and interrupted zoom/unzoom starts from the current sampled geometry. The native runtime
panel exposes this as
`motion proof: shared-runtime+run-state+scalar-value+scalar-sample+explicit-models+policy-gates+layout-projection+projection-clips+sampled-progress+retargeted-identity+reduced-motion-final-state+high-frequency-bypass`.
The remaining render-measured drop-scene probe is intentionally limited to tab-label facts whose
bounds depend on text shaping; presentation-scene facts own root, pane, tab bar, empty, and
floating-title targets.

Cross-window preview cleanup is part of the same semantic contract. A routed hover may leave a
source-window route marker and a target-window preview at the same time, but those are distinct
overlay layers: source route markers become `RouteMarker` transition descriptors, target previews
own payload tab previews, and replacing the route target must clear the old target overlay before a
release can commit. Escape cancellation during a real GPUI docking drag clears the active drag,
source marker, target preview, and runtime session together without making the docking host steal
ordinary panel focus when no drag is active.

Shared split primitive coverage now owns generic fraction normalization, one-fill-child share
resolution, and pixel-delta adjacent resize helpers in `open_gpui_ui_core`. Docking consumes those
helpers for graph normalization, render flex shares, presentation-scene split layout, and splitter
drag resize transactions. Docking-local geometry should remain limited to docking-specific
drop-guide boxes, central-region target policy adapters, and GPUI `Bounds<Pixels>` conversion.
Docking-private spatial navigation now resolves nearest pane focus targets from the current
presentation scene using direction filtering, perpendicular overlap priority, and distance
tie-breaking. The direction enum is a docking command input, while the rectangle-neighbor resolver
remains private to docking because it depends on docking pane semantics and rendered presentation
facts.

Current docking accessibility output maps supported descriptor data into GPUI element state:
stable IDs, roles, labels, selected/disabled state, orientation, numeric splitter values, tab
focus/select actions, and splitter increment/decrement actions. Docking keeps hint strings and
drop affordance descriptors in its renderer-neutral scene, but GPUI currently has no generic
element API for an accessibility hint/description field or a platform drop action callback. Active
drop, drag-source, and rejected-target affordance nodes are therefore exposed as labeled group
descriptors without inventing unsupported platform actions, and focused tests assert that those
nodes disappear when the visual affordance scene is empty.

Docking visual affordance runtime work should use `DockVisualAffordanceScene` as the visual
feedback authority for drop guides, tab insertion, route markers, divider/corner affordances,
focus rings, zoom egress, accessibility, visual-affordance motion identity, and native diagnostics.
Target previews, route markers, accessibility descriptors, transition plans, and runtime
diagnostics now consume visual affordance descriptors directly; no `DockOverlayScene` semantic
adapter remains. The native docking runtime panel reads runtime-owned visual affordance status and
shows one compact affordance line per viewport with layer count, active layer, scope/state, target
node, zone, payload index, frame generation, and visual-affordance motion state.

Focused visual affordance runtime gates:

```sh
cargo fmt --all -- --check
cargo nextest run -p open-gpui-docking host_viewport_preview_tests host_viewport_preview_visual_tests host_viewport_route_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_render_tests host_transition_tests host_render_geometry_parity_tests --no-fail-fast
cargo nextest run -p open-gpui-docking host_accessibility_tests host_divider_hit_map_tests host_debug --no-fail-fast
cargo nextest run -p open-gpui-docking host_interaction_tests host_outside_release host_viewport_drop --no-fail-fast
cargo check -p open-gpui-docking
cargo check -p open-gpui-docking-native
git diff --check
```

Native dogfood command:

```sh
RUST_LOG=info,open_gpui_docking=debug,open_gpui=info RUST_BACKTRACE=1 cargo run -p open-gpui-docking-native --bin open-gpui-docking-native 2>&1 | tee /tmp/open-gpui-docking-native.log
```

Table gallery gates now follow the same split: `open-gpui-ui-core` tests prove row-model,
manual row-model stages, manual expansion, child-load metadata, virtualizer, column sizing,
column-window, row pinning, and resize-math contracts without rendering, including grouped row ids,
expansion lookup behavior, expandable unloaded branches, built-in group-row aggregate cells,
pinned-column region splitting, center-column virtual windows, top/center/bottom row regions,
keep-pinned versus page-only policies, manual filtering/sorting/pagination cache keys, pagination
row/page totals, per-column facet metadata, manual facet payload cache keys, and on-end/on-change
resize deltas. `open-gpui-ui-components` tests prove adapter exports, state metadata, manual
row-model render-plan metadata, faceting render-plan metadata, row-pinning render-plan metadata,
expansion payload metadata, resize callback wiring, center-window header/body mounting, fixed
row-pinned bands, and scroll ownership; gallery smokes prove long table scroll input stays inside
the table viewport, `release-resize` column dragging updates the controlled sample without moving
the outer Components page, wide center lanes scroll independently from fixed left/right pinned
lanes, `row-pinning` keeps top/bottom row bands fixed while the center body scrolls, `server-paged`
renders an app-owned page snapshot with total counts plus caller-provided facet summaries,
`filter-board` exposes client-derived status counts and score ranges, and `server-tree` renders
app-owned manual child loading. The focused proofs are:

`components_gallery_smoke_grouped_table_pinned_center_scroll_stays_inside_sample` is the focused
sticky-pinned Table proof: it enters the Table family view, scrolls the `release-rollup` center
lane horizontally, and asserts left/right pinned lanes plus the outer Components page stay fixed.

`components_gallery_smoke_matrix_table_center_column_window_stays_inside_sample` is the focused
center-column virtualization proof: it enters the Table family view, scrolls the `release-matrix`
center lane horizontally, verifies far center metric cells are unmounted before scrolling and
mounted after scrolling, and asserts left/right pinned lanes plus the outer Components page stay
fixed.

`components_gallery_smoke_row_pinning_table_scroll_stays_inside_sample` is the focused row-pinning
proof: it enters the Table family view, aligns the `row-pinning` sample to an interactive center
cell, wheels inside the center body, and asserts the sample, top pinned band, bottom pinned band,
and left-pinned cells stay fixed while the center row window changes.

`components_gallery_smoke_table_server_tree_loads_children_from_expansion_request` is the focused
manual-expansion proof: it enters the Table family view, starts with `server-api` absent from the
app-supplied source snapshot, clicks the unloaded `server-workspace` disclosure, verifies the
expansion payload carries zero loaded children plus idle child-load state, and then confirms the
new child row renders after the gallery runtime supplies the loaded snapshot.

`components_gallery_smoke_faceted_filter_updates_table_rows` is the focused faceted-filter proof:
it enters the Table family view, opens the `filter-board` status popover, verifies wheel input on
the popup content stays local, selects the exact `Done` token, checks the controlled change payload
and filtered row counts, then toggles the token off and confirms the original row window returns.

`components_gallery_smoke_range_filter_updates_table_rows` is the focused numeric range proof:
it enters the Table family view, opens the `filter-board` score popover, verifies wheel input on
the popup content stays local, types a minimum score, checks the controlled
`TableRangeFilterChange` payload and filtered row counts against the same `TableState` contract,
and confirms a lower-score row leaves the rendered window.

`components_gallery_smoke_predicate_filter_updates_table_rows` is the focused predicate-filter
proof: it enters the Table family view, types into the `filter-board` name predicate control,
checks the controlled `TablePredicateFilterChange` payload and sample-owned predicate override,
verifies filtered/final row counts against the resolved `TableState`, and confirms the rendered
row window changes without moving the outer Components page.

`components_gallery_smoke_editable_table_cell_updates_sample_rows` is the focused text-cell editing
proof: it enters the Table family view, targets the `editable-release` sample, edits a rendered
`name` cell through the nested `TextInput`, verifies `TableCellEditChange` targets the stable
`(row_id, column_id)` pair, confirms the gallery applies the change to its app-owned `TableState`,
and proves a read-only `status` cell does not mount an editor.

`components_gallery_smoke_checkbox_table_cell_updates_sample_rows` is the focused checkbox editing
proof: it enters the Table family view, targets the `toggle-release` sample, toggles a rendered
`enabled` cell through the nested `Checkbox`, verifies `TableCellEditChange` targets the stable
`(row_id, column_id)` pair, confirms the gallery applies the bool change to its app-owned
`TableState`, and proves the checkbox cell does not mount a text editor.

`components_gallery_smoke_multiline_table_cell_updates_sample_rows` is the focused multiline
cell-editing proof: it enters the Table family view, targets the `multiline-release` sample, edits
a rendered `notes` cell through the nested `Textarea`, verifies the same `TableCellEditChange`
payload preserves newlines, confirms the gallery applies the change to its app-owned
`TableState`, and proves non-multiline/read-only cells do not mount the wrong editor.

`components_gallery_smoke_content_fit_table_cell_edit_widens_name_column` is the focused
content-fit proof: it enters the Table family view, targets the `content-fit-release` sample,
edits the visible `name` cell, verifies the sample keeps the fixed `score` lane anchored, and
proves the adapter-measured `name` column widens while header and body stay aligned.

`table_runtime_measured_row_height_reflows_after_paint` is the focused measured-row proof: it
renders a measured `Table` with wrapped body content, verifies the first row grows beyond the
fallback row height, and confirms the second row is laid out below the expanded row after the
measurement cache settles.

`components_gallery_smoke_select_table_cell_updates_sample_rows` is the focused select-edit
proof: it enters the Table family view, targets the `select-release` sample, opens a fixed-option
`Select` editor, picks `blocked`, verifies `TableCellEditChange` targets the stable
`(row_id, column_id)` pair, confirms the gallery applies the text change to its app-owned
`TableState`, and proves the select cell does not activate or select the row.

`open-gpui-ui-components` table tests also cover the select editor adapter path directly:
`table_behavior_snapshot_exposes_editable_leaf_cell_kinds_for_leaf_cells_only`,
`table_runtime_select_cell_edit_emits_change_without_row_interaction`, and the other table cell
edit gates prove the fixed-option `Select` editor stays a leaf-cell recipe rather than a new row
interaction path.
The Table modules are now verified by ownership layer. `open-gpui-ui-core` owns renderer-neutral
row-model, column, header, filtering, faceting, aggregation, sizing, selection, and virtualizer
contracts. `open-gpui-ui-components` owns the `Table` facade, behavior snapshots, crate-private
render-plan resolution, keyed runtime, header/body/cell/editor/resize element assembly, callback
payloads, and public export inventory.
`open-gpui-ui-foundation-gallery` owns the end-to-end samples and scroll containment proofs. For a
Table-only change, prefer the focused commands below before the full `xtask` gate; keep the public
surface and gallery-conformance commands when moving code between modules so source-owner drift is
detected early.

```powershell
cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
cargo check -p open-gpui-ui-core --tests
cargo check -p open-gpui-ui-components --tests
cargo nextest run -p open-gpui-ui-core table
cargo nextest run -p open-gpui-ui-components --test table --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery table
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata components_page_conformance_gates_reference_core_and_gallery_contracts components_gallery_smoke_focused_table_scroll_stays_inside_sample components_gallery_smoke_grouped_table_pinned_center_scroll_stays_inside_sample components_gallery_smoke_matrix_table_center_column_window_stays_inside_sample components_gallery_smoke_row_pinning_table_scroll_stays_inside_sample
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_faceted_filter_updates_table_rows components_gallery_smoke_range_filter_updates_table_rows components_gallery_smoke_predicate_filter_updates_table_rows components_gallery_smoke_column_visibility_updates_release_matrix components_gallery_smoke_resizable_table_resize_updates_sample components_gallery_smoke_grouped_table_column_reorder_updates_sample
cargo nextest run -p open-gpui-ui-core numeric_range_filters_match_finite_number_cells_inclusively numeric_range_filters_normalize_open_and_reversed_bounds categorical_filters_match_exact_tokens_and_multiple_values
cargo nextest run -p open-gpui-ui-components --test table --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test text_input --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata components_page_conformance_gates_reference_core_and_gallery_contracts components_gallery_smoke_focuses_catalog_family_and_restores_all_mode components_gallery_smoke_editable_table_cell_updates_sample_rows components_gallery_smoke_checkbox_table_cell_updates_sample_rows components_gallery_smoke_select_table_cell_updates_sample_rows components_gallery_smoke_multiline_table_cell_updates_sample_rows
```

Key sentinels inside those binaries include
`public_reexports_stay_explicit_without_wildcards`,
`crate_root_and_prelude_exports_remain_explicit`,
`table_public_exports_include_core_table_and_virtualizer_contracts`,
`component_api_inventory_uses_stable_ownership_vocabulary`,
`table_component_source_mapping_tracks_split_render_owners`,
`table_range_filter_state_resolves_bounds_and_popover_contract`,
`table_range_filter_change_updates_filters_and_resets_pagination`,
`table_behavior_snapshot_exposes_faceting_metadata`,
`table_behavior_snapshot_exposes_editable_leaf_cell_kinds_for_leaf_cells_only`, and
`controlled_text_input_on_change_accepts_input_without_supplied_controller`.

`VirtualizedList` follows the same split at component scale: `open-gpui-ui-components` tests prove
render-plan rows, scroll-target math, PageDown reveal, and Enter/Space activation payloads, while
the gallery metadata and smoke tests prove the official catalog entry, 10k-item rendered sample,
and inner scroll containment inside the overflowing Components page. The focused proof is:

```powershell
cargo nextest run -p open-gpui-ui-components virtualized_list
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_virtualized_list_scroll_stays_inside_sample
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_virtualized_list_card_wheel_does_not_leak_to_page
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_virtualized_list_keyboard_reveals_and_activates
```

The components package includes runtime smoke coverage for Switch, TextInput, Textarea, RadioGroup,
Listbox, Select, Combobox, Command, Tabs, and Toolbar keyboard navigation. The focused Switch test
renders a controlled switch, clicks its real root selector, verifies `on_change` receives the next
checked value, and confirms disabled switches do not emit changes. The focused TextInput tests
render a standalone controller-backed input, click its real root, accept simulated platform text,
sanitize single-line input, verify the controller caret ends at the inserted text, and assert
password display mode masks one glyph per grapheme while preserving the stored value. The focused
Textarea checks prove newline-preserving controlled payloads in component tests and inner viewport
wheel containment in the Components gallery. The focused
RadioGroup test renders real radio
items, rejects disabled clicks, skips disabled items with arrow navigation, verifies default
selection seeding, click and arrow-selection payloads, and confirms Space on an already selected radio does not emit a duplicate
selection change. The focused Listbox test renders real standalone, separator, and grouped options,
rejects disabled clicks, keeps arrow navigation selection-free, skips disabled/separator rows, and
verifies Enter and Space dispatch both option-level and listbox-level selection callbacks. The
focused Select test opens the real trigger, rejects disabled popup option clicks, verifies click and
keyboard selection payloads, closes after selection, and confirms popup Listbox arrow navigation
skips disabled rows. The focused Combobox tests click the controller-backed text input, type a
query, open the filtered popup by trigger and keyboard paths, verify filtered Listbox options, and
select filtered options with ordered select/open callbacks. The focused Command tests cover
renderer-neutral ranking, controlled and default query ownership, stable-value selection across
descriptor reorder, multi-select selected chips, virtualized result render plans, app-owned index
snapshots, core `CommandDescriptor` projection into Command/Menu/ContextMenu surfaces, inline and
dialog command filtering, keyboard activation, shortcut payloads, non-dialog content persistence,
and dialog Escape/outside press dismissal. Command ownership is split across
`command/descriptor.rs`, `command/model.rs`, `command/style.rs`, `command/render_plan.rs`, and
`command/runtime.rs`, while `command/mod.rs` remains the public builder facade. Menu,
ContextMenu, Tree, and Table behavior snapshots now follow the same source-owner discipline:
`menu/` owns descriptor/model/render-plan/runtime/style plus the facade, `context_menu/` owns the
point-anchor facade and neutral state, `tree/` owns descriptor/model/movement/render-plan/runtime
boundaries, and `table/behavior/` owns counts, columns, header, rows, and tree summary snapshots.
The focused gallery Command smoke renders ranked, multi-select, virtualized, and indexed/loading samples in focused family mode,
verifies selected chips, stable selected values, and snapshot metadata are inspectable, and
confirms wheel input on the virtualized sample does not move the surrounding card.
The Components-page command contracts also cover the `registry-dispatch` sample for
`CommandCenter` shortcut/dispatch projection plus empty shortcut diagnostics, and the
`provider-search` sample for
`CommandProviderSource` refresh into a rendered `CommandIndexSnapshot`, including provider request
id, query metadata, projected shortcuts, and empty shortcut diagnostics. The `context-stack`
sample proves that `CommandContextStack` scopes command descriptors and projects the GPUI keymap
binding active for the focused key context. The command crate provider lifecycle tests cover
center-issued request ids, bound responses, stale async responses being ignored without mutating
registry sources, explicit `CommandSourceHandle`/`CommandProviderHandle` unregister behavior, and
the `CommandProviderRefreshController` query/loading/response/snapshot pipeline. The command crate
also covers `CommandKeyBindingRegistry`, which lets app/plugin sources contribute command-id keyed
shortcut dictionaries, projects valid entries into concrete GPUI `KeyBinding` values, preserves
GPUI chord and key-context predicate semantics, reports missing-action or parse diagnostics without
panicking, reports same-context command shortcut conflicts, and returns an install report when
app shells append projected bindings into a GPUI keymap. Conflict coverage includes global
no-context bindings that overlap concrete context bindings under GPUI runtime precedence rules.
The UI component command tests now also cover `CommandPaletteProjection`, which adapts a
`CommandCenter` query/keymap projection into a `PreFiltered` `CommandIndexSnapshot`, provider
statuses, and shortcut diagnostics; `CommandPaletteController`, which coordinates palette query
changes across provider refresh controllers, refreshes registered synchronous providers, exposes
missing-provider ids for app-owned async tasks, ignores stale async responses through the existing
provider request guard, and wraps command-center query-history navigation so up/down history keys
can reuse the current query as a prefix and restore the draft query at the newest boundary; plus
`CommandProviderPaletteProjection`, which adapts a provider refresh projection into a `PreFiltered`
`CommandIndexSnapshot`, carries loading provider status into `CommandLoadingState`, and lets
`Command::provider_refresh_projection` bind query and snapshot metadata without app-owned snapshot
glue.
Run the focused proof with:

```powershell
cargo nextest run -p open-gpui-ui-components command
cargo nextest run -p open-gpui-ui-components command_palette_controller --no-fail-fast
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-command center_reports_command_key_binding_conflicts_and_install_report center_reports_global_key_binding_context_conflicts --no-fail-fast
cargo nextest run -p open-gpui-command center_projects_command_key_bindings_into_gpui_keymap center_reports_command_key_binding_projection_diagnostics --no-fail-fast
cargo nextest run -p open-gpui-command center_exposes_query_history_navigation memory_history_promotes_duplicate_queries memory_history_navigates_recent_queries_with_prefix --no-fail-fast
cargo nextest run -p open-gpui-command context_stack keymap_shortcut_projection_can_respect_context_stack center_context_stack_drives_scopes_keymap_and_provider_requests --no-fail-fast
cargo nextest run -p open-gpui-command source_and_provider_handles_unregister_their_runtime_state --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_descriptors --no-fail-fast
cargo nextest run -p open-gpui-ui-components command menu --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery command
```

For the reusable command ecosystem, also keep `docs/ui/command-ecosystem.md` current. It records
the split between GPUI `Action`/`Keymap` execution, the app-owned
`open_gpui_command::CommandCenter` facade, scoped source registration/unregistration, availability
guards, shortcut projection, dynamic provider responses, fuzzy search/history ranking, menu
projection, and command-id dispatch.

The focused Tabs test renders real tabs,
preserves the `default_selected` seed on the first frame, rejects disabled tab clicks, keeps manual
arrow navigation as focus-only, and activates focused tabs with Enter and Space. The focused
Toolbar test renders real toolbar items, moves roving focus with arrow/Home keys, skips disabled and
separator items, and activates the focused item with Enter.

The components package also includes low-state primitive coverage for Separator, Kbd, Progress,
Skeleton, Avatar, AvatarGroup, and AvatarGroupCount. Those tests verify resolved state branches,
explicit root/prelude exports, theme color intents, stable rendered debug selectors, decorative
separator semantics, progress clamping, indeterminate progress, Avatar fallback initials,
explicit accessible labels, size metrics, `Role::Image`, group visible/hidden counts, overflow
label state, and source metadata staying outside image-loading ownership. The gallery metadata and
short-viewport smoke tests also verify those primitives are listed as official catalog entries and
render visible samples with stable debug selectors.
The public API inventory gate lives in `crates/ui_components/tests/public_surface.rs`. Its focused
contract modules live under `crates/ui_components/tests/public_surface/`, while shared manifest
projectors live in `crates/ui_components/tests/support/public_surface/mod.rs`. The product source
of truth lives under `crates/ui_components/src/component_contract/`: `rows.rs` owns canonical
contract rows, `projections.rs` owns query APIs, `api_inventory.rs` owns public API inventory and
method baselines, `surfaces.rs` owns adjacent public-surface rows, and `source_mapping.rs` owns
source-owner projections. Tests and gallery consume those typed contract rows instead of reading gallery
source strings for shipped status. The component crate root and prelude both re-export the curated default surface from
`crates/ui_components/src/public_api/default.rs`; GPUI runtime adapter helpers remain explicitly
namespaced under `open_gpui_ui_components::gpui_adapter`. Key sentinels include
`component_api_inventory_covers_official_gallery_catalog`,
`component_api_inventory_uses_stable_ownership_vocabulary`, and
`component_contract_projection_functions_delegate_to_contract_rows`,
`component_contract_rows_are_split_by_responsibility`, and
`root_and_prelude_exports_match_contract_default_surface_intent`. Run the focused proof with:

```sh
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
```

That gate checks that every contract-official component has a matching API inventory row, that
overlay families are explicitly listed, that public method baselines catch top-level builder
drift, that render/controlled/default/policy vocabulary stays consistent, that root/prelude
default exports match contract intent, and that renderer-neutral resolved state remains free of
GPUI runtime types.

Accessibility contract coverage now has its own semantic gate. `ComponentA11yContract` validates
role/name/value/action facts without a live platform backend, while the existing GPUI adapter tests
continue to prove role, orientation, toggled-state, and action mapping into GPUI. Run:

```powershell
cargo nextest run -p open-gpui-ui-components a11y --no-fail-fast
```

That gate covers `A11yLabelSource`, `A11yDescriptionSource`, `A11yValueMetadata`,
`A11yValueKind`, `A11yContractError`, and `A11yContractViolation` across representative Button,
IconButton, Checkbox, Slider, NumberInput, Progress, Dialog, Menu, Listbox, Tree, Table,
VirtualizedList, and Splitter contracts. The Components gallery conformance gate also exposes
component-owned `COMPONENT_A11Y_EVIDENCE` plus gallery-owned `COMPONENT_A11Y_CLAIMS`
selector bindings, so sample selector metadata stays aligned with roles, label sources, value
metadata, orientation, and supported actions.

Theme portability is guarded by the theme focused gate:

```powershell
cargo nextest run -p open-gpui-ui-components theme --no-fail-fast
cargo run -p xtask -- scan-theme-drift
cargo run -p xtask -- scan-theme-schema
```

That gate keeps runtime `ThemeContext` rendering, code-built `ThemeDefinition` registration, and
the JSON loader facade working: `THEME_JSON_SCHEMA_VERSION`, `theme_json_schema`,
`theme_definition_from_json_str`, `theme_definition_from_json_file`, `register_theme_json_str`,
and `register_theme_json_file`. Production component render paths should resolve color intents from
`ThemeResolver::current(cx)` or an explicit snapshot; direct `ThemeResolver::resolve(...)` is a
legacy default-light compatibility path and should not appear in `crates/ui_components/src`
rendering code. Focus-ring painting follows the same rule: production render paths should use
`focus_ring_shadow_with_theme(...)`, while `focus_ring_shadow(...)` remains a default-light
compatibility helper guarded by the public-surface adapter tests.
Loader failures are structured as `ThemeLoadError` / `ThemeFileField` for unsupported schema
versions, missing identity fields, unsupported token or state names, duplicate token/state pairs,
and invalid RGB values.

The foundation component family gate covers the shipped disclosure, numeric, navigation, display,
action, and feedback additions: Accordion, Collapsible, Slider, NumberInput, ToggleGroup, Link,
Breadcrumb, Tag, and ToastStack. These tests keep one canonical API per family, explicit
root/prelude exports, ownership vocabulary, resolved-state purity, official catalog metadata, and
focused Components-page rendering aligned:

```powershell
cargo nextest run -p open-gpui-ui-components --test public_surface --test form --test navigation --test primitives --test theme --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery official_component_catalog_entries_have_signals_and_sample_selectors components_gallery_smoke_focuses_every_focusable_catalog_entry components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation
```

The choice and runtime seams are guarded separately: `choice.rs` owns stable-value and normalized
query helpers for `Command`, `Combobox`, and `Select`; `roving_focus.rs` owns the shared enabled-item
navigation targets used across listbox-like surfaces; and `menu/runtime.rs` owns submenu hover
timing plus local scroll state for `Menu` and `ContextMenu`.
Feedback coverage now promotes `StatusCue` and `EmptyState` as official rendered Components
catalog entries. The focused component tests verify root/prelude exports, feedback intent labels,
resolved roles, metrics, and theme color intents. The gallery metadata tests require their
component/state `SIGNALS` entries and stable `gallery:component-status-cue-sample:{id}` /
`gallery:component-empty-state-sample:{id}` selectors, while the short-viewport smoke verifies the
real `status-cue:*:root` and `empty-state:*:root` debug selectors render.
`official_component_catalog_entries_have_signals_and_sample_selectors` is the gallery contract
gate for catalog drift: every official `COMPONENT_CATALOG` entry must have matching component and
resolved-state `SIGNALS` entries plus one rendered `gallery:component-*-sample:{id}` selector in
the Components page.
`gallery_story_contracts_cover_components_state_readouts_and_overlays` is the story-probe contract
gate. It requires official component samples, renderer-neutral state readouts, and overlay samples
to expose a reusable `StoryContract` with public selectors and user-observable probe operations:
open, dismiss, select, edit, scroll, focus, activate, and read-public-payload. Gallery smokes should
prefer `component_story_contract_for(name)` and `component_story_contracts_for_focus(mode)` before
falling back to raw debug selectors for adapter-internal details. The official sample selector
pairs and state readout selector pairs are derived from those story contracts so focused catalog
traversal and selector metadata stay aligned.
`state_contract_catalog_entries_have_signals_and_readout_selectors` is the companion pre-renderer
contract gate. Entries marked `state-contract` must declare `state_contract_selector`, must not
declare official `sample_selector`, and must stay disjoint from `official_sample_selector_pairs`.
The current state contracts are `TreeState` and `VirtualizedListState`; their signals cover state,
descriptor, action/result, helper, and payload types. `TreeState` remains a reusable hierarchy
contract even though `Tree` is now an official rendered component, matching the
`VirtualizedListState` / `VirtualizedList` split. The Components page smoke also verifies every
`state_contract_readout_pairs()` selector is visible.
The official Table gate requires `Table`, `TableState`, `VirtualizerState`,
`TableFacetedFilter`, `TableRangeFilter`, `TablePredicateFilter`, `TableColumnVisibility`, role
signals for table rows and cells, and at least one `gallery:component-table-sample:{id}` selector.
Table smokes and state tests assert that rendered row selectors stay bounded by the virtualizer's
visible rows plus overscan, scroll input stays inside the table viewport, sortable header actions
emit state-update payloads, controlled column resize callbacks carry stable sizing payloads,
categorical faceted filters emit controlled exact-token updates, numeric range filters emit
controlled finite-bound updates, predicate filters emit controlled operator/value updates, column
visibility emits controlled hide/show payloads, editable text cells emit controlled stable
row/column change payloads without
triggering row interaction callbacks, row activation and expansion request payloads stay controlled,
source-tree row
models keep nested descendants addressable by stable row id, manual source-tree snapshots expose
unloaded/loading/failed child metadata, row-pinning regions split top/center/bottom rows with
keep-pinned and page-only policies, and grouped / expanded row models keep collapsed descendants
addressable by stable row id, controlled column order changes emit stable before/after placement
payloads, and the reorder helpers keep the rest of `TableState` untouched. The Components gallery
now carries `release-rollup`, a grouped Table sample that mixes expanded and collapsed team groups,
exposes aggregate count and score cells, pins the identifier and status columns, and has its own
sticky-header plus inner-scroll smoke. It also carries
`server-paged`, a manual filtering/sorting/pagination sample that renders only the current
app-supplied page snapshot while exposing server-known total row and page counts through the
gallery summary and `TableBehaviorSnapshot`. It also carries `release-resize`, a controlled
column-sizing sample whose resize smoke drags the `name` handle, records the app-owned committed
width, and verifies header and first-row cell widths stay aligned. `filter-board` is also the
faceted-filter proof: it renders a `status` `TableFacetedFilter`, records
`TableFacetedFilterChange` payloads in the sample runtime log, proves selecting `Done` changes the
rendered row window, proves clearing restores it, and confirms popup wheel input does not move the
outer table sample. It also renders a score `TableRangeFilter`, records
`TableRangeFilterChange` payloads in the same runtime log, applies the range to a sample-owned
`TableState` override, proves filtered/final row counts match the core contract, and confirms
popup wheel input stays local. It also renders a name `TablePredicateFilter`, records
`TablePredicateFilterChange` payloads in the same runtime log, applies the operator/value
predicate to a sample-owned `TableState` override, and proves the rendered row window follows the
core filtered row model. `release-matrix` also renders a `TableColumnVisibility` toolbar
control, records `TableColumnVisibilityChange` payloads in the sample runtime log, applies
visibility overrides to the sample-owned `TableState`, proves hiding a metric column removes its
header and cells, proves show-all restores the column, and confirms popup wheel input stays local.
`release-rollup` now also proves controlled column-order changes: the sample runtime log records
`TableColumnOrderChange` payloads, applies the app-owned override to the sample `TableState`, and
shows the score column re-rendering before team while the sample card stays anchored.
`components_gallery_smoke_grouped_table_scroll_stays_inside_sample` is the focused vertical
sticky-header proof: it enters the Table family view, wheels the `release-rollup` body, and
asserts the header band stays fixed while the body row window advances.
`editable-release` is the text-cell editing proof: it renders editable `name` and `team` columns,
keeps `status` read-only, records `TableCellEditChange` payloads in the sample runtime log, applies
changes to a sample-owned `TableState` override, and proves the changed row text re-renders through
the normal Table pipeline.
`release-matrix` is the wide center-column virtualization and column-visibility sample: it pins the
identity and status lanes, exposes fourteen center metrics, locks identity/status visibility, and
has focused smokes that prove off-window center columns unmount/remount, hide/show visibility
changes update rendered headers/cells, and horizontal / popup wheel input remains inside the
sample. `row-pinning` is the row-region sample: it pins top and bottom review rows around a paged center body, exposes
top/center/bottom readouts, and proves center-body wheel input changes the center row window
without moving the fixed row bands or outer sample. The Table adapter keeps the row and column
virtualizers separate internally; public tests assert the resulting two-axis behavior through
`TableBehaviorSnapshot` plus gallery runtime probes. `dependency-tree`
is the source-hierarchy
sample: it proves nested `TableRow` children resolve to visible tree rows,
keeps collapsed descendants addressable by stable id, exposes tree-depth and tree-branch summary
metadata, and drives controlled expansion plus row activation through the gallery runtime log.
`server-tree` is the manual-expansion sample: it preserves the app-supplied source snapshot,
renders unloaded, loading, and failed branch affordances, records loaded-child and load-state
metadata in expansion payloads, and proves that child rows appear only after the gallery runtime
supplies the loaded snapshot.
Core table tests also assert that `TableAggregation` exposes stable built-in aggregate labels,
resolves count, sum, min, max, and average cells for grouped rows without hiding the grouping
column value, and lets `TableState::with_aggregation_fn` resolve named custom aggregate callbacks
with safe empty fallback for unknown names. Core and component tests assert that
`TableColumnPinning` splits visible columns into
left, center, and right regions after visibility/order resolution, ignores unknown or invisible
pinned ids, removes moved columns from their previous pinned side, and exposes matching
header/body region metadata and debug selectors. They also assert `TableRowPinning` deduplicates
ordered top/bottom ids, ignores unknown/filtered/collapsed rows, preserves pinned rows outside the
current page by default, supports page-only behavior, feeds only center rows into the vertical
virtualizer, and renders fixed row-pinned bands around the scrollable center body.
The official Tree gate requires `Tree`, `TreeState`, `TreeMetrics`, tree/tree-item role signals,
and at least one `gallery:component-tree-sample:{id}` selector. Component runtime tests verify
expansion, reveal, and selection payloads; gallery smokes verify keyboard expansion/selection
through the sample runtime log and prove Tree wheel input stays inside the sample viewport.
`TreeChildrenLoadState` adds the lazy-branch gate: unit tests prove expanded unloaded/loading/failed
branches do not synthesize fake child rows, toggle payloads carry loaded-child and load-state
metadata, and loading branches do not repeat toggle requests. The `remote-workspace` gallery sample
proves unloaded, loading, loaded, and failed branch affordances plus runtime payload metadata.
Tree typeahead is covered by a pure state test and a runtime adapter test: the pure helper matches
visible, focusable row labels with wraparound and skips disabled/collapsed rows, while the rendered
adapter buffers printable keys and moves focus without selecting. The `document-outline` gallery
smoke now verifies typing `n o` focuses the visible Notes row after the expand/select path.
Tree and virtualized-list state-contract samples are verified through
`components_page_samples_expose_component_metadata`: Tree readouts assert visible flattening,
disabled-row position skipping, navigation skipping, toggle payloads, and Enter/Space selection
actions; virtualized-list state-contract readouts assert active/selected indices, PageUp/PageDown
clamping, activation payloads, viewport item count, overscan, and semantic scroll strategy labels.
The same metadata test now also checks the official `Tree` sample's role metadata and keyboard
toggle payload, the official `remote-workspace` Tree sample's child-load metadata, plus the
official `VirtualizedList` sample's 10k item count, listbox roles, active/selected state, visible
range, and overscan summary.

The focused Tree proof is:

```powershell
cargo nextest run -p open-gpui-ui-components tree_state_resolves_lazy_branch_load_metadata_without_synthetic_children tree_toggle_payload_includes_child_load_state_and_blocks_loading feedback_tree_and_virtualized_list_public_exports_remain_explicit
cargo nextest run -p open-gpui-ui-components tree_typeahead_targets_visible_focusable_items_from_current_focus tree_runtime_typeahead_focuses_visible_matching_row
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata components_gallery_smoke_tree_expands_and_selects components_gallery_smoke_tree_lazy_branches_emit_load_metadata
```

The gallery package also includes a compact-shell runtime smoke that switches the gallery to the
compact viewport policy, verifies the derived mobile shell and compact density, scrolls the left
navigation rail to deep pages, and confirms switching away and back resets the page scroll position.

The gallery package also includes Overlay-page runtime smoke coverage for popover, modal dialog,
alert dialog, non-modal sheet, menu, and ContextMenu right-click hotspot opening plus Escape
dismissal. Popover and Dialog smokes open the real component trigger, assert Dialog initial focus,
and assert focus restoration to the trigger after outside press, modal barrier dismissal, and
Escape dismissal. The AlertDialog smoke opens the real trigger, confirms the cancel action gets the
default focus, verifies the primary action closes the dialog, and confirms Escape dismissal
restores focus to the trigger. The Overlay gallery intentionally keeps default-open contract
samples visually closed at page load so modal barriers and floating layers do not block page
scrolling; the metadata rows still report each sample's resolved default-open contract.

The focused Overlay catalog gates are:

```powershell
cargo nextest run -p open-gpui-ui-foundation-gallery overlay_page_catalog_entries_have_signals_and_sample_selectors
cargo nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke_renders_catalog_entries_and_official_samples
```

The `open-gpui-ui-core` overlay tests are the renderer-neutral gate for shared overlay behavior.
They should cover layer kind, presence, outside-press policy, Escape policy, focus restore intent,
initial focus intent, and `resolve_overlay_placement` side/alignment/fit/trace behavior for
explicit neutral placement inputs without opening a GPUI window.
The `open-gpui-ui-components` overlay helper tests should cover the GPUI adapter mapping for
deferred priority, snap margin, anchor conversion, placement resolution, outside-press open-change,
and Escape open-change without introducing a global overlay runtime. Trigger-anchored components
that do not provide measured trigger/content bounds should not be documented as owning
safe-bounds flip/shift at render time until a measured overlay runtime exists.
For GPUI runtime focus assertions, `VisualTestContext::debug_selector_is_focused` and
`VisualTestContext::focused_debug_selector` are the preferred test hooks. They use test-only
debug-selector-to-focus-handle data and keep focus checks independent from component internals.
The public surface manifest keeps adapter-only, renderer-neutral state, primitive, gallery, and
docs ownership explicit while the UI component architecture is being deepened. Table, Tree,
VirtualizedList, and Command expose behavior snapshots or state readouts; renderer assembly plans
stay crate-private unless a future component deliberately promotes a narrower state contract.
For the UI architecture deepening refactor, keep the focused gates below close to the code that
changes them. They cover the component contract rows, public export map, removed primitive
aliases, overlay runtime policy, choice/search behavior, the Command ownership split, the Table
behavior-snapshot and internal render-plan boundary, shared row-window projection, theme registry,
and gallery catalog/conformance/runtime/sample/render module split:

For the deep UI framework module refactor, run the focused ownership gates below before the full
workspace gate. They cover runtime theme context, typed a11y evidence, removed registry history,
shared overlay placement, `open_gpui_ui_core::grid_viewport::RowWindow`, gallery story-contract
projection, and `open_gpui_command::CommandDescriptor` projection:

```powershell
cargo fmt --all
cargo check -p open-gpui-ui-core --tests
cargo check -p open-gpui-ui-components --tests
cargo check -p open-gpui-ui-foundation-gallery --tests
cargo nextest run -p open-gpui-ui-core overlay grid_viewport --no-fail-fast
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components theme a11y menu context_menu command --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_descriptors --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery component --no-fail-fast
cargo run -p xtask -- scan-theme-drift
cargo run -p xtask -- scan-theme-schema
cargo run -p xtask -- scan-ui-contract
rg -n "ThemeResolver::resolve\(" crates/ui_components/src -g "*.rs"; if ($LASTEXITCODE -eq 0) { exit 1 } elseif ($LASTEXITCODE -ne 1) { exit $LASTEXITCODE } else { exit 0 }
git diff --check
```

For the contract-backed family-boundary refactor, run the focused ownership gates whenever
`component_contract` source mappings or the Menu, ContextMenu, Tree, or Table behavior owners move:

```powershell
cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
cargo check -p open-gpui-ui-components --tests
cargo check -p open-gpui-ui-foundation-gallery --tests
cargo nextest run -p open-gpui-ui-components public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-components menu --no-fail-fast
cargo nextest run -p open-gpui-ui-components context_menu --no-fail-fast
cargo nextest run -p open-gpui-ui-components tree --no-fail-fast
cargo nextest run -p open-gpui-ui-components table --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery overlay --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery tree --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery table --no-fail-fast
```

For component contract, a11y, gallery conformance, and theme productization work, start from the
reusable UI contract audit before dropping to focused behavior tests:

```powershell
cargo run -p xtask -- scan-ui-contract
cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
cargo check -p open-gpui-ui-components --tests
cargo check -p open-gpui-ui-foundation-gallery --tests
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-components a11y --no-fail-fast
cargo nextest run -p open-gpui-ui-components theme --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata components_page_conformance_gates_reference_core_and_gallery_contracts --no-fail-fast
```

`scan-ui-contract` checks the component contract tables, default root/prelude exports, source
homes, docs tokens, removed primitive targets, gallery conformance evidence, representative
`COMPONENT_A11Y_EVIDENCE`, gallery `COMPONENT_A11Y_CLAIMS`, and the committed theme schema
artifact. Use the narrower
`scan-theme-schema`, `scan-theme-drift`, and focused nextest commands when investigating a specific
failure.

Run the full component and gallery package gates only after broad contract-table, theme, or gallery
changes:

```powershell
cargo nextest run -p open-gpui-ui-components --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery --no-fail-fast
```

The Components gallery root keeps `runtime`, `samples`, and render ownership private. Stable
gallery API names are re-exported explicitly from `components.rs`; sample families live under
`examples/ui-foundation-gallery/src/pages/components/samples/`, runtime probes under
`examples/ui-foundation-gallery/src/pages/components/runtime/`, and render orchestration/readouts
under `examples/ui-foundation-gallery/src/pages/components/render/`. Source-contract tests should
reject `pub mod runtime`, `pub mod samples`, `pub use runtime::*`, and `pub use samples::*` in the
Components facade.

```powershell
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-core overlay
cargo nextest run -p open-gpui-ui-components --test overlay --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test choice --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test table --no-fail-fast
cargo nextest run -p open-gpui-ui-core virtualizer
cargo nextest run -p open-gpui-ui-components --test layout --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test theme --no-fail-fast
cargo run -p xtask -- scan-theme-drift
cargo nextest run -p open-gpui-ui-foundation-gallery --no-fail-fast
```

The binary-level gates above include these focused sentinels:
`primitive_deletion_target_inventory_blocks_removed_shallow_reexports`,
`primitive_modules_do_not_reexport_ui_core_as_pass_through_aliases`,
`surface_manifest_classifies_public_surface_once`,
`surface_manifest_aligns_adjacent_gallery_statuses`,
`surface_manifest_tracks_exports_gallery_and_docs_contracts`,
`adapter_only_public_surfaces_match_allowlist`,
`gpui_adapter_exports_group_runtime_specific_surfaces`,
`overlay_open_change_helpers_match_core_policies`,
`dialog_runtime_respects_escape_policy_and_restores_trigger_focus`,
`choice_surfaces_share_stable_value_resolution_and_query_normalization`,
`table_component_source_mapping_tracks_split_render_owners`, `row_window`,
`command_component_source_mapping_tracks_split_owners`,
`virtualized_list_behavior_snapshot_uses_item_descriptors_and_virtualizer_contracts`,
`virtualized_list_behavior_snapshot_applies_builder_metrics`,
`table_behavior_snapshot_exposes_center_column_summary_without_window_internals`,
`table_behavior_snapshot_exposes_pinned_column_regions`, `theme_registry`, `theme_resolver`,
`theme_snapshots`, `components_catalog_metadata_is_separate_from_rendering`,
`components_catalog_consumes_component_contract_rows`,
`official_component_catalog_entries_have_signals_and_sample_selectors`,
`state_contract_catalog_entries_have_signals_and_readout_selectors`,
`gallery_story_contracts_cover_components_state_readouts_and_overlays`,
`components_gallery_smoke_focuses_catalog_family_and_restores_all_mode`, and
`components_gallery_smoke_focuses_every_focusable_catalog_entry`.

The theme drift scan is the focused gate for component color recipes and built-in theme token
coverage. It requires all `ThemeResolver::*_colors` component calls to be implemented and listed
in `crates/ui_components/src/theme/recipes.rs`, rejects component-local `impl ThemeResolver`
extensions, and checks that light, dark, and high-contrast palettes expose the same token/state
shape. Add or move recipes in the theme module first, then update the catalog entry in the same
patch.

The `open-gpui-ui-components` public contract tests should also keep
`public_resolved_state_contracts_avoid_gpui_runtime_types` passing. That test is the hard
headless-readiness guard for public resolved-state structs: it prevents `Window`, `App`,
`Context`, `RenderOnce`, `IntoElement`, `ElementId`, `Entity`, focus handles, scroll handles, and
callback storage from entering state contracts. The companion extraction-blocker inventory tests in
`open-gpui-ui-components` and `open-gpui-ui-core` pin the extraction gate deliberately. Component
public-state blockers are currently empty: resolved overlay contracts expose `OverlayResolvedState`, while
`GpuiOverlayState` stays in the GPUI adapter helper surface for deferred priority and snap margin.
Public component metrics and accessibility state now use neutral UI-core vocabulary; adding public
GPUI `Pixels`, `Bounds`, `Point`, or `Size` aliases to resolved-state contracts should fail the
guard inventory. `open-gpui-ui-core` is now renderer-neutral: it has no `open_gpui` dependency,
no UI-core source references to `open_gpui`, and no `UiPx` conversion impls for GPUI style types.
Adaptive policies accept neutral `UiPx` thresholds and inputs instead of GPUI `Pixels`; GPUI
callers should convert their concrete window or viewport width at the adapter boundary before
invoking UI-core adaptive helpers. The companion strict-boundary inventory must stay empty.
`adapter_only_public_surfaces_match_allowlist` and
`gpui_adapter_exports_group_runtime_specific_surfaces` guard the intentionally public GPUI helper
surface: `TextInputController`, externally supplied `ScrollHandle`, `focus_ring_shadow`,
`focus_ring_shadow_with_theme`, `GpuiOverlayState`, the adapter accessibility/geometry conversions,
and related adapter scheduling helpers must stay classified under
`open_gpui_ui_components::gpui_adapter` instead of drifting into the crate root, prelude default
interface, or resolved state. `FocusRing` itself uses neutral `UiPx`; only the GPUI focus-ring
shadow helpers return `BoxShadow`, and production render paths should use the explicit-theme helper.

When changing GPUI accessibility repair or component metadata that creates explicit cross-node
relationships, also run:

```sh
cargo check -p open-gpui
cargo nextest run -p open-gpui --lib window::a11y::tests::repair_tree_update
```

Manual UI foundation dogfood should use the dedicated gallery after the automated checks pass:

```sh
cargo run -p open-gpui-ui-foundation-gallery
cargo run -p open-gpui-ui-foundation-gallery -- --page components
```

1. Open `Tokens` and confirm the semantic token registry shows surface, text, accent, focus ring,
   destructive, overlay, and modal overlay keys without introducing a styled component layer.
2. Open `Sizing & Density`, switch between compact and desktop from the summary panel, and confirm
   the highlighted density and default size change with the foundation policies.
3. Open `Adaptive`, use the same compact/desktop switch, and confirm device samples show mobile /
   desktop shell mode, compact / regular / expanded class, and panel samples show compact / medium /
   wide classes.
4. Open `Focus & A11y`, tab through the focusable controls, confirm the focus-visible outline is
   visible, click the counter and reset controls, and toggle the switch. The visible counter and
   switch state should match the accessible role/state vocabulary shown by the page.
5. Open `Overlay`, click `open overlay`, confirm the anchored popover appears from the trigger, then
   close it from the popover or press Escape. The geometry readout should keep anchor, layout,
   visual, preferred, and safe-window rectangles visible. The behavior contract matrix should show
   distinct tooltip, popover, dialog, and menu policies for presence, outside press, Escape, focus,
   underlay blocking, and GPUI adapter fields such as deferred priority and snap margin. In the
   Tooltip samples, hover `Hover or focus`, tab to `Focus only`, and confirm each reveals
   descriptive tooltip content while `Disabled` remains unfocusable and closed; `Manual delayed`
   should stay visible and report its custom delay policy. In the HoverCard samples, confirm
   `Profile preview` reports its default-open interactive contract without visually blocking the
   page at load, `Focus preview` opens only from keyboard focus, and `Manual card` opens and closes
   from its gallery control with pass-through or consume outside-press metadata shown in the state
   row. In the Popover samples, confirm `Default open` reports the default-open contract
  without visually blocking the page at load, `Controlled` opens and closes from its gallery
  control, Escape closes the controlled popover, outside press closes the visible popovers, and the
  `Consume outside` sample reports a consuming outside-press policy while `Disabled` remains
  closed. In the Dialog samples, open and close `Controlled modal`, confirm Escape and the modal
  barrier can close it without activating underlay controls, confirm `Default open` reports a
  blocking modal layer without visually blocking the page at load, confirm `Outside ignored`
  reports the sticky outside policy, and confirm `Disabled` stays closed. In the AlertDialog
  samples, open `Delete project`, confirm the destructive action is explicit, cancel receives the
  default focus, outside press is consumed without dismissing, Escape closes it, and focus returns
  to the trigger; confirm the safe cancel sample reports its default-open and modal-underlay
  contract without visually blocking the page at load. In the Sheet samples, confirm the left modal
  sheet reports blocking underlay input, the right non-modal sheet opens from its gallery control
  and reports pass-through outside behavior without a blocking modal barrier, and the bottom sticky
  sheet reports bottom-edge attachment, hidden close affordance, and ignored outside press. In the
  Menu samples, confirm arrow keys move roving focus over enabled
   action items while skipping separators and disabled items, Enter/Space activates the focused
   action and closes the menu, Escape closes the controlled menu, and `Outside ignored` keeps its
   explicit outside policy. In the ContextMenu samples, right-click the hotspot and confirm the
   menu opens from the pointer point, snaps inside the window near edges, and closes on outside
   press or Escape.
6. Open `Components`, or start there directly with
   `cargo run -p open-gpui-ui-foundation-gallery -- --page components`, and confirm Button, Badge,
   Accordion, Collapsible, Slider, NumberInput, ToggleGroup, Link, Breadcrumb, Tag, ToastStack,
   IconButton, Separator, Kbd, Progress, Skeleton, Avatar, ScrollArea, Splitter, Switch, Checkbox,
   RadioGroup, Toggle, Label, TextInput, Textarea, Field, Tabs, Toolbar, Sidebar, Listbox, Select,
   Combobox, Command, Table, and VirtualizedList samples render with enabled, disabled, selected, checked, unchecked,
   indeterminate, pressed, invalid, required, read-only, placeholder, value, help, error,
   control-association, decorative, semantic, indeterminate-progress, fallback-initial,
   source-metadata, roving-focus, popup, overflow-axis, scroll-reset, resize-constraint, row-model,
   and virtualized-viewport states. The Badge, Kbd, Skeleton, and non-removable Tag samples should
   remain display-only.
   The Accordion and Collapsible samples should expose stable disclosure values, disabled rows, and
   open-state readouts. Slider and NumberInput samples should expose clamped min/max/step metadata,
   disabled/read-only or invalid states, and keyboard or step payload semantics. ToggleGroup should
   expose single and multiple stable-value selection with disabled-item skipping. Link and
   Breadcrumb should expose accessible navigation labels and activation metadata. Tag should expose
   removable and disabled-remove metadata. ToastStack should expose visible stack ordering,
   overflow, timeout pruning, dismiss reasons, and action metadata without owning timers.
   Use a few catalog cards, such as Table, Tree, and VirtualizedList, to enter focused
   component-family mode; confirm unrelated samples are hidden, the section directory stays
   available, nested sample scrolling still stays inside the sample, and `All components` restores
   the full conformance page with the page scroll reset. The Separator samples should distinguish semantic and
   decorative roles. The Progress samples should cover determinate and indeterminate values, with
   indeterminate progress rendering as a short non-percentage segment rather than a fixed 33% fill.
   The Avatar samples should show derived fallback initials, explicit fallback text, explicit
   accessible labels, and source metadata without owning image loading. The IconButton samples
   should be square controls with visible focus and explicit accessible labels. The ScrollArea samples should cover vertical overflow, horizontal overflow,
   and two-axis overflow; wheel or trackpad scrolling should stay inside each constrained viewport
   while the state readout reports the expected axis and reset policy. Scroll each constrained
   ScrollArea once, then continue scrolling the same viewport after the content has moved; it should
   keep moving instead of snapping back to the origin after the redraw caused by the first scroll.
   The gallery navigation rail should also scroll independently inside its own viewport so deep
   sections remain reachable on compact windows. The vertical Tabs sample should keep its tab rail
   scrollable inside the constrained gallery card, and the focused component smoke now verifies the
   shared `ScrollArea` viewport directly through `tabs_vertical_tablist_scrolls_when_constrained`.
   The Splitter samples should
   show horizontal and vertical panel groups, stable handle affordances, min/max fraction readouts,
   collapsed-panel metadata, and pointer-drag resizing without changing surrounding layout. Drag the
   vertical collapsed sample far enough to restore the collapsed panel, then confirm subsequent
   dragging resizes it normally. The RadioGroup samples should
   cover vertical required selection and horizontal navigation that skips disabled items. The Toggle
   samples should expose button-like pressed state without behaving like a checkbox. The Tabs
   samples should cover horizontal automatic activation and vertical manual activation; use arrow
   keys, Home/End, Enter, and Space to confirm focus movement and activation behavior. The vertical
    sample should keep its tab rail scrollable inside the constrained gallery card. The Toolbar
    samples should expose horizontal and vertical command groups; use arrow keys plus Home/End to
    confirm roving focus skips disabled items and separators, and use Enter/Space to activate
    action/toggle items. The component runtime smoke now verifies the rendered Toolbar keyboard path
    for disabled-item/separator skipping and activation payloads. The Sidebar samples should expose
   expanded, icon-collapsed, and long scrollable navigation; icon collapse should hide visible labels
   while keeping item labels
   explicit, disabled items should be skipped, and the long sidebar should scroll inside its sample
   frame. The component smoke now verifies the shared `ScrollArea` viewport directly through
   `sidebar_long_navigation_scrolls_inside_shared_scroll_area`, and the gallery smoke verifies the
   long sidebar's internal viewport moves relative to its sample card. The Listbox samples should
   expose
   grouped options, disabled option skipping, selected and active descendant metadata, empty-state
   behavior, and keyboard navigation/activation with Up/Down/Home/End plus Enter/Space. The
   component runtime smoke now verifies rendered Listbox disabled clicks, selection-free arrow
   navigation, disabled/separator skipping, and option/listbox callback parity for keyboard
   activation. The Select
   samples should expose closed, controlled-open, and disabled states; confirm the trigger label
   reflects the selected option, the open sample uses a non-modal dismissible listbox popup with a
   scrollable long option set, Escape/outside press dismisses it, disabled empty select remains
   closed, and the state readout keeps trigger-selected value distinct from popup listbox selection.
   The component runtime smoke now verifies rendered Select trigger opening, disabled popup
   option rejection, click selection, keyboard popup selection that skips disabled rows, selection
   payloads, and ordered popup close callbacks. The Combobox samples should expose editable
   filtering, selected value metadata that does not disappear when the query hides the selected
   option, an empty filtered state, disabled input/popup suppression, and visible query/typeahead
   metadata. The component runtime
   smoke now verifies real Combobox text-input editing, filtered popup options, filtered option
   click selection, and close callbacks. The Command samples should expose ranked search results,
   selected chips for multi-select, stable selected values independent of result order, a 10k-item
   virtualized command result window, app-owned
   indexed/loading metadata, shortcut labels, inline and dialog-backed presentation, and modal
   dialog outside/Escape dismissal while preserving the Components page scrollability. The component
   runtime smoke now verifies real Command text-input editing, inline filtering, keyboard
   activation, shortcut payloads, non-dialog content persistence, multi-select toggling, virtualized
   scrolling/reveal behavior, and app-owned index snapshot state. The default TextInput
   sample should accept real text editing through the controller-backed path, and the password
   sample should show masked display metadata while preserving the underlying value contract. The
   Textarea samples should expose placeholder, filled, overflowing, and invalid states; wheel input
   inside the overflowing textarea should scroll its multiline content without moving the sample
   card or outer Components page. The
   gallery remains scrollable and keeps focus visible when the page overflows. The Table samples
   should expose the `release-queue` 10k-row virtualized window,
    the filtered/sorted/paginated `filter-board` model with working status `TableFacetedFilter`,
    score `TableRangeFilter`, and name `TablePredicateFilter` controls,
    the controlled `release-resize` sizing
    sample, the grouped and sticky pinned `release-rollup` model with left/right fixed lanes and a
    horizontally scrollable center lane, the wide `release-matrix` center-column window with a
    working `TableColumnVisibility` control, the source-tree `dependency-tree` sample with nested
    rows and controlled expansion, stable selected row ids, the editable `editable-release`
    text-cell sample with app-owned row updates, the `multiline-release` textarea-cell sample with
    newline-preserving app-owned row updates, table/row/cell accessibility metadata, sortable header
    metadata, resize handle metadata, row activation, expansion, column-visibility, and cell-edit
    log entries, and internal body viewports that scroll
    without moving the outer Components page.
    The Tree sample should expose `document-outline`,
    tree/tree-item accessibility metadata, expandable `Paper` children, a state readout, an inner
    viewport that scrolls without moving the outer Components page, and selection/toggle events
    through the gallery sample runtime log. The VirtualizedList sample should expose the
    `release-navigation` 10k-item window, listbox/listbox-option roles, active/selected
    metadata, visible/overscan readouts, an internal viewport that scrolls without moving the
    outer Components page, card-chrome wheel containment, and PageDown plus Enter/Space activation
    through the gallery sample runtime log. The app should stay open after opening `Components`;
    an `accesskit_consumer`
   panic during that navigation is a
   regression in the accessibility repair gate. The Components page also serves as a conformance
   surface: confirm the visible component catalog distinguishes official components from
    adapter-only helpers and internal anatomy, and confirms Separator, Kbd, Progress, Skeleton, and
    Avatar are official entries with state types, then confirm the visible gate cards for explicit
    crate exports, gallery metadata, ScrollArea redraw persistence, Splitter runtime constraints,
    Tabs overflow, `table-virtualization`, `tree-renderer`, `virtualized-list-renderer`, and
    explicit accessible metadata on icon-only and label-association samples.
   The Overlay Menu and ContextMenu samples should expose action, checkbox, radio, separator,
   disabled, submenu, typeahead, controlled-open, outside-policy, and point-anchor variants. Use
   `cargo nextest run -p open-gpui-ui-components menu` and `cargo nextest run -p
   open-gpui-ui-components context_menu` to verify rich item payloads, pure typeahead,
   visible-submenu keyboard navigation, submenu hover delay / close timing, local menu scrollability,
   context-menu reuse, and long-menu wheel containment through
   `context_menu_runtime_long_menu_scroll_stays_inside_surface`. Use `cargo nextest run -p
   open-gpui-ui-components
   menu_runtime_hover_opens_submenu_and_preserves_child_focus
   menu_runtime_hover_switches_between_submenu_branches` together with the menu family command to
   cover the hover-delay runtime. Use
   `cargo nextest run -p open-gpui-ui-foundation-gallery
   overlay_page_menu_samples_expose_roving_focus_and_dismiss_contracts
   overlay_page_context_menu_samples_expose_point_anchor_contracts
   overlay_page_catalog_entries_have_signals_and_sample_selectors
   overlay_gallery_smoke_closes_menu_from_escape_and_outside_press
   overlay_gallery_smoke_opens_menu_submenu_from_hover
   overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses` plus `cargo check -p
   open-gpui-ui-foundation-gallery --tests` after changing the overlay menu family.
7. Re-run `cargo nextest run -p open-gpui-ui-components` and `cargo nextest run -p
   open-gpui-ui-foundation-gallery` if a manual check exposes a component or gallery regression.

For UI component productization checkpoint work, additionally review
`docs/adr/0008-open-gpui-ui-component-productization-roadmap.md` after the automated component
tests pass. If a future task explicitly reopens extraction, also review
`docs/adr/0006-open-gpui-ui-headless-extraction-checkpoint.md` and
`docs/adr/0007-open-gpui-ui-headless-boundary-design.md`. The checkpoint should continue to
identify which behavior is neutral, which behavior remains GPUI adapter-owned, and why the current
crates remain the active product boundary.

CI runs a three-platform matrix for pushes to `master` / `main`, pull requests, and manual workflow
dispatches:

- Windows runs the same local gate, `cargo nextest run -p xtask`,
  `cargo nextest run -p open-gpui-docking-native --no-fail-fast`, and
  `cargo check -p open-gpui-windows --all-features --locked`.
- Linux runs `cargo check -p open-gpui-linux --all-features --locked` after installing the system
  headers needed for Wayland, X11, fontconfig, freetype, and pkg-config.
- macOS runs `cargo check -p open-gpui-macos --features font-kit --locked`.
- All three platforms run `cargo check -p open-gpui-wgpu --features font-kit --locked`.

Run the native renderer smoke explicitly with:

```sh
cargo run -p xtask -- renderer-smoke
```

That command runs the focused `open-gpui-wgpu` smoke test that requests a real native `wgpu` adapter and
device, creates the renderer bind group layouts, and builds the core render pipelines. It is not
part of the default `verify` gate because it depends on local GPU, driver, and session availability.

Run the docking smoke surface explicitly after changing `open-gpui-docking`:

```sh
cargo fmt --all -- --check
cargo check --tests -p open-gpui-docking
cargo nextest run -p open-gpui-docking
cargo nextest run -p open-gpui-docking-native --no-fail-fast
cargo check -p open-gpui-docking-native
cargo run -p open-gpui-docking-native
```

For docking presentation/preview/motion work, the focused semantic gates are:

```sh
cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests --no-fail-fast
cargo nextest run -p open-gpui-docking-native runtime_status_panel_formats_platform_capabilities --no-fail-fast
```

The docking native example exercises the public multi-window setup: applications build one
`DockController`, wrap it in a `DockViewportRuntimeHandle`, register window-close cleanup, and open
controller-backed primary and secondary `DockHost` viewports. The runtime panel reports both the
last route target and the route selection source, so dogfood runs can distinguish trusted hovered
window routes, window-stack fallback routes, focus-stamp fallback routes, and current-facts
rejections. It also reports the current platform viewport capability snapshot, splitting route facts
from placement facts so platform-boundary regressions are visible during native dogfood. The
placement restore line reports matched and missing restored windows, and the tear-off status line
reports whether a viewport opened from suggested bounds or drag-source geometry, so placement
authority regressions are visible in the same panel.

Docking target previews are scene-owned. During dogfood, every target-window preview should be
explainable from the same capability model: `DockPresentationScene` resolves panes, tab bars,
tab labels, splitters, floating containers, focus regions, and overlay anchors; `DockPreviewScene`
describes allowed/rejected target facts; `DockVisualAffordanceScene` gives stable layer identity for
preview bodies, guide boxes, tab insertion, payload tabs, route markers, and rejected state;
`DockTransitionPlan` describes motion/reduced-motion semantics; divider hit maps, zoom/focus state,
accessibility descriptors, and runtime diagnostics consume the same descriptor path. Rendering must
not recreate guide availability independently from those descriptors. Debug selectors reflect that
contract:
target-stack guides use `dock:<space>:drop-guide:inner:<tabs>:<zone>`, root/host guides use
`dock:<space>:drop-guide:outer:<zone>`, the split body is exposed separately from the full target
preview container as `dock:<space>:drop-preview:body`, and center/tab docking exposes a
`dock:<space>:drop-preview:tab-insertion` affordance before payload tab previews.

Manual native docking dogfood should use the same example after the automated checks pass:

1. Launch `cargo run -p open-gpui-docking-native` and confirm the app opens `Docking demo`,
   `Docking preview`, and `Empty central dogfood` windows.
2. Drag a primary-class tab from `Docking demo` into another primary-compatible target; the preview
   must appear in the destination window and release must select the moved item there.
3. Drag the `Preview` / `Diff` secondary-class stack from `Docking preview` back into `Docking demo`;
   item order and the active tab must be preserved.
4. Drag `Preview` / `Diff` over `Empty central dogfood`; the route must render as rejected and
   release must not mutate the graph because the central space only accepts central-class panels.
5. Use `Restore central note` from the runtime status panel; the `Central note` panel must reopen in
   the empty central window and recover the central-region identity instead of becoming ordinary
   root-only content.
6. Drag a tab or stack outside every docking window; a new runtime-backed viewport must open before
   the graph moves the source payload.
7. Dock the torn-off viewport content back into an existing window; the destination window must
   activate and the moved item must become the selected tab.
8. Move runtime-opened windows across displays, choose `Save placement`, then use `Reopen closed
   demo viewports`; restored placement should use saved bounds only as placement input while live
   drag routing continues to use current viewport bounds. On macOS, windows on a secondary display
   should keep non-overlapping desktop-space bounds while routing between viewports.
9. Exercise the runtime panel close-policy controls for prevent, retain, and merge-back behavior;
   closing a viewport must match the selected policy without losing descriptor-backed panel restore
   or leaving a stale cross-window route preview in another viewport.
10. Start a cross-window drag, hover a valid target, then move to an area of the same viewport with
   no current dock target before releasing; the previous preview must not commit from stale target
   state.
11. Drag over the empty central dogfood window; empty central-space preview, rejection, and
   passthrough behavior must match the visible policy state.
12. While dragging over a valid tabs target, confirm the guide affordance is target-owned and
    box-shaped rather than a floating five-button cluster. Center hover should show a center box,
    side hover should highlight only the corresponding side box, and inactive boxes should remain
    visibly weaker than the active one.
13. Hover the center of a compatible target and confirm the destination window renders one dock
    preview plus one contained payload tab preview. Hover any split edge and confirm the preview
    becomes an edge band and the payload tab preview disappears.
14. Reproduce a nested-target case by docking into a child region, then dragging another tab into
    the remaining nested leaf. Confirm hovering the left or right side of that nested leaf resolves
    inside the nested leaf itself rather than snapping to the neighboring region or to the root
    edge.
15. Drag a tab or stack outside every valid host. Confirm the route marker reads as tear-off or
    rejection only; no fake blue dock target or payload tooltip should appear at the source.
16. Drag the two-tab `Preview` / `Diff` stack over a compatible target stack center. The
    destination preview must show a shared preview body plus two selected-tab-like payload tab
    previews in payload order; the previews should clip to the target tab bar instead of becoming a
    single dark rectangle.
17. Repeat the same two-tab stack drag across windows. The target window must render the same
    payload tab preview structure as a local hover, while the source window shows only route-marker
    feedback when applicable.
18. Hover a side drop box for the root central leaf and a side drop box for a nested child leaf.
    The root central leaf should use outer split semantics; the nested child leaf should keep
    inner split semantics.
19. Hover rejected center and route targets. The target preview must use rejected tokens, suppress
    payload tab previews, and leave the graph unchanged on release.
20. Start a routed cross-window hover, then hover a different compatible viewport before release.
    The old target window must lose both its target preview and payload tab previews, while the new
    target owns the current preview.
21. Press Escape during a routed drag after a target preview appears. The source route marker,
    target preview, runtime drag session, and GPUI active drag must all clear in one frame.

Current docking multi-viewport capability states:

- Coordinate facts are explicit runtime state. `DockViewportCoordinateStatusRecord` reports whether
  each registered viewport is using shared global-screen bounds or receiver-local window bounds, and
  the runtime panel exposes that generation next to the route selection source. Mixed-DPI and
  display-ambiguous backends should fail closed or degrade to local-only routing until the platform
  backend can publish stronger facts. Automated owners: `host_viewport_route_tests` and
  `viewport_lifecycle_record_reports_window_local_coordinate_status`.
- Viewport flags are capability-gated platform sync requests. No-input can be applied when a
  backend advertises native pointer-input routing; no-focus-on-appearing, no-focus-on-click, alpha,
  topmost, and no-taskbar use `PlatformViewportFlagCapabilities` and are recorded as unsupported
  requests until a backend exposes real live mutation support. The native runtime panel reports
  both capability snapshots and the latest applied/skipped/unsupported sync counts. Automated owners:
  `host_viewport_platform_capability_tests`,
  `viewport_runtime_syncs_supported_options_when_reusing_window`, and
  `empty_central_passthrough_syncs_window_pointer_input`.
- Preview proof is semantic rather than pixel-perfect. `DockPreviewVisualDescriptor` records the
  allowed/rejected decision, active layer, active zone, tab insertion descriptor, payload tab
  previews, and route-preview marker shape, while debug selectors continue to anchor rendered
  dogfood checks. Presentation, overlay, transition, zoom/focus, divider hit map, and accessibility
  descriptors are covered by focused tests. The native runtime panel exposes preview capability as
  `preview proof: presentation-scene+real-content-reveal+overlay-motion+tab-insertion+retargeting+splitter-motion+zoom-focus+divider-hit-map+corner-drag+a11y+route-cleanup+reduced-motion`
  and motion runtime capability as
  `motion proof: shared-runtime+run-state+scalar-value+scalar-sample+explicit-models+policy-gates+layout-projection+projection-clips+sampled-progress+retargeted-identity+reduced-motion-final-state+high-frequency-bypass`.
  The transition executor currently productizes sampled pane, divider, visual-affordance, zoom, and
  focus motion on top of explicit timeline or spring scalar models.
  Overlay-scene-to-transition conversion for tab insertion, payload ghosts, route markers, and
  rejected state is descriptor proof, not an every-frame drag-preview animation guarantee.
  Automated owners: `host_presentation_scene_tests`, `host_viewport_preview_visual_tests`,
  `host_transition_tests`, `host_zoom_focus_tests`, `host_divider_hit_map_tests`, and
  `host_accessibility_tests`. Transparent payload-window rendering, platform accessibility mapping,
  and screenshot or pixel-regression baselines remain explicitly deferred follow-up work.
- Routed overlay cleanup is fail-closed. Source-window route markers and target-window previews are
  separately renderable, but releases revalidate against current viewport facts instead of trusting
  cached preview state. Starting a new routed drag clears the previous session's routed preview,
  replacing a route target removes stale previews from the old target window, and Escape clears the
  GPUI active drag plus all routed preview state. Automated owners:
  `host_viewport_preview_tests`, `host_transition_tests`, and `host_render_tests`.
- Test ownership is split by concern. Route, lifecycle, placement, close, preview, platform
  capability, and visual-proof assertions live in focused `host_viewport_*_tests` modules; the old
  monolithic runtime test files have been deleted. Rendered native dogfood tests remain
  end-to-end integration coverage.
- `DockViewportRuntimeHandle` remains the application-facing facade. Platform sync and pointer-input
  requests now live behind `viewport_platform_sync`, window effects live behind
  `viewport_runtime_effects`, route/scene/close handle methods are split into
  `viewport_runtime_handle::{route_ops,scene_ops,close_ops}`, and coordinate facts live with the
  viewport registry/status model. New tests should target those owning modules instead of adding
  crate-private pass-throughs to the handle.

Before publishing a crate, confirm that the packaged archive carries the expected attribution files:

```sh
cargo package -p open-gpui --list --allow-dirty
```

For the canvas crate specifically, run:

```sh
cargo package -p open-gpui-canvas --list --allow-dirty
cargo publish -p open-gpui-canvas --dry-run --allow-dirty
```

Every published Open GPUI crate should include `README.md`, `LICENSE-APACHE`, and `NOTICE`. Cargo
does not package files outside a crate root through `include`, so each publishable crate root keeps
its own `NOTICE` copy.

The import-boundary scan rejects dependency files that reintroduce Zed's GPL tracing stack
(`ztracing`, `ztracing_macro`, `zlog`), the old `zed-sum-tree` dependency, the Zed monorepo as a
Cargo git dependency, retired Zed Git fork sources that have already been migrated, or the removed
Zed `perf` crate dependency. The retired `zed-scap` package and `zed-industries/scap` Git source
are also rejected now that screen capture resolves through the Open GPUI-owned
`open-gpui-scap` fork. The old crates.io `zed-font-kit` package is retired and should not be
reintroduced; font-kit resolves through the Open GPUI-owned fork configured in the crate manifests.
