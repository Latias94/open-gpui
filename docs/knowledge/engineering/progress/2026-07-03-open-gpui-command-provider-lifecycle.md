---
type: Work Progress
title: Open GPUI command provider lifecycle
status: verified
timestamp: 2026-07-03T17:45:00+08:00
git_branch: feat/command-provider-lifecycle
tags:
  - command
  - provider
  - lifecycle
---

# Summary

`open_gpui_command` now has a lifecycle-safe provider response path. Apps can issue tracked
provider requests, bind async responses to those requests, and let `CommandCenter` ignore stale
responses that complete after a newer query.

# Shipped Capability

- Added `CommandProviderRequestId` and optional request ids on `CommandProviderRequest` and
  `CommandProviderResponse`.
- Added `CommandProviderResponse::for_request` and
  `CommandCenter::apply_provider_response_for_request` for app-owned async boundaries.
- Added `CommandProviderApplyOutcome::{Applied, Stale}` and `CommandProviderStaleResponse`.
- `CommandCenter::begin_provider_request` now issues per-provider monotonic request ids and records
  the latest request.
- `refresh_provider` and `refresh_providers` automatically issue tracked requests for synchronous
  provider callbacks.
- Bound responses whose request id is older than the latest provider request return `Stale` and do
  not replace dynamic sources.
- Provider request counters are not reset by unregister/re-register, so late responses cannot
  collide with a new provider generation that reuses the same id.
- `CommandProviderStatus` now records the producing request id and query.
- The gallery provider readout and contracts now expose request id/query metadata for
  `provider-search`.

# Design Notes

Unbound responses still apply for compatibility and for intentionally fire-and-forget hosts. Stale
protection is activated when a response carries a center-issued request id. The command crate still
does not own async scheduling, cancellation tokens, or a task runtime.

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

Commit, merge to `main`, and push if the branch remains scoped.

# Citations

- [Provider model](../../../../crates/open-gpui-command/src/provider.rs)
- [Command center lifecycle](../../../../crates/open-gpui-command/src/center.rs)
- [Command ecosystem docs](../../../ui/command-ecosystem.md)
