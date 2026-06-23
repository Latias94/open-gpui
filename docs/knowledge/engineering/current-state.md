---
type: "Current State"
title: "Current Engineering State"
description: "Short durable summary of the active engineering state."
tags: ["engineering-memory"]
timestamp: 2026-06-19T00:00:00Z
status: "active"
---

# Current State

- Goal: Continue docking + multiviewport hardening against `repo-ref/imgui` docking branch, especially backend authority, focus/activation, tear-off, and thin routing shims.
- Branch: `refactor/docking-viewport-authority-focus`
- Last verified: `cargo fmt --all`, `cargo check -p open-gpui-docking --lib`, and `cargo nextest run -p open-gpui-docking viewport_panel_request_selects_hidden_tab_before_restoring_focus viewport_activation_restores_recorded_last_focused_panel viewport_activation_clears_failed_panel_focus_request platform_activation_does_not_restore_panel_focus_while_mouse_is_pressed close_recovery_does_not_reveal_hidden_recorded_panel --no-fail-fast`.
- Done: Moved `global_screen_viewport_hits` onto `DockViewportAdapter`; deleted `crates/gpui_docking/src/viewport_target.rs`; removed the `mod viewport_target` entry from `lib.rs`; switched call sites in viewport, placement, host tests, and drop routing to the adapter method. Renamed the route authority formerly called `SingleGeometryHit` to `UniqueGeometryHit`, preserving ImGui-like unique-geometry fallback while keeping `SourceOnly` cross-viewport unique geometry fail-closed. Updated crate docs to remove stale `active-window` arbitration wording. Renamed runtime viewport `last_focused` ordering to `recent_focus` so it reads as diagnostic/recovery ordering, not route authority.
- In progress: Evaluating the next ImGui-aligned refactor: continue against drop route / preview / commit seams after the focus/activation cleanup. The current focus path no longer uses the host-layer `select_tab_and_request_focus()` shim; viewport activation now reveals hidden tabs by selecting them and then requesting focus through the live panel registration path. The remaining high-value seam is still the drop route pipeline, especially whether preview and commit can be collapsed into a single accepted route token.
- Done: Collapsed tear-off source invalidation from `Missing/Moved` into a single `SourceUnavailable` path, so runtime no longer reclassifies source-state twice before cancelling a pending tear-off.
- Done: Collapsed duplicated runtime viewport registration post-processing into `DockViewportRuntime::register_runtime_viewport`, so regular viewport open and tear-off completion share one path for owned-window tracking, retired-window revival, recent-focus stamping, replaced-window cleanup, and close-plan discard.
- Done: Removed the shallow `DockController::select_tab_and_request_focus()` helper and the host-side fallback branch that tried to special-case `ViewportActivation`; the host now resolves visible-panel focus directly, reveals hidden tabs explicitly, and then requests focus through the registered panel view.
- Verification: `cargo fmt --all`, `cargo check -p open-gpui-docking --lib`, and `cargo nextest run -p open-gpui-docking viewport_panel_request_selects_hidden_tab_before_restoring_focus viewport_activation_restores_recorded_last_focused_panel viewport_activation_clears_failed_panel_focus_request platform_activation_does_not_restore_panel_focus_while_mouse_is_pressed close_recovery_does_not_reveal_hidden_recorded_panel --no-fail-fast` passed (5 tests, 599 skipped).
- Verification: `CARGO_TARGET_DIR=target-ce-work cargo check -p open-gpui-docking --lib` and `CARGO_TARGET_DIR=target-ce-work cargo nextest run -p open-gpui-docking viewport_runtime_handle_closes_unregistered_window_when_tear_off_source_closes viewport_runtime_handle_closes_unregistered_window_when_tear_off_source_moves --no-fail-fast` passed.
- Blocked: None.
- Next action: Continue comparing the drop route seam against ImGui's `MouseViewport` / dock preview + commit pipeline, especially whether `viewport_drop_route.rs`, `viewport_target_resolver.rs`, and `viewport_drop_authority.rs` can collapse route selection, preview target, and authorized delivery into one registry/graph-produced route plan.

# Citations
- [Engineering log](log.md)
