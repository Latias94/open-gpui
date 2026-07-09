---
title: DevTools Ecosystem Deepening - Plan
type: feat
date: 2026-07-09
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: ce-plan-bootstrap
execution: code
---

# DevTools Ecosystem Deepening - Plan

## Goal Capsule

Build the next Open GPUI ecosystem layer around first-party observability: command inspection, timeline/event tracing, and layout inspection. The DevTools crate remains a read-only, redacted snapshot surface; source crates remain the runtime authorities and expose only the minimum public facts DevTools needs.

Authority order: current user request, AGENTS.md repository rules, existing public API direction in `docs/plans/2026-07-08-004-feat-devtools-real-probes-dogfood-plan.md`, then this plan. Stop and re-plan only if a required public fact cannot be exposed without making DevTools a runtime owner, or if a verification gate reveals a breaking API conflict outside this plan's scope.

Execution profile: implement on a feature branch, commit meaningful slices, and periodically merge/push `main` when a slice is green. Use subagents for read-only research or review; keep code edits coordinated in one lane because this repository has shared checkout semantics.

## Product Contract

### Summary

Open GPUI already has motion, docking, command, resource, form, and real DevTools probe foundations. The next ecosystem win is to make those systems explain themselves through a small set of first-party DevTools panels that mirror mature UI frameworks: tree inspection, command inspection, layout inspection, and timeline/event inspection.

### Problem Frame

The framework is becoming a cross-platform UI ecosystem, not only a rendering crate. Users need to answer "what is mounted, what command will run, why did this layout look like that, and what changed over time" without attaching a debugger or reading private runtime state. Mature frameworks solve this with observability surfaces: Flutter DevTools has inspector/layout and performance views, React DevTools exposes component/performance tracks, and Dear ImGui ships a built-in metrics/debugger window.

### Requirements

Runtime boundaries:

- R1. DevTools must stay read-only and snapshot-based; it must not mutate command dispatch, layout, animation, docking, resource, or form state.
- R2. Source crates must stay runtime authorities and provide public snapshot DTOs or adapters; DevTools must not reach through private fields.
- R3. All exported string and JSON channels must keep using the existing sanitizer/redaction path for probe ids, node ids, labels, payloads, diagnostics, and custom kind text.

Inspector experience:

- R4. `DevtoolsInspectorState` must expose category-aware projections so UI and tests can reason about command, timeline, layout, and existing snapshot families without reparsing JSON.
- R5. `DevtoolsInspector` must render a compact read-only surface that can distinguish snapshot families and show selected trees, summary metrics, and diagnostics without nested card clutter.
- R6. Filtering and selection must remain deterministic across categories and must select the first visible snapshot when the current selection is filtered away.

Command inspection:

- R7. The gallery DevTools page must dogfood the existing `open_gpui_devtools::command` adapters using real command registry/projection/resolution sample data.
- R8. Command inspection must surface registry entries, keybinding diagnostics/conflicts, and keymap resolution facts as `SnapshotKind::Command`; it must not persist keymaps or invoke commands.

Timeline/event tracing:

- R9. DevTools must add a renderer-neutral timeline/event snapshot model with bounded event collections, stable event ids, kind labels, timestamps or ordering, durations when known, and sanitized payloads.
- R10. Motion frame demand/timeline facts must become the first real timeline producer so the model is exercised by an existing ecosystem crate.
- R11. Timeline snapshots must degrade to stable diagnostics when a runtime source is unavailable; they must not require a tracing subscriber, remote transport, or time-travel runtime.

Layout inspection:

- R12. DevTools must add a layout snapshot model for committed public facts such as bounds, size, scroll viewport, docking presentation/status, and component layout summaries.
- R13. Layout inspection must preserve a single geometry authority by consuming public committed facts from GPUI or owning crates instead of recomputing layout inside DevTools.
- R14. Layout snapshots must be visualization-ready as a tree plus summary payloads, even if the first UI rendering is textual.

Gallery, docs, and cleanup:

- R15. `examples/ui-foundation-gallery` must collect command, timeline, and layout examples through `DevtoolsRegistry`, not static fixture builders.
- R16. Tests must assert the new probe ids, snapshot kinds, redaction behavior, and category projections.
- R17. README and verification docs must document the new feature gates, adapter boundaries, and targeted test commands.
- R18. Obsolete demo-only helpers or duplicate models introduced during implementation must be removed before completion.

### Acceptance Examples

