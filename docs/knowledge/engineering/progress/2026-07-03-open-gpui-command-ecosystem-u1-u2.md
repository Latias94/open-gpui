---
type: Work Progress
title: Open GPUI command ecosystem U1/U2
status: verified
timestamp: 2026-07-03T09:50:17+08:00
git_branch: feat/open-gpui-command-ecosystem
related_plan: docs/plans/2026-07-03-001-feat-open-gpui-command-ecosystem-plan.md
verified_by:
  - cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components
  - cargo check -p open-gpui-ui-core --tests
  - cargo check -p open-gpui-ui-components --tests
  - cargo nextest run -p open-gpui-ui-core command --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components command --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
tags:
  - command
  - ui-core
  - ui-components
---

# Summary

Implemented the first `open-gpui-command` ecosystem slice from
`docs/plans/2026-07-03-001-feat-open-gpui-command-ecosystem-plan.md` on
`feat/open-gpui-command-ecosystem`.

# Shipped Capability

- `open_gpui_ui_core::CommandRegistry` owns deterministic command contribution registration.
- `CommandRegistry` rejects duplicate stable command ids and preserves contribution insertion order.
- Batch registration is atomic: duplicate ids reject the full batch without leaving partial
  contributions in the registry.
- `CommandContribution` keeps optional caller-owned source metadata for app, crate, or plugin-like
  modules.
- `CommandRegistrySnapshot` is an immutable projection that exposes contributions and descriptor
  iteration without depending on `open-gpui-ui-components`.
- `CommandRegistryError` gives duplicate-id diagnostics without introducing a new dependency.
- `open_gpui_ui_components::CommandIndexSnapshot::from_registry_snapshot` projects a renderer-neutral
  registry snapshot into the existing command palette index path.
- The default component public API exports `CommandContribution`, `CommandRegistry`,
  `CommandRegistryError`, and `CommandRegistrySnapshot` alongside `CommandDescriptor`.

# Boundaries

- This slice does not replace GPUI `Action`, `Keymap`, keystroke parsing, context predicates, or
  window dispatch.
- No global singleton registry was introduced; apps own registry lifetime and revision labels.
- `disabled`, `when`, `shortcut`, keywords, group, and menu path remain caller-owned metadata in
  this slice.
- Context expression evaluation, keymap shortcut projection, command history, Vim/editor modes, and
  dispatch adapters remain deferred to later units.

# Next Action

Continue with U3 by projecting display shortcuts from the existing GPUI keymap/action machinery into
registered command descriptors. U4 should then map `CommandSelection` back to app-owned GPUI action
dispatch without adding a second dispatch engine.

# Citations

- [Plan](../../../plans/2026-07-03-001-feat-open-gpui-command-ecosystem-plan.md)
- [Verification evidence](../verification/open-gpui-command-ecosystem-u1-u2-20260703.md)
