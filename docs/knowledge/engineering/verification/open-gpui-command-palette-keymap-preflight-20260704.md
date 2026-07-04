---
type: Verification Evidence
title: Open GPUI command palette keymap preflight verification
status: passed
timestamp: 2026-07-04T00:00:00+08:00
git_branch: main
---

# Commands

```powershell
cargo nextest run -p open-gpui-ui-components command_palette_controller_preflights_keymap_dispatch_with_query --no-fail-fast
cargo nextest run -p open-gpui-ui-components choice --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface crate_root_and_prelude_exports_remain_explicit --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_search_samples_expose_combobox_and_command_contracts components_page_samples_expose_component_metadata components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
cargo check -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --tests
cargo fmt -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery --check
git diff --check
```

# Result

Focused controller, public-surface, and gallery command dogfood tests passed. The affected
components and gallery test targets compile. Formatting checks passed, and `git diff --check`
reported only the repo's normal LF-to-CRLF working-copy warnings.
