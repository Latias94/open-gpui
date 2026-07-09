---
type: Verification Evidence
title: DevTools workbench hardening verification
timestamp: 2026-07-09T19:27:23+08:00
git_branch: main
related_plan: ../../../plans/2026-07-09-005-refactor-devtools-workbench-hardening-plan.md
git_commits:
  - c70feea1
  - 8cf0ce74
  - 8d24f25e
  - b6dd6285
  - fc8440ba
  - a2de7c26
---

# DevTools Workbench Hardening Verification

## Verified Behavior

- DevTools core still compiles without optional features.
- The GPUI feature module compiles after the ownership-based split.
- Event selection is identity-first; cross-scope same-sequence events, new recorder sequences, and
  sanitized selector fragments are covered.
- Gallery owns a live DevTools workbench and refreshes it from allowlisted shell facts.
- Docking-native embeds a real GPUI inspector backed by a bounded `DevtoolsSession` over public
  `DockViewportRuntimeStatus` facts.
- The docking runtime crate does not depend on `open_gpui_devtools`; the example owns integration.
- Release docs, doc links, public API tier scan, and DevTools doctests passed after the changes.

## Commands

Passed:

```sh
cargo fmt -p open-gpui-devtools -p open-gpui-ui-foundation-gallery -p open-gpui-docking-native
cargo check -p open-gpui-devtools --no-default-features --tests --locked
cargo check -p open-gpui-devtools --features gpui --tests --locked
cargo nextest run -p open-gpui-devtools --all-features --test inspector_contracts --test diff_contracts --test session_contracts --test framework_adapters --no-fail-fast --locked
cargo check -p open-gpui-ui-foundation-gallery --all-targets --locked
cargo nextest run -p open-gpui-ui-foundation-gallery devtools_gallery --no-fail-fast --locked
cargo check -p open-gpui-docking-native --all-targets --locked
cargo nextest run -p open-gpui-docking-native --no-fail-fast --locked
cargo run -p xtask -- verify-release-docs
cargo run -p xtask -- scan-doc-links
cargo run -p xtask -- scan-public-api --check
cargo test -p open-gpui-devtools --doc --all-features --locked
```

## Source Guards

Passed:

```sh
rg -n "select_event\\(" crates/devtools examples/ui-foundation-gallery examples/docking-native
rg -n "devtools-inspector:event:0|devtools-inspector:event:\\{sequence\\}" crates/devtools examples/ui-foundation-gallery examples/docking-native
rg -n "open_gpui_devtools" crates/gpui_docking -g "*.rs" -g "*.toml"
```

The first two guards returned no sequence-only selection or selector usages in source/tests. The
dependency guard returned no `open_gpui_devtools` usage inside `crates/gpui_docking`.

## Citations

- [Plan](../../../plans/2026-07-09-005-refactor-devtools-workbench-hardening-plan.md)
- [Progress](../progress/2026-07-09-devtools-workbench-hardening.md)
- [Superseded selector memory](2026-07-09-devtools-inspector-click-dogfood.md)
