---
type: Work Progress
title: Open GPUI command context keymap
status: verified
timestamp: 2026-07-03T21:51:12+08:00
git_branch: feat/command-context-keymap
tags:
  - command
  - keymap
  - ui-components
---

# Summary

`open_gpui_command` now has `CommandContextStack`, the app-owned context input for command scopes
and GPUI key contexts.

# Shipped Capability

- Added `CommandContextStack` with ordered command scopes and ordered GPUI `KeyContext` values.
- `CommandCenter` now owns a context stack instead of a bare active-scope vector.
- Existing `set_active_scopes`, `clear_active_scopes`, and `active_scopes` APIs remain available.
- New `set_context_stack`, `context_stack`, `context_stack_mut`, `set_key_contexts`,
  `clear_key_contexts`, and `key_contexts` APIs let apps drive scope and keymap projection together.
- App-level `snapshot_for_keymap`, `search_snapshot_for_keymap`, menu projection, and shortcut
  diagnostics now use context-aware keymap shortcut projection.
- Provider requests receive the same active command scopes from the context stack.
- `GpuiCommandActionMap` now exposes `registry_snapshot_with_keymap_shortcuts_in_context` and
  `shortcut_diagnostics_for_keymap_in_context` for direct snapshot callers.
- UI component root/prelude default exports include `CommandContextStack`.
- The foundation gallery now has a `context-stack` command sample proving focused scope descriptor
  override and focused editor shortcut projection.

# Design Notes

Command scopes and GPUI key contexts are intentionally adjacent but not merged. Command scopes
decide which command descriptors are visible. GPUI key contexts decide which app-level keymap
bindings are active for display and diagnostics. Focused-window projection still delegates to
`Window::highest_precedence_binding_for_action`, because GPUI owns the live rendered focus tree.

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

Run final focused verification, commit this slice, merge it to `main`, push, and delete
`feat/command-context-keymap`.

# Citations

- [Command context stack](../../../../crates/open-gpui-command/src/context.rs)
- [Command center context integration](../../../../crates/open-gpui-command/src/center.rs)
- [GPUI command action map](../../../../crates/open-gpui-command/src/gpui.rs)
- [Gallery command samples](../../../../examples/ui-foundation-gallery/src/pages/components/samples/choice.rs)
- [Command ecosystem docs](../../../ui/command-ecosystem.md)
- [Verification evidence](../verification/open-gpui-command-context-keymap-20260703.md)
