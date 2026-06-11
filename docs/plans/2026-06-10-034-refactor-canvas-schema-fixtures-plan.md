---
title: Refactor Canvas Schema Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Schema Fixtures

## Summary

Migrate the remaining schema test fixture setup in `schema.rs` to `document_fixture()`. The
kind-registry mutation path must still reject invalid data atomically on a real editor document.

---

## Requirements

- R1. Replace the selected default document setup in `schema.rs` with `document_fixture()`.
- R2. Keep the kind-registry mutation path and atomic rejection assertions unchanged.
- R3. Preserve the existing invalid-data error shape and empty-document expectation.
- R4. Keep the rest of the schema validation and policy tests out of this batch.

---

## Implementation Units

### U1. Schema Fixtures

- **Files:** `crates/canvas/src/schema.rs`.
- **Verification:** Registered kind mutation rejection test passes.

### U2. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Continue the remaining schema policy tests only if more pure fixture setup remains.
