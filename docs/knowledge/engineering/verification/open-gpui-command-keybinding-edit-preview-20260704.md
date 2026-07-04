---
type: Verification Evidence
title: Open GPUI command keybinding edit preview verification
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
cargo nextest run -p open-gpui-command center_previews_key_binding_patch_without_mutating_registry center_reports_missing_key_binding_patch_target --no-fail-fast
cargo nextest run -p open-gpui-ui-components command_keybinding_capture_and_preview_state_model_edit_patch command_keybinding_editor_state_filters_conflicts_and_keeps_diagnostics --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface crate_root_and_prelude_exports_remain_explicit --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts components_page_samples_expose_component_metadata components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
cargo nextest run -p open-gpui-command --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test choice --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery --no-fail-fast
```

# Result

All listed commands passed on `main`.

# Coverage

The tests prove:

- patch preview can create a candidate conflict without mutating the source registry;
- missing patch targets are reported as `TargetMissing`;
- captured key sequences normalize into GPUI display syntax and invalid sequences carry errors;
- editor preview state exposes patch operation, outcome, candidate rows, conflicts, and diagnostics;
- root/prelude exports include all new patch/capture/preview types;
- the gallery renders stable selectors for keybinding edit preview, capture, patch, and preview rows.

