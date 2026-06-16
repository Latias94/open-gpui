---
type: "Current State"
title: "Current Engineering State"
description: "Short durable summary of the active engineering state."
tags: ["engineering-memory"]
timestamp: 2026-06-16T02:46:46Z
status: "active"
---

# Current State

## 2026-06-16

- Done: Completed U4 of the overlay component series by adding descriptive `Tooltip` to
  `open-gpui-ui-components` with `TooltipState`, hover/focus/manual open intent, delay policy,
  placement metadata, token/metric resolution, explicit exports, component tests, and Overlay
  gallery samples that reveal tooltip content from hover or keyboard focus while keeping disabled
  triggers closed.
- Done: Updated `docs/ui/component-contract.md` and `docs/verification.md` so Tooltip is documented
  as a descriptive non-interactive overlay contract, with timing execution and trigger/focus wiring
  remaining in the GPUI adapter/gallery layer.
- Last verified: `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the U4 Tooltip work.
- Next action: Start U5 Popover on top of the shared overlay adapter, covering controlled/default
  open state, Escape/outside dismissal, placement, and focus restoration.
- Done: Started U2 by extending `open-gpui-ui-core::overlay` from geometry helpers to
  renderer-neutral overlay behavior contracts: layer identity/kind, presence, outside-press policy,
  Escape policy, dismiss reason, focus restore intent, initial focus intent, layer-state resolution,
  Escape stack resolution, and anchor/placement input. These contracts intentionally avoid GPUI
  runtime types.
- Done: Updated the foundation gallery overlay page to expose a behavior contract matrix for
  tooltip, popover, dialog, and menu policies, and updated `docs/ui/component-contract.md` with the
  overlay resolved-state boundary.
- Last verified: `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-core`, and `cargo nextest
  run -p open-gpui-ui-foundation-gallery` passed after the U2 overlay behavior contract work.
- Done: Started U3 by adding `open_gpui_ui_components::overlay`, a narrow GPUI adapter mapping
  layer that resolves deferred priority, snap margin, GPUI anchor/offset, Escape open-change, and
  outside-press open-change from the U2 renderer-neutral policy without owning a global overlay
  runtime or storing GPUI callbacks.
- Last verified: `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the U3 adapter helper work.
- Done: Started the overlay component series with U1 accessibility/gallery runtime gate work:
  added direct coverage for valid and invalid AccessKit cross-node references in
  `crates/gpui/src/window/a11y.rs`, removed the compile-time bundled-font dependency from the
  `svg_renderer` test harness so the `open-gpui` library tests compile in this checkout, and added
  a Gallery metadata test plus `--page components` startup path that lock explicit accessible
  labels, label-to-control association metadata, and the direct Components runtime smoke.
- Last verified: `cargo check -p open-gpui`, `cargo check -p open-gpui-ui-components`, `cargo
  check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui --lib
  window::a11y::tests::repair_tree_update`, `cargo nextest run -p open-gpui --lib
  svg_renderer::tests::`, `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run
  -p open-gpui-ui-foundation-gallery` passed for U1. `cargo run -p
  open-gpui-ui-foundation-gallery -- --page components` stayed alive until the 30s smoke timeout
  and did not reproduce the `accesskit_consumer` panic.
- Done: Committed the U6 Badge/IconButton slice as
  `9206210 feat(ui): add badge and icon button components`.
- Done: Wrote the next-series overlay component plan at
  `docs/plans/2026-06-16-001-feat-ui-overlay-component-series-plan.md`.
- Decision: The next execution series starts with an accessibility/gallery runtime gate, then
  renderer-neutral overlay behavior contracts, GPUI overlay adapter helpers, `Tooltip`, `Popover`,
  `Dialog`, `Menu`/`ContextMenu`, and finally a headless-readiness checkpoint. `ScrollArea`,
  `Splitter`, `Toolbar`, and `Sidebar` move to the following layout/shell-navigation series.
- Next action: Start U1 of the overlay component series: prove the AccessKit repair and gallery
  runtime smoke path before adding new overlay-heavy components.
- Done: Completed U6 of the official component roadmap by adding `Badge` and `IconButton` to
  `open-gpui-ui-components` with pure resolved-state contracts, GPUI adapters, explicit exports,
  theme intents, gallery dogfood, and targeted tests.
- Done: Added `Size::icon_size()` to the UI foundation sizing vocabulary so icon-bearing controls
  do not hide glyph metrics in individual component adapters.
- Done: Hardened GPUI accessibility tree repair to strip invalid cross-node references such as
  `labelled_by`, `controls`, and active-descendant pointers before handing updates to AccessKit
  platform adapters. This addresses the Components-page crash where `accesskit_consumer` panicked
  while resolving an explicit label reference to a missing node.
- Last verified: `cargo fmt --all --check`, `cargo check -p open-gpui`, `cargo check -p
  open-gpui-ui-core`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-core`, `cargo nextest run
  -p open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery` passed
  during U6 implementation.
- Verification note: `cargo nextest run -p open-gpui --lib
  window::a11y::tests::repair_tree_update_strips_invalid_node_references` could not compile the
  `open-gpui` test harness because local font assets under `assets/fonts/ibm-plex-sans` and
  `assets/fonts/lilex` are missing. The regular `cargo check -p open-gpui` path passes.
- Done: The next-series plan resolves the prior fork by doing the accessibility runtime smoke first
  and then continuing into shared overlay behavior.
- Done: Completed U5 of the official component roadmap by adding `RadioGroup` and `Toggle` to
  `open-gpui-ui-components` with pure resolved-state contracts, GPUI adapters, exports, gallery
  dogfood, and targeted tests.
- Done: Applied a follow-up U5 cleanup that shared Tabs/Radio selection helpers, removed the
  Toggle gallery sample drift, and gave `Toggle` its own exported metrics/colors aliases while
  keeping the Button implementation as the underlying visual model.
- Done: Committed the main U5 slice as `5e562f3 feat(ui): add radio group and toggle components`.
- Done: `RadioGroup` now reuses the U4 roving-focus helpers, exposes group required/disabled
  metadata plus per-item selected/focused/tab-stop state, and maps radio items with
  `Role::RadioButton` + `aria_selected` on the current GPUI/AccessKit surface.
- Done: `Toggle` now models button-like pressed state through `Role::Button` + `aria_toggled`
  while staying separate from Checkbox tri-state semantics.
- Last verified: `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed during U5 implementation and again after
  the follow-up cleanup.
