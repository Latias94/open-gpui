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

`open_gpui_command` owns reusable command metadata and projections:

- `CommandCenter` is the recommended app-owned facade. It composes scoped metadata registration,
  source unregistration, availability projection, GPUI action mapping, shortcut projection,
  dispatch, menu projection, fuzzy search, and in-memory usage/query history.
- `CommandProvider*` types model dynamic command providers and async-friendly provider responses
  without owning a task runtime.
- `CommandDescriptor` stores id, label, group, keywords, shortcut display text, disabled state,
  optional disabled reason, caller-owned `when` metadata, and optional menu path.
- `CommandRegistry` stores deterministic command contributions and duplicate-id diagnostics.
- `CommandRegistrySnapshot` is the stable projection handed to UI components and adapters.
- `ScopedCommandRegistry` projects active scopes and supports source/scope unregistration.
- `CommandAvailabilityMap` projects visible, disabled, or hidden command state without evaluating a
  context-expression DSL.
- `CommandMenuTree` builds a neutral menu hierarchy from command metadata.
- `MemoryCommandHistory` records in-memory usage/query hints for ranking and query recall.
- `GpuiCommandActionMap` maps command ids to GPUI actions, projects current shortcut labels from a
  `Keymap` or focused `Window`, and dispatches command ids through `App` or `Window`.

`open_gpui_ui_components` owns presentation:

- `Command` renders inline or dialog command palettes.
- `CommandIndexSnapshot::from_registry_snapshot` turns registry metadata into searchable palette
  rows.

The split is intentional: `open_gpui_command` owns reusable command domain contracts, GPUI remains
the runtime authority, and UI components only render projections.

## Command Center Registration

```rust
use open_gpui::{actions, KeyBinding, Keymap};
use open_gpui_command::{CommandCenter, CommandContribution, CommandDescriptor};
use open_gpui_ui_components::CommandIndexSnapshot;

actions!(workspace_actions, [OpenWorkspace, SaveWorkspace]);

let mut center = CommandCenter::new("workspace-v1");
center.register_source(
    "global",
    "workspace",
    [
        CommandContribution::new(
            CommandDescriptor::new("workspace.open", "Open Workspace")
                .group("Workspace")
                .keyword("project")
                .menu_path(["File", "Open"]),
        ),
        CommandContribution::new(
            CommandDescriptor::new("workspace.save", "Save Workspace")
                .group("Workspace")
                .keyword("persist")
                .menu_path(["File", "Save"]),
        ),
    ],
)?;
center
    .register_action("workspace.open", OpenWorkspace)
    .register_action("workspace.save", SaveWorkspace);

let mut keymap = Keymap::default();
keymap.add_bindings([
    KeyBinding::new("ctrl-p", OpenWorkspace, None),
    KeyBinding::new("ctrl-shift-p", OpenWorkspace, None),
    KeyBinding::new("ctrl-s", SaveWorkspace, None),
]);

let command_index =
    CommandIndexSnapshot::from_registry_snapshot(&center.snapshot_for_keymap(&keymap));
```

Applications can keep one center per app, workspace, plugin host, window, or surface. A center is
deliberately app-owned state, not a global singleton.

## Dynamic Providers

Dynamic providers are for command results that depend on query text, current scopes, async indexes,
or external state. `open_gpui_command` keeps this boundary runtime-neutral: providers return
`CommandProviderResponse` values, and applications may compute those values synchronously or in
their own async task system.

```rust
use open_gpui_command::{
    CommandContribution, CommandDescriptor, CommandProviderResponse, CommandProviderSource,
};

let registration = center.register_provider("recent-files", |request| {
    CommandProviderResponse::ready().source(CommandProviderSource::new(
        "workspace",
        "recent-files-source",
        [CommandContribution::new(
            CommandDescriptor::new(
                format!("recent.open.{}", request.query()),
                format!("Open Recent {}", request.query()),
            )
            .group("Recent"),
        )],
    ))
});

center.refresh_provider(registration.provider_id().clone(), "readme");
```

