# Multiple native roots in one shared session

The example uses the public root/prelude API. One `DockspaceSession` owns Project,
Editor, Inspector, and Preview. When GPUI advertises platform viewport windows,
Preview starts in a second logical surface opened as another managed native root.
Both physical windows share the original pane entities, layout, selection, and
native window group. Editor remains the central tabs leaf in the main root.
The application also owns an isolated session containing Activity and Notes.
Both sessions deliberately reuse the numeric main-surface and root identities;
their document lineages, pane registrations, and native owners remain separate.

`DemoApp` retains both workspaces, and each workspace retains its typed
`Entity<ExamplePanel>` values. The adapter receives clones through `DockPane`.
Panel movement does not construct replacement views. Closing the shared anchor
drains its native group without requesting application exit or shutting down the
isolated workspace. The secondary window is an application root, not a claimed
native child/transient-parent capability demonstration.

Each workspace installs `DockSurface::set_close_handler` before opening any roots.
The handler explicitly resolves every pending item in each committed content or
native close plan with `CloseDecision::Allow`, since these are non-document demo
panes. Failures are logged. Pane registrations and application entities remain
retained when a tab closes, so Preview can reopen with the same item identity,
entity, and local edits. This policy is specific to the demo: the adapter does not
grant default permission to discard document contents. Document applications must
supply their own save, cancel, or discard decisions.

The ordinary secondary application root uses the default `RetainLayout` native
close policy. Closing its window preserves the logical surface and Preview's
placement while the shared primary and isolated window remain open. Move Preview
can then return the same retained pane to Editor. The example does not exercise a
native child's `RecoverPresentation` close policy or an explicit policy override.

The panel controls expose the following behavior:

- **Edit this retained View** increments entity-local state. Edit Preview, move
  it to Editor, and move it back to observe that the same view keeps its edits.
- **Select Editor** issues `select_item_current`, then checks committed selection.
  Startup also requests this selection after a short delay. Native activation and
  focus still depend on real host observations.
- **Move Preview between Editor and secondary root** uses stable item/central
  anchors through `DockSurface::update`. An unavailable, closed, or busy target is
  reported without constructing another controller or a replacement entity. A
  closed Preview can reopen beside Editor using its registered retained entity.
- **Export workspace snapshot** coalesces explicit requests for 250 ms, captures
  `DockWorkspaceDocument`, and validates decoding into a separate candidate with
  exact document/key recognition. Startup and successful button-driven moves also
  request exports. The latest encoded bytes stay in application memory, and the
  panel shows their size. The example does not write a snapshot file.

Persistent item identities are allocated through `DockspaceDocumentBootstrap`.
Captures retain complete core topology, selection, key history, and any native
presentation preferences actually saved by the session. Host surface aliases are
built from the current published surface roster, including surfaces created by
native gestures. No GPUI entity, local edit counter, window handle, submission
proof, or fabricated logical-to-physical placement enters the document. A busy
capture keeps the previous export and logs that the user can retry.

Every export logs actual native reports and the owning windows' capability and
presentation facts. Reading advertised capabilities, receiving a request, and
observing a submitted frame are separate facts. Document decode validation does
not open restored windows or validate platform restore readiness. Real native
creation, placement, input, activation, focus, and cross-window drag support still
need the project's platform qualification workflow.

The old facade's change subscription and activation-completion callback have no
equivalent on this `DockSurface` API. Exports are explicitly requested rather than
automatically subscribed to every splitter, tab, or native-window change; use the
export control after interactive changes. Selection checks do not stand in for
native activation completion. The former viewport restore-readiness check is
replaced only by document decode validation, not by a native capability verdict.

If platform viewport windows are unavailable, Preview stays in Editor's tabs.
If opening the secondary root fails, the example attempts to return Preview to
Editor and reports the original open failure. If a pending native cleanup blocks
that attempt, the move control can retry while the primary windows stay running.
The independent workspace still
uses its own ordinary native root. All layouts use weighted dockspace splits;
they do not restore the retired binary/central absolute-size behavior.

Validation for the coordinating thread, run serially from the repository root:

```text
cargo check -p open-gpui-docking-multiviewport --all-targets
cargo nextest run -p open-gpui-docking-multiviewport shared_preview_moves_and_anchor_close_preserves_isolated_workspace
```

The deterministic `TestAppContext` regression calls the complete application
startup and reads the installed `DemoApp` global and root registry. It edits
Preview to seven edits, issues `request_close_item_current` (the core tab-close
request), and waits for close-handler dispatch. It checks that Preview leaves the
committed model while its registration, entity, and edits survive. Move Preview
then reopens the same item beside Editor and moves it to the secondary root, with
snapshot exports checked along the way.

The regression next simulates closing the secondary window while it contains
Preview. It checks that only that window is removed, both other windows stay
alive, and Preview's retained logical placement and seven edits survive. It moves
Preview back to Editor and reopens the secondary native root before closing the
shared anchor, preserving the original native-group shutdown coverage. It then
edits and saves the surviving isolated workspace before closing it separately.
Both groups must reach native quiescence without an application quit request in
`QuitMode::Explicit`.

Cargo checks, tests, and GUI validation have not been run for this change. These
regression assertions exercise core close requests and simulated native-window
callbacks; they do not verify physical OS tab or window close-button clicks.

After the shared workspace has compiled and been formatted, platform validation
should cover both shared roots and the isolated window; retained edits across
Preview moves; selection and snapshot exports after tab/split/floating changes;
closing either shared root; and continued Activity/Notes edits after closing the
shared anchor. Also cover secondary-open failure and the capability fallback.
No native capability validation is implied by this migration.
