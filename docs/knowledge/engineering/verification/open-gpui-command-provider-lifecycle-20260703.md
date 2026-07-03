---
type: Verification Evidence
title: Open GPUI command provider lifecycle verification
status: verified
timestamp: 2026-07-03T17:45:00+08:00
git_branch: feat/command-provider-lifecycle
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

- provider request ids on `CommandProviderRequest`;
- response binding through `CommandProviderResponse::for_request`;
- synchronous `refresh_provider` request id/query propagation into `CommandProviderStatus`;
- unbound external responses still applying for compatibility;
- lifecycle-bound late responses returning `CommandProviderApplyOutcome::Stale`;
- stale responses preserving existing provider-owned sources until the current response applies.
- provider request ids not being reused across unregister/re-register cycles.

The public-surface run covered root and prelude exports for the new lifecycle types:
`CommandProviderApplyOutcome`, `CommandProviderRequestId`, and `CommandProviderStaleResponse`.

The gallery contract runs covered the `provider-search` sample retaining request id `1`, query
`alpha`, ready status, one source, and two rendered provider commands.

# Citations

- [Provider model](../../../../crates/open-gpui-command/src/provider.rs)
- [Command center lifecycle](../../../../crates/open-gpui-command/src/center.rs)
