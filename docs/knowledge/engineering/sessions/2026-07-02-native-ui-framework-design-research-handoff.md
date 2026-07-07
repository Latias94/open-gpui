---
type: "Session Handoff"
title: "Native UI framework design research handoff"
description: "Continuation state after the 28-item native UI framework design research report."
timestamp: 2026-07-02T07:48:09Z
tags: ["open-gpui", "ui", "research", "registry", "component-library"]
status: "active"
git_branch: "main"
git_commit: "22e86ce722486bbecb9edd111a8cc1cf23c0196e"
verified_by: "native UI framework research bundle generated and validated before archival"
---

# Summary

The native UI framework design research is complete and summarized in
`docs/research/native-ui-framework-design-research.md`. It covers 28 references including shadcn/ui,
Radix, Floating UI, React Aria, Zag, Ark, Base UI, fret, gpui-component, Zed UI / GPUI, SwiftUI,
Compose, Flutter, Slint, Iced, egui, Xilem, Makepad, Tauri, Electron, TanStack, Storybook,
Style Dictionary, AccessKit, Cargo tooling, AI-era component distribution, and a hybrid registry
model.

The central conclusion is that Open GPUI should not replicate a web source registry. The better
fit is Cargo crate distribution plus a metadata registry that drives scaffold recipes, component
contracts, gallery samples, theme token validation, accessibility claims, and verification commands.

# Verified State

- The archived raw research bundle contained 28 JSON results.
- Each result covered all 36 required fields from the generated field schema.
- The one-time generator produced the Markdown report before the raw bundle was removed.
- `docs/research/native-ui-framework-design-research.md` contains 28 table-of-contents entries and 28
  detailed anchors.
- The current working tree has the research bundle and engineering memory updates as uncommitted
  changes.

# Open Threads

- Decide whether to turn the strategy into a formal ADR now or wait until the first concrete
  implementation plan defines schema names, CLI commands, crate boundaries, and compatibility rules.
- Convert the research conclusion into a `ce-plan` if the next step is implementation.
- Keep broad component breadth deferred until the shared behavior foundations are stronger:
  overlay positioning, focus/dismiss/layer, accessibility semantics, theme token schema, and
  component contract metadata.

# Next Action

Recommended next action is to write a focused architecture plan for the hybrid registry MVP:
public manifest shape, scaffold recipe shape, registry-to-docs/gallery derivation, verification
commands, and the first official components to prove the loop.

Write a formal ADR after that plan chooses durable public names and compatibility policy.

# Citations

- [Native UI framework design research report](../../../../docs/research/native-ui-framework-design-research.md)
- [Hybrid registry strategy decision](../decisions/open-gpui-native-ui-framework-distribution-strategy.md)
- [Research report verification](../verification/native-ui-framework-research-report-20260702.md)
