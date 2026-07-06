---
type: Work Progress
title: Open GPUI command provider refresh controller
status: verified
timestamp: 2026-07-03T18:08:00+08:00
git_branch: feat/command-provider-refresh-controller
tags:
  - command
  - provider
  - controller
---

# Summary

`open_gpui_command` now has a reusable provider refresh controller for command-palette query
pipelines.

# Shipped Capability

- Added `CommandProviderRefreshController` and `CommandProviderRefreshProjection`.
- Added `CommandCenter::provider_response_for_request` so a controller can reuse registered
  synchronous providers with an existing lifecycle request.
- The controller starts a new provider request only when the query changes.
- It can apply an optional loading response when a new query starts.
- It supports registered synchronous refresh through `refresh_provider`.
- It supports app-owned async completion through `apply_response`.
- It returns a projection containing provider id, query, current request, query-changed flag,
  application outcome, provider status, and the current `CommandRegistrySnapshot`.
- The foundation gallery `provider-search` sample now uses the controller to project its
  provider-backed `CommandIndexSnapshot`.

# Design Notes

The controller remains UI-neutral. It projects `CommandRegistrySnapshot`; concrete UI crates still
convert that snapshot into `CommandIndexSnapshot` when needed. This preserves the command crate's
boundary while removing the repeated app code for begin/apply/stale/search projection.

# Verified

```powershell
cargo fmt -p open-gpui-command -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
python C:\Users\Frankorz\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
git diff --check
```

# Next Action

Commit and merge to `main` if the branch remains scoped.

# Citations

- [Refresh controller](../../../../crates/open-gpui-command/src/refresh.rs)
- [Command center](../../../../crates/open-gpui-command/src/center.rs)
- [Gallery command sample](../../../../examples/ui-foundation-gallery/src/pages/components/samples/choice.rs)
