---
type: Verification Evidence
title: DevTools headless artifact pipeline verification
timestamp: 2026-07-10T00:38:52+08:00
git_branch: main
related_plan: ../../../plans/2026-07-09-006-feat-devtools-headless-artifact-pipeline-plan.md
verified_by: full local Verification Contract
tags:
  - devtools
  - artifact-pipeline
  - verification
---

# Verification

Final local verification for `docs/plans/2026-07-09-006-feat-devtools-headless-artifact-pipeline-plan.md`
passed on Windows PowerShell.

## Passed Gates

- `cargo fmt -p open-gpui-devtools -p open-gpui-ui-foundation-gallery -p open-gpui-docking-native -p xtask --check`
- `cargo check -p open-gpui-devtools --tests --locked`
- `cargo check -p open-gpui-devtools --all-features --tests --locked`
- `cargo nextest run -p open-gpui-devtools --all-features --test report_contracts --no-fail-fast --locked` passed 9/9 after adding the Gallery fixture noise regression.
- `cargo nextest run -p open-gpui-devtools --all-features --test artifact_contracts --test report_contracts --test docking_runtime_contracts --no-fail-fast --locked` passed 17/17.
- `cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked` passed 101/101.
- `cargo check -p xtask --locked`
- `cargo test -p xtask public_api_snapshot --locked` passed 6/6 targeted public API scanner tests.
- `cargo nextest run -p xtask --test devtools_cli_contracts --no-fail-fast --locked` passed 8/8.
- CLI smoke passed for `devtools --help`, `report`, `diagnose`, `diff`, `stream`, `query`, `assert`, and `follow` over checked-in fixtures.
- `cargo check -p open-gpui-ui-foundation-gallery --all-targets --locked`
- `cargo nextest run -p open-gpui-ui-foundation-gallery devtools --no-fail-fast --locked` passed 16/16 selected tests.
- `cargo check -p open-gpui-docking-native --all-targets --locked`
- `cargo nextest run -p open-gpui-docking-native devtools --no-fail-fast --locked` passed 4/4 selected tests.
- `cargo nextest run -p open-gpui-docking-native docking_native_headless --no-fail-fast --locked` passed 2/2.
- `cargo run -p xtask -- scan-public-api --check`
- `cargo run -p xtask -- scan-doc-links`
- `cargo run -p xtask -- verify-release-docs`
- `git diff --check`

## Notes

- The initial U7 run of `cargo nextest run -p xtask --test devtools_cli_contracts --no-fail-fast --locked`
  found one stale expected count: `simple-capture.json` now produces both the original domain
  diagnostic and the new layout invalid-bounds report rule. The contract was updated to expect two
  warning findings, and the rerun passed.
- The Gallery report fixture was checked and locked to the known single
  `capture-diagnostic.runtime.unavailable` warning so the first layout/motion/command/form/resource
  rules do not make the Gallery fixture noisy.
- `xtask/src/devtools/watch.rs` changed only because `cargo fmt -p xtask` normalized a nested
  `match` expression.

## Citations

- [Progress note](../progress/2026-07-10-devtools-headless-artifact-pipeline.md)
- [Plan](../../../plans/2026-07-09-006-feat-devtools-headless-artifact-pipeline-plan.md)
- Commits before this note: `ff7c7f22`, `5325f3f0`, `854b947d`, `5555946f`, `7bbce638`,
  `2cb1df4e`, `5c9bb343`.
