---
type: "Current State"
title: "Current Engineering State"
description: "Short durable summary of the active engineering state."
tags: ["engineering-memory"]
timestamp: 2026-06-16T22:53:21Z
status: "active"
---

# Current State

## 2026-06-17

- Done: Wrote the next extraction-prep plan at
  `docs/plans/2026-06-17-001-refactor-ui-headless-extraction-prep-plan.md`. The plan keeps
  `open-gpui-ui-headless` deferred and targets the blockers recorded by ADR 0006: public GPUI
  geometry aliases, direct GPUI focus/a11y re-exports, adapter-facing `GpuiOverlayState`, and
  ambiguous adapter-only APIs such as `TextInputController`, `ScrollHandle`, and `focus_ring_shadow`.
- Decision: The next implementation series should first strengthen the extraction guard inventory,
  then migrate neutral geometry/metrics, add focus and accessibility facades, split neutral overlay
  state from GPUI adapter scheduling, classify adapter-only APIs, and finally update ADR 0006 with
  the crate-extraction readiness decision.
- Done: Implemented U1 guard inventory. `open-gpui-ui-components` now has a companion extraction
  blocker allowlist for public `*State` and `*Metrics` contracts, while `open-gpui-ui-core` has a
  new `tests/headless_contracts.rs` guard for direct GPUI focus/a11y and geometry blockers.
- Last verified: `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-core`, `cargo check -p open-gpui-ui-components`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and `git diff --check`
  passed for U1.
- Done: Implemented U2 neutral geometry. `open-gpui-ui-core` now owns `UiPx`, `UiPoint`,
  `UiSize`, `UiRect`, and `UiEdges`; overlay `Rect`, `OverlaySize`, `OverlayEdges`, point anchors,
  offsets, and safe bounds now use those neutral values. `open-gpui-ui-components` converts at the
  GPUI adapter edge, and `ContextMenuState` exposes a neutral point anchor instead of
  `Point<Pixels>`.
- Last verified: U2 passed `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-foundation-gallery`.
- Done: Implemented U3 neutral component metrics. `Size` helpers and public component `*Metrics`
  now return `UiPx` instead of GPUI `Pixels`, gallery sizing samples use `UiPx`, and the component
  extraction-blocker allowlist shrank to the remaining `GpuiOverlayState` public-state blockers.
  `UiPx` has GPUI style-conversion impls in UI core as a transitional convenience for the current
  adapter-first crates; a later strict headless boundary should move that conversion out with the
  overlay/focus split.
- Last verified: U3 passed `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery`, and `git diff --check` with only existing CRLF warnings.
- Done: Implemented U4 focus and accessibility facades. `open-gpui-ui-core` now defines neutral
  `Role`, `Toggled`, `Orientation`, `AccessibleAction`, and `FocusTargetId` instead of
  re-exporting GPUI focus/a11y types. `open-gpui-ui-components::a11y` owns the GPUI/AccessKit
  adapter mapping, and render code applies neutral roles/states/actions through explicit
  `ui_*` adapter methods.
- Last verified: U4 passed `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core -p
  open-gpui-ui-components -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, `cargo nextest run -p
  open-gpui-ui-foundation-gallery`, and `git diff --check` with only existing CRLF warnings.
- Done: Implemented U5 neutral overlay state split. `open-gpui-ui-core` now owns
  `OverlayResolvedState`; Tooltip, Popover, Dialog, Menu, ContextMenu, AlertDialog, Sheet,
  HoverCard, Select, Combobox, and Command expose that neutral state publicly. GPUI deferred
  priority and snap margin remain in `GpuiOverlayState` and are derived at render/adapter sites.
- Last verified: U5 passed `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p
  open-gpui-ui-foundation-gallery`, `cargo check -p open-gpui-ui-core`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run
  -p open-gpui-ui-core`, `cargo nextest run -p open-gpui-ui-components`, and `cargo nextest run -p
  open-gpui-ui-foundation-gallery`.
- Next action: Commit U5, then start U6 by classifying GPUI-specific text input, scroll handle,
  focus-ring, and rendering helper APIs as adapter-only surfaces.

## 2026-06-16

