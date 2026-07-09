# open-gpui-devtools

Read-only inspection snapshots and local devtools surfaces for Open GPUI applications.

This crate owns the devtools probe and snapshot vocabulary. Default builds stay renderer-neutral and
do not depend on GPUI. Optional features connect specialized panels and GPUI UI surfaces later:

- `form` for `open-gpui-form` snapshots.
- `resource` for `open-gpui-resource` snapshots.
- `ui-components` for theme and accessibility contract snapshots.
- `motion` for `open-gpui-motion` frame-demand snapshots.
- `docking` for `open-gpui-docking` runtime diagnostics.
- `gpui` for core GPUI scroll snapshots and native inspector UI elements. It also enables
  `ui-components` because the inspector UI uses component primitives.

The first contract is read-only. Devtools can collect, filter, copy, and export snapshots; runtime
mutation and live property editing are intentionally out of scope for the initial surface.

## Public Contract

- `DevtoolsProbe` is implemented by app-owned snapshot providers.
- `DevtoolsRegistry` collects snapshots and converts probe failures into diagnostics instead of
  panicking.
- `SnapshotEnvelope`, `SnapshotTree`, `SnapshotNode`, and `SnapshotKind` are serializable DTOs for
  tests, gallery samples, and downstream tools.
- `SnapshotRedactionSummary` records what was removed before a snapshot reached devtools.
- `adapters` contains shared helpers for stable node ids, sanitized payloads, and diagnostic-safe
  labels.
- `form` and `resource` expose feature-gated first-party adapters that consume public headless
  snapshots without making source crates depend on devtools.
- `DevtoolsInspectorState` provides filter, selection, row projection, diagnostics, and JSON export
  without requiring a GPUI window.
- `DevtoolsInspector` is available only with the `gpui` feature and renders a read-only local
  inspector with existing UI components.

## Basic Use

```rust
use open_gpui_devtools::{
    adapters::snapshot_node_with_payload, DevtoolsRegistry, SnapshotKind, SnapshotProbeSnapshot,
    SnapshotTree,
};

let mut registry = DevtoolsRegistry::default();
registry.register_snapshot_probe("theme", SnapshotKind::Theme, || {
    Ok(SnapshotProbeSnapshot::new(SnapshotTree::new([
        snapshot_node_with_payload(
            ["theme"],
            "Theme tokens",
            serde_json::json!({ "mode": "dark" }),
        ),
    ])))
})?;
let collection = registry.collect();
assert_eq!(collection.snapshots.len(), 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

With the `form` feature enabled, convert a public form snapshot directly:

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# #[cfg(feature = "form")]
# {
use open_gpui_devtools::{form, ProbeId};
use open_gpui_form::FormSnapshot;

let snapshot = FormSnapshot::default();
let envelope = form::form_snapshot_envelope(ProbeId::new("form")?, &snapshot);
assert_eq!(envelope.probe_id.as_str(), "form");
# }
# Ok(())
# }
```

With the `resource` feature enabled, pass query, mutation, and paginated snapshots through one
resource adapter:

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# #[cfg(feature = "resource")]
# {
use open_gpui_devtools::{resource, ProbeId};
use open_gpui_resource::{QueryKey, ResourceSnapshot, ResourceStatus};

let snapshot = ResourceSnapshot {
    key: QueryKey::new(["projects"])?,
    status: ResourceStatus::Success,
    data: None,
    error: None,
    observer_count: 1,
    fetch_attempts: 1,
};
let envelope = resource::resource_snapshot_envelope(
    ProbeId::new("resource")?,
    [&snapshot],
    [],
    [],
);
assert_eq!(envelope.redaction.redacted_values, 0);
# }
# Ok(())
# }
```

## Redaction Policy

Devtools should inspect framework facts, not retain secrets. Adapter helpers sanitize probe ids,
node ids, labels, payload strings, redaction notes, diagnostics, and custom snapshot-kind labels by
default. Form and resource adapters derive `SnapshotRedactionSummary` from
`RedactedValue::Redacted` and `RedactedResourceValue::Redacted` values instead of trusting callers
to count manually. Values marked as JSON by the source snapshot are the only values eligible for
exposure in payloads.

## Verification

For focused devtools changes, run:

```sh
cargo fmt -p open-gpui-devtools
cargo check -p open-gpui-devtools --tests --locked
cargo check -p open-gpui-devtools --features form,resource --tests --locked
cargo check -p open-gpui-devtools --features gpui,motion,docking --tests --locked
cargo nextest run -p open-gpui-devtools --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features form,resource form_resource_adapters --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features gpui,motion,docking framework_adapters --no-fail-fast --locked
```

When changing the gallery inspector surface, also run:

```sh
cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked
cargo run -p open-gpui-ui-foundation-gallery -- --page devtools
```
