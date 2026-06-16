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

## Current Known Gaps

The runtime theme table currently covers semantic component colors for light, dark, and
high-contrast snapshots, but there is not yet an app-level theme registry, user theme loading, or
JSON schema. Single-line editable text input now uses GPUI's `EntityInputHandler`/
`ElementInputHandler` path through `TextInputController`; richer editor behavior such as
multiline input, password masking, undo/redo, and completion remains out of scope. `Field` still
stays separate from the editing controller and remains composition-only. `focus_ring_shadow` is
GPUI-adapter code and should stay out of a future headless crate if `FocusRing` is extracted.
`Checkbox` now exposes checked, unchecked, and indeterminate resolved state plus theme intents for
the box, indicator, label, and focus ring. `Label` now exposes control-association metadata at the
resolved-state layer while keeping the visual adapter small. `Tabs` now keeps the roving-focus
contract in resolved state, with orientation, activation mode, selected/focused/tab-stop metadata,
while the GPUI adapter owns the focus handles and `aria` wiring. `RadioGroup` reuses the same
roving-focus helpers as `Tabs`, exposes group required/disabled metadata plus per-item
selected/focused/tab-stop state, and maps items through `Role::RadioButton` with `aria_selected`
because the current AccessKit surface exposed by GPUI does not provide a separate checked
property. `Toggle` is button-like: it exposes `pressed`, maps to `Role::Button` with
`aria_toggled`, and intentionally stays separate from `Checkbox` tri-state semantics. `Badge` is
display-only and exposes no role in resolved state. `IconButton` reuses Button visual variants and
focus-ring color intents, but requires an explicit accessible label because the visible icon glyph
is not a reliable accessible name.
