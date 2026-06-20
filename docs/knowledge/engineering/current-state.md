---
type: Current State
title: Open GPUI current engineering state
status: active
timestamp: 2026-06-20
---

# Current State

- Goal: Continue UI component contract alignment and remove evidence-backed behavioral drift without preserving old compatibility layers.
- Branch: `main`
- Last verified: `cargo nextest run -p open-gpui-ui-foundation-gallery --tests` after tightening the Components-page choice/search contract so `Select` and `Command` now keep `selected_value` and `active_value` distinct in gallery samples and tests.
- Done: Re-checked the Components-page crash line and `repo-ref/fret`'s scroll/list-box helper layering. Also moved Focus & A11y and Overlay page-local shell state out of `shell.rs` into page modules so the shell only retains cross-page/navigation state. The latest choice/search scan did not find a safe deletion seam, so the work shifted to contract hardening instead of more field removal.
- In progress: The gallery contract pass is still compile-healthy; the remaining work is to record the new choice/search evidence and decide whether to commit the contract hardening.
- Blocked: None.
- Next action: Commit the choice/search contract hardening if the diff remains scoped to the gallery sample/test files.

# Citations

[1] [Plan](../../plans/2026-06-18-001-refactor-ui-component-contract-alignment-plan.md)
[2] [Verification](verification/menu-runtime-focus-regression-20260620.md)