- Next action: Start the next roadmap item (`Badge` / `IconButton`).
- Done: Completed U4 of the official component roadmap by adding `Tabs` to
  `open-gpui-ui-components` with a pure resolved-state contract, GPUI adapter, roving-focus
  helpers, gallery dogfood, and targeted tests.
- Done: Fixed the vertical Tabs dogfood so the left tab rail scrolls inside a constrained gallery
  card, matching the user-reported overflow issue.
- Done: Updated the Components gallery and verification docs to cover horizontal automatic
  activation and vertical manual activation, plus keyboard roving-focus verification.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the Tabs scroll fix.
- Done: Committed the Tabs slice as `f0dbf96 feat(ui): add Tabs roving focus slice`.
- Next action: Start the next roadmap item (`RadioGroup` / `Toggle`).

## 2026-06-15

- Goal: Grow the official Open GPUI component system under the adapter-first, headless-ready
  architecture from ADR 0005.
- Branch: `feat/open-gpui-ui-core`
- Last verified: `cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`,
  `cargo check -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`,
  `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery` passed after
  the Checkbox/Label slice.
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
- Done: Added `FocusRing` to `open-gpui-ui-components`, migrated Button/Switch/TextInput and the
  focus/a11y gallery demo to paint focus-visible state with GPUI box-shadow instead of changing
  border width, and covered the token intent plus no-layout-shift contract in tests.
- Done: Implemented the real single-line editable `TextInputController` slice in
  `open-gpui-ui-components`, including GPUI `EntityInputHandler` / `ElementInputHandler`
  integration, UTF-16 selection and marked-range conversion, grapheme-aware deletion, clipboard
  actions, and gallery dogfood for the default components sample.
- Done: Completed U3 of the official component roadmap by adding `Checkbox` and `Label` to
  `open-gpui-ui-components` with resolved state, GPUI adapters, theme intents, tests, gallery
  samples, and updated verification guidance.
- Done: Updated `docs/verification.md` so the Components manual dogfood now includes Checkbox and
  Label association checks in addition to Button, Switch, TextInput, and Field.
- Done: Updated the component contract to record that `TextInputController` now owns the editable
  single-line path while `Field` remains composition-only, and that multiline/password/undo/redo/
  completion stay out of scope.
- Done: Wrote the next-series roadmap at
  `docs/plans/2026-06-15-004-feat-ui-component-roadmap-plan.md`. The planned order is runtime
  theme table, real editable TextInput controller, Checkbox/Label, roving focus/Tabs,
  RadioGroup/Toggle, Badge/IconButton, shared overlay behavior, Tooltip/Popover, Dialog,
  Menu/ContextMenu, ScrollArea/Splitter, Toolbar/Sidebar, gallery conformance, then a headless
  extraction readiness review.
- Done: Recorded the planning decision that `open-gpui-ui-headless` remains deferred until repeated
  contracts exist across Button/Switch/TextInput/Field, Checkbox/Radio, Tabs, and at least one
  overlay family. Reference repositories remain inputs, not runtime dependencies.
- Done: Recorded reference repository findings at
  `docs/knowledge/engineering/subagents/ui-component-roadmap-reference-research.md`: use
  `gpui-component` for GPUI-native implementation patterns, `fret-ui-kit` for policy-layer
  references, and do not copy Fret runtime or `gpui-component` editor-grade input code wholesale.
- Done: Completed the runtime theme table slice by adding `ColorState`, `ThemeMode`,
  `ThemeColor`, and immutable `ThemeSnapshot` support to `open-gpui-ui-components`.
  `ThemeResolver::resolve_with` now resolves `(TokenKey, ColorState)` from light, dark, or
  high-contrast snapshots before falling back to intent RGB; the gallery token page exposes
  mode/revision metadata.
- Done: Recorded runtime theme reference guidance at
  `docs/knowledge/engineering/subagents/runtime-theme-reference-research.md`: keep U1 to
  immutable snapshots plus fallback semantics; defer app-level registries, user theme files, JSON
  schema, and hot reload.
- Done: Recorded editable TextInput controller reference guidance at
  `docs/knowledge/engineering/subagents/text-input-controller-research.md`: keep U2 to a
  single-line controller plus GPUI input handler adapter; defer multiline/password/editor features.
- Done: Updated `docs/ui/component-contract.md` to include Checkbox indeterminate state and Label
  association metadata in the resolved-state contract.
- Blocked: None.
- Next action: Commit the Checkbox/Label slice, then start U4 on roving focus and Tabs.

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
[12] [Official UI component roadmap](../../plans/2026-06-15-004-feat-ui-component-roadmap-plan.md)
[13] [Roadmap reference research](subagents/ui-component-roadmap-reference-research.md)
[14] [Runtime theme reference research](subagents/runtime-theme-reference-research.md)
[15] [Text input controller research](subagents/text-input-controller-research.md)
