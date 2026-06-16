# Open GPUI Component Contract

Official Open GPUI components use an adapter-first, headless-ready shape. A component may render
with GPUI today, but its behavior and semantic state should be extractable later without rewriting
the public API.

## Resolved State

Every component should expose a resolved state or descriptor type. The state type is the primary
unit for tests and documentation.

Resolved state should contain:

- semantic input state such as disabled, selected, checked, indeterminate, open, invalid, read-only,
  and required;
- activation or editability rules;
- navigation state for composite widgets such as selected, focused, and tab-stop position;
- accessibility intent such as role, label requirements, value presence, and required actions;
- metrics derived from `open_gpui_ui_core::Size`;
- token intents derived from `open_gpui_ui_core::ThemeTokens`;
- component anatomy metadata when it affects composition.

Resolved state should avoid GPUI render/runtime types such as `Window`, `App`, `Context`,
`RenderOnce`, `IntoElement`, `ElementId`, focus handles, scroll handles, and callback types.

## GPUI Adapter

The concrete component owns the GPUI adapter. This layer may use:

- `div()`, `ElementId`, `RenderOnce`, `IntoElement`, and fluent style calls;
- focus handles, tab stops, focus-visible styles, and focus restoration;
- AccessKit role/action/state mapping;
- hitboxes, pointer events, keyboard actions, cursor behavior, and event propagation;
- scroll handles, overlay anchoring, portals, and deferred rendering;
- concrete color values produced by `open_gpui_ui_components::ThemeResolver`.

The adapter should read from the resolved state rather than duplicating semantic decisions in the
render body.

## Overlay Behavior

Overlay component state should use renderer-neutral policy types from `open_gpui_ui_core` before
reaching GPUI adapters. The shared contract distinguishes:

- semantic `open` state from `present` and `interactive` overlay presence;
- layer kind (`Tooltip`, non-modal dismissible, `Modal`, and menu-like surfaces);
- outside-press policy (`ignore`, `consume`, `dismiss + consume`, and `dismiss + pass-through`);
- Escape-key policy and dismiss reason;
- initial focus and focus restoration intent;
- anchor and placement inputs that do not store `Window`, `Context`, `FocusHandle`, `ElementId`, or
  callback types.

GPUI adapters remain responsible for `deferred` and `anchored` rendering, event subscriptions,
hitboxes, focus handles, concrete focus restoration, and AccessKit relationship wiring.
`open_gpui_ui_components::overlay` provides the narrow GPUI mapping layer: deferred priority,
snap-to-window margin, GPUI anchor mapping, and open-change decisions derived from the shared
policy. It does not own global overlay ordering, callback storage, or window subscriptions.
`open_gpui_ui_core::overlay` owns renderer-neutral stack ordering through
`resolve_escape_key`, `resolve_outside_press`, and `resolve_focus_restore`, so nested overlay
behavior can be tested without a GPUI window before an adapter wires concrete events and focus
handles.

`TooltipState` is the first descriptive overlay component contract. It records content kind,
disabled/open state, hover/focus/manual open intent, placement preference, delay policy, resolved
metrics, token intents, and tooltip layer state. The first slice is intentionally non-interactive:
hover/focus subscriptions, focus handles, timing execution, and anchored/deferred rendering remain
GPUI adapter responsibilities. Rich hover cards and action-bearing tooltip content should not reuse
the descriptive tooltip contract as-is.

`PopoverState` is the first interactive non-modal overlay contract. It records controlled versus
uncontrolled open mode, default-open state, trigger expanded/selected intent, placement preference,
outside-press policy, initial focus intent, focus restore intent, resolved metrics, token intents,
and non-modal dismissible layer state. The GPUI adapter owns the concrete trigger/content elements,
`deferred`/`anchored` rendering, outside-press subscription, and focus handles. Nested popovers,
modal popover variants, and full focus-scope coordination remain follow-up work.

`DialogState` is the first modal overlay contract. It records controlled versus uncontrolled open
mode, default-open state, title and description metadata, Escape policy, outside-press policy,
initial focus intent, focus restore intent, resolved metrics, token intents, and modal layer state.
The GPUI adapter owns the barrier, concrete dialog surface, close callbacks, keyboard events,
deferred rendering, and focus handles. Alert dialogs, nested modal stacking, and full focus-trap
derivatives build on this contract; nested modal stacking and full focus-trap coordination remain
follow-up work.

