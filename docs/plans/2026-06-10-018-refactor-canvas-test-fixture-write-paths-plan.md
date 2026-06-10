---
title: Refactor Canvas Test Fixture Write Paths
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Test Fixture Write Paths

## Summary

Start reducing direct `CanvasDocument` test writes by adding a shared fixture builder backed by
`CanvasDocumentBuilder`, then migrate high-value gesture/diff tests to it. This keeps production
mutation discipline clear without doing a noisy one-shot rewrite of every canvas test.

---

## Requirements

- R1. Keep production mutation APIs unchanged.
- R2. Give tests an explicit construction helper for initial documents.
- R3. Keep tests that model edits on existing documents using transactions/editor/store APIs.
- R4. Preserve all existing gesture, relation, mutation, and persistence behavior.
- R5. Leave tests that intentionally exercise document internals in place until they are migrated
  deliberately.

---

## Implementation Units

### U1. Add A Shared Fixture Builder

- **Goal:** Add `document_fixture()` in `test_support.rs`, backed by `CanvasDocumentBuilder`.
- **Files:** `crates/canvas/src/test_support.rs`.
- **Verification:** Fixture helper compiles in canvas tests and does not affect non-test builds.

### U2. Migrate Gesture Diff Tests

- **Goal:** Replace direct insert/update fixture writes in gesture tests with fixture construction
  or transaction application.
- **Files:** `crates/canvas/src/gesture.rs`.
- **Verification:** Gesture tests still cover coalesced commits, cancel, node/edge diffs, and
  relation diffs.

### U3. Document Follow-Up Scope

- **Goal:** Keep the plan honest: this is the first cleanup slice, not a full mechanical rewrite.
- **Files:** `docs/plans/2026-06-10-018-refactor-canvas-test-fixture-write-paths-plan.md`.
- **Verification:** Plan status is completed after tests pass and commit lands.

---

## Deferred Work

- Migrate GPUI, tool, index, runtime, snap, and persistence tests in focused batches.
- Remove remaining test-only `CanvasDocument` raw helper wrappers after the last fixture batch.
- Add small test-support helpers for common connected-node, layered-record, and related-record
  documents only when repetition proves they are worth naming.
