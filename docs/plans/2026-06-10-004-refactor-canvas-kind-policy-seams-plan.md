---
title: Refactor Canvas Kind Policy Seams
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas Kind Policy Seams

## Summary

This plan deepens `CanvasKindRegistry` so per-kind behavior is registered as focused policy seams instead of one wide node, edge, or shape trait. The registry remains the application-facing entry point, while schema, geometry, interaction, render, and transform rules become separable contracts that can evolve independently for Figma-like, draw.io-like, mind-map, and xyflow-like canvases.

---

## Problem Frame

`CanvasGeometryFacts`, `CanvasRuntime`, and the mutation journal now have clear ownership boundaries. `CanvasKindRegistry` is the next pressure point. The current `CanvasNodeKind`, `CanvasEdgeKind`, and `CanvasShapeKind` traits mix defaults, migration, validation, geometry, hit testing, paint metadata, labels, and resize rules. That makes simple kinds implement a broad surface, and it gives complex kinds no natural place to split schema, geometry, interaction, and rendering policy.

Because the crate has not shipped a stable public API, this refactor should remove the old wide trait shape rather than preserve it behind deprecated aliases.

---

## Requirements

**Registry entry point**

- R1. `CanvasKindRegistry` remains the single public registry for node, edge, and shape kinds.
- R2. Registered kinds are stored as bundled policy objects, not as one trait that owns every behavior category.
- R3. Unknown kinds continue to pass through unchanged for open imported documents.

**Focused policies**

- R4. Schema policy owns defaults, migration, and validation.
- R5. Geometry policy owns node bounds, shape bounds, and handle positions.
- R6. Interaction policy owns precise node and shape hit decisions.
- R7. Render policy owns renderer-neutral paint and label metadata.
- R8. Transform policy owns resize proposal validation and clamping.

**Call-site alignment**

- R9. Document loading, transaction normalization, editor gestures, runtime caches, geometry facts, and GPUI paint all consume policies through `CanvasKindRegistry`.
- R10. Examples, benches, tests, and README snippets use the new policy API.

---

## Key Technical Decisions

- KTD1. Keep `CanvasKindRegistry` as the deep module facade. Applications should still register a kind by string and hand the registry to document, editor, runtime, or paint APIs.
- KTD2. Turn `CanvasNodeKind`, `CanvasEdgeKind`, and `CanvasShapeKind` into bundled kind definitions with builder-style policy setters. The old trait names are too valuable as public concepts to keep as wide traits.
- KTD3. Name extension traits by concern: schema, geometry, interaction, render, and transform. This mirrors the architecture language already used by `CanvasGeometryFacts` and avoids forcing paint or resize logic into schema validators.
- KTD4. Preserve fallback behavior. A kind with no schema policy leaves data unchanged; a kind with no geometry policy uses document bounds; a kind with no render policy lets record style and theme win.
- KTD5. Do not introduce concrete persistence, CRDT, or routing adapters in this pass. This refactor prepares those seams but does not expand product scope.

---

## Implementation Units

### U1. Replace wide kind traits with bundled policy definitions

- **Goal:** Introduce `CanvasNodeKind`, `CanvasEdgeKind`, and `CanvasShapeKind` as policy bundles, with focused policy traits for schema, geometry, interaction, render, and transform behavior.
- **Files:** `crates/canvas/src/schema.rs`, `crates/canvas/src/lib.rs`.
- **Patterns:** Preserve the registry method names where they remain semantically correct; remove old wide trait methods instead of adding compatibility aliases.
- **Test scenarios:** Open registry leaves unknown records unchanged; node, edge, and shape bundles can install only the policies they need; default bundles use document geometry and empty schema behavior.
- **Verification:** `cargo check -p open-gpui-canvas --all-features`.

### U2. Migrate document, geometry, runtime, paint, and tool call sites

