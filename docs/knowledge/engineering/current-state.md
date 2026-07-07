---
type: Current State
title: Open GPUI post-v0.2.0 stabilization state
status: active
timestamp: 2026-07-07T17:52:09+08:00
git_branch: refactor/post-v020-stabilization
related_plan:
  - ../../plans/2026-07-07-002-refactor-post-v020-stabilization-plan.md
related_adr:
  - ../../adr/0012-docking-runtime-capability-alignment.md
  - ../../adr/0015-ui-motion-runtime-foundation.md
  - ../../adr/0016-ui-motion-spring-foundation.md
verified_by:
  - cargo run -p xtask -- verify
  - cargo check -p open-gpui-docking-minimal --locked
  - cargo run -p xtask -- dependency-health
---

# Current State

- Snapshot timestamp: 2026-07-07T17:52:09+08:00.
- Goal: finish `docs/plans/2026-07-07-002-refactor-post-v020-stabilization-plan.md` for the next pre-1.0 breaking stabilization release after v0.2.0.
- Branch: `refactor/post-v020-stabilization`.
- Last fully verified state: after final review fixes, `cargo run -p xtask -- verify` passed on `refactor/post-v020-stabilization`.
- Current work: final review fixes are locally implemented. `examples/docking-minimal` is the common docking example; `examples/docking-native` remains the viewport-runtime dogfood surface.
- Next action: commit the final review fixes, then merge/push the completed stabilization branch when ready.
- Blocked: none.

# Integrated Summary

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

- [Post-v0.2.0 stabilization plan](../../plans/2026-07-07-002-refactor-post-v020-stabilization-plan.md)
- [Verification guide](../../verification.md)
- [Breaking change inventory](../../release/breaking-changes.md)
- [Changelog](../../../CHANGELOG.md)
- [Docking runtime capability ADR](../../adr/0012-docking-runtime-capability-alignment.md)
- [Motion runtime ADR](../../adr/0015-ui-motion-runtime-foundation.md)
- [Motion spring ADR](../../adr/0016-ui-motion-spring-foundation.md)
