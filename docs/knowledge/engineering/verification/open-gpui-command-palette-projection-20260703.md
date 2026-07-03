---
type: Verification Evidence
title: Open GPUI command palette projection verification
status: verified
timestamp: 2026-07-03T19:32:19+08:00
git_branch: feat/command-palette-session
---

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

The focused command projection test covered:

- `CommandPaletteProjection::from_center_for_keymap`;
- provider status retention from `CommandCenter`;
- empty shortcut diagnostics for action-bound static and provider commands;
- `Command::palette_projection` producing a `PreFiltered` command state with projected shortcuts.

The public-surface run covered:

- root and prelude exports for `CommandPaletteProjection`;
- `Command::palette_projection` in the component API inventory and method baseline.

The gallery run covered:

- `registry-dispatch` still proving shortcut/dispatch projection and empty diagnostics;
- `provider-search` now using `CommandPaletteProjection`;
- provider dynamic commands carrying projected shortcuts and empty shortcut diagnostics.

# Citations

- [Command projection test](../../../../crates/ui_components/tests/choice.rs)
- [Public-surface exports](../../../../crates/ui_components/tests/public_surface/exports.rs)
- [Gallery sample contracts](../../../../examples/ui-foundation-gallery/tests/foundation_gallery/component_sample_contracts.rs)
