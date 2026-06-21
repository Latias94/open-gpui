---
type: "Work Progress"
title: "Gallery components directory fixed and scroll regressions stabilized"
description: "Components page directory is now separated from the page scroll area; gallery regressions keep directory state stable, verify scroll area contracts, and isolate wheel input on the release-queue sample card."
timestamp: 2026-06-21T12:18:41Z
tags: ["open-gpui", "ui-foundation-gallery", "components", "scroll", "tests"]
source_session: "019ec6c8-5566-7062-8458-21ebe1360573"
git_branch: "main"
git_commit: "a7f0b96"
verified_by: "cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_scroll_area_samples_scroll_inside_page components_gallery_smoke_release_queue_scroll_stays_inside_sample components_gallery_smoke_release_queue_card_wheel_does_not_leak_to_page components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation components_gallery_smoke_vertical_tabs_scroll_inside_sample components_gallery_smoke_directory_jump_scrolls_to_tabs_section"
---

# Summary

# Details

# Next Action

# Citations
