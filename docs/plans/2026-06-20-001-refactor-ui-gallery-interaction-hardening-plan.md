---
title: Open GPUI UI Gallery Interaction Hardening Plan
type: refactor
date: 2026-06-20
execution: code
---

# Open GPUI UI Gallery Interaction Hardening Plan

## Summary

Harden the UI gallery so the page shell, navigation rail, component samples, overlay surfaces, and splitter samples all keep their documented behavior under composition.

The plan keeps the current productized-crate direction. It does not introduce a headless crate, and it treats the gallery as the contract surface for composed behavior.

## Problem Frame

The gallery already proves a lot of component state, but the composed experience still needs sharper contracts around scrolling, dismissal, and vertical resizing.

Recent smoke coverage passes, yet the reported symptoms point to gaps at the composed surface: a viewport can look scrollable but not actually move, a popup can open without a reliable close path, and a vertical splitter can appear constrained until its runtime state is exercised under the gallery shell.

## Requirements

### Scroll and viewport

- R1. The Components page must remain scrollable when the content exceeds the viewport.
- R2. The left navigation rail must keep its own scroll position and still expose deep sections when the rail overflows.
- R3. The vertical Tabs sample must scroll its rail inside the sample card when the sample is constrained.
- R4. The ScrollArea sample must keep responding to wheel input after redraws and repeated scroll attempts.

### Overlay dismissal

- R5. Interactive overlay surfaces must close on the documented outside-press and Escape paths.
- R6. Dismissal must restore focus to the documented trigger or fallback target when the surface contract requires it.

### Splitter interaction

- R7. Vertical Splitter samples must resize by pointer drag, restore a collapsed panel, and keep resizing after restoration.

### Evidence and documentation

- R8. Verification and durable memory must record the new regression gates so later sessions do not rediscover the same composed-behavior gaps.

## Key Technical Decisions

- Keep the gallery shell as the top-level composition root. The page viewport and the navigation rail stay separate scroll surfaces, and component samples own their own local scroll or drag behavior.
- Prove the bugs at the composed seam. Component-state tests stay useful, but the gallery smoke tests must catch regressions that only appear when the shell composes real widgets.
- Keep dismissal and focus restoration inside the existing overlay contracts. The gallery should exercise them, not redefine them.
- Keep the `repo-ref/fret` lesson as a design input, not a new crate boundary. The useful pattern is thin facade plus deep helper and clear visible/window policy, not headless extraction.

## High-Level Technical Design

The gallery shell stays responsible for layout and viewport composition.

The component crates stay responsible for their own resolved state, runtime interaction, and adapter behavior.

The verification layer sits on top of both. It uses focused component tests for the contract and gallery smoke tests for the composed user flow.

## Implementation Units

### U1. Scroll and viewport consistency

Files:
- `examples/ui-foundation-gallery/src/shell.rs`
- `examples/ui-foundation-gallery/src/pages/components.rs`
- `examples/ui-foundation-gallery/src/pages/components/render.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs`
- `crates/ui_components/tests/components.rs`

Goal:
- Keep the page viewport, navigation rail, and embedded component samples independently scrollable under the gallery shell.
- Tighten the regression coverage so the vertical Tabs rail and ScrollArea samples fail when scrolling stops working under composition.

Execution note:
- Characterization-first for the gallery smoke, then fix the layout or scroll seam that the test exposes.

Patterns to follow:
- `examples/ui-foundation-gallery/src/shell.rs` for the page and navigation viewport shells.
- `examples/ui-foundation-gallery/src/pages/components/render.rs` for sample-specific scroll containers.
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs` for bounded wheel-event helpers and visibility assertions.
- `crates/ui_components/tests/components.rs` for `ScrollArea` and vertical `Tabs` runtime coverage.

Test scenarios:
- The Components page scrolls to a deep sample on a short viewport.
- The left navigation rail scrolls independently and still selects the Components page.
- The vertical Tabs rail scrolls inside the sample card when constrained.
- The ScrollArea sample continues to move after redraws and repeated wheel events.

Verification:
- `cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation components_gallery_smoke_scroll_area_samples_scroll_inside_page components_gallery_smoke_tabs_and_splitter_interactions_survive_full_page_composition`
- `cargo nextest run -p open-gpui-ui-components scroll_area_default_handle_survives_reconstructed_component_values scroll_area_reset_key_resets_default_runtime_handle scroll_area_runtime_scrolls_horizontal_and_two_axis_content tabs_vertical_tablist_scrolls_when_constrained`

### U2. Overlay dismissal and focus restoration

Files:
- `examples/ui-foundation-gallery/src/shell.rs`
- `examples/ui-foundation-gallery/src/pages/overlay.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs`
- `crates/ui_components/tests/components.rs`

Goal:
- Make the gallery-proved overlay flows close reliably on outside press and Escape.
- Preserve the documented focus restoration targets after dismissal.

Execution note:
- Characterization-first for any popup that is currently hard to dismiss, then fix the real contract seam rather than layering a gallery-only workaround.

Patterns to follow:
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs` overlay smokes for Popover, HoverCard, Dialog, Sheet, Menu, ContextMenu, Select, Combobox, and Command.
- `crates/ui_components/tests/components.rs` overlay adapter and runtime dismissal tests.
- `examples/ui-foundation-gallery/src/shell.rs` for the top-level Escape handler and page-level reset behavior.

