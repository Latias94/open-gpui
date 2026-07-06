---
type: Verification Evidence
title: Open GPUI command shortcut inspector and keybinding editor verification
tags:
  - open-gpui
  - command
  - keymap
  - gallery
timestamp: 2026-07-04T00:00:00Z
git_branch: main
---

# Commands

```powershell
cargo check -p open-gpui-command -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --tests
cargo nextest run -p open-gpui-command center_projects_command_key_bindings_into_gpui_keymap center_reports_command_key_binding_conflicts_and_install_report --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_keybinding_editor_state_filters_conflicts_and_keeps_diagnostics command_palette_controller_preflights_keymap_dispatch_with_query --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface crate_root_and_prelude_exports_remain_explicit --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts components_page_samples_expose_component_metadata components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
cargo fmt -p open-gpui-command -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery -- --check
git diff --check
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test choice --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery --no-fail-fast
```

# Result

All listed commands passed on `main`.

# Coverage

The focused tests prove:

- projected command binding entries preserve source id, command id, normalized shortcut, and
  normalized context;
- shortcut inspector state preserves query, input label, matched command, pending command, and
  primary dispatchable id from command palette preflight;
- keybinding editor state filters conflicts, keeps diagnostics, and exports through crate root and
  prelude;
- gallery data contracts expose the inspector/editor states;
- the gallery shell renders stable debug selectors for inspector matched rows, editor rows,
  conflicts, and diagnostics.