- **Goal:** Route all kind behavior through the new focused policy accessors without changing editor, runtime, paint, hit-test, or resize semantics.
- **Files:** `crates/canvas/src/document.rs`, `crates/canvas/src/geometry_facts.rs`, `crates/canvas/src/gpui.rs`, `crates/canvas/src/runtime.rs`, `crates/canvas/src/runtime_query.rs`, `crates/canvas/src/spatial_cache.rs`, `crates/canvas/src/tool.rs`, `crates/canvas/src/transform.rs`.
- **Patterns:** `CanvasGeometryFacts` remains the geometry truth source; the registry supplies policy, not final runtime query semantics.
- **Test scenarios:** Kind geometry still affects hit records, culling, transform handles, snap candidates, and paint bounds; kind hit policy still rejects points inside coarse bounds; resize policy remains atomic on rejection.
- **Verification:** `cargo nextest run -p open-gpui-canvas geometry runtime tool`.

### U3. Update examples, benches, README, and ADR language

- **Goal:** Make all public-facing code demonstrate the deeper policy API and remove stale references to wide kind handlers.
- **Files:** `crates/canvas/README.md`, `crates/canvas/benches/large_canvas.rs`, `examples/smoke-native/src/main.rs`, `examples/canvas-notes/src/main.rs`, `docs/adr/0002-open-gpui-canvas-architecture.md`.
- **Patterns:** Keep examples concise by registering only the policies each kind actually uses.
- **Test scenarios:** Native examples compile; README snippets name the new policy concepts; ADR describes registry-driven policy bundles.
- **Verification:** `cargo check -p open-gpui-canvas --all-features`.

### U4. Strengthen tests around policy separation

- **Goal:** Add coverage that proves policy categories are independent and still converge through the registry facade.
- **Files:** `crates/canvas/src/schema.rs`, `crates/canvas/src/geometry_facts.rs`, `crates/canvas/tests/spatial_index_strategies.rs`.
- **Patterns:** Use small test policy structs that each implement one concern, so regressions cannot hide behind a single all-purpose kind implementation.
- **Test scenarios:** Schema-only node policy does not affect geometry; geometry-only policy affects bounds without validation; render-only policy supplies paint and labels; transform-only policy clamps resize; interaction-only policy affects precise hit testing.
- **Verification:** `cargo nextest run -p open-gpui-canvas`.

---

## Scope Boundaries

- This plan does not add a concrete shape utility registry like tldraw `ShapeUtil`; it prepares the policy seams needed for that later layer.
- This plan does not add relationships, grouping, frames, snap anchors, or obstacle routing.
- This plan does not introduce Loro, `redb`, or `rkyv` implementations.
- This plan does not split the GPUI adapter into separate painter/input/style modules.

---

## System-Wide Impact

This intentionally breaks the pre-release wide kind trait API. The payoff is a smaller conceptual surface for each extension point: schema authors do not need to understand paint, render adapters do not need to understand migration, and geometry policies can be used consistently by runtime caches, tools, and GPUI paint through the existing registry facade.

---

## Risks & Dependencies

- Public API churn can miss README, examples, or benchmarks. Grep for old `impl CanvasNodeKind`, `impl CanvasEdgeKind`, and `impl CanvasShapeKind` before shipping.
- Builder-style bundles can become verbose. Keep registration examples short and add convenience constructors only when they preserve policy separation.
- Policy trait object wiring can hide default behavior regressions. Tests should cover schema-only, geometry-only, interaction-only, render-only, and transform-only registrations separately.

---

## Sources

- `crates/canvas/src/schema.rs` currently contains the wide kind traits and registry facade.
- `crates/canvas/src/geometry_facts.rs` consumes registry geometry and hit policy.
- `crates/canvas/src/tool.rs` consumes resize policy during editor gestures.
- `crates/canvas/src/gpui.rs` consumes renderer-neutral paint and label metadata.
- `docs/adr/0002-open-gpui-canvas-architecture.md` describes the registry as a major extension seam.
