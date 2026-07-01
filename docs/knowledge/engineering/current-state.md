---
type: Current State
title: Docking runtime capability follow-up state
status: active
timestamp: 2026-07-01T15:32:49+08:00
git_branch: main
related_plan:
  - docs/plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md
  - docs/plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md
related_adr:
  - docs/adr/0010-docking-presentation-scene-motion-model.md
  - docs/adr/0011-docking-split-motion-primitive-boundary.md
  - docs/adr/0012-docking-runtime-capability-alignment.md
verified_by:
  - cargo check -p open-gpui-docking --tests
  - cargo nextest run -p open-gpui-docking host_transition_tests host_render_tests --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_viewport_preview_visual_tests host_transition_tests host_interaction_tests::tab_bar_preview_positions_payload_tab_at_leading_and_middle_slots host_interaction_tests::tab_bar_append_preview_shifts_payload_tab_right_of_existing_tab host_interaction_tests::dragging_tab_to_target_tab_bar_empty_area_appends drop_runtime::tests::reorder_target_updates_insert_index_within_same_tab_stack drop_runtime::tests::tabs_drop_preserves_reorder_target_while_pointer_stays_inside_tab host_viewport_preview_tests::handle_suite::source_hover_over_known_viewport_renders_target_drop_preview --no-fail-fast
  - cargo nextest run -p open-gpui-docking geometry host_accessibility_tests --no-fail-fast
  - cargo fmt --all -- --check
  - cargo nextest run -p open-gpui-ui-core --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests --no-fail-fast
  - cargo check -p open-gpui-docking-native
  - cargo check -p open-gpui-docking --tests
  - cargo check -p open-gpui-ui-core --tests
  - cargo check -p open-gpui-ui-components --tests
  - cargo nextest run -p open-gpui-docking host_accessibility_tests --no-fail-fast
  - cargo nextest run -p open-gpui-ui-core a11y --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components a11y --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_render_tests host_interaction_tests host_transition_tests host_zoom_focus_tests --no-fail-fast
  - cargo check -p open-gpui-ui-core --tests
  - cargo check -p open-gpui-docking --tests
  - cargo nextest run -p open-gpui-ui-core split --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast
  - cargo nextest run -p open-gpui-docking geometry workspace_resize_policy_tests host_interaction_tests::horizontal_splitter_drag_updates_width_fractions host_interaction_tests::vertical_splitter_drag_updates_height_fractions host_interaction_tests::splitter_drag_clamps_to_minimum_pane_size host_render_tests::central_split_child_uses_remaining_render_space host_render_tests::horizontal_split_uses_normalized_flex_shares host_render_tests::vertical_split_uses_normalized_flex_shares host_render_tests::unnormalized_split_fractions_are_repaired_for_rendering --no-fail-fast
  - cargo nextest run -p open-gpui-docking spatial_navigation_tests host_zoom_focus_tests::host_focus_neighbor_command_uses_spatial_navigation host_accessibility_tests::accessibility_splitter_actions_resize_through_transaction_path host_accessibility_tests::accessibility_vertical_splitter_actions_target_vertical_axis host_divider_hit_map_tests host_interaction_tests::horizontal_splitter_drag_updates_width_fractions host_interaction_tests::vertical_splitter_drag_updates_height_fractions host_interaction_tests::splitter_drag_clamps_to_minimum_pane_size host_interaction_tests::corner_splitter_drag_updates_both_axes_through_rendered_events interaction::tests::corner_splitter_drag_produces_two_axis_resize_request interaction::tests::corner_splitter_drag_clamps_one_axis_without_corrupting_other_axis render::tests::divider_affordance_states_have_distinct_feedback_colors --no-fail-fast
  - cargo nextest run -p open-gpui-docking transition_plan_from_route_overlay_describes_source_marker source_hover_over_known_viewport_renders_target_drop_preview routed_preview_replacement_clears_old_target_overlay_without_stale_payload escape_clears_routed_marker_target_overlay_and_active_drag viewport_runtime_begin_payload_drag_clears_previous_routed_preview viewport_runtime_revalidates_routed_preview_release_against_current_policy --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_render_tests::viewport_failed_panel_focus_preserves_current_focus_and_history host_viewport_preview_tests::handle_suite::escape_clears_routed_marker_target_overlay_and_active_drag --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_viewport_preview_tests host_render_tests host_transition_tests --no-fail-fast
  - cargo check -p open-gpui-docking-native
  - cargo nextest run -p open-gpui-docking-native runtime_status_panel_formats_platform_capabilities --no-fail-fast
  - cargo check -p open-gpui-docking
  - cargo check -p open-gpui-docking --tests
  - cargo nextest run -p open-gpui-docking host_zoom_focus_tests host_transition_tests host_divider_hit_map_tests host_viewport_preview_visual_tests host_render_tests::transition_sample_overlay_renders_from_executor --no-fail-fast
  - cargo nextest run -p open-gpui-ui-core split motion --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests host_interaction_tests workspace_resize_policy_tests --no-fail-fast
  - git diff --check
  - python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering
---

# Current State

- Goal: Execute `docs/plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md`
  with fearless docking runtime capability refactors.
- Branch: `main`, ahead of `origin/main` by local docking commits.
- Done: `docs/plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md` is implemented
  and committed through `3497a85`.
