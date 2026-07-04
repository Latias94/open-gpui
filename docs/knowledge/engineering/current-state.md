---
type: Current State
title: Open GPUI UI productization state
status: active
timestamp: 2026-07-04T08:39:45+08:00
git_branch: main
related_plan:
  - docs/plans/2026-07-01-001-refactor-ui-contract-test-modules-plan.md
  - docs/plans/2026-07-01-002-refactor-ui-public-gallery-boundaries-plan.md
  - docs/plans/2026-07-01-003-refactor-ui-component-contract-registry-plan.md
  - docs/plans/2026-07-01-004-refactor-ui-family-boundaries-plan.md
  - docs/plans/2026-07-01-005-refactor-ui-contract-a11y-theme-plan.md
  - docs/plans/2026-07-02-001-refactor-ui-contract-tooling-plan.md
  - docs/plans/2026-07-02-002-refactor-native-ui-hybrid-registry-architecture-plan.md
  - docs/plans/2026-07-02-002-refactor-docking-flat-motion-runtime-plan.md
  - docs/plans/2026-07-02-003-refactor-ui-motion-runtime-foundation-plan.md
  - docs/plans/2026-07-02-003-refactor-ui-framework-deep-modules-plan.md
  - docs/plans/2026-07-02-004-refactor-docking-render-authority-convergence-plan.md
  - docs/plans/2026-07-03-001-refactor-docking-visual-affordance-runtime-plan.md
  - docs/plans/2026-07-03-002-refactor-docking-affordance-authority-cleanup-plan.md
  - docs/plans/2026-07-03-003-feat-command-center-runtime-plan.md
related_research:
  - native-ui-framework-design-research/report.md
related_adr:
  - docs/adr/0006-open-gpui-ui-headless-extraction-checkpoint.md
  - docs/adr/0008-open-gpui-ui-component-productization-roadmap.md
  - docs/adr/0010-docking-presentation-scene-motion-model.md
  - docs/adr/0011-docking-split-motion-primitive-boundary.md
  - docs/adr/0012-docking-runtime-capability-alignment.md
  - docs/adr/0013-open-gpui-native-ui-hybrid-registry.md
  - docs/adr/0014-remove-native-ui-hybrid-registry.md
  - docs/adr/0015-ui-motion-runtime-foundation.md
related_decision:
  - docs/knowledge/engineering/decisions/open-gpui-native-ui-framework-distribution-strategy.md
