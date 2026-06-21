---
type: Work Progress
title: Session log
status: active
---

# Log

- 2026-06-21: Stabilized the Components page by moving the directory into its own fixed strip above the page scroll area; replaced the flaky data-grid wheel-motion regression with a stable state-level contract assertion and kept the release queue horizontal scroll smoke as the runtime proof.
- 2026-06-21: Added gallery-level wheel isolation on the ScrollArea sample card so wheel gestures on the release-queue chrome stay local and do not leak to the page shell; kept the release queue runtime scroll proof intact.
- 2026-06-21: Rechecked the splitter and overlay contract surface at `a7f0b96`; focused splitter, overlay, and gallery composition nextest runs remained green, and no new behavior gaps were found.
- 2026-06-21: Added an Overlay gallery smoke for `AlertDialog` on the real trigger, cancel default focus, primary action close, and Escape dismissal; this filled the remaining overlay contract gap without changing the component implementation.
- 2026-06-21: Refreshed `docs/knowledge/engineering/current-state.md` and `docs/ui/component-contract.md` so the gallery scroll regression gate points at commit `14efadc` and the next action stays focused on the remaining overlay / splitter review; later AlertDialog work advanced `main` to `d64f5d6`, then the memory refresh advanced `main` to `a7f0b96`.
- 2026-06-21: Added gallery scroll hardening in `examples/ui-foundation-gallery/src/pages/components/render.rs` and added smoke coverage for navigation rail scrolling, constrained vertical Tabs scrolling, and ScrollArea wheel scrolling in `examples/ui-foundation-gallery/tests/foundation_gallery.rs`.
- 2026-06-21: Focused gallery and component nextest runs passed, including the existing overlay and splitter runtime gates.
- 2026-06-21: Updated `docs/verification.md` to record the new Components-page regression gates.
- 2026-06-20: `f5e5d3a` pushed to `origin/main` for close-recovery test source alignment.
- 2026-06-20: `54304fc` pushed to `origin/main` for the final close-recovery test fix.
- 2026-06-20: Remaining dirty files are confined to `crates/gpui_docking/*`; current pass treats them as likely formatting /整理 noise until proven otherwise.
- 2026-06-20: Focused docking verification completed with `cargo nextest run -p open-gpui-docking --tests` passing 597/597. The current `crates/gpui_docking/*` diff still reads as formatting / import-reorder churn rather than a confirmed behavior change.
- 2026-06-20: `repo-ref/fret` research points to a thin facade + deep helper split for diagnostics: `fretboard` only forwards CLI entry points, `fret-diag` owns the real tooling contract, and scroll/virtual-list logic separates `ScrollHandle` state from `visible_range`/`window_range` policy. That pattern is a better fit for our gallery than extracting a headless crate right now.
- 2026-06-20: Current repo state was refreshed after verification: working tree is clean, `main` matches `origin/main`, and the next meaningful plan should start fresh around scroll / popup / splitter rather than re-opening the old headless discussion.
