---
type: Current State
title: Open GPUI UI productization state
status: active
timestamp: 2026-07-02T17:02:11+08:00
git_branch: refactor/native-ui-hybrid-registry
related_plan:
  - docs/plans/2026-07-01-001-refactor-ui-contract-test-modules-plan.md
  - docs/plans/2026-07-01-002-refactor-ui-public-gallery-boundaries-plan.md
  - docs/plans/2026-07-01-003-refactor-ui-component-contract-registry-plan.md
  - docs/plans/2026-07-01-004-refactor-ui-family-boundaries-plan.md
  - docs/plans/2026-07-01-005-refactor-ui-contract-a11y-theme-plan.md
  - docs/plans/2026-07-02-001-refactor-ui-contract-tooling-plan.md
  - docs/plans/2026-07-02-002-refactor-native-ui-hybrid-registry-architecture-plan.md
related_research:
  - native-ui-framework-design-research/report.md
related_adr:
  - docs/adr/0006-open-gpui-ui-headless-extraction-checkpoint.md
  - docs/adr/0008-open-gpui-ui-component-productization-roadmap.md
  - docs/adr/0013-open-gpui-native-ui-hybrid-registry.md
related_decision:
  - docs/knowledge/engineering/decisions/open-gpui-native-ui-framework-distribution-strategy.md
verified_by:
  - cargo run -p xtask -- verify
  - cargo run -p xtask -- scan-ui-registry
  - cargo test -p xtask ui_registry
  - cargo test -p xtask commands
  - cargo nextest run -p open-gpui-ui-components component_registry_manifest --no-fail-fast
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_catalog_consumes_component_contract_registry official_component_catalog_entries_have_signals_and_sample_selectors gallery_catalog_entries_satisfy_component_registry_manifest_evidence gallery_story_contracts_reference_component_registry_manifest_rows gallery_story_contracts_cover_components_state_readouts_and_overlays --no-fail-fast
  - cargo test -p xtask
  - cargo run -p xtask -- scan-ui-contract
  - cargo run -p xtask -- scan-theme-schema
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
  - git diff --check
  - python native-ui-framework-design-research/generate_report.py
  - python -m py_compile native-ui-framework-design-research/generate_report.py
  - python C:\Users\Frankorz\.codex\skills\research\validate_json.py -f native-ui-framework-design-research\fields.yaml -d native-ui-framework-design-research\results
  - python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering
---

# Current State

- Branch: `refactor/native-ui-hybrid-registry`; latest `main` has been merged into this branch.
- Done: Public-surface tests now consume the component contract registry instead of gallery/test
  helper maps. The registry owns official components, state contracts, adapter-only helpers,
  internal anatomy, removed targets, source mappings, docs tokens, gallery status, and default
  export intent.
- Done: `Command`, `Menu`, `ContextMenu`, `Tree`, and Table behavior snapshots now have explicit
  owner modules. The completed family-boundary pass keeps public behavior stable while replacing
  stale single-file source assumptions.
- Done: `component_contract` is split into responsibility modules; focused a11y contracts now cover
  representative component families; the theme JSON schema and loader facade are exported through
  root and prelude.
- Done on `refactor/ui-contract-tooling-audit`: `xtask` is split into command/scanner modules;
  `scan-ui-contract` audits registry rows, default exports, docs tokens, source homes, gallery
  conformance evidence, representative a11y claims, and the committed theme schema artifact.
- Done on `refactor/ui-contract-tooling-audit`: `docs/schemas/open-gpui-theme-v1.schema.json` is a
  reviewable artifact generated from `theme_json_schema()` through
  `open-gpui-ui-components --example export_theme_schema`, with `scan-theme-schema` drift coverage.
- Done: Native UI framework design research is complete in
  `native-ui-framework-design-research/report.md`. The current ecosystem strategy is Cargo-first
  distribution plus a machine-readable metadata registry for component contracts, scaffold recipes,
  gallery samples, theme tokens, a11y claims, and verification commands.
- Done: `docs/plans/2026-07-02-002-refactor-native-ui-hybrid-registry-architecture-plan.md` is
  implementation-ready. It sequences manifest model, scaffold recipe metadata, JSON/schema export,
  gallery evidence alignment, `xtask` drift checks, docs, memory, and ADR 0013.
- Done on `refactor/native-ui-hybrid-registry`: the component registry manifest is implemented as
  a deterministic projection of `component_contract`, scaffold recipes are recorded in
  `component_contract/recipes.rs`, and committed artifacts now live at
  `docs/registry/open-gpui-component-registry-v1.json` and
  `docs/schemas/open-gpui-component-registry-v1.schema.json`.
- Done on `refactor/native-ui-hybrid-registry`: `cargo run -p xtask -- scan-ui-registry` compares
  generated registry/schema output with committed artifacts, checks recipe references and generated
  file intents, and now runs before `scan-ui-contract` in `xtask verify`.
- Done on `refactor/native-ui-hybrid-registry`: foundation gallery tests consume the manifest for
  catalog and story evidence without moving selector constants into `open-gpui-ui-components`.
- Done: Full focused UI verification passed before the merge to `main`: component public surface,
  Menu, ContextMenu, Tree, Table, gallery metadata, overlay, tree, table, full
  `open-gpui-ui-components`, and full `open-gpui-ui-foundation-gallery`.
- Current docs direction: component ecosystem changes start with
  `cargo run -p xtask -- scan-ui-registry` and then `cargo run -p xtask -- scan-ui-contract`,
  followed by public-surface, a11y, theme, or gallery focused nextest gates for behavior proof.
- Not current roadmap work: broad splitting of every remaining 1k+ component file and
  `open-gpui-ui-headless` extraction.
- Blocked: None.
- Next action: commit the hybrid registry docs, memory, and ADR close-out.

# Citations

- [UI contract module refactor plan](../../plans/2026-07-01-001-refactor-ui-contract-test-modules-plan.md)
- [UI public gallery boundary plan](../../plans/2026-07-01-002-refactor-ui-public-gallery-boundaries-plan.md)
- [UI component contract registry plan](../../plans/2026-07-01-003-refactor-ui-component-contract-registry-plan.md)
- [UI family boundary plan](../../plans/2026-07-01-004-refactor-ui-family-boundaries-plan.md)
- [UI contract/a11y/theme plan](../../plans/2026-07-01-005-refactor-ui-contract-a11y-theme-plan.md)
- [UI contract tooling plan](../../plans/2026-07-02-001-refactor-ui-contract-tooling-plan.md)
- [Native UI hybrid registry architecture plan](../../plans/2026-07-02-002-refactor-native-ui-hybrid-registry-architecture-plan.md)
- [Native UI framework design research report](../../../native-ui-framework-design-research/report.md)
- [Native UI framework distribution strategy decision](decisions/open-gpui-native-ui-framework-distribution-strategy.md)
- [Native UI framework strategy architecture page](../../architecture/native-ui-framework-strategy.md)
- [Native UI hybrid registry implementation guide](../../architecture/native-ui-hybrid-registry.md)
- [ADR 0013: Open GPUI Native UI Hybrid Registry](../../adr/0013-open-gpui-native-ui-hybrid-registry.md)
- [Native UI framework research handoff](sessions/2026-07-02-native-ui-framework-design-research-handoff.md)