verified_by:
  - cargo check -p open-gpui-ui-components --test table
  - cargo nextest run -p open-gpui-ui-components table --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components --test table --no-fail-fast
  - cargo check -p open-gpui-ui-foundation-gallery --tests
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_catalog_metadata_is_separate_from_rendering components_catalog_consumes_component_contract_rows components_page_conformance_gates_reference_core_and_gallery_contracts --no-fail-fast
  - cargo nextest run -p open-gpui-ui-foundation-gallery table --no-fail-fast
  - cargo nextest run -p open-gpui-ui-foundation-gallery tree virtualized_list --no-fail-fast
  - cargo nextest run -p open-gpui-ui-foundation-gallery --test foundation_gallery --no-fail-fast
  - cargo run -p xtask -- verify
  - cargo test -p xtask commands
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_catalog_consumes_component_contract_rows official_component_catalog_entries_have_signals_and_sample_selectors gallery_catalog_entries_satisfy_component_contract_evidence gallery_story_contracts_reference_component_contract_rows gallery_story_contracts_cover_components_state_readouts_and_overlays --no-fail-fast
  - cargo test -p xtask
  - cargo run -p xtask -- scan-ui-contract
  - cargo run -p xtask -- scan-theme-schema
  - cargo fmt --all --check
  - cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
  - cargo check -p open-gpui-ui-components --tests
  - cargo check -p open-gpui-ui-foundation-gallery --tests
  - cargo nextest run -p open-gpui-ui-components public_surface --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components menu --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components context_menu --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components tree --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components table --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components --no-fail-fast
  - cargo nextest run -p open-gpui-ui-foundation-gallery --no-fail-fast
  - cargo nextest run -p open-gpui-docking transition_plan_from_overlay_scene_uses_current_bounds_for_matching_layers transition_plan_keeps_preview_layers_at_current_target_bounds overlay_replacement_keeps_preview_layers_at_current_target_bounds --no-fail-fast
  - cargo nextest run -p open-gpui-docking transition_executor_samples_timeline_and_reveal_geometry transition_executor_replaces_active_execution_and_completes_reduced_motion_immediately transition_sample_overlay_renders_from_executor source_hover_over_known_viewport_renders_target_drop_preview routed_preview_replacement_clears_old_target_overlay_without_stale_payload --no-fail-fast
  - cargo check -p open-gpui-docking
  - cargo check -p open-gpui-docking-native
  - cargo nextest run -p open-gpui-docking host_viewport_preview_visual_tests host_presentation_scene_tests host_divider_hit_map_tests --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_transition_tests host_render_tests host_viewport_preview_visual_tests --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_accessibility_tests host_divider_hit_map_tests host_debug --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_viewport_preview_visual_tests host_transition_tests host_render_tests --no-fail-fast
  - cargo check -p open-gpui-docking-native --tests
  - cargo check -p open-gpui-ui-core --tests
  - cargo check -p open-gpui-ui-components --tests
  - cargo check -p open-gpui-ui-foundation-gallery --tests
  - cargo nextest run -p open-gpui-ui-core overlay grid_viewport command --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components theme a11y menu context_menu command --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components command_descriptors --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
  - cargo nextest run -p open-gpui-ui-foundation-gallery component --no-fail-fast
  - cargo nextest run -p open-gpui-command --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components choice --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components command::runtime::tests --no-fail-fast
  - cargo fmt -p open-gpui-command -p open-gpui-ui-components
  - cargo nextest run -p open-gpui-command center_exposes_query_history_navigation memory_history_promotes_duplicate_queries memory_history_navigates_recent_queries_with_prefix --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components command_palette_controller_navigates_query_history_with_prefix --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components command_palette_controller --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
  - cargo nextest run -p open-gpui-command center_projects_command_key_bindings_into_gpui_keymap center_reports_command_key_binding_projection_diagnostics --no-fail-fast
  - cargo nextest run -p open-gpui-command center_reports_command_key_binding_conflicts_and_install_report center_reports_global_key_binding_context_conflicts --no-fail-fast
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_page_choice_samples_expose_listbox_and_select_contracts --no-fail-fast
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts --no-fail-fast
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components command_palette_projection_builds_status_items_from_provider_failures_and_diagnostics command_state_accepts_explicit_status_items --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata components_page_search_samples_expose_combobox_and_command_contracts component_gallery_shell_reads_choice_active_metadata_from_resolved_state --no-fail-fast
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
  - cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
  - cargo check -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --tests
  - cargo nextest run -p open-gpui-ui-components command --no-fail-fast
  - cargo nextest run -p open-gpui-ui-foundation-gallery command --no-fail-fast
  - cargo run -p xtask -- scan-ui-contract
  - cargo fmt -p open-gpui-command -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
  - python C:\Users\Frankorz\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
  - cargo run -p xtask -- scan-theme-drift
  - cargo run -p xtask -- scan-theme-schema
  - cargo run -p xtask -- scan-ui-contract
  - no production matches from rg -n "ThemeResolver::resolve\(" crates/ui_components/src -g "*.rs"
  - only focus.rs compatibility matches from rg -n "focus_ring_shadow\(|ThemeContext::light\(\)" crates/ui_components/src -g "*.rs"
  - git diff --check
  - cargo nextest run -p open-gpui-docking host_render_tests host_presentation_scene_tests host_interaction_tests --no-fail-fast
  - cargo nextest run -p open-gpui-docking render_tab_bar_bounds_match_presentation_scene_tab_bar render_floating_bounds_match_presentation_scene_container render_tiny_floating_handle_clamps_to_presentation_title_bar render_measured_tab_label_fact_overrides_scene_equal_slot_estimate runtime_nested_tab_tear_off_uses_leaf_size_not_tab_label --no-fail-fast
  - cargo nextest run -p open-gpui-docking render_measured_tab_label_fact_overrides_scene_equal_slot_estimate rendered_host_scene_frame_seeds_deterministic_facts_from_presentation_scene --no-fail-fast
  - python native-ui-framework-design-research/generate_report.py
  - python -m py_compile native-ui-framework-design-research/generate_report.py
  - python C:\Users\Frankorz\.codex\skills\research\validate_json.py -f native-ui-framework-design-research\fields.yaml -d native-ui-framework-design-research\results
  - python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering
