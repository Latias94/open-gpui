---
title: Refactor Canvas Geometry Facts
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Geometry Facts

## Summary

This plan deepens the canvas geometry boundary so bounds, handles, edge route geometry, precise hit tests, selection bounds, transform handles, and snap candidates all consume one semantic facts source. The work keeps rendering batched and runtime-owned, while removing the older resolver-shaped API that made nearby modules re-walk document records in subtly different ways.

---

## Problem Frame

`CanvasGeometryResolver` already centralizes important behavior, but the name and surrounding call sites still treat it like a helper. `spatial_cache`, `snap`, `transform`, `runtime_query`, and GPUI paint each interpret part of the geometry model. That is tolerable for simple nodes, but it becomes risky once custom shape policies, custom routers, precise edge hit testing, snap anchors, resize handles, and future obstacle routing must agree.

The next refactor should turn geometry into a deeper module: callers ask for geometry facts, not for a low-level resolver plus local loops.

---

## Requirements

**Geometry truth source**

- R1. The canvas exposes `CanvasGeometryFacts` as the geometry API and removes the old `CanvasGeometryResolver` public surface.
- R2. Node bounds, shape bounds, handle bounds, endpoint positions, edge routes, edge hit radius, and precise record containment come from `CanvasGeometryFacts`.
- R3. Hit-record materialization for runtime caches and legacy `SpatialIndex` uses `CanvasGeometryFacts`, so index candidates and runtime precise hits cannot drift.

**Interaction geometry**

- R4. Selection bounds and snap candidate bounds use typed `CanvasRecordId` facts instead of local string keys or repeated document traversal.
- R5. Transform handles are built from the same selected-record bounds facts used by snapping.

**Renderer and runtime alignment**

- R6. GPUI paint continues to draw from `CanvasRuntime` edge geometry snapshots, not by re-routing edges in the adapter.
- R7. Custom kind geometry and custom edge routers remain covered by tests for culling, precise hit testing, paint frame collection, snap, and transform handles.

---

## Key Technical Decisions

- KTD1. Rename the public concept from resolver to facts: The module is no longer a helper for resolving one value; it is the semantic source for geometry policy. Because the crate is pre-1.0, the old public name should be removed rather than deprecated.
- KTD2. Keep coarse indexes dumb: `CanvasSpatialCache` and `SpatialIndex` should continue to store `HitRecord` candidates, while `CanvasGeometryFacts` owns how those records are produced.
- KTD3. Keep edge path drawing in the GPUI adapter: The adapter can interpret `CanvasRouteSegment` to draw pixels, but it must receive the route from the runtime geometry cache.
- KTD4. Use typed record IDs for interaction geometry: Snap and selection geometry should compare `CanvasRecordId` values, not formatted strings.

---

## Implementation Units

### U1. Introduce CanvasGeometryFacts

- **Goal:** Replace `CanvasGeometryResolver` with `CanvasGeometryFacts` and move public exports, docs, and tests to the new name.
- **Files:** `crates/canvas/src/geometry_facts.rs`, `crates/canvas/src/lib.rs`, `docs/adr/0002-open-gpui-canvas-architecture.md`, `docs/adr/0003-open-gpui-canvas-spatial-index-strategy.md`.
- **Patterns:** Keep existing `with_router_and_kind_registry` constructors and current error behavior; do not add compatibility aliases.
- **Test scenarios:** Existing resolver tests must pass under the facts name; endpoint position, handle role picking, edge nearest-point, and edge hit-radius behavior remain unchanged.
- **Verification:** `cargo check -p open-gpui-canvas --all-features`.

### U2. Move hit-record materialization into geometry facts

- **Goal:** Make geometry facts the only module that turns document records into `HitRecord` values.
- **Files:** `crates/canvas/src/geometry_facts.rs`, `crates/canvas/src/spatial_cache.rs`, `crates/canvas/src/index.rs`, `crates/canvas/src/runtime_query.rs`.
- **Patterns:** Preserve `CanvasSpatialCache` as a candidate cache and preserve runtime query ordering/filtering semantics.
- **Test scenarios:** Rebuild and incremental cache parity still holds for inserted, removed, moved, and routed records; custom routers still affect edge culling and hit-test candidates.
- **Verification:** `cargo nextest run -p open-gpui-canvas spatial`.

### U3. Route snap and transform through geometry facts

- **Goal:** Remove local snap/transform document walking that duplicates geometry policy.
- **Files:** `crates/canvas/src/geometry_facts.rs`, `crates/canvas/src/snap.rs`, `crates/canvas/src/transform.rs`, `crates/canvas/src/tool.rs`.
- **Patterns:** Use `CanvasRecordId` for selected and candidate records; keep snap output and transform handle output stable.
- **Test scenarios:** Snap ignores selected, locked, and hidden records; kind-registry bounds affect snap and transform handles; selected node and shape bounds use the same facts.
- **Verification:** `cargo nextest run -p open-gpui-canvas snap transform`.

### U4. Confirm GPUI/runtime geometry alignment

- **Goal:** Keep GPUI paint model on runtime edge geometry and update tests/docs to name the facts seam.
- **Files:** `crates/canvas/src/gpui.rs`, `crates/canvas/src/runtime.rs`, `crates/canvas/src/runtime_query.rs`, `docs/adr/0002-open-gpui-canvas-architecture.md`.
- **Patterns:** `CanvasPaintModel` may map document-space route segments into view-space segments, but it must not route edges itself.
- **Test scenarios:** Paint frame collection uses the custom-router runtime edge path; precise connection hit testing uses cached edge geometry; kind-registry geometry still affects paint frame bounds.
- **Verification:** `cargo nextest run -p open-gpui-canvas gpui runtime`.

---

## Scope Boundaries

- This plan does not replace the spatial backend with a new R-tree or packed AABB implementation.
- This plan does not split the full GPUI adapter into separate painter/input/style modules.
- This plan does not introduce obstacle routing, snap grids, or CRDT geometry serialization.

---

## System-Wide Impact

The change intentionally breaks the pre-release `CanvasGeometryResolver` API. Internal modules should become less coupled to document layout details, and future adapters for routing, snap policies, and shape-specific geometry should have one place to integrate.

---

## Risks & Dependencies

- Public rename churn can hide accidental compatibility aliases. The implementation should grep for the old resolver name before shipping.
- Moving materialization into geometry facts can accidentally change hit order. Existing spatial parity tests should catch this.
- Snap and transform changes can miss kind-registry behavior because default bounds still pass common tests. Add explicit kind-policy coverage before relying on existing tests.

---

## Sources

- `crates/canvas/src/geometry_facts.rs` holds endpoint, bounds, edge geometry, record geometry, and precise containment.
- `crates/canvas/src/spatial_cache.rs` used to materialize `HitRecord` values and now delegates that semantic work to geometry facts.
- `crates/canvas/src/snap.rs` and `crates/canvas/src/transform.rs` currently duplicate selection and candidate bounds walking.
- `crates/canvas/src/gpui.rs` already paints edges from `CanvasRuntime::edge_geometry`.
