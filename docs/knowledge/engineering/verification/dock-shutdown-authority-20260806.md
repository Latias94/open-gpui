---
type: Verification Evidence
title: Dock live-undock shutdown cleanup authority
status: complete
scope: U29
timestamp: 2026-08-06
git_branch: refactor/ui-framework-authority-convergence
git_commit: 7ac29a90
verified_by:
  - cargo nextest run --workspace
  - cargo nextest run -p open-gpui-docking
  - cargo nextest run -p open-gpui
  - cargo fmt --all -- --check
  - git diff --check
  - cargo run -p xtask -- scan-theme-drift
  - cargo run -p xtask -- scan-theme-schema
  - cargo run -p xtask -- scan-ui-contract
  - cargo run -p xtask -- scan-doc-links
  - cargo run -p xtask -- scan-public-api
  - cargo run -p xtask -- scan-import-boundary
  - cargo run -p xtask -- verify-release-docs
---

# Verification

U29 makes pre-commit live-undock cleanup an explicit, aggregate authority. Rehost state,
host presentation leases, retained visual tickets, and native captured-drag transport now produce
typed cleanup evidence and settle through one orphan-cleanup receipt. Shutdown recovery cannot
publish a terminal fact until that receipt exists; if preparation or preflight cannot prove exact
cleanup, the session parks in a fail-closed shutdown-cleanup phase without retrying or fabricating
success.

Deferred live-undock pump work is accepted only while a strong owner-retaining drain permit exists.
An owner disappearing after enqueue therefore cannot silently discard an `AdoptRelease` or leave a
payload drag with no settlement path. Exact provisional reveal tickets also have an atomic
`Cancelled` terminal outcome, and a queued native reveal rechecks its ticket immediately before
dispatch.

# Test Results

- `cargo nextest run --workspace`: 4,328 passed, 4 skipped by platform filters.
- `cargo nextest run -p open-gpui-docking`: 1,254 passed, 0 skipped.
- `cargo nextest run -p open-gpui`: 740 passed, 0 skipped.
- The focused shutdown fallback, dependency-preservation, logical-close, provisional-reveal, and
  retained-presentation retry scenarios are included in the package and workspace totals.
- Formatting and whitespace checks passed.
- Theme drift, theme schema, federated UI contract, public documentation links, public API tiers,
  import boundaries, and release-document inventory scans passed.

# Explicitly Unverified

The real Windows interactive workflow was not run on this development host. In particular, this
evidence does not claim a two-HWND system-pointer drag, same-HWND promotion, renderer-before-HWND
destruction, or ephemeral self-hosted runner isolation. Those remain U28/native-CI evidence gates.

# References

- Plan: `docs/plans/2026-07-10-001-refactor-ui-framework-authority-convergence-plan.md`
- Migration guide: `docs/ui/migration-v0.3.md`
- Breaking-change inventory: `docs/release/breaking-changes.md`
- Dock live-undock reducer: `crates/gpui_docking/src/surface/live_undock.rs`
- Dock live-undock executor: `crates/gpui_docking/src/surface/live_undock_runtime.rs`
- Deferred effect pump: `crates/gpui_docking/src/surface/live_undock_pump.rs`
