---
title: Optimize Canvas Selection Scope Follow-Ups
type: perf
date: 2026-06-12
---

# Optimize Canvas Selection Scope Follow-Ups

## Summary

This plan addresses the remaining non-blocking performance follow-ups from the selection scope
refactor. It keeps the work small: remove duplicate resize scope resolution, add relation traversal
benchmarks, and only introduce relation indexes if the benchmark data justifies the added state.

---

## Problem Frame

Selection scope now centralizes normalized explicit records, structural descendants, action records,
and paint markers. That gives the canvas a cleaner semantic boundary, but two performance questions
remain after the refactor:

- Resize pointer-down can resolve selection scope once to ask whether structural descendants exist
  and then resolve again to collect the structural resize records.
- `CanvasRecordRelations` still stores parent and group facts as ordered vectors. The current scan
  model is simple and correct, but large nested canvases need benchmark data before deciding whether
  to add indexed lookup state.

The goal is to optimize known duplicate work first, then let measurement decide whether the relation
model needs a deeper read-side index.

---

## Requirements

**Resize Scope**

- R1. Resize pointer-down resolves structural resize scope once and derives both the structural flag
  and resize record ids from that result.
- R2. Direct multi-record resize behavior remains unchanged when no structural descendants are
  present.
- R3. Structural group/frame resize continues to include resizable descendants and internal edges.

**Relation Traversal**

- R4. Benchmarks cover parent-child traversal, group-member traversal, and mixed nested
  parent-plus-group traversal.
- R5. Benchmark fixtures include shallow wide graphs and deeper nested graphs so a future index
  decision is based on realistic canvas shapes.
- R6. Any relation index introduced by this work must be crate-private, rebuilt or updated through
  existing relation mutation paths, and invisible to serialized snapshots.

**Decision Discipline**

- R7. Do not add a relation index unless benchmark results show the vector scan model is a measurable
  bottleneck in selection, paint, copy, or resize workloads.
- R8. Keep existing public APIs and document snapshot format stable.

---

## Key Technical Decisions

- KTD1. **Fix duplicate resize scope resolution first:** This is a direct hot-path cleanup with low
  design risk and existing tests around structural resize behavior.
- KTD2. **Benchmark before indexing relations:** Indexed relation lookups add invalidation and memory
  responsibilities. The current ordered vector representation should remain the write model unless
  measurements show it is not enough.
- KTD3. **Prefer crate-private read helpers over public API growth:** Any new helper should serve
  internal action paths or benchmarks first. Public selection scope APIs are already broad enough for
  this stage.
- KTD4. **Keep relation order semantics intact:** Relation operation order and snapshot round-trips
  are existing contracts; read-side acceleration must not reorder serialized relation facts.

---

## Implementation Units

### U1. Resolve Structural Resize Scope Once

- **Goal:** Replace the boolean probe plus second record collection in resize pointer-down with one
  helper that returns structural status and node/edge/shape ids together.
- **Requirements:** R1, R2, R3, R8.
- **Files:**
  - `crates/canvas/src/tool/context.rs`
  - `crates/canvas/src/tool/select.rs`
  - `crates/canvas/src/tool.rs`
- **Patterns:** Follow the existing `structural_resize_record_ids` and
  `selection_has_structural_resize_descendants` predicates; preserve the same resizable-record
  policy.
- **Test scenarios:**
  - `crates/canvas/src/tool.rs`: resizing a direct multi-selection still stays per-record and does
    not include structural descendants.
  - `crates/canvas/src/tool.rs`: resizing a selected group/frame still resizes structural
    descendants and internal edge routes.
  - `crates/canvas/src/tool.rs`: locked or hidden descendants remain excluded from resize action
    records.
- **Verification:** `cargo test -p open-gpui-canvas --lib` and
  `cargo nextest run -p open-gpui-canvas`.

### U2. Add Relation Traversal Benchmarks

