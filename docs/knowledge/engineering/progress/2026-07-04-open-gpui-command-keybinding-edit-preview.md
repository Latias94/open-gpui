---
type: Work Progress
title: Open GPUI command keybinding edit preview
tags:
  - open-gpui
  - command
  - keymap
  - gallery
timestamp: 2026-07-04T00:00:00Z
git_branch: main
---

# Summary

The command keybinding editor now has an edit-preview workflow:

- `CommandKeyBindingEditTarget` identifies a source binding by source id, command id, raw
  keystrokes, and raw context.
- `CommandKeyBindingPatch` represents app-owned add, replace, and remove edits.
- `CommandKeyBindingRegistry::preview_patch` and `CommandCenter::preview_key_binding_patch` apply a
  patch to a cloned candidate registry and return `CommandKeyBindingPatchPreview`.
- `CommandKeyBindingCaptureState` parses captured key sequences for UI use.
- `CommandKeyBindingEditorPreviewState` adapts patch previews into UI-ready rows, diagnostics, and
  conflicts.
- The Components gallery `keymap-resolution` command sample now dogfoods a captured
  `ctrl-k ctrl-s` replacement patch that resolves the sample conflict while preserving existing
  diagnostics.

# Boundary

This remains a preview and handoff layer. It does not persist keymap files, choose source priority,
or mutate GPUI keymaps. Callers can inspect the patch preview and then persist the app-owned patch
using their own keymap storage.

# Next Action

The next layer can introduce an actual key-capture widget/runtime adapter that listens to GPUI
keystroke events and emits `CommandKeyBindingCaptureState`, while keeping persistence outside the
component.

