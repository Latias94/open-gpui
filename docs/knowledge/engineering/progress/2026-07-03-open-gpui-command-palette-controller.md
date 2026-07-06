---
type: Work Progress
title: Open GPUI command palette controller
status: verified
timestamp: 2026-07-03T20:50:16+08:00
git_branch: feat/command-palette-controller
tags:
  - command
  - ui-components
  - provider
---

# Summary

`open_gpui_ui_components::CommandPaletteController` is now the UI-side query/provider lifecycle
controller for `CommandCenter`-backed command palettes.

# Shipped Capability

- Added `CommandPaletteController` in the `Command` descriptor layer.
- The controller keeps palette query state and a list of `CommandProviderRefreshController`
  instances, but leaves `CommandCenter`, dispatch, and async scheduling app-owned.
- `set_query_for_keymap` and `set_query_for_window` refresh configured providers and return a
  `CommandPaletteControllerUpdate` with provider projections, missing provider ids, and the
  complete `CommandPaletteProjection`.
- Registered synchronous providers are refreshed immediately for changed queries.
- Providers without a registered callback keep their loading projection and are reported through
  `missing_provider_ids()`, so applications can run their own async task.
- `apply_provider_response_for_keymap` and `apply_provider_response_for_window` accept external
  provider responses and preserve the existing stale-response request guard.
- Root and prelude exports now include `CommandPaletteController` and
  `CommandPaletteControllerUpdate`.
- The gallery `provider-search` command sample now uses the controller instead of directly holding
  a provider refresh controller.

# Design Notes

This is intentionally a UI integration controller, not a new global command bus. It coordinates
projection and provider refresh state for a palette surface, while command registration, action
binding, dispatch, provider work scheduling, and long-lived ownership remain above it.

Provider-only code can still use `CommandProviderRefreshController` directly. Full palette surfaces
should prefer `CommandPaletteController` when they need query, provider status, loading state,
shortcut projection, and diagnostics in one update object.

# Verified

```powershell
cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
cargo test -p open-gpui-ui-components --test public_surface -- --nocapture
cargo nextest run -p open-gpui-ui-components command_palette_controller --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_palette --no-fail-fast
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components choice --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts components_page_samples_expose_component_metadata components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
python C:\Users\Frankorz\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
git diff --check
```

# Next Action

Run the final docs/memory validation, commit this slice, merge it to `main`, push, and delete
`feat/command-palette-controller`.

# Citations

- [Command descriptor controller](../../../../crates/ui_components/src/command/descriptor.rs)
- [Command builder facade](../../../../crates/ui_components/src/command/mod.rs)
- [Public API defaults](../../../../crates/ui_components/src/public_api/default.rs)
- [Command tests](../../../../crates/ui_components/tests/choice.rs)
- [Gallery command sample](../../../../examples/ui-foundation-gallery/src/pages/components/samples/choice.rs)
- [Command ecosystem docs](../../../ui/command-ecosystem.md)
- [Verification evidence](../verification/open-gpui-command-palette-controller-20260703.md)
