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
- `CommandContextStack` carries the current command scope stack and GPUI key context stack from
  broad app/workspace context to focused surface context.
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
  `Keymap` or focused `Window`, dispatches command ids through `App` or `Window`, and diagnoses
  command/action/keymap drift.

`open_gpui_ui_components` owns presentation:

- `Command` renders inline or dialog command palettes.
- `CommandPaletteController` owns UI-side palette query/provider refresh lifecycle and emits
  `CommandPaletteProjection` updates without taking ownership of `CommandCenter`, dispatch, or
  async scheduling.
- `CommandIndexSnapshot::from_registry_snapshot` turns registry metadata into searchable palette
  rows.
- `CommandPaletteProjection` adapts a `CommandCenter` query projection into a UI-ready command
  snapshot, provider statuses, and shortcut diagnostics.
- `CommandProviderPaletteProjection` adapts a provider refresh projection into a UI-ready command
  snapshot, loading state, and provider-status readout without moving UI semantics into
  `open_gpui_command`.

The split is intentional: `open_gpui_command` owns reusable command domain contracts, GPUI remains
the runtime authority, and UI components only render projections.

## Command Center Registration

```rust
use open_gpui::{actions, KeyBinding, KeyContext, Keymap};
use open_gpui_command::{
    CommandCenter, CommandContextStack, CommandContribution, CommandDescriptor,
};
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
center.set_context_stack(
    CommandContextStack::new()
        .scope("global")
        .scope("workspace")
        .key_context(KeyContext::parse("Workspace")?),
);

let mut keymap = Keymap::default();
keymap.add_bindings([
    KeyBinding::new("ctrl-p", OpenWorkspace, Some("Workspace")),
    KeyBinding::new("ctrl-shift-p", OpenWorkspace, Some("Workspace")),
    KeyBinding::new("ctrl-s", SaveWorkspace, Some("Workspace")),
]);

let command_index =
    CommandIndexSnapshot::from_registry_snapshot(&center.snapshot_for_keymap(&keymap));
```

Applications can keep one center per app, workspace, plugin host, window, or surface. A center is
deliberately app-owned state, not a global singleton.

## Context Stack And Shortcuts

`CommandContextStack` keeps command scopes and GPUI key contexts adjacent but distinct:

- command scopes select the registry sources visible to `CommandCenter` snapshots, menus,
  diagnostics, dispatch, and provider requests;
- GPUI key contexts select the keymap bindings used when projecting shortcut labels or shortcut
  diagnostics from an app-level `Keymap`;
- focused-window projections still use `Window::highest_precedence_binding_for_action`, which is
  the runtime authority for the live rendered focus tree.

```rust
use open_gpui::{KeyBinding, KeyContext, Keymap};
use open_gpui_command::{CommandCenter, CommandContextStack};

center.set_context_stack(
    CommandContextStack::new()
        .scope("workspace")
        .scope("editor")
        .key_context(KeyContext::parse("Workspace")?)
        .key_context(KeyContext::parse("Editor vim_mode=normal")?),
);

let mut keymap = Keymap::default();
keymap.add_bindings([
    KeyBinding::new("ctrl-p", OpenWorkspace, Some("Workspace")),
    KeyBinding::new("ctrl-e", OpenWorkspace, Some("Editor")),
]);

let snapshot = center.snapshot_for_keymap(&keymap);
```

In the example above, an `editor` contribution can override a `workspace` command descriptor, and
the displayed shortcut for `OpenWorkspace` becomes the `Editor` binding. The lower-level
`GpuiCommandActionMap` exposes the same behavior through
`registry_snapshot_with_keymap_shortcuts_in_context` and
`shortcut_diagnostics_for_keymap_in_context` for callers that manage snapshots directly.

## Dynamic Providers

Dynamic providers are for command results that depend on query text, current scopes, async indexes,
or external state. `open_gpui_command` keeps this boundary runtime-neutral: providers return
`CommandProviderResponse` values, and applications may compute those values synchronously or in
their own async task system.

```rust
use open_gpui_command::{
    CommandContribution, CommandDescriptor, CommandProviderApplyOutcome,
    CommandProviderRefreshController, CommandProviderResponse, CommandProviderSource,
};

let provider = center.register_provider("recent-files", |request| {
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

center.refresh_provider(provider.provider_id().clone(), "readme");
```

For asynchronous providers, run the work in application code and apply the latest response when it
finishes:

```rust
let request = center.begin_provider_request("search-index", "readme");
let response = CommandProviderResponse::loading("Searching").source(CommandProviderSource::new(
    "workspace",
    "search-index-source",
    search_results,
)).for_request(&request);

match center.apply_provider_response_for_request("search-index", &request, response)? {
    CommandProviderApplyOutcome::Applied(status) => {
        // Render the status or project the updated snapshot.
    }
    CommandProviderApplyOutcome::Stale(_) => {
        // A newer query started first; ignore this response.
    }
}
```

For reusable per-provider command-palette query pipelines, use
`CommandProviderRefreshController` to connect query changes, optional loading status, response
application, stale-response handling, and registry snapshot projection:

```rust
let mut controller =
    CommandProviderRefreshController::new("recent-files").with_loading_message("Searching");

let projection = controller
    .refresh_provider(&mut center, "readme")
    .expect("provider is registered")?;
let registry_snapshot = projection.snapshot();
let status = projection.provider_status();
```

For a full UI command palette that coordinates one query across one or more providers, use
`CommandPaletteController` from `open_gpui_ui_components`. It wraps provider refresh controllers,
refreshes registered synchronous providers on query changes, keeps loading projections for
providers that are driven by app-owned async work, and returns the complete `CommandPaletteProjection`
that the `Command` component consumes:

