# ADR 0006: Open GPUI UI Headless Extraction Checkpoint

**Status**: Accepted
**Date**: 2026-06-17

## Context

ADR 0005 chose an adapter-first, headless-ready component architecture and deferred a standalone
`open-gpui-ui-headless` crate until repeated behavior contracts existed and the public boundary was
clean enough to move.

The current component stack now has that boundary evidence:

- `open_gpui_ui_core` has no `open_gpui` manifest dependency, no source references to
  `open_gpui`, and no GPUI style conversion impls for neutral geometry.
- `open_gpui_ui_core::overlay` owns renderer-neutral overlay policy, presence, dismissal,
  focus-intent, stack ordering, placement vocabulary, and `OverlayResolvedState`.
- `open_gpui_ui_core` owns neutral geometry, accessibility, and focus facades:
  `UiPx`, `UiPoint`, `UiSize`, `UiRect`, `UiEdges`, `Role`, `Toggled`, `Orientation`,
  `AccessibleAction`, and `FocusTargetId`.
- `TooltipState`, `PopoverState`, `DialogState`, `MenuState`, `ContextMenuState`,
  `AlertDialogState`, `SheetState`, `HoverCardState`, `SelectState`, `ComboboxState`, and
  `CommandState` expose neutral overlay state instead of `GpuiOverlayState`.
- `ToolbarState`, `SidebarState`, `TabsState`, `RadioGroupState`, `MenuState`, and `ListboxState`
  reuse roving-focus and disabled-skip helper vocabulary.
- `ListboxState`, `SelectState`, `ComboboxState`, and `CommandState` share grouped item anatomy,
  selected versus active value semantics, disabled option handling, empty states, and scroll
  viewport metadata.
- `ScrollAreaState` and `SplitterState` prove that runtime-sensitive GPUI handles and pointer
  interaction can stay in adapters while viewport intent and resize constraints remain testable.
- Component tests guard public resolved-state structs against GPUI runtime/rendering types such as
  `Window`, `App`, `Context`, `RenderOnce`, `IntoElement`, `ElementId`, `Entity`, focus handles,
  scroll handles, and callbacks.
- Adapter-only public surfaces are explicitly inventoried: `TextInputController`, externally
  supplied `ScrollHandle`, `focus_ring_shadow`, `GpuiOverlayState`, GPUI geometry conversion
  helpers, and GPUI overlay scheduling helpers.

This is enough evidence to start a focused behavior-extraction design, but not enough to create or
publish a new crate in the current branch.

## Decision

Do **not** create `open-gpui-ui-headless` yet.

The strict UI-core boundary is clean, so the old core-boundary blockers are no longer active. The
remaining decision is sequencing: the next change should be a narrow extraction plan that moves one
behavior family at a time, keeps GPUI adapter APIs behind `open_gpui_ui_components::gpui_adapter`,
and proves each move with existing tests before any package is published.

ADR 0007 records that design gate. It identifies the first extraction candidates and the adapter
surfaces that must not move.

Likely first extraction candidates:

- overlay policy, presence, dismissal, stack ordering, focus-intent, and placement vocabulary;
- roving-focus navigation helpers;
- listbox collection navigation, selected/active item resolution, disabled skip behavior, and
  typeahead target helpers;
- scroll viewport intent;
- splitter resize constraint solvers.

Do not extract:

- GPUI `anchored`/`deferred` adapter helpers;
- concrete `div()` render trees;
- focus handle allocation and concrete focus movement;
- outside-press event subscriptions;
- barrier rendering and hit-test blocking;
- theme resolution into GPUI colors;
- AccessKit relationship wiring that depends on concrete element IDs;
- `ScrollHandle`;
- `TextInputController`;
- `focus_ring_shadow`;
- GPUI geometry conversion helpers.

## Rationale

The current component catalog has enough reusable behavior to justify extraction planning. The
important shift since the original checkpoint is that public component state and UI-core contracts
are now renderer-neutral: geometry, metrics, focus/a11y semantics, overlay state, adaptive policy,
and strict core dependencies no longer require GPUI runtime types.

Creating a crate in this same series would still mix two different changes: boundary cleanup and
module relocation. Keeping the crate deferred lets reviewers evaluate the next move by behavior
family, not by a broad package split.

## Consequences

Positive:

- Public component resolved state is guarded against GPUI runtime/render leaks.
- UI core is now a clean dependency for future behavior crates or non-GPUI adapters.
- GPUI adapter APIs are named and grouped rather than hidden inside neutral-looking contracts.
- Future extraction can focus on moving pure modules instead of untangling every component first.

Negative:

- Other UI frameworks still cannot consume a standalone Open GPUI behavior package.
- Crate users can still import GPUI adapter helpers from compatibility root/prelude exports, so
  docs and tests must keep the adapter-only classification visible.
- The next extraction plan must be careful not to move adapter-owned runtime responsibilities along
  with neutral policy/state.

## Extraction Gate

Revisit crate creation when all of the following are true:

1. At least two component families outside simple buttons share the same behavior helpers.
   Completed for overlay, roving focus, listbox/choice navigation, scroll viewport intent, and
   splitter constraints.
2. Public resolved-state types avoid GPUI runtime/rendering types and callback types. Completed for
   component `*State` contracts and guarded by
   `public_resolved_state_contracts_avoid_gpui_runtime_types`.
3. Public component geometry, metrics, and overlay state avoid GPUI geometry aliases. Completed for
   component public state and guarded by `public_contract_extraction_blockers_match_allowlist`.
4. UI-core public contracts and package metadata avoid GPUI dependencies. Completed and guarded by
   `ui_core_strict_boundary_blockers_match_allowlist`.
5. GPUI conversions for neutral values live in the adapter layer. Completed through
   `open_gpui_ui_components::gpui_adapter`.
6. Focus scope, dismissible layer ordering, and focus restoration have window-free tests. Partially
   complete: outside-press and focus-restore ordering are covered; full focus-trap traversal remains
   deferred until nested overlays require it.
7. Adapter-only public APIs are inventoried and kept out of resolved state. Completed for
   `TextInputController`, externally supplied `ScrollHandle`, `focus_ring_shadow`,
   `GpuiOverlayState`, GPUI geometry conversion helpers, and overlay scheduling helpers.

## Follow-Up Work

- Completed 2026-06-16: generic roving-focus helpers moved out of `tabs.rs` into
  `open_gpui_ui_components::roving_focus`; `Tabs` keeps compatibility re-exports while `Menu` and
  `RadioGroup` depend on the neutral module.
- Completed 2026-06-16: `ContextMenuState` now stores renderer-neutral `OverlayPlacementInput`;
  GPUI placement is resolved only inside the adapter/render boundary.
- Completed 2026-06-16: added window-free overlay stack ordering tests for outside press and focus
  restoration through `resolve_outside_press` and `resolve_focus_restore`.
- Completed 2026-06-17: neutral geometry values, public component metrics, focus/a11y facades,
  neutral `OverlayResolvedState`, adapter-only API classification, neutral adaptive policy, and
  adapter-owned GPUI geometry conversions landed.
- Completed 2026-06-17: `open_gpui_ui_core` dropped its `open_gpui` dependency and the strict
  boundary blocker inventory became empty.
- Next: implement the ADR 0007 extraction design with a narrow plan that moves one behavior family
  at a time and leaves GPUI adapter APIs behind.
- Keep `docs/ui/component-contract.md` and `docs/verification.md` current whenever a component
  state type adds new behavior metadata or a public adapter-only surface changes.
