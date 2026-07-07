---
type: "Decision"
title: "Open GPUI native UI framework distribution strategy"
description: "Cargo crate plus source-inspection strategy for the native UI framework and component ecosystem."
timestamp: 2026-07-02T07:48:09Z
tags: ["open-gpui", "ui", "components", "ai-native"]
status: "active"
git_branch: "main"
git_commit: "22e86ce722486bbecb9edd111a8cc1cf23c0196e"
verified_by:
  - "native UI framework research bundle generated and validated before archival"
  - "cargo run -p xtask -- scan-ui-contract"
related_adr: "docs/adr/0014-remove-native-ui-hybrid-registry.md"
---

# Decision

Open GPUI should treat its native UI ecosystem as a Rust-first framework with a Cargo-first
distribution model:

- Core primitives and official components ship as normal Cargo crates.
- Crate source and typed component contract rows are the inspection surface for humans and AI
  agents.
- Focused local checks prove component contracts, theme tokens, accessibility claims, gallery
  samples, and verification commands.
- Generated registry manifests and source-copy scaffold recipes should not become a product surface
  unless real application work proves direct source inspection is insufficient.

This means Open GPUI should borrow shadcn/ui's strong examples and local verification discipline,
but not copy the registry/scaffold distribution model.

ADR 0014 supersedes the earlier hybrid registry experiment. The current reusable drift gate is
`cargo run -p xtask -- scan-ui-contract`.

# Context

The July 2026 native UI framework research compared 28 references across frontend component
distribution, headless primitives, native UI frameworks, Rust distribution tooling, design tokens,
accessibility, gallery tooling, and AI-era component workflows.

The strongest pattern is not a single framework to clone. The reusable pattern is a layered
contract: behavior kernels, typed component anatomy, explicit state ownership, design tokens,
accessibility semantics, gallery examples, and automated verification. Rust already has a strong
package distribution base through Cargo and crates.io, and AI agents can inspect crate source
directly, so a generated metadata registry did not earn its maintenance cost.

The local codebase already points in this direction through `component_contract`, `xtask`
verification, committed theme schema artifacts, gallery metadata, and focused component contract
tests.

# Alternatives

- **Clone shadcn/ui source registry.** This maximizes copy-to-own familiarity, but fights Cargo,
  complicates upgrades, and risks turning the ecosystem into unmanaged pasted source.
- **Use Cargo crates plus source inspection.** This is idiomatic Rust and matches AI-era workflows
  when paired with typed contract rows and focused verification.
- **Build a closed first-party component crate.** This is simplest operationally, but does not
  create the ecosystem surface the project wants.
- **Adopt a hybrid registry model.** Cargo remains the package authority, while metadata registry
  entries make components inspectable, scaffoldable, verifiable, and AI-friendly. This was trialed
  and removed by ADR 0014.

Cargo crates plus direct source inspection are the current preferred direction.

# Consequences

- Future architecture plans should prioritize overlay positioning, focus/dismiss/layer management,
  accessibility semantics, theme token schema, and component contract metadata before expanding
  component breadth.
- Official components should continue to prove behavior through tests, gallery metadata, contract
  scanners, and `xtask` checks.
- ADR 0014 is now the formal decision removing registry schema names, export commands, and scaffold
  recipe metadata.
- A later implementation plan should map this strategy onto existing files such as
  `crates/ui_components/src/component_contract/`, `xtask/src/ui_contract.rs`,
  `docs/schemas/open-gpui-theme-v1.schema.json`, and `docs/ui/component-contract.md`.

# Citations

- [Native UI framework design research report](../../../../docs/research/native-ui-framework-design-research.md)
- Research outline and field schema were one-time generated artifacts and have been removed after
  report archival.
- [Research report verification](../verification/native-ui-framework-research-report-20260702.md)
- [ADR 0014: Remove Open GPUI Native UI Hybrid Registry](../../../adr/0014-remove-native-ui-hybrid-registry.md)
