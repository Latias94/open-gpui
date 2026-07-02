---
type: Current State
title: Open GPUI UI productization state
status: active
timestamp: 2026-07-02T15:49:41+08:00
git_branch: main
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
related_decision:
  - docs/knowledge/engineering/decisions/open-gpui-native-ui-framework-distribution-strategy.md
verified_by:
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

- Branch: `main`; the UI family-boundary refactor has been merged and pushed to `origin/main`.
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
- Done: Full focused UI verification passed before the merge to `main`: component public surface,
  Menu, ContextMenu, Tree, Table, gallery metadata, overlay, tree, table, full
  `open-gpui-ui-components`, and full `open-gpui-ui-foundation-gallery`.
- Current docs direction: new registry, a11y, theme, or gallery conformance work starts with
  `cargo run -p xtask -- scan-ui-contract`, then drops to public-surface, a11y, theme, or gallery
  focused nextest gates for behavior proof.
- Not current roadmap work: broad splitting of every remaining 1k+ component file and
  `open-gpui-ui-headless` extraction.
- Blocked: None.
- Next action: run `ce-work` or goal-mode execution on
  `docs/plans/2026-07-02-002-refactor-native-ui-hybrid-registry-architecture-plan.md`.

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
- [Native UI framework research handoff](sessions/2026-07-02-native-ui-framework-design-research-handoff.md)
