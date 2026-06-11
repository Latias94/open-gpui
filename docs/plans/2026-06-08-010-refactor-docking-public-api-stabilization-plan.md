---
title: "refactor: Stabilize docking public API and docs"
type: refactor
status: completed
date: 2026-06-08
---

# refactor: Stabilize docking public API and docs

## Summary

Stabilize the user-facing docking setup path after the shared controller, lazy panel lifecycle,
in-window floating, and viewport placement work. This pass should make the preferred API obvious to
application authors while preserving the existing graph and layout escape hatches for advanced
callers.

---

## Problem Frame

The docking crate now has the right architectural split: `DockGraph` stores layout structure,
`DockController` owns the mutable workspace, `DockHost` renders one logical space, panel factories
stay outside layout persistence, and `DockViewportAdapter` owns platform-window snapshots. The
remaining risk is API drift. Users can assemble the pieces, but the teaching path is still spread
across raw graph constructors, workspace registration methods, controller constructors, the native
example, and roadmap documents.

The next step should not expand into full platform tear-off lifecycle. It should make the current
capability set coherent: a simple controller builder for common setup, clear crate-level docs,
public smoke tests that describe intended usage, and removal of stale phase-era API text.

---

## Requirements

- R1. Provide a recommended setup path that starts from a logical dock space, default layout, panel
  registrations, policy, and host mounting through `DockController`.
- R2. Keep direct `DockGraph`, `DockLayout`, `DockWorkspace`, and `DockAction` APIs available for
  advanced callers.
- R3. Keep `DockLayout` graph-only and keep viewport placement persistence in adapter-level DTOs.
- R4. Keep lazy panel factory registration as the recommended default while retaining eager view
  registration for tests and simple applications.
- R5. Remove or rewrite stale public docs that still describe in-window floating as deferred.
- R6. Add tests that characterize the public setup surface rather than only internal graph behavior.
- R7. Do not introduce OS-window open/close lifecycle management in this pass.

---

## Scope Boundaries

In scope:

- A small `DockControllerBuilder` or equivalent convenience setup API.
- Crate-level and type-level documentation for the recommended architecture.
- Public API smoke tests for builder setup, lazy panel registration, layout restore, and viewport
  placement separation.
- Cleanup of stale public options or doc comments left from earlier phases.

Out of scope:

- Automatic platform window creation for detached viewports.
- Full multi-window close/reopen lifecycle.
- Floating resize handles, snapping, keyboard navigation, or accessibility polish.
- Replacing the existing graph/layout advanced APIs.

---

## Implementation Units

### U1. Controller Builder

**Goal:** Add a concise app-author setup path centered on `DockController`.

**Files:**

- `crates/gpui_docking/src/controller.rs`
- `crates/gpui_docking/src/lib.rs`
- `crates/gpui_docking/src/tests.rs`

**Approach:** Add a builder that can set a graph or editor default layout, register eager and lazy
panels, configure policy/options, and build a `DockController`. Keep fallible layout import explicit
when restoring serialized `DockLayout`.

**Verification:** Tests prove the builder creates a controller with the expected graph, policy, and
registered panels without requiring callers to manually create a `DockWorkspace`.

### U2. Public Documentation Cleanup

**Goal:** Make the public docs explain the architecture accurately.

**Files:**

- `crates/gpui_docking/src/lib.rs`
- `crates/gpui_docking/src/host.rs`
- `crates/gpui_docking/src/controller.rs`
- `crates/gpui_docking/src/viewport.rs`

**Approach:** Add crate-level usage docs and update type docs so the preferred layering is clear:
graph/layout for persisted structure, controller for mutable state, host for rendering, adapter for
runtime windows. Remove stale floating-placeholder text.

**Verification:** `cargo doc -p open-gpui-docking --no-deps` succeeds with `#![warn(missing_docs)]`.

### U3. User-Facing Characterization Tests

**Goal:** Add tests that lock the public API contract instead of only internal mutation details.

**Files:**

- `crates/gpui_docking/src/tests.rs`
- `crates/gpui_docking/src/host_tests.rs`
- `crates/gpui_docking/src/viewport.rs`

**Approach:** Cover the builder path, restore-from-layout path, lazy panel metadata behavior, and
the fact that adapter placement remains separate from `DockLayout`.

**Verification:** `cargo nextest run -p open-gpui-docking` passes.

---

## Verification

- `cargo fmt --check`
- `cargo nextest run -p open-gpui-docking`
- `cargo clippy -p open-gpui-docking --all-targets`
- `cargo doc -p open-gpui-docking --no-deps`
