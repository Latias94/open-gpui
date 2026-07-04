---
type: Work Progress
title: Open GPUI command keymap resolution
status: done
timestamp: 2026-07-04T10:26:22+08:00
git_branch: feat/command-keymap-scopes
---

# Summary

`open_gpui_command` now exposes command-aware GPUI keymap resolution. Apps can parse a
whitespace-separated key sequence, resolve it through GPUI's `Keymap` and active `KeyContext`
stack, map matched GPUI actions back to command ids, and inspect whether the sequence is still
pending as a chord.

# Shipped

- Added `parse_command_key_sequence`, `CommandKeymapResolution`,
  `CommandKeymapResolvedCommand`, and `CommandKeymapCommandState`.
- Added `GpuiCommandActionMap::resolve_keymap_input`,
  `GpuiCommandActionMap::resolve_keymap_sequence`, and `command_id_for_action`.
- Added `CommandCenter::resolve_key_input_for_keymap` and
  `CommandCenter::resolve_key_sequence_for_keymap`, using the center's active scopes,
  availability map, and key contexts.
- Exposed the new API through the `open_gpui_ui_components` default root/prelude surface.
- Documented the keymap resolution boundary in `docs/ui/command-ecosystem.md`.

# Notes

GPUI remains the dispatch and chord authority. This layer is intentionally a typed projection and
preflight API: it reports matched commands, command-specific pending continuations, and
availability state without dispatching actions.

