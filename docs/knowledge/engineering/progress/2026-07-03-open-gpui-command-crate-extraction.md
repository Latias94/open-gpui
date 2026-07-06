---
type: Work Progress
title: Open GPUI command crate extraction
status: active
timestamp: 2026-07-03T23:59:00+08:00
git_branch: refactor/open-gpui-command-crate
related_plan: docs/plans/2026-07-03-002-refactor-open-gpui-command-crate-plan.md
tags:
  - command
  - architecture
  - ui-components
---

# Summary

`open_gpui_command` is now the canonical command ecosystem crate. The old
`open_gpui_ui_core::command` owner was deleted instead of preserved as a compatibility alias.

# Shipped Capability

- `crates/open-gpui-command` owns `CommandDescriptor`, `CommandContribution`,
  `CommandRegistry`, `CommandRegistrySnapshot`, duplicate-id errors, and source ids.
- `ScopedCommandRegistry` projects active scopes in caller order, emits duplicate override
  diagnostics, and supports source/scope unregistration.
- `CommandAvailabilityMap` projects commands as available, disabled with optional reason, or hidden.
- `CommandMenuTree` builds neutral menu hierarchy from `menu_path`.
- `MemoryCommandHistory` records in-memory command usage/query hints and can rank registry snapshots.
- `GpuiCommandActionMap` moved out of `ui_components` and now dispatches command ids directly,
  including availability-guarded dispatch and usage recording on successful dispatch.
- `ui_components` consumes command metadata for `Command`, `Menu`, and `ContextMenu` projection but
  no longer owns command registries or GPUI command adapter helpers.

# Verified So Far

```powershell
cargo check -p open-gpui-command --tests
cargo check -p open-gpui-ui-components --tests
cargo check -p open-gpui-ui-foundation-gallery --tests
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components command --no-fail-fast
cargo nextest run -p open-gpui-ui-components menu context_menu --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
```

# Next Action

Finish the final verification sweep: formatting, docs/memory validation, focused gallery command
tests, `git diff --check`, and an `xtask verify` attempt if feasible.

# Citations

- [Plan](../../../plans/2026-07-03-002-refactor-open-gpui-command-crate-plan.md)
- [Command crate](../../../../crates/open-gpui-command/src/lib.rs)
