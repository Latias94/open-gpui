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
- `DockTransitionPlan::from_focus_region` emits `FocusRing` overlay transition descriptors aligned
  with GPUI focus requests instead of replacing focus authority.
- Motion review against `$emil-design-eng` / `$review-animations` kept zoom/unzoom on the existing
  180ms ease-out layout spec and made public focus commands immediate because focus is high-frequency
  and keyboard reachable. Explicit internal/test entry points can still pass an animated
  `MotionSpec` for lower-frequency focus-ring transition proofs.
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

## U7 / Phase C Split Primitive Cleanup

Implemented U7 shared split primitive cleanup:

- `open_gpui_ui_core::split` now owns generic split fraction normalization, fill-child share
  resolution, `SplitterState::resize_by_pixels`, and pixel-delta adjacent resize helpers.
- Docking graph canonicalization, graph mutation, edge-dock insertion, render flex shares,
  presentation-scene split layout, and splitter drag transactions consume those helpers directly.
- Deleted the docking-local `split_fraction.rs` module and the `DockSplitLayout` wrapper. Docking
  geometry now keeps docking-specific drop-guide boxes and GPUI `Bounds<Pixels>` conversion instead
  of carrying generic split math.
- `docs/verification.md` records the shared primitive boundary so future split changes keep generic
  fraction/pixel math in `ui_core` and docking-specific policy in docking.

## U7 Commands

- `cargo check -p open-gpui-ui-core --tests` - passed.
- `cargo check -p open-gpui-docking --tests` - passed.
- `cargo nextest run -p open-gpui-ui-core split --no-fail-fast` - passed, 19 tests.
- `cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast` -
  passed, 13 tests.
- `cargo nextest run -p open-gpui-docking geometry workspace_resize_policy_tests host_interaction_tests::horizontal_splitter_drag_updates_width_fractions host_interaction_tests::vertical_splitter_drag_updates_height_fractions host_interaction_tests::splitter_drag_clamps_to_minimum_pane_size host_render_tests::central_split_child_uses_remaining_render_space host_render_tests::horizontal_split_uses_normalized_flex_shares host_render_tests::vertical_split_uses_normalized_flex_shares host_render_tests::unnormalized_split_fractions_are_repaired_for_rendering --no-fail-fast` -
  passed, 44 tests.

## U7 Verification Notes

The U7 extraction deliberately did not export rectangle-neighbor navigation or docking policy
objects. `resolve_split_fractions_with_fill_child` is a generic fill-child helper; docking decides
which child, if any, is the central fill child before calling it.

## U8 / Phase C Corner Drag And Spatial Navigation

Implemented U8 visible corner drag and docking-private spatial navigation:

- `DockDividerHitMap` now derives corner affordance records from the same scene-backed handle and
  junction targets that drive splitter hit testing.
- Corner affordances expose explicit rendered states for idle, hover, active, and disabled
  feedback. Render paints corner grips from those descriptors and sets resize cursors for both
  single-axis handles and corner junctions. Clamp and rejected-resize behavior is covered by resize
  transaction tests rather than phantom visual states.
- Real rendered diagonal corner dragging now begins a two-axis splitter drag and commits both resize
  updates through the existing workspace transaction path.
- Runtime resize tests lock the min-size clamp behavior where one axis can clamp without corrupting
  the other axis update.
- Splitter accessibility action tests now cover both horizontal increment and vertical decrement.
- `DockSpatialDirection` is a public docking command input. The rectangle-neighbor resolver remains
  private to docking and ranks candidates by direction, perpendicular overlap, then distance using
  the current presentation scene.

## U8 Commands

- `cargo check -p open-gpui-docking --tests` - passed.
- `cargo nextest run -p open-gpui-docking spatial_navigation_tests host_zoom_focus_tests::host_focus_neighbor_command_uses_spatial_navigation host_accessibility_tests::accessibility_splitter_actions_resize_through_transaction_path host_accessibility_tests::accessibility_vertical_splitter_actions_target_vertical_axis host_divider_hit_map_tests host_interaction_tests::horizontal_splitter_drag_updates_width_fractions host_interaction_tests::vertical_splitter_drag_updates_height_fractions host_interaction_tests::splitter_drag_clamps_to_minimum_pane_size host_interaction_tests::corner_splitter_drag_updates_both_axes_through_rendered_events interaction::tests::corner_splitter_drag_produces_two_axis_resize_request interaction::tests::corner_splitter_drag_clamps_one_axis_without_corrupting_other_axis render::tests::divider_affordance_states_have_distinct_feedback_colors --no-fail-fast` -
  passed, 17 tests.
- `cargo fmt --all -- --check` - passed.
- `git diff --check` - passed.

## U8 Verification Notes

The spatial navigation algorithm intentionally stays in `gpui_docking` rather than
`open_gpui_ui_core`: it depends on docking presentation panes, selected tab focus regions, and
host focus commands. The split primitive boundary from U7 remains intact.

## U9 / Phase C Routed Overlay Cleanup

Implemented U9 routed overlay cleanup proof:

- Source-window route markers and target-window drop previews now have explicit test coverage as
  separate overlay responsibilities. Source markers do not render target payload tab previews, and
  target previews do not render source route marker regions.
- `DockTransitionPlan::from_overlay_scene` now has focused coverage for route-marker transitions,
  proving routed source feedback participates in the descriptor-based motion path instead of being a
  render-only special case.
- Replacing a routed target preview clears the previous target window's runtime preview and rendered
  payload tab previews before release, so stale target facts cannot commit.