- AE1. Given the gallery DevTools collection, when snapshots are collected, the collection includes command, timeline, and layout probe ids alongside the existing accessibility, form, motion, resource, and theme probes.
- AE2. Given an inspector state with a filter of `command`, when rows are projected, only command rows or rows with command-matching node labels are visible and selection moves to the first visible row.
- AE3. Given a command keybinding sample with a conflict diagnostic, when converted through DevTools adapters, the exported JSON contains a sanitized conflict node and no command execution side effects occur.
- AE4. Given a motion frame demand sample, when converted to timeline, DevTools exposes ordered event nodes with stable ids and sanitized payload.
- AE5. Given a committed scroll viewport or layout sample, when converted to layout, DevTools exposes bounds/size/offset facts as payload data and reports unavailable runtime facts as diagnostics.

### Scope Boundaries

In scope: local read-only DevTools models, feature-gated adapters, GPUI inspector projection/rendering, gallery dogfood, docs, and package-scoped tests.

Out of scope: remote debugging, live mutation tools, time travel, persistent command/keymap editing, tracing subscriber integration, GPU frame capture, screenshot baselines as the primary correctness gate, and private runtime introspection.

### Sources and Research

- Prior repo plan: `docs/plans/2026-07-08-004-feat-devtools-real-probes-dogfood-plan.md`.
- Prior repo memory: `docs/knowledge/engineering/progress/2026-07-08-open-gpui-devtools-real-probes-dogfood.md`.
- Command ecosystem doc: `docs/ui/command-ecosystem.md`.
- Local references: `repo-ref/zed`, `repo-ref/egui`, `repo-ref/imgui`, `repo-ref/tldraw`, `repo-ref/xyflow`, `repo-ref/accesskit`.
- Flutter DevTools inspector and Layout Explorer: https://docs.flutter.dev/tools/devtools/inspector
- Flutter DevTools performance view: https://docs.flutter.dev/tools/devtools/performance
- React DevTools Performance tracks: https://react.dev/reference/dev-tools/react-performance-tracks
- Dear ImGui metrics/debugger source: https://github.com/ocornut/imgui/blob/master/imgui_demo.cpp

## Planning Contract

### Key Technical Decisions

- KTD1. Extend the existing snapshot envelope instead of creating a second DevTools data plane. `SnapshotKind`, `SnapshotTree`, `SnapshotNode`, `SnapshotEnvelope`, and `SnapshotCollection` already encode the stable redacted export shape.
- KTD2. Add narrow DTO/adapters per capability before richer UI. Command already has adapters; timeline and layout should add typed builders that produce the same tree/envelope shape.
- KTD3. Category projection belongs in `DevtoolsInspectorState`, not in GPUI rendering. Tests and non-GPUI consumers need the same read model.
- KTD4. Timeline is event-model-first, not tracing-subscriber-first. The first useful primitive is a bounded, redacted event list that can later be fed by tracing or runtime hooks.
- KTD5. Layout inspection consumes committed public facts. DevTools can display bounds and layout summaries, but GPUI/docking/ui-components remain geometry authorities.
- KTD6. Gallery dogfood is the integration gate. Static fixture builders are allowed only as source-crate sample constructors outside DevTools collection; the DevTools page itself must collect through `DevtoolsRegistry`.
- KTD7. Keep feature gates explicit. `command`, `motion`, `docking`, `gpui`, and `ui-components` remain opt-in so downstream consumers can pay only for the adapters they use.

### High-Level Technical Design

```text
source crate facts
  command registry/projection/resolution
  motion frame demand / timeline
  gpui committed scroll/layout facts
  docking presentation/status facts
          |
          v
feature-gated DevTools adapters
  command.rs
  timeline.rs
  layout.rs
          |
          v
SnapshotEnvelope + SnapshotKind + SnapshotTree
          |
          v
DevtoolsInspectorState projections
  rows
  category summaries
  selected snapshot JSON
          |
          v
GPUI DevtoolsInspector + gallery dogfood
```

### Sequencing

1. Expand inspector read projections first because every later adapter needs deterministic category behavior.
2. Land command gallery dogfood next because the command adapters already exist and prove the projection path quickly.
3. Add timeline DTOs and motion-backed adapter after command because it introduces a new model but uses existing motion facts.
4. Add layout DTOs/adapters after timeline because it may need tighter public-fact boundaries with GPUI/docking/ui-components.
5. Polish `DevtoolsInspector`, docs, and verification after all data sources are present.

### Risks and Dependencies

- RD1. Layout facts may tempt private runtime reach-through. Mitigation: expose narrow public snapshots in the owning crate or emit an unavailable diagnostic.
- RD2. Broad Windows `nextest` runs can hit local resource limits. Mitigation: verify package-scoped slices first, then run broader gates when code is stable.
- RD3. Public API additions affect v0.3 surface. Mitigation: keep new types small, documented, and feature-gated; run public API scan if touched paths participate in release checks.
- RD4. Timeline naming can become too tracing-specific. Mitigation: name the model around DevTools timeline events and spans, not subscriber internals.

