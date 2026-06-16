# ADR 0006: Open GPUI UI Headless Extraction Checkpoint

**Status**: Proposed
**Date**: 2026-06-16

## Context

ADR 0005 chose an adapter-first, headless-ready component architecture and deferred a standalone
`open-gpui-ui-headless` crate until repeated behavior contracts existed.

The overlay component series now provides the first meaningful extraction evidence:

- `open_gpui_ui_core::overlay` owns renderer-neutral overlay policy, presence, dismissal,
  focus-intent, stack, and placement vocabulary.
- `TooltipState`, `PopoverState`, `DialogState`, `MenuState`, and `ContextMenuState` expose
  testable resolved state for the first overlay family.
- `MenuState` and the earlier `Tabs`/`RadioGroup` work prove reusable roving-focus behavior.
- `open_gpui_ui_components::overlay` adapts shared policy into GPUI `anchored` and `deferred`
  rendering fields without owning a global overlay runtime.
- The foundation gallery now exposes deterministic samples for tooltip, popover, dialog, menu, and
  context-menu behavior.

This is enough evidence to decide whether to create a headless crate immediately or continue
hardening the boundary in place.

## Decision

Do **not** create `open-gpui-ui-headless` yet.

Keep the headless-ready behavior inside `open-gpui-ui-core` and `open-gpui-ui-components` until the
remaining adapter leaks are removed and at least one more non-overlay composite family reuses the
same contracts.

The next extraction target should be a small behavior crate only after the public boundary can avoid
GPUI runtime and rendering types. The likely extraction candidates are:

- overlay policy, presence, dismissal, focus-intent, and placement vocabulary;
- roving-focus navigation helpers;
- component resolved-state descriptors that do not contain `Window`, `App`, `Context`,
  `RenderOnce`, `IntoElement`, callback types, focus handles, scroll handles, or GPUI element IDs.

Do not extract these yet:

- GPUI `anchored`/`deferred` adapter helpers;
- concrete `div()` render trees;
- focus handle allocation and concrete focus restoration;
- outside-press event subscriptions;
- barrier rendering and hit-test blocking;
- theme resolution into GPUI colors;
- AccessKit relationship wiring that depends on concrete element IDs.

## Rationale

The overlay series proves real reuse, but the boundary is not clean enough for a stable headless
crate:

- Several state types still expose `open_gpui::Pixels`, `Point`, or component-specific GPUI adapter
  state because sizing and placement currently depend on Open GPUI geometry.
- `ContextMenuState` includes resolved GPUI placement. This is useful for gallery verification, but
  it is not a framework-agnostic contract.
- Concrete focus restoration is still intent-only in state and implemented by adapters.
- Tooltip hover/focus timing and outside-press subscriptions remain adapter work.
- Dialog focus trapping and menu typeahead/submenus are intentionally deferred, so extracting now
  would freeze an incomplete behavioral surface.

Keeping the contracts in place lets the project continue to learn from real components without
locking the wrong crate boundary.

## Consequences

Positive:

- The current overlay components remain usable and testable.
- Future work can keep improving resolved state without cross-crate migration churn.
- The project avoids creating a nominally headless crate that still depends on GPUI details.

Negative:

- Other UI frameworks cannot consume a standalone Open GPUI headless package yet.
- Some behavior helpers live beside GPUI adapter code longer than ideal.
- Future extraction may require public API migration if current state types keep GPUI geometry.

## Extraction Gate

Revisit `open-gpui-ui-headless` when all of the following are true:

1. At least two component families outside simple buttons share the same behavior helpers.
2. Public resolved-state types avoid GPUI runtime/rendering types and callback types.
3. Geometry vocabulary either lives in `open-gpui-ui-core` as renderer-neutral aliases or has a
   clear non-GPUI representation.
4. Focus scope, dismissible layer ordering, and focus restoration have tests that do not require a
   GPUI window.
5. Gallery samples and component tests can identify which behavior is headless and which behavior
   is GPUI adapter-owned.

## Follow-Up Work

- Move remaining generic roving-focus helpers out of `tabs.rs` into a neutral component behavior
  module.
- Separate renderer-neutral menu placement input from `GpuiOverlayPlacement`.
- Add focus-scope and dismissible-layer ordering tests before implementing nested popovers,
  submenus, command dialogs, or sheets.
- Keep `docs/ui/component-contract.md` current whenever a component state type adds new behavior
  metadata.