- Escape during a real GPUI docking drag clears the source route marker, target preview, scoped
  runtime drag session, and GPUI active drag together.
- Docking drag focus handling is scoped to active drag frames. The host can receive Escape while a
  docking drag is active, but ordinary failed panel focus still preserves the current GPUI focus and
  history.

## U9 Commands

- `cargo check -p open-gpui-docking --tests` - passed.
- `cargo nextest run -p open-gpui-docking transition_plan_from_route_overlay_describes_source_marker source_hover_over_known_viewport_renders_target_drop_preview routed_preview_replacement_clears_old_target_overlay_without_stale_payload escape_clears_routed_marker_target_overlay_and_active_drag viewport_runtime_begin_payload_drag_clears_previous_routed_preview viewport_runtime_revalidates_routed_preview_release_against_current_policy --no-fail-fast` -
  passed, 6 tests.
- `cargo nextest run -p open-gpui-docking host_render_tests::viewport_failed_panel_focus_preserves_current_focus_and_history host_viewport_preview_tests::handle_suite::escape_clears_routed_marker_target_overlay_and_active_drag --no-fail-fast` -
  passed, 2 tests.
- `cargo nextest run -p open-gpui-docking host_viewport_preview_tests host_render_tests host_transition_tests --no-fail-fast` -
  passed, 78 tests.

## U9 Verification Notes

The routed overlay cleanup keeps current viewport facts authoritative. Cached route previews can
drive visible feedback, but release still revalidates against current policy and current target
facts. Escape cancellation intentionally uses render-scoped focus capture only while a
`DockDragPayload` is active, avoiding a permanent focusable root around normal docking content.

## U10 / Phase Closeout Dogfood Proof

Recorded U10 phase closeout evidence:

- The native runtime status panel proof string now names the currently shipped preview/motion
  capabilities: presentation scene, overlay layers, tab insertion, motion, zoom, divider hit map,
  corner drag, accessibility, route cleanup, and reduced motion.
- `docs/verification.md` dogfood steps cover routed target replacement cleanup and Escape
  cancellation after a target preview appears.
- Engineering memory now treats U9 as complete and points the next action at final U11 ADR/helper
  cleanup.

## U10 Commands

- `cargo check -p open-gpui-docking-native` - passed. Existing upstream/macOS warnings remain,
  including `objc` macro `unexpected_cfgs`; U11 cleanup resolved the docking-local dead-code
  warnings reported during this check.
- `cargo nextest run -p open-gpui-docking-native runtime_status_panel_formats_platform_capabilities --no-fail-fast` -
  passed, 1 test.

## U10 Verification Notes

The proof string is intentionally capability-oriented rather than pixel-oriented. It does not claim
transparent drag windows, platform-native accessibility action callbacks, screenshot baselines, or
every-frame-perfect animation; those remain outside the current shipped boundary unless a later plan
adds them.

## U11 / Phase Closeout ADR And Helper Cleanup

Implemented U11 final ADR/helper cleanup:

- Added ADR 0012 to record the runtime capability boundary after implementation: graph semantics,
  presentation scenes, overlay descriptors, transition executor scope, current-facts route authority,
  GPUI accessibility mapping limits, and native dogfood proof text.
- Removed the unused overlay-scene `FocusRing` layer variant. Focus-ring feedback remains a
  transition-plan overlay kind, which is the runtime path that actually constructs it.
- Removed unconstructed corner affordance visual states. The renderer now only exposes the states
  produced by `DockDividerHitMap`: idle, hover, active, and disabled.
- Narrowed test-only descriptor helpers with `#[cfg(test)]`, including payload ghost inspection and
  overlay-scene-to-transition conversion for route/tab/payload/rejected descriptor proof.
- Wired stale zoom target cleanup into the render path so a zoom target removed from the graph is
  cleared during the next presentation-scene refresh.

## U11 Commands

- `cargo check -p open-gpui-docking` - passed with no `open-gpui-docking` local warnings.
- `cargo check -p open-gpui-docking --tests` - passed.
- `cargo nextest run -p open-gpui-docking host_zoom_focus_tests host_transition_tests host_divider_hit_map_tests host_viewport_preview_visual_tests host_render_tests::transition_sample_overlay_renders_from_executor --no-fail-fast` -
  passed, 32 tests.
- `cargo fmt --all -- --check` - passed.
- `cargo nextest run -p open-gpui-ui-core split motion --no-fail-fast` - passed, 21 tests.
- `cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast` -
  passed, 13 tests.
- `cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests host_interaction_tests workspace_resize_policy_tests --no-fail-fast` -
  passed, 97 tests.
- `cargo check -p open-gpui-docking-native` - passed. Existing macOS `objc` macro warnings and the
  workspace `block v0.1.6` future-incompat warning remain outside this docking cleanup.
- `$emil-design-eng` / `$review-animations` follow-up review found and fixed one focus-motion issue:
  public `focus_pane` and `focus_neighbor_pane` now expose immediate focus-ring feedback instead of a
  120ms animation.
- `cargo nextest run -p open-gpui-docking host_zoom_focus_tests --no-fail-fast` - passed, 12 tests
  after the public focus command motion correction.

## U11 Verification Notes

The remaining accessibility `cfg_attr(not(test), allow(dead_code))` annotations are intentional
descriptor retention, not obsolete scaffolding. GPUI render adapters consume the currently supported
element subset, while tests and docs preserve descriptor fields that GPUI cannot yet expose.
