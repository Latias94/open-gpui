---
type: Verification Evidence
title: Docking runtime capability alignment verification
status: active
timestamp: 2026-07-01T00:00:00+08:00
git_branch: main
related_plan: docs/plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md
---

# Verification Evidence

## Phase A / P0

Implemented U1-U3 foundation for runtime transition capability:

- `DockTransitionExecutor` now stores transition start state, samples deterministic progress, applies easing, exposes completion and next-frame intent, and clears completed transitions after the final sample.
- Reduced motion transitions expose a final sample once and complete immediately.
- Entering panes keep final-size content bounds from the first animated sample while a reveal clip grows over time.
- Sampled divider and overlay geometry is available as crate-private render-time data.
- `DockHost::render` consumes sampled transition output as a root-level visual layer over the final semantic layout.
- Transition execution notifies the host; continuous animation-frame requests happen only from render-time sampling, avoiding `Window::request_animation_frame` outside GPUI render phases.

## Commands

- `cargo nextest run -p open-gpui-docking transition_executor_samples_timeline_and_reveal_geometry transition_executor_replaces_active_execution_and_completes_reduced_motion_immediately transition_sample_overlay_renders_from_executor --no-fail-fast` - passed.
- `cargo nextest run -p open-gpui-docking host_transition_tests host_render_tests --no-fail-fast` - passed, 53 tests.
- `cargo fmt --all -- --check` - passed.
- `cargo check -p open-gpui-docking --tests` - passed.
- `git diff --check` - passed.

## Notes

The current Phase A render layer is intentionally descriptor-first and does not replace docking's recursive/flex pane layout. It renders sampled overlay, clip, divider, focus, and payload geometry above the final layout. Full absolute sampled pane rendering remains deferred until Phase A evidence proves it necessary.

## Phase B / P0

Implemented U4 and the routed-preview portion of U9:

- Center/tab preview now emits explicit visible `PayloadTab` layers plus separate `PayloadGhost` layers for transition descriptors.
- `DockOverlayScene::apply_payload_tab_layout` applies resolved body, insertion, payload tab, and ghost bounds to a single overlay descriptor before rendering.
- `DockHost::render_target_drop_preview` resolves tab insertion geometry once from rendered tab label bounds and renders from overlay layer bounds rather than keeping a separate render-only model.
- Tab-bar drag hover now prefers rendered tab label hit targets before falling back to append, so hovering the first tab's left half and the middle tab slot updates the insertion index precisely.
- Same-stack reorder hold now preserves a tab reorder target only for same-slot or leaf-center fallback, and no longer freezes a changed tab insertion slot.
- Routed target previews expose the same payload tab / payload ghost overlay contract while source route markers remain separate.

## Phase B Commands

- `cargo nextest run -p open-gpui-docking overlay_scene_orders_center_tab_insertion_after_guides_before_payload_tabs overlay_scene_applies_precise_tab_layout_to_payload_tabs_and_ghosts transition_plan_from_overlay_scene_describes_tab_insertion_and_payload_ghosts payload_tab_render_inputs_come_from_overlay_layers tab_bar_append_preview_shifts_payload_tab_right_of_existing_tab --no-fail-fast` - passed.
- `cargo nextest run -p open-gpui-docking reorder_target_updates_insert_index_within_same_tab_stack tabs_drop_preserves_reorder_target_while_pointer_stays_inside_tab tab_bar_preview_positions_payload_tab_at_leading_and_middle_slots tab_bar_append_preview_shifts_payload_tab_right_of_existing_tab dragging_tab_to_target_tab_bar_empty_area_appends --no-fail-fast` - passed.
- `cargo nextest run -p open-gpui-docking source_hover_over_known_viewport_renders_target_drop_preview overlay_scene_orders_center_tab_insertion_after_guides_before_payload_tabs overlay_scene_applies_precise_tab_layout_to_payload_tabs_and_ghosts transition_plan_from_overlay_scene_describes_tab_insertion_and_payload_ghosts --no-fail-fast` - passed.
- `cargo nextest run -p open-gpui-docking host_viewport_preview_visual_tests host_transition_tests host_interaction_tests::tab_bar_preview_positions_payload_tab_at_leading_and_middle_slots host_interaction_tests::tab_bar_append_preview_shifts_payload_tab_right_of_existing_tab host_interaction_tests::dragging_tab_to_target_tab_bar_empty_area_appends drop_runtime::tests::reorder_target_updates_insert_index_within_same_tab_stack drop_runtime::tests::tabs_drop_preserves_reorder_target_while_pointer_stays_inside_tab host_viewport_preview_tests::handle_suite::source_hover_over_known_viewport_renders_target_drop_preview --no-fail-fast` - passed, 21 tests.
- `cargo fmt --all -- --check` - passed.
- `cargo check -p open-gpui-docking --tests` - passed.
- `git diff --check` - passed.