`AlertDialogState` is the action-critical modal derivative. It records required title and
description text, cancel and primary action metadata, destructive intent, action disabled state,
initial focus preference, Escape policy, outside-press policy, focus restore intent, token intents,
and modal layer state. Alert dialogs default to consuming outside press without dismissing so the
underlay stays inert and critical decisions require an explicit action. The primary destructive
action is represented as metadata, while the cancel action remains the default initial focus target.
The GPUI adapter owns concrete button rendering, callbacks, keyboard handling, deferred rendering,
and focus handles.

`SheetState` is the edge-attached overlay contract. It records controlled versus uncontrolled open
mode, default-open state, attached side, modal versus non-modal mode, close affordance visibility,
title and optional description metadata, Escape policy, outside-press policy, initial focus intent,
focus restore intent, resolved metrics, token intents, and layer state. Modal sheets block underlay
input and default to dismissing while consuming outside press. Non-modal sheets use the same
surface anatomy while mapping to the non-modal dismissible layer kind and defaulting to
dismiss-and-pass-through outside behavior without installing a blocking barrier. The GPUI adapter
owns the barrier for modal sheets, edge positioning, concrete close control, callbacks, keyboard
handling, deferred rendering, and focus handles.

`MenuState` and `ContextMenuState` are the first menu overlay contracts. `MenuState` records
controlled versus uncontrolled open mode, action and separator items, disabled item state, roving
focus, activation payloads, Escape policy, outside-press policy, placement preference, resolved
metrics, token intents, and menu layer state. `ContextMenuState` reuses the same item and roving
focus model while adding a point anchor and renderer-neutral placement input. Submenus, menu bars,
typeahead,
checkbox/radio items, and application menu integration remain follow-up work.

## Focus Rings

Interactive component state should expose `FocusRing` metadata instead of rendering focus by
changing border width. `FocusRing` keeps the focus color as a `ColorIntent`, records the paint
width, and documents that it does not change layout.

The GPUI adapter should apply the ring inside `focus_visible` using
`open_gpui_ui_components::focus_ring_shadow`. This paints an outer box shadow, so keyboard focus
visibility does not resize or move the focused component.

## Public API

Prefer Rust builder-style APIs with explicit enums and semantic event names. Use names such as
`on_activate`, `on_change`, `on_open_change`, and `on_selection_change` when adding new events.
Device-specific names such as `on_click` are acceptable only when maintaining an existing unstable
bootstrap API.

Keep crate-root exports explicit. Do not use wildcard public re-exports in component crates.

## Theme Resolution

Component state should expose `ColorIntent` values rather than concrete GPUI colors. A color intent
keeps the semantic `TokenKey`, `ColorState`, and fallback RGB visible for tests, documentation, and
future headless extraction.

The GPUI adapter should resolve intents through `ThemeResolver` immediately before calling style
APIs such as `bg`, `border_color`, and `text_color`. `ThemeResolver::resolve` uses the default
light `ThemeSnapshot` for compatibility. New code that has an explicit theme should call
`ThemeResolver::resolve_with(intent, snapshot)` so `(TokenKey, ColorState)` lookups come from the
runtime theme table before falling back to the intent RGB.

`ThemeSnapshot` is an immutable table view with a `ThemeMode`, `revision`, and color entries. The
revision is the cache invalidation hook for future app-level theme providers. Components should not
read global theme state directly; keep the resolved component state renderer-neutral and pass theme
snapshots at the adapter edge.

## Accessibility References

Adapters may wire explicit AccessKit relationships such as controls, labelled-by, active descendant,
and popup references, but those references must point to nodes that are present in the current
accessibility tree update. GPUI defensively strips invalid cross-node references before handing the
tree update to the platform adapter, because AccessKit consumers may panic when explicit labels or
other references target missing nodes.

Component adapters should still prefer stable element IDs for both the referring node and referenced
node. The repair layer is a crash barrier, not a substitute for correct IDs.