---

# Current State

- Branch: `main`; this state includes the merged docking render authority convergence and UI
  framework deep-module refactor work.
- Done on `refactor/ui-framework-deepening`: component render paths now resolve color intents from
  `ThemeResolver::current(cx)` / `ThemeContext` or an explicit snapshot. Direct
  `ThemeResolver::resolve(...)` is documented as default-light compatibility only, and `rg -n
  "ThemeResolver::resolve\(" crates/ui_components/src -g "*.rs"` has no production hits.
  Focus-ring painting follows the same runtime theme rule through
  `focus_ring_shadow_with_theme`; the default-light `focus_ring_shadow` compatibility helper is
  fenced to `focus.rs` by public-surface tests.
- Done on `refactor/ui-framework-deepening`: `open_gpui_ui_core::overlay::resolve_overlay_placement`
  is the shared anchored-placement solver for explicit neutral placement inputs, while
  `open_gpui_ui_components::overlay` owns GPUI host mapping through `GpuiOverlayPlacement` and
  relative/positioned/full-window layer helpers. Trigger-anchored components still rely on GPUI for
  final live measured placement until a measured overlay runtime exists.
- Done on `refactor/ui-framework-deepening`: `open_gpui_ui_core::grid_viewport::RowWindow` and
  `RowWindowItem` are the shared renderer-neutral row-window projection for Table,
  VirtualizedList, and Tree; component-specific selection, hierarchy, activation, and pinned-row
  contracts stay local.
- Done on `refactor/ui-framework-deepening`: gallery selector/readout/focus traversal now derives
  from `StoryContract` through `component_story_contract_for(name)` and
  `component_story_contracts_for_focus(mode)`.
- Done on `refactor/open-gpui-command-crate`: `open_gpui_command` is the command ecosystem owner.
  It owns `CommandDescriptor`, deterministic registries, scoped registration, availability
  projection, neutral menu trees, memory history, and GPUI command-id dispatch adapters.
  `open_gpui_ui_core::command` was deleted; Command, Menu, ContextMenu, and gallery samples now
  consume command metadata from `open_gpui_command`.
- Done on `feat/command-center-runtime`: `open_gpui_command::CommandCenter` is the recommended
  app-owned runtime facade over scoped source registration, source/scope unregistration,
  availability, GPUI action mapping, shortcut projection, fuzzy search, menu projection,
  dispatch, and bounded usage/query history. The command UI now carries disabled reasons through
  descriptors, resolved item state, behavior snapshots, and row aria labels; command runtime
  navigation handles Vim-style control aliases, PageUp/PageDown disabled landings, and
  `prefer_character_input` IME/character input guards. The gallery `registry-dispatch` sample now
  uses `CommandCenter` instead of manually joining `CommandRegistry` and `GpuiCommandActionMap`.
- Done on `feat/command-provider-runtime`: `open_gpui_command` now has runtime-neutral dynamic
  provider primitives (`CommandProviderRequest`, `CommandProviderResponse`,
  `CommandProviderSource`, provider status/state) and `CommandCenter` can register provider
  callbacks, refresh providers by query, apply externally produced async responses, atomically
  replace provider-owned dynamic sources, and unregister provider sources without affecting static
  command registrations.
