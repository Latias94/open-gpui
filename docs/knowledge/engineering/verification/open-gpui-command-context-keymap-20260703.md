---
type: Verification Evidence
title: Open GPUI command context keymap verification
status: verified
timestamp: 2026-07-03T21:51:12+08:00
git_branch: feat/command-context-keymap
---

# Verified

```powershell
cargo fmt -p open-gpui-command -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts components_page_samples_expose_component_metadata components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
python C:\Users\Frankorz\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering
git diff --check
```

The command crate tests covered:

- repeated command scopes moving to the focused end of `CommandContextStack`;
- GPUI key contexts preserving broad-to-focused depth order;
- `GpuiCommandActionMap` projecting shortcuts through a key context stack;
- `CommandCenter` using one context stack for scope projection, keymap shortcuts, diagnostics,
  and provider request active scopes.

The public-surface run covered:

- root and prelude default exports for `CommandContextStack`;
- `CommandCenter` accepting the stack and exposing active scopes plus GPUI key contexts.

The gallery runs covered:

- the appended `context-stack` command sample;
- focused `editor` scope overriding the broader `workspace.open` descriptor;
- `OpenRegistryCommand` displaying the `Editor` shortcut `ctrl-E`;
- `editor.format` displaying `ctrl-shift-F`;
- empty shortcut diagnostics for the healthy sample.

# Citations

- [Command context tests](../../../../crates/open-gpui-command/src/context.rs)
- [Command center tests](../../../../crates/open-gpui-command/src/center.rs)
- [Action map tests](../../../../crates/open-gpui-command/src/gpui.rs)
- [Public-surface exports](../../../../crates/ui_components/tests/public_surface/exports.rs)
- [Gallery sample contracts](../../../../examples/ui-foundation-gallery/tests/foundation_gallery/component_sample_contracts.rs)
