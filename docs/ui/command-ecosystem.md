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
  dispatch, menu projection, fuzzy search, and in-memory usage/query history. It also exposes
  `record_query`, `recent_queries`, `previous_query`, `next_query`, and
  `reset_query_navigation` so app shells do not need to reach into `history_mut()` for palette
  query recall.
- `CommandContextStack` carries the current command scope stack and GPUI key context stack from
  broad app/workspace context to focused surface context.
- `CommandKeyBindingRegistry` stores command-id keyed shortcut dictionaries from apps or plugins
  and projects them into concrete GPUI `KeyBinding` values using `GpuiCommandActionMap`. Missing
  actions and bad GPUI keystroke/context syntax are reported as projection diagnostics instead of
  panicking. Same-context shortcut conflicts are reported separately, while valid bindings still
  install so GPUI remains the runtime precedence authority.
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
  `CommandNavigationBehavior` keeps keyboard traversal explicit: Up/Down wraps by default for
  command-palette ergonomics, `loop_navigation(false)` makes boundaries stop, and Alt+Up/Alt+Down
  jumps to the first focusable command in the previous or next rendered group.
- `CommandPaletteController` owns UI-side palette query/provider refresh lifecycle and emits
  `CommandPaletteProjection` updates without taking ownership of `CommandCenter`, dispatch, or
  async scheduling. It also wraps command-center query history navigation for palette surfaces:
  `previous_query_for_keymap` / `next_query_for_keymap` and their window variants capture the
  current input as the history prefix and restore that draft query after moving past the newest
  matching history entry.
- `CommandIndexSnapshot::from_registry_snapshot` turns registry metadata into searchable palette
  rows.
- `CommandPaletteProjection` adapts a `CommandCenter` query projection into a UI-ready command
  snapshot, provider statuses, shortcut diagnostics, and `CommandStatusItem` rows. Failed
  providers become error status rows; shortcut/action/keymap drift becomes warning rows.
- `CommandProviderPaletteProjection` adapts a provider refresh projection into a UI-ready command
  snapshot, loading state, provider-status readout, and failed-provider status rows without moving
  UI semantics into `open_gpui_command`.

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

## Command Key Binding Sources

Use `CommandKeyBinding` when a plugin or app module wants to contribute shortcuts by command id
without directly constructing GPUI action values. The command center keeps the binding source
lifecycle separate from command metadata sources, then projects valid entries into a GPUI `Keymap`:

```rust
use open_gpui::{KeyContext, Keymap};
use open_gpui_command::CommandKeyBinding;

center
    .register_action("workspace.open", OpenWorkspace)
    .register_action("workspace.save", SaveWorkspace);

let shortcuts = center.register_key_bindings(
    "workspace-shortcuts",
    [
        CommandKeyBinding::new("workspace.open", "ctrl-k ctrl-o").context("Workspace"),
        CommandKeyBinding::new("workspace.save", "ctrl-s")
            .context("Workspace && mode == normal"),
    ],
);

let mut keymap = Keymap::default();
let report = center.install_key_bindings(&mut keymap);
assert!(report.is_clean());
assert_eq!(report.installed_count(), 2);

center.set_key_contexts([KeyContext::parse("Workspace mode=normal")?]);
let snapshot = center.snapshot_for_keymap(&keymap);

shortcuts.unregister(&mut center);
```

The installation APIs (`install_key_bindings`, `install_key_bindings_in_app`, and
`CommandKeyBindingRegistry::install_into_keymap`) append valid projected bindings to the target
GPUI keymap and return `CommandKeyBindingInstallReport`. The report exposes skipped-entry
diagnostics, same-context conflicts, the concrete binding count, and the underlying projection.
Because GPUI keymaps do not expose source-level removal, unregistering a command key binding source
updates the registry only; app shells that need live shortcut reload should rebuild their command
owned keymap layer before reinstalling.

Conflict reports are intentionally conservative. They flag entries that normalize to the same GPUI
keystroke display string and the same normalized context predicate while targeting different
command ids. A global binding with no context is also reported against concrete same-keystroke
context bindings, because GPUI treats no-context bindings as active in focused contexts. Those
bindings still install, and GPUI's usual precedence rules decide dispatch order: deeper focused
contexts win first, then later registered bindings win within the same depth.

For compatibility, `CommandKeyBindingProjection::is_clean()` means there were no skipped-entry
projection errors. Use `has_conflicts()` or `is_strictly_clean()` when conflicts should fail a
plugin-host validation gate. `CommandKeyBindingInstallReport::is_clean()` is strict because install
reports are new and meant for startup validation.

This layer does not replace GPUI's key dispatch engine. Chords still use GPUI's whitespace-separated
keystroke sequences, context/mode checks still use GPUI key binding predicates such as
`Workspace && mode == normal`, and focused-window precedence still comes from
`Window::highest_precedence_binding_for_action`.

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

`Command::palette_projection` and `Command::provider_refresh_projection` copy status rows into the
component state, so app shells do not need to rebuild provider-error or shortcut-diagnostic UI for
the common palette case. Apps can still add their own rows with `status_item` or `status_items`.

Palette query history stays in `CommandCenter`, but the controller owns the per-surface navigation
prefix. Record accepted queries through the center, seed the controller with the user's current
input, and call the history helpers from your key handler:

```rust
center
    .record_query("open file")
    .record_query("open settings");

let mut palette_controller = CommandPaletteController::new().with_query("open");

if let Some(update) = palette_controller.previous_query_for_keymap(&mut center, &keymap) {
    let update = update?;
    // update.query() == "open settings"
    // update.palette_projection() is ready for Command::palette_projection(...)
}

if let Some(update) = palette_controller.next_query_for_keymap(&mut center, &keymap) {
    let update = update?;
    // Moving past the newest matching entry restores the original "open" draft query.
}
```

When a configured provider has no registered synchronous callback, the update records a
`CommandPalettePendingProviderRequest` in `pending_provider_requests()`. Applications can hand those
provider/request pairs to their own async task and feed the result back through
`apply_provider_response_for_keymap` or `apply_provider_response_for_window`; stale responses keep
using the same `CommandCenter` request-id guard and do not replace newer results.
`missing_provider_ids()` remains available as a compatibility summary when callers only need to
know which providers are async-backed.

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