Test scenarios:
- A controlled popup closes on outside press and restores focus when the contract says it should.
- A controlled popup closes on Escape and does not leave a stale overlay behind.
- A dialog or context menu closes through its documented dismissal path and still allows re-entry.
- The gallery smoke fails if a popup opens but cannot be dismissed from the documented path.

Verification:
- `cargo nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke_dismisses_popover_from_outside_press overlay_gallery_smoke_opens_hover_card_from_real_trigger_and_dismisses overlay_gallery_smoke_closes_dialog_from_modal_barrier_and_escape overlay_gallery_smoke_closes_non_modal_sheet_from_outside_press overlay_gallery_smoke_closes_menu_from_escape_and_outside_press overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses components_gallery_smoke_closes_select_popup_from_outside_press`
- `cargo nextest run -p open-gpui-ui-components overlay_adapter_config_defaults_follow_overlay_kind_policy overlay_open_change_helpers_match_core_policies`

### U3. Vertical Splitter behavior

Files:
- `examples/ui-foundation-gallery/src/pages/components/render.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs`
- `crates/ui_components/tests/components.rs`

Goal:
- Keep the vertical Splitter sample draggable under the gallery shell.
- Preserve collapsed-panel restore behavior and subsequent resizing after restore.

Execution note:
- Test-first for the vertical drag and restore regression.

Patterns to follow:
- `crates/ui_components/src/splitter.rs` for the existing orientation and collapsed-panel contract.
- `crates/ui_components/tests/components.rs` for the horizontal and vertical drag smoke.
- `examples/ui-foundation-gallery/src/pages/components/render.rs` for the composed gallery sample shell.

Test scenarios:
- A vertical Splitter drag grows the first panel and shrinks the second.
- A collapsed vertical panel restores after crossing the restore threshold.
- A restored panel still responds to later pointer drags.
- The gallery smoke fails if the vertical Splitter only works in isolation but not under the Components page.

Verification:
- `cargo nextest run -p open-gpui-ui-components splitter_runtime_drag_resizes_horizontal_and_vertical_panels splitter_state_normalizes_panel_fractions_and_constraints splitter_resize_delta_clamps_to_adjacent_min_max splitter_runtime_fraction_overrides_still_use_resize_constraints splitter_collapsed_panel_uses_collapsed_fraction`
- `cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_tabs_and_splitter_interactions_survive_full_page_composition`

### U4. Evidence, docs, and memory

Files:
- `docs/ui/component-contract.md`
- `docs/verification.md`
- `docs/knowledge/engineering/current-state.md`
- `docs/knowledge/engineering/log.md`

Goal:
- Record the updated scroll, dismissal, and splitter regression gates.
- Preserve the current design lessons so the next session starts from verified evidence instead of chat context.

Test scenarios:
- The verification doc lists the focused component and gallery commands that protect the new behavior.
- The engineering memory captures the current branch state, the verified commands, and the next action.

## Scope Boundaries

- Do not introduce `open-gpui-ui-headless`.
- Do not rewrite the entire gallery shell just to fix one composed behavior seam.
- Do not preserve compatibility fields when a resolved-state or runtime contract already owns the truth.
- Do not widen the plan to unrelated pages unless they share the same scroll, dismissal, or resize failure mode.

## Risks & Dependencies

| Risk | Impact | Mitigation |
| --- | --- | --- |
| The current smoke tests can stay green while the user symptom still exists. | Medium | Tighten assertions at the composed seam and keep the regression tests close to the actual viewport or trigger. |
| Scroll and dismissal behavior can vary by platform event delivery. | Medium | Prefer deterministic wheel, drag, and bounds checks over timing-sensitive assumptions. |
| A fix in one surface may need a small shell adjustment as well as a component change. | Medium | Keep the units separate and re-run the gallery smoke after each slice. |

## Acceptance Examples

- AE1. Given a short gallery viewport, when the Components page scrolls, a deep sample becomes visible, and switching away and back resets the page scroll.
- AE2. Given a constrained vertical Tabs sample, when the tab rail receives wheel input, later tabs move into view.
- AE3. Given a controlled overlay, when outside press or Escape fires, the popup closes and focus returns to the documented target.
- AE4. Given a collapsed vertical Splitter panel, when the handle crosses the restore threshold, the panel reopens and later drags still resize it.

## Sources / Research

- `examples/ui-foundation-gallery/src/shell.rs`
- `examples/ui-foundation-gallery/src/pages/components.rs`
- `examples/ui-foundation-gallery/src/pages/components/render.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs`
- `crates/ui_components/src/scroll_area.rs`
- `crates/ui_components/src/tabs.rs`
- `crates/ui_components/src/splitter.rs`
- `crates/ui_components/tests/components.rs`
- `docs/ui/component-contract.md`
- `docs/verification.md`
- `repo-ref/fret/crates/fret-ui/src/declarative/host_widget/layout/scrolling.rs`
- `repo-ref/fret/crates/fret-ui/src/tree/prepaint/virtual_list.rs`
- `repo-ref/fret/crates/fret-diag/src/stats/windowed_rows.rs`
- `repo-ref/fret/crates/fret-diag/src/stats/vlist.rs`

## Documentation / Operational Notes

- If a new regression test becomes the canonical gate for one of these behaviors, update `docs/verification.md` in the same slice.
- After the implementation lands, refresh `docs/knowledge/engineering/current-state.md` and `docs/knowledge/engineering/log.md` with the verified commands and the next action.
