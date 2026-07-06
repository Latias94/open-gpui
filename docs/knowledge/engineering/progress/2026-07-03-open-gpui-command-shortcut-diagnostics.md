---
type: Work Progress
title: Open GPUI command shortcut diagnostics
status: verified
timestamp: 2026-07-03T19:08:50+08:00
git_branch: feat/command-app-integration-diagnostics
tags:
  - command
  - diagnostics
  - ui-components
---

# Summary

`open_gpui_command` now exposes shortcut diagnostics for validating the join between command
metadata, GPUI action bindings, and effective keymap/window shortcut projection.

# Shipped Capability

- Added `CommandShortcutDiagnostic` and `CommandShortcutDiagnosticKind`.
- Added `GpuiCommandActionMap::shortcut_diagnostics_for_keymap` and
  `shortcut_diagnostics_for_window` for strict snapshot diagnostics.
- Diagnostics cover missing actions, orphan actions, missing shortcuts, and duplicated projected
  shortcut labels.
- Added `CommandCenter::shortcut_diagnostics_for_keymap` and
  `shortcut_diagnostics_for_window` for the recommended app-owned facade.
- Re-exported the diagnostic types through the curated `open_gpui_ui_components` root/prelude
  default surface.
- Extended the foundation gallery `registry-dispatch` command sample to retain an empty healthy
  shortcut diagnostic set as part of its command-center proof.

# Design Notes

The lower-level `GpuiCommandActionMap` is intentionally strict: it diagnoses against exactly the
snapshot it receives and reports action bindings outside that snapshot as `OrphanAction`.

`CommandCenter` diagnoses against the current visible snapshot but suppresses orphan diagnostics
for command ids that still exist in active scoped sources and are only hidden by availability. That
keeps app validation useful without treating a hidden command as a stale plugin action.

This remains a diagnostics layer, not a second shortcut engine. GPUI `Action`, `Keymap`, and
focused-window binding precedence remain the runtime authority.

# Verified

```powershell
cargo fmt -p open-gpui-command -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts components_page_samples_expose_component_metadata components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
python C:\Users\Frankorz\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
git diff --check
```

# Next Action

Commit this slice, merge it to `main`, push, and delete
`feat/command-app-integration-diagnostics`.

# Citations

- [Command GPUI adapter](../../../../crates/open-gpui-command/src/gpui.rs)
- [Command center facade](../../../../crates/open-gpui-command/src/center.rs)
- [Gallery command sample](../../../../examples/ui-foundation-gallery/src/pages/components/samples/choice.rs)
- [Command ecosystem docs](../../../ui/command-ecosystem.md)
- [Verification evidence](../verification/open-gpui-command-shortcut-diagnostics-20260703.md)
