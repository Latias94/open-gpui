---
type: "Work Progress"
title: "Native UI hybrid registry implementation"
description: "Implementation progress for the Cargo-first component registry manifest, scaffold recipe metadata, artifacts, gallery evidence, and xtask scan."
timestamp: 2026-07-02T17:02:11+08:00
tags: ["open-gpui", "ui", "registry", "component-library", "ce-work", "superseded"]
status: "superseded"
superseded_by: "docs/adr/0014-remove-native-ui-hybrid-registry.md"
related_plan: "docs/plans/2026-07-02-002-refactor-native-ui-hybrid-registry-architecture-plan.md"
git_branch: "refactor/native-ui-hybrid-registry"
verified_by:
  - "cargo run -p xtask -- verify"
  - "cargo run -p xtask -- scan-ui-registry"
  - "cargo test -p xtask ui_registry"
  - "cargo test -p xtask commands"
  - "cargo nextest run -p open-gpui-ui-components component_registry_manifest --no-fail-fast"
  - "cargo nextest run -p open-gpui-ui-foundation-gallery components_catalog_consumes_component_contract_registry official_component_catalog_entries_have_signals_and_sample_selectors gallery_catalog_entries_satisfy_component_registry_manifest_evidence gallery_story_contracts_reference_component_registry_manifest_rows gallery_story_contracts_cover_components_state_readouts_and_overlays --no-fail-fast"
---

# Summary

Superseded by ADR 0014. This note is retained as historical implementation context for the removed
hybrid registry experiment.

Implemented the first native UI hybrid registry slice from the July 2 plan.

# Completed

- Added a deterministic component registry manifest projection under
  `crates/ui_components/src/component_contract/manifest.rs`.
- Added scaffold recipe metadata under `crates/ui_components/src/component_contract/recipes.rs`.
- Added export examples for the manifest and schema.
- Committed generated artifacts at `docs/registry/open-gpui-component-registry-v1.json` and
  `docs/schemas/open-gpui-component-registry-v1.schema.json`.
- Added `cargo run -p xtask -- scan-ui-registry` and wired it into `xtask verify` before
  `scan-ui-contract`.
- Added gallery-side manifest evidence tests so catalog rows and story contracts reference manifest
  rows without moving gallery selector constants into `open-gpui-ui-components`.
- Documented the workflow in `docs/ui/component-contract.md`, `docs/verification.md`, and
  `docs/architecture/native-ui-framework-strategy.md`.

# Remaining

- Commit the hybrid registry docs, memory, and ADR close-out.

# Citations

- [Native UI hybrid registry architecture plan](../../../plans/2026-07-02-002-refactor-native-ui-hybrid-registry-architecture-plan.md)
- [Component contract docs](../../../ui/component-contract.md)
- [Verification docs](../../../verification.md)
- [Native UI framework strategy](../../../architecture/native-ui-framework-strategy.md)
- [Native UI hybrid registry implementation guide](../../../architecture/native-ui-hybrid-registry.md)
- [ADR 0013: Open GPUI Native UI Hybrid Registry](../../../adr/0013-open-gpui-native-ui-hybrid-registry.md)
