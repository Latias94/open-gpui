---
type: Current State
title: Open GPUI v0.3.0 runtime and docking hardening state
status: active
timestamp: 2026-07-08T17:38:34+08:00
git_branch: main
related_plan:
  - ../../plans/2026-07-08-003-refactor-runtime-docking-core-hardening-plan.md
  - ../../plans/2026-07-07-002-refactor-post-v020-stabilization-plan.md
related_adr:
  - ../../adr/0012-docking-runtime-capability-alignment.md
  - ../../adr/0015-ui-motion-runtime-foundation.md
  - ../../adr/0016-ui-motion-spring-foundation.md
verified_by:
  - CARGO_TARGET_DIR=/tmp/open-gpui-u1-check cargo check -p open-gpui-windows --all-features --locked
  - cargo test -p xtask public_api_snapshot --locked
  - cargo run -p xtask -- scan-public-api --check
  - cargo check -p open-gpui-web --target wasm32-unknown-unknown --tests --locked -j 1
  - cargo check -p open-gpui-docking --tests --locked
  - cargo fmt --all --check
  - git diff --check
---

# Current State

- Snapshot timestamp: 2026-07-08T17:38:34+08:00.
- Goal: finish `docs/plans/2026-07-08-003-refactor-runtime-docking-core-hardening-plan.md` for the next v0.3.0-oriented runtime and docking hardening slice.
- Branch: `main`.
- Last fully verified state: `docs/plans/2026-07-07-002-refactor-post-v020-stabilization-plan.md` passed `cargo run -p xtask -- verify` before this follow-up slice.
- Current work: U1-U6 of the runtime/docking/core hardening plan are implemented and committed locally; U7 records focused verification and local runner limitations.
- Next action: let CI own full-workspace confirmation, then continue with v0.3.0 user-facing API work only when it is intentionally planned.
- Blocked: full local `open-gpui` package checks, docking nextest runs, and repeated `xtask` scans can stall in build-script or test-runner startup on this workstation after prior cargo work has completed.

# Integrated Summary

- Done: Windows runtime recovery paths fail closed instead of panicking. Device-lost recovery errors are logged and retried, renderer refresh failures re-mark device invalidation, and optional clipboard metadata/image formats are skipped when custom format registration is unavailable.
- Done: web dispatcher startup state has focused unit coverage, and the public API scanner rejects forbidden file-backed and inline public module leaks.
- Done: docking routed-preview query/update/replace/clear behavior lives in `crates/gpui_docking/src/viewport_runtime/routed_preview.rs`; `DockViewportRuntime` remains the state owner.
- Done: docking crate docs keep common panel open/close/reopen paths facade-first through `DockSurface`, with controller/workspace access documented as low-level integration.
- Done: core GPUI internals are thinner. `AppCell`, `AppRef`, and `AppRefMut` live in `app/cell.rs`; `WindowInvalidator` lives in `window/invalidator.rs`; `Div` scroll and tooltip interactivity live in focused child modules.
- Done: docking public exports are tiered. Common app APIs remain in the crate root and `prelude`; transition/runtime diagnostics moved to `open_gpui_docking::advanced`.
- Done: motion frame ownership is adapter-facing. `MotionFrameHost::reset` now requires an explicit `MotionFrameHostResetReason`, and first-party consumers share frame-demand and reduced-motion policy ownership.
- Done: `VirtualizedList` is split into descriptor, model, render-plan, runtime, render, style, and motion modules while preserving key-first public semantics.
- Done: `VirtualizedList` now has explicit async/infinite status rows, keyed prepend reveal, sticky overlay metadata, and theme-backed colors.
- Done: web verification has a stable browser smoke for app readiness, canvas initialization, focus/input, single-window shell interaction, and explicit unsupported platform viewport capability.
- Done: release automation now verifies release notes, public docs links, README versions, crate README metadata, and breaking-change inventory before publishing, and the release workflow can create or update GitHub Release notes.
- Done: workspace MSRV is Rust 1.92, enforced through `xtask dependency-health`, a dedicated CI workflow, cargo-audit, and duplicate dependency allowlisting.
- Done: public README and crate README entry points now describe supported behavior, non-goals, and focused verification commands without depending on historical plans.

# Current Entry Points

- Framework startup: `README.md`, `crates/gpui/README.md`, and `crates/gpui_platform/README.md`.
- Component library: `crates/ui_components/README.md` and `cargo run -p open-gpui-ui-foundation-gallery`.
- Motion foundation: `crates/motion/README.md`.
- Docking common API: `crates/gpui_docking/README.md` and `cargo run -p open-gpui-docking-minimal`.
- Docking viewport/runtime dogfood: `cargo run -p open-gpui-docking-native`.
- Web backend: `crates/gpui_web/README.md` and `cargo run -p xtask -- web-smoke`.
- Verification matrix: `docs/verification.md`.
- Breaking public API inventory: `docs/release/breaking-changes.md` and the Unreleased section of `CHANGELOG.md`.

# Historical Navigation

Older command, component, docking, motion, and native UI framework research progress remains available through `index.md`, `progress/`, `verification/`, `sessions/`, `subagents/`, and ADR links. Treat those files as historical evidence unless the current plan, README, changelog, workflow, or crate source confirms the same state.

# Citations

- [Runtime/docking/core hardening plan](../../plans/2026-07-08-003-refactor-runtime-docking-core-hardening-plan.md)
- [Post-v0.2.0 stabilization plan](../../plans/2026-07-07-002-refactor-post-v020-stabilization-plan.md)
- [Verification guide](../../verification.md)
- [Breaking change inventory](../../release/breaking-changes.md)
- [Changelog](../../../CHANGELOG.md)
- [Docking runtime capability ADR](../../adr/0012-docking-runtime-capability-alignment.md)
- [Motion runtime ADR](../../adr/0015-ui-motion-runtime-foundation.md)
- [Motion spring ADR](../../adr/0016-ui-motion-spring-foundation.md)
