---
type: Work Progress
title: Open GPUI DevTools real probes and gallery dogfood
status: verified
timestamp: 2026-07-09T00:56:22+08:00
git_branch: feat/devtools-real-probes-dogfood
git_commit: 9885f5d8
related_plan: docs/plans/2026-07-08-004-feat-devtools-real-probes-dogfood-plan.md
tags:
  - ce-work
  - devtools
  - form
  - resource
  - ui-components
  - motion
  - docking
---

# Summary

The real-probe dogfood branch replaces static DevTools gallery fixtures with registry-collected
first-party adapters. DevTools now exposes shared adapter helpers, feature-gated form/resource
adapters, framework fact adapters, and sanitized diagnostics with stable codes.

# Completed Slices

- U1: Added `open_gpui_devtools::adapters` helpers for stable sanitized node ids, payloads,
  redaction summary merging, and sanitized diagnostics. Commit: `aa7c94b0`.
- U2: Added `open_gpui_devtools::form` and `open_gpui_devtools::resource` adapters over public
  form/resource snapshots, plus `PaginatedResourceSnapshotView` re-export. Commit: `660dd985`.
- U4: Reworked the UI foundation gallery DevTools page to collect form/resource probes through
  `DevtoolsRegistry` over deterministic component sample snapshots. Commit: `c664fbd9`.
- U3: Added framework adapters for theme, accessibility evidence, motion frame demand/driver,
  scroll viewport snapshots, and docking runtime status diagnostics. Commit: `9885f5d8`.

# Current Claim

U1-U5 implementation and focused verification are complete on
`feat/devtools-real-probes-dogfood`. Final local `main` merge and `origin/main` push remain after
the U5 commit and final diff review.

# Verification So Far

- `cargo fmt -p open-gpui-devtools -p open-gpui-form -p open-gpui-resource -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery -p open-gpui --check`: passed.
- `cargo check -p open-gpui-devtools --tests --locked`: passed.
- `cargo check -p open-gpui-devtools --features form,resource --tests --locked`: passed.
- `cargo check -p open-gpui-devtools --features gpui,motion,docking --tests --locked`: passed.
- `cargo nextest run -p open-gpui-devtools --features form,resource form_resource_adapters --no-fail-fast --locked`: passed 3/3.
- `cargo nextest run -p open-gpui-devtools --features gpui,motion,docking framework_adapters --no-fail-fast --locked`: passed 5/5.
- `cargo nextest run -p open-gpui-devtools --no-fail-fast --locked`: passed 20/20 after final redaction hardening.
- `cargo check -p open-gpui-ui-foundation-gallery --tests --locked`: passed.
- `cargo nextest run -p open-gpui-ui-foundation-gallery devtools form resource component_sample_contracts --no-fail-fast --locked`: passed 20/20.
- `cargo nextest run -p open-gpui-ui-components form resource public_surface --no-fail-fast --locked`: passed 48/48.
- `cargo run -p xtask -- verify-release-docs`: passed.
- `cargo run -p xtask -- scan-doc-links`: passed.
- `cargo run -p xtask -- scan-import-boundary`: passed.
- `cargo run -p xtask -- scan-theme-drift`: passed.
- `cargo run -p xtask -- scan-theme-schema`: passed.
- `cargo test -p open-gpui-devtools --doc --locked`: passed 3/3.
- `python <engineering-wiki-memory>/scripts/wiki_memory.py validate --root docs\knowledge\engineering`: passed with existing warnings.
- Single-feature checks for `form`, `resource`, `motion`, `docking`, `gpui`, and `ui-components`
  passed.

# Citations

- Plan: `docs/plans/2026-07-08-004-feat-devtools-real-probes-dogfood-plan.md`
- Work registration: `docs/knowledge/engineering/registry/open-gpui-devtools-real-probes-dogfood-codex-root.md`
- Verification: `docs/knowledge/engineering/verification/open-gpui-devtools-real-probes-dogfood-20260709.md`
