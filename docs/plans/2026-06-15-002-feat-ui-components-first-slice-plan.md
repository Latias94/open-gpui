---
title: "Open GPUI UI Components First Slice"
date: 2026-06-15
status: planned
execution: code
branch: feat/open-gpui-ui-core
depends_on:
  - docs/adr/0004-open-gpui-component-library-strategy.md
  - docs/plans/2026-06-15-001-feat-ui-foundation-gallery-plan.md
---

# Open GPUI UI Components First Slice

## Goal

Create the first real component consumer crate on top of `open-gpui-ui-core`.
The slice should prove that foundation vocabulary can drive concrete controls without moving
styled component behavior back into the foundation crate.

## References

- `repo-ref/gpui-component/crates/ui/src/button/button.rs`
- `repo-ref/gpui-component/crates/ui/src/switch.rs`
- `F:/SourceCodes/Rust/fret/ecosystem/fret-ui-shadcn/src/button.rs`
- `F:/SourceCodes/Rust/fret/ecosystem/fret-ui-shadcn/src/switch.rs`
- `F:/SourceCodes/Rust/fret/ecosystem/fret-ui-kit/src/imui/options/controls/button_image/button.rs`
- `F:/SourceCodes/Rust/fret/ecosystem/fret-ui-kit/src/imui/options/controls/boolean/switch.rs`
- `examples/ui-foundation-gallery/src/shell.rs`

## Scope Boundaries

- Do not clone the full `gpui-component` or `fret-ui-shadcn` component surface.
- Do not add `TextInput` / `Field` in this slice.
- Do not make `open-gpui-ui-core` render components.
- Do not add canvas, docking, or app-shell dependencies to the components crate.
- Do not introduce a theme runtime or global token resolver yet; use stable style defaults derived
  from the existing token and sizing vocabulary.

## Architecture

Add `crates/ui_components` as `open-gpui-ui-components`.

Dependencies:

- `open_gpui`
- `open_gpui_ui_core`

The crate owns styled, concrete GPUI elements. It should export:

- `Button`
- `ButtonVariant`
- `ButtonState`
- `Switch`
- `SwitchState`
- a `prelude` module for consumer imports

The first API should be intentionally small:

- Builder-style configuration.
- Explicit `id`, `label`, `size`, `variant`, `disabled`, and `selected` / `checked` state.
- `Role::Button` and `Role::Switch` semantics.
- Focus-visible styling.
- Size metrics from `open_gpui_ui_core::Size`.
- Color choices derived from `open_gpui_ui_core::ThemeTokens` names and temporary default RGB
  values until a real token resolver exists.

## Implementation Units

### U1: Scaffold `open-gpui-ui-components`

Goal: Add a new workspace crate that depends only on GPUI and `ui_core`.

Files:

- Create `crates/ui_components/Cargo.toml`
- Create `crates/ui_components/src/lib.rs`
- Create `crates/ui_components/src/prelude.rs`
- Modify `Cargo.toml`

Patterns to follow:

- `crates/ui_core/Cargo.toml`
- `examples/ui-foundation-gallery/Cargo.toml`

Test scenarios:

- `cargo check -p open-gpui-ui-components` succeeds.
- The root workspace dependency table exposes `open_gpui_ui_components`.

Verification:

- `cargo check -p open-gpui-ui-components`

### U2: Implement `Button`

Goal: Provide a first concrete button element consuming foundation size, token, focus, and a11y
vocabulary.

Files:

- Create `crates/ui_components/src/button.rs`
- Modify `crates/ui_components/src/lib.rs`
- Modify `crates/ui_components/src/prelude.rs`
- Create or update `crates/ui_components/tests/components.rs`

Patterns to follow:

- `repo-ref/gpui-component/crates/ui/src/button/button.rs`
- `F:/SourceCodes/Rust/fret/ecosystem/fret-ui-shadcn/src/button.rs`
- `examples/ui-foundation-gallery/src/shell.rs`

Test scenarios:

- Default button style state uses `Role::Button`, medium size metrics, and default variant.
- Destructive variant uses destructive token intent.
- Disabled state blocks activation metadata and has stable disabled styling inputs.
- Size helpers from `Sizable` apply the requested foundation size.

Verification:

- `cargo nextest run -p open-gpui-ui-components`

### U3: Implement `Switch`

Goal: Provide a first concrete switch element consuming foundation size, token, focus, and a11y
vocabulary.

Files:

- Create `crates/ui_components/src/switch.rs`
- Modify `crates/ui_components/src/lib.rs`
- Modify `crates/ui_components/src/prelude.rs`
- Update `crates/ui_components/tests/components.rs`

Patterns to follow:

- `repo-ref/gpui-component/crates/ui/src/switch.rs`
- `F:/SourceCodes/Rust/fret/ecosystem/fret-ui-shadcn/src/switch.rs`
- `F:/SourceCodes/Rust/fret/ecosystem/fret-ui-kit/src/imui/options/controls/boolean/switch.rs`

Test scenarios:

- Checked switch maps to `Toggled::True`.
- Unchecked switch maps to `Toggled::False`.
- Disabled switch keeps the a11y role but blocks activation metadata.
- Size metrics produce deterministic track and thumb values.

Verification:

- `cargo nextest run -p open-gpui-ui-components`

### U4: Add Component Gallery Coverage

Goal: Let the existing UI foundation gallery dogfood the new components crate without turning it
into a broad component catalog.

Files:

- Modify `examples/ui-foundation-gallery/Cargo.toml`
- Modify `examples/ui-foundation-gallery/src/pages/mod.rs`
- Modify `examples/ui-foundation-gallery/src/shell.rs`
- Update `examples/ui-foundation-gallery/tests/foundation_gallery.rs`
- Modify `docs/verification.md`

Patterns to follow:

- Current gallery section registration and scroll container behavior.

Test scenarios:

- The gallery section list includes a `components` page.
- The gallery manifest depends on `open_gpui_ui_components`.
- Component samples expose Button and Switch metadata.

Verification:

- `cargo check -p open-gpui-ui-foundation-gallery`
- `cargo nextest run -p open-gpui-ui-foundation-gallery`

## Verification

Run:

```sh
cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery
cargo check -p open-gpui-ui-components
cargo check -p open-gpui-ui-foundation-gallery
cargo nextest run -p open-gpui-ui-components
cargo nextest run -p open-gpui-ui-foundation-gallery
```

Manual dogfood:

```sh
cargo run -p open-gpui-ui-foundation-gallery
```

Open the new Components page and confirm Button and Switch samples render, focus, and fit inside
the existing scrollable content area.
