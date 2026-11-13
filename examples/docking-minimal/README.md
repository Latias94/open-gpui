# Minimal shared-session docking

The example creates one `DockspaceSession` and passes it to the public
`DockSurface::new_managed` API. Its ordinary GPUI window is registered through
`open_native_root`. The application keeps the original `Entity<DemoPanel>` values
in its `Workspace` global and registers clones through `DockPane`.

Before opening the root, the example installs `DockSurface::set_close_handler`.
The handler explicitly resolves each pending item in a committed close plan with
`CloseDecision::Allow`: these panes are non-document demo UI. Tab and native-window
requests still follow the core close-plan flow. Closing a pane preserves its
registration and the application's retained entity; resolution failures are
printed to stderr. This is an application decision, not a default permission to
discard document contents. A document editor must supply its own save, cancel, or
discard decisions.

Explorer occupies an initial horizontal share of 24%. The remaining 76% contains
Editor and Terminal in a vertical 70:30 split. Editor is the central tabs leaf.
These are relative weights subject to dockspace's layout and minimum-size rules.
They do not reproduce the retired binary layout's absolute size references or
central sibling-size preservation. Contained floating remains enabled.

The existing `primary_close_converges_without_app_quit` regression now closes the
managed root with `QuitMode::Explicit`. It checks window removal, absence of an
application quit request, adapter errors, and `native_is_quiescent()` rather than
the retired facade's window-session phase.

Validation for the coordinating thread, run serially from the repository root:

```text
cargo check -p open-gpui-docking-minimal --all-targets
cargo nextest run -p open-gpui-docking-minimal primary_close_converges_without_app_quit
```

Cargo checks, tests, and GUI validation have not been run for this change. The
deterministic close regression simulates the native close request in
`TestAppContext`; it does not verify a physical OS close-button click or qualify a
real platform's native capabilities. Actual native presentation, input, DPI
changes, floating gestures, and shutdown still require the project's platform
validation workflow.
