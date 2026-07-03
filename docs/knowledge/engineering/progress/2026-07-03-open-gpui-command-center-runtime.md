---
type: Work Progress
title: Open GPUI CommandCenter runtime facade
status: verified
timestamp: 2026-07-03T16:18:49+08:00
git_branch: feat/command-center-runtime
related_plan: docs/plans/2026-07-03-003-feat-command-center-runtime-plan.md
tags:
  - command
  - architecture
  - ui-components
---

# Summary

`open_gpui_command::CommandCenter` is now the recommended app/plugin-owned command runtime facade.
It sits above the existing registry, scope, availability, menu, history, and GPUI action adapter
primitives without moving UI rendering ownership into the command crate.

# Shipped Capability

- `CommandCenter` owns the standard command pipeline:
  active scopes, availability, shortcut projection, history ranking, fuzzy search, menu projection,
  and guarded dispatch.
- Apps/plugins can register one source inside one scope and keep a `CommandSourceRegistration`
  token for explicit unregistration.
- Dispatch through `CommandCenter` returns structured outcomes and records usage/query history only
  after successful GPUI action dispatch.
- `CommandItemDescriptor`, `CommandItemState`, command render plans, and behavior snapshots now
  carry optional disabled reasons from `CommandDescriptor`.
- Command keyboard runtime handles control navigation aliases and PageUp/PageDown disabled landings,
  while respecting GPUI `prefer_character_input` events.
- The gallery `registry-dispatch` sample now builds its command snapshot from `CommandCenter`
  instead of manually joining `CommandRegistry` and `GpuiCommandActionMap`.

# Verified

```powershell
cargo fmt -p open-gpui-command -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components choice --no-fail-fast
cargo nextest run -p open-gpui-ui-components command::runtime::tests --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_choice_samples_expose_listbox_and_select_contracts --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata --no-fail-fast
python C:\Users\Frankorz\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
git diff --check
```

# Current Direction

Keep `open_gpui_command` UI-neutral. New command palette rendering behavior belongs in
`open_gpui_ui_components::command`; app policy, Vim modes, keymap contexts, and chords remain with
GPUI/application code.

# Citations

- [Plan](../../../plans/2026-07-03-003-feat-command-center-runtime-plan.md)
- [Command crate](../../../../crates/open-gpui-command/src/lib.rs)
- [Command ecosystem docs](../../../ui/command-ecosystem.md)
