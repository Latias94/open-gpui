---
type: Verification Evidence
title: Open GPUI command query history verification
status: verified
timestamp: 2026-07-03T22:58:55+08:00
git_branch: feat/command-query-history
tags:
  - open-gpui-command
  - command
  - ui-components
  - history
---

# Scope

Focused verification for the command query-history facade and command palette controller history
navigation.

# Evidence

Added failing proofs first:

- `center_exposes_query_history_navigation` failed because `CommandCenter` did not expose query
  history methods.
- `command_palette_controller_navigates_query_history_with_prefix` failed because
  `CommandPaletteController` had no query-history navigation API.

After implementation, the following gates passed:

```powershell
cargo fmt -p open-gpui-command -p open-gpui-ui-components
cargo nextest run -p open-gpui-command center_exposes_query_history_navigation memory_history_promotes_duplicate_queries memory_history_navigates_recent_queries_with_prefix --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_palette_controller_navigates_query_history_with_prefix --no-fail-fast
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_palette_controller --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
```

# Coverage

- `CommandCenter` records and navigates recent queries without requiring callers to use
  `history_mut()`.
- Duplicate query recording promotes the newest occurrence and keeps recent-query output unique.
- Controller-level history navigation captures the current query as the matching prefix.
- Controller-level next navigation restores the captured draft query after moving past the newest
  matching history entry.
- Controller-level history changes still produce `CommandPaletteProjection` updates.
- Public-surface tests remain green after adding methods to the exported
  `CommandPaletteController` type.

# Citations

- [Command center tests](../../../../crates/open-gpui-command/src/center.rs)
- [History tests](../../../../crates/open-gpui-command/src/history.rs)
- [UI controller tests](../../../../crates/ui_components/tests/choice.rs)
