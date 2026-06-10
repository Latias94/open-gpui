---
title: "feat: Add Canvas Product Demo Pack"
type: feat
status: active
date: 2026-06-08
---

# feat: Add Canvas Product Demo Pack

## Summary

This plan adds focused examples that prove `open-gpui-canvas` can support real product shapes:
xyflow-style flow editing, MarginNote-style node notes, and Obsidian Canvas interchange. The demos
should act as API pressure tests, not marketing pages.

---

## Problem Frame

The native smoke example exercises core interactions, but it is intentionally small. A general
canvas crate needs examples that demonstrate how applications compose the model, registry, editor,
persistence boundary, and GPUI adapter into recognizable workflows. Without these examples, API
gaps stay hidden until external users try to build a product.

---

## Requirements

**Demo Coverage**

- R1. Add a flow editor example with typed node data, handles, edges, connection creation, duplicate,
  delete, and z-order commands.
- R2. Add a note-map example with text-like note cards, groups or shapes, free placement, selection,
  drag, resize, and snap guides.
- R3. Add JSON Canvas import/export coverage that can load and save a small Obsidian Canvas-style
  document.
- R4. Examples must use public `open_gpui_canvas` APIs rather than crate-private hooks.

**Developer Experience**

- R5. Each example must have a documented run command and minimal seed data.
- R6. Examples must stay small enough to maintain as regression fixtures.
- R7. Example code must reflect recommended command and registry patterns.

**Validation**

- R8. CI or test coverage must at least compile the examples in the supported workspace matrix.
- R9. Example fixtures must avoid network dependencies and large binary assets.

---

## Key Technical Decisions

- KTD1. **Examples are API tests:** any awkward example workaround should trigger core API cleanup
  rather than stay hidden in example code.
- KTD2. **Use GPUI batched paint as the default path:** demos may add UI controls, but canvas records
  should not become one element per node.
- KTD3. **Seed data stays local and text-based:** JSON fixtures are enough for repeatability and
  make import/export behavior reviewable.
- KTD4. **Avoid premature full product chrome:** examples should show workflows, not become a
  separate app framework.

---

## Implementation Units

### U1. Flow Editor Example

**Goal:** Add a compact GPUI example for graph editing.

**Requirements:** R1, R4, R5, R7, R8.

**Files:**

- `examples/canvas-flow/Cargo.toml`
- `examples/canvas-flow/src/main.rs`
- `Cargo.toml`
- `.github/workflows/verify.yml`

**Approach:** Build on the smoke-native event mapping and add typed node kinds, visible handles,
connection gestures, duplicate, delete, and z-order shortcuts.

**Test scenarios:**

- `examples/canvas-flow/src/main.rs`: example compiles with workspace dependencies.
- `examples/canvas-flow/src/main.rs`: shortcuts call `CanvasEditor` commands.
- `.github/workflows/verify.yml`: example check is included or intentionally documented if platform
  constraints require a narrower CI command.

### U2. Note Map Example

**Goal:** Add a note-card canvas that exercises resize, snap, grouping-like shapes, and JSON data.

**Requirements:** R2, R4, R5, R7, R8.

**Files:**

- `examples/canvas-notes/Cargo.toml`
- `examples/canvas-notes/src/main.rs`
- `examples/canvas-notes/assets/sample.canvas`
- `Cargo.toml`

**Approach:** Use plain local fixtures and kind policy for note cards. Keep rich text editing
deferred, but show enough card payload and resize behavior to mirror note-map workflows.

**Test scenarios:**

- `examples/canvas-notes/src/main.rs`: loads seed data and constructs a `CanvasEditor`.
- `examples/canvas-notes/assets/sample.canvas`: round-trips through the JSON Canvas adapter in a
  test.
- `examples/canvas-notes/src/main.rs`: resize and snap affordances compile against public APIs.

### U3. Example Fixture Tests

**Goal:** Add regression tests for demo data and import/export expectations.

**Requirements:** R3, R6, R8, R9.

**Files:**

- `crates/canvas/tests/json_canvas_examples.rs`
- `examples/canvas-notes/assets/sample.canvas`
- `examples/canvas-flow/assets/sample.canvas`

**Approach:** Keep fixtures small and assert stable record counts, edge endpoints, handles, and
round-trip fields.

**Test scenarios:**

- `crates/canvas/tests/json_canvas_examples.rs`: sample note map imports with expected node and edge
  counts.
- `crates/canvas/tests/json_canvas_examples.rs`: export preserves file, link, text, group, color,
  and side-handle fields.

### U4. README And Release Docs

**Goal:** Make examples discoverable and align documentation with the supported API.

**Requirements:** R5, R7.

**Files:**

- `README.md`
- `crates/canvas/README.md`
- `CHANGELOG.md`

**Approach:** Add run commands, feature notes, and limitations. Keep attribution and license notes
consistent with the fork strategy.

**Test scenarios:**

- `README.md`: commands reference real package names.
- `crates/canvas/README.md`: example code matches public APIs after the example pack lands.

---

## Scope Boundaries

- The plan does not ship a production flow editor or note-taking app.
- The plan does not add remote collaboration, sync, or cloud storage.
- The plan does not require rich text editing; note text can stay in payload data.
- The plan does not add large binary image fixtures.

---

## System-Wide Impact

The examples will become regression pressure on public APIs. They should expose whether the editor
command surface, kind registry, JSON Canvas adapter, persistence boundary, and GPUI adapter compose
cleanly outside crate-local tests.

---

## Risks & Dependencies

- **Risk: Examples force broad API changes.** Mitigation: treat that as useful feedback and keep
  changes in small commits.
- **Risk: CI becomes platform-heavy.** Mitigation: compile examples by default and run native GUI
  execution only where runners support it.
- **Risk: Examples drift from crate behavior.** Mitigation: use shared fixtures and tests that fail
  when import/export or command APIs change.

---

## Sources / Research

- `examples/smoke-native/src/main.rs`
- `crates/canvas/src/json_canvas.rs`
- `crates/canvas/README.md`
- `docs/adr/0002-open-gpui-canvas-architecture.md`
- `repo-ref/xyflow`
- `repo-ref/tldraw`