## U5 / Phase C Zoom And Focus

Implemented U5 zoom/unzoom/focus presentation capability:

- Public host `zoom_pane`, `unzoom`, and `focus_pane` commands now use the latest rendered
  presentation scene when available and schedule transition executor plans through the same adapter
  path as drag/drop transitions.
- `DockTransitionPlan::from_zoom_scene` consumes `DockZoomScene.egress` so non-target panes leave
  through deterministic touching-preferred edges, while the zoomed pane/focus region become the
  final semantic scene without mutating `DockGraph`.
- `DockTransitionPlan::from_focus_region` emits a short-lived `FocusRing` overlay transition aligned
  with GPUI focus requests instead of replacing focus authority.
- Motion review against `$emil-design-eng` / `$review-animations` kept zoom/unzoom on the existing
  180ms ease-out layout spec and made focus pulse 120ms ease-out overlay-only motion because focus
  can be high-frequency and keyboard reachable.
- Render caches the latest presentation frame from the host-scene probe so public commands can use
  real rendered bounds without asking application callers to provide geometry.

## U5 Commands

- `cargo nextest run -p open-gpui-docking host_zoom_focus_tests --no-fail-fast` - passed, 10 tests.
- `cargo nextest run -p open-gpui-docking host_transition_tests host_render_tests::transition_sample_overlay_renders_from_executor --no-fail-fast` - passed, 8 tests.
- `cargo check -p open-gpui-docking --tests` - passed.
- `cargo fmt --all -- --check` - passed.
- `git diff --check` - passed.

## U5 Verification Notes

During U5 verification, stale background `cargo test -- --list` processes from prior interrupted
work caused temporary test-binary startup hangs sampled at macOS `dyld_start`. Killing the stale
test-list processes restored nextest execution; no source change was required for that environment
issue.

## U6 / Phase C Accessibility Mapping

Implemented U6 GPUI-facing accessibility output:

- `DockAccessibilityScene` now maps docking descriptors into stable GPUI-facing element records with
  deterministic IDs, renderer-neutral role, GPUI role, bounds, labels, hints, selected/disabled
  state, orientation, numeric splitter values, and supported actions.
- Render adapters consume the mapping for tab lists, tabs, selected tab panels, splitter handles,
  and active overlay drop/drag/reject markers.
- Tab accessibility focus actions select the addressed tab, while splitter increment/decrement
  actions route through the existing resize transaction path by simulating a small splitter drag
  from the last presentation scene.
- Active overlay accessibility output remains separate from final-scene output. Drop/drag/reject
  markers are labeled group nodes and do not invent unsupported platform drop actions.
- `docs/verification.md` records the current GPUI platform limitation: generic hint/description and
  drop action callbacks are retained in descriptor data but are not yet exposed through GPUI element
  APIs.

## U6 Commands

- `cargo check -p open-gpui-docking --tests` - passed.
- `cargo check -p open-gpui-ui-core --tests` - passed.
- `cargo check -p open-gpui-ui-components --tests` - passed.
- `cargo nextest run -p open-gpui-docking host_accessibility_tests --no-fail-fast` - passed, 9 tests.
- `cargo nextest run -p open-gpui-ui-core a11y --no-fail-fast` - passed, 2 tests.
- `cargo nextest run -p open-gpui-ui-components a11y --no-fail-fast` - passed, 4 tests.
- `cargo nextest run -p open-gpui-docking host_render_tests host_interaction_tests host_transition_tests host_zoom_focus_tests --no-fail-fast` - passed, 105 tests.
- `cargo fmt --all -- --check` - passed.
- `git diff --check` - passed.

## U6 Verification Notes

`host_accessibility_tests::accessibility_final_semantics_match_reduced_and_animated_zoom` locks the
U6 requirement that reduced motion changes timing only, not final accessibility semantics.
`host_render_tests::viewport_failed_panel_focus_preserves_current_focus_and_history` passed in the
broader regression run, which protects against accessibility metadata accidentally making tab/panel
elements steal GPUI focus during render.