## Implementation Units

### U1. Inspector Category Projection

Goal: make `DevtoolsInspectorState` expose stable category-aware rows and summaries for command, timeline, layout, diagnostics, and existing snapshot kinds.

Requirements: R1, R3, R4, R6, R16.

Files:

- `crates/devtools/src/inspector.rs`
- `crates/devtools/src/snapshot.rs`
- `crates/devtools/src/lib.rs`
- `crates/devtools/tests/inspector_contracts.rs`

Approach:

- Add `DevtoolsSnapshotCategory` or equivalent public enum with labels derived from `SnapshotKind`.
- Add row category fields and a category summary projection that counts snapshots, root nodes, total nodes, diagnostics, and redactions.
- Keep `snapshot_rows()` backward-compatible if possible, but allow a breaking row shape if the new projection is cleaner and tests/docs are updated.
- Add tests for category derivation, filtering, selection movement, JSON export, and sanitizer preservation.

Test scenarios:

- Empty collection returns no rows and no selected snapshot.
- Mixed command/timeline/layout/custom snapshots produce stable category labels.
- Filtering by category or nested node label updates rows and selected probe.

Verification:

- `cargo nextest run -p open-gpui-devtools inspector --no-fail-fast --locked`

### U2. Command Inspector Gallery Dogfood

Goal: register command registry, keybinding projection, and keymap resolution snapshots in the gallery DevTools page through the existing command adapters.

Requirements: R1, R3, R7, R8, R15, R16.

Files:

- `examples/ui-foundation-gallery/Cargo.toml`
- `examples/ui-foundation-gallery/src/pages/devtools.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery/devtools_contracts.rs`
- `crates/devtools/tests/command_adapters.rs`
- `crates/devtools/README.md`

Approach:

- Enable the `command` DevTools feature for the gallery crate if needed.
- Build deterministic command sample data from `open_gpui_command` and `open_gpui_ui_components` public snapshots.
- Register command probes in `devtools_gallery_collection()` through `DevtoolsRegistry`.
- Update gallery tests to assert command probe ids, command kind labels, conflict/diagnostic payloads, and lack of static demo snapshot helpers.

Test scenarios:

- Gallery collection includes command snapshots in deterministic order.
- Command JSON includes registry/projection/resolution facts and no raw secret text.
- DevTools command adapter tests still pass under `--features command`.

Verification:

- `cargo check -p open-gpui-devtools --features command --tests --locked`
- `cargo nextest run -p open-gpui-devtools --features command --test command_adapters --no-fail-fast --locked`
- `cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked`

### U3. Timeline Event Snapshot Foundation

Goal: introduce a renderer-neutral DevTools timeline/event model and connect motion frame demand as the first real producer.

Requirements: R1, R2, R3, R9, R10, R11, R15, R16.

Files:

- `crates/devtools/Cargo.toml`
- `crates/devtools/src/lib.rs`
- `crates/devtools/src/snapshot.rs`
- `crates/devtools/src/timeline.rs`
- `crates/devtools/src/motion.rs`
- `crates/devtools/tests/timeline_adapters.rs`
- `examples/ui-foundation-gallery/src/pages/devtools.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery/devtools_contracts.rs`

Approach:

- Add `SnapshotKind::Timeline` with stable serialization and label.
- Add `TimelineEventSnapshot`, `TimelineSpanSnapshot`, or a minimal equivalent with stable id, label, category, order/timestamp, optional duration, and sanitized payload.
- Add builders that cap event counts and summarize omitted events.
- Add a motion-backed adapter that maps `MotionFrameDemand` or existing motion timeline facts into timeline nodes.
- Register a timeline probe in the gallery and test deterministic output.

Test scenarios:

- Timeline builder sanitizes labels and payloads.
- Event collections are bounded and report omitted counts.
- Motion frame demand yields a timeline snapshot without requiring a live renderer.

Verification:

- `cargo check -p open-gpui-devtools --features motion --tests --locked`
- `cargo nextest run -p open-gpui-devtools --features motion timeline --no-fail-fast --locked`
- `cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked`

### U4. Layout Inspection Snapshot Foundation

Goal: add layout inspection DTOs and adapters over committed public facts, starting with GPUI scroll viewport facts and layout-ready summaries.

Requirements: R1, R2, R3, R12, R13, R14, R15, R16.

Files:

- `crates/devtools/Cargo.toml`
- `crates/devtools/src/lib.rs`
- `crates/devtools/src/snapshot.rs`
- `crates/devtools/src/layout.rs`
- `crates/devtools/src/gpui.rs`
- `crates/devtools/tests/layout_adapters.rs`
- `examples/ui-foundation-gallery/src/pages/devtools.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery/devtools_contracts.rs`

