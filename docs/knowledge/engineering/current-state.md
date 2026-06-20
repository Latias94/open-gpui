---
type: Current State
title: Open GPUI current engineering state
status: active
timestamp: 2026-06-20
---

# Current State

- Goal: Continue UI component contract alignment and remove evidence-backed behavioral drift without preserving old compatibility layers.
- Branch: `main`
- Last verified: `cargo fmt --all --check` and `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` after tightening the Components-page choice/search contract so `Select`, `Combobox`, and `Command` now keep `selected_value` and `active_value` distinct in gallery samples and tests.
- Done: Re-checked the Components-page crash line and `repo-ref/fret`'s scroll/list-box helper layering. Also moved Focus & A11y and Overlay page-local shell state out of `shell.rs` into page modules so the shell only retains cross-page/navigation state. The latest choice/search scan did not find a safe deletion seam, so the work shifted to explicit contract hardening and was committed as `b66b5a0`.
- In progress: Current automatically discoverable gallery contract drift is closed. The checkout still has unrelated `crates/gpui_docking/*` dirty files that were intentionally not staged or committed in this pass.
- In progress: The current `crates/gpui_docking/*` dirt is a close-recovery contract cleanup pass. The suspected `close_recovery_does_not_reveal_hidden_recorded_panel` failure was a test-source mismatch, not a runtime regression. Close-recovery and merge-back tests now pass on focused `cargo nextest run -p open-gpui-docking` checks.
- Blocked: None.
- Next action: If continuing the architecture loop, either commit the current `gpui_docking` cleanup or start a fresh plan for the next UI component slice; do not re-open the completed gallery choice/search contract pass without new evidence.

# Citations

[1] [Plan](../../plans/2026-06-18-001-refactor-ui-component-contract-alignment-plan.md)
[2] [Verification](verification/menu-runtime-focus-regression-20260620.md)