- Done: Shared split and motion primitives live in `open_gpui_ui_core`; `ui_components::Splitter`
  renders through core state; docking presentation, overlay, transition descriptors, zoom/focus,
  divider hit maps, accessibility descriptors, and resize transactions consume those primitives.
- Done: Read-only subagent research synthesized the next capability gaps: real transition timeline
  sampling, sampled overlay/clip/divider render integration, precise tab insertion, payload ghost
  cleanup, zoom/focus user surfaces, GPUI accessibility mapping, corner-drag proof, and continued
  split primitive cleanup.
- Done: Phase A / P0 for the runtime capability plan is implemented locally. `DockTransitionExecutor`
  now samples progress with deterministic test timing, reduced motion exposes a final sample once,
  entering panes keep final-size content bounds while reveal clips animate, and render consumes
  sampled overlay/clip/divider output as a root transition layer over the final semantic layout.
- Done: Phase B / P0 local and routed preview capability is implemented locally. Center/tab preview
  now has explicit `PayloadTab` and `PayloadGhost` overlay layers, precise insertion slot geometry can
  be applied to overlay descriptors, rendered tab-bar hover resolves label-specific insert indexes
  before falling back to append, and same-stack reorder hold no longer freezes a changed insert slot.
  Routed target previews now expose the same payload tab/ghost layer contract while source route
  markers remain separate.
- Done: U5 zoom/focus presentation is implemented locally. Public host zoom/focus commands now use
  the latest rendered presentation frame when available, emit transition executor plans, preserve
  `DockGraph`, and keep public focus feedback immediate because focus is high-frequency and keyboard
  reachable. Zoom egress uses deterministic touching-preferred edges from `DockZoomScene.egress`;
  reduced-motion zoom/unzoom/focus samples preserve final scene semantics.
- Done: U6 GPUI accessibility mapping is implemented locally. Docking descriptors now map to
  GPUI-facing stable element IDs, roles, labels, selected/disabled/orientation/numeric state, tab
  focus/select actions, splitter increment/decrement callbacks through resize transactions, and
  short-lived overlay descriptors for active drop/drag/reject feedback. GPUI currently lacks a
  generic hint/description field and platform drop action callback; the limitation is recorded in
  `docs/verification.md`.
- Done: U7 split primitive cleanup is implemented locally. `open_gpui_ui_core::split` now owns
  generic fraction normalization, fill-child share resolution, `SplitterState::resize_by_pixels`,
  and pixel-delta adjacent resize helpers. Docking consumes those helpers for graph normalization,
  render flex shares, presentation split layout, and splitter drag transactions; the docking-local
  `split_fraction.rs` module and `DockSplitLayout` wrapper were deleted.
- Done: U8 visible corner drag and docking-private spatial navigation are implemented locally.
  Corner divider affordances now have explicit visible states, render-time grip feedback and
  resize cursors, real rendered diagonal corner drag updates both split axes, a11y splitter actions
  target horizontal and vertical axes, and `DockSpatialDirection` drives a private
  rectangle-neighbor resolver for pane focus commands.
- Done: U9 routed overlay cleanup proof is implemented locally. Source route markers and target
  previews have separate overlay ownership, route markers enter transition descriptors, replacing a
  routed target clears stale target overlays, and Escape during a real GPUI docking drag clears the
  source marker, target preview, runtime session, and active drag without stealing normal panel
  focus.
- Done: U10 native dogfood closeout is implemented locally. The runtime status panel proof string
  now names corner-drag and route-cleanup alongside presentation, overlay, tab insertion, motion,
  zoom, divider-hit-map, a11y, and reduced-motion capability proofs.
- Done: U11 final ADR/helper cleanup is committed. ADR 0012 records the runtime
  capability boundary, unused overlay `FocusRing` layer and phantom corner visual states were
  deleted, test-only descriptor helpers were narrowed with `#[cfg(test)]`, stale zoom targets now
  clear on render, and `open-gpui-docking` no longer emits the local dead-code warnings found by
  native compile.
- Done: Animation/UI interaction review found and fixed public focus-command motion. Public
  `focus_pane` and `focus_neighbor_pane` now expose immediate focus-ring semantic feedback instead of
  a 120ms high-frequency animation.
- Last verified: Phase A, Phase B, U5, U6, U7, U8, U9, U10, and U11 focused/final gates passed
  locally; see the runtime capability verification evidence file.
- Blocked: None.
- Next action: no required work remains for the runtime capability alignment plan. Deferred follow-up
  work remains limited to the explicitly deferred public animation executor, public rectangle-neighbor
  extraction, broader platform accessibility automation, and optional absolute pane renderer.

# Citations

- [Split primitive plan](../../plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md)
- [Runtime capability follow-up plan](../../plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md)
- [ADR 0010](../../adr/0010-docking-presentation-scene-motion-model.md)
- [ADR 0011](../../adr/0011-docking-split-motion-primitive-boundary.md)
- [ADR 0012](../../adr/0012-docking-runtime-capability-alignment.md)
- [Progress note](progress/2026-06-30-docking-split-motion-primitives.md)
- [Verification evidence](verification/docking-split-motion-primitives-20260630.md)
- [Runtime capability verification evidence](verification/docking-runtime-capability-alignment-20260701.md)
- [Follow-up subagent synthesis](subagents/docking-runtime-capability-followup-20260630.md)