- Done on `feat/command-provider-gallery`: the foundation gallery now has a `provider-search`
  command sample appended after `registry-dispatch`. It registers a query-dependent
  `CommandCenter` provider, refreshes provider results for `alpha`, records provider status, and
  renders the dynamic provider source through a `CommandIndexSnapshot` without moving provider
  ownership into the UI component state model.
- Done on `feat/command-provider-lifecycle`: provider requests can now carry center-issued
  `CommandProviderRequestId` values, provider responses can bind to those requests, and
  `CommandCenter` reports `CommandProviderApplyOutcome::Stale` without mutating the registry when
  an old async response arrives after a newer query. Request ids are not reused across
  unregister/re-register cycles. Provider status now carries the producing request id and query for
  gallery/readout contracts.
- Done on `feat/command-provider-refresh-controller`: `CommandProviderRefreshController` is the
  reusable provider-backed command palette query pipeline. It owns query-change detection, optional
  loading status, registered-provider refresh, async response application, stale-response handling,
  provider status capture, and `CommandRegistrySnapshot` projection. The gallery `provider-search`
  sample now uses this controller instead of hand-writing begin/apply/search calls.
- Done on `feat/command-refresh-ui-bridge`: `open_gpui_ui_components` now owns the UI bridge from
  `CommandProviderRefreshProjection` to command palette state. `CommandProviderPaletteProjection`
  projects provider snapshots as `PreFiltered`, carries loading provider status into
  `CommandLoadingState`, preserves `CommandProviderStatus` for readouts, and
  `Command::provider_refresh_projection` binds query/snapshot metadata without app-owned
  `CommandIndexSnapshot::from_registry_snapshot(...)` glue.
- Done on `feat/command-app-integration-diagnostics`: `open_gpui_command` now exposes
  `CommandShortcutDiagnostic` and `CommandShortcutDiagnosticKind` plus strict
  `GpuiCommandActionMap` diagnostics for command/action/keymap drift. `CommandCenter` exposes the
  same app-owned check while filtering orphan diagnostics for hidden commands that remain
  registered in active scopes. The gallery `registry-dispatch` sample now proves a healthy empty
  shortcut diagnostic set.
- Done on `feat/command-palette-session`: `open_gpui_ui_components::CommandPaletteProjection` is
  the UI-side app integration projection for command palettes. It turns a `CommandCenter` query plus
  keymap/window shortcut precedence into a `PreFiltered` `CommandIndexSnapshot`, retained provider
  statuses, and shortcut diagnostics. `Command::palette_projection` consumes it directly. The
  gallery `provider-search` sample now binds provider-generated command ids to GPUI actions and
  shortcuts before projection, keeping the provider-backed sample dispatch-ready and
  diagnostic-clean.
- Done on `feat/command-palette-controller`: `open_gpui_ui_components::CommandPaletteController`
  is the UI-side palette query/provider lifecycle controller. It refreshes registered synchronous
  providers on query changes, reports configured providers that need app-owned async work, applies
  external async responses through the existing request-id stale guard, and returns complete
  `CommandPaletteProjection` updates for `Command`. The gallery `provider-search` sample now uses
  this controller instead of directly owning a provider refresh controller.
- Done on `feat/command-context-keymap`: `open_gpui_command::CommandContextStack` now carries
  command scopes and GPUI key contexts together. `CommandCenter` snapshots, menus, diagnostics,
  provider requests, and app-level keymap shortcut projection consume the same stack, while
  focused-window projection remains delegated to GPUI window precedence. The gallery `context-stack`
  command sample proves focused scope descriptor override plus context-aware shortcut projection.
- Done on `feat/command-source-handles`: `open_gpui_command` now exposes
  `CommandSourceHandle` and `CommandProviderHandle` as the recommended explicit lifecycle tokens
  for plugin-like registrations. The older `CommandSourceRegistration` and
  `CommandProviderRegistration` names remain compatibility aliases. Handles can unregister
  themselves against an app-owned `CommandCenter`, while center methods still expose borrowed
  unregister entry points for hosts that keep handles in registries.
