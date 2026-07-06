---
type: Verification Evidence
title: Open GPUI command keybinding conflicts verification
status: active
timestamp: 2026-07-04T00:10:27+08:00
git_branch: feat/command-keybinding-conflicts
related_progress:
  - progress/2026-07-04-open-gpui-command-keybinding-conflicts.md
---

# Evidence

- Proof-first red check:
  `cargo nextest run -p open-gpui-command center_reports_command_key_binding_conflicts_and_install_report --no-fail-fast`
  failed before implementation because `CommandKeyBindingProjection::conflicts` and
  `CommandCenter::install_key_bindings` did not exist.
- Focused green check:
  `cargo nextest run -p open-gpui-command center_reports_command_key_binding_conflicts_and_install_report --no-fail-fast`
  passed after implementation with 1/1 tests passing.
- Focused command keybinding check:
  `cargo nextest run -p open-gpui-command center_reports_command_key_binding_conflicts_and_install_report center_reports_global_key_binding_context_conflicts center_projects_command_key_bindings_into_gpui_keymap center_reports_command_key_binding_projection_diagnostics --no-fail-fast`
  passed with 4/4 tests passing.
- Public surface check:
  `cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast` passed
  with 36/36 tests passing.
- Full command crate check:
  `cargo nextest run -p open-gpui-command --no-fail-fast` passed with 49/49 tests passing.
- Formatting check:
  `cargo fmt -p open-gpui-command -p open-gpui-ui-components --check` passed.
- Engineering memory validation:
  `python C:\Users\Frankorz\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering`
  passed.
- Whitespace check:
  `git diff --check` passed with only existing Windows LF/CRLF warnings.

# Notes

The focused tests prove that same-context command shortcut conflicts and global-vs-context
conflicts are reported without blocking installation, and that GPUI still returns the later binding
first for same-depth dispatch.