- Done: Completed U8 of the UI shell, choice, and headless-readiness series by updating ADR 0006
  after Toolbar/Sidebar, ScrollArea/Splitter, Listbox/Select, Combobox, and Command landed. The
  decision remains to defer `open-gpui-ui-headless`: reusable behavior is now proven across
  overlay policy, roving focus, listbox navigation, scroll viewport intent, and splitter
  constraints, but extraction still needs neutral geometry/focus/a11y facades, an overlay-state
  split, and a decision on GPUI-backed text editing.
- Done: Added a component contract guard,
  `public_resolved_state_contracts_avoid_gpui_runtime_types`, that scans public resolved-state
  structs for GPUI runtime/rendering/callback leaks (`Window`, `App`, `Context`, `RenderOnce`,
  `IntoElement`, `ElementId`, `Entity`, focus handles, scroll handles, and `Rc<dyn` callback
  storage). `Pixels`/geometry aliases remain a documented blocker rather than a failing gate.
- Last verified: U8 reused the U7 final checks plus the new contract guard:
  `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, `cargo
  nextest run -p open-gpui-ui-foundation-gallery`, and `git diff --check`.
- Next action: Commit U8, then plan extraction-prep work rather than creating
  `open-gpui-ui-headless` immediately. The obvious next plan should split neutral geometry/a11y/
  focus vocabulary, rename/split `GpuiOverlayState`, and classify `TextInputController` as
  adapter-only or factor out a smaller neutral editing model.
- Done: Continued the UI shell, choice, and headless-readiness series with U7 Combobox and
  Command. Added `Combobox`, `ComboboxState`, grouped/standalone option descriptors, editable
  query metadata, selected value/label metadata that survives filtering, active option metadata,
  empty state, non-modal popup overlay policy, scroll viewport metadata, and explicit crate-root/
  prelude exports to `open_gpui_ui_components`.
- Done: Added `Command`, `CommandState`, command groups/items, shortcut metadata, loading metadata,
  empty state, optional dialog wrapper state, modal dialog overlay policy, and command selection
  payloads. Command v1 remains a local search/list surface and intentionally defers async loading,
  fuzzy ranking, multi-select chips, virtualized result sets, and global app command registration.
- Done: Added Components gallery Combobox and Command samples for filtered grouped options, empty
  search, disabled search, dialog-backed workspace commands, inline empty loading commands, and
  disabled commands. Gallery tests now assert combobox roles/filtering/empty state and command
  dialog/loading/shortcut contracts.
- Review: U7 review subagent `u7_review_fast` returned after the main U7 commit. It flagged a
  gallery coverage gap for combobox selection persistence, which was fixed in the gallery sample
  and tests. Its Escape-policy concern matched an already-policy-gated render path using
  `escape_open_change`, so a direct overlay-policy assertion was added instead of changing the
  close handler.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, `cargo
  nextest run -p open-gpui-ui-foundation-gallery`, and `git diff --check` passed during U7.
- Next action: Commit U7, then start U8 headless-readiness checkpoint. The checkpoint should audit
  public resolved-state types for GPUI runtime leaks and decide whether ADR 0006 needs an
  extraction-plan follow-up.
- Done: Continued the UI shell, choice, and headless-readiness series with U6 Listbox and Select.
  Added `Listbox`, `ListboxState`, grouped/standalone descriptors, option/separator anatomy,
  selected and active descendant metadata, disabled/separator skipping, typeahead target metadata,
  and explicit crate-root/prelude exports to `open_gpui_ui_components`.
- Done: Added `Select`, `SelectState`, controlled/uncontrolled open mode, placeholder and selected
  trigger label metadata, nested `ListboxState`, non-modal dismissible overlay policy, scroll
  viewport metadata, and keyed runtime state for open/selected/active behavior. Select v1 composes
  trigger + overlay + Listbox; searchable Combobox/Command behavior remains deferred.
- Done: Added Components gallery Listbox and Select samples for grouped choices, empty state,
  controlled-open long select, closed selected select, and disabled empty select. Gallery tests now
  assert choice roles, listbox navigation/activation/typeahead, Select overlay/focus/outside-press
  policy, and scrollable popup metadata.
- Last verified: `cargo fmt --all`, `cargo nextest run -p open-gpui-ui-components`, `cargo
  nextest run -p open-gpui-ui-foundation-gallery`, and `cargo check -p
  open-gpui-ui-foundation-gallery` passed during the U6 Listbox/Select slice.
- Next action: Finish U6 review/doc polish and commit, then start U7 Combobox and Command. Listbox
  real text event accumulation for typeahead, multi-select, option virtualization, richer
  active-descendant AccessKit references, and Select item-aligned positioning remain deferred.
- Done: Continued the UI shell, choice, and headless-readiness series with U5 HoverCard. Added
  `HoverCard`, `HoverCardState`, hover/focus/manual open intent, controlled/uncontrolled open
  mode, delay policy, interactive non-modal overlay metadata, token intents, metrics, and explicit
  crate-root/prelude exports to `open_gpui_ui_components`.
- Done: HoverCard uses a non-modal dismissible overlay contract instead of reusing descriptive
  Tooltip semantics. It defaults to no initial focus and no focus restoration, dismiss-and-pass-
  through outside behavior, open/close delay handling, and keyed runtime state for hover/focus
  lifetime so keyboard focus can move between trigger and content without immediately closing.
- Done: Added Overlay gallery HoverCard samples for default-open profile preview, focus-only
  preview, and manual controlled card behavior. Gallery tests cover interactive overlay metadata,
  manual/controlled policy, default delay, focus restore defaults, and placement intent.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, `cargo
  nextest run -p open-gpui-ui-foundation-gallery`, and `git diff --check` passed after the U5
  HoverCard slice.
- Next action: Commit U5 HoverCard, then start U6 Listbox and Select foundation.
  HoverCard safe pointer corridors, arrows, text-selection leases, richer focus-scope traversal,
  and role refinement beyond `Role::Window` remain deferred.
- Done: Continued the UI shell, choice, and headless-readiness series with U4 AlertDialog and
  Sheet. Added `AlertDialog`, `AlertDialogState`, action metadata, destructive intent, and `Sheet`,
  `SheetState`, side/modal/close-affordance enums to `open_gpui_ui_components`, keeping
  critical-action and edge-attached overlay semantics in resolved state while GPUI adapters own
  focus handles, callbacks, deferred rendering, barriers, and placement.
- Done: AlertDialog defaults to `Role::AlertDialog`, cancel-first initial focus metadata, trigger
  focus restore, modal underlay blocking, and outside-press consume-without-dismiss. Sheet models
  left/right/top/bottom attachment, modal versus non-modal overlay kind, close affordance
  visibility, and explicit outside-press policy instead of inheriting Dialog defaults.
- Done: Added Overlay gallery AlertDialog samples for destructive confirmation and safe cancel,
  plus Sheet samples for left modal, right non-modal, and bottom sticky behavior. Gallery tests now
  assert critical action contracts, sheet edge/policy contracts, and visible state metadata.
- Done: U4 subagent review found render-time state ownership drift, gallery sample contract drift,
  and hidden/disabled initial-focus target risks. The fixes preserve uncontrolled mode when runtime
  open changes, align controlled gallery samples with metadata, skip unavailable initial-focus
  targets, and defer overlay initial focus until after the layer is scheduled.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo nextest run -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, and `cargo nextest
  run -p open-gpui-ui-foundation-gallery` passed after the U4 review fixes.
- Next action: Commit U4, then start U5 HoverCard. AlertDialog/Sheet full focus trap traversal,
  labelled-by/described-by relationships, animation lifecycle, sheet resizing, and part-based
  composition APIs remain deferred.
- Done: Continued the UI shell, choice, and headless-readiness series with U3 Sidebar. Added
  `Sidebar`, `SidebarState`, `SidebarSection`, `SidebarItem`, descriptors, selection payloads,
  side/variant/collapse enums, metrics, colors, vertical roving-focus navigation, and explicit
  crate-root/prelude exports to `open_gpui_ui_components`.
- Done: Sidebar resolved state now models expanded, icon-collapsed, and offcanvas-collapsed
  behavior without GPUI runtime types. Icon collapse hides visible text but keeps item labels and
  focusability; offcanvas collapse removes items from roving focus; disabled items are skipped and
  cannot produce activation payloads.
- Done: Added Components gallery Sidebar samples for expanded workspace navigation, icon rail, and
  a long scrollable reports navigation, plus gallery tests for navigation metadata, roles,
  collapse behavior, disabled skip behavior, and scrollability.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the U3 Sidebar slice.
- Next action: Commit U3 after review, then start U4 AlertDialog and Sheet. Sidebar provider
  contexts, mobile sheet routing, nested submenus, route integration, persisted layout preferences,
  animation lifecycle, shortcuts, and command registry integration remain deferred.
- Done: Continued the UI shell, choice, and headless-readiness series with U2 Toolbar. Added
  `Toolbar`, `ToolbarState`, `ToolbarItem`, `ToolbarItemDescriptor`, `ToolbarItemState`, and
  `ToolbarSelection` to `open_gpui_ui_components`, keeping command grouping, item kind, disabled
  state, pressed state, tab-stop state, metrics, colors, and roving-focus decisions in resolved
  state while the GPUI adapter owns focus handles, events, and rendering.
- Done: Added horizontal and vertical Toolbar samples to the Components gallery, including action,
  toggle, separator, disabled, focused, and pressed item states. Gallery tests now assert the
  Toolbar metadata and roving-focus contract, and component tests assert disabled/separator skip
  behavior plus keyboard activation payloads.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, `cargo nextest
  run -p open-gpui-ui-foundation-gallery`, and `git diff --check` passed after the U2 Toolbar
  slice.
- Next action: Commit U2, then start U3 Sidebar on the same resolved-state plus GPUI-adapter
  boundary. Toolbar overflow behavior, shortcut display, app command registry integration,
  customization, and icon asset resolution remain deferred.
- Done: Started the UI shell, choice, and headless-readiness series with
  `docs/plans/2026-06-16-002-feat-ui-shell-choice-headless-series-plan.md`. U1 adds a visible
  Components conformance gate surface for explicit crate/prelude exports, gallery metadata,
  ScrollArea redraw persistence, Splitter runtime constraints, Tabs overflow/roving focus, and
  explicit accessible labels.
- Done: The Components gallery now renders the conformance gates, and gallery tests assert stable
  gate ids/evidence alongside the existing component metadata samples. `open_gpui_ui_components`
  also has an isolated crate-root/prelude export smoke test so deleting a public re-export fails
  the intended surface.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, `cargo nextest
  run -p open-gpui-ui-foundation-gallery`, and `git diff --check` passed after the U1 conformance
  gate slice.
- Next action: Start U2 Toolbar on the existing resolved-state plus GPUI-adapter boundary.
- Done: Completed the U11 Splitter half by adding `Splitter` to `open_gpui_ui_components`.
  `SplitterState` now records group id, orientation, panel fractions, min/max constraints,
  collapsible/collapsed metadata, handle adjacency, disabled state, and metrics. It also owns
  `resized_by` so min/max delta clamping is testable without a GPUI window.
- Done: Added Components gallery Splitter samples for horizontal workspace panes and a vertical
  collapsed/details stack, plus gallery metadata tests for panel/handle state. The concrete adapter
  now renders resolved fractions and handle affordances and wires local pointer dragging through a
  keyed runtime. Drag move events are handled on the root splitter so pixel deltas are measured
  against the full splitter bounds, then fed through `SplitterState::resized_by`; drag payloads carry
  the group id to avoid multi-splitter cross-talk.
- Done: Added `SplitterState::with_panel_fractions` so live runtime fractions reuse the same
  normalization and min/max constraint path as descriptor-based state. Keyboard resizing,
  controlled resize callbacks, persisted layouts, RTL behavior, and nested splitter arbitration
  remain deferred.
- Done: Fixed the vertical collapsed Splitter drag path: dragging a collapsed collapsible panel
  below its restore threshold keeps the collapsed fraction stable; dragging far enough clears
  `collapsed` and resumes normal min/max resizing. This fixes the gallery's vertical
  `details-split` sample, whose top panel starts collapsed.
- Done: Fixed `ScrollArea` appearing non-scrollable in the Components gallery. The default
  `ScrollHandle` now lives in `ScrollAreaRuntime` keyed by the viewport element id instead of being
  allocated inside each `ScrollArea::new` builder value, so wheel scrolling survives the redraw that
  the scroll event itself triggers. Externally supplied handles remain supported for callers that
  need to inspect or manipulate offset directly.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and
  `cargo nextest run -p open-gpui-ui-foundation-gallery` passed after the first Splitter slice.
  `cargo check -p open-gpui-ui-components` and `cargo nextest run -p open-gpui-ui-components`
  passed again after the pointer-drag runtime. `cargo check -p open-gpui-ui-foundation-gallery`
  also passed after the collapsed-panel restore fix. `cargo fmt -p open-gpui-ui-components`,
  `cargo check -p open-gpui-ui-components`, `cargo nextest run -p open-gpui-ui-components`, and
  `cargo check -p open-gpui-ui-foundation-gallery` passed after the ScrollArea runtime-handle fix.
- Next action: Run a manual Components-gallery dogfood pass, then move to U12 Toolbar/Sidebar or
  the next gallery conformance item.
- Done: Started the layout/shell-navigation component series by adding `ScrollArea` to
  `open_gpui_ui_components`. `ScrollAreaState` records stable viewport id, axis, reset policy/key,
  size, and scrollbar metrics without storing GPUI handles; the concrete adapter owns
  `ScrollHandle`, GPUI overflow styles, scrollbar width, and reset-on-key-change offset mutation.
- Done: Added Components gallery ScrollArea samples for vertical, horizontal, and two-axis overflow,
  plus gallery metadata coverage for axis/reset/metrics. Also repaired the gallery ContextMenu test
  to assert renderer-neutral `OverlayPlacementInput` fields after the prior placement extraction.
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and
  `cargo nextest run -p open-gpui-ui-foundation-gallery` passed after the ScrollArea slice.
- Next action: Continue the layout/shell-navigation series with Splitter/ResizablePanel primitives
  or Toolbar, using the same resolved-state plus GPUI-adapter boundary. For ScrollArea, custom
  scrollbar anatomy, hover/auto visibility, nested scroll routing, and wheel arbitration remain
  deferred until the base viewport is dogfooded.
- Done: Finished the ADR 0006 stack-ordering follow-up by adding window-free overlay stack ordering
  primitives in `open_gpui_ui_core::overlay`: `resolve_outside_press` and
  `resolve_focus_restore`, plus tests for topmost dismissible-layer handling and focus restoration.
- Last verified: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-core`,
  `cargo nextest run -p open-gpui-ui-core`, `cargo check -p open-gpui-ui-components`, and
  `cargo check -p open-gpui-ui-foundation-gallery` passed after the overlay stack resolver work.
