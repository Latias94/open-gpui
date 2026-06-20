---
type: "Session Handoff"
title: "Open GPUI component library planning handoff"
description: "Capture the current component-library decision, the ADR update, and the next action: keep building the UI foundation before expanding components."
timestamp: 2026-06-15T08:19:48Z
tags: ["open-gpui", "session", "handoff", "ui"]
status: "active"
source_session: "019ec6c8-5566-7062-8458-21ebe1360573"
git_branch: "main"
git_commit: "6d1caf947e1116419a7e55a1d3636712947541d0"
---

# Summary

The current work has moved from strategy into the first implementation slice. The ADR still says the library must stay outside `open-gpui` core, and the new `open-gpui-ui-core` crate now carries the first foundation primitives.

The reference repository set is now part of the durable record: `../../../../../fret`,
`../../../../../fret/ecosystem/fret-ui-kit`, `../../../../../fret/ecosystem/fret-ui-shadcn`,
`repo-ref/gpui-component`, and broader open source references such as Flutter, Jetpack Compose,
Radix UI, React Aria, React Spectrum, shadcn/ui, and Apple HIG / SwiftUI.

The first consumer plan is now written as `docs/plans/2026-06-15-001-feat-ui-foundation-gallery-plan.md`.
It chooses a dedicated pure-foundation gallery example instead of repurposing `smoke-native`.

U1 of that plan is now complete: `examples/ui-foundation-gallery` is a first-class workspace
package with a small library, a thin `open-gpui-ui-foundation-gallery` binary, a pure foundation
dependency surface, an empty shell, section metadata for tokens/sizing/adaptive/focus-a11y/overlay,
and targeted package tests.

U2 is now complete as well: tokens, sizing/density, and adaptive pages render real
`open-gpui-ui-core` data models. The shell includes a compact/desktop viewport switch that drives
`DeviceShellSwitchPolicy` and `DeviceAdaptivePolicy`; focus/a11y and overlay remain scoped to U3.

U3 is now complete: focus/a11y renders focus-visible controls, `Role::SpinButton`, `Role::Button`,
`Role::Switch`, `AccessibleAction::Increment` / `AccessibleAction::Decrement`, and `Toggled`
state. Overlay renders deterministic geometry from `anchor_rect_from_point`,
`prefer_visual_bounds`, and `outer_bounds_with_window_margin`, plus an anchored deferred popover
that opens from the gallery trigger and closes from the popover or Escape.

U4 is now complete: `docs/verification.md` includes the UI foundation gallery as the manual
dogfood path, with focused `cargo fmt`, `cargo check`, and `cargo nextest` commands plus manual
compact/desktop, focus/a11y, and overlay checks.

The foundation slice has been committed as `f626464 feat(ui): add UI foundation core gallery`.
The next plan is `docs/plans/2026-06-15-002-feat-ui-components-first-slice-plan.md`, which creates
`open-gpui-ui-components` as the first concrete component consumer crate on top of `ui_core`.

That slice is now complete: `open-gpui-ui-components` exists, Button and Switch are implemented with
state/metrics/color helpers, and the Components gallery page renders real samples backed by those
components.

ADR 0005 now captures the next architectural move: keep the official component layer adapter-first
and headless-ready, then defer a dedicated `open-gpui-ui-headless` crate until multiple components
prove a shared behavior model.

# Verified State

- Git head is `6d1caf947e1116419a7e55a1d3636712947541d0` on `main`.
- The worktree already had `docs/adr/README.md` modified and `docs/adr/0004-open-gpui-component-library-strategy.md` untracked before the latest ADR edit.
- The branch is now `feat/open-gpui-ui-core`.
- `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery` passed.
- `cargo check -p open-gpui-ui-core` passed.
- `cargo check -p open-gpui-ui-components` passed.
- `cargo check -p open-gpui-ui-foundation-gallery` passed.
- `cargo nextest run -p open-gpui-ui-core` passed with 14/14 tests green.
- `cargo nextest run -p open-gpui-ui-components` passed with 10/10 tests green.
- `cargo nextest run -p open-gpui-ui-foundation-gallery` passed with 11/11 tests green.
- Commit `f626464` records the foundation crate, gallery, ADR, verification docs, and memory bundle; the current working tree continues the next components slice.
- ADR 0005 defines the official component architecture direction: adapter-first, headless-ready, and future-proofed for a later headless extraction.
- The new crate exports component sizing, color intents, Button/Switch state, and a prelude for consumers.
- The gallery manifest depends on `open_gpui`, `open_gpui_platform`, `open_gpui_ui_core`, and `open_gpui_ui_components`; tests guard against accidental `open_gpui_canvas`, `open_gpui_docking`, or styled `open_gpui_ui` dependencies.
- Gallery page tests now cover semantic token order, sizing/density metrics, adaptive samples, focus/a11y roles, overlay geometry, and the Components page metadata contract.

# Open Threads

- Whether the next slice after Button/Switch should add `TextInput` / `Field`, richer token plumbing, visual regression coverage, or more runtime-facing helpers.
- Whether any later helper needs to move into `open-gpui` core because the foundation crate proves a real runtime gap.

# Next Action

Use ADR 0005 to guide the next proof-point component slice, starting with `TextInput` and `Field`
and the supporting theme/focus primitives.

# Citations

[1] [ADR 0004](../../../adr/0004-open-gpui-component-library-strategy.md)
[2] [Decision](../decisions/open-gpui-ui-foundation-first.md)
[3] [Plan](../../../plans/2026-06-15-001-feat-ui-foundation-gallery-plan.md)
[4] [Manual verification guide](../../../verification.md)
[5] [Components first slice plan](../../../plans/2026-06-15-002-feat-ui-components-first-slice-plan.md)
