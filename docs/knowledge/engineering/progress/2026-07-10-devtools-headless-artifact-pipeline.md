---
type: Work Progress
title: DevTools headless artifact pipeline
timestamp: 2026-07-10T00:38:52+08:00
git_branch: main
related_plan: ../../../plans/2026-07-09-006-feat-devtools-headless-artifact-pipeline-plan.md
git_commits:
  - ff7c7f22
  - 5325f3f0
  - 854b947d
  - 5555946f
  - 7bbce638
  - 2cb1df4e
  - 5c9bb343
tags:
  - devtools
  - artifact-pipeline
  - ce-work
---

# DevTools Headless Artifact Pipeline

## Summary

The DevTools headless artifact pipeline plan is implemented on `main` through U7.
The product shape is now local, read-only, artifact-first, and app-owned:
applications decide when to refresh runtime facts, DevTools serializes sanitized capture/session/report
records, and `xtask devtools` consumes those records through query/assert/follow workflows.

## Completed Units

- U1 added `open-gpui-devtools-artifact-record/v1`, renderer-neutral artifact metadata, file/JSONL
  sinks, atomic latest-file replacement, and writer contract tests.
- U2 split `xtask devtools` into artifact loading, command dispatch, render, query, and watch/follow
  modules.
- U3 added `query`, `assert`, `follow`, stdin/stdout, bounded wait, JSON/Markdown output, and
  fixture-backed CLI contract tests.
- U4 added Gallery headless session/report artifact helpers and deterministic Gallery fixtures.
- U5 added docking-native headless artifacts and strengthened docking runtime findings for explicit
  public capability and route-fact diagnostics.
- U6 added report-rule findings for layout bounds/scroll anomalies, timeline order regressions,
  terminal motion frame demand, command keybinding diagnostics, form validation/submission state,
  and resource error/retry facts.
- U7 documented the pipeline in README/ADR/verification/changelog and added a devtools root export
  allowlist to `scan-public-api`.

## Current State

- Latest implementation/documentation commit before this memory note: `5c9bb343`.
- Public artifact writer types are root-exported intentionally and guarded by `cargo run -p xtask -- scan-public-api --check`.
- Report-rule internals remain private; findings are emitted through `DevtoolsReport`.
- Fixture artifacts live under `crates/devtools/tests/fixtures/` and are sufficient for CLI tests
  without launching Gallery or docking-native.
- No remote transport, CDP clone, runtime mutation API, screenshot baseline store, or persistent
  trace database was introduced.

## Citations

- [Plan](../../../plans/2026-07-09-006-feat-devtools-headless-artifact-pipeline-plan.md)
- [ADR 0020](../../../adr/0020-open-gpui-devtools-artifact-pipeline.md)
- [DevTools README](../../../../crates/devtools/README.md)
- [Verification guide](../../../verification.md)
