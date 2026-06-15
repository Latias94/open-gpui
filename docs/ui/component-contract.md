# Open GPUI Component Contract

Official Open GPUI components use an adapter-first, headless-ready shape. A component may render
with GPUI today, but its behavior and semantic state should be extractable later without rewriting
the public API.

## Resolved State

Every component should expose a resolved state or descriptor type. The state type is the primary
unit for tests and documentation.

Resolved state should contain:

- semantic input state such as disabled, selected, checked, open, invalid, read-only, and required;
- activation or editability rules;
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
- concrete token-to-color fallback resolution until the theme resolver exists.

The adapter should read from the resolved state rather than duplicating semantic decisions in the
render body.

## Public API

Prefer Rust builder-style APIs with explicit enums and semantic event names. Use names such as
`on_activate`, `on_change`, `on_open_change`, and `on_selection_change` when adding new events.
Device-specific names such as `on_click` are acceptable only when maintaining an existing unstable
bootstrap API.

Keep crate-root exports explicit. Do not use wildcard public re-exports in component crates.

## Current Known Gaps

The first component slices still rely on fallback RGB values because the theme resolver is not
implemented yet. Focus rings use the current GPUI focus-visible style path and may need replacement
when a no-layout-shift focus primitive lands. Rich text input editing must use GPUI's
`EntityInputHandler`/`ElementInputHandler` path and is intentionally separate from display-level
field composition.
