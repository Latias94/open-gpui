---
type: Work Progress
title: Open GPUI command keybinding registry
status: verified
timestamp: 2026-07-03T23:26:02+08:00
git_branch: feat/command-keybinding-registry
tags:
  - open-gpui-command
  - command
  - keymap
  - plugin
---

# Summary

`open_gpui_command` now has a command-id keyed keybinding source registry. Apps and plugins can
register shortcut dictionaries against command ids, then project valid entries into concrete GPUI
`KeyBinding` values through `CommandCenter`.

# Shipped Capability

- Added `CommandKeyBinding`, `CommandKeyBindingRegistry`, `CommandKeyBindingHandle`,
  `CommandKeyBindingProjection`, and diagnostic types.
- Added `CommandCenter` keybinding registry accessors and lifecycle methods:
  `register_key_bindings`, `unregister_key_binding_handle`, `unregister_key_bindings`, and
  `unregister_key_binding_source`.
- Added `CommandCenter::key_binding_projection` and `add_key_bindings_to_keymap`.
- Valid entries clone the registered GPUI action prototype from `GpuiCommandActionMap` and use
  `KeyBinding::load`, so GPUI remains the keyboard parsing and dispatch authority.
- Invalid entries are skipped with diagnostics for missing actions, invalid keystrokes, or invalid
  context predicates.
- UI default exports now include the new command keybinding types.

# Design Notes

This does not replace GPUI's key dispatch engine. Chords still use GPUI whitespace-separated
keystroke sequences, mode checks still use GPUI key binding predicates such as
`Workspace && mode == normal`, and focused-window precedence still comes from GPUI window state.

The registry is source-owned rather than global. Re-registering a source replaces that source's
bindings while preserving registration order for GPUI precedence.

# Verified

```powershell
cargo fmt -p open-gpui-command -p open-gpui-ui-components
cargo nextest run -p open-gpui-command center_projects_command_key_bindings_into_gpui_keymap center_reports_command_key_binding_projection_diagnostics --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-command --no-fail-fast
```

# Next Action

Run final docs/memory validation and `git diff --check`, then commit, merge to `main`, push, and
delete `feat/command-keybinding-registry`.

# Citations

- [Command keybinding module](../../../../crates/open-gpui-command/src/keybinding.rs)
- [Command center facade](../../../../crates/open-gpui-command/src/center.rs)
- [Public API defaults](../../../../crates/ui_components/src/public_api/default.rs)
- [Public surface exports](../../../../crates/ui_components/tests/public_surface/exports.rs)
- [Command ecosystem docs](../../../ui/command-ecosystem.md)
- [Verification evidence](../verification/open-gpui-command-keybinding-registry-20260703.md)
