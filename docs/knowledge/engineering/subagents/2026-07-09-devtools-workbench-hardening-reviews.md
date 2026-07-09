---
type: Subagent Finding
title: DevTools workbench hardening review findings
timestamp: 2026-07-09T19:27:23+08:00
git_branch: main
related_plan: ../../../plans/2026-07-09-005-refactor-devtools-workbench-hardening-plan.md
---

# DevTools Workbench Hardening Review Findings

## Finding

Plan review agents converged on four constraints that shaped the implementation:

- Gallery live refresh must use real `GalleryShell` facts, not a deterministic fixture generation.
- Docking-native refresh must be an explicit action/helper outside render; render should only display
  the current inspector controller.
- `DevtoolsEventIdentity` is an event-instance identity. Exact identity preserves selection; a new
  recorder sequence is a new event instance rather than an implicit logical remap.
- R24 should stay minimal: app authors need one stable public-primitives path, not a new tutorial
  framework during this refactor.

## Evidence

- U4 implemented `GalleryDevtoolsWorkbench` with allowlisted shell live facts and refresh status
  selectors.
- U5 implemented `DockingDevtoolsPanel` inside the example, with `refresh_devtools_inspector` as the
  only path that advances the docking DevTools session after construction.
- U2 added identity-first selection regressions and selector sanitization coverage.
- U3 updated the README and release docs with the breaking identity-first migration path.

## Recommendation

Keep future DevTools ecosystem work on the same ownership boundary:
runtime crates publish public read-only facts, app examples own live session wiring, and
`open_gpui_devtools` sanitizes, stores bounded frames, diffs, and renders inspectors.

## Disposition

Applied during implementation. A later U5-only review agent was interrupted after timing out; no
additional findings were received before the U5 commit, and targeted/full docking-native tests were
green.

## Citations

- [Plan](../../../plans/2026-07-09-005-refactor-devtools-workbench-hardening-plan.md)
- [Progress](../progress/2026-07-09-devtools-workbench-hardening.md)
