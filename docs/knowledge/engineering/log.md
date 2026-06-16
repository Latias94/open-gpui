# Engineering Memory Update Log

## 2026-06-16
* **Decision**: Completed the U8 headless-readiness checkpoint and added ADR 0006. The decision is
  to keep `open-gpui-ui-headless` deferred for now: the overlay family proves reusable behavior
  contracts, but several state types still expose GPUI geometry or adapter placement state. Future
  extraction should first neutralize those leaks, move generic roving-focus helpers out of
  component-specific modules, and add window-free focus-scope / dismissible-layer tests.
* **Update**: Completed U5 of the overlay component series by adding the interactive non-modal
  `Popover` component and `PopoverState` contract to `open-gpui-ui-components`. The first slice
  models controlled versus uncontrolled open mode, default-open state, trigger expanded/selected
  intent, outside-press policy, placement metadata, initial focus intent, focus restoration intent,
  non-modal dismissible layer state, token intents, and metrics.
* **Update**: Added Overlay gallery Popover samples for default-open, controlled, consuming
  outside press, and disabled cases. The controlled sample is owned by gallery shell state and
  closes through the shared Escape handler.
* **Update**: Documented Popover in `docs/ui/component-contract.md` and `docs/verification.md`,
  with nested popovers, modal popover barriers, and a reusable focus-scope runtime explicitly
  deferred.
* **Update**: Verified U5 with `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Completed U4 of the overlay component series by adding the descriptive `Tooltip`
  component and `TooltipState` contract to `open-gpui-ui-components`. The first slice models text
  or simple element content, hover/focus/manual open intent, delay policy, placement metadata,
  disabled trigger behavior, tooltip layer state, token intents, and metrics while keeping the
  content non-interactive.
* **Update**: Added Overlay gallery Tooltip samples for hover/focus, focus-only, manual delayed,
  and disabled cases. The gallery wires hover state and focus handles at the adapter layer so
  keyboard traversal can reveal descriptive tooltip content without putting GPUI runtime types into
  `TooltipState`.
* **Update**: Documented Tooltip in `docs/ui/component-contract.md` and `docs/verification.md`,
  including the current `Role::Label` mapping until GPUI exposes a public tooltip role wrapper.
* **Update**: Verified U4 with `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Started U3 by adding `open_gpui_ui_components::overlay`, a narrow GPUI adapter helper
  layer that maps shared overlay policy into deferred priority, snap margin, GPUI anchor/offset,
  Escape open-change, and outside-press open-change decisions without introducing a global overlay
  runtime.
