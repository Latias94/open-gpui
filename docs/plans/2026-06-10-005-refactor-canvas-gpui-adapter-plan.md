---
title: Refactor Canvas GPUI Adapter
type: refactor
status: completed
date: 2026-06-10
---

# Refactor Canvas GPUI Adapter

## Summary

This plan deepens the GPUI adapter boundary by splitting `crates/canvas/src/gpui.rs` into focused adapter modules. The public API stays stable through a facade, while paint model snapshots, input mapping, frame construction, widget overlay placement, style resolution, and low-level GPUI painting stop sharing one implementation file.

---

## Problem Frame

The canvas core now has deeper seams for committed mutations, tool state, geometry facts, runtime query, and kind policy. `gpui.rs` remains the largest shallow surface: it owns snapshot construction, event mapping, view helper wiring, culling, frame construction, overlay placement, style fallback, label prepaint, and actual GPUI path/quad painting.

That coupling is tolerable for the 0.1 demo, but it will make later custom node painters, text overlays, selected-widget overlays, and richer interaction feedback harder to add without touching unrelated GPUI code.

---

## Requirements

- R1. Keep `gpui.rs` as the public facade so existing `open_gpui_canvas::*` exports remain stable.
- R2. Move paint model and interaction snapshot state into a model module.
- R3. Move `CanvasInputMapper`, editor input mapping, and key conversion into an input module.
- R4. Move visible-record collection, interaction frame construction, label metadata resolution, prepared-frame construction, and widget overlay placement into a frame module.
- R5. Move renderer-neutral paint fallback resolution into a style module.
- R6. Move low-level GPUI quad, path, and label painting into a painter module.
- R7. Move `canvas_view` and `canvas_editor_view*` helper construction into a view module.
- R8. Preserve batched paint and avoid one GPUI element per canvas record.

---

## Key Technical Decisions

- KTD1. This is a module-depth refactor, not a rendering redesign. Public function names and data structs should continue to export from `gpui.rs`.
- KTD2. `frame` owns semantic paint-frame construction; `painter` owns GPUI drawing primitives. This keeps tests for culling and overlay placement separate from low-level paint functions.
- KTD3. `style` owns all paint fallback logic, including record style, kind render policy, and theme fallback order.
- KTD4. Keep tests in the facade during the first split. Moving tests can wait until the module boundaries are stable; the immediate value is separating production code.

---

## Implementation Units

### U1. Split adapter modules behind the facade

- **Goal:** Introduce `gpui/model.rs`, `gpui/input.rs`, `gpui/frame.rs`, `gpui/style.rs`, `gpui/painter.rs`, and `gpui/view.rs`.
- **Files:** `crates/canvas/src/gpui.rs`, new files under `crates/canvas/src/gpui/`.
- **Patterns:** Keep `pub use` exports in `gpui.rs`; keep module-local helpers `pub(super)` only where another adapter module needs them.
- **Test scenarios:** Existing GPUI tests compile without changing test intent.
- **Verification:** `cargo check -p open-gpui-canvas --all-targets --all-features`.

### U2. Preserve frame, style, and painter behavior

- **Goal:** Ensure culling, label prepaint, style fallback, selection feedback, snap guides, connection previews, and transform handles behave exactly as before.
- **Files:** `crates/canvas/src/gpui/frame.rs`, `crates/canvas/src/gpui/style.rs`, `crates/canvas/src/gpui/painter.rs`, `crates/canvas/src/gpui.rs`.
- **Patterns:** GPUI paint reads `CanvasPreparedPaintFrame`; it does not re-route edges or rebuild runtime state.
- **Test scenarios:** Existing `gpui::tests::*` pass, including custom router culling, kind label metadata, paint style fallback, widget overlay placement, and input mapper cases.
- **Verification:** `cargo nextest run -p open-gpui-canvas gpui`.

### U3. Update architecture docs

- **Goal:** Document the GPUI adapter as a deep facade over focused modules.
- **Files:** `docs/adr/0002-open-gpui-canvas-architecture.md`, `crates/canvas/README.md`.
- **Patterns:** Keep the no-DOM-wrapper and batched-paint principle explicit.
- **Test scenarios:** Documentation no longer implies one monolithic GPUI adapter.
- **Verification:** `rg "GPUI adapter" crates/canvas/README.md docs/adr/0002-open-gpui-canvas-architecture.md`.

---

## Scope Boundaries

- This plan does not add custom painter traits.
- This plan does not add text-edit overlays or widget overlay event routing.
- This plan does not change canvas visual output.
- This plan does not change document, runtime, tool, or persistence APIs.

---

## Risks & Dependencies

- Large mechanical movement can hide visibility mistakes. Run all canvas targets after the split.
- Cyclic module dependencies are likely if frame construction calls painter or style calls frame. Keep direction strict: `view -> frame + painter`, `painter -> style`, `frame -> model`.
- Tests may rely on private helpers through `super::*`. Keep facade re-exports for private test helpers or update tests carefully.

---

## Sources

- `crates/canvas/src/gpui.rs` currently owns all GPUI adapter concerns.
- `docs/adr/0002-open-gpui-canvas-architecture.md` already states the adapter should keep batched paint and avoid one element per record.
