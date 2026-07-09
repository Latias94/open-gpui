---
type: Verification Evidence
title: DevTools live runtime workbench verification
timestamp: 2026-07-09
status: passed
related_plan: ../../../plans/2026-07-09-004-feat-devtools-live-runtime-workbench-plan.md
tags:
  - devtools
  - verification
  - nextest
  - gallery
  - docking
---

# Verification

Final focused verification on Windows / PowerShell passed:

- `cargo fmt -p open-gpui-devtools -p open-gpui-ui-foundation-gallery -p open-gpui-docking-native`
- `cargo check -p open-gpui-devtools --no-default-features --tests --locked`
- `cargo check -p open-gpui-devtools --features gpui --tests --locked`
- `cargo check -p open-gpui-devtools --features docking --tests --locked`
- `cargo check -p open-gpui-devtools --all-features --tests --locked`
- `$env:CARGO_BUILD_JOBS = '1'; cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked` passed 83/83.
- `cargo check -p open-gpui-ui-foundation-gallery --tests --locked`
- `cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked` passed 12/12.
- `cargo check -p open-gpui-docking-native --tests --locked`
- `cargo nextest run -p open-gpui-docking-native runtime_status_panel_exports_devtools_dogfood_capture --no-fail-fast --locked` passed 1/1.
- Static builder guard returned no matches: `rg "fn (theme_snapshot|form_snapshot|resource_snapshot|docking_snapshot)" examples/ui-foundation-gallery/src/pages/devtools.rs`.
- `cargo run -p xtask -- scan-doc-links`
- `cargo run -p xtask -- scan-public-api --check`
- `git diff --check`

# Notes

- Broad devtools `nextest` was run with `CARGO_BUILD_JOBS=1` to avoid Windows resource contention.
- `scan-public-api --check` passed without xtask inventory changes, so new public DevTools APIs are accepted by the current tier scan.
- `scan-doc-links` passed after updating `crates/devtools/README.md` and `docs/verification.md`.

# Citations

- [Progress](../progress/2026-07-09-devtools-live-runtime-workbench.md)
- [Plan](../../../plans/2026-07-09-004-feat-devtools-live-runtime-workbench-plan.md)
- [Repository verification matrix](../../../verification.md)
