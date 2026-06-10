---
title: Refactor Canvas Relation Change Journal
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Relation Change Journal

## Summary

`CanvasRecordRelations` is a document-level fact, but committed mutations currently expose relation
updates only through `CanvasDocumentDiff::relations_changed`. This plan adds structured relation
changes to the mutation journal so persistence, audit, and future CRDT adapters can observe parent
and group membership semantics without diffing whole relation snapshots.

---

## Problem Frame

`CanvasCommittedMutation` already reports actual semantic node, edge, and shape changes. That is
why deleting a node reports both the node delete and implicit incident edge deletes. Relation
changes need the same treatment.

Today, a transaction that sets a parent or adds a group membership produces a useful document diff
flag, but the committed operation batch remains empty. That makes relation-only durable entries
hard for future redb, Loro, rkyv, sync, or audit adapters to consume incrementally.

---

## Requirements

- R1. Introduce typed relation change facts for parent/group relation upsert and delete.
- R2. Store relation changes on `CanvasCommittedMutation` beside record changes.
- R3. Derive relation changes from the actual before/after relation snapshots, not command intent,
  so pruning caused by record deletion is observable.
- R4. Expose relation changes through committed persistence log entries.
- R5. Keep legacy transaction-only replay compatible and explicitly relation-operation-free.
- R6. Avoid adding concrete CRDT, redb, or rkyv dependencies.

---

## Key Technical Decisions

- KTD1. Relation changes are a sibling fact to record changes, not fake `CanvasRecordChange`
  variants. Node/edge/shape operations remain about records; relation operations are structural
  links between records.
- KTD1a. Use `CanvasRecordRelation::{Parent, Group}` plus
  `CanvasRelationChange::{Upsert, Delete}` instead of action-specific verbs. This mirrors
  `CanvasRecordChange` and leaves future relation kinds open without multiplying operation names.
- KTD2. The journal derives relation changes by comparing `CanvasRecordRelations` before and after
  mutation. This captures implicit pruning and avoids trusting command intent.
- KTD3. The persistence log should carry a committed relation operation batch in the same entry as
  the committed record operation batch. Future adapters can consume either or both without
  replaying transactions.

---

## Implementation Units

### U1. Add relation change value types

- **Goal:** Add relation change and relation operation batch types near existing change facts.
- **Files:** `crates/canvas/src/changes.rs`, `crates/canvas/src/lib.rs`.
- **Patterns:** Mirror `CanvasRecordChange`, `CanvasRecordOperation`, and
  `CanvasRecordOperationBatch` naming and sequence metadata.
- **Test scenarios:** Parent set, parent clear, group add, group remove expose relation IDs and
  sequence/order metadata.
- **Verification:** `cargo nextest run -p open-gpui-canvas changes`.

### U2. Derive committed relation changes from the mutation journal

- **Goal:** Add relation changes to `CanvasCommittedMutation` using before/after relation
  snapshots.
- **Files:** `crates/canvas/src/mutation.rs`, `crates/canvas/src/relations.rs`,
  `crates/canvas/src/document.rs`.
- **Patterns:** Reuse committed-diff semantics; never derive from `DocumentCommand` intent.
- **Test scenarios:** Relation-only transaction reports relation changes; deleting a record reports
  relation deletes caused by pruning; no-op relation commands report no committed relation changes.
- **Verification:** `cargo nextest run -p open-gpui-canvas mutation relations document::tests::*relation*`.

### U3. Persist committed relation operation batches

- **Goal:** Carry committed relation operation batches on `CanvasLogEntry` and through JSON byte
  codec round trips.
- **Files:** `crates/canvas/src/persistence/store.rs`, `crates/canvas/src/persistence/tests.rs`.
- **Patterns:** Keep new committed fields optional/defaulted for older logs. Legacy replay entries
  still report no committed relation operations.
- **Test scenarios:** Committed log entries expose relation operations; relation-only committed log
  entries are not empty semantic facts; legacy entries expose no relation operations; JSON codec
  round-trips the new optional batch.
- **Verification:** `cargo nextest run -p open-gpui-canvas persistence::tests::*relation* persistence::tests::*committed*`.

### U4. Update public guidance

- **Goal:** Document relation changes as part of the committed mutation journal fact source.
- **Files:** `crates/canvas/README.md`, `docs/adr/0002-open-gpui-canvas-architecture.md`.
- **Patterns:** Keep CRDT/redb/rkyv as future adapters; describe only the committed fact boundary.
- **Verification:** `rg "relation operation|relation changes|relation operations" crates/canvas/README.md docs/adr/0002-open-gpui-canvas-architecture.md`.

---

## Scope Boundaries

- This plan does not add CRDT, redb, or rkyv adapters.
- This plan does not change JSON Canvas import/export relation mapping.
- This plan does not add group or frame UI behavior.
- This plan does not remove legacy transaction-only replay.

---

## Sources

- `crates/canvas/src/mutation.rs` currently derives committed record changes from actual diff.
- `crates/canvas/src/relations.rs` defines parent and group relation facts.
- `crates/canvas/src/persistence/store.rs` stores committed record operation batches but not
  committed relation operation batches.
