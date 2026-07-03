# Open GPUI Command Ecosystem

Open GPUI's command ecosystem is a projection layer over GPUI's native action and keymap runtime.
It is the recommended starting point for `Cmd+K` / `Ctrl+P` command palettes, command-menu reuse,
and plugin-like command metadata contribution.

## Authority Boundary

`open_gpui` remains the low-level runtime authority:

- `Action` values are the executable command payload.
- `Keymap` and `Window::highest_precedence_binding_for_action` resolve shortcut precedence.
- `App::dispatch_action` and `Window::dispatch_action` execute actions.
- Application state owns enablement, modal editing state, Vim-style modes, and chord policies.

`open_gpui_ui_core` owns renderer-neutral command metadata:

- `CommandDescriptor` stores id, label, group, keywords, shortcut display text, disabled state,
  caller-owned `when` metadata, and optional menu path.
- `CommandRegistry` stores deterministic command contributions and duplicate-id diagnostics.
- `CommandRegistrySnapshot` is the stable projection handed to UI components and adapters.

`open_gpui_ui_components` owns presentation and GPUI bridging:

- `Command` renders inline or dialog command palettes.
- `CommandIndexSnapshot::from_registry_snapshot` turns registry metadata into searchable palette
  rows.
- `open_gpui_ui_components::gpui_adapter::GpuiCommandActionMap` maps command ids to GPUI actions,
  projects current shortcut labels from a `Keymap` or focused `Window`, and dispatches a
  `CommandSelection` through `App` or `Window`.

The split is intentional: the UI crates describe and present commands, but they do not become a
global application command runtime.

## Static Registration

```rust
use open_gpui_ui_core::{CommandDescriptor, CommandRegistry};

let mut registry = CommandRegistry::new("workspace-v1");
registry.register(
    CommandDescriptor::new("workspace.open", "Open Workspace")
        .group("Workspace")
        .keyword("project"),
)?;
registry.register(
    CommandDescriptor::new("workspace.save", "Save Workspace")
        .group("Workspace")
        .keyword("persist"),
)?;
```

Applications can keep one registry per app, per workspace, or per plugin host. The snapshot is
immutable and cheap to hand to UI code:

```rust
let snapshot = registry.snapshot();
let command_index = CommandIndexSnapshot::from_registry_snapshot(&snapshot);
```

## Plugin-Like Contributions

Plugins should register metadata and action bindings through the same command id. The command id is
the only join key shared between metadata, shortcut projection, palette selection, and dispatch.

```rust
use open_gpui::{KeyBinding, Keymap, actions};
use open_gpui_ui_components::gpui_adapter::GpuiCommandActionMap;

actions!(workspace_actions, [OpenWorkspace, SaveWorkspace]);

let action_map = GpuiCommandActionMap::new()
    .action("workspace.open", OpenWorkspace)
    .action("workspace.save", SaveWorkspace);

let mut keymap = Keymap::default();
keymap.add_bindings([
    KeyBinding::new("ctrl-p", OpenWorkspace, None),
    KeyBinding::new("ctrl-shift-p", OpenWorkspace, None),
]);

let command_index =
    action_map.command_index_snapshot_with_keymap_shortcuts(&registry.snapshot(), &keymap);
```

If two contributions use the same command id, `CommandRegistry::register` rejects the duplicate.
If an action map contains repeated ids, the last binding wins so apps can layer plugin overrides
without mutating earlier contributions.

## Palette Dispatch

`CommandSelection::value()` is the command id. A command palette should dispatch by looking up that
id in the app-owned action map:

```rust
command.on_select(move |selection, window, cx| {
    action_map.dispatch_selection_in_window(&selection, window, cx);
});
```

Use `dispatch_selection_in_window` when the selection belongs to a concrete focused surface. Use
`dispatch_selection_in_app` when the application wants GPUI's app-level action routing to decide
between focused windows and global handlers.

## Modes And Chords

This layer does not invent a second Vim/chord engine. Mode state and chord resolution stay with the
application and GPUI keymap system. The command ecosystem carries display and discoverability facts:

- command id and label;
- current shortcut label after keymap precedence resolution;
- grouping and menu path;
- disabled and `when` metadata for app-owned policy projection.

That keeps the component library useful for editors, design tools, and multi-viewport apps without
forcing one global registry or one input-mode model on every application.

## Verification

Focused command ecosystem gates:

```powershell
cargo nextest run -p open-gpui-ui-core command --no-fail-fast
cargo nextest run -p open-gpui-ui-components gpui_adapter --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts --no-fail-fast
```

The gallery's `registry-dispatch` command sample proves that a registry-backed palette can project
current keymap shortcut labels and preserve the command id used for dispatch.
