---
type: Current State
title: open-gpui gallery interaction hardening state
status: active
source_session: 019ec6c8-5566-7062-8458-21ebe1360573
git_branch: main
git_commit: d64f5d6
verified_by:
  - cargo nextest run -p open-gpui-ui-foundation-gallery components_gallery_smoke_navigation_rail_scrolls_inside_shell components_gallery_smoke_vertical_tabs_scroll_inside_sample components_gallery_smoke_scroll_area_samples_scroll_inside_page components_gallery_smoke_scrolls_short_viewport_and_resets_page_on_navigation
  - cargo nextest run -p open-gpui-ui-components scroll_area_default_handle_survives_reconstructed_component_values scroll_area_reset_key_resets_default_runtime_handle scroll_area_runtime_scrolls_horizontal_and_two_axis_content tabs_vertical_tablist_scrolls_when_constrained
  - cargo nextest run -p open-gpui-ui-components alert_dialog_state_records_required_actions_and_destructive_intent alert_dialog_state_blocks_underlay_and_restores_focus_to_trigger
  - cargo nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke_dismisses_popover_from_outside_press overlay_gallery_smoke_opens_hover_card_from_real_trigger_and_dismisses overlay_gallery_smoke_closes_dialog_from_modal_barrier_and_escape overlay_gallery_smoke_closes_non_modal_sheet_from_outside_press overlay_gallery_smoke_closes_menu_from_escape_and_outside_press overlay_gallery_smoke_opens_context_menu_from_right_click_and_dismisses components_gallery_smoke_closes_select_popup_from_outside_press
  - cargo nextest run -p open-gpui-ui-foundation-gallery overlay_gallery_smoke_closes_alert_dialog_from_action_and_escape
  - cargo nextest run -p open-gpui-ui-components overlay_adapter_config_defaults_follow_overlay_kind_policy overlay_open_change_helpers_match_core_policies splitter_runtime_drag_resizes_horizontal_and_vertical_panels splitter_state_normalizes_panel_fractions_and_constraints splitter_resize_delta_clamps_to_adjacent_min_max splitter_runtime_fraction_overrides_still_use_resize_constraints splitter_collapsed_panel_uses_collapsed_fraction
---

# Current State

- Goal: 继续审查 open-gpui 的 gallery / component 行为契约一致性，允许无畏重构，但只收真实问题。
- Branch: `main`
- Last verified: 2026-06-21, focused gallery and component nextest commands passed after hardening the Components-page scroll surfaces and adding the AlertDialog gallery gate.
- Done: Added gallery layout constraints so the navigation rail, ScrollArea sample wrappers, and vertical Tabs sample keep their own overflow surfaces.
- Done: Added gallery smoke coverage for navigation rail scrolling, constrained vertical Tabs scrolling, and ScrollArea wheel scrolling in the Components page.
- Done: Added gallery smoke coverage for AlertDialog trigger -> action -> Escape dismissal and focus restoration.
- Done: Confirmed existing overlay and splitter runtime regression gates remain green.
- Blocked: 暂无。
- Next action: stage the docs memory refresh, commit it on a feature branch, merge it back to `main`, and push if the diff stays clean; the remaining overlay / splitter review should now focus on any residual gaps outside AlertDialog.

# Citations

[1] Plan `docs/plans/2026-06-20-001-refactor-ui-gallery-interaction-hardening-plan.md`
[2] Commit `d64f5d6` - `fix(gallery): cover alert dialog dismissal path`
[3] Commit `14efadc` - `fix(gallery): harden components page scroll surfaces`
[4] AlertDialog gallery gate added on 2026-06-21
[5] Session `019ec6c8-5566-7062-8458-21ebe1360573`
