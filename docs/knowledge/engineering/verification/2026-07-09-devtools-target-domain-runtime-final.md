---
type: "Verification Evidence"
title: "DevTools target-domain runtime final"
timestamp: 2026-07-09T05:04:23Z
status: "passed"
related_plan: "docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md"
git_branch: "main"
git_commit: "a0e1cb6b"
verified_by: "cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked"
---

# Summary

Completed U7 and the final verification pass for the DevTools target/domain/runtime plan.

# Verified State

- Gallery `devtools_gallery_state()` now derives from `devtools_gallery_capture()`.
- `devtools_gallery_capture()` projects legacy registry snapshots into app/probe targets, command/layout/timeline/data domains, diagnostics, and a bounded local event row.
- `devtools_gallery_collection()` remains the legacy snapshot view by returning `capture.snapshot_collection()`.
- DevTools README, root README, and verification docs describe target/domain/event capture, legacy snapshot compatibility, local read-only boundaries, and the absence of a remote debugging protocol.
- Source search and gallery tests guard against reintroducing static DevTools demo snapshot builders.

# Commands

- `cargo fmt -p open-gpui-devtools -p open-gpui-ui-foundation-gallery`
- `cargo check -p open-gpui-ui-foundation-gallery --tests --locked`
- `cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked`
- `cargo check -p open-gpui-devtools --all-features --tests --locked`
- `CARGO_BUILD_JOBS=1 cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked`
- `cargo run -p xtask -- scan-doc-links`
- `cargo run -p xtask -- scan-public-api --check`
- `rg "fn (theme_snapshot|form_snapshot|resource_snapshot|docking_snapshot)" examples/ui-foundation-gallery/src/pages/devtools.rs`
- `git diff --check`

# Result

All verification gates passed locally. The static-builder source search returned no matches. The DevTools all-features nextest gate passed 53 tests with 53 passing when run with `CARGO_BUILD_JOBS=1` to avoid Windows linker out-of-memory.

# Citations

- `docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md`
- `examples/ui-foundation-gallery/src/pages/devtools.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery/devtools_contracts.rs`
- `crates/devtools/README.md`
- `docs/verification.md`
