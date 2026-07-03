---
type: Work Progress
title: Open GPUI command palette projection
status: verified
timestamp: 2026-07-03T19:32:19+08:00
git_branch: feat/command-palette-session
tags:
  - command
  - ui-components
  - provider
---

# Summary

`open_gpui_ui_components::CommandPaletteProjection` now provides the recommended UI-side app
integration projection for `CommandCenter` palettes.

# Shipped Capability

- Added `CommandPaletteProjection` in the `Command` descriptor layer.
- The projection adapts a `CommandCenter` query plus app-level `Keymap` or focused `Window`
  shortcut precedence into:
  - a `PreFiltered` `CommandIndexSnapshot`;
  - retained provider statuses;
  - shortcut/action/keymap diagnostics.
- Added `Command::palette_projection(&projection)` so apps can feed the projection directly into a
  `Command` surface.
- Updated the public API inventory and root/prelude exports.
- Migrated gallery `registry-dispatch` and `provider-search` command samples to the projection.
- The provider gallery sample now binds dynamic provider command ids to GPUI actions and shortcuts,
  proving a provider-backed palette can remain dispatch-ready and diagnostic-clean.

# Design Notes

`CommandPaletteProjection` does not own `CommandCenter` and does not dispatch by itself. Apps still
own mutable command runtime state and should dispatch selections through
`CommandCenter::dispatch_in_window` or `dispatch_in_app`.

Provider commands that should dispatch through `CommandCenter` must still have command-id-to-action
bindings. Provider results that represent caller-specific data can instead be handled directly in
`Command::on_select`.

# Verified

```powershell
cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
cargo nextest run -p open-gpui-ui-components choice --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_palette_projection_adapts_center_query_shortcuts_providers_and_diagnostics command_provider_palette_projection_maps_refresh_projection_to_prefiltered_index command_provider_palette_projection_carries_loading_status_into_index_snapshot --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts components_page_samples_expose_component_metadata components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
python C:\Users\Frankorz\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
git diff --check
```

# Next Action

Commit this slice, merge it to `main`, push, and delete `feat/command-palette-session`.

# Citations

- [Command descriptor projection](../../../../crates/ui_components/src/command/descriptor.rs)
- [Command builder facade](../../../../crates/ui_components/src/command/mod.rs)
- [Gallery command samples](../../../../examples/ui-foundation-gallery/src/pages/components/samples/choice.rs)
- [Command ecosystem docs](../../../ui/command-ecosystem.md)
- [Verification evidence](../verification/open-gpui-command-palette-projection-20260703.md)
