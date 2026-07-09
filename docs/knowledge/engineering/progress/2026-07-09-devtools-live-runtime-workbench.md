---
type: Work Progress
title: DevTools live runtime workbench implementation
timestamp: 2026-07-09
status: implemented
related_plan: ../../../plans/2026-07-09-004-feat-devtools-live-runtime-workbench-plan.md
tags:
  - devtools
  - session
  - docking
  - gpui
  - gallery
---

# Summary

Implemented the DevTools live runtime workbench plan through five reviewable commits on `main`:

- `cd8a7868` documented the implementation-ready plan.
- `6bd0b10a` added session frames, bounded history, import validation, and sanitized capture diffing.
- `934239ec` added session-frame/diff projections to inspector state and the GPUI inspector controller.
- `6f72b9c7` added docking runtime inspection rows, explicit platform capability diagnostics, and GPUI runtime metadata capture.
- `e7c8e6cd` switched Gallery DevTools to a deterministic two-refresh session workbench and added docking-native dogfood capture.
- The merge with remote `84fccaf9` added Gallery click dogfood coverage; resolving it moved GPUI event row debug selectors from sequence-only ids to `DevtoolsEventIdentity::as_key()` so same-sequence events across scopes remain clickable.

# Durable Decisions

- Replay means local/offline import of already captured frames after schema, protocol, size, history, and event-count validation. It is not a remote transport.
- Capture diff compares sanitized values only. Duplicate or redaction-collided identities produce explicit collision diagnostics instead of overwriting rows.
- GPUI runtime instrumentation is devtools-owned metadata. Apps fill `GpuiRuntimeSnapshot` from public facts; raw text input, clipboard payloads, unredacted titles, and accessibility labels stay outside the contract.
- Docking DevTools consumes public `DockViewportRuntimeStatus` records. Missing private facts are not inferred; unsupported platform viewport windows become diagnostics only when the public capability record is present.
- Gallery keeps `devtools_gallery_capture()` and `devtools_gallery_collection()` compatibility, but the primary state now comes from `devtools_gallery_session_frame()`.
- GPUI event row debug selectors must use event identity keys, not sequence numbers. Session-backed captures can contain multiple events with `sequence=0` from different producers.

# Current Surface

- Core no-default APIs: `DevtoolsSession`, `DevtoolsSessionFrame`, `DevtoolsSessionExport`, `DevtoolsCaptureDiff`, `DevtoolsDiffRow`, `DevtoolsEventIdentity`.
- GPUI feature APIs: `DevtoolsInspectorController::update_capture`, `update_session_frame`, session workbench rendering, and `GpuiRuntimeSnapshot` capture/provider/probe helpers.
- Docking feature APIs: `docking_runtime_inspection`, structured runtime rows, `DOCKING_PLATFORM_VIEWPORT_WINDOWS_UNSUPPORTED`, and diagnostics wired into capture/domain outputs.
- Dogfood: Gallery produces a second-generation session frame with visible diff rows; docking-native renders a DevTools capture summary from runtime status and has a focused dogfood test.

# Next Action

Keep future DevTools additions registry-backed, capture-backed, or session-backed. Do not reintroduce static Gallery snapshot builders or remote/mutation semantics without a new plan.

# Citations

- [Plan](../../../plans/2026-07-09-004-feat-devtools-live-runtime-workbench-plan.md)
- [DevTools README](../../../../crates/devtools/README.md)
- [Verification](../verification/2026-07-09-devtools-live-runtime-workbench.md)
