# ADR 0011: Docking Split Motion Primitive Boundary

**Status**: Accepted
**Date**: 2026-06-30

## Context

ADR 0010 established docking's presentation-scene model: `DockGraph` remains the semantic
authority, while derived presentation, overlay, motion, zoom, divider, and accessibility
descriptors explain what the user sees and interacts with.

The follow-up refactor introduced reusable split and motion primitives in `open_gpui_ui_core` and
adapted both `ui_components::Splitter` and docking to them. Without an explicit boundary, docking
could grow a second private split layout solver, `ui_components` could re-own pure resize math, and
future animation work could mix platform scheduling with renderer-neutral intent.

## Decision

`open_gpui_ui_core` owns renderer-neutral split, motion, and accessibility vocabulary:

- `SplitterState` resolves panel fractions, min/max constraints, collapsed state, and resize
  deltas.
- `SplitterLayoutScene` resolves panels and handles into absolute rectangles for a single layout
  pass.
- `SplitterHitMap` resolves handle and junction hits from already-resolved handle rectangles.
- Motion policy descriptors describe transition intent and reduced-motion behavior without
  scheduling frames.
- Accessibility vocabulary includes splitter roles, orientation, selected/disabled state, and
  action descriptors.

`open_gpui_ui_components` owns GPUI adapters for those primitives:

- `Splitter` renders resolved core state and sends pointer deltas back through
  `SplitterState::resized_by`.
- GPUI accessibility helpers map renderer-neutral roles and state to GPUI's accessibility API.
- Adapter code may own element ids, focus handles, cursor styling, runtime drag state, and frame
  scheduling, but it must not invent a parallel solver.

`open_gpui_docking` owns docking semantics and consumes the primitives:

- `DockGraph` remains the persistent semantic model for splits, tabs, floating containers, central
  regions, routes, and mutation validation.
- `DockPresentationScene` resolves docking graph/session facts into absolute pane, tab, splitter,
  overlay anchor, zoom, and accessibility geometry.
- Docking uses core split scenes and hit maps for splitters and corner junctions rather than
  private handle-hit rectangle helpers.
- Drop previews, route markers, tab insertion previews, focus rings, payload ghosts, rejected
  states, transition plans, zoom scenes, and accessibility descriptors derive from the same
  presentation scene.
- Resize commits use graph-validated transactions such as `DockSplitResize`; presentation hit
  results are advisory input, not mutation authority.

The shared goal is capability alignment, not pixel-level styling parity with ImGui, BonSplit, or
SuperSplit. Styling, timing curves, and platform animation backends can evolve after the semantic
descriptors stay stable.

## Alternatives Considered

### Keep Docking's Private Split Geometry Helpers

Decision: rejected.
Private pane/handle geometry was easy to add quickly, but it created two hit-map authorities: one
inside docking and one inside the shared splitter primitive. Docking now resolves graph,
presentation, and render split panel bounds through `SplitterLayoutScene`, routes handle/junction
hit testing through `SplitterHitMap`, and keeps only docking graph mutation semantics local.

### Move DockGraph Into UI Core

Decision: rejected.
UI core should not learn docking concepts such as tab bars, floating containers, current route
facts, central regions, or multi-viewport release validation. UI core owns generic split and motion
primitives; docking owns its domain graph.

### Put Animation Execution In UI Core Immediately

Decision: rejected.
The useful shared contract today is scene-to-motion intent plus reduced-motion behavior. GPUI frame
scheduling, platform windows, and future native animation backends are adapter concerns until more
components require the same executor.

### Preserve Compatibility Shims Until All Styling Work Is Done

Decision: rejected.
The plan allows breakage of crate-private APIs. Once tests cover the shared primitive behavior, old
render-local helpers should be deleted so future work cannot accidentally reintroduce a parallel
geometry path.

## Consequences

- Future docking UI/UX work should add semantic fields to presentation, overlay, motion, or
  accessibility descriptors before adding ad hoc rectangles in render code.
- Splitter behavior shared by components and docking belongs in `open_gpui_ui_core`; GPUI-specific
  event wiring belongs in `open_gpui_ui_components` or docking adapters.
- Tests should prefer descriptor and transaction assertions over screenshot-style pixel matching.
- Platform accessibility mapping can still be incremental, but renderer-neutral descriptor coverage
  is now part of the primitive contract.
- Smooth per-frame zoom/split animations remain a follow-up capability; the current code provides
  deterministic transition and zoom descriptors plus reduced-motion completion.

## Related Documents

- `docs/adr/0010-docking-presentation-scene-motion-model.md`
- `docs/plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md`
- `docs/ui/component-contract.md`
- `docs/verification.md`
- `repo-ref/bonsplit/README.md`
