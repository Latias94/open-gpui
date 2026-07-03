---
type: Verification Evidence
title: Open GPUI command source handles verification
status: active
tags:
  - open-gpui-command
  - command
  - lifecycle
git_branch: feat/command-source-handles
---

# Scope

Focused verification for explicit source/provider lifecycle handles in `open_gpui_command`.

# Evidence

- Added the failing proof first:
  `cargo nextest run -p open-gpui-command source_and_provider_handles_unregister_their_runtime_state --no-fail-fast`
  failed because `CommandSourceRegistration` and `CommandProviderRegistration` did not expose
  handle-level `unregister` methods.
- After implementation, the same focused proof passed:
  `cargo nextest run -p open-gpui-command source_and_provider_handles_unregister_their_runtime_state --no-fail-fast`.

# Final Gates

Passed before shipping:

- `cargo fmt -p open-gpui-command -p open-gpui-ui-components --check`
- `cargo nextest run -p open-gpui-command --no-fail-fast`
- `cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast`
- `python C:\Users\Frankorz\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering`
- `git diff --check` with only LF/CRLF warnings.
