---
title: Refactor Canvas Format Module
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Format Module

## Summary

This plan creates a focused `CanvasFormat` module for document format version facts, snapshot
migration sequencing, and persistence-envelope document-version validation. It keeps the current
wire format unchanged while giving future JSON, redb, rkyv, and Loro adapters one place to ask what
format versions are supported.

---

## Problem Frame

Snapshot version constants, migration ordering, and persistence envelope checks currently live in
adjacent modules. `document.rs` owns `CANVAS_DOCUMENT_FORMAT_VERSION`,
`CANVAS_DOCUMENT_MIN_SUPPORTED_FORMAT_VERSION`, `CanvasSnapshotMigration`, and
`migrate_canvas_snapshot`, while `persistence/codec.rs` repeats the supported document-version
range check for envelopes.

That is manageable for v1, but it weakens locality once the crate adds a real v2 migration, a redb
store, an rkyv snapshot codec, or a CRDT adapter that has to normalize incoming document records.

---

## Requirements

- R1. Keep all serialized field names, current version values, and public re-export names stable.
- R2. Move format version constants and migration-chain validation into `crates/canvas/src/format.rs`.
- R3. Make snapshot migration and supported-version checks share the same module.
- R4. Make persistence envelope document-version validation call the format module instead of
  duplicating the version range condition.
- R5. Keep `CanvasDocument::from_snapshot` and `CanvasSnapshot::migrate_to_current` as the public
  restore entry points.

---

## Key Technical Decisions

- KTD1. `CanvasFormat` is a facts module, not a codec. It should not know about JSON bytes, redb,
  rkyv, or persistence store ordering.
- KTD2. `CanvasSnapshotMigration` remains public so future migration table entries remain visible
  and testable.
- KTD3. Persistence keeps its own codec-version validation, but document-format validation delegates
  to `CanvasFormat`.
- KTD4. The migration table remains empty for v1; this refactor only moves the seam.

---

## Implementation Units

### U1. Introduce the format module

- **Goal:** Add `crates/canvas/src/format.rs` with current/min supported version constants,
  `CanvasSnapshotMigration`, `CANVAS_SNAPSHOT_MIGRATIONS`, and format validation helpers.
- **Files:** `crates/canvas/src/format.rs`, `crates/canvas/src/lib.rs`.
- **Patterns:** Keep names re-exported from `lib.rs` so downstream code can continue importing from
  `open_gpui_canvas::*`.
- **Test scenarios:** Migration table monotonicity still compiles against the same public names.
- **Verification:** `cargo check -p open-gpui-canvas --all-targets --all-features`.

### U2. Route document snapshot migration through CanvasFormat

- **Goal:** Move `migrate_canvas_snapshot` out of `document.rs` while preserving
  `CanvasSnapshot::migrate_to_current` and `CanvasDocument::from_snapshot` behavior.
- **Files:** `crates/canvas/src/document.rs`, `crates/canvas/src/format.rs`.
- **Patterns:** Avoid circular dependencies by letting `format.rs` depend on `CanvasSnapshot` and
  `DocumentError`, while `document.rs` only imports format facts.
- **Test scenarios:** Current snapshot no-op migration, future-version rejection, minimum-version
  rejection, and snapshot restore tests still pass.
- **Verification:** `cargo nextest run -p open-gpui-canvas document::tests::*snapshot*`.

### U3. Share document-version validation with persistence codecs

- **Goal:** Replace persistence codec's inline document-format range check with the format module.
- **Files:** `crates/canvas/src/persistence/codec.rs`, `crates/canvas/src/format.rs`.
- **Patterns:** Keep codec-version validation local to persistence; only document format semantics
  move.
- **Test scenarios:** Unsupported persistence document-format version still reports the same
  `CanvasPersistenceCodecError`.
- **Verification:** `cargo nextest run -p open-gpui-canvas persistence::tests::json_persistence_codec_rejects_unsupported_document_format_version`.

### U4. Update architecture docs

- **Goal:** Document `CanvasFormat` as the format-evolution fact source.
- **Files:** `crates/canvas/README.md`, `docs/adr/0002-open-gpui-canvas-architecture.md`.
- **Patterns:** Keep the explanation short; do not imply concrete redb/rkyv/Loro adapters exist.
- **Test scenarios:** Docs point future adapters to one format boundary.
- **Verification:** `rg "CanvasFormat|format evolution" crates/canvas/README.md docs/adr/0002-open-gpui-canvas-architecture.md`.

---

## Scope Boundaries

- This plan does not add a v2 format or any migration entry.
- This plan does not change JSON snapshot shape or persistence envelope shape.
- This plan does not introduce redb, rkyv, or Loro dependencies.
- This plan does not change document mutation semantics.

---

## Risks & Dependencies

- Moving migration helpers can accidentally create a module cycle. Keep `format.rs` small and
  dependent only on document types, not persistence.
- Error type conversions can drift. Preserve `DocumentError::UnsupportedFormatVersion` and
  `CanvasPersistenceCodecError::UnsupportedDocumentFormatVersion` observable behavior.
- Tests may still refer to old public names. Keep `lib.rs` re-exports stable.

---

## Sources

- `crates/canvas/src/document.rs` owns snapshot versions and migration today.
- `crates/canvas/src/persistence/codec.rs` repeats document-format envelope checks.
- `docs/adr/0002-open-gpui-canvas-architecture.md` already describes explicit snapshot evolution.
