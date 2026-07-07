---
type: "Work Progress"
title: "Native UI hybrid registry architecture planning"
description: "Implementation-ready plan for the Cargo-first metadata registry and scaffold recipe MVP."
timestamp: 2026-07-02T08:08:24Z
tags: ["open-gpui", "ui", "registry", "plan", "component-library", "superseded"]
status: "superseded"
superseded_by: "docs/adr/0014-remove-native-ui-hybrid-registry.md"
related_plan: "docs/plans/2026-07-02-002-refactor-native-ui-hybrid-registry-architecture-plan.md"
git_branch: "main"
git_commit: "22e86ce722486bbecb9edd111a8cc1cf23c0196e"
verified_by: "rg heading/frontmatter/path scan"
---

# Summary

Superseded by ADR 0014. This note is retained as historical planning context for the removed
hybrid registry experiment.

Created the implementation-ready plan for the native UI hybrid registry architecture MVP.

The plan turns the July 2026 native UI framework research into executable work: derive a versioned component registry manifest from `component_contract`, define scaffold recipe metadata, export committed JSON/schema artifacts, connect gallery evidence without moving gallery selectors into the component crate, extend `xtask` drift checks, document the workflow, and write ADR 0013 after the implementation proves durable names.

# Details

- Plan path: `docs/plans/2026-07-02-002-refactor-native-ui-hybrid-registry-architecture-plan.md`.
- Strategy: Cargo crates remain the official distribution authority; the registry is metadata and recipe infrastructure, not a shadcn-style source registry.
- Implementation units: U1 manifest model, U2 recipe metadata, U3 JSON/schema export, U4 gallery evidence, U5 `xtask` registry scan, U6 docs/memory, U7 ADR.
- Plan review applied three clarity fixes before handoff: added missing `docs/architecture` and `docs/registry` scope entries, moved registry JSON artifacts out of `docs/schemas`, and reordered recipe metadata before artifact export.
- Verification so far: heading/frontmatter/path scan passed, YAML metadata parsed as `implementation-ready`, unit IDs are U1-U7, requirements are R1-R15, and `git diff --check` passed for the plan file.

# Next Action

Run `ce-work` or goal-mode execution on the plan.
The first implementation slice should start at U1 and U2 so the manifest and recipe vocabulary are complete before generating committed artifacts.

# Citations

- [Native UI hybrid registry architecture plan](../../../plans/2026-07-02-002-refactor-native-ui-hybrid-registry-architecture-plan.md)
- [Native UI framework research report](../../../../docs/research/native-ui-framework-design-research.md)
- [Hybrid registry strategy decision](../decisions/open-gpui-native-ui-framework-distribution-strategy.md)
