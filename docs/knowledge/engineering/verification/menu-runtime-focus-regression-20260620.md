---
type: Verification Evidence
title: Menu runtime focus regression verification
status: complete
timestamp: 2026-06-20
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
related_plan: docs/plans/2026-06-18-001-refactor-ui-component-contract-alignment-plan.md
---

# Verification

- `cargo check -p open-gpui-ui-components` passed after fixing the `menu.rs` enumerate binding regression.
- `cargo nextest run -p open-gpui-ui-components --test components menu_runtime_keyboard_navigation_preserves_focused_value_after_rerender context_menu_runtime_keyboard_navigation_preserves_focused_value_after_rerender` passed.
- The worktree still contains unrelated dirty changes under `crates/gpui_docking/*`; they were not modified.
