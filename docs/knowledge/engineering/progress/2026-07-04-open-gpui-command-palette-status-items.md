---
type: Work Progress
title: Open GPUI command palette status items
status: active
timestamp: 2026-07-04T01:34:04+08:00
git_branch: feat/command-palette-polish
related_progress:
  - progress/2026-07-03-open-gpui-command-palette-projection.md
  - progress/2026-07-03-open-gpui-command-refresh-ui-bridge.md
  - progress/2026-07-03-open-gpui-command-shortcut-diagnostics.md
---

# Summary

`open_gpui_ui_components::Command` now renders palette-level status rows derived from provider
failures and command/action/shortcut drift diagnostics.

# Changes

- Added `CommandStatusIntent` and `CommandStatusItem` as the UI-side diagnostic row contract for
  command palettes.
- `CommandPaletteProjection` now adapts failed provider statuses and shortcut diagnostics into
  status rows before `Command::palette_projection` resolves component state.
- `CommandProviderPaletteProjection` now exposes failed-provider status rows for provider-refresh
  adapters.
- `CommandState` and `Command` now carry explicit status items plus warning/error counters, and
  callers can append custom rows through `status_item` / `status_items`.
- The GPUI command runtime renders status rows between loading state and results, with stable debug
  selectors for focused gallery smoke coverage.
- The foundation gallery now includes a `diagnostics-empty` command sample proving a failed
  provider, missing shortcut/action diagnostics, status counters, and the empty list state together.
- Component contract inventory, public-surface exports, and command docs now describe the status
  row adapter boundary.

# Design Notes

- Status rows remain UI presentation data. `open_gpui_command` still owns provider lifecycle,
  registry/action/keymap diagnostics, dispatch, and history.
- Failed providers are errors because the provider could not refresh. Shortcut diagnostics are
  warnings because valid command rows can still render and dispatch through other paths.
- `Command::palette_projection` and `Command::provider_refresh_projection` replace the component
  state's projected status rows. Apps that need custom rows can append them after applying the
  projection.
- Empty-state rendering now composes with status rows instead of forcing diagnostics into gallery
  prose outside the component.

# Next Action

Run final formatting, focused component/gallery verification, engineering memory validation, and
commit the branch if clean.
