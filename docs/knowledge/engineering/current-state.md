---
type: "Current State"
title: "Current Engineering State"
description: "Short durable summary of the active engineering state."
tags: ["engineering-memory"]
timestamp: 2026-06-15T08:19:48Z
status: "active"
---

# Current State

- Goal: Grow the official Open GPUI component system under the adapter-first, headless-ready
  architecture from ADR 0005.
- Branch: `feat/open-gpui-ui-core`
- Last verified: `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`,
  `cargo check -p open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`,
  `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-foundation-gallery` passed for the TextInput/Field slice.
- Done: Added the `open-gpui-ui-core` crate with sizing, density, adaptive, token, overlay, a11y, and focus foundation vocabulary; ADR 0004 and memory bundle now point at the foundation-first direction and explicitly record the reference repositories (`fret`, `fret-ui-kit`, `fret-ui-shadcn`, `gpui-component`, plus broader open source UI references).
- Done: Wrote the first follow-up plan for a dedicated pure-foundation gallery example at `docs/plans/2026-06-15-001-feat-ui-foundation-gallery-plan.md`.
- Done: Completed U1 of the gallery plan by adding `examples/ui-foundation-gallery` as a workspace package with a small library, thin binary entrypoint, pure foundation dependency surface, empty shell, section registry, and targeted tests.
- Done: Completed U2 by replacing the U1 placeholder for tokens, sizing/density, and adaptive pages with real `open-gpui-ui-core` data models, rendered sample tables, and a compact/desktop viewport switch.
- Done: Completed U3 by replacing focus/a11y and overlay placeholders with interactive demos: focus-visible controls, accessibility roles/actions/state, overlay geometry samples, and an anchored deferred popover.
- Done: Completed U4 by adding the UI foundation gallery to `docs/verification.md` with focused package commands and manual compact/desktop, focus/a11y, and overlay dogfood checks.
- Done: Committed the foundation slice as `f626464 feat(ui): add UI foundation core gallery`.
- Done: Wrote the next plan for `open-gpui-ui-components` at `docs/plans/2026-06-15-002-feat-ui-components-first-slice-plan.md`, scoped to Button, Switch, gallery dogfood, and verification.
- Done: Completed the first components slice by scaffolding `crates/ui_components` as `open-gpui-ui-components`, implementing Button and Switch, wiring the Components gallery page, and updating the engineering memory bundle.
- Done: Drafted ADR 0005 for the official component architecture, choosing an adapter-first, headless-ready model and a future extraction path for `open-gpui-ui-headless`.
- Done: Wrote the TextInput/Field implementation plan at
  `docs/plans/2026-06-15-003-feat-ui-text-field-slice-plan.md` and the component contract guide at
  `docs/ui/component-contract.md`.
- Done: Implemented `TextInput` and `Field` in `open-gpui-ui-components` with resolved state,
  metrics, token intents, role/message metadata, tests, explicit exports, and gallery dogfood.
- Done: Recorded subagent research showing full editable text input must use GPUI's
  `EntityInputHandler` / `ElementInputHandler` path, so this slice intentionally remains a
  display/semantic contract slice.
- Done: Committed the TextInput/Field slice as `33842c4 feat(ui): add text field component slice`.
- Done: Added `ThemeResolver` to `open-gpui-ui-components`, moved Button/Switch/TextInput/Field
  render-time color conversion through it, and kept `ColorIntent` as the resolved state contract for
  token-aware tests and future headless extraction.
- Blocked: None.
- Next action: Plan and implement the no-layout-shift focus ring primitive from ADR 0005.

# Citations

[1] [ADR 0004](../../adr/0004-open-gpui-component-library-strategy.md)
[2] [Decision](decisions/open-gpui-ui-foundation-first.md)
[3] [Session handoff](sessions/open-gpui-component-library-handoff.md)
[4] [Verification](../../adr/0004-open-gpui-component-library-strategy.md#success-metrics)
[5] [Plan](../../plans/2026-06-15-001-feat-ui-foundation-gallery-plan.md)
[6] [Manual verification guide](../../verification.md)
[7] [Components first slice plan](../../plans/2026-06-15-002-feat-ui-components-first-slice-plan.md)
[8] [Official component architecture](../../adr/0005-open-gpui-official-component-architecture.md)
[9] [TextInput/Field plan](../../plans/2026-06-15-003-feat-ui-text-field-slice-plan.md)
[10] [Component contract guide](../../ui/component-contract.md)
[11] [Text input subagent finding](subagents/text-input-patterns.md)
