---
title: "perf: Replace Canvas Runtime Spatial Backend Behind Existing API"
type: perf
status: active
date: 2026-06-08
---

# perf: Replace Canvas Runtime Spatial Backend Behind Existing API

## Summary

This plan turns the hybrid spatial cache prototype into a measured production backend by replacing
the internal stable base and optional dynamic overlay while keeping `CanvasRuntime` as the only
public query owner.

---

## Problem Frame

The current runtime-owned hybrid cache fixes ownership and invalidation shape, but its base and
overlay can still use simple sorted record sets. That is acceptable for 0.1 correctness, yet large
documents need better visible-query and point-hit behavior. The replacement should happen behind the
internal cache boundary so applications do not depend on concrete index crates.

---

## Requirements

**Backend Strategy**

- R1. The stable base must support fast visible-region queries for large mostly-static documents.
- R2. The dynamic overlay must support active edits and recent diffs without rebuilding the stable
  base on every pointer move.
- R3. Canvas-owned filtering, half-open bounds behavior, and z-order ordering must remain the final
  semantics.
- R4. Third-party index dependency names must remain outside public APIs.

**Correctness**

- R5. Backend results must match `SpatialIndex` oracle behavior for hidden, locked, handles,
  margins, routes, and custom kind geometry.
- R6. Diff updates must refresh moved nodes, handles, incident edges, inserted records, and removed
  records.
- R7. Cache compaction or base rebuild heuristics must not drop overlay changes.

**Measurement**

- R8. Benchmarks must compare current vector, runtime hybrid, packed static base, and dynamic
  overlay candidates across grid, overlap, clustered, long-edge, and mixed-kind fixtures.
- R9. The default backend should change only after benchmark data and parity tests support the move.

---

## Key Technical Decisions

- KTD1. **Keep `SpatialIndex` as correctness oracle:** it remains the simple fallback and test
  comparison target even if runtime uses another internal backend.
- KTD2. **Use third-party indexes as coarse filters:** candidates may return bounding-box matches,
  but canvas code rechecks exact bounds and hit semantics.
- KTD3. **Swap internals, not APIs:** `CanvasRuntime` query and hit methods should remain stable.
- KTD4. **Choose by workload data:** static packed AABB is the likely stable base, dynamic R-tree is
  the likely overlay, and benchmarks decide whether either becomes default.

---

## Implementation Units

### U1. Backend Trait And Candidate Adapters

**Goal:** Add an internal backend trait for stable-base and overlay storage.

**Requirements:** R1, R2, R3, R4.

**Files:**

- `crates/canvas/src/spatial_cache.rs`
- `crates/canvas/src/runtime.rs`
- `crates/canvas/Cargo.toml`

**Approach:** Keep the trait crate-private. Implement the current vector backend first, then add
feature-gated or dev-only adapters for selected candidate crates once parity scaffolding is ready.

**Test scenarios:**

- `crates/canvas/src/spatial_cache.rs`: vector backend parity remains unchanged.
- `crates/canvas/src/spatial_cache.rs`: backend swaps do not change query or hit ordering.

### U2. Packed Static Base

**Goal:** Replace or supplement the stable base with a packed static AABB backend when measurements
justify it.

**Requirements:** R1, R3, R5, R8, R9.

**Files:**

- `crates/canvas/src/spatial_cache.rs`
- `crates/canvas/tests/spatial_index_strategies.rs`
- `crates/canvas/benches/spatial_index_strategies.rs`

**Approach:** Treat the static index as a coarse candidate source. Reapply canvas bounds checks and
sort final records by z-order.

**Test scenarios:**

- `crates/canvas/tests/spatial_index_strategies.rs`: static-base candidate matches oracle targets
  across all fixtures.
- `crates/canvas/benches/spatial_index_strategies.rs`: visible-query benchmarks improve on the
  vector base for large grid fixtures.

### U3. Dynamic Overlay Backend

**Goal:** Evaluate or add a dynamic overlay backend for active move and resize gestures.

**Requirements:** R2, R3, R5, R6, R7, R8.

**Files:**

- `crates/canvas/src/spatial_cache.rs`
- `crates/canvas/src/runtime.rs`
- `crates/canvas/tests/spatial_index_strategies.rs`
- `crates/canvas/benches/spatial_index_strategies.rs`

**Approach:** Use overlay records for changed semantic IDs and stale suppression. Add drag-like and
resize-like benchmark loops that apply diffs before querying.

**Test scenarios:**

- `crates/canvas/tests/spatial_index_strategies.rs`: moved nodes refresh node, handles, and incident
  edges.
- `crates/canvas/tests/spatial_index_strategies.rs`: removed nodes suppress base and overlay records.
- `crates/canvas/benches/spatial_index_strategies.rs`: drag-update benchmark reports update and
  query costs separately.

### U4. Backend Selection And Documentation

**Goal:** Decide whether to make a new backend the internal default and document the choice.

**Requirements:** R4, R8, R9.

**Files:**

- `docs/adr/0003-open-gpui-canvas-spatial-index-strategy.md`
- `docs/research/canvas-spatial-index-benchmark-results.md`
- `crates/canvas/README.md`

**Approach:** Update ADR and benchmark notes with measured results. Keep any public strategy API
deferred unless a real application needs it.

**Test scenarios:**

- `docs/research/canvas-spatial-index-benchmark-results.md`: contains the benchmark matrix used for
  the decision.
- API review confirms public exports do not name backend crates.

---

## Scope Boundaries

- The plan does not expose user-selectable index strategies.
- The plan does not implement GPU culling or tile rendering.
- The plan does not change document serialization.
- The plan does not optimize text layout or rich widgets.

---

## System-Wide Impact

This change affects runtime query performance and cache invalidation internals. It should be
invisible to applications except through faster culling, hit testing, and active gestures.

---

## Risks & Dependencies

- **Risk: A backend crate uses incompatible bounds semantics.** Mitigation: use it only for coarse
  candidate selection and reapply canvas checks.
- **Risk: Faster query loses to rebuild or update cost.** Mitigation: benchmark rebuild, query,
  hit-test, and drag-update separately.
- **Risk: Public dependency leaks through types.** Mitigation: keep adapters private and audit
  `pub use` before commit.

---

## Sources / Research

- `docs/adr/0003-open-gpui-canvas-spatial-index-strategy.md`
- `docs/research/canvas-spatial-index.md`
- `docs/research/canvas-spatial-index-benchmark-results.md`
- `crates/canvas/src/spatial_cache.rs`
- `crates/canvas/tests/spatial_index_strategies.rs`
- `crates/canvas/benches/spatial_index_strategies.rs`
