# open-gpui-command Ecosystem Plan

## Goal

Create the first reusable command ecosystem layer for Open GPUI without replacing GPUI's existing
`Action`, `Keymap`, `Keystroke`, or window dispatch systems.

The external seam should let apps and plugin-like modules register command metadata once, then
project it into:

- a searchable command palette index,
- shortcut display metadata,
- menu/context-menu descriptors,
- and eventually GPUI action dispatch.

## Source Context

- GPUI already owns the low-level action registry, JSON action construction, keymap matching,
  context predicates, keystroke parsing, and pending multi-keystroke matching.
- `open_gpui_ui_core::CommandDescriptor` is the current renderer-neutral command metadata record.
- `open_gpui_ui_components::Command` already owns palette rendering, local ranking, grouped command
  state, multi-select state, dialog presentation, and `CommandIndexSnapshot` projection.
- Zed's command palette proves that the useful product seam is around available actions, filtering,
  ranking, shortcut projection, history, and dispatch, not a second keymap engine.
- cmdk is useful for stable values, grouped item semantics, controlled search/selection, and
  ranking expectations. Its DOM-authoritative compound component model should not be copied into
  native Rust.

## Non-Goals

- Do not replace `open_gpui::Action`.
- Do not replace `open_gpui::Keymap`.
- Do not implement Vim mode or editor modal state in this slice.
- Do not introduce persistence/history in the first slice.
- Do not move the concrete `Command` component out of `open-gpui-ui-components` yet.
- Do not create a global singleton registry that is required for all apps.

## Proposed Module Shape

First place the reusable model in `open-gpui-ui-core` so the seam is proven before splitting a new
crate:

```text
crates/ui_core/src/command.rs
  CommandDescriptor          // existing metadata record
  CommandContribution        // one registered command plus grouping/source metadata
  CommandRegistry            // deterministic collection of contributions
  CommandRegistrySnapshot    // immutable projection result
  CommandRegistryError       // duplicate id diagnostics
```

Later, when GPUI action dispatch adapters are added, extract this model plus adapters into:

```text
crates/open-gpui-command/
  registry
  keymap_projection
  palette_projection
  dispatch_adapter
```

## U1: Registry and Snapshot Projection

Add a deterministic command registry in `open-gpui-ui-core`:

- Register `CommandDescriptor` values with optional source metadata.
- Reject duplicate stable ids.
- Preserve insertion order for equal-ranked projections.
- Project directly to the shape needed by `CommandIndexSnapshot` without depending on
  `open-gpui-ui-components`.
- Keep disabled and `when` as caller-owned facts; do not evaluate context expressions in U1.

Acceptance:

- Unit tests prove duplicate-id rejection, deterministic order, grouped descriptors, and disabled /
  shortcut / keyword preservation.
- `open-gpui-ui-components` can consume the registry snapshot via existing
  `CommandIndexSnapshot::command_descriptors`.

## U2: Component Projection Helpers

Add `CommandIndexSnapshot` convenience constructors in `open-gpui-ui-components`:

- Build a palette snapshot from a `CommandRegistrySnapshot`.
- Preserve group labels and source order.
- Keep ranking in the existing `CommandState` path.

Acceptance:

- Focused command tests show registry-backed command palettes rank/search/activate the same as
  manually supplied command descriptors.

## U3: Keymap Shortcut Projection

Add a GPUI adapter that maps registered command ids to display shortcut labels:

- Use existing `App::key_bindings` / `Keymap::bindings_for_action` capabilities.
- Do not parse external keymap JSON here.
- Expose a projection that updates descriptors' `shortcut` field.

Acceptance:

- Tests prove displayed shortcut selection follows the current keymap precedence rule.

## U4: Palette Dispatch Adapter

Add a small adapter that turns `CommandSelection` back into app-owned action dispatch:

- Selection value is the command id.
- Registry stores the dispatch target or action factory.
- Dispatch still goes through GPUI's `Window` / `App` action path.

Acceptance:

- A gallery sample opens a registry-backed command palette and records the dispatched command id.

## U5: Ecosystem Documentation

Document the command ecosystem as the native Open GPUI answer to `Cmd+K` / `Ctrl+P`:

- Describe the split between low-level GPUI actions/keymaps and command ecosystem projection.
- Show static command registration, plugin-like contribution registration, and palette projection.
- Explain why Vim/modal behavior remains app/editor-owned.

## Verification

Run after U1:

```powershell
cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components
cargo check -p open-gpui-ui-core --tests
cargo nextest run -p open-gpui-ui-core command --no-fail-fast
cargo nextest run -p open-gpui-ui-components command --no-fail-fast
git diff --check
```

Broaden to gallery tests when U4 adds a visible sample.
