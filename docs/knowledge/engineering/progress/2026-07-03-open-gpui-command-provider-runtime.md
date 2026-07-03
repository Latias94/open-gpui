---
type: Work Progress
title: Open GPUI command provider runtime
status: verified
timestamp: 2026-07-03T17:35:00+08:00
git_branch: feat/command-provider-runtime
tags:
  - command
  - provider
  - architecture
---

# Summary

`open_gpui_command` now has a runtime-neutral dynamic provider layer for query-dependent and
async-produced command results.

# Shipped Capability

- `CommandProviderRequest` carries the current query and active scopes to provider callbacks.
- `CommandProviderResponse` carries ready/loading/failed state plus provider-owned dynamic
  sources.
- `CommandProviderSource` is projected into the same scoped registry path as static command
  sources, so availability, shortcuts, menu projection, fuzzy search, and dispatch see one command
  model.
- `CommandCenter` can register provider callbacks, refresh one or all providers by query, apply
  externally produced responses for app-owned async tasks, keep latest provider status, and
  unregister provider sources.
- Provider response application is atomic: invalid duplicate command ids return an error without
  replacing the previous provider source snapshot.

# Design Notes

The command crate still does not own an async runtime, persistence layer, or UI loading surface.
Applications decide how to schedule provider work and when to apply responses. This keeps the crate
usable for GPUI apps, plugin hosts, tests, and non-UI command catalogs.

# Verified

```powershell
cargo fmt -p open-gpui-command -p open-gpui-ui-components --check
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
python C:\Users\Frankorz\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
git diff --check
```

# Next Action

Commit the provider runtime slice, then merge it back to `main` if no broader verification is
needed.

# Citations

- [Provider model](../../../../crates/open-gpui-command/src/provider.rs)
- [Command center facade](../../../../crates/open-gpui-command/src/center.rs)
- [Command ecosystem docs](../../../ui/command-ecosystem.md)