## Toolbar Contract

`ToolbarState` describes renderer-neutral command grouping: stable toolbar label, orientation,
foundation size, disabled state, action/toggle/separator items, pressed toggle metadata, focused
item, tab stop, shared button metrics, and focus-ring/color intents. Separators are visual only and
must not participate in roving focus or activation.

The GPUI `Toolbar` adapter owns focus handles, keyboard/click dispatch, and concrete item rendering.
It should expose `Role::Toolbar`, `aria_orientation`, explicit item labels, button roles for action
and toggle items, and toggled metadata for pressed toggle items. It should reuse the shared
roving-focus helpers so arrow keys, Home, and End skip disabled items and separators consistently
with Tabs, RadioGroup, and Menu.

Toolbar v1 is a primitive command surface, not an application command registry. Automatic overflow
menus, shortcut rendering, command enablement policies, persisted customization, and icon asset
resolution remain app/adapter responsibilities until the command and sidebar work proves a common
contract.

## Sidebar Contract

`SidebarState` describes renderer-neutral shell navigation: side, variant, collapse mode,
effective collapsed state, accessible label, sections, flattened navigation items, disabled state,
selected item, focused item, tab stop, scrollability, metrics, colors, and focus-ring intent. It
keeps selection app-owned; activating an item produces a `SidebarSelection` payload but does not
own routing or persistent preferences.

Icon collapse keeps navigation items visible and focusable while hiding visible text; item labels
remain explicit accessibility labels. Offcanvas collapse removes items from roving focus by making
them invisible and non-focusable. `SidebarCollapseMode::None` ignores collapsed input and keeps the
expanded width. Disabled items are skipped by the shared vertical roving-focus helper and cannot
produce activation payloads.

The GPUI `Sidebar` adapter owns focus handles, click and keyboard dispatch, concrete rendering,
scroll handles through `ScrollArea`, and AccessKit mapping. It should expose `Role::Navigation` on
the container, `Role::Section` for groups, explicit item labels, selected and disabled metadata,
and set-position metadata for focusable items.

Sidebar v1 is a bounded navigation primitive, not a full application shell. Provider contexts,
mobile sheet routing, nested submenus, route integration, keyboard shortcut toggles, persisted
layout preferences, animated offcanvas unmounting, and command registry integration remain
follow-up work.

## Scroll Viewports

`ScrollAreaState` describes renderer-neutral viewport intent: stable viewport id, axis
(`vertical`, `horizontal`, or `both`), reset policy, optional reset key, foundation size, and
scrollbar metrics. It does not store `ScrollHandle`, current offset, child bounds, window bounds, or
event callbacks.

The GPUI `ScrollArea` adapter owns `ScrollHandle`, maps axis intent to GPUI overflow style, reserves
scrollbar width from the resolved metrics, and performs reset-on-key-change by mutating the concrete
scroll handle after the component has a keyed runtime. Layout shells should pass an externally owned
handle when another view needs to inspect or manipulate scroll state; resolved state remains the
testable contract for docs and future headless extraction.

The default `ScrollHandle` must live in the adapter's keyed runtime, not in the `ScrollArea::new`
builder value. Render code commonly reconstructs `RenderOnce` component values every frame, so a
handle allocated by the builder would reset the scroll offset on every notify/redraw and make the
viewport appear non-scrollable. An explicitly supplied external handle remains caller-owned, but the
default path must preserve offset across reconstructed component values.

## Splitter Constraints

`SplitterState` describes renderer-neutral resize constraints: stable group id, orientation, panel
fractions, per-panel min/max bounds, collapsible/collapsed metadata, handle adjacency, disabled
state, and handle metrics. The state owns the constraint solver for normalizing fractions and
clamping handle deltas; tests should exercise those rules without a GPUI window.

The GPUI `Splitter` adapter renders resolved panel fractions and resize handles from that state and
wires pointer dragging through keyed runtime state. Drag move events use the root splitter bounds to
translate pixels into fraction deltas, then feed those deltas through `SplitterState::resized_by`.
Dragging a collapsible panel past its restore threshold clears its collapsed state and resumes
normal min/max resizing; dragging below that threshold keeps the collapsed fraction stable.
The adapter may use GPUI layout primitives, cursor styles, drag callbacks, and `Entity` runtime
state, but it should not invent sizing rules in the render body. Keyboard splitter resizing,
controlled resize callbacks, application-level layout persistence, RTL behavior, and nested
splitter arbitration should build on `SplitterState::resized_by` instead of duplicating
min/max/collapse logic in adapter code.

