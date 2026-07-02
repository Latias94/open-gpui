# ADR 0014: Remove Open GPUI Native UI Hybrid Registry

**Status**: Accepted
**Date**: 2026-07-02

## Context

ADR 0013 introduced a generated component registry manifest, JSON/schema artifacts, scaffold recipe metadata, gallery evidence checks, and `scan-ui-registry`.
That experiment duplicated facts already available in Rust source and typed component contract rows.

The expected consumers were humans, tooling, and AI agents.
In practice, AI agents can inspect the crate source directly, while maintainers still need the typed contract tables and focused tests rather than a second generated registry surface.
Keeping the manifest layer creates maintenance cost across Rust types, generated artifacts, `xtask`, docs, gallery tests, and memory without adding a proven workflow.

## Decision

Open GPUI removes the native UI hybrid registry layer.

- Remove the component registry manifest API and schema export.
- Remove scaffold recipe metadata as a public component contract surface.
- Remove committed registry JSON/schema artifacts.
- Remove `cargo run -p xtask -- scan-ui-registry` and its `xtask verify` integration.
- Keep `crates/ui_components/src/component_contract/` as the local typed contract authority for public-surface tests, docs tokens, gallery status, source mappings, and export intent.
- Keep `cargo run -p xtask -- scan-ui-contract` as the reusable drift gate for component productization.
- Keep gallery selectors and story probes gallery-owned; gallery tests may consume typed contract rows directly.

## Consequences

Positive:

- The UI productization workflow has fewer generated artifacts and fewer duplicate facts.
- AI and human contributors read the crate source and typed contract rows directly.
- `xtask verify` loses a scan that mostly checked generated registry drift rather than user-facing behavior.
- The remaining contract system is easier to reason about: source rows plus focused tests.

Negative:

- External tools no longer have a ready-made JSON manifest of the component surface.
- Scaffold recipe metadata is no longer available as a structured artifact.
- Any future hosted registry, marketplace, or `gpui add` work must start from a new ADR and fresh evidence.

## Follow-Up Work

- Keep component contract rows small and test-owned; do not rebuild the removed manifest under another name.
- Prefer source inspection, focused gallery samples, and behavior tests when adding component capabilities.
- Revisit public registry/scaffold tooling only if real application work shows repeated manual friction that source inspection cannot solve.

## Related Documents

- `docs/adr/0013-open-gpui-native-ui-hybrid-registry.md`
- `docs/architecture/native-ui-framework-strategy.md`
- `docs/ui/component-contract.md`
- `docs/verification.md`
