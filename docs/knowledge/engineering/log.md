# Engineering Memory Update Log

## 2026-06-15
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
