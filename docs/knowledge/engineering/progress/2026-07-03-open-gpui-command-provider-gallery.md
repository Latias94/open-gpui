---
type: Work Progress
title: Open GPUI command provider gallery proof
status: verified
timestamp: 2026-07-03T17:21:00+08:00
git_branch: feat/command-provider-gallery
tags:
  - command
  - provider
  - gallery
---

# Summary

The foundation gallery now has a visible provider-backed command sample on top of the
runtime-neutral `open_gpui_command` provider layer.

# Shipped Capability

- `pages::components::command_samples` now returns six command samples. The existing
  `registry-dispatch` sample remains at index 4, and the new `provider-search` sample is appended
  at index 5.
- `provider-search` creates an app-owned `CommandCenter`, registers a query-dependent provider,
  refreshes it for `alpha`, captures the provider status, projects the center search snapshot into
  `CommandIndexSnapshot`, and renders it through the existing `Command` component.
- `CommandSample` exposes optional provider status so gallery contracts can verify provider id,
  ready/loading state, source count, and command count without moving provider ownership into
  `CommandState`.
- The shell card readout shows provider status for provider-backed samples while leaving local and
  registry-backed samples unchanged.

# Design Notes

This is intentionally a gallery proof, not a new UI runtime. `open_gpui_command` remains UI-neutral,
and the component crate still receives only command descriptors or index snapshots. Applications can
use the same pattern with synchronous providers or call `apply_provider_response` after app-owned
async work completes.

# Verified

```powershell
cargo fmt -p open-gpui-ui-foundation-gallery
cargo fmt -p open-gpui-ui-foundation-gallery --check
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
python C:\Users\Frankorz\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
git diff --check
```

# Next Action

Run final formatting/wiki/diff gates, commit this gallery layer, then merge back to `main` if the
working tree remains scoped.

# Citations

- [Gallery command samples](../../../../examples/ui-foundation-gallery/src/pages/components/samples/choice.rs)
- [Gallery command shell readout](../../../../examples/ui-foundation-gallery/src/shell/components.rs)
- [Command ecosystem docs](../../../ui/command-ecosystem.md)
