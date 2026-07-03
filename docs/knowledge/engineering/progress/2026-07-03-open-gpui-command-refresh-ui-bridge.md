---
type: Work Progress
title: Open GPUI command refresh UI bridge
status: verified
timestamp: 2026-07-03T18:31:00+08:00
git_branch: feat/command-refresh-ui-bridge
tags:
  - command
  - provider
  - ui-components
---

# Summary

`open_gpui_ui_components` now owns the provider-refresh-to-command-palette bridge while
`open_gpui_command` stays renderer-neutral.

# Shipped Capability

- Added `CommandProviderPaletteProjection` in the UI command descriptor layer.
- The projection adapts `CommandProviderRefreshProjection` into:
  - a `PreFiltered` `CommandIndexSnapshot`;
  - projected `CommandLoadingState` when the provider status is loading;
  - the latest `CommandProviderStatus` for readouts.
- Added `Command::provider_refresh_projection` so apps can bind query and snapshot metadata without
  hand-writing `CommandIndexSnapshot::from_registry_snapshot(...)` glue.
- Updated the component public API inventory and default root/prelude exports.
- Updated the foundation gallery `provider-search` sample to consume the UI adapter.

# Design Notes

Provider refresh snapshots are projected as `PreFiltered` because `CommandCenter::search_snapshot`
has already searched for the provider query. The command component preserves that provider result
set instead of applying a second local filter. Ready and failed provider status stays available as
metadata; only loading status becomes `CommandLoadingState`.

# Verified

```powershell
cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
cargo nextest run -p open-gpui-ui-components command --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
python C:\Users\Frankorz\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
git diff --check
```

# Next Action

Commit, merge to `main`, push, and delete `feat/command-refresh-ui-bridge` after final memory
validation.

# Citations

- [Command descriptor bridge](../../../../crates/ui_components/src/command/descriptor.rs)
- [Command builder facade](../../../../crates/ui_components/src/command/mod.rs)
- [Gallery command sample](../../../../examples/ui-foundation-gallery/src/pages/components/samples/choice.rs)
- [Command ecosystem docs](../../../ui/command-ecosystem.md)
