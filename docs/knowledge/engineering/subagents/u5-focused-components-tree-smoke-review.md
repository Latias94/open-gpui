---
type: "Subagent Finding"
title: "U5 focused Components Tree smoke review"
description: "Subagent review of the focused Components gallery Tree smoke and root click-to-focus behavior."
timestamp: 2026-06-22T00:00:00Z
tags: ["open-gpui", "gallery", "tree", "focused-mode", "subagent"]
---

# Finding
- The original Tree gallery failure was a test-path issue, not a broken Tree renderer: the gallery smoke had been trying to prove keyboard focus by clicking a point that could land on a visible row instead of blank Tree chrome.
- The Tree root click-to-focus behavior is safe to keep. It only applies when the click lands on the Tree container chrome/blank area; row clicks still win when they hit a concrete row because row handlers stop propagation.
- The focused-mode gallery path is the right place to verify Tree keyboard expand/select behavior. The smoke should enter focused Tree mode first, then click the concrete `paper` row.

# Evidence
- `cargo nextest run -p open-gpui-ui-components tree_runtime_expands_reveals_and_selects_items`
- `cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_tree_expands_and_selects`
- `cargo nextest run -p open-gpui-ui-components`
- `cargo nextest run -p open-gpui-ui-foundation-gallery`

# Recommendation
- Keep the Tree root click-to-focus behavior.
- Keep the gallery smoke in focused Tree mode and click the explicit `paper` row, not the root center or another guessed chrome point.

# Disposition
- Accepted. No further Tree-specific repair needed for U5.