- Done on `feat/command-query-history`: `CommandCenter` now exposes app-facing query history
  methods for recording, listing, previous/next navigation, and reset. `MemoryCommandHistory`
  promotes duplicate queries to the newest position. `CommandPaletteController` now wraps history
  navigation for keymap/window projections, captures the current query as the navigation prefix, and
  restores that draft query after moving past the newest matching history entry.
- Done on `feat/command-keybinding-registry`: `open_gpui_command` now has
  `CommandKeyBindingRegistry` and `CommandCenter` keybinding projection APIs. Apps/plugins can
  register command-id keyed shortcut dictionaries, project valid entries into GPUI `KeyBinding`
  values, and receive diagnostics for missing actions or invalid GPUI keystroke/context syntax.
  GPUI remains the chord, mode predicate, and focused-window precedence authority.
- Done on `feat/command-keybinding-conflicts`: command keybinding projections now report
  same-context and global-vs-context shortcut conflicts through `CommandKeyBindingConflict`, and
  app shells can use `CommandKeyBindingInstallReport` from `CommandCenter::install_key_bindings` or
  registry install helpers to inspect append-only GPUI keymap installation count, skipped-entry
  diagnostics, and conflicts together. `CommandKeyBindingProjection::is_clean()` keeps its
  diagnostic-only compatibility meaning; strict validation uses `has_conflicts()` or
  `is_strictly_clean()`. The design deliberately does not claim source-level removal from external
  GPUI keymaps; plugin hosts should rebuild their command-owned keymap layer before reinstalling.
- Done on `feat/command-palette-polish`: command palette projections now adapt provider failures
  and shortcut/action/keymap drift diagnostics into `CommandStatusItem` rows. `CommandState` and
  `Command` expose status item builders plus warning/error counters, the runtime renders status
  rows before result rows, and the gallery `diagnostics-empty` sample proves failed-provider,
  warning-diagnostic, and empty-state rendering in one component surface.
- Done: Public-surface tests now consume the component contract rows instead of gallery/test
  helper maps. The contract table owns official components, state contracts, adapter-only helpers,
  internal anatomy, removed targets, source mappings, docs tokens, gallery status, and default
  export intent.
- Done: `Command`, `Menu`, `ContextMenu`, `Tree`, and Table behavior snapshots now have explicit
  owner modules. The completed family-boundary pass keeps public behavior stable while replacing
  stale single-file source assumptions.
- Done: `component_contract` is split into responsibility modules; focused a11y contracts now cover
  representative component families; the theme JSON schema and loader facade are exported through
  root and prelude.
- Done on `refactor/ui-contract-tooling-audit`: `xtask` is split into command/scanner modules;
  `scan-ui-contract` audits contract rows, default exports, docs tokens, source homes, gallery
  conformance evidence, representative a11y claims, and the committed theme schema artifact.
- Done on `refactor/ui-contract-tooling-audit`: `docs/schemas/open-gpui-theme-v1.schema.json` is a
  reviewable artifact generated from `theme_json_schema()` through
  `open-gpui-ui-components --example export_theme_schema`, with `scan-theme-schema` drift coverage.
- Done: Native UI framework design research is complete in
  `native-ui-framework-design-research/report.md`. The registry part of that research was trialed,
  then removed by ADR 0014 because generated manifest/scaffold artifacts duplicated crate source and
  typed contract facts.
- Done: ADR 0014 supersedes ADR 0013. The hybrid registry manifest API, scaffold recipe metadata,
  JSON/schema artifacts, export examples, `xtask ui_registry`, and `scan-ui-registry` command have
  been removed. `component_contract` typed rows remain as internal verification tables.
- Done: foundation gallery tests consume the typed component contract rows directly for catalog and
  story evidence without moving selector constants into `open-gpui-ui-components`.
