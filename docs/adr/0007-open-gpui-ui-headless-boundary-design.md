# ADR 0007: Open GPUI UI Headless Boundary Design

**Status**: Accepted
**Date**: 2026-06-17

## Context

ADR 0006 now records a clean strict boundary for `open_gpui_ui_core`: no `open_gpui` dependency,
no GPUI source reference, and no UI-core conversion impls from neutral geometry into GPUI style
types. The remaining question is how to move reusable behavior without dragging concrete GPUI
runtime responsibilities into a future headless crate.

This ADR is a design gate only. It does not create `open-gpui-ui-headless` and does not move
modules.

ADR 0008 later recentered the active roadmap on current-crate productization. This document remains
the extraction-boundary reference if the project explicitly revisits a standalone behavior crate.

Reference repositories support the direction rather than dictating APIs. Fret's headless crate
keeps small deterministic state machines such as roving focus and presence logic free from theme,
recipe, and runtime policy. Open GPUI should use the same split: behavior/state can move; rendering,
handles, subscriptions, AccessKit node wiring, and GPUI scheduling stay in adapters.

## Decision

Create a follow-up implementation plan before creating any crate. The first extraction should move
one behavior family at a time from the current crates into a future behavior crate, with each move
backed by existing tests and a small compatibility layer in `open-gpui-ui-components`.

The first candidates are:

- overlay policy and placement vocabulary;
- roving-focus navigation helpers;
- listbox navigation and typeahead target resolution;
- scroll viewport intent;
- splitter resize constraints.

The following remain GPUI adapter-owned:

- GPUI render trees and `div()` composition;
- `FocusHandle`, focus allocation, and concrete focus movement;
- AccessKit node wiring, node ids, labelled-by/controls relationships, and active-descendant
  wiring;
- `ScrollHandle` and runtime scroll mutation;
- `TextInputController` and GPUI `EntityInputHandler` integration;
- `focus_ring_shadow`;
- `GpuiOverlayState`, deferred priority, snap margin, `anchored`, `deferred`, outside-press
  subscriptions, and concrete Escape event wiring;
- neutral-to-GPUI geometry conversion helpers under `open_gpui_ui_components::gpui_adapter`.

## Interaction Ownership Matrix

