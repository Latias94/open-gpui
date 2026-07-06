---
type: Verification Evidence
title: Open GPUI command provider gallery verification
status: verified
timestamp: 2026-07-03T17:21:00+08:00
git_branch: feat/command-provider-gallery
---

# Verified

Focused provider-gallery checks:

```powershell
cargo fmt -p open-gpui-ui-foundation-gallery
cargo fmt -p open-gpui-ui-foundation-gallery --check
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
python C:\Users\Frankorz\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
git diff --check
```

The sample contract run covered:

- `provider-search` selected and active values resolving to `provider.open.alpha`;
- provider status id `recent-provider`, `Ready` state, one dynamic source, and two commands;
- `gallery-provider-center-v1` snapshot revision and `PreRankedFilter` mode;
- the rendered command state carrying the `Provider` group and two query-specific provider rows.

The catalog contract run covered:

- command sample count increasing from five to six;
- `registry-dispatch` remaining at index 4;
- `provider-search` appended at index 5 with provider status and snapshot metadata intact.

The focused command smoke still renders the command family in gallery focused mode after the sample
count changed and the provider status readout was added.

# Residual Risk

This does not add cancellation, streaming provider updates, async scheduling, or provider result
deduping policy. Those remain app/runtime concerns above the neutral provider response boundary.

# Citations

- [Gallery command samples](../../../../examples/ui-foundation-gallery/src/pages/components/samples/choice.rs)
- [Command ecosystem docs](../../../ui/command-ecosystem.md)
