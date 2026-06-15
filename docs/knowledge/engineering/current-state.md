---
type: "Current State"
title: "Current Engineering State"
description: "Short durable summary of the active engineering state."
tags: ["engineering-memory"]
timestamp: 2026-06-15T07:40:07Z
status: "active"
---

# Current State

- Goal: Land the first Open GPUI UI foundation slice and preserve the foundation-first decision in durable memory.
- Branch: `feat/open-gpui-ui-core`
- Last verified: `git diff --check -- docs/verification.md examples/ui-foundation-gallery crates/ui_core`, `cargo check -p open-gpui-ui-core`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-core`, and `cargo nextest run -p open-gpui-ui-foundation-gallery` all passed; nextest is green with 14/14 `ui-core` tests and 10/10 gallery tests.
- Done: Added the `open-gpui-ui-core` crate with sizing, density, adaptive, token, overlay, a11y, and focus foundation vocabulary; ADR 0004 and memory bundle now point at the foundation-first direction and explicitly record the reference repositories (`fret`, `fret-ui-kit`, `fret-ui-shadcn`, `gpui-component`, plus broader open source UI references).
- Done: Wrote the first follow-up plan for a dedicated pure-foundation gallery example at `docs/plans/2026-06-15-001-feat-ui-foundation-gallery-plan.md`.
- Done: Completed U1 of the gallery plan by adding `examples/ui-foundation-gallery` as a workspace package with a small library, thin binary entrypoint, pure foundation dependency surface, empty shell, section registry, and targeted tests.
- Done: Completed U2 by replacing the U1 placeholder for tokens, sizing/density, and adaptive pages with real `open-gpui-ui-core` data models, rendered sample tables, and a compact/desktop viewport switch.
- Done: Completed U3 by replacing focus/a11y and overlay placeholders with interactive demos: focus-visible controls, accessibility roles/actions/state, overlay geometry samples, and an anchored deferred popover.
- Done: Completed U4 by adding the UI foundation gallery to `docs/verification.md` with focused package commands and manual compact/desktop, focus/a11y, and overlay dogfood checks.
- Blocked: None.
- Next action: Run `cargo run -p open-gpui-ui-foundation-gallery` for a manual visual dogfood pass, then commit the scoped UI foundation work when ready.

# Citations

[1] [ADR 0004](../../adr/0004-open-gpui-component-library-strategy.md)
[2] [Decision](decisions/open-gpui-ui-foundation-first.md)
[3] [Session handoff](sessions/open-gpui-component-library-handoff.md)
[4] [Verification](../../adr/0004-open-gpui-component-library-strategy.md#success-metrics)
[5] [Plan](../../plans/2026-06-15-001-feat-ui-foundation-gallery-plan.md)
[6] [Manual verification guide](../../verification.md)
