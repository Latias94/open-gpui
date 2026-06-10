---
title: Fix Canvas Relation Inverse Invariants
type: bugfix
status: completed
date: 2026-06-10
---

# Fix Canvas Relation Inverse Invariants

## Summary

`CanvasMutationJournal` must make the committed inverse restore the same semantic facts that the
committed diff reports. Snapshot loading must reject relation states that cannot be produced through
the document mutation API.

---

## Problem Frame

`RemoveNode` and related record deletion commands can remove extra records and prune relations after
the command inverse has already been computed. The committed mutation correctly reports relation
deletes, but its inverse can restore only records, not the pruned parent/group facts.

`CanvasRecordRelations` also assumes a child has at most one parent, while snapshot loading accepts
duplicate parent facts. Once loaded, lookup and mutation APIs only see or change the first matching
parent, leaving stale relation facts behind.

---

## Requirements

- R1. A committed inverse must restore relations pruned by actual record deletion.
- R2. Editor undo/redo and persistence replay must observe the same committed relation facts.
- R3. Snapshot loading must reject duplicate parent facts for the same child.
- R4. Relation validation should keep group facts deduplicated as well, matching the mutation API.

---

## Implementation Units

### U1. Reproduce journal inverse relation loss

- **Files:** `crates/canvas/src/mutation.rs`, `crates/canvas/src/persistence/tests.rs`.
- **Tests:** Remove a node that implicitly deletes an edge with parent/group relations; committed
  inverse, persistent undo, and replay must restore those relations.

### U2. Complete inverse relation facts inside the journal

- **Files:** `crates/canvas/src/mutation.rs`.
- **Approach:** After preparing the draft document, apply the base inverse to a clone of the draft
  and append only the relation commands still needed to match the previous document.

### U3. Validate relation uniqueness at snapshot boundaries

- **Files:** `crates/canvas/src/document.rs`.
- **Tests:** Loading a snapshot with duplicate parent facts for one child returns a relation
  invariant error instead of accepting ambiguous state.

---

## Scope Boundaries

- This plan does not change the public JSON shape.
- This plan does not add CRDT/redb/rkyv adapters.
- This plan does not optimize mutation preparation cloning.
