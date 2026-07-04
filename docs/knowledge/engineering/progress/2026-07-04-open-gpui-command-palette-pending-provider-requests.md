---
type: Work Progress
title: Open GPUI command palette pending provider requests
status: active
timestamp: 2026-07-04T09:56:04+08:00
git_branch: feat/command-navigation-polish
tags:
  - open-gpui
  - command
  - ui-components
  - async-provider
---

# Summary

`CommandPaletteControllerUpdate` now exposes app-owned async provider work as concrete pending
provider requests instead of only listing missing provider ids.

The new `CommandPalettePendingProviderRequest` carries:

- the `CommandProviderId` that should produce the async response;
- the exact `CommandProviderRequest` that must be passed back to
  `apply_provider_response_for_keymap` or `apply_provider_response_for_window`.

# Implementation Notes

- `pending_provider_requests()` is the preferred controller update API for app shells that schedule
  async provider work.
- `pending_provider_request(provider_id)` is a convenience lookup for single-provider palettes.
- `missing_provider_ids()` remains available as a compatibility summary derived from pending
  requests.
- Root/prelude exports and public-surface tests cover the new type and accessors.
- Command ecosystem docs now describe the pending request handoff as the app-owned async boundary.

# Next Action

The next command ecosystem slice can dogfood these pending provider requests in a real app-shell or
gallery runtime path: query change emits pending requests, app code schedules async work, and a later
response applies through the stale-response guard.

# Citations

- [Command ecosystem docs](../../../ui/command-ecosystem.md)
- [Verification docs](../../../verification.md)
- [Verification evidence](../verification/open-gpui-command-palette-pending-provider-requests-20260704.md)
