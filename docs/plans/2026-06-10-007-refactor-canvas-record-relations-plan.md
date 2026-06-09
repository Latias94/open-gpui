---
title: Refactor Canvas Record Relations
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Record Relations

## Summary

This plan introduces a small `CanvasRecordRelations` module so parent, containment, group, and
layout-owner relationships have a typed home in the document model. The first slice is deliberately
foundational: serialize relations, validate referenced records, prune relations when records are
deleted, and expose read APIs. It does not implement group editing tools or frame layout behavior.

---

## Problem Frame

The core data model separates nodes, edges, and shapes, which is the right base for xyflow-like
graphs and whiteboards. The missing piece is record-to-record relationship semantics. Figma frames,
draw.io groups, MarginNote mind-map hierarchy, JSON Canvas groups, and xyflow parent extents all
need parent or ownership rules.

Without a typed relation layer, applications will store these facts in arbitrary `CanvasValue`
payloads. That makes selection, z-order, export, layout, validation, persistence, and CRDT
translation repeat the same relationship parsing in each app.

---

## Requirements

- R1. Add typed, serializable `CanvasRecordRelations` to `CanvasDocument` and `CanvasSnapshot`.
- R2. Express relationships by `CanvasRecordId`, not by separate node/edge/shape-only IDs.
- R3. Support at least parent and group membership as first-class relationship facts.
- R4. Validate that every relationship endpoint references an existing document record.
- R5. Prune relationships when the mutation journal deletes records, including implicit incident
  edge deletes.
- R6. Keep current documents backward compatible by defaulting missing relations to empty.
- R7. Keep group/frame editing behavior out of scope for this slice.

---

## Key Technical Decisions

- KTD1. Relations are document records' structural metadata, not schema-kind payload. They belong
  beside metadata and record collections so all adapters can read the same facts.
- KTD2. The first API is read-heavy and mutation-through-transaction. Direct mutable relation access
  should not be public.
- KTD3. Pruning should happen in the mutation journal path so runtime, history, persistence, and
  future CRDT adapters observe actual relation changes through committed diffs.
- KTD4. Relation changes should mark document metadata/structure changed without pretending a node,
  edge, or shape payload changed.

---

## Implementation Units

### U1. Add typed relation records

- **Goal:** Create `crates/canvas/src/relations.rs` with `CanvasRecordRelations`,
  `CanvasRecordParentRelation`, and `CanvasRecordGroupRelation`.
- **Files:** `crates/canvas/src/relations.rs`, `crates/canvas/src/lib.rs`.
- **Patterns:** Use `IndexMap` / `IndexSet` where stable ordering matters, matching document and
  selection patterns.
- **Test scenarios:** Empty relations default, parent lookup, children lookup, group member lookup,
  and duplicate membership deduplication.
- **Verification:** `cargo nextest run -p open-gpui-canvas relations`.

### U2. Store relations in document snapshots

- **Goal:** Add a defaulted `relations` field to `CanvasDocument` and `CanvasSnapshot`, expose
  `CanvasDocument::relations`, and include relations in `to_snapshot` / `from_snapshot`.
- **Files:** `crates/canvas/src/document.rs`, `crates/canvas/src/relations.rs`.
- **Patterns:** Keep missing-field deserialization backward compatible through `#[serde(default)]`.
- **Test scenarios:** Existing JSON without relations restores; snapshot round-trip preserves parent
  and group relations.
- **Verification:** `cargo nextest run -p open-gpui-canvas document::tests::*relations*`.

### U3. Validate and prune relations through mutation

- **Goal:** Make transaction application reject dangling relation endpoints and remove relation
  entries that point at deleted records.
- **Files:** `crates/canvas/src/document.rs`, `crates/canvas/src/mutation.rs`,
  `crates/canvas/src/relations.rs`.
- **Patterns:** Reuse actual committed mutation/diff semantics so implicit edge deletion also prunes
  relation facts.
- **Test scenarios:** Dangling parent rejected, dangling group member rejected, deleting a child
  removes its parent relation, deleting a group removes memberships, deleting a node with incident
  edges prunes edge relations.
- **Verification:** `cargo nextest run -p open-gpui-canvas relations document::tests::*relations* mutation::tests::*relation*`.

### U4. Update architecture docs

- **Goal:** Document relations as the future group/frame/layout seam without claiming full tools.
- **Files:** `crates/canvas/README.md`, `docs/adr/0002-open-gpui-canvas-architecture.md`.
- **Patterns:** Keep the first slice explicit: typed facts and validation, not group UI.
- **Test scenarios:** Docs explain why relations are not arbitrary payload.
- **Verification:** `rg "CanvasRecordRelations|relations" crates/canvas/README.md docs/adr/0002-open-gpui-canvas-architecture.md`.

---

## Scope Boundaries

- This plan does not implement group selection, frame layout, clipping, or parent-relative
  transforms.
- This plan does not import JSON Canvas group containment into relations yet.
- This plan does not change z-order behavior for parented records.
- This plan does not add CRDT, redb, or rkyv adapters.

---

## Risks & Dependencies

- Adding document fields changes serialized snapshots. Default empty relations keep old snapshots
  readable, and current v1 can remain valid because the field is additive and defaulted.
- Pruning relations must not hide validation failures for inserted dangling relationships. Validate
  after pruning deleted records but before committing.
- `CanvasDocumentDiff` may need a relation-changed flag. Keep it explicit instead of overloading
  metadata changes if the implementation requires observers to distinguish relationship updates.

---

## Sources

- `docs/adr/0002-open-gpui-canvas-architecture.md` calls out Figma frames, draw.io groups,
  MarginNote hierarchy, and xyflow parent extent as future requirements.
- `crates/canvas/src/document.rs` currently has metadata plus node/edge/shape collections but no
  typed relationship layer.
- `crates/canvas/src/mutation.rs` is the committed mutation fact source and should own actual
  relationship pruning semantics.
