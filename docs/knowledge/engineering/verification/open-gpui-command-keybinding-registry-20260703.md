---
type: Verification Evidence
title: Open GPUI command keybinding registry verification
status: verified
timestamp: 2026-07-03T23:26:02+08:00
git_branch: feat/command-keybinding-registry
tags:
  - open-gpui-command
  - command
  - keymap
---

# Scope

Focused verification for command-id keyed keybinding source registration and projection.

# Evidence

Added failing proofs first:

- `center_projects_command_key_bindings_into_gpui_keymap` failed because `CommandCenter` had no
  keybinding registry, projection, or add-to-keymap API.
- `center_reports_command_key_binding_projection_diagnostics` failed because keybinding diagnostic
  types did not exist.

After implementation, these gates passed:

```powershell
cargo fmt -p open-gpui-command -p open-gpui-ui-components
cargo nextest run -p open-gpui-command center_projects_command_key_bindings_into_gpui_keymap center_reports_command_key_binding_projection_diagnostics --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-command --no-fail-fast
```

# Coverage

- Command-id keybinding sources project into GPUI `KeyBinding` values through registered action
  prototypes.
- GPUI chord behavior remains active for generated bindings.
- GPUI key context predicate syntax remains the mode/context contract.
- Source handles unregister their keybinding entries.
- Missing actions and invalid context predicates produce diagnostics and skip bad entries.
- Root and prelude public surface exports include the new keybinding types.

# Citations

- [Command center tests](../../../../crates/open-gpui-command/src/center.rs)
- [Command keybinding module](../../../../crates/open-gpui-command/src/keybinding.rs)
- [Public surface exports](../../../../crates/ui_components/tests/public_surface/exports.rs)
