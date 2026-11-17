# Open GPUI Docking

`open-gpui-docking` binds one renderer-neutral `DockspaceSession` to retained GPUI
Views and physical windows. The crate root and `prelude` use this shared session
by default. Layout, selection, gestures and durable pane identities have one
semantic owner; the adapter owns GPUI entities, exact frame submission and native
transport. Documents, history and engine rendering remain application-owned.

## Start With One Surface

```rust
use open_gpui::{App, WindowOptions};
use open_gpui_docking::{DockPane, DockSurface, GpuiDockspaceConfig};
use open_gpui_docking::dockspace::{
    model::{DockspaceLayout, DockspaceNode, DockspaceRootLayout,
            DockspaceSurfaceLayout, ItemId, RootId, SurfaceId},
    policy::DockPolicy,
    runtime::DockspaceSession,
};

fn open(cx: &mut App, content: impl Into<open_gpui::AnyView>) -> anyhow::Result<()> {
    let surface_id = SurfaceId::new(1);
    let item = ItemId::new(1);
    let layout = DockspaceLayout::new([DockspaceSurfaceLayout::new(
        surface_id,
        DockspaceRootLayout::new(RootId::new(1), DockspaceNode::central_tabs([item])),
    )])?;
    let dock = DockSurface::new_managed(
        DockspaceSession::from_layout(layout, DockPolicy::default())?,
        [DockPane::new(item, "Editor", content)],
        GpuiDockspaceConfig::default(),
        cx,
    )?;
    dock.open_native_root(surface_id, true, WindowOptions::default(), cx)?;
    Ok(())
}
```

Use `DockSurface::new` for embedded logical content and `new_managed` for native
workspaces. These enroll separate local and managed pointer producers; a session
cannot use both. Render one logical surface with `element`, or disjoint regions
sharing one physical window with `regions`. The adapter aggregates all regions
under the original Window's exact frame ticket.

Each `DockPane` is one View occurrence. Register a distinct GPUI entity and pane
identity for each View, including Views that share a document. Moving a pane
retains that entity. Duplicating a View creates another entity sharing the
application document. One entity cannot be registered under two pane identities.
Use `with_view` to inspect the published session model and `update` to submit
programmatic changes through the sole semantic frame.

Use `update_action(update, map, cx)` when the caller needs a command result.
Return the core's submitted token from `update`; `map` receives that token's
exact `HostInputOutcome` before the host consumes the report. An `Ok` frame can
still contain a product rejection or a stale action. The mapper receives `None`
for a token outside that committed frame. Copy the application's result in the
mapper; it runs synchronously under the host borrow and must not reenter the
surface. Native effects and output receipts stay with the host.

Register `set_commit_handler` before issuing actions to observe committed core
notifications and presentation transition terminals outside the host borrow.
`notification().document_changed()` supports application-owned autosave,
including native placement changes. Match a pending command's transition ID to
`presentation_transitions()` for its eventual result. A command's acceptance
does not prove its destination was presented. `clear_commit_handler` removes
the observer and its undelivered notification copies; no history is retained
while no handler is installed.

Bind a pane's existing content focus with `DockPane::with_focus_handle`. Default
chrome exposes close, overflow and presentation controls, keyboard tab navigation
and accessibility actions through the shared core's revision-bound commands.
Content activation waits for the original output to become interactive; merely
painting hidden or deferred content does not grant focus.

Arrow/Home/End navigation follows the focused tab; Ctrl+Tab and Ctrl+Shift+Tab
cycle documents. Independent key events retain their original core witness
through the temporary inert packet used for submission, and a later application
focus or blur claim ends that continuation. Enter hands ownership to pane
content. Tab-list Up/Down and Home/End likewise compose before the next
presentation. Enter/Space confirms the reached row and Escape returns to the
opener; either terminal ends the menu's key prefix. Overflowing strips and tab
lists accept real scroll events; phased
precise scrolling keeps its original chrome/content owner through tail events.
Layout actions operate on the complete Root, including its split tree. Tab group
actions float, move to a native window, or request one atomic close of the
captured group, retaining sibling groups. Group grips drag that same scope.
The layout icon and group ellipsis use core-owned, separate allocations. Menus
dismiss when their original session or workspace revision changes, and each
opening has its own callback lifetime. Up/Down, Home/End and Enter/Space navigate
enabled actions; Escape dismisses. Ordered keys that open and confirm a menu
wait for its exact interactive presentation before submitting the command.