```rust
use open_gpui_ui_components::{Command, CommandPaletteController};

let mut palette_controller = CommandPaletteController::new()
    .provider_with_loading("recent-files", "Searching recent files");

let update = palette_controller.set_query_for_keymap(&mut center, "readme", &keymap)?;
let command = Command::new("workspace-palette", "Workspace commands")
    .palette_projection(update.palette_projection());
```

When a configured provider has no registered synchronous callback, the update records the provider
id in `missing_provider_ids()`. Applications can then run their own async task and feed the result
back through `apply_provider_response_for_keymap` or `apply_provider_response_for_window`; stale
responses keep using the same `CommandCenter` request-id guard and do not replace newer results.

UI crates can use the provider-only adapter when they only need a refresh projection:

```rust
use open_gpui_ui_components::{Command, CommandProviderPaletteProjection};

let palette_projection = CommandProviderPaletteProjection::from_refresh_projection(&projection);
let status = palette_projection.provider_status();
let command = Command::new("recent-files", "Recent files")
    .provider_refresh_projection(&projection);
```

For a complete command-center palette projection, prefer `CommandPaletteProjection`. It joins the
current query, keymap/window shortcut projection, provider statuses, and shortcut diagnostics before
feeding the `Command` component:

```rust
use open_gpui_ui_components::{Command, CommandPaletteProjection};

let provider_projection = controller
    .refresh_provider(&mut center, "readme")
    .expect("provider is registered")?;
let palette_projection = CommandPaletteProjection::from_center_for_keymap(
    &center,
    provider_projection.query(),
    &keymap,
);
let command = Command::new("workspace-palette", "Workspace commands")
    .palette_projection(&palette_projection)
    .on_select(move |selection, window, cx| {
        center.dispatch_in_window(selection.value(), palette_projection.query(), window, cx);
    });
```

Both adapters treat command-center snapshots as `PreFiltered`: the command center has already
searched and ranked the registry for the query, so the component preserves that result set instead
of applying a second local filter. Loading provider status is projected into `CommandLoadingState`,
while ready and failed provider status remains available through provider-status accessors.

Applying a provider response atomically replaces that provider's previous dynamic sources. If a new
response has duplicate command ids for a scope, the existing registry state is preserved and the
error is returned. Responses bound to a center-issued request id are ignored as stale when a newer
request has already started for the same provider.

## Plugin-Like Contributions

Plugins should register metadata and action bindings through the same command id. The command id is
the only join key shared between metadata, shortcut projection, palette selection, and dispatch.

```rust
let source = center.register_source(
    "workspace",
    "spellcheck-plugin",
    [CommandContribution::new(
        CommandDescriptor::new("spellcheck.apply", "Apply Spelling Suggestion")
            .group("Editor")
            .keyword("typo"),
    )],
)?;

source.unregister(&mut center);
```

`CommandSourceHandle` and `CommandProviderHandle` are explicit lifecycle handles. They are
intentionally not `Drop`-driven RAII guards because `CommandCenter` is app-owned rather than a
global singleton; plugin hosts decide when they have mutable access to the center and call
`handle.unregister(&mut center)` or `center.unregister_source_handle(&handle)`. The older
`CommandSourceRegistration` and `CommandProviderRegistration` names remain aliases for existing
callers.

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

Dynamic provider commands can also dispatch through `CommandCenter`, but only when their dynamic
command ids are bound to GPUI actions. If a provider result represents application-specific data
that should not become a GPUI action, keep the command id stable and handle it directly in
`on_select` instead of calling `dispatch_in_window`.

For custom ranking, call `center.search_snapshot(query)` or
`center.search_snapshot_for_keymap(query, &keymap)` before converting the result into
`CommandIndexSnapshot`.

## Shortcut Diagnostics

Shortcut diagnostics are for application startup checks, plugin host validation, and gallery or
test assertions. They report mismatches between command metadata, GPUI action bindings, and the
effective keymap projection:

```rust
use open_gpui_command::CommandShortcutDiagnosticKind;

let diagnostics = center.shortcut_diagnostics_for_keymap(&keymap);
assert!(
    diagnostics
        .iter()
        .all(|diagnostic| diagnostic.kind() != CommandShortcutDiagnosticKind::MissingAction)
);
```

The lower-level `GpuiCommandActionMap` diagnoses against exactly the snapshot it receives. That
strict mode is useful for framework tests because an action bound to a command id outside the
snapshot is reported as `OrphanAction`.

`CommandCenter` runs diagnostics against the current visible snapshot, but it suppresses orphan
diagnostics for commands that still exist in active scoped sources and are only hidden by current
availability. Hidden commands should disappear from palettes and menus without looking like stale
plugin registrations.

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
recommended facade can project current keymap shortcut labels, preserve the command id used for
dispatch, and surface an empty shortcut diagnostic set for the healthy sample.

The gallery's `provider-search` command sample uses `CommandPaletteController` and
`CommandPaletteProjection` to prove that query-specific `CommandProviderSource` results can be
applied to the center, bound to GPUI actions and shortcuts, checked for empty shortcut diagnostics,
converted into a `CommandIndexSnapshot`, and rendered by the existing `Command` component without
making `open_gpui_command` depend on UI component types.

The gallery's `context-stack` command sample uses `CommandContextStack` to prove that focused
command scopes can override broader scope descriptors while GPUI key contexts project the shortcut
active for the focused editor surface.
