---
type: Verification Evidence
title: Open GPUI command ecosystem U1/U2 verification
status: verified
timestamp: 2026-07-03T09:50:17+08:00
git_branch: feat/open-gpui-command-ecosystem
related_plan: docs/plans/2026-07-03-001-feat-open-gpui-command-ecosystem-plan.md
---

# Verified

- `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components`
- `cargo check -p open-gpui-ui-core --tests`
- `cargo check -p open-gpui-ui-components --tests`
- `cargo nextest run -p open-gpui-ui-core command --no-fail-fast`
- `cargo nextest run -p open-gpui-ui-components command --no-fail-fast`
- `cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast`

# Evidence Scope

- `open-gpui-ui-core` command tests cover descriptor metadata, deterministic registry order,
  duplicate-id rejection, atomic `register_all` failure behavior, and direct snapshot creation.
- `open-gpui-ui-components` command tests cover `CommandIndexSnapshot::from_registry_snapshot`,
  group preservation, revision preservation, disabled metadata, shortcuts, and `when` projection.
- Public-surface tests prove the crate root and prelude expose the registry types explicitly through
  the curated default API surface without wildcard leakage.

# Notes

- The registry is intentionally a projection layer over existing GPUI action/keymap infrastructure,
  not a replacement for it.
- The next verification expansion should cover keymap shortcut projection once U3 adds the GPUI
  adapter.

# Citations

- [Plan](../../../plans/2026-07-03-001-feat-open-gpui-command-ecosystem-plan.md)
- [Progress note](../progress/2026-07-03-open-gpui-command-ecosystem-u1-u2.md)
