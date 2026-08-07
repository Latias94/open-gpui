---
type: Verification Evidence
title: Dock provisional window runtime authority and shutdown retirement
status: complete
scope: U28-U29
timestamp: 2026-08-06
git_branch: refactor/ui-framework-authority-convergence
git_commit: 2e611636
verified_by:
  - cargo nextest run -p open-gpui-docking
  - cargo fmt --all -- --check
  - git diff --check
  - cargo run -p xtask -- scan-public-api
  - cargo run -p xtask -- scan-import-boundary
---

# Verification

The viewport Runtime now registers each live-undock provisional window during its builder
transaction, before the reducer decides whether the returned window is admitted, retired, owned by
an already-frozen shutdown, or stale. The reducer carries that exact generation-bound completion
instead of a Boolean registration claim. Frozen admission no longer creates an unowned native
window or closes it before the opening return has transferred terminal authority.

WindowSession terminal ownership is independent from presentation binding validity. A late
shutdown window first receives an exact native-terminal ticket; an invalid or unavailable binding
can prevent dependency transfer without bypassing native retirement. If WindowSession cannot
accept the ticket, Runtime reclaims the exact shutdown-owned generation and issues its ordinary
retirement effect. Dynamic late adoption and the parallel fallback window registry were removed.

Surface shutdown copies the immutable Runtime-issued reservation handles before entering the
commit and publication transaction. A panic before Runtime commit, after Runtime commit, after the
surface commit sink returns, or in the capture-failure retirement path therefore cannot discard the
dependent and anchor handles. Required close effects still run dependent-first, terminal tickets
settle, Runtime ownership becomes empty, and the session reaches `Closed` before the first panic is
propagated or attached to the typed capture failure.

# Test Results

- `open-gpui-docking`: 1,270 passed, 0 skipped.
- Builder-time freeze tests cover admission frozen before registration, synchronous initial close,
  open abort, close-observer panic, stale reducer return, and exact Runtime record removal.
- Shutdown tests cover invalid presentation binding, a stale open return after reducer retirement,
  held native terminals, Runtime commit panic, surface commit publication panic through the
  production scheduling entry, and the parallel capture-failure retirement path.
- Formatting, whitespace, public API tier, and import-boundary checks passed.
- Independent state-machine reviews found no remaining reproducible P1 or P2 lifecycle failure in
  this slice. The completion token remains `Copy`; exact generation checks currently enforce its
  single effective settlement, while a future deep-session API may make that linearity structural.

# Explicitly Unverified

The real Windows interactive workflow was not run on this development host. This evidence does not
claim a real two-HWND captured drag, provisional visibility before physical button release,
same-HWND promotion, renderer-before-HWND destruction, z-order correctness, or self-hosted runner
isolation. Those remain native U28/CI gates.

# References

- Plan: `docs/plans/2026-07-10-001-refactor-ui-framework-authority-convergence-plan.md`
- Window session ADR: `docs/adr/0030-open-gpui-dock-surface-window-session-authority.md`
- Dock surface orchestration: `crates/gpui_docking/src/surface.rs`
- Live-undock reducer: `crates/gpui_docking/src/surface/live_undock.rs`
- Viewport Runtime facade: `crates/gpui_docking/src/viewport_runtime_handle.rs`
- Runtime window ownership: `crates/gpui_docking/src/viewport_window_ownership.rs`
