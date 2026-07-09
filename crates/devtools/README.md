# open-gpui-devtools

Read-only inspection snapshots and local devtools surfaces for Open GPUI applications.

This crate owns the devtools target, domain, event, probe, and snapshot vocabulary. Default builds
stay renderer-neutral and do not depend on GPUI. Optional features connect specialized panels and
GPUI UI surfaces later:

- `form` for `open-gpui-form` snapshots.
- `resource` for `open-gpui-resource` snapshots.
- `ui-components` for theme and accessibility contract snapshots.
- `motion` for `open-gpui-motion` frame-demand snapshots.
- `docking` for `open-gpui-docking` runtime diagnostics.
- `command` for `open-gpui-command` registry, keybinding, and keymap-resolution snapshots.
- `gpui` for core GPUI scroll snapshots and native inspector UI elements. It also enables
  `ui-components` because the inspector UI uses component primitives.

The first contract is read-only. Devtools can collect, filter, copy, and export snapshots; runtime
mutation and live property editing are intentionally out of scope for the initial surface.

## Public Contract

- `DevtoolsProbe` is implemented by app-owned legacy snapshot providers.
- `DevtoolsCaptureProvider` is implemented by app-owned rich capture providers that contribute
  targets, domains, events, compatibility snapshots, and diagnostics.
- `DevtoolsRegistry` collects legacy probes and capture providers, preserves `collect()` as the
  snapshot-only compatibility path, and converts collection failures into diagnostics instead of
  panicking.
- `DevtoolsCapture` is the rich local protocol output. It contains a target tree, domain outputs,
  bounded event records, compatibility snapshots, and diagnostics.
- `DevtoolsSession` wraps a registry with bounded local history, monotonic generations, and
  `DevtoolsSessionFrame` records. Refreshing a session collects a sanitized capture and computes
  a sanitized diff from the previous retained frame.
- `DevtoolsCaptureDiff` compares sanitized targets, domains, events, snapshots, and diagnostics.
  Redaction-induced identity collisions are explicit diff rows and never overwrite another row.
- `DevtoolsSessionExport` is the offline replay/import envelope. Replay means loading already
  captured local frames into inspector state after schema, protocol, history, size, and event-count
  validation. It is not a remote transport and does not mutate application state.
- `DevtoolsTargetSnapshot`, `DevtoolsDomainSnapshot`, and `DevtoolsEventRecord` are serializable
  DTOs for target/domain/event inspection. They are local read-only facts, not a remote debugging
  bridge or Chrome DevTools Protocol clone.
- `SnapshotEnvelope`, `SnapshotTree`, `SnapshotNode`, and `SnapshotKind` remain the legacy-compatible
  snapshot DTOs for tests, gallery samples, and downstream tools. `SnapshotKind::Command`,
  `SnapshotKind::Timeline`, and `SnapshotKind::Layout` are first-class observability families.
- `SnapshotRedactionSummary` records what was removed before a snapshot reached devtools.
- `adapters` contains shared helpers for stable node ids, sanitized payloads, and diagnostic-safe
  labels.
- `form` and `resource` expose feature-gated first-party adapters that consume public headless
  snapshots without making source crates depend on devtools. They also expose data-domain capture
  helpers.
- `command` exposes feature-gated adapters for command registries, keybinding projections,
  projection diagnostics, shortcut conflicts, and keymap resolution. It also exposes command-domain
  capture helpers.
- `DevtoolsEventRecorder` is a scoped, bounded local recorder. It exports retained count, omitted
  count, capacity, next sequence, scope id, and scope label; `drain()` clears retained events
  without resetting the append sequence.
- `DevtoolsEventIdentity` is the event-instance identity for selection, debug selectors, diff rows,
  and replay frames. Event `sequence` remains display metadata only; callers should select events
  with `DevtoolsInspectorState::select_event_identity` and read
  `DevtoolsInspectorState::selected_event_identity`.
- `timeline` exposes renderer-neutral bounded event snapshots and timeline-domain capture helpers.
- `layout` exposes renderer-neutral committed geometry snapshots and layout-domain capture helpers.
- `docking` exposes capture-first runtime diagnostics, structured multi-viewport inspection rows,
  explicit capability diagnostics, and a capture provider constructor when public docking status
  records are available.
- `gpui` exposes metadata-only `GpuiRuntimeSnapshot` adapters for app/window/focus/input/frame and
  scroll facts. Raw user input, clipboard payloads, editable text values, unredacted titles, and
  accessibility labels do not belong in runtime metadata.
- `DevtoolsInspectorState` provides filter, selection, category summaries, row projection,
  session-frame loading, diff rows, target/domain/event navigation, diagnostics, selected-detail
  JSON, and legacy snapshot export without requiring a GPUI window.
- `DevtoolsInspector` is available only with the `gpui` feature and renders a static read-only
  local inspector with existing UI components.