- Done: `examples/ui-foundation-gallery/tests/foundation_gallery.rs` is now a helper/module facade.
  Test ownership lives under `tests/foundation_gallery/` split by foundation contracts, overlay
  contracts/smoke, component catalog/sample contracts, shell/navigation smoke, Table interaction
  smoke, Table model smoke, and Tree/VirtualizedList smoke.
- Done: `crates/ui_components/tests/table.rs` is now a small helper/module facade. Test ownership
  lives under `crates/ui_components/tests/table/` split by behavior rows, filters/toolbar,
  editing contracts, layout contracts, public exports, runtime interactions, and runtime layout.
- Done: `examples/ui-foundation-gallery/src/shell.rs` now keeps the `GalleryShell` state, render
  facade, window entry points, and public crate re-exports. Private shell implementation moved into
  `src/shell/support.rs`, `src/shell/components.rs`, and `src/shell/overlay.rs`, so Components and
  Overlay sample rendering no longer live in the shell facade.
- Done: Full focused UI verification passed before the merge to `main`: component public surface,
  Menu, ContextMenu, Tree, Table, gallery metadata, overlay, tree, table, full
  `open-gpui-ui-components`, and full `open-gpui-ui-foundation-gallery`.
- Done: the hybrid registry work was merged to `main` and pushed as `e257d52f`; the remote feature
  branch `origin/refactor/native-ui-hybrid-registry` was deleted after merge. The local merged branch
  still exists as a historical pointer and should not be deleted unless requested.
- Done: `refactor/docking-flat-motion-runtime` merged the docking motion runtime pass into `main`.
  Docking now uses real final-size pane content reveal, sampled pane/divider/zoom retargeting,
  presentation-scene-seeded drop facts, programmatic Splitter motion, and a shared
  `open_gpui_ui_core::MotionTimeline` runtime primitive.
- Done: Dock overlay/drop-preview geometry now follows Dear ImGui's current-target model: preview
  rectangles stay pinned to the current semantic target instead of interpolating from previous
  preview bounds. Overlay motion remains lifecycle/opacity-only; pane, divider, zoom, and
  programmatic Splitter interpolation remain because they represent real layout motion.
- Done: ADR 0015 records the generalized UI motion runtime boundary after native registry ADRs
  occupied ADR 0013 and ADR 0014.
- Done on `refactor/docking-render-authority-convergence`: U1-U5 of
  `docs/plans/2026-07-02-004-refactor-docking-render-authority-convergence-plan.md` now converge
  deterministic docking geometry on shared scene/layout helpers. Render parity tests cover root,
  nested, floating, empty-central, splitter, tab-bar, tiny floating, and zoomed bounds; deterministic
  viewport facts are scene-seeded; split geometry uses `split_geometry`; tab/floating chrome uses
  `chrome_geometry`; and the only remaining render-measured probe is the tab-label helper whose
  bounds depend on GPUI text shaping and close-button layout.
- Done in the render-authority review tail: duplicate tab-label facts now no-op instead of
  advancing viewport host-scene generation, stable measured labels are preserved across equivalent
  base-scene registrations, stale measured labels are dropped when tab slots disappear, divider hit
  testing now uses the same zoom-resolved render scene as viewport host-scene facts, split layout
  resolution no longer materializes docking-side panel/handle Vecs, and render-geometry parity
  tests live in `host_render_geometry_parity_tests.rs`.
- Done on `refactor/docking-visual-affordance-runtime`: docking visual feedback now has a
  crate-private `DockVisualAffordanceScene` that describes drop target bodies, guide boxes, tab
  insertion slots, payload tab/ghost previews, route markers, rejected targets, divider handles and
  corners, focus rings, and zoom egress. Target preview rendering, visual-affordance motion,
  accessibility descriptors, divider/focus/zoom diagnostics, and the native runtime panel consume
  affordance summaries directly; the old `DockOverlayScene` bridge was deleted. Runtime visual
  diagnostics are published through `DockViewportRuntimeStatus`, so the native panel reads
  runtime-owned status instead of opening hosts for diagnostics.
