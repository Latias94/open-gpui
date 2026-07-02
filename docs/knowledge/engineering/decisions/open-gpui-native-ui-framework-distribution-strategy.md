---
type: "Decision"
title: "Open GPUI native UI framework distribution strategy"
description: "Hybrid Cargo crate plus metadata registry strategy for the native UI framework and component ecosystem."
timestamp: 2026-07-02T07:48:09Z
tags: ["open-gpui", "ui", "registry", "components", "ai-native"]
status: "active"
git_branch: "main"
git_commit: "22e86ce722486bbecb9edd111a8cc1cf23c0196e"
verified_by: "python C:\\Users\\Frankorz\\.codex\\skills\\research\\validate_json.py -f native-ui-framework-design-research\\fields.yaml -d native-ui-framework-design-research\\results"
---

# Decision

Open GPUI should treat its native UI ecosystem as a Rust-first framework with a hybrid distribution
model:

- Core primitives and official components ship as normal Cargo crates.
- A machine-readable metadata registry describes component contracts, theme tokens, accessibility
  claims, gallery samples, scaffold recipes, and verification commands.
- Source-copy recipes are useful for app-owned customization, but they should not become the primary
  distribution authority.
- The first-class ecosystem loop should be `add/scaffold -> modify -> verify -> document`, with
  local Rust tooling and CI gates proving the result.

This means Open GPUI should borrow shadcn/ui's AI-friendly documentation, registry metadata, and
copy-to-own ergonomics, but not copy the web-specific source registry model as-is.

# Context

The July 2026 native UI framework research compared 28 references across frontend component
distribution, headless primitives, native UI frameworks, Rust distribution tooling, design tokens,
accessibility, gallery tooling, and AI-era component workflows.

The strongest pattern is not a single framework to clone. The reusable pattern is a layered contract:
headless behavior kernels, typed component anatomy, explicit state ownership, design tokens,
accessibility semantics, gallery examples, and automated verification. Rust already has a strong
package distribution base through Cargo and crates.io, so a shadcn-style registry should become
metadata and recipe infrastructure rather than the source of truth for official components.

The local codebase already points in this direction through `component_contract`, `xtask`
verification, committed theme schema artifacts, gallery metadata, and focused component contract
tests.

# Alternatives

- **Clone shadcn/ui source registry.** This maximizes copy-to-own familiarity, but fights Cargo,
  complicates upgrades, and risks turning the ecosystem into unmanaged pasted source.
- **Use Cargo crates only.** This is idiomatic Rust, but misses AI-era discoverability, scaffold
  intent, component anatomy metadata, docs/gallery derivation, and local verification recipes.
- **Build a closed first-party component crate.** This is simplest operationally, but does not
  create the ecosystem surface the project wants.
- **Adopt a hybrid model.** Cargo remains the package authority, while metadata registry entries
  make components inspectable, scaffoldable, verifiable, and AI-friendly.

The hybrid model is the current preferred direction.

# Consequences

- Future architecture plans should prioritize overlay positioning, focus/dismiss/layer management,
  accessibility semantics, theme token schema, and component contract metadata before expanding
  component breadth.
- Registry work should start as a small metadata manifest and scaffold recipe format, not a full
  source registry.
- Official components should continue to prove behavior through tests, gallery metadata, contract
  scanners, and `xtask` checks.
- Formal ADR should be written when the team commits to concrete public schema names, CLI commands,
  crate boundaries, and compatibility policy.
- A later implementation plan should map this strategy onto existing files such as
  `crates/ui_components/src/component_contract/`, `xtask/src/ui_contract.rs`,
  `docs/schemas/open-gpui-theme-v1.schema.json`, and `docs/ui/component-contract.md`.

# Citations

- [Native UI framework design research report](../../../../native-ui-framework-design-research/report.md)
- [Research outline](../../../../native-ui-framework-design-research/outline.yaml)
- [Research fields](../../../../native-ui-framework-design-research/fields.yaml)
- [Research report verification](../verification/native-ui-framework-research-report-20260702.md)
