---
title: Refactor Canvas Relation Binding Seam
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Relation Binding Seam

## Summary

Deepen the relationship layer so structural relations are owned by `relations.rs` instead of being
defined beside change-feed types. This keeps parent/group facts first-class document data and gives
future binding records a clear place to land without adding edge binding, layout, CRDT, or grouping
behavior in this slice.

---

## Problem Frame

`CanvasRecordRelations` already stores parent and group membership facts, and committed mutations
already report relation changes. The current seam is still slightly shallow because
`CanvasRecordRelation` is defined in `changes.rs`, while `relations.rs` owns the concrete parent and
group relation storage. That makes relation identity look like a change-log concern rather than a
document-model concern.

The immediate goal is to move relation record vocabulary into the relation module, add an explicit
construction API for relation sets, and make callers query relation records through one local API.
Bindings should be named as the next relationship family, but not implemented before a concrete edge
binding or frame containment feature exercises the design.

---

## Requirements

- R1. Keep parent and group membership facts serialized exactly as before.
- R2. Move relation record identity to `relations.rs`; change feeds should consume relation records,
  not define them.
- R3. Add a `CanvasRecordRelationsBuilder` for clipboard/import/fixtures that need to construct a
  relation set without direct low-level mutation calls.
- R4. Add relation query helpers that work over the unified `CanvasRecordRelation` enum.
- R5. Preserve journal, store, persistence, gesture, clipboard, and JSON snapshot behavior.
- R6. Document where future binding records belong, while explicitly deferring binding behavior.

---

## Implementation Units

### U1. Move Relation Record Vocabulary

- **Goal:** Define `CanvasRecordRelation` and relation kind/identity helpers in `relations.rs`.
- **Files:** `crates/canvas/src/relations.rs`, `crates/canvas/src/changes.rs`,
  `crates/canvas/src/lib.rs`.
- **Verification:** Existing relation operation batch tests and persistence tests still compile
  without changing public crate imports.

### U2. Add Relation Builder And Unified Query API

- **Goal:** Add explicit relation construction helpers and generic relation containment queries.
- **Files:** `crates/canvas/src/relations.rs`, `crates/canvas/src/clipboard.rs`,
  `crates/canvas/src/gesture.rs`, `crates/canvas/src/mutation.rs`.
- **Verification:** Clipboard relation preservation, gesture relation diffs, committed relation
  changes, and relation equality tests continue to pass.

### U3. Document Binding Roadmap

- **Goal:** Make clear that parent/group are structural relations and future edge/frame bindings
  should be added as first-class relation-family records, not arbitrary payload conventions.
- **Files:** `crates/canvas/README.md`, `docs/adr/0002-open-gpui-canvas-architecture.md`.
- **Verification:** Docs distinguish implemented relations from deferred bindings.

---

## Scope Boundaries

- Do not add edge binding records yet.
- Do not change snapshot JSON shape for existing parent/group relations.
- Do not add frame/group editing tools or layout ownership behavior.
- Do not introduce Loro/redb/rkyv adapters in this slice.
