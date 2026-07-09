---
type: "Verification Evidence"
title: "DevTools target-domain runtime MVP checkpoint"
timestamp: 2026-07-09T04:20:00Z
status: "passed"
related_plan: "docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md"
git_branch: "main"
verified_by: "cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked"
---

# Summary

The post-U3 MVP checkpoint passed before starting U4.

# Verified State

- `DevtoolsInspectorState::from_capture()` consumes `DevtoolsCapture`.
- Inspector state exposes deterministic target, domain, and event rows.
- Filtering by a real producer label moves selection to the matching target/domain/event.
- Legacy snapshot rows and selected probe behavior remain available.

# Commands

- `cargo check -p open-gpui-devtools --no-default-features --tests --locked`
- `cargo nextest run -p open-gpui-devtools --test inspector_contracts --test target_domain_contracts --test event_recorder_contracts --no-fail-fast --locked`
- `cargo check -p open-gpui-devtools --all-features --tests --locked`
- `cargo nextest run -p open-gpui-devtools --all-features --no-fail-fast --locked`

# Result

All listed gates passed locally. The checkpoint demonstrates that legacy projection, a first-party timeline domain, ordered sanitized events, and inspector state rows compose in one testable slice.

# Next Action

Start U4 docking target/domain capture while keeping the legacy docking snapshot wrapper stable.
