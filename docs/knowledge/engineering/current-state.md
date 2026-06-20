---
type: Current State
title: Open GPUI current engineering state
status: active
timestamp: 2026-06-20
---

# Current State

- Goal: Continue UI component contract alignment and remove evidence-backed behavioral drift without preserving old compatibility layers.
- Branch: `main`
- Last verified: `cargo check -p open-gpui-ui-components`; `cargo nextest run -p open-gpui-ui-components --test components avatar_fallback_initials_derive_from_display_names_and_empty_names avatar_explicit_fallback_overrides_derived_initials avatar_source_metadata_does_not_own_loading_state avatar_accessible_label_can_be_explicit_for_source_and_fallback_avatars avatar_size_metrics_and_token_intents_are_stable avatar_renders_stable_debug_selector`; `cargo nextest run -p open-gpui-ui-foundation-gallery official_component_catalog_entries_have_signals_and_sample_selectors components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation`
- Done: Confirmed the Avatar contract and gallery contract gates still pass, and re-checked `repo-ref/fret`'s scroll/visibility layering. The reference repo splits that work into helper modules, which supports the current-crate productization roadmap rather than a new headless boundary.
- In progress: No evidence-backed UI drift remains from the current scan; waiting for a concrete mismatch before making more changes.
- Blocked: None.
- Next action: Stop the scan unless a new regression appears, or move to the next explicit product slice.

# Citations

[1] [Plan](../../plans/2026-06-18-001-refactor-ui-component-contract-alignment-plan.md)
[2] [Verification](verification/menu-runtime-focus-regression-20260620.md)
