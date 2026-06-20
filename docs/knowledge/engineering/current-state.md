---
type: Current State
title: Open GPUI current engineering state
status: active
timestamp: 2026-06-20
---

# Current State

- Goal: Continue UI component contract alignment and remove evidence-backed behavioral drift without preserving old compatibility layers.
- Branch: `main`
- Last verified: `cargo test -p open-gpui-ui-foundation-gallery --bin open-gpui-ui-foundation-gallery initial_page_parses_equals_form`; `cargo test -p open-gpui-ui-foundation-gallery --bin open-gpui-ui-foundation-gallery initial_page_parses_split_form_and_falls_back`; `cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation components_gallery_smoke_tabs_and_splitter_interactions_survive_full_page_composition`; direct `cargo run -p open-gpui-ui-foundation-gallery -- --page components` stayed alive until timeout instead of reproducing the earlier `accesskit_consumer` exit.
- Done: Re-checked the Components-page crash line and `repo-ref/fret`'s scroll/list-box helper layering. The reference repo still points at thin entry points with deeper helper modules, which supports the current-crate productization roadmap rather than a new headless boundary.
- In progress: No evidence-backed UI drift remains from the current scan; the old Components-page exit is currently not reproducible in this checkout.
- Blocked: None.
- Next action: Wait for a fresh failing path before changing code, or move to the next explicit product slice.

# Citations

[1] [Plan](../../plans/2026-06-18-001-refactor-ui-component-contract-alignment-plan.md)
[2] [Verification](verification/menu-runtime-focus-regression-20260620.md)
