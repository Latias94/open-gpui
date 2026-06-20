---
type: Current State
title: Open GPUI current engineering state
status: active
timestamp: 2026-06-20
---

# Current State

- Goal: Continue UI component contract alignment and remove evidence-backed behavioral drift without preserving old compatibility layers.
- Branch: `main`
- Last verified: `cargo check -p open-gpui-ui-foundation-gallery --tests` after moving focus/a11y and overlay shell-local state into page modules, formatting with `cargo fmt --all -- examples/ui-foundation-gallery/src/shell.rs examples/ui-foundation-gallery/src/pages/focus_a11y.rs examples/ui-foundation-gallery/src/pages/overlay.rs examples/ui-foundation-gallery/tests/foundation_gallery.rs`, and confirming the old Components-page crash still is not reproducible in this checkout.
- Done: Re-checked the Components-page crash line and `repo-ref/fret`'s scroll/list-box helper layering. Also moved Focus & A11y and Overlay page-local shell state out of `shell.rs` into page modules so the shell only retains cross-page/navigation state.
- In progress: The gallery shell refactor is mid-cleanup but compile-healthy; the remaining work is to finish review and decide whether to commit the page-state extraction.
- Blocked: None.
- Next action: Run targeted gallery tests, then either commit the shell/page-state extraction or stop if no further evidence-backed drift appears.

# Citations

[1] [Plan](../../plans/2026-06-18-001-refactor-ui-component-contract-alignment-plan.md)
[2] [Verification](verification/menu-runtime-focus-regression-20260620.md)
