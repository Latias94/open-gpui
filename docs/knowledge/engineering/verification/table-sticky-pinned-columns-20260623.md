---
type: Verification Evidence
title: Table sticky pinned columns verification
status: complete
timestamp: 2026-06-23
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
---

# Verification

- `cargo nextest run -p open-gpui-ui-components table_runtime_pinned_body_scrolls_without_moving_parent` passed 1/1 after adding the pinned Table body wheel-containment runtime test.
- `cargo nextest run -p open-gpui-ui-components table` passed 28/28, including the sticky pinned body scroll test, existing pinned center-lane scroll test, header sort parity, and resize handle gates.
- `cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_grouped_table_scroll_stays_inside_sample` passed 1/1 after the grouped `release-rollup` sample used a real body-cell wheel target.
- `cargo nextest run -p open-gpui-ui-foundation-gallery table` passed 7/7, including `components_gallery_smoke_grouped_table_pinned_center_scroll_stays_inside_sample`, which proves horizontal center-lane scroll keeps left/right pinned cells and the outer Components page fixed.