- Next action: Start the next official component roadmap item after ADR 0006; likely candidates are
  ScrollArea/Toolbar/Sidebar, or a focused geometry-alias cleanup if the headless boundary should
  be tightened further first. Full focus-trap/scope traversal remains deferred until nested overlay
  components need it.
- Done: Continued ADR 0006 follow-up by moving `ContextMenuState` to renderer-neutral
  `OverlayPlacementInput` instead of storing resolved `GpuiOverlayPlacement`. The GPUI placement
  is now derived only at the adapter/render boundary.
- Last verified: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-components`,
  `cargo nextest run -p open-gpui-ui-components`, and `cargo check -p
  open-gpui-ui-foundation-gallery` passed for the context-menu placement extraction slice.
- Next action: Keep removing remaining GPUI geometry leaks from resolved state, then add
  window-free focus-scope and dismissible-layer ordering tests before reconsidering a headless
  crate.
- Done: Started ADR 0006 follow-up by moving shared roving-focus helpers out of `tabs.rs` into
  `open_gpui_ui_components::roving_focus`. `Tabs` preserves compatibility re-exports for the old
  helper paths, while `Menu` and `RadioGroup` now depend on the neutral behavior module.
- Last verified: `cargo fmt --all --check`, `cargo check -p open-gpui-ui-components`,
  `cargo nextest run -p open-gpui-ui-components`, and `cargo check -p
  open-gpui-ui-foundation-gallery` passed for this roving-focus extraction slice.
- Next action: Separate renderer-neutral menu/context-menu placement input from
  `GpuiOverlayPlacement`, then add window-free focus-scope and dismissible-layer ordering tests
  before reconsidering a headless crate.
- Done: Completed U8 of the overlay component series by adding ADR 0006
  (`docs/adr/0006-open-gpui-ui-headless-extraction-checkpoint.md`). The checkpoint keeps
  `open-gpui-ui-headless` deferred: overlay components now prove repeated behavior contracts, but
  several state types still expose GPUI geometry or adapter placement state, so extraction would
  freeze the wrong boundary.
- Done: Updated `docs/ui/component-contract.md`, `docs/verification.md`, and engineering memory so
  the overlay family documents which behavior is renderer-neutral, which remains GPUI adapter
  responsibility, and what gate must be met before a future headless crate.
- Last verified: Final overlay-series quality pass ran `cargo fmt --all`, `cargo check -p
  open-gpui-ui-components`, `cargo check -p open-gpui-ui-foundation-gallery`, `cargo nextest run -p
  open-gpui-ui-components`, and `cargo nextest run -p open-gpui-ui-foundation-gallery`.
- Next action: Start the next official component roadmap slice from ADR 0006 follow-ups: neutralize
  shared roving-focus helpers and remove GPUI placement leaks before reconsidering a headless crate.
- Done: Completed U7 of the overlay component series by adding `Menu` and `ContextMenu` to
  `open-gpui-ui-components` with shared menu item descriptors, action/separator items, disabled
  item state, roving-focus navigation, keyboard activation payloads, Escape/outside policies,
  trigger-anchored menu placement, point-anchored context-menu placement, exports, and tests.
- Done: Added Overlay gallery Menu samples for default-open, controlled, outside-ignored, and
  disabled cases, plus ContextMenu samples for point-anchor, controlled, and default-open cases.
  Gallery tests now cover menu roving-focus contracts and context-menu point-anchor placement.
- Last verified: `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the U7 Menu/ContextMenu work.
- Next action: Start U8 overlay examples checkpoint and headless-readiness review, then run the
  final quality pass for the full overlay component series.
