# open-gpui-devtools

Read-only inspection snapshots and local devtools surfaces for Open GPUI applications.

This crate owns the devtools probe and snapshot vocabulary. Default builds stay renderer-neutral and
do not depend on GPUI. Optional features connect specialized panels and GPUI UI surfaces later:

- `form` for `open-gpui-form` snapshots.
- `resource` for `open-gpui-resource` snapshots.
- `docking` for docking snapshots.
- `motion` for motion snapshots.
- `gpui` for native inspector UI elements.

The first contract is read-only. Devtools can collect, filter, copy, and export snapshots; runtime
mutation and live property editing are intentionally out of scope for the initial surface.

## Public Contract

- `DevtoolsProbe` is implemented by app-owned snapshot providers.
- `DevtoolsRegistry` collects snapshots and converts probe failures into diagnostics instead of
  panicking.
- `SnapshotEnvelope`, `SnapshotTree`, `SnapshotNode`, and `SnapshotKind` are serializable DTOs for
  tests, gallery samples, and downstream tools.
- `SnapshotRedactionSummary` records what was removed before a snapshot reached devtools.
- `DevtoolsInspectorState` provides filter, selection, row projection, diagnostics, and JSON export
  without requiring a GPUI window.
- `DevtoolsInspector` is available only with the `gpui` feature and renders a read-only local
  inspector with existing UI components.

## Basic Use

```rust
use open_gpui_devtools::{
    DevtoolsProbe, DevtoolsRegistry, ProbeId, ProbeSnapshotError, SnapshotEnvelope, SnapshotKind,
    SnapshotNode, SnapshotTree,
};

struct ThemeProbe {
    id: ProbeId,
}

impl DevtoolsProbe for ThemeProbe {
    fn id(&self) -> &ProbeId {
        &self.id
    }

    fn snapshot(&self) -> Result<SnapshotEnvelope, ProbeSnapshotError> {
        Ok(SnapshotEnvelope::new(
            self.id.clone(),
            SnapshotKind::Theme,
            SnapshotTree::new([SnapshotNode::new("theme", "Theme tokens")]),
        ))
    }
}

let mut registry = DevtoolsRegistry::default();
registry.register(ThemeProbe {
    id: ProbeId::new("theme")?,
})?;
let collection = registry.collect();
assert_eq!(collection.snapshots.len(), 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Redaction Policy

Devtools should inspect framework facts, not retain secrets. Form and resource integrations should
send redacted snapshots, then use `SnapshotRedactionSummary` to explain how many values or paths
were hidden. The inspector preserves that summary during selection, copy, and JSON export.

## Verification

For focused devtools changes, run:

```sh
cargo fmt -p open-gpui-devtools
cargo check -p open-gpui-devtools --tests --locked
cargo check -p open-gpui-devtools --features gpui --tests --locked
cargo nextest run -p open-gpui-devtools --no-fail-fast --locked
```

When changing the gallery inspector surface, also run:

```sh
cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked
cargo run -p open-gpui-ui-foundation-gallery -- --page devtools
```
