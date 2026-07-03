---
type: Verification Evidence
title: Open GPUI command provider refresh controller verification
status: verified
timestamp: 2026-07-03T18:08:00+08:00
git_branch: feat/command-provider-refresh-controller
---

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

The command crate run covered:

- registered provider refresh for a changed query;
- optional loading response before ready results;
- unchanged queries not starting a new request;
- stale async responses preserving the current query snapshot;
- current async responses projecting the latest provider result.

The public-surface run covered root and prelude exports for
`CommandProviderRefreshController` and `CommandProviderRefreshProjection`.

The gallery contract runs covered the `provider-search` sample still rendering two provider
commands with request id/query provider status after migrating to the controller.

# Citations

- [Refresh controller](../../../../crates/open-gpui-command/src/refresh.rs)
- [Gallery command sample](../../../../examples/ui-foundation-gallery/src/pages/components/samples/choice.rs)
