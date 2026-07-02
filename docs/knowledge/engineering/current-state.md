---
type: Current State
title: Open GPUI UI productization state
status: active
timestamp: 2026-07-02T23:45:00+08:00
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
  - git diff --check
  - python native-ui-framework-design-research/generate_report.py
  - python -m py_compile native-ui-framework-design-research/generate_report.py
  - python C:\Users\Frankorz\.codex\skills\research\validate_json.py -f native-ui-framework-design-research\fields.yaml -d native-ui-framework-design-research\results
  - python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering
---

# Current State

- Branch: `main`; `origin/main` was at `f3a7de9` before the docking flat motion runtime merge.
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
- Current docs direction: component ecosystem changes start with
  `cargo run -p xtask -- scan-ui-contract`, followed by public-surface, a11y, theme, or gallery
  focused nextest gates for behavior proof. Docking preview follow-up should start from the native
  example dogfood paths and focused docking nextest gates listed here.
- Not current roadmap work: broad splitting of every remaining 1k+ component file and
  `open-gpui-ui-headless` extraction.
- Blocked: None.
- Next action: push merged `main`, then continue the fearless refactor sequence with the remaining
  large gallery render owners, especially
  `examples/ui-foundation-gallery/src/pages/components/render/sections.rs`.

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
- [Native UI framework design research report](../../../native-ui-framework-design-research/report.md)
- [Native UI framework distribution strategy decision](decisions/open-gpui-native-ui-framework-distribution-strategy.md)
- [Native UI framework strategy architecture page](../../architecture/native-ui-framework-strategy.md)
- [ADR 0013: Open GPUI Native UI Hybrid Registry](../../adr/0013-open-gpui-native-ui-hybrid-registry.md)
- [ADR 0014: Remove Open GPUI Native UI Hybrid Registry](../../adr/0014-remove-native-ui-hybrid-registry.md)
- [ADR 0015: UI Motion Runtime Foundation](../../adr/0015-ui-motion-runtime-foundation.md)
- [Native UI framework research handoff](sessions/2026-07-02-native-ui-framework-design-research-handoff.md)
- [Docking flat motion runtime progress](progress/2026-07-02-docking-flat-motion-runtime-plan.md)
- [UI motion runtime foundation progress](progress/2026-07-02-ui-motion-runtime-foundation.md)