- Done on `refactor/docking-visual-affordance-runtime`: `open_gpui_ui_core` now owns renderer-neutral
  rect motion helpers (`MotionEdge`, preferred edge selection, offscreen source rects, reveal rects,
  and rect interpolation). Docking transition sampling consumes those primitives, graph layout
  reuses `resolve_dock_split_layout`, and split/divider conversions to `UiRect` are centralized.
- Current docs direction: component ecosystem changes start with
  `cargo run -p xtask -- scan-ui-contract`, followed by public-surface, a11y, theme, or gallery
  focused nextest gates for behavior proof. Docking preview follow-up should start from the native
  example dogfood paths and focused docking nextest gates listed here.
- Not current roadmap work: broad splitting of every remaining 1k+ component file and
  `open-gpui-ui-headless` extraction.
- Blocked: None.
- Next action: continue command ecosystem hardening with either async provider UX or palette
  navigation polish.

# Citations

- [UI contract module refactor plan](../../plans/2026-07-01-001-refactor-ui-contract-test-modules-plan.md)
- [UI public gallery boundary plan](../../plans/2026-07-01-002-refactor-ui-public-gallery-boundaries-plan.md)
- [UI component contract rows plan](../../plans/2026-07-01-003-refactor-ui-component-contract-registry-plan.md)
- [UI family boundary plan](../../plans/2026-07-01-004-refactor-ui-family-boundaries-plan.md)
- [UI contract/a11y/theme plan](../../plans/2026-07-01-005-refactor-ui-contract-a11y-theme-plan.md)
- [UI contract tooling plan](../../plans/2026-07-02-001-refactor-ui-contract-tooling-plan.md)
- [Native UI hybrid registry architecture plan](../../plans/2026-07-02-002-refactor-native-ui-hybrid-registry-architecture-plan.md)
- [Docking flat motion runtime plan](../../plans/2026-07-02-002-refactor-docking-flat-motion-runtime-plan.md)
- [UI motion runtime foundation plan](../../plans/2026-07-02-003-refactor-ui-motion-runtime-foundation-plan.md)
- [UI framework deep modules plan](../../plans/2026-07-02-003-refactor-ui-framework-deep-modules-plan.md)
- [UI framework deep modules verification](verification/2026-07-02-ui-framework-deep-modules.md)
- [Docking render authority convergence plan](../../plans/2026-07-02-004-refactor-docking-render-authority-convergence-plan.md)
- [Docking visual affordance runtime plan](../../plans/2026-07-03-001-refactor-docking-visual-affordance-runtime-plan.md)
- [Docking visual affordance runtime progress](progress/2026-07-03-docking-visual-affordance-runtime.md)
- [Docking affordance authority cleanup plan](../../plans/2026-07-03-002-refactor-docking-affordance-authority-cleanup-plan.md)
- [Native UI framework design research report](../../../native-ui-framework-design-research/report.md)
- [Native UI framework distribution strategy decision](decisions/open-gpui-native-ui-framework-distribution-strategy.md)
- [Native UI framework strategy architecture page](../../architecture/native-ui-framework-strategy.md)
- [ADR 0013: Open GPUI Native UI Hybrid Registry](../../adr/0013-open-gpui-native-ui-hybrid-registry.md)
- [ADR 0014: Remove Open GPUI Native UI Hybrid Registry](../../adr/0014-remove-native-ui-hybrid-registry.md)
- [ADR 0015: UI Motion Runtime Foundation](../../adr/0015-ui-motion-runtime-foundation.md)
- [Native UI framework research handoff](sessions/2026-07-02-native-ui-framework-design-research-handoff.md)
- [Docking flat motion runtime progress](progress/2026-07-02-docking-flat-motion-runtime-plan.md)
- [UI motion runtime foundation progress](progress/2026-07-02-ui-motion-runtime-foundation.md)
- [Open GPUI command palette status items](progress/2026-07-04-open-gpui-command-palette-status-items.md)
- [Open GPUI command palette status items verification](verification/open-gpui-command-palette-status-items-20260704.md)
