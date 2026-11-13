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
`take_native_reports` transfers observations to the application. Read-only
`inspection` returns an owned snapshot or `DockInspectionError::Busy` during a
mutable host borrow; it does not consume reports or drive the runtime. DevTools'
`docking` feature projects that snapshot; it does not enable the old engine.

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
The old native dogfood example and browser unsupported-native-window smoke
retain their own compatibility qualification until replacement scenarios run:

```sh
cargo run -p open-gpui-docking-native --features legacy-docking
```

See [compatibility notes](LEGACY.md), the
[migration guide](../../docs/ui/migration-v0.3.md#default-shared-docking-session),
[ADR 0034](../../docs/adr/0034-shared-dockspace-session-integration.md) and the
[cutover evidence](../../docs/knowledge/engineering/verification/dockspace-default-cutover-20261114.md).
Historical receipts retain source paths at their recorded commits; incumbent
files now live under `src/legacy/`.

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
