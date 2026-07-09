---
type: Verification Evidence
title: DevTools runtime ecosystem provider controller verification
timestamp: 2026-07-09T06:54:18Z
git_branch: main
related_plan: ../../../plans/2026-07-09-003-refactor-devtools-runtime-ecosystem-plan.md
---

# DevTools Runtime Ecosystem Provider Controller Verification

Date: 2026-07-09

## Summary

Implemented the next DevTools runtime ecosystem slice from `docs/plans/2026-07-09-003-refactor-devtools-runtime-ecosystem-plan.md`.

The architecture now has:

- `DevtoolsCaptureProvider` and closure-backed `CaptureProvider` for capture-first producers.
- `DevtoolsRegistry` support for legacy probes and capture providers, with provider failures reported as diagnostics.
- Provider-only capture collection without injecting an empty legacy `app` target.
- Scoped `DevtoolsEventRecorder` metadata: scope id/label, retained count, omitted count, capacity, next sequence, export, and drain.
- Capture diagnostics for events that reference missing targets or domains.
- Inspector state commands for target/domain/event movement, filter clearing, selected-detail copy/export, whole-capture export, and explicit active detail kind.
- `DevtoolsInspectorController`, a GPUI stateful inspector entity with row click handlers, keyboard selection commands, copy/export feedback state, and clipboard writes.
- Gallery DevTools dogfood through a capture provider and scoped event recorder.
- Docking runtime capture provider constructor over public `DockViewportRuntimeStatus`.

## Decisions

- Legacy `collect()` remains snapshot-only. New capture providers participate only in `collect_capture()`.
- Provider ids share the same identity namespace as legacy probes to avoid ambiguous diagnostics.
- Empty registries do not synthesize an `app` target. Legacy probe collections still project through `DevtoolsCapture::from_snapshot_collection`.
- Event lists are search-filtered timeline rows, not hidden behind the selected target/domain. Selecting an event still synchronizes target/domain context.
- `DevtoolsEventRecorder::clear()` and `drain()` do not reset `next_sequence`; sequence remains an append-time lifecycle order.
- GPUI interactive behavior is owned by `DevtoolsInspectorController`. The old `DevtoolsInspector` remains the static compatibility element.
- Docking DevTools continues to consume only public docking runtime status records.

## Verification

Focused gates run during the implementation:

- `cargo check -p open-gpui-devtools --no-default-features --tests --locked`
- `cargo check -p open-gpui-devtools --features gpui --tests --locked`
- `cargo check -p open-gpui-devtools --features docking --tests --locked`
- `cargo nextest run -p open-gpui-devtools --test snapshot_contracts --no-fail-fast --locked`
- `cargo nextest run -p open-gpui-devtools --test event_recorder_contracts --no-fail-fast --locked`
- `cargo nextest run -p open-gpui-devtools --test target_domain_contracts --test event_recorder_contracts --no-fail-fast --locked`
- `cargo nextest run -p open-gpui-devtools --test inspector_contracts --no-fail-fast --locked`
- `cargo nextest run -p open-gpui-devtools --features docking framework_adapters_convert_docking_runtime_status --no-fail-fast --locked`
- `cargo check -p open-gpui-ui-foundation-gallery --tests --locked`
- `cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked`
- `cargo check -p open-gpui-docking-native --tests --locked`
- `git diff --check`

## Follow-Up Risk

The GPUI controller includes real click handlers and clipboard actions, but the gallery visual test harness did not reliably dispatch simulated clicks into this entity subtree during this run. State command contracts cover selection/copy/export behavior, and package checks cover the GPUI wiring. A future dogfood pass should add a harness-level click test once the event dispatch route is understood.

Resolved later on 2026-07-09 by `docs/knowledge/engineering/verification/2026-07-09-devtools-inspector-click-dogfood.md`, which adds a gallery-level click smoke that verifies rendered row/action selectors update the stateful controller entity.
