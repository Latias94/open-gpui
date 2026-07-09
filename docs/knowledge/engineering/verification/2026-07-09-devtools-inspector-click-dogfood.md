---
type: Verification Evidence
title: DevTools inspector click dogfood
timestamp: 2026-07-09T16:05:45+08:00
git_branch: main
related_plan: ../../../plans/2026-07-09-003-refactor-devtools-runtime-ecosystem-plan.md
---

# DevTools Inspector Click Dogfood

## Summary

Added a gallery-level smoke test for the stateful `DevtoolsInspectorController` interaction path.
The test opens the UI foundation gallery DevTools page, clicks rendered debug selectors for legacy
snapshot rows, target rows, domain rows, event rows, copy detail, and export capture, then reads the
controller entity state to verify the click handlers updated selection and feedback.

This closes the previous follow-up risk that GPUI inspector click handlers existed but were not
covered by a harness-level click test.

## Superseded Selector Note

The original event-row selector in this memory was sequence-only. It is superseded by the
identity-first DevTools workbench hardening in
[2026-07-09-devtools-workbench-hardening-verification](2026-07-09-devtools-workbench-hardening-verification.md).
Event rows now use sanitized `DevtoolsEventIdentity::as_key()` values in selectors; do not copy the
obsolete sequence-only event-row selector pattern into new tests.

## Verified Behavior

- The gallery renders the stateful DevTools inspector controller at
  `devtools-inspector:gallery-devtools-inspector:root`.
- Clicking `devtools-inspector:row:resource` selects the `resource` legacy snapshot and reports
  `Selected snapshot resource`.
- Clicking `devtools-inspector:copy-detail` writes through the controller action path and reports
  `Selected detail JSON copied`.
- Clicking `devtools-inspector:target:probe.form` selects the form target and switches the active
  detail kind to domain detail.
- Clicking the rendered form domain row selects its domain and reports `Selected domain ...`.
- At the time of this historical test, clicking a sequence-only event-row selector selected event
  sequence `0`; that selector shape is now obsolete because event selection is identity-first.
- Clicking `devtools-inspector:export-capture` reports `DevTools capture JSON exported`.

## Verification

Passed:

```sh
cargo nextest run -p open-gpui-ui-foundation-gallery devtools_gallery_smoke_clicks_inspector_rows_and_actions --no-fail-fast --locked
```

## Citations

- [DevTools runtime ecosystem plan](../../../plans/2026-07-09-003-refactor-devtools-runtime-ecosystem-plan.md)
- [Previous provider/controller verification](2026-07-09-devtools-runtime-ecosystem-provider-controller.md)
- [Gallery DevTools contracts](../../../../examples/ui-foundation-gallery/tests/foundation_gallery/devtools_contracts.rs)