- Done: Completed U6 of the overlay component series by adding modal `Dialog` to
  `open-gpui-ui-components` with `DialogState`, controlled/uncontrolled open mode, default-open
  state, title/description metadata, Escape policy, outside-press policy, initial focus and
  focus-restore intent, modal layer state, token/metric resolution, exports, and targeted tests.
- Done: Added Overlay gallery Dialog samples for controlled modal, default-open modal,
  outside-ignored modal, and disabled trigger. The controlled sample is owned by gallery shell state;
  Escape and the modal barrier can close it, while open modal layers block underlay input.
- Last verified: `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the U6 Dialog work.
- Next action: Start U7 Menu/ContextMenu on top of the shared overlay policy and Dialog/Popover
  precedent, covering item roles, selection/disabled state, keyboard/Escape behavior, and context
  trigger positioning.
- Done: Completed U5 of the overlay component series by adding interactive non-modal `Popover` to
  `open-gpui-ui-components` with `PopoverState`, controlled/uncontrolled open mode, default-open
  state, trigger expanded/selected intent, outside-press policy, placement metadata, initial focus
  and focus-restore intent, token/metric resolution, exports, and targeted tests.
- Done: Added Overlay gallery Popover samples for default-open, controlled, consuming outside
  press, and disabled cases. The controlled sample is owned by gallery shell state and closes on
  Escape via the shared shell handler.
- Last verified: `cargo check -p open-gpui-ui-components`, `cargo check -p
  open-gpui-ui-foundation-gallery`, `cargo nextest run -p open-gpui-ui-components`, and `cargo
  nextest run -p open-gpui-ui-foundation-gallery` passed after the U5 Popover work.
- Next action: Start U6 Dialog on top of the shared overlay adapter and Popover precedent, covering
  modal layer state, title/description metadata, Escape/outside policies, and focus restoration.
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
