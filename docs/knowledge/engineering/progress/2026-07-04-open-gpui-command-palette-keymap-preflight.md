---
type: Work Progress
title: Open GPUI command palette keymap preflight
status: done
timestamp: 2026-07-04T00:00:00+08:00
git_branch: main
---

# Summary

`CommandPaletteController` now exposes a UI-layer keymap dispatch preflight for app shells and
shortcut inspectors. The pure command crate still owns keymap resolution; the palette controller
adds the currently visible palette query so callers can dispatch with the same query that produced
the palette state.

# Shipped

- Added `CommandPaletteKeymapPreflight` to `open-gpui-ui-components`.
- Added `CommandPaletteController::preflight_key_sequence_for_keymap`, returning the preflight
  wrapper without dispatching.
- Exported the preflight type through the crate root and prelude public surface.
- Updated the Components gallery keymap-resolution sample to consume the controller preflight path.
- Documented when to use controller preflight versus `CommandCenter::resolve_key_sequence_for_keymap`.

# Notes

The preflight intentionally does not call `dispatch_in_app` or `dispatch_in_window`. It reports the
candidate command id, pending chord state, availability state, and captured query so applications
retain final dispatch policy.
