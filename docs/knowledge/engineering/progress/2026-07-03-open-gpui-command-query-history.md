---
type: Work Progress
title: Open GPUI command query history ergonomics
status: verified
timestamp: 2026-07-03T22:58:55+08:00
git_branch: feat/command-query-history
tags:
  - open-gpui-command
  - command
  - ui-components
  - history
---

# Summary

`CommandCenter` now exposes query-history navigation directly, and
`CommandPaletteController` wraps it for palette surfaces that want Cmd+K-style up/down query
recall.

# Shipped Capability

- Added `CommandCenter::record_query`, `recent_queries`, `previous_query`, `next_query`, and
  `reset_query_navigation` as the app-facing facade over `MemoryCommandHistory`.
- `MemoryCommandHistory::record_query` now promotes an existing duplicate query to the newest
  position instead of keeping stale duplicate entries.
- `CommandPaletteController` now stores the per-surface query-history prefix captured when history
  navigation starts.
- Added `previous_query_for_keymap`, `next_query_for_keymap`,
  `previous_query_for_window`, and `next_query_for_window`.
- History navigation refreshes configured providers and returns a complete
  `CommandPaletteControllerUpdate`, so callers can immediately re-render `Command`.
- Moving past the newest matching query restores the draft query that was present before history
  navigation began.

# Design Notes

The durable query history remains owned by `CommandCenter`; the controller only owns the temporary
navigation prefix for one palette surface. This keeps app/workspace history shared while allowing
multiple palette surfaces to browse independently.

The API intentionally stays action-free. Applications still decide which physical keybindings or
Vim-style modes call these controller methods.

# Verified

```powershell
cargo fmt -p open-gpui-command -p open-gpui-ui-components
cargo nextest run -p open-gpui-command center_exposes_query_history_navigation memory_history_promotes_duplicate_queries memory_history_navigates_recent_queries_with_prefix --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_palette_controller_navigates_query_history_with_prefix --no-fail-fast
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_palette_controller --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
```

# Next Action

Run docs/memory validation and `git diff --check`, commit this slice, merge it to `main`, push, and
delete `feat/command-query-history`.

# Citations

- [Command center facade](../../../../crates/open-gpui-command/src/center.rs)
- [Memory command history](../../../../crates/open-gpui-command/src/history.rs)
- [Command palette controller](../../../../crates/ui_components/src/command/descriptor.rs)
- [Command controller tests](../../../../crates/ui_components/tests/choice.rs)
- [Command ecosystem docs](../../../ui/command-ecosystem.md)
- [Verification evidence](../verification/open-gpui-command-query-history-20260703.md)
