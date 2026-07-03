---
type: Work Progress
title: Open GPUI command keybinding conflicts and install reports
status: active
timestamp: 2026-07-04T00:10:27+08:00
git_branch: feat/command-keybinding-conflicts
related_progress:
  - progress/2026-07-03-open-gpui-command-keybinding-registry.md
---

# Summary

`open_gpui_command` now extends the command-id keybinding registry with conservative conflict
reporting and explicit install reports.

# Changes

- Added `CommandKeyBindingConflict` and `CommandKeyBindingConflictEntry` to report shortcut entries
  that normalize to the same GPUI keystroke display string and the same normalized GPUI context
  predicate while targeting different command ids.
- Global no-context bindings are also reported against concrete same-keystroke context bindings,
  matching GPUI runtime behavior where no-context bindings remain active in focused contexts.
- Added `CommandKeyBindingInstallReport` so app shells can append projected command bindings into a
  GPUI `Keymap` or app-level keymap and inspect installed count, skipped-entry diagnostics, and
  conflicts from one value.
- Added `CommandCenter::install_key_bindings` and `CommandCenter::install_key_bindings_in_app`.
- Kept the previous `add_key_bindings_to_keymap` projection-returning API as compatibility sugar.
- Exposed the new conflict/report types through the `open-gpui-ui-components` root and prelude
  public surface.

# Design Notes

- Conflicts are warnings, not projection failures. Valid bindings still install, and GPUI remains
  the dispatch and precedence authority.
- `CommandKeyBindingProjection::is_clean()` keeps its compatibility meaning: no skipped-entry
  projection diagnostics. `has_conflicts()` and `is_strictly_clean()` express conflict state.
- Conflict detection is intentionally conservative: same normalized keystrokes plus same normalized
  context predicate, with the extra global-vs-context case above. It does not try to solve
  arbitrary overlapping GPUI predicates.
- GPUI `Keymap` exposes append and clear, but not source-level removal. The install report therefore
  does not claim lifecycle-based uninstall from an external keymap. Plugin hosts that need live
  reload should rebuild their command-owned keymap layer before reinstalling.

# Next Action

Run the full focused verification set, update verification memory with final evidence, then commit
the branch if clean.
