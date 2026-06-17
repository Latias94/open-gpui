# ADR 0006: Open GPUI UI Headless Extraction Checkpoint

**Status**: Accepted
**Date**: 2026-06-17

## Context

ADR 0005 chose an adapter-first, headless-ready component architecture and deferred a standalone
`open-gpui-ui-headless` crate until repeated behavior contracts existed and the public boundary was
clean enough to move.

The overlay, shell, layout, and choice/search series now provide meaningful extraction evidence:

- `open_gpui_ui_core::overlay` owns renderer-neutral overlay policy, presence, dismissal,
  focus-intent, stack ordering, placement vocabulary, and `OverlayResolvedState`.
- `TooltipState`, `PopoverState`, `DialogState`, `MenuState`, `ContextMenuState`,
  `AlertDialogState`, `SheetState`, `HoverCardState`, `SelectState`, `ComboboxState`, and
  `CommandState` expose neutral overlay state instead of `GpuiOverlayState`.
- `ToolbarState`, `SidebarState`, `TabsState`, `RadioGroupState`, `MenuState`, and `ListboxState`
  reuse the same roving-focus and disabled-skip helper vocabulary.
- `ListboxState`, `SelectState`, `ComboboxState`, and `CommandState` share grouped item anatomy,
  selected versus active value semantics, disabled option handling, empty states, and scroll
  viewport metadata.
- `ScrollAreaState` and `SplitterState` prove that runtime-sensitive GPUI handles and pointer
  interaction can stay in adapters while viewport intent and resize constraints remain testable.
- `open_gpui_ui_core` now owns neutral geometry, accessibility, and focus facades:
  `UiPx`, `UiPoint`, `UiSize`, `UiRect`, `UiEdges`, `Role`, `Toggled`, `Orientation`,
  `AccessibleAction`, and `FocusTargetId`.
- Component tests now include structural guards that public resolved-state structs avoid GPUI
  runtime/rendering types such as `Window`, `App`, `Context`, `RenderOnce`, `IntoElement`,
  `ElementId`, `Entity`, focus handles, scroll handles, and callbacks.
- Adapter-only public surfaces are explicitly inventoried: `TextInputController`, externally
  supplied `ScrollHandle`, `focus_ring_shadow`, `GpuiOverlayState`, and GPUI overlay scheduling
  helpers.

This is enough evidence to start a focused extraction design, but not enough to create or publish
`open-gpui-ui-headless` in the current branch.

## Decision

Do **not** create `open-gpui-ui-headless` yet.

The extraction-prep series cleared the component resolved-state blockers, but a strict crate
boundary still has two unresolved core-level decisions:

1. `open_gpui_ui_core::adaptive` still exposes adaptive viewport policy through GPUI `Pixels`.
2. `UiPx` still carries GPUI style-conversion impls as an adapter convenience.

Keep the reusable behavior in the current crates until those two boundary questions are resolved.
The next correct step is a narrow extraction plan or ADR, not a crate in this branch. That plan
should start from the pure behavior modules and leave GPUI adapter APIs behind.

Likely first extraction candidates:

- overlay policy, presence, dismissal, stack ordering, focus-intent, and placement vocabulary;
- neutral geometry values once GPUI conversion impls are moved behind the adapter boundary;
- roving-focus navigation helpers;
- listbox collection navigation, selected/active item resolution, disabled skip behavior, and
  typeahead target helpers;
- scroll viewport intent and splitter resize constraint solvers.

Do not extract:

- GPUI `anchored`/`deferred` adapter helpers;
- concrete `div()` render trees;
- focus handle allocation and concrete focus movement;
- outside-press event subscriptions;
- barrier rendering and hit-test blocking;
- theme resolution into GPUI colors;
- AccessKit relationship wiring that depends on concrete element IDs;
- `TextInputController`, which is a concrete GPUI `EntityInputHandler` adapter rather than a
  framework-neutral text editing core.

