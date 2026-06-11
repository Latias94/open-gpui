---
title: Refactor Canvas Geometry And Index Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Geometry And Index Fixtures

## Summary

Continue the document mutation discipline cleanup by migrating low-level geometry, snap, and
spatial-index tests away from direct fixture writes. Initial document construction should use the
shared `document_fixture()` builder, while tests that exercise document changes should produce diffs
through `CanvasTransaction` and `DocumentCommand`.

---

## Requirements

- R1. Use `document_fixture()` for initial records in `index`, `geometry_facts`, and `snap` tests.
- R2. Keep incremental-index diff tests on transaction-generated document diffs.
- R3. Preserve hit-test ordering, hidden/locked filtering, handle hits, route geometry, snap guides,
  and kind-registry geometry behavior.
- R4. Keep the scope test-only and avoid changing runtime behavior.
- R5. Remove stale test-only document update helpers once diff-producing tests use transactions.

---

## Implementation Units

### U1. Migrate Spatial Index Tests

- **Files:** `crates/canvas/src/index.rs`.
- **Verification:** Spatial hit-test, culling, route bounds, handle-hit, and incremental diff tests
  pass.

### U2. Migrate Geometry Facts And Snap Tests

- **Files:** `crates/canvas/src/geometry_facts.rs`, `crates/canvas/src/snap.rs`.
- **Verification:** Geometry fact materialization, endpoint picking, nearest-point hit geometry, and
  snap guide tests pass.

### U3. Verify Canvas Suite

- **Files:** `crates/canvas/src/document.rs`.
- **Scope:** Delete unused direct update fixture helpers and keep update validation covered through
  the transaction path.
- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Migrate `graph.rs`, `schema.rs`, and `store.rs` tests in a smaller follow-up batch.
- Migrate `gpui.rs` and `tool.rs` separately because they have larger setup surfaces and more
  behavior-specific fixtures.