Application commands can use `capture_tabs_group(DockAnchor)` and pass the
original capture to `frame.request_close_tabs_current` through `update` or
`update_action`. `set_close_handler` receives a single plan with each member's
decision token. Approve all members to close the group; any veto retains every
member. Keep deferred plans and their exact tokens for asynchronous saving.
Do not recapture the anchor or loop over independent item-close requests.

Use `DockPane::with_sizing` and `update_pane_metadata` to retain a validated
`dockspace::geometry::PaneSizing` with each View. Minimum, maximum, fixed and
preferred extents describe content in logical points; core adds docking chrome.
Only the selected visible tab constrains its pane, while inactive visible tabs
also supply metadata for future selection and movement. A maximum or fixed
extent can leave background inside a larger window. Preferences do not override
user weights during ordinary layout; focus a splitter and press Enter/Space, or
use its accessibility Click action, to restore preferred sizes explicitly.
Without preferences, that action equalizes the adjacent panes within their
constraints. A primary-button double click requests the same restoration after
its original release. Set `config.input.splitter_double_click` to
`Some(SplitterDoubleClick::Equalize)` to equalize despite declared preferences,
or `None` to disable pointer resets. Dragging, cancelled capture and covered
release targets do not trigger a reset. Keyboard and accessibility restoration
remain available independently. `LogicalSize` and `with_minimum_size` remain
minimum-only shortcuts.

Updating a title or sizing invalidates core measurements and the GPUI packet
without replacing the View. `take_closed_pane` transfers a closed View registration only after
the old input, packets and renderer obligations drain. Dropping the returned pane
releases that registration's resources without deleting its durable external key;
passing it to `open_pane` registers the same View again. Both operations may return
`DockPaneError::Busy` while existing owners still need to progress.

## Native Lifecycle And Inspection

Open ordinary roots through `open_native_root`, or use `bind_native_root` inside
a custom original GPUI window creation transaction. A failed root open returns
`NativeOpenFailure`, retaining its original `WindowOpenError`, binding error and
rollback ticket. Custom builders pass their detailed-open failures through
`retain_native_root_open_failure` so rollback authority survives until retirement.

Native close policy supports allow, veto and defer. `begin_native_shutdown`
freezes admission and drains the exact workspace group, with dependents before
the anchor. `native_is_quiescent` waits for the original physical owners; logical
close, renderer/capture quiescence and actual native destruction are separate.
Native close dispatch waits until the original captured-drag release terminal
has been collected. The original activated drag generation is retained even
before its first native motion callback arrives.
`take_native_reports` transfers observations to the application. Read-only
`inspection` returns an owned snapshot or `DockInspectionError::Busy` during a
mutable host borrow; it does not consume reports or drive the runtime. DevTools'
`docking` feature projects that snapshot; it does not enable the old engine.

After shutdown starts, `update` rejects application intent before invoking its
callback and `open_pane` leaves its View unregistered. Original renderer terminals,
native observations and cleanup still progress through their existing owners.
`failure` copies the primary background diagnostic with the original core/native
error categories, when available. The first protocol/internal failure survives
later cleanup errors; `error` retains its text-only convenience interface.
Neither diagnostic consumes an affine result or authorizes input replay. A
permanently rejected native FIFO head can prevent semantic drain even after the
windows close. There is no in-place error-clear/restart API. An application can
explicitly choose `begin_failed_native_shutdown` when it decides to replace a
failed generation. Wait for `when_native_terminated` before reconstructing a fresh
managed session from committed logical state and application-owned Views.
`DockNativeTermination::Drained` proves semantic drain;
`DockNativeTermination::Failed` proves physical termination while the old rejected
FIFO remains owned by its original session. `native_is_quiescent` stays false for
that failed generation. No old bindings, input, effects or outputs enter the new
session. A diagnostic category alone never starts failed shutdown automatically.