* **Update**: Verified U3 with `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Started U2 of the overlay component series by extending
  `open-gpui-ui-core::overlay` with renderer-neutral behavior contracts for layer kind, presence,
  outside-press policy, Escape policy, dismiss reason, focus restore, initial focus, layer-state
  resolution, Escape stack resolution, and anchor/placement input.
* **Update**: Updated the UI foundation gallery overlay page to show the shared behavior contract
  matrix for tooltip, popover, dialog, and menu policies, and documented the overlay resolved-state
  boundary in `docs/ui/component-contract.md`.
* **Update**: Verified U2 with `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-core`, and `cargo nextest
  run -p open-gpui-ui-foundation-gallery`.
* **Update**: Started U1 of the overlay component series. Added direct AccessKit repair tests for
  valid and invalid cross-node references in `crates/gpui/src/window/a11y.rs`, converted the
  `svg_renderer` font fixtures to runtime optional loading so the `open-gpui` lib test harness
  compiles without the missing bundled fonts in this checkout, and added a Foundation Gallery test
  that locks explicit accessible labels plus label-to-control association metadata.
* **Update**: Added `--page components` support to the UI foundation gallery binary so the prior
  Components-page `accesskit_consumer` crash path can be smoke-tested directly without manual tab
  navigation first.
* **Update**: Added focused verification guidance for GPUI accessibility repair changes to
  `docs/verification.md`, including the `open-gpui` a11y smoke test and the Components-page
  regression note for the `accesskit_consumer` crash.
* **Update**: Verified U1 with focused `open-gpui`/component/gallery checks, nextest runs, and a
  direct `cargo run -p open-gpui-ui-foundation-gallery -- --page components` smoke that stayed
  alive until the 30s timeout instead of reproducing the Components-page crash.
* **Update**: Wrote the next-series overlay component plan at
  `docs/plans/2026-06-16-001-feat-ui-overlay-component-series-plan.md`, scoped to the post-U6
  sequence: accessibility/gallery runtime gate, shared overlay behavior contracts, GPUI overlay
  adapter helpers, Tooltip, Popover, Dialog, Menu/ContextMenu, and a headless-readiness checkpoint.
* **Decision**: Start the next series with the AccessKit repair smoke instead of jumping directly
  to Tooltip/Popover. Overlay components will increase explicit accessibility relationships, so the
  crash barrier needs a repeatable verification gate first.
* **Decision**: Keep `open-gpui-ui-headless` deferred for now. The overlay series should produce
  renderer-neutral behavior contracts inside the current crates, then reassess extraction after
  multiple overlay components reuse them.
* **Update**: Completed U6 of the official component roadmap by adding `Badge` and `IconButton`
  to `open-gpui-ui-components` with resolved state, GPUI adapters, theme intents, exports, gallery
  samples, component tests, and foundation gallery metadata tests.
* **Update**: Hardened GPUI accessibility tree repair so invalid cross-node AccessKit references
  (`labelled_by`, `controls`, `active_descendant`, and related node-id properties) are stripped
  before the update reaches platform adapters. This addresses the Components-page crash reported
  with `accesskit_consumer` panicking while resolving a missing explicit label reference.
* **Update**: Added `Size::icon_size()` to `open-gpui-ui-core` and moved `IconButton` glyph sizing
  onto that shared foundation helper.
* **Update**: Verified U6 with `cargo fmt --all --check`, focused `cargo check` for `open-gpui`,
  `open-gpui-ui-core`, `open-gpui-ui-components`, and `open-gpui-ui-foundation-gallery`, plus
  `cargo nextest run -p open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and
  `cargo nextest run -p open-gpui-ui-foundation-gallery`.
* **Verification Note**: The direct `open-gpui` a11y unit test command could not compile the
  package test harness because the local checkout is missing bundled test fonts under
  `assets/fonts/ibm-plex-sans` and `assets/fonts/lilex`; normal `cargo check -p open-gpui` passes.
* **Update**: Applied U5 follow-up cleanup after review: GPUI `div` now exposes `aria_required`
  and `aria_disabled`, RadioGroup uses those flags plus per-item disabled semantics, Tabs/Radio
  share stable-key selection and roving navigation helpers, and Toggle exports its own
  metrics/colors aliases while reusing Button visuals internally.
* **Update**: Accepted the U5 efficiency review findings by avoiding full `BTreeMap` focus-handle
  clones in RadioGroup render and skipping redundant active-state writes on repeated activation.
* **Update**: Verified the U5 cleanup with `cargo fmt --all`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run
  -p open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Committed the main U5 slice as
  `5e562f3 feat(ui): add radio group and toggle components`.
* **Update**: Completed U5 of the official component roadmap by adding `RadioGroup` and `Toggle`
  to `open-gpui-ui-components` with pure resolved-state contracts, GPUI adapters, explicit
  exports, gallery dogfood, and targeted tests.
* **Decision**: `RadioGroup` reuses the U4 roving-focus helpers and maps items with
  `Role::RadioButton` plus `aria_selected` because the current GPUI AccessKit wrapper does not
  expose a separate checked property. `Toggle` remains button-like (`Role::Button` +
  `aria_toggled`) and does not reuse Checkbox tri-state semantics.
* **Update**: Updated `docs/ui/component-contract.md`, `docs/verification.md`, and the Components
  gallery samples to cover RadioGroup required/disabled/roving state and Toggle pressed state.