No ADR 0007 is added in this checkpoint because the strict crate gate is not fully clear. ADR 0007
should be written with the actual extraction design once the adaptive and `UiPx` conversion
boundary decisions are made.

## Rationale

The current component catalog has enough reusable behavior to justify extraction planning. The
important shift since the original checkpoint is that public component state is now mostly neutral:
geometry, metrics, focus/a11y semantics, and overlay state no longer require GPUI runtime types.

However, creating a crate now would either:

- pull GPUI dependencies through `adaptive`/`UiPx` conversion convenience APIs, which defeats the
  point of a headless crate; or
- require hurried API surgery in the same change that creates the crate, which makes the boundary
  harder to review.

Keeping the contracts in place lets the next plan extract behavior deliberately while preserving the
working GPUI component stack.

## Consequences

Positive:

- Public component resolved state is now guarded against GPUI runtime/render leaks.
- GPUI adapter APIs are named and grouped rather than hidden inside neutral-looking contracts.
- Future extraction can focus on moving pure modules instead of untangling every component first.

Negative:

- Other UI frameworks still cannot consume a standalone Open GPUI headless package.
- `open_gpui_ui_core` still carries a small GPUI dependency surface through adaptive viewport
  pixels and `UiPx` style conversions.
- Crate users can still import GPUI adapter helpers from the compatibility root/prelude exports,
  so docs and tests must keep the adapter-only classification visible.

## Extraction Gate

Revisit `open-gpui-ui-headless` when all of the following are true:

1. At least two component families outside simple buttons share the same behavior helpers.
   Completed for overlay, roving focus, listbox/choice navigation, scroll viewport intent, and
   splitter constraints.
2. Public resolved-state types avoid GPUI runtime/rendering types and callback types. Completed for
   component `*State` contracts and guarded by
   `public_resolved_state_contracts_avoid_gpui_runtime_types`.
3. Public component geometry, metrics, and overlay state avoid GPUI geometry aliases. Completed for
   component public state and guarded by `public_contract_extraction_blockers_match_allowlist`.
4. UI-core public contracts either avoid GPUI geometry or classify it intentionally. Partially
   complete: the only current allowlisted blocker is adaptive viewport `Pixels as Px`.
5. GPUI conversion impls for neutral values are moved out of the future headless dependency path.
   Not complete: `UiPx` still has GPUI style-conversion impls in UI core for the current adapter
   stack.
6. Focus scope, dismissible layer ordering, and focus restoration have window-free tests. Partially
   complete: outside-press and focus-restore ordering are covered; full focus-trap traversal remains
   deferred until nested overlays require it.
7. Adapter-only public APIs are inventoried and kept out of resolved state. Completed for
   `TextInputController`, externally supplied `ScrollHandle`, `focus_ring_shadow`,
   `GpuiOverlayState`, and overlay scheduling helpers.

## Follow-Up Work

- Completed 2026-06-16: generic roving-focus helpers moved out of `tabs.rs` into
  `open_gpui_ui_components::roving_focus`; `Tabs` keeps compatibility re-exports while `Menu` and
  `RadioGroup` depend on the neutral module.
- Completed 2026-06-16: `ContextMenuState` now stores renderer-neutral `OverlayPlacementInput`;
  GPUI placement is resolved only inside the adapter/render boundary.
- Completed 2026-06-16: added window-free overlay stack ordering tests for outside press and focus
  restoration through `resolve_outside_press` and `resolve_focus_restore`.
- Completed 2026-06-17: neutral geometry values, public component metrics, focus/a11y facades,
  neutral `OverlayResolvedState`, and adapter-only API classification landed.
- Next: write a focused extraction design for a small behavior crate. It should first decide how to
  move or classify adaptive viewport `Pixels` and `UiPx` GPUI conversions, then extract only pure
  behavior modules.
- Keep `docs/ui/component-contract.md` and `docs/verification.md` current whenever a component
  state type adds new behavior metadata or a public adapter-only surface changes.