Retain an application owner outside the retiring group, and keep the process alive
under its last-window exit policy. The [dockspace editor example](../../examples/dockspace-editor/README.md)
demonstrates an independent recovery window, deferred application callback,
retained View entities and explicit retry after partial native-open cleanup.

Application observations keep the latest 256 historical entries. Unresolved
deferred native close decisions remain protected until consumed, resolved or
made obsolete by their exact native terminal. Inspection includes the historical
discard count, current ingress head, original rejection categories and blocked
stationary challenge. This retention policy never truncates internal ingress or
its original lifecycle receipts. A primary protocol failure rejects subsequent
application intent with `DockHostAdmissionError`, while existing cleanup owners
continue to run.

Register `set_close_handler` before opening windows to decide the original
committed content plans, including tab-close and native surface plans. The
handler receives its surface without needing to capture a strong clone. It runs
outside the host borrow after the update lease and earlier native input settle;
use `update` and the plan's exact `pending_close_decisions` tokens to allow, veto
or defer. No handler means no automatic permission to discard application
content. Closing a demo View does not close its application document. Ordinary
application roots default to native `RetainLayout`; children recover their
original presentation. `set_native_close_policy` explicitly overrides that first
native decision, separately from pane-content decisions. Forced anchor shutdown
still drains the group independently.

Content remains `SubtreePresentation::Inert` until its matching original Submitted
artifact is admitted. Candidate acceptance alone does not authorize input,
focus, IME or accessibility. A submitted output retains its original callback
and binding across physical Root continuation. Desktop physical coordinates,
target-display DPI, native hit stacks and OS capture require actual platform
facts; ordinary logical mouse events do not supply them.

Custom `NativeBridge` hosts using `App::consume_native_captured_drag` must call
`NativeCallbacks::observe_capture_release` in its early ReleaseLocker callback
before routing the event to `NativeBridge::captured_drag`. The callback owner
retains the original natural-release barrier through delayed FIFO collection.
Observing a completed gesture later cannot reconstruct that release receipt.

Managed stationary hover and strip scrolling use one fresh native sample for the
core's finite deadline, tied to the original captured-drag generation. Unsupported
sampling preserves the held gesture and blocks that unchanged challenge until
real input rearms it. Windows provides the native sampler; other backends remain
explicitly unsupported until they implement fresh physical observations.

After validating a saved `DockWorkspaceDocument`, construct a fresh managed
workspace and call `open_restored_native_roots` with its `DockHostDocument` and
explicit anchor. The helper checks the complete mapping and window options before
opening, resolves current displays, constrains logical hints to current work
areas, and drains earlier roots if a later open fails. Keep the host alive until
its original cleanup becomes quiescent. The multiviewport example demonstrates
restoring a snapshot into isolated native roots with distinct application Views.
Use `when_native_quiescent` after opening or starting cleanup to release an
application registry's workspace entry at that exact boundary. The one-shot
callback runs outside the host borrow; capture an instance key or weak owner.

Platform focus hints wait for earlier native visibility/lifecycle input to commit
before the bridge maps a GPUI window to known Dock focus. A retained binding may
still be hidden, unobserved or cleanup-only; existence alone cannot grant focus
authority. The core's `native_binding_can_report_focus` query decides that state.

## Save And Restore

