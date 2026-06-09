---
title: Refactor Canvas Relation-Aware Clipboard
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Relation-Aware Clipboard

## Summary

`CanvasRecordRelations` is now a canonical document fact, so clipboard payloads must preserve
internal parent and group membership relations when records are copied, pasted, cut, or duplicated.
This slice keeps the behavior narrow: copy only relationships whose endpoints are present in the
payload, remap them during paste, and leave full group/frame tools out of scope.

---

## Problem Frame

The clipboard currently copies nodes, shapes, selected edges, and implicit internal edges. After the
record relation module exists, this is no longer enough. A duplicated frame/note pair or mind-map
subtree can lose its parent or group facts even though those facts are structural document data.

If clipboard behavior ignores relations, applications will compensate with ad hoc payload parsing,
which is exactly what the relation module was meant to prevent.

---

## Requirements

- R1. Add relations to `CanvasClipboardPayload` with a default empty value for existing serialized
  payloads.
- R2. Copy parent relations only when both child and parent records are included in the payload.
- R3. Copy group membership only when both group and member records are included in the payload.
- R4. Remap relation endpoints during paste using the same ID mapping as records.
- R5. Insert relation commands after copied records have been inserted, so document validation sees
  existing endpoints.
- R6. Keep relation preservation inside the clipboard module; do not add group editing tools,
  layout behavior, clipping, or parent-relative transforms.

---

## Key Technical Decisions

- KTD1. Clipboard payloads should carry `CanvasRecordRelations`, not raw relation commands, because
  relations are structural facts and need stable serialization just like nodes, edges, and shapes.
- KTD2. Relation copying is strict and internal-only. External parents or groups are omitted rather
  than kept dangling, matching current internal-edge copy behavior.
- KTD3. Paste remapping should use a single `CanvasRecordId` remap helper over node, edge, and shape
  ID maps, so future relation kinds do not reimplement the same branching logic.

---

## Implementation Units

### U1. Carry relations in clipboard payloads

- **Goal:** Add a defaulted `relations` field to `CanvasClipboardPayload` and populate it from the
  selected internal record set.
- **Files:** `crates/canvas/src/clipboard.rs`.
- **Patterns:** Mirror existing internal edge inclusion: include only facts that are fully inside
  the copied payload.
- **Test scenarios:** Copy frame plus child preserves parent and group relation; copying only the
  child omits external parent and group facts; serialized payloads without relations remain valid.
- **Verification:** `cargo nextest run -p open-gpui-canvas clipboard::tests::*relation*`.

### U2. Remap relations during paste and duplicate

- **Goal:** Add remapped `SetRecordParent` and `AddRecordToGroup` commands to paste transactions
  after record inserts.
- **Files:** `crates/canvas/src/clipboard.rs`, `crates/canvas/src/tool.rs`.
- **Patterns:** Let existing editor paste/duplicate methods consume the enriched paste
  transaction; no separate editor mutation path.
- **Test scenarios:** Paste creates parent/group relations with copied IDs; duplicate selection
  preserves internal relations and selects pasted records; invalid external relations are not
  emitted.
- **Verification:** `cargo nextest run -p open-gpui-canvas clipboard tool::tests::*duplicate*`.

---

## Scope Boundaries

- This plan does not implement group selection semantics beyond copied records.
- This plan does not preserve relations to records outside the copied payload.
- This plan does not change JSON Canvas import/export relationship mapping.
- This plan does not change z-order, transform, snap, or layout semantics for parented records.

---

## Sources

- `crates/canvas/src/relations.rs` defines canonical parent and group membership facts.
- `crates/canvas/src/clipboard.rs` currently copies records and internal edges but not relations.
- `crates/canvas/src/tool.rs` routes copy, cut, paste, and duplicate through clipboard payloads.
