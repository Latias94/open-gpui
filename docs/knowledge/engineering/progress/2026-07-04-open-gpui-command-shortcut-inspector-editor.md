---
type: Work Progress
title: Open GPUI command shortcut inspector and keybinding editor state
tags:
  - open-gpui
  - command
  - keymap
  - gallery
timestamp: 2026-07-04T00:00:00Z
git_branch: main
---

# Summary

The command ecosystem now has UI-ready read models for the two Zed-inspired surfaces the user
approved:

- `CommandShortcutInspectorState` projects a `CommandPaletteKeymapPreflight` into command rows that
  app shells can render for captured shortcut input.
- `CommandKeyBindingEditorState` projects a `CommandKeyBindingProjection` into filterable valid
  binding rows, conflict counts, conflicts, and diagnostics.
- `CommandKeyBindingProjection::projected_entries()` exposes valid projected binding metadata
  without forcing UI code to inspect GPUI `KeyBinding` internals.
- The Components gallery `keymap-resolution` Command sample now renders keymap resolutions,
  shortcut inspector state, and keybinding editor conflicts/diagnostics together.

# Design Notes

The boundary follows `repo-ref/zed`:

- command palette remains responsible for query/search and dispatch preflight;
- shortcut inspector is a readout over a typed preflight result;
- keybinding editor is a separate projection over keybinding sources, conflicts, and diagnostics;
- GPUI `Keymap` remains the runtime authority for parsing, context predicates, chords, and
  dispatch precedence.

This deliberately stops before persistence/editing policy. Apps still own user keymap files, source
priority, undo/rollback, and whether conflicts are warnings or install blockers.

# Touched Areas

- `crates/open-gpui-command/src/keybinding.rs`
- `crates/ui_components/src/command/descriptor.rs`
- `examples/ui-foundation-gallery/src/pages/components/samples/choice.rs`
- `examples/ui-foundation-gallery/src/shell/components.rs`
- `docs/ui/command-ecosystem.md`

# Next Action

The next meaningful command slice is an actual editable keybinding workflow: capture a new
keystroke sequence, validate it through `CommandKeyBindingRegistry`, preview conflicts, then hand an
app-owned patch object back to the caller for persistence.

