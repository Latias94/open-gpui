---
type: Verification
title: Open GPUI command keymap resolution verification
status: passed
timestamp: 2026-07-04T10:26:22+08:00
git_branch: feat/command-keymap-scopes
---

# Commands

```powershell
cargo nextest run -p open-gpui-command keymap_resolution --no-fail-fast
cargo nextest run -p open-gpui-command center_resolves_keymap_input_for_active_context_stack --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata components_page_search_samples_expose_combobox_and_command_contracts components_gallery_smoke_focused_command_samples_cover_depth_behaviors --no-fail-fast
cargo check -p open-gpui-ui-foundation-gallery --tests
cargo fmt -p open-gpui-ui-foundation-gallery --check
cargo nextest run -p open-gpui-ui-foundation-gallery --no-fail-fast
cargo nextest run -p open-gpui-command --no-fail-fast
cargo fmt -p open-gpui-command -p open-gpui-ui-components --check
cargo check -p open-gpui-command -p open-gpui-ui-components --tests
git diff --check
```

# Result

All focused command keymap resolution tests, the full `open-gpui-command` crate, UI public-surface
tests, focused gallery keymap dogfood tests, full gallery nextest, formatting checks, and
test-target compilation passed. `git diff --check` reported only the repo's normal LF-to-CRLF
working-copy warnings.
