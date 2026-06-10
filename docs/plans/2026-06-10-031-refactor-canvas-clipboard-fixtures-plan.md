---
title: Refactor Canvas Clipboard Fixtures
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Clipboard Fixtures

## Summary

Migrate clipboard helper fixtures in `clipboard.rs` to `document_fixture()` so the copy/paste
tests keep exercising the clipboard payload and transaction paths without manual document assembly.

---

## Requirements

- R1. Replace direct initial document writes in clipboard helper fixtures with `document_fixture()`.
- R2. Keep clipboard payload generation and paste replay assertions on the existing code paths.
- R3. Preserve edge and relation round-trip coverage.
- R4. Keep unrelated document, mutation, and persistence tests out of this batch.

---

## Implementation Units

### U1. Clipboard Fixtures

- **Files:** `crates/canvas/src/clipboard.rs`.
- **Verification:** Connected-document and related-document clipboard tests pass.

### U2. Verify Canvas Suite

- **Verification:** `cargo fmt`, `cargo check`, `cargo nextest`, and rustdoc tests pass.

---

## Deferred Work

- Continue other files with pure fixture helpers after this batch.