- **Goal:** Add criterion coverage for relation traversal shapes that selection scope depends on.
- **Requirements:** R4, R5, R7.
- **Files:**
  - `crates/canvas/Cargo.toml`
  - `crates/canvas/benches/relation_traversal.rs`
  - `crates/canvas/README.md`
- **Patterns:** Mirror the existing Criterion setup in `crates/canvas/benches/large_canvas.rs` and
  `crates/canvas/benches/spatial_index_strategies.rs`.
- **Test scenarios:**
  - Benchmark shallow wide parent trees.
  - Benchmark deep nested parent chains.
  - Benchmark shallow wide group memberships.
  - Benchmark mixed parent and group traversal with duplicate suppression.
- **Verification:** `cargo bench -p open-gpui-canvas --bench relation_traversal -- --sample-size 10`
  for local signal, plus normal `xtask verify` for compile and formatting.

### U3. Gate Relation Read Indexing Behind Benchmark Results

- **Goal:** Decide whether to keep vector scans or add a crate-private read index for parent and
  group traversal.
- **Requirements:** R6, R7, R8.
- **Files:**
  - `docs/research/canvas-relation-traversal-benchmark-results.md`
  - `crates/canvas/src/relations.rs`
  - `crates/canvas/src/document.rs`
- **Patterns:** If an index is justified, keep vectors as the source of serialized truth and use the
  index only as a derived read model, similar in spirit to runtime caches rather than document
  storage.
- **Test scenarios:**
  - Existing relation ordering and snapshot tests continue to pass.
  - Parent lookup, group lookup, and descendant traversal match the current vector-scan oracle.
  - Relation mutation commands update or rebuild the read model without stale parent/group facts.
- **Verification:** If no index is added, the research note records that decision and no code change
  is needed for this unit. If an index is added, run `cargo nextest run -p open-gpui-canvas` and
  `cargo run -p xtask -- verify`.

---

## Acceptance Examples

- AE1. A selected group with two child nodes and one internal edge enters resize state after one
  structural scope resolution and still resizes all eligible structural records.
- AE2. A direct selection of two unrelated nodes enters non-structural resize state and keeps the
  existing per-record resize behavior.
- AE3. Relation traversal benchmarks show separate measurements for shallow-wide, deep-nested, and
  mixed parent/group graphs.
- AE4. If relation indexing is not justified, the result is captured as a research decision instead
  of adding unused cache state.

---

## Scope Boundaries

### Active Scope

- Remove duplicate structural resize scope resolution.
- Add relation traversal benchmarks for selection-scope workloads.
- Document whether relation indexing is warranted after measuring.

### Deferred

- Persistent relation index redesign beyond parent/group traversal.
- CRDT-aware relation indexing.
- General benchmark automation in CI.
- Large-scale paint or routing benchmarks unrelated to relation traversal.

---

## Risks & Dependencies

- **Benchmark noise:** Local Criterion runs can vary. Mitigation: use benchmarks to compare relative
  shapes and record enough context in the research note.
- **Premature indexing:** Adding relation indexes without clear data increases mutation complexity.
  Mitigation: make indexing conditional on benchmark evidence.
- **Resize behavior regression:** Combining helper paths can accidentally change direct resize
  behavior. Mitigation: keep direct-selection and structural-selection tests explicit.

---

## Sources & Research

- `docs/plans/2026-06-12-001-refactor-canvas-selection-scope-plan.md`: selection scope contract and
  action-scope semantics.
- `crates/canvas/src/tool/select.rs`: resize pointer-down currently probes structural descendants
  before collecting structural resize records.
- `crates/canvas/src/tool/context.rs`: resize scope helpers and selection action scope predicates.
- `crates/canvas/src/relations.rs`: current vector-backed relation lookup and descendant traversal.
- `crates/canvas/benches/large_canvas.rs`: existing Criterion benchmark style.
- `crates/canvas/benches/spatial_index_strategies.rs`: existing large-canvas benchmark workload
  patterns.
