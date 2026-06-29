---
type: Work Progress
title: Menu hover-open submenu proof
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: feat/scroll-surface-containment
verified_by:
  - cargo check -p open-gpui-ui-components
  - cargo nextest run -p open-gpui-ui-components menu_runtime_hover_opens_submenu_and_preserves_child_focus
  - cargo nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke_opens_menu_submenu_from_hover
---

# Summary

Implemented submenu hover-open for `Menu` items in `open-gpui-ui-components` and added a
gallery smoke on the Overlay page's `rich-items` sample. The proof currently covers hover-open
and branch retention when moving into the open submenu child, but not the more complete safe-hover
corridor follow-up.

# Verified State

- `MenuItem` hover now drives the submenu runtime open path for submenu triggers.
- The components test `menu_runtime_hover_opens_submenu_and_preserves_child_focus` passes.
- The gallery smoke `overlay_gallery_smoke_opens_menu_submenu_from_hover` passes.

# Open Threads

- Safe-hover corridor geometry and delayed close behavior still remain as follow-up work if we
  decide to harden pointer exit races further.
- Menu bars, application menu integration, global command dispatch, and native OS menu bridging
  remain deferred.

# Next Action

Stage and commit the hover-open submenu slice, then pick the next component-depth boundary.

# Citations

[1] `crates/ui_components/src/menu.rs`
[2] `crates/ui_components/tests/components.rs`
[3] `examples/ui-foundation-gallery/tests/foundation_gallery.rs`
[4] `docs/ui/component-contract.md`
[5] `docs/verification.md`