`persistence::DockWorkspaceDocument` embeds the shared core's public document and
stable external View-key history, including closed occurrences. Runtime entities,
focus handles, native handles and frame proofs never enter that document. Restore
validates all keys before any content factory runs. `capture_document` refuses a
borrowed host or unaccepted update without advancing its save generation;
accepted outputs waiting on a renderer do not block capture.

Resolve an existing closed occurrence with `item_id_for_external_key`, then use
`open_pane` to atomically register its distinct retained View and open it. A busy
lease, duplicate entity or rejected placement publishes neither operation.

Legacy v2/v3 data migrates through the same module without `legacy-docking`.
Choose `LegacyTopologyMigration::RelativeWeights` explicitly. Tab order,
selection, floating bounds and representable relative shares are retained;
legacy structural identities and absolute size references stay in a provenance
receipt. Binary rounding, central sibling-size preservation and absolute sizing
are not weighted N-ary equivalence claims. Persisted logical window positions
remain hints until the current platform establishes physical facts.

## Examples And Compatibility

```sh
cargo run -p open-gpui-docking-minimal
cargo run -p open-gpui-docking-multiviewport
cargo run -p open-gpui-dockspace-editor
```

The minimal example demonstrates one managed root. Multiviewport demonstrates
shared-session windows and an isolated workspace. The editor example retains two
Views of one document and has a bounded native smoke trajectory through the
repository's macOS probe runner.

The displaced engine is available only through `open_gpui_docking::legacy` and
its explicit `legacy-docking` feature. Enabling it never changes the root or
prelude types. It is not a fallback or a second writer for a migrated workspace.
The old Windows native dogfood example retains its compatibility qualification
while the ten shared-session replacement scenarios receive physical Windows
qualification:

```sh
cargo run -p open-gpui-docking-native --features legacy-docking
```

See [compatibility notes](LEGACY.md), the
[migration guide](../../docs/ui/migration-v0.3.md#default-shared-docking-session),
[ADR 0034](../../docs/adr/0034-shared-dockspace-session-integration.md) and the
[cutover evidence](../../docs/knowledge/engineering/verification/dockspace-default-cutover-20261114.md).
Historical receipts retain source paths at their recorded commits; incumbent
files now live under `src/legacy/`.

The browser unsupported-native-window probe now uses the shared session. Unused
canonical migration and detached payload experiments have been removed. The
[shared-session migration record](../../docs/knowledge/engineering/verification/dockspace-shutdown-migration-20261115.md)
lists the exact source commits and physical Windows qualification boundary. Copy a logical recovery
checkpoint with `dock.with_view(|view| view.to_layout())`; continue using the
document envelope when external View-key history and save generations matter.

## Compiler And Verification Scope

This crate and its selected shared core require Rust 1.95. Core GPUI and the
component crates retain Rust 1.93. The default path has no production dependency
on the displaced engine's motion, UI core or graph containers. Applications map
an owned `DockVisualPalette` or `DockVisualStyle` into `DockspaceColors`; content
styling remains application-owned.

```sh
cargo fmt -p open-gpui-docking
cargo nextest run -p open-gpui-docking --locked --lib --test-threads=1
cargo nextest run -p open-gpui-docking --locked --all-features --lib --test-threads=1
cargo check -p open-gpui-docking-minimal -p open-gpui-docking-multiviewport --locked --all-targets
cargo check -p open-gpui-devtools --features docking --locked --all-targets
cargo run -p xtask -- scan-public-api
```

CPU renderer tests establish model, ownership and exact correlation behavior.
Physical captured dragging, mixed-DPI movement, OS IME and each foreign backend
retain separate qualification. See [verification](../../docs/verification.md).

Run the shared retained-host CPU workload independently of the legacy/ImGui
comparator with `python tools/benchmark_docking_drag.py --suite shared --json`
from the workspace root. Its TestPlatform and simulated-submission measurements
are labeled separately and do not claim native input or GPU latency. For a quick
debug-profile execution, use `cargo bench --locked -p open-gpui-docking --bench
shared_session_cpu --features test-support --profile dev`.