## Gallery Conformance Surface

`examples/ui-foundation-gallery` is the durable conformance surface for official UI components. It
should expose stable sample ids, real resolved state, and a short gate list that names the
regression-prone behaviors each slice must keep covered.

The Components page should keep these gates visible:

- crate-root and prelude exports stay explicit;
- gallery samples continue to show real resolved state for each shipped component;
- ScrollArea redraws preserve the default keyed runtime handle;
- Splitter runtime fractions continue to share one constraint solver;
- Tabs keep overflow and roving-focus behavior visible in the page;
- icon-only affordances and labels keep their accessible metadata explicit.

## Current Known Gaps

The runtime theme table currently covers semantic component colors for light, dark, and
high-contrast snapshots, but there is not yet an app-level theme registry, user theme loading, or
JSON schema. Single-line editable text input now uses GPUI's `EntityInputHandler`/
`ElementInputHandler` path through `TextInputController`; richer editor behavior such as
multiline input, password masking, undo/redo, and completion remains out of scope. `Field` still
stays separate from the editing controller and remains composition-only. `focus_ring_shadow` is
GPUI-adapter code and should stay out of a future headless crate if `FocusRing` is extracted.
ADR 0006 keeps `open-gpui-ui-headless` deferred after the overlay checkpoint because several
resolved state types still expose GPUI geometry aliases and full focus-scope traversal remains
future work. Shared roving-focus helpers now live in
`open_gpui_ui_components::roving_focus`, with `Tabs` preserving compatibility re-exports.
`ContextMenuState` now stores renderer-neutral `OverlayPlacementInput`; GPUI placement is resolved
only inside the adapter/render boundary. Overlay stack Escape, outside-press, and focus-restore
ordering now have window-free tests in `open_gpui_ui_core`.
`Checkbox` now exposes checked, unchecked, and indeterminate resolved state plus theme intents for
the box, indicator, label, and focus ring. `Label` now exposes control-association metadata at the
resolved-state layer while keeping the visual adapter small. `Tabs` now keeps the roving-focus
contract in resolved state, with orientation, activation mode, selected/focused/tab-stop metadata,
while the GPUI adapter owns the focus handles and `aria` wiring. `RadioGroup` reuses the shared
roving-focus helpers, exposes group required/disabled metadata plus per-item
selected/focused/tab-stop state, and maps items through `Role::RadioButton` with `aria_selected`
because the current AccessKit surface exposed by GPUI does not provide a separate checked
property. `Toggle` is button-like: it exposes `pressed`, maps to `Role::Button` with
`aria_toggled`, and intentionally stays separate from `Checkbox` tri-state semantics. `Badge` is
display-only and exposes no role in resolved state. `IconButton` reuses Button visual variants and
focus-ring color intents, but requires an explicit accessible label because the visible icon glyph
is not a reliable accessible name. `Tooltip` is descriptive-only and currently maps its surface to
`Role::Label` until the public GPUI/AccessKit role wrapper exposes a tooltip role; trigger
association and timed hover/focus execution stay in the adapter layer. `Popover` currently covers
basic non-modal dismissible surfaces with default-open and controlled-open state; nested popover
coordination, modal popover barriers, and a full reusable focus-scope runtime remain deferred.
`ScrollArea` covers viewport overflow, axis metadata, scrollbar width metrics, and explicit
reset-on-key-change semantics. It intentionally does not yet expose custom scrollbar anatomy,
nested scroll arbitration, or Radix-style hover/auto scrollbar visibility.
`Splitter` covers panel fraction normalization, min/max constraints, collapsed-panel metadata,
stable handle anatomy, and local pointer dragging through keyed runtime state. Keyboard resizing,
controlled resize callbacks, persisted layouts, RTL behavior, and nested splitter arbitration
remain follow-up work.
