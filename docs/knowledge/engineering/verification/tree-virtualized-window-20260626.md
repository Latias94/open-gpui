---
type: Verification Evidence
title: Tree virtualized window verification
status: complete
timestamp: 2026-06-26
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
---

# Verification

- `cargo nextest run -p open-gpui-ui-components tree_render_plan_virtualizes_visible_rows_with_stable_metadata feedback_tree_and_virtualized_list_public_exports_remain_explicit component_api_inventory_uses_stable_ownership_vocabulary` passed 3/3.
- `cargo nextest run -p open-gpui-ui-foundation-gallery components_page_samples_expose_component_metadata components_gallery_smoke_virtualized_tree_scrolls_inside_sample` passed 2/2.
- Tree render-plan coverage proves fixed-row window resolution, overscan range math, stable row keys, and exported plan types.
- Gallery metadata coverage proves the new `release-outline` Tree sample is registered and resolved as a virtualized sample.
- The current gallery smoke proves the virtualized Tree sample remains inside the Components page shell and keeps the initial Tree window mounted.
