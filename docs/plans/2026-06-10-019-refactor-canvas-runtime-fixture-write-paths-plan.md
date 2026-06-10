---
title: Refactor Canvas Runtime Fixture Write Paths
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Runtime Fixture Write Paths

## Summary

Continue the fixture cleanup by migrating runtime, runtime-query, and spatial-cache tests to the
shared `document_fixture()` construction helper. These modules exercise cache rebuilds, diff
application, stale-record suppression, and edge-geometry refreshes, so keeping setup construction
separate from mutation paths makes the invariants easier to read.

---

## Requirements

- R1. Replace direct initial-record writes in runtime/cache tests with `document_fixture()`.
- R2. Keep mutation behavior under test on transaction paths such as `commit_transaction` and
  `apply_transaction_with_diff`.
- R3. Preserve custom-router, kind-registry geometry, runtime query ordering, stale suppression, and
  spatial-cache overlay behavior.
- R4. Keep `document_fixture()` small and warning-free.

---

## Implementation Units

### U1. Migrate Runtime Tests

- **Files:** `crates/canvas/src/runtime.rs`.
- **Verification:** Runtime rebuild, committed mutation application, custom router geometry, kind
  registry geometry, and precise hit-test tests pass.

### U2. Migrate Runtime Query And Spatial Cache Tests

- **Files:** `crates/canvas/src/runtime_query.rs`, `crates/canvas/src/spatial_cache.rs`.
- **Verification:** Runtime query filtering/order, stale suppression, base cache order, overlay
  replacement, and incident-edge refresh tests pass.

### U3. Verify Full Canvas Suite

- **Files:** `crates/canvas/src/test_support.rs`.
- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Migrate index tests next; they still contain many direct document fixture writes.
- Migrate GPUI and tool tests in separate batches because they have larger setup surfaces.
