---
title: "feat: Add docking splitter resize"
type: feat
status: completed
date: 2026-06-08
---

# feat: Add docking splitter resize

## Summary

Add interactive splitter handles to `open-gpui-docking` so users can resize adjacent split children while preserving canonical, finite, normalized split fractions. This phase builds on the completed `DockWorkspace` owner seam and keeps tab drag/drop, preview overlays, and floating chrome deferred.

## Problem Frame

The docking graph already stores split fractions and exposes `DockOp::SetSplitFractions` plus `DockOp::SetSplitFractionTwo`, and static rendering already displays split children using normalized flex shares. The missing piece is the interaction path: rendered split handles, pure fraction math with minimum-size clamping, and tests proving a pointer drag updates graph state through the retained host.

This is the smallest useful next interaction after tab activation. It validates mouse-driven mutation through the owner-backed `DockHost` before the broader drag/drop resolver and overlay work begins.

## Requirements

- R1. Render a splitter handle between adjacent children of every split node.
- R2. Resize adjacent split panes by updating graph fractions through checked docking operations.
- R3. Preserve finite, non-negative fractions whose normalized sum is `1.0`.
- R4. Clamp adjacent pane sizes to a configurable minimum size.
- R5. Keep split resize state out of `DockGraph`; the graph stores only layout structure and fractions.
- R6. Keep existing tab selection, static split rendering, missing-panel placeholders, and deferred floating placeholders working.
- R7. Cover the resize math with pure unit tests and the rendered behavior with GPUI visual interaction tests.

## Scope Boundaries

In scope:

- A `splitter` module for pure fraction updates.
- Splitter handle rendering inside `crates/gpui_docking/src/render.rs`.
- A host option for minimum split pane size and handle thickness.
- Owner-backed mutation through existing graph operation paths.
- Focused tests in `crates/gpui_docking/src/host_tests.rs` and module tests for `splitter.rs`.

Deferred:

- Tab drag/drop, drop-target resolution, and preview overlays.
- In-window floating chrome and floating drag.
- Nested same-axis coupled resizing beyond adjacent-child fraction updates unless required by tests.
- Platform-window detach or cross-window routing.

## Key Technical Decisions

- KTD1. Use pure math first: `splitter.rs` computes updated fractions from current fractions, split extent, handle delta, and minimum pane size without reading GPUI state.
- KTD2. Update adjacent panes only: dragging handle `i` changes children `i` and `i + 1`, then renormalizes the full fraction vector.
- KTD3. Render handles as normal GPUI elements: handles sit between split child wrappers and listen for mouse down, move, and up events.
- KTD4. Keep graph purity: drag state and pixel bounds live in render/host state; persisted layout remains only dock spaces, nodes, items, fractions, and floating bounds.
- KTD5. Test through rendered bounds: visual tests assert split child bounds before and after a simulated drag instead of relying on screenshots.

## Implementation Units

### U1. Splitter Math

**Goal:** Add a pure helper that updates adjacent split fractions with minimum-size clamping.

**Files:**

- `crates/gpui_docking/src/splitter.rs`
- `crates/gpui_docking/src/lib.rs`

**Test scenarios:**

- Horizontal-style positive delta increases the first pane and decreases the second pane.
- Negative delta decreases the first pane and increases the second pane.
- Dragging past either minimum clamps both adjacent panes.
- Mismatched, non-finite, or zero-sum input fractions are repaired before update.
- Invalid handle indexes return `None` instead of panicking.

### U2. Host Options And Debug Regions

**Goal:** Make splitter handles configurable and observable in tests.

**Files:**

- `crates/gpui_docking/src/host.rs`
- `crates/gpui_docking/src/debug.rs`

**Test scenarios:**

- Default options expose a positive handle thickness and minimum pane size.
- A rendered split emits a debug selector for each splitter handle.

### U3. Rendered Splitter Interaction

**Goal:** Render handles between split children and update fractions during drag.

**Files:**

- `crates/gpui_docking/src/render.rs`
- `crates/gpui_docking/src/host_tests.rs`

**Test scenarios:**

- Dragging a horizontal split handle changes child widths and preserves normalized graph fractions.
- Dragging a vertical split handle changes child heights and preserves normalized graph fractions.
- Dragging beyond the configured minimum clamps the affected pane.
- A mouse-up clears active splitter drag state.

## Verification

- `cargo fmt --check`
- `cargo nextest run -p open-gpui-docking`
- `cargo clippy -p open-gpui-docking --all-targets --no-deps -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" cargo doc -p open-gpui-docking --no-deps`
