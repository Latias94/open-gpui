---
type: Work Progress
title: DevTools workbench hardening
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

# DevTools Workbench Hardening

## Summary

The DevTools workbench hardening plan is implemented through U5 on `main`.
The core direction is now identity-first, session-backed, and app-owned:
DevTools captures and diffs remain sanitized and bounded, while Gallery and docking-native own their
local workbench wiring instead of pushing runtime authority into DevTools or docking crates.

## Completed Units

- U1 split the GPUI DevTools feature module into ownership-based `gpui/mod.rs`, `runtime.rs`,
  `inspector.rs`, and `render.rs`.
- U2 removed the public sequence-only event selection path. Event row selection and selectors now use
  sanitized `DevtoolsEventIdentity::as_key()` values.
- U3 documented the identity-first breaking change and minimal app-author integration path in the
  DevTools docs and release notes.
- U4 moved Gallery DevTools to a `GalleryShell`-owned live workbench with bounded frame history,
  explicit refresh controls, shell live facts, diffs, and selection retention status.
- U5 embedded a real `DevtoolsInspectorController` in `examples/docking-native`, backed by a local
  `DevtoolsSession` over public `DockViewportRuntimeStatus` facts and an explicit refresh action.

## Current State

- Latest pushed commit for this slice: `a2de7c26 feat(docking-native): embed devtools inspector`.
- `crates/gpui_docking` does not depend on `open_gpui_devtools`; docking-native remains the
  integration owner.
- Historical memory that mentioned `devtools-inspector:event:0` is marked superseded because
  sequence-only event selectors are obsolete.

## Citations

- [Plan](../../../plans/2026-07-09-005-refactor-devtools-workbench-hardening-plan.md)
- [DevTools README](../../../../crates/devtools/README.md)
- [Gallery DevTools page](../../../../examples/ui-foundation-gallery/src/pages/devtools.rs)
- [Docking-native example](../../../../examples/docking-native/src/main.rs)