For asynchronous providers, run the work in application code and apply the latest response when it
finishes:

```rust
let response = CommandProviderResponse::loading("Searching").source(CommandProviderSource::new(
    "workspace",
    "search-index-source",
    search_results,
));
center.apply_provider_response("search-index", response)?;
```

Applying a provider response atomically replaces that provider's previous dynamic sources. If a new
response has duplicate command ids for a scope, the existing registry state is preserved and the
error is returned.

## Plugin-Like Contributions

Plugins should register metadata and action bindings through the same command id. The command id is
the only join key shared between metadata, shortcut projection, palette selection, and dispatch.

```rust
let registration = center.register_source(
    "workspace",
    "spellcheck-plugin",
    [CommandContribution::new(
        CommandDescriptor::new("spellcheck.apply", "Apply Spelling Suggestion")
            .group("Editor")
            .keyword("typo"),
    )],
)?;

center.unregister(&registration);
```

If two contributions use the same command id, `CommandRegistry::register` rejects the duplicate.
If an action map contains repeated ids, the last binding wins so apps can layer plugin overrides
without mutating earlier contributions.

The lower-level `CommandRegistry`, `ScopedCommandRegistry`, and `GpuiCommandActionMap` remain public
for tests, specialized hosts, and framework authors. Product code should usually start with
`CommandCenter`.

## Availability And Menu Projection

Availability is a value projection, not a DSL evaluator:

```rust
use open_gpui_command::CommandAvailabilityMap;

center.set_availability(
    CommandAvailabilityMap::new()
        .disabled("workspace.save", "Workspace is read-only")
        .hidden("workspace.close"),
);

let visible_snapshot = center.snapshot_for_keymap(&keymap);
let menu_tree = center.menu_tree_for_keymap(&keymap);
```

Applications own the policy that decides those states. The command crate only applies the result
consistently for palettes, menus, and dispatch guards.

## Palette Dispatch

`CommandSelection::value()` is the command id. A command palette should dispatch through the
app-owned command center:

```rust
command.on_select(move |selection, window, cx| {
    center.dispatch_in_window(selection.value(), current_query.as_str(), window, cx);
});
```

Use `dispatch_in_window` when the selection belongs to a concrete focused surface. Use
`dispatch_in_app` when the application wants GPUI's app-level action routing to decide between
focused windows and global handlers. Successful dispatch records usage/query history, while hidden
or disabled commands are blocked by the same availability projection used by palettes and menus.

For custom ranking, call `center.search_snapshot(query)` or
`center.search_snapshot_for_keymap(query, &keymap)` before converting the result into
`CommandIndexSnapshot`.

## Modes And Chords

This layer does not invent a second Vim/chord engine. Mode state and chord resolution stay with the
application and GPUI keymap system. The command ecosystem carries display and discoverability facts:

- command id and label;
- current shortcut label after keymap precedence resolution;
- grouping and menu path;
- disabled and `when` metadata for app-owned policy projection;
- bounded query/usage history for app-owned palette ranking;
- dynamic provider status for app-owned async/search UX.

That keeps the component library useful for editors, design tools, and multi-viewport apps without
forcing one global registry or one input-mode model on every application.

## Verification

Focused command ecosystem gates:

```powershell
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components choice --no-fail-fast
cargo nextest run -p open-gpui-ui-components command::runtime::tests --no-fail-fast
cargo nextest run -p open-gpui-ui-components menu context_menu --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata --no-fail-fast
```

The gallery's `registry-dispatch` command sample uses `CommandCenter` to prove that the
recommended facade can project current keymap shortcut labels and preserve the command id used for
dispatch.

The gallery's `provider-search` command sample uses a `CommandCenter` provider refresh to prove
that query-specific `CommandProviderSource` results can be applied to the center, converted into a
`CommandIndexSnapshot`, and rendered by the existing `Command` component without making
`open_gpui_command` depend on UI component types.
