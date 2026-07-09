---
type: "Subagent Finding"
title: "DevTools ecosystem prior memory research"
description: "Read-only research summary for command, timeline, layout, and gallery DevTools planning."
timestamp: 2026-07-09T01:46:28Z
tags: ["devtools", "command", "timeline", "layout", "gallery", "ce-plan"]
related_plan: "docs/plans/2026-07-09-001-feat-devtools-ecosystem-deepening-plan.md"
git_branch: "feat/devtools-ecosystem-deepening"
---

# Finding

- DevTools is already defined in repo history as a read-only, redacted snapshot surface. It should not mutate command dispatch, keymap persistence, layout, resource/form state, docking, or motion runtime state.
- The right dependency direction is source crate public snapshot facts -> feature-gated DevTools adapters -> `SnapshotEnvelope`; source crates must not depend on DevTools.
- Command inspection should productize the existing `crates/devtools/src/command.rs` adapters instead of creating a second command model.
- Timeline/event tracing is not yet modeled. Start with a renderer-neutral bounded event DTO and use motion frame demand/timeline facts as the first producer.
- Layout inspection should consume committed public facts such as scroll viewport snapshots and docking presentation/status facts. Avoid taffy/render private reach-through.
- Gallery dogfood is the integration gate because prior component-only tests missed shell composition issues.

# Evidence

- Existing plan and memory around real DevTools probes already reject static gallery fixture builders and require `DevtoolsRegistry` collection.
- Current gallery DevTools page registers accessibility, form, motion, resource, and theme probes, then emits unmounted scroll/docking diagnostics.
- Current command adapter tests cover command registry, keybinding projection, and keymap resolution to `SnapshotKind::Command`.
- Prior command ecosystem memory establishes GPUI as keymap/chord/dispatch authority, `open_gpui_command` as registry/snapshot/preflight owner, and UI components as render projections.
- Prior docking/layout memory establishes presentation scene / visual affordance scene as geometry authorities.

# Recommendation

- Sequence work as command gallery dogfood, inspector read projection, timeline DTO/adapters, layout DTO/adapters, then UI/docs polish.
- Preserve sanitizer coverage for probe ids, labels, node ids, payloads, diagnostics, custom snapshot kinds, paths, email-like text, token-like text, and key-value secrets.
- Prefer package-scoped verification on Windows and use `-j 1` only when local resource failures appear unrelated to source changes.

# Disposition

- Incorporated into `docs/plans/2026-07-09-001-feat-devtools-ecosystem-deepening-plan.md`.
- Two slower read-only research agents were interrupted after the plan had enough local evidence; they made no file changes.

# Citations

- `docs/plans/2026-07-08-004-feat-devtools-real-probes-dogfood-plan.md`
- `docs/knowledge/engineering/progress/2026-07-08-open-gpui-devtools-real-probes-dogfood.md`
- `docs/ui/command-ecosystem.md`
- `crates/devtools/src/command.rs`
- `examples/ui-foundation-gallery/src/pages/devtools.rs`