| Candidate | Current home | Behavior-owned policy/state | Adapter-owned GPUI responsibilities | Keyboard and focus ownership | Accessibility ownership | Existing tests |
| --- | --- | --- | --- | --- | --- | --- |
| Overlay policy and placement vocabulary | `crates/ui_core/src/overlay.rs`, `crates/ui_components/src/overlay.rs` | `OverlayLayerPolicy`, `OverlayPresence`, `OverlayResolvedState`, dismissal policy, Escape policy, focus restore intent, initial focus intent, placement input, stack ordering | `GpuiOverlayState`, deferred priority, snap margin, `Anchor`, `Point<Pixels>`, `anchored`, `deferred`, barrier rendering, outside-press subscriptions, concrete Escape callbacks | Behavior owns intent and stack ordering; adapter owns `FocusHandle` lookup, focus movement, event subscription, and scheduling | Behavior owns semantic policy and modal/non-modal state; adapter owns AccessKit node references and relationships | `overlay_adapter_config_defaults_follow_overlay_kind_policy`, `overlay_open_change_helpers_match_core_policies`, `overlay_page_samples_expose_behavior_contracts`, overlay component state tests |
| Roving focus | `crates/ui_components/src/roving_focus.rs` | `first_enabled`, `last_enabled`, `next_enabled`, `active_index_from_str_keys`, disabled skip and fallback selection | Concrete focus handles, `track_focus`, key event routing, visible render state | Behavior owns index math and disabled skip; adapter owns focus requests and DOM/GPUI focus handles | Behavior can own neutral selected/focused/tab-stop metadata; adapter owns AccessKit focus node wiring | `tabs_state_resolution_tracks_selected_focus_and_tab_stop`, `radio_group_reuses_roving_focus_helpers_and_skips_disabled_items`, `toolbar_state_exposes_roving_focus_and_toggle_metadata`, `sidebar_navigation_helper_skips_disabled_items` |
| Listbox navigation and typeahead | `crates/ui_components/src/listbox.rs`, reused by `select.rs`, `combobox.rs`, and `command.rs` | option/group flattening, separator and disabled skip, selected/active/tab-stop resolution, activation payloads, typeahead target helpers, empty state | popup rendering, scroll container styling, pointer selection, event dispatch, command callbacks | Behavior owns active/selected target resolution; adapter owns focus handles, click handlers, keyboard event binding, and popup focus movement | Behavior owns neutral roles, selected/focused/disabled metadata, position-in-set and size-of-set values; adapter owns AccessKit active-descendant node ids | `listbox_state_resolves_grouped_options_navigation_and_typeahead`, `select_state_records_popup_listbox_overlay_and_scroll_contract`, `combobox_state_filters_query_without_clearing_selection`, `components_page_choice_samples_expose_listbox_and_select_contracts` |
| Scroll viewport intent | `crates/ui_components/src/scroll_area.rs` | axis, reset policy, reset key, size, scrollbar metrics, `should_reset_for_key_change` | `ScrollHandle`, scroll offset mutation, redraw behavior, overflow styling, wheel handling | Behavior owns reset intent; adapter owns runtime scroll handle and mutation timing | Behavior owns neutral viewport metadata; adapter owns concrete node ids and any scroll-region AccessKit wiring | `scroll_area_state_exposes_axis_metrics_and_reset_policy`, `scroll_area_builder_state_keeps_gpui_handle_out_of_resolved_state`, `scroll_area_default_handle_survives_reconstructed_component_values`, `scroll_area_reset_key_resets_default_runtime_handle` |
| Splitter constraints | `crates/ui_components/src/splitter.rs` | panel normalization, min/max fraction clamping, collapsed fraction, handle adjacency, `resized_by` solver | pointer capture, drag preview, cursor, rendering, live runtime overrides | Behavior owns resize math; adapter owns pointer events and keyboard resize bindings | Behavior owns neutral orientation and panel/handle metadata; adapter owns concrete separator handles and AccessKit relationships | `splitter_state_normalizes_panel_fractions_and_constraints`, `splitter_resize_delta_clamps_to_adjacent_min_max`, `splitter_runtime_fraction_overrides_still_use_resize_constraints`, `splitter_collapsed_panel_uses_collapsed_fraction` |

## Extraction Sequence

1. Move pure helpers with no renderer dependency first: roving focus, listbox navigation helpers,
   scroll viewport intent, and splitter constraints.
2. Move overlay policy after validating the public API shape for stack ordering and focus intent.
   Keep `open_gpui_ui_components::overlay` as the concrete GPUI mapping layer.
3. Keep component builders and render adapters in `open-gpui-ui-components`. They should depend on
   the behavior crate, not move wholesale.
4. Add compatibility re-exports only when they preserve stable imports without making GPUI adapter
   APIs look headless.
5. Do not extract `TextInputController` until a smaller renderer-neutral text model exists.

## Non-Goals

- No new crate in this series.
- No cross-platform adapter implementation in this series.
- No full focus-trap traversal, nested focus scopes, submenu focus arbitration, or cross-window
  overlay runtime in this series.
- No rewrite of component render trees.
- No wholesale copy from Fret, Radix, React Aria, shadcn, DaisyUI, or `gpui-component`.

## Review Gate For The Next Plan

A future crate-creation plan should name the exact modules to move, the compatibility re-exports to
keep, and the tests that must move with each module. It should also prove:

- `open_gpui_ui_core` remains dependency-clean;
- `open_gpui_ui_components::gpui_adapter` remains the only public GPUI helper grouping;
- public component `*State` types still avoid runtime/rendering/callback types;
- AccessKit node wiring remains adapter-owned;
- the new behavior crate has no `open_gpui` dependency.
