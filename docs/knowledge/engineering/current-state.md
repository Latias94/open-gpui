---
type: Current State
title: Open GPUI current engineering state
status: active
timestamp: 2026-06-20
---

# Current State

- Goal: Continue UI component contract alignment and remove evidence-backed behavioral drift without preserving old compatibility layers.
- Branch: `main`
- Last verified: `cargo check -p open-gpui-ui-components`; `cargo nextest run -p open-gpui-ui-components --test components menu_runtime_keyboard_navigation_preserves_focused_value_after_rerender context_menu_runtime_keyboard_navigation_preserves_focused_value_after_rerender`
- Done: Fixed a `menu.rs` enumerate binding regression that broke `open-gpui-ui-components` compilation, and re-verified the menu/context-menu rerender focus tests.
- In progress: Deciding whether to continue the contract alignment loop or stop after this focused repair.
- Blocked: None.
- Next action: Use the confirmed contract plan to pick the next evidence-backed UI behavior mismatch.

# Citations

[1] [Plan](../../plans/2026-06-18-001-refactor-ui-component-contract-alignment-plan.md)
[2] [Verification](verification/menu-runtime-focus-regression-20260620.md)