Approach:

- Add `SnapshotKind::Layout` with stable serialization and label.
- Add layout summary DTOs for bounds, size, offset, scroll extent, and child summaries using plain serializable values.
- Reuse or move scroll viewport snapshot conversion into layout adapters so scroll remains available while layout has its own category.
- Emit stable unavailable diagnostics for missing live layout sources.
- Register a deterministic layout probe in gallery using committed/sample public facts.

Test scenarios:

- Layout adapter serializes bounds/size/offset facts without private runtime data.
- Unavailable runtime facts produce `runtime.unavailable` diagnostics.
- Gallery collection includes layout and preserves existing scroll/docking diagnostics until those runtimes are mounted.

Verification:

- `cargo check -p open-gpui-devtools --features gpui --tests --locked`
- `cargo nextest run -p open-gpui-devtools --features gpui layout --no-fail-fast --locked`
- `cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked`

### U5. Inspector UI Facets, Docs, and Cleanup

Goal: make the GPUI inspector visibly useful for the new ecosystem probes and document the supported extension pattern.

Requirements: R4, R5, R15, R16, R17, R18.

Files:

- `crates/devtools/src/gpui.rs`
- `crates/devtools/README.md`
- `README.md`
- `docs/verification.md`
- `examples/ui-foundation-gallery/src/pages/devtools.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery/devtools_contracts.rs`

Approach:

- Render category summaries above or beside the row list with compact labels and counts.
- Keep the selected snapshot tree as the primary detail pane; add family-specific summary text only when it comes from `DevtoolsInspectorState`.
- Avoid cards-inside-cards; use full-width bands, compact rows, and debug selectors for automated tests.
- Update docs with command/timeline/layout adapter examples and targeted verification commands.
- Delete duplicate helper code or abandoned experiment files from the diff before final verification.

Test scenarios:

- GPUI inspector exposes debug selectors for root, category summaries, rows, selected detail, and diagnostics.
- Docs mention feature gates and the read-only adapter boundary.
- Source search confirms no new static DevTools gallery snapshot builders.

Verification:

- `cargo check -p open-gpui-devtools --all-features --tests --locked`
- `cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked`
- `cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked`

## Verification Contract

Required local gates before declaring the plan complete:

- `cargo fmt -p open-gpui-devtools -p open-gpui-ui-foundation-gallery`
- `cargo check -p open-gpui-devtools --all-features --tests --locked`
- `cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked`
- `cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked`
- `cargo run -p xtask -- scan-doc-links`

Conditional gates:

- Run `cargo run -p xtask -- scan-public-api --check` if new public exports affect v0.3 API inventory.
- Run package-scoped `nextest` with `-j 1` if Windows resource limits cause unrelated broad-run failures.
- Manual gallery smoke via `cargo run -p open-gpui-ui-foundation-gallery -- --page devtools` is required only if the GPUI inspector render code changes in a way tests cannot observe.

Quality gates:

- Run a simplification pass after U3 or U4 if timeline/layout DTOs duplicate structure.
- Run a code-review pass before the final merge to catch sanitizer, feature-gate, and public API mistakes.
- Record major subagent findings, verification results, and final state in `docs/knowledge/engineering/` rather than mutating this plan.

## Definition of Done

Global done criteria:

- Every non-deferred requirement R1-R18 is implemented or explicitly covered by a stable diagnostic boundary.
- Command, timeline, and layout probes are collected through `DevtoolsRegistry` in the gallery.
- Inspector state exposes category-aware projections and tests cover filtering/selection behavior.
- New public DevTools types have docs and compile under `#![warn(missing_docs)]`.
- Sanitizer coverage applies to all new label, id, diagnostic, and payload channels.
- Required verification commands pass, or any non-code environmental failure is documented with the exact command and reason.
- Dead-end code, duplicate adapters, and static gallery demo helpers introduced during the work are removed.
- Meaningful commits exist for stable slices, and `main` is merged/pushed after green landing points.

Per-unit done criteria:

- U1 is done when inspector projection tests pass and existing callers either compile unchanged or are updated intentionally.
- U2 is done when gallery command dogfood is registry-backed and command adapter tests pass with `--features command`.
- U3 is done when timeline DTO/adapters are documented, bounded, sanitized, and motion-backed tests pass.
- U4 is done when layout DTO/adapters expose committed public facts and unavailable runtime diagnostics are tested.
- U5 is done when the GPUI inspector renders category summaries, docs are updated, and final gates pass.

## Appendix

### Deferred Ideas

- Remote DevTools protocol.
- Persistent user-editable keybinding UI inside DevTools.
- Time-travel playback.
- GPU frame capture.
- Screenshot-based visual regression as the primary DevTools gate.

