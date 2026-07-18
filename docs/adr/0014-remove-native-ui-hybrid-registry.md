# ADR 0014: Remove Open GPUI Native UI Hybrid Registry

**Status**: Accepted
**Date**: 2026-07-02

## Context

ADR 0013 introduced a generated component registry manifest, JSON/schema artifacts, scaffold
recipe metadata, gallery evidence checks, and `scan-ui-registry`. The replacement initially kept a
large local contract row with source maps, API method inventories, export intent, Gallery status,
docs tokens, accessibility evidence, and test names. That still duplicated facts owned by Rust
modules, Gallery stories, DevTools adapters, native tests, and documentation.

The expected consumers were humans, tooling, and AI agents.
In practice, AI agents can inspect the crate source directly, while maintainers still need the typed contract tables and focused tests rather than a second generated registry surface.
Keeping the manifest layer creates maintenance cost across Rust types, generated artifacts, `xtask`, docs, gallery tests, and memory without adding a proven workflow.

## Decision

Open GPUI removes the native UI hybrid registry layer.

- Remove the component registry manifest API and schema export.
- Remove scaffold recipe metadata as a public component contract surface.
- Remove committed registry JSON/schema artifacts.
- Remove `cargo run -p xtask -- scan-ui-registry` and its `xtask verify` integration.
- Keep `crates/ui_components/src/component_contract/` as a narrow product authority for the 48
  official component ids, revisions, families, and required scenario ids.
- Generate typed public-export facts from the same macro declaration as each `pub use`; common,
  extended, and Table diagnostic owners remain explicit.
- Keep Gallery selectors, presentation groups, and runtime probes Gallery-owned. Official stories
  carry canonical `ComponentContractMetadata`; Gallery-local adapter/anatomy/state rows do not.
- Keep native test coordinates in sibling `*.scenarios.toml` artifacts owned by each integration
  test target.
- Project canonical id/revision/family metadata into DevTools without giving DevTools registry
  ownership.
- Keep `cargo run -p xtask -- scan-ui-contract` as a join-and-execute gate. It validates the narrow
  product rows, typed export declarations, Gallery projection, docs projection, scenario bindings,
  and exact nextest coordinates, but owns none of those downstream facts.

## Consequences

Positive:

- The UI productization workflow has fewer generated artifacts and fewer duplicate facts.
- AI and human contributors read the crate source and typed contract rows directly.
- `xtask verify` loses a scan that mostly checked generated registry drift rather than user-facing behavior.
- The remaining contract system is easier to reason about: narrow product rows plus federated,
  executable owner projections.

Negative:

- External tools no longer have a ready-made JSON manifest of the component surface.
- Scaffold recipe metadata is no longer available as a structured artifact.
- Any future hosted registry, marketplace, or `gpui add` work must start from a new ADR and fresh evidence.

## Follow-Up Work

- Keep component contract rows small; do not add source paths, Rust methods, Gallery selectors,
  package names, test functions, or docs ownership back to them.
- Prefer source inspection, focused gallery samples, and behavior tests when adding component capabilities.
- Revisit public registry/scaffold tooling only if real application work shows repeated manual friction that source inspection cannot solve.

## Related Documents

- `docs/adr/0013-open-gpui-native-ui-hybrid-registry.md`
- `docs/architecture/native-ui-framework-strategy.md`
- `docs/ui/component-contract.md`
- `docs/verification.md`
