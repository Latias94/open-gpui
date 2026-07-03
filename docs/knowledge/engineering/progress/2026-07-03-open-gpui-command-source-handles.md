---
type: Work Progress
title: Open GPUI command source handles
status: active
tags:
  - open-gpui-command
  - command
  - lifecycle
  - plugin
git_branch: feat/command-source-handles
---

# Summary

`open_gpui_command` now treats source/provider registration tokens as explicit lifecycle handles.
`CommandSourceHandle` and `CommandProviderHandle` are the recommended public names; the older
`CommandSourceRegistration` and `CommandProviderRegistration` names remain compatibility aliases
for existing callers.

# Design Decision

Handles are explicit, not `Drop`-driven RAII guards. `CommandCenter` is app-owned rather than a
global singleton, so automatic drop cleanup would require hidden shared ownership or interior
mutability. Plugin hosts keep the handle and call `handle.unregister(&mut center)` when they have
mutable access to the center.

# Implementation Notes

- `CommandSourceHandle::unregister(self, &mut CommandCenter)` removes all contributions for the
  handle's source id across scopes, matching the existing center-wide source-id lifecycle model.
- `CommandProviderHandle::unregister(self, &mut CommandCenter)` removes the provider callback,
  provider status, latest request, and applied provider-owned sources.
- Borrowed center entry points remain available through `unregister_source_handle`,
  `unregister_provider_handle`, `unregister`, and `unregister_provider`.
- UI default exports include the new handle names while preserving the registration aliases.

# Next Action

After this slice lands, the next command ecosystem depth candidate is query history ergonomics or a
stronger fuzzy ranking implementation.
