# ADR 0006: Open GPUI UI Headless Extraction Checkpoint

**Status**: Proposed
**Date**: 2026-06-16

## Context

ADR 0005 chose an adapter-first, headless-ready component architecture and deferred a standalone
`open-gpui-ui-headless` crate until repeated behavior contracts existed.

The overlay component series provided the first meaningful extraction evidence:

- `open_gpui_ui_core::overlay` owns renderer-neutral overlay policy, presence, dismissal,
  focus-intent, stack, and placement vocabulary.
- `TooltipState`, `PopoverState`, `DialogState`, `MenuState`, and `ContextMenuState` expose
  testable resolved state for the first overlay family.
- `MenuState` and the earlier `Tabs`/`RadioGroup` work prove reusable roving-focus behavior.
- `open_gpui_ui_components::overlay` adapts shared policy into GPUI `anchored` and `deferred`
  rendering fields without owning a global overlay runtime.
- The foundation gallery now exposes deterministic samples for tooltip, popover, dialog, menu, and
  context-menu behavior.

The later shell, layout, and choice/search series add stronger evidence:

- `ToolbarState`, `SidebarState`, `TabsState`, `RadioGroupState`, `MenuState`, and `ListboxState`
  reuse the same roving-focus and disabled-skip helper vocabulary.
- `ListboxState`, `SelectState`, `ComboboxState`, and `CommandState` share grouped item anatomy,
  selected versus active value semantics, disabled option handling, empty states, and scroll
  viewport metadata.
- `ScrollAreaState` and `SplitterState` prove that runtime-sensitive GPUI handles and pointer
  interaction can stay in adapters while viewport intent and resize constraints remain testable.
- Component tests now include a structural guard that public resolved-state structs avoid GPUI
  runtime/rendering types such as `Window`, `App`, `Context`, `RenderOnce`, `IntoElement`,
  `ElementId`, `Entity`, focus handles, scroll handles, and callbacks.

This is enough evidence to reassess whether to create a headless crate immediately or continue
hardening the boundary in place.

## Decision

Do **not** create `open-gpui-ui-headless` yet.

Keep the headless-ready behavior inside `open-gpui-ui-core` and `open-gpui-ui-components` until the
remaining extraction blockers are removed. The project now has enough cross-family reuse to justify
planning extraction, but not enough boundary cleanliness to publish a stable headless crate.

The next extraction target should be a small behavior crate only after the public boundary can avoid
GPUI runtime and rendering types. The likely extraction candidates are:

- overlay policy, presence, dismissal, focus-intent, and placement vocabulary;
- roving-focus navigation helpers, promoted from `pub(crate)` to a public neutral module after its
  API is named independently from GPUI adapters;
- listbox collection navigation, selected/active item resolution, disabled skip behavior, and
  typeahead target helpers;
- scroll viewport intent and splitter resize constraint solvers after geometry units are
  normalized;
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
- `TextInputController`, which is a concrete GPUI `EntityInputHandler` adapter rather than a
  framework-neutral text editing core.

## Rationale

The component series proves real reuse, but the boundary is still not clean enough for a stable
headless crate:

- Several state types still expose `open_gpui::Pixels` or `Point` geometry aliases because sizing
  and point anchors currently depend on Open GPUI geometry.
- `open_gpui_ui_core` still re-exports GPUI accessibility and focus types (`Role`, `Toggled`,
  `FocusHandle`, `Focusable`) instead of defining a framework-neutral facade.
- Overlay component states expose `GpuiOverlayState`; its policy is neutral, but the type name and
  snap/deferred fields are adapter-facing and should be split before extraction.
- Concrete focus restoration is still intent-only in state and implemented by adapters.
- `TextInputController` is intentionally GPUI-backed; a future headless package would need either a
  smaller text-editing model or an explicit adapter-only classification for editable text.
- Tooltip/hover-card timing, dialog focus trapping, nested overlay focus scopes, menu submenus, and
  application command registry integration remain intentionally deferred, so extracting now would
  freeze an incomplete behavioral surface.

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
   Completed for roving focus and listbox/choice navigation.
2. Public resolved-state types avoid GPUI runtime/rendering types and callback types. Guarded by
   `public_resolved_state_contracts_avoid_gpui_runtime_types`.
3. Geometry vocabulary either lives in `open-gpui-ui-core` as renderer-neutral aliases or has a
   clear non-GPUI representation.
4. Overlay state is split into neutral policy/presence/focus data and adapter-only deferred/snap
   metadata instead of exposing `GpuiOverlayState` from component resolved state.
5. Focus scope, dismissible layer ordering, and focus restoration have tests that do not require a
   GPUI window. The current checkpoint covers overlay stack outside-press and focus-restore
   ordering; full focus-trap/scope traversal remains future component work.
6. Gallery samples and component tests can identify which behavior is headless and which behavior
   is GPUI adapter-owned.

## Follow-Up Work

- Completed 2026-06-16: generic roving-focus helpers moved out of `tabs.rs` into
  `open_gpui_ui_components::roving_focus`; `Tabs` keeps compatibility re-exports while `Menu` and
  `RadioGroup` depend on the neutral module.
- Completed 2026-06-16: `ContextMenuState` now stores renderer-neutral `OverlayPlacementInput`;
  GPUI placement is resolved only inside the adapter/render boundary.
- Completed 2026-06-16: added window-free overlay stack ordering tests for outside press and focus
  restoration through `resolve_outside_press` and `resolve_focus_restore`. Full focus-trap/scope
  traversal remains deferred until the first nested overlay component needs it.
- Completed 2026-06-17: shell/layout/choice/search components added additional extraction evidence
  through Toolbar, Sidebar, ScrollArea, Splitter, Listbox, Select, Combobox, and Command. A
  structural component test now guards public resolved-state structs against GPUI runtime/rendering
  type leaks.
- Next: write a focused extraction-prep plan that splits neutral geometry/focus/a11y facades,
  separates `GpuiOverlayState` into neutral and adapter halves, and decides whether
  `TextInputController` remains adapter-only or gains a smaller framework-neutral editing model.
- Keep `docs/ui/component-contract.md` current whenever a component state type adds new behavior
  metadata.