- `DevtoolsInspectorController` is available only with the `gpui` feature and owns interactive
  inspector state, row selection, keyboard navigation, copy/export feedback, and clipboard writes.

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
let capture = registry.collect_capture();
assert_eq!(capture.domains.len(), 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Capture-first producers can register alongside or instead of legacy probes:

```rust
use open_gpui_devtools::{
    DevtoolsCapture, DevtoolsRegistry, DevtoolsTargetSnapshot, DevtoolsTargetId,
    DevtoolsTargetKind, DevtoolsTargetTree,
};

let mut registry = DevtoolsRegistry::default();
registry.register_capture_provider_fn("runtime.commands", || {
    let target = DevtoolsTargetSnapshot::new(
        DevtoolsTargetId::new("runtime.commands"),
        DevtoolsTargetKind::Runtime,
        "Command runtime",
    );
    Ok(DevtoolsCapture::new(
        DevtoolsTargetTree::new([target]),
        [],
        [],
        [],
        [],
    ))
})?;
assert_eq!(registry.collect_capture().targets.targets.len(), 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Target/Domain/Event Capture

New integrations should prefer `DevtoolsCapture` when they need inspector navigation. A capture is
still local and read-only:

- Targets identify the inspected producer, such as an application, runtime subsystem, viewport, or
  legacy probe.
- Domains group facts by concern, such as command, layout, timeline, data, docking, or motion.
- Events are bounded append-time records for recent local activity. Older records are omitted by the
  recorder capacity instead of growing without limit. Scopes make application, window, or runtime
  sessions explicit without requiring a global event bus.
- `capture.snapshot_collection()` keeps old snapshot consumers working while new inspectors use
  `DevtoolsInspectorState::from_capture(capture)`.

Feature-gated adapters provide capture helpers such as `command_registry_capture`, `form_capture`,
`resource_capture`, `layout_capture`, and `timeline_capture`. The GPUI inspector consumes the same
state model but does not mutate application state.

Event rows are identity-first. Use the row identity for selection and keep `sequence` for display
only:

```rust
use open_gpui_devtools::{DevtoolsCapture, DevtoolsInspectorState};

let state = DevtoolsInspectorState::from_capture(DevtoolsCapture::default());
let next = state.event_rows().first().map(|row| row.event_identity.clone());
if let Some(identity) = next {
    let state = state.select_event_identity(&identity)?;
    assert_eq!(state.selected_event_identity(), Some(&identity));
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Sessions, Diffs, And Replay

Use `DevtoolsSession` when an inspector needs to answer what changed after a local runtime action.
The session owns only a local registry and a bounded in-memory frame history:

```rust
use open_gpui_devtools::{DevtoolsRegistry, DevtoolsSession};

let registry = DevtoolsRegistry::default();
let mut session = DevtoolsSession::new("local.devtools", registry).with_history_limit(4);
let first = session.refresh()?;
let second = session.refresh()?;
assert_eq!(first.generation, 1);
assert_eq!(second.previous_generation, Some(1));
assert!(second.diff_from_previous.is_some());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Session export/import is an offline replay path for already-sanitized local frames. Import validates
schema, protocol, history bounds, JSON size, and per-frame event count before rebuilding canonical
diffs. It deliberately has no network transport, no remote debugging protocol, and no mutation API.

## Minimal App-Author Workbench

An application-owned DevTools workbench should keep runtime authority in the app and let DevTools
own only sanitized frames and inspector state:

1. Build a `DevtoolsRegistry` from legacy probes, capture providers, or narrow app-owned DTOs.
2. Wrap it in `DevtoolsWorkbench::new(...).with_history_limit(...)`.
3. Call `refresh()` from an explicit user action or test helper.
4. Read `workbench.inspector_state()` or update an existing
   `DevtoolsInspectorController` with `update_session_frame(frame, cx)`.
5. Read `workbench.diff_state_label()`, `diff_row_count()`, or `export()` for sanitized change
   summaries.

With the `gpui` feature, Gallery and docking-native use this same pattern: the shell/example owns
the `DevtoolsWorkbench`, app code supplies allowlisted runtime facts, and the controller is only the
local inspector view. `DevtoolsWorkbench` is renderer-neutral; it does not mutate app state or own a
GPUI controller.

```rust
use open_gpui_devtools::{DevtoolsRegistry, DevtoolsWorkbench};

let registry = DevtoolsRegistry::default();
let mut workbench = DevtoolsWorkbench::new("app.devtools", registry).with_history_limit(8);
let frame = workbench.refresh()?;
let state = workbench.inspector_state();

assert_eq!(frame.generation, 1);
assert_eq!(state.session_frame().map(|frame| frame.generation), Some(1));
assert_eq!(workbench.diff_state_label(), "no-previous-frame");
# Ok::<(), Box<dyn std::error::Error>>(())
```

With `gpui`, update the controller from an explicit action or test helper, not during render:

```rust,ignore
let frame = workbench.refresh()?;

inspector.update(cx, |inspector, cx| {
    inspector.update_session_frame(frame, cx);
});
```

## Headless Reports and CLI Diagnostics

The primary automation surface is the artifact contract, not the GUI viewer. `DevtoolsReport`
summarizes a `DevtoolsCapture`, `DevtoolsSessionFrame`, or `DevtoolsSessionExport` into
`open-gpui-devtools-report/v1`: source metadata, target/domain/event/snapshot counts, diff counts,
redaction counts, and severity-ranked findings with stable ids and recommendations.

```rust
# use open_gpui_devtools::{
#     DevtoolsCapture, DevtoolsDomainSnapshot, DevtoolsEventRecord, DevtoolsReport,
#     DevtoolsTargetId, DevtoolsTargetKind, DevtoolsTargetSnapshot, DevtoolsTargetTree,
#     SnapshotDiagnostic, SnapshotEnvelope,
# };
let capture = DevtoolsCapture::new(
    DevtoolsTargetTree::new([DevtoolsTargetSnapshot::new(
        DevtoolsTargetId::new("app"),
        DevtoolsTargetKind::App,
        "App",
    )]),
    Vec::<DevtoolsDomainSnapshot>::new(),
    Vec::<DevtoolsEventRecord>::new(),
    Vec::<SnapshotEnvelope>::new(),
    Vec::<SnapshotDiagnostic>::new(),
);
let report = DevtoolsReport::from_capture(&capture);

assert_eq!(report.schema_version, "open-gpui-devtools-report/v1");
assert_eq!(report.summary.target_count, 1);
assert_eq!(report.summary.finding_count, 0);
```

`xtask devtools` consumes report, session export, or capture JSON artifacts without launching a GUI:

```sh
cargo run -p xtask -- devtools report --input devtools-session.json --format json
cargo run -p xtask -- devtools diagnose --input devtools-session.json --format markdown --fail-on warning
cargo run -p xtask -- devtools diff --before before-capture.json --after after-capture.json --format markdown
cargo run -p xtask -- devtools stream --input devtools-session.json --format jsonl --interval-ms 50
```

Artifact reads are non-blocking by default: missing or partial inputs fail fast. Add
`--timeout-ms <n>` to poll for a producer that is writing the artifact, and tune polling with
`--poll-ms <n>`. `stream` flushes one report record per retained frame so command-line consumers can
process JSONL incrementally.

Application-owned workbenches can also write schema-versioned artifact records for headless
consumers. The application still decides when to refresh runtime facts; DevTools only wraps and
writes already-sanitized captures, sessions, or reports:

```rust
# use open_gpui_devtools::{
#     DevtoolsArtifact, DevtoolsArtifactFileMode, DevtoolsArtifactFileSink,
#     DevtoolsArtifactMetadata, DevtoolsArtifactRecord, DevtoolsArtifactSink, DevtoolsCapture,
#     DevtoolsReport,
# };
let capture = DevtoolsCapture::default();
let report = DevtoolsReport::from_capture(&capture);
let record = DevtoolsArtifactRecord::new(
    DevtoolsArtifactMetadata::new("my-app.devtools")
        .scenario_id("startup")
        .sequence(1)
        .flush_reason("manual-refresh"),
    DevtoolsArtifact::report(&report),
);

let output_path = std::env::temp_dir().join("open-gpui-devtools-latest-report.json");
let mut sink = DevtoolsArtifactFileSink::new(
    &output_path,
    DevtoolsArtifactFileMode::ReplaceAtomic,
);
sink.write_record(&record)?;
std::fs::remove_file(output_path)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Use `DevtoolsArtifactFileMode::AppendJsonl` or `DevtoolsArtifactJsonlSink` when a caller wants one
flushed JSON record per refresh. Artifact records use
`open-gpui-devtools-artifact-record/v1` and carry sanitized producer/scenario metadata, sequence,
generation/session hints when available, flush reason, optional timestamp, and redaction counts.

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
cargo check -p open-gpui-devtools --features command --tests --locked
cargo check -p open-gpui-devtools --features motion --tests --locked
cargo check -p open-gpui-devtools --features gpui --tests --locked
cargo check -p open-gpui-devtools --features form,resource --tests --locked
cargo check -p open-gpui-devtools --features gpui,motion,docking --tests --locked
cargo check -p open-gpui-devtools --all-features --tests --locked
cargo nextest run -p open-gpui-devtools --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --test inspector_contracts --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --test session_contracts --test diff_contracts --test report_contracts --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features command --test command_adapters --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features motion timeline --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features gpui layout --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features form,resource form_resource_adapters --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features gpui,motion,docking framework_adapters --no-fail-fast --locked
cargo nextest run -p open-gpui-devtools --features docking --test docking_runtime_contracts --no-fail-fast --locked
cargo check -p xtask --locked
cargo run -p xtask -- devtools --help
```

When changing the gallery inspector surface, also run:

```sh
cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked
cargo check -p open-gpui-docking-native --tests --locked
cargo nextest run -p open-gpui-docking-native runtime_status_panel_exports_devtools_dogfood_capture --no-fail-fast --locked
cargo run -p open-gpui-ui-foundation-gallery -- --page devtools
```

The gallery DevTools page dogfoods the session path through `devtools_gallery_session_frame()` and
keeps `devtools_gallery_capture()` plus `devtools_gallery_collection()` as compatibility views. The
native docking example dogfoods `docking_runtime_inspection()` over real runtime status. Keep future
gallery probes registry-backed or capture-backed; do not reintroduce static DevTools snapshot
builders for the page itself.
