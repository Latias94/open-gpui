---
type: "Work Progress"
title: "DevTools layout snapshot foundation"
description: "U4 completed for DevTools ecosystem deepening."
timestamp: 2026-07-09T02:21:05Z
tags: ["devtools", "layout", "gpui", "gallery", "ce-work"]
related_plan: "docs/plans/2026-07-09-001-feat-devtools-ecosystem-deepening-plan.md"
git_branch: "feat/devtools-ecosystem-deepening"
git_commit: "aedc2bf1"
verified_by: "cargo check -p open-gpui-devtools --features gpui --tests --locked; cargo nextest run -p open-gpui-devtools --features gpui layout --no-fail-fast --locked; cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked; cargo check -p open-gpui-devtools --all-features --tests --locked"
---

# Summary

- Completed U4 layout inspection snapshot foundation.
- Added `open_gpui_devtools::layout` with committed geometry DTOs and sanitized tree/envelope conversion.
- Added GPUI scroll viewport -> layout snapshot adapters and registry-backed gallery dogfood via `layout.scroll-viewport`.

# Details

- Layout facts stay renderer-neutral in `layout.rs`; GPUI-specific conversion lives in `gpui.rs`.
- The layout sample uses public `ScrollViewportSnapshot` facts: bounds, content size, scroll offset, max offset, generation, and source.
- Existing unavailable scroll/docking diagnostics remain in the gallery because the deterministic layout sample is not a live mounted runtime.

# Next Action

- Start U5: render inspector category summaries, finalize docs, run final verification gates, then review/merge/push.

# Citations

- Commit: `aedc2bf1`
- Plan: `docs/plans/2026-07-09-001-feat-devtools-ecosystem-deepening-plan.md`
- Files: `crates/devtools/src/layout.rs`, `crates/devtools/src/gpui.rs`, `crates/devtools/tests/layout_adapters.rs`, `examples/ui-foundation-gallery/src/pages/devtools.rs`
