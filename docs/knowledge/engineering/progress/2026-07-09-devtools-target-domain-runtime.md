---
type: "Progress"
title: "DevTools target-domain runtime"
timestamp: 2026-07-09T05:04:23Z
status: "complete"
related_plan: "docs/plans/2026-07-09-002-refactor-devtools-target-domain-runtime-plan.md"
git_branch: "main"
git_commit: "a0e1cb6b"
---

# Summary

Implemented the DevTools target/domain/runtime refactor plan through U7.

# Shipped

- Target/domain/event capture core and `DevtoolsRegistry::collect_capture()`.
- Bounded event recorder and timeline projection from event batches.
- Inspector state target/domain/event rows, explicit selections, selected-detail priority, copy/export labels, and GPUI debug selectors.
- Docking runtime capture from public runtime status records while preserving the legacy docking snapshot wrapper.
- First-party command, form, resource, layout, and timeline capture helpers while preserving legacy snapshot APIs.
- Gallery dogfood through `devtools_gallery_capture()` with legacy `devtools_gallery_collection()` compatibility.
- DevTools docs and focused verification guidance for capture-based integrations.

# Verification

See `docs/knowledge/engineering/verification/2026-07-09-devtools-target-domain-runtime-final.md`.
