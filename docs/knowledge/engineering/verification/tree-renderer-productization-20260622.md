---
type: Verification Evidence
title: Tree renderer productization verification
status: complete
timestamp: 2026-06-22
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
---

# Verification

- `cargo nextest run -p open-gpui-ui-components` passed 180/180 after adding the official `Tree` renderer.
- `cargo nextest run -p open-gpui-ui-foundation-gallery` passed 66/66 after adding the Tree gallery sample, catalog gate, and smoke coverage.
- Focused Tree smokes passed before the full run: `components_gallery_smoke_tree_expands_and_selects`, `components_gallery_smoke_tree_card_wheel_does_not_leak_to_page`, and `gallery_smoke_compact_shell_scrolls_navigation_and_resets_page_on_navigation`.
- The gallery compact-shell smoke now uses the Components directory Tree jump instead of slow wheel-only scrolling to a deeper sample.
