---
title: Refactor Canvas GPUI Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas GPUI Fixtures

## Summary

Migrate GPUI paint and input-adapter tests to use `document_fixture()` for initial document setup.
The GPUI adapter should continue to test editor snapshots, paint frames, overlays, and interaction
feedback without relying on direct test-only record insertion.

---

## Requirements

- R1. Build paint-model and editor snapshot fixture documents through `document_fixture()`.
- R2. Keep editor behavior under test on `CanvasEditor` command/tool-effect paths.
- R3. Preserve visible-record culling, widget overlays, kind-registry geometry, label metadata,
  style resolution, and connection preview behavior.
- R4. Keep the change test-only and avoid altering GPUI adapter runtime code.

---

## Implementation Units

### U1. Paint And Overlay Fixtures

- **Files:** `crates/canvas/src/gpui.rs`.
- **Verification:** Paint frame, label, style, overlay, and snapshot tests pass.

### U2. Helper Fixture Construction

- **Files:** `crates/canvas/src/gpui.rs`.
- **Verification:** Large-grid and connected-edge helper documents still exercise culling and custom
  route behavior.

### U3. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Migrate `tool.rs` in multiple batches: selection/resize, connect, layer ordering, custom tools,
  and registry/schema behavior.
