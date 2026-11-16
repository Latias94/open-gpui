# Dockspace Editor

Run the retained document View example:

```text
cargo run --locked -p open-gpui-dockspace-editor
```

The two docking windows share a document while retaining separate View entities
and local drafts. Edit a View, move its tab, then choose **Rebuild docking
windows** in the recovery control window. The same View entities and their edits
reappear in the last committed layout under a fresh docking session.

On Windows, this example reserves 8 MiB of virtual main-thread stack space through
its binary's linker options. The unoptimized dockspace frame/reducer call chain
exceeds the 1 MiB Windows executable default. The stack commit size is unchanged;
no extra thread or event loop is introduced.

The control window stays outside the managed docking group. It owns the workspace
while its native windows retire, keeping the application alive under Windows'
last-window exit policy. Closing the control window retires the docking group
before closing the control itself. Closing it during reconstruction cancels the
pending rebuild.
Process-wide App shutdown also cancels a queued rebuild before any successor
windows can open.

## Recovery ownership

`recovery.rs` owns the application decision to replace a generation. Clicking
rebuild captures `DockSurface::with_view(|view| view.to_layout())` before shutdown
can recover Child content into its anchor. The application retains the original
pane entities and document separately.

A healthy generation uses `begin_native_shutdown`. If the user explicitly chooses
rebuild or close after a classified protocol/internal failure, this example
chooses `begin_failed_native_shutdown`. Observing a diagnostic alone never starts
teardown. A later failure that blocks an ordinary drain requires another explicit
click on the retry control. Transient capacity and operation conflicts retain the
ordinary drain path.

Both paths wait for `when_native_terminated`, including renderer shutdown,
original native terminal callbacks and outstanding callback handoffs. The
application callback defers reentry into its owner because a native pump can
complete synchronously. `DockNativeTermination::Failed` does not claim semantic
drain: its original rejected FIFO remains in the old session. Reconstruction
creates a fresh managed session/provider without transferring old native
bindings, input, gesture identities or presentation outputs.

If only some replacement windows open, their generation and original rollback
tickets stay owned until termination. The control window then offers **Retry
reconstruction**; there is no automatic retry loop. View state and the checkpoint
remain available through that cleanup.

This example uses an in-memory logical checkpoint. It does not persist document
files, external pane keys, undo history, native window placement preferences or
cross-process recovery. Applications that need those records should use the
adapter's document persistence interface and retain their own business state.

## Verification

The deterministic lifecycle tests use GPUI's TestPlatform and can run on hosted
CI without an interactive desktop:

```text
cargo nextest run --locked -p open-gpui-dockspace-editor --test-threads=1
```

Local native smoke commands require a real desktop and renderer. Run them
serially:

```text
cargo run --locked -p open-gpui-dockspace-editor -- --smoke-native
cargo run --locked -p open-gpui-dockspace-editor -- --smoke-recovery
```

`--smoke-native` qualifies native Child creation, actual Submitted frames,
retained View edits through moves, group retirement and an isolated sibling.
`--smoke-recovery` deliberately sends duplicate button-press records through the
public native ingress. It qualifies a classified protocol failure with real
native windows and renderer terminals, then checks fresh bindings, the original
Views, the committed move, continued editing and final retirement. The malformed
records are programmatic fault injection, not physical pointer input or an OS
capture qualification. Neither smoke is part of GitHub-hosted CI.
Both smoke modes use explicit process exit so closing the last native window
cannot end the process before its final success receipt is checked.
