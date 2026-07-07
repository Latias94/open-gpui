---
type: Work Progress
title: Runtime canvas docking depth refactor final pass
status: complete
timestamp: 2026-07-07T12:09:14Z
git_branch: main
related_plan: docs/plans/2026-07-07-003-refactor-runtime-canvas-docking-depth-plan.md
tags:
  - ce-work
  - canvas
  - docking
  - gpui-runtime
---

# Summary

The runtime/canvas/docking depth refactor plan is implemented on local `main` through the final verification pass. The work split Canvas document/tool/GPUI adapter internals, Docking viewport route/drop/runtime internals, and GPUI app/window/frame internals into narrower private modules, then added final guardrails for public surface drift.

# Final Follow-Ups Landed

- Added Canvas root public-surface guard tests so the crate facade keeps explicit exports while split internals stay private.
- Moved Docking split-only imports behind test configuration so workspace checks remain warning-clean.
- Documented focused verification gates for Canvas document/tool/GPUI/root-facade work and Docking drop route/delivery/public-surface work.
- Updated `ScrollArea` component API inventory to include the existing `on_scroll_viewport_changed` public method and callback baseline.
- Conditioned the `MouseButton` import in `crates/gpui/src/window.rs` for non-wasm builds to keep stable wasm checks warning-clean.

# Review

Two read-only review agents checked the current refactor range. Both reported no blocking findings. Residual suggestions were resolved by adding the Canvas public-surface guard and verification-doc focused gates.

# Commit State

The branch was `main`, ahead of `origin/main` by 12 commits before this final pass. The final pass is ready to commit after the verification evidence in `../verification/runtime-canvas-docking-depth-20260707.md`.

# Citations

- Plan: `docs/plans/2026-07-07-003-refactor-runtime-canvas-docking-depth-plan.md`
- Verification: `docs/knowledge/engineering/verification/runtime-canvas-docking-depth-20260707.md`
- Review agents: `/root/u12_readonly_review`, `/root/u12_public_surface_review`
