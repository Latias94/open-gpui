---
title: "feat: Add Tree virtualized render window"
type: feat
date: 2026-06-26
---

# feat: Add Tree virtualized render window

## Summary

Add an opt-in fixed-row virtualized render window for the official `Tree` component. The slice
keeps `TreeState` as the visible flattened hierarchy contract and adds a renderer-facing
`TreeRenderPlan` that reuses `VirtualizerState::resolve_fixed_window` for large visible row sets.
It does not replace the current flattening model or implement a full async tree data source.

---

## Requirements

- R1. `TreeRenderPlan` exposes the resolved `TreeState`, `VirtualizerResolvedState`, and rendered
  row window without materializing row measurements outside the overscan range.
- R2. The GPUI `Tree` adapter can opt into virtualized rendering while preserving the existing
  non-virtualized default.
- R3. Virtualized Tree rendering uses fixed row height from `TreeMetrics` and stable row keys.
- R4. The adapter keeps scroll ownership local to the Tree `ScrollArea`; wheel input must not move
  the Components page.
- R5. Existing Tree selection, expansion, lazy-branch metadata, and typeahead contracts remain
  intact for non-virtualized trees.
- R6. The Components gallery adds a large Tree sample and smoke coverage for the initial rendered
  window plus shell-contained Tree navigation.
- R7. Public exports, component API inventory, contract docs, verification docs, and engineering
  memory reflect the shipped boundary.

---

## Non-Goals

- Incremental full-tree flattening or data-source traits.
- Dynamic row heights.
- Drag-and-drop hierarchy editing.
- Reworking Tree focus around a root active-descendant model.
- Searching unloaded or collapsed descendants.

---

## Implementation Units

### U1. Render-plan contract

- Add `TreeRowRenderPlan` and `TreeRenderPlan`.
- Resolve fixed-height virtual rows from `TreeState` and `VirtualizerState`.
- Add component tests for visible/overscan ranges and stable row metadata.

### U2. Adapter opt-in

- Add `Tree::virtualized`, `Tree::viewport_item_count`, and `Tree::overscan_count`.
- Render virtualized rows inside a relative-height body when enabled.
- Keep existing full rendering path as the default.

### U3. Gallery proof

- Add a large `release-outline` or similar Tree sample.
- Add smoke coverage proving the initial Tree window stays mounted and page scrolling remains
  outside the sample shell.

### U4. Docs and verification

- Update exports/inventory baselines, component contract, verification notes, and engineering
  memory.
- Run focused nextest gates and `git diff --check`.