* **Update**: Verified the U5 component and gallery surfaces with `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run
  -p open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Completed U4 of the official component roadmap by adding `Tabs` to
  `open-gpui-ui-components` with a pure resolved-state contract, GPUI adapter, roving-focus
  helpers, gallery dogfood, and targeted tests.
* **Update**: Fixed the vertical Tabs dogfood so the left tab rail scrolls inside a constrained
  gallery card, matching the user-reported overflow issue.
* **Update**: Updated `docs/ui/component-contract.md`, `docs/verification.md`, and the foundation
  gallery samples to cover Tabs roving-focus behavior, horizontal automatic activation, and
  vertical manual activation.
* **Update**: Verified the Tabs slice with `cargo fmt --all`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run
  -p open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Committed the Tabs slice as `f0dbf96 feat(ui): add Tabs roving focus slice`.

## 2026-06-15
* **Update**: Completed U3 of the official component roadmap by adding `Checkbox` and `Label` to
  `open-gpui-ui-components` with resolved state, GPUI adapters, theme intents, tests, gallery
  samples, and updated verification guidance.
* **Update**: Updated `docs/ui/component-contract.md` so the resolved-state contract now includes
  Checkbox indeterminate state and Label association metadata.
* **Update**: Updated `docs/verification.md` so the Components manual dogfood now includes Checkbox
  and Label association checks in addition to Button, Switch, TextInput, and Field.
* **Update**: Implemented the real single-line editable `TextInputController` slice in
  `open-gpui-ui-components`, including GPUI `EntityInputHandler` / `ElementInputHandler`
  integration, UTF-16 selection and marked-range conversion, grapheme-aware deletion, clipboard
  actions, and gallery dogfood for the default components sample.
* **Subagent Finding**: Recorded editable TextInput controller research at
  `docs/knowledge/engineering/subagents/text-input-controller-research.md`: use GPUI's native
  input handler path for single-line editing and defer multiline/password/editor-grade behavior.
* **Update**: Updated `docs/ui/component-contract.md` so the contract now records that
  `TextInputController` owns the editable single-line path while `Field` stays composition-only,
  and multiline/password/undo/redo/completion remain out of scope.
* **Update**: Completed the runtime theme table slice from the official component roadmap. Added
  `ColorState`, `ThemeMode`, `ThemeColor`, and immutable `ThemeSnapshot` support, taught
  `ThemeResolver::resolve_with` to resolve `(TokenKey, ColorState)` before falling back to intent
  RGB, and exposed light/dark/high-contrast mode metadata in the foundation gallery.
* **Update**: Wrote the official UI component roadmap at
  `docs/plans/2026-06-15-004-feat-ui-component-roadmap-plan.md`. The next-series order is runtime
  theme table, editable TextInput controller, Checkbox/Label, roving focus/Tabs,
  RadioGroup/Toggle, Badge/IconButton, shared overlay behavior, Tooltip/Popover, Dialog,
  Menu/ContextMenu, ScrollArea/Splitter, Toolbar/Sidebar, gallery conformance, and then headless
  extraction readiness review.
* **Decision**: Keep `open-gpui-ui-headless` deferred. The project should first prove repeated
  renderer-neutral contracts across Button, Switch, TextInput/Field, Checkbox/Radio, Tabs, and at
  least one overlay family; `gpui-component`, `fret-ui-kit`, and `fret-ui-shadcn` remain references
  rather than runtime dependencies.
* **Subagent Finding**: Recorded roadmap reference research at
  `docs/knowledge/engineering/subagents/ui-component-roadmap-reference-research.md`: use
  `gpui-component` for GPUI-native implementation patterns, `fret-ui-kit` for policy-layer
  references, and avoid wholesale copying from either repository.
* **Update**: Added the shared `FocusRing` primitive to `open-gpui-ui-components` and migrated
  Button, Switch, TextInput, and the focus/a11y gallery demo from border-width focus styling to a
  box-shadow focus-visible adapter that does not change layout.
* **Update**: Added `ThemeResolver` to `open-gpui-ui-components` and migrated Button, Switch,
  TextInput, and Field render paths to resolve `ColorIntent` values centrally while keeping token
  intent visible in component state.
* **Update**: Implemented the TextInput/Field component slice from ADR 0005: added
  `TextInputState`, `FieldState`, `FieldMessage`, component exports, gallery samples, tests, and
  `docs/ui/component-contract.md`; focused component and gallery checks pass.
* **Update**: Recorded text input research showing that full editable text input must use GPUI's
  `EntityInputHandler` / `ElementInputHandler` path, so this slice intentionally keeps platform
  text editing as a follow-up rather than faking input with key events.
* **Update**: Drafted ADR 0005 for the official component architecture. It records the adapter-first, headless-ready direction, the GPUI adapter boundary, the future `open-gpui-ui-headless` extraction trigger, and the next follow-up work on `TextInput` / `Field`, theme resolution, and focus rings.
* **Update**: Completed the first `open-gpui-ui-components` slice: the workspace now has Button and Switch components backed by `ui_core` sizing, tokens, role/toggled state, and a Components gallery page; `cargo check` and `cargo nextest` pass for `open-gpui-ui-core`, `open-gpui-ui-components`, and `open-gpui-ui-foundation-gallery`.
* **Update**: Committed the first UI foundation slice as `f626464 feat(ui): add UI foundation core gallery`, then created the next plan at `docs/plans/2026-06-15-002-feat-ui-components-first-slice-plan.md` for `open-gpui-ui-components` with Button, Switch, gallery dogfood, and verification.
* **Update**: Completed U4 of the UI foundation gallery plan: `docs/verification.md` now documents focused `open-gpui-ui-core` / gallery checks and the manual `cargo run -p open-gpui-ui-foundation-gallery` dogfood path; package checks and nextest runs pass.
* **Update**: Completed U3 of the UI foundation gallery plan: focus/a11y and overlay now have interactive demos backed by `open-gpui-ui-core` focus/a11y/overlay vocabulary, and `cargo nextest run -p open-gpui-ui-foundation-gallery` passes 10/10 tests.
* **Update**: Completed U2 of the UI foundation gallery plan: tokens, sizing/density, and adaptive pages now render real `open-gpui-ui-core` data models, the shell has a compact/desktop switch, and `cargo nextest run -p open-gpui-ui-foundation-gallery` passes 8/8 tests.
* **Update**: Completed U1 of the UI foundation gallery plan by adding `examples/ui-foundation-gallery` as a workspace package with a small library, thin binary, pure foundation dependency surface, empty shell, section registry, and passing `cargo nextest run -p open-gpui-ui-foundation-gallery`.
* **Update**: Wrote the first follow-up plan at `docs/plans/2026-06-15-001-feat-ui-foundation-gallery-plan.md` and locked the first consumer choice to a dedicated pure-foundation gallery example.
* **Update**: Recorded the reference repository set for the Open GPUI UI strategy: `fret`, `fret-ui-kit`, `fret-ui-shadcn`, `gpui-component`, plus broader open source comparators such as Flutter, Jetpack Compose, Radix UI, React Aria, React Spectrum, shadcn/ui, and Apple HIG / SwiftUI.
* **Update**: Implemented the first Open GPUI UI foundation slice on `feat/open-gpui-ui-core` with the new `open-gpui-ui-core` crate, sizing/adaptive/token/overlay helpers, a11y/focus re-exports, and passing `cargo nextest run -p open-gpui-ui-core`.
* **Update**: Updated ADR 0004 to prioritize a11y, focus, overlay, tokens, sizing, density, and adaptive layout before broad component rollout; added decision and session memory for the UI foundation-first strategy.
* **Initialization**: Created engineering wiki memory bundle.
