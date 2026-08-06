---
type: Verification Evidence
title: DockSurface window session authority and deterministic teardown
status: complete
timestamp: 2026-07-28
git_branch: refactor/ui-framework-authority-convergence
verified_by:
  - cargo nextest run -p open-gpui-docking --test-threads 1 --no-fail-fast
  - cargo nextest run -p open-gpui-devtools --features docking --test-threads 1 --no-fail-fast
  - cargo nextest run -p open-gpui-docking-native --test-threads 1 --no-fail-fast
  - cargo nextest run -p open-gpui --features test-support --test-threads 1 --no-fail-fast
  - cargo check -p open-gpui-docking-minimal -p open-gpui-docking-multiviewport -p open-gpui-docking-native --all-targets
  - cargo check --manifest-path crates/gpui_web/examples/smoke_web/Cargo.toml --target wasm32-unknown-unknown
  - cargo run -p xtask -- scan-public-api --check
  - target/debug/xtask.exe scan-ui-contract
  - target/debug/xtask.exe scan-doc-links
  - target/debug/xtask.exe scan-theme-drift
  - target/debug/xtask.exe scan-theme-schema
  - cargo fmt --all -- --check
  - git diff --check
---

# Verification

- A facade-managed `DockSurface` now owns one generation-bound window session with explicit
  `Vacant`, `Opening`, `Active`, `ShuttingDown`, and `Closed` phases. The viewport runtime remains
  the sole registry for committed, opening, tear-off, and provisional handles.
- A primary opening becomes active only after GPUI commits the exact full `WindowId`. Native close
  during create/map, root construction, initial draw, or hidden initial presentation rolls back
  without leaving a Dock registration. Post-commit presentation failure enters forced shutdown.
- Anchor shutdown freezes the exact generation, retires dependent work, bypasses ordinary child
  close policy, closes dependents before the still-live anchor, and reaches `Closed` only after the
  runtime and every native terminal ticket converge. App shutdown uses an explicit pre-clear path,
  then relies on GPUI's retained native-retirement owners rather than treating registry clearing as
  terminal proof.
- Native owner hints resolve from the exact active anchor. Stale or alien owner tokens fail with a
  typed error, while unsupported backends omit the hint and unmanaged runtimes retain their
  explicit ownership contract.
- Every platform backend must deliver its `FnOnce` terminal callback exactly once. Native close
  ingress completes logical cleanup, tab cleanup, and terminal publication even if an observer
  panics; drop-only Web and Linux headless windows publish the same terminal fact from `Drop`.
- Old drag, prepared delivery, scene publication, categorized update, mutation, activation, and
  close work cannot cross from generation G1 into G2. Two surfaces may use the same logical space
  id while retaining independent anchors, revisions, activation, and teardown.
- Persistent host-bound changes publish one generation-bound `ObservedViewportPlacement` revision;
  transient hit regions and current pointer geometry remain non-persistent route facts.
- `docking-native` uses the facade primary and managed viewport path and no longer calls
  `App::quit` to hide incomplete teardown. The multiview example opens two independent surfaces.
  DevTools exposes phase, generation, anchor, reason, terminal convergence, and runtime state.
  Every target, domain, probe, and reference is namespaced by the exact capture provider identity,
  including provider ids that would otherwise normalize to the same punctuation.

# Test Results

- `open-gpui --features test-support`: 577 passed.
- `open-gpui-docking`: 1,100 passed.
- `open-gpui-devtools --features docking`: 93 passed.
- `open-gpui-docking-native`: 26 passed and one intentionally skipped platform test.
- Focused GPUI opening and close ordering matrix: 11 passed.
- Focused Dock session, owner, lineage, public facade, and DevTools tests all passed before the
  package-level runs.
- Desktop examples and the Web smoke consumer compile against the breaking facade contract; the
  Web smoke was checked for `wasm32-unknown-unknown` rather than a host desktop target.
- The Linux headless cross-check was attempted with
  `cargo check -p open-gpui-linux --no-default-features --target x86_64-unknown-linux-gnu`, but the
  third-party `psm` build script could not find `x86_64-linux-gnu-gcc` on this Windows host. It did
  not reach project compilation and is recorded as an environment limitation, not a passing gate.
- Public API, federated UI contract, documentation link, theme drift, and theme schema scanners
  passed.

# Contract Boundaries

- `DockSurfaceViewports` replaces `DockSurfaceViewportSession` without an alias. Managed viewport
  readiness requires an active primary session and reports typed `SessionInactive` status.
- `host_view` remains an embedded rendering path. It does not infer an anchor or register managed
  route and activation authority. Applications that need a managed native window group use
  `open_primary_window`; advanced application-owned lifetimes use the explicit low-level runtime.
- Selection and focus remain separate transactions. Selecting an inert or hidden panel may commit
  while the exact focus completion reports `Rejected`.
- Native owner/transient relationships are presentation hints, not teardown authority. Dock
  always performs explicit generation-scoped convergence and never owns application exit.

# Deferred Native Evidence

This evidence uses GPUI's deterministic test platform for lifecycle ordering. It does not claim
that a real dragged HWND is visible before release, that Win32 capture transports pointer facts
across windows, or that renderer surfaces retire before native destruction. U27 owns live
provisional transport and presentation; U28 owns real HWND, renderer, capture, z-order, and process
lifetime verification.

# References

- Architecture decision: `docs/adr/0030-open-gpui-dock-surface-window-session-authority.md`
- Change and activation authority: `docs/adr/0028-open-gpui-dock-surface-change-and-activation-authority.md`
- Migration guide: `docs/ui/migration-v0.3.md`
- Public facade: `crates/gpui_docking/src/surface.rs`
- Session authority: `crates/gpui_docking/src/surface/window_session.rs`
- Runtime lineage: `crates/gpui_docking/src/viewport_runtime_lineage.rs`
- DevTools projection: `crates/devtools/src/docking.rs`
