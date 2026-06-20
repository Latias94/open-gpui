---
title: "Open GPUI UI Gallery State Contract Alignment Plan"
type: refactor
date: 2026-06-18
execution: code
depends_on:
  - docs/adr/0005-open-gpui-official-component-architecture.md
  - docs/adr/0007-open-gpui-ui-headless-boundary-design.md
  - docs/adr/0008-open-gpui-ui-component-productization-roadmap.md
  - docs/plans/2026-06-18-001-refactor-ui-component-contract-alignment-plan.md
  - docs/plans/2026-06-18-002-refactor-ui-gallery-catalog-metadata-plan.md
---

# Open GPUI UI Gallery State Contract Alignment Plan

## Summary

Continue productizing the current UI crates by making the gallery consume official resolved state as
the behavior contract. The gallery sample layer may keep display-only content, but it should not
carry a second copy of size, orientation, selection, disabled, open, query, or loading state when a
component `*State` already exposes those facts.

## Problem Frame

The Components gallery is the conformance and dogfood surface for the UI component ecosystem. It
currently mixes two responsibilities:

- display fixtures such as panel copy, row labels, icon glyphs, and sample titles;
- behavior contract facts that already exist in resolved state.

That makes some modules shallow: deleting a duplicated sample field should not change behavior, but
the shell can still accidentally read the sample copy. The next alignment pass should make the
resolved state interface the test surface and keep sample structs focused on display-only material.

## Requirements

- R1. Shell rendering for interactive and layout components must read size, orientation,
  selected/focused values, disabled state, placeholder/query text, dialog state, loading state, and
  open policy from resolved state when available.
- R2. Sample structs should only retain display fixtures that cannot be reconstructed from resolved
  state without losing gallery-specific copy or icons.
- R3. Any retained helper must provide real locality or leverage. A helper that only renames a
  single field should be deleted or folded into the caller.
- R4. Tests must catch regressions where catalog selectors, sample factories, and rendered debug
  selectors drift.
- R5. No standalone headless crate is introduced in this pass.

## Implementation Units

### U1. Restore and Lock the Build Baseline

Files:
- `examples/ui-foundation-gallery/src/pages/components.rs`
- `examples/ui-foundation-gallery/src/shell.rs`

Goal:
- Recover from any partial gallery state-contract refactor and ensure the gallery compiles before
  further deletions.

Verification:
- `cargo check -p open-gpui-ui-foundation-gallery --tests`

### U2. Delete Behavior Duplicates From Layout Samples

Files:
- `examples/ui-foundation-gallery/src/pages/components.rs`
- `examples/ui-foundation-gallery/src/shell.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs`

Goal:
- Remove layout sample fields whose values are already exposed by resolved state, starting with
  `SplitterSample.orientation` and `SplitterSample.size`.

Test scenarios:
- Splitter shell rendering uses `SplitterState::orientation()` and `SplitterState::size()`.
- Splitter sample metadata tests assert the state contract owns those values.

### U3. Audit Choice and Command Collections

Files:
- `examples/ui-foundation-gallery/src/pages/components.rs`
- `examples/ui-foundation-gallery/src/shell.rs`

Goal:
- Decide, with code evidence, whether `ListboxSample`, `SelectSample`, `ComboboxSample`, and
  `CommandSample` should keep display collection fixtures or reconstruct component builders from
  resolved state.

Design rule:
- Scalars should move to resolved state when available.
- Collections should move only when the conversion helper is clearly deep enough to improve
  locality instead of creating a broad type-conversion seam in the shell.

### U4. Strengthen Contract Automation

Files:
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs`

Goal:
- Add narrow tests that prove resolved state is the source for layout and choice metadata.
- Keep smoke tests focused on rendered behavior, not on hard-coded duplicate lists.

Verification:
- `cargo nextest run -p open-gpui-ui-foundation-gallery --tests`

### U5. Record Findings and Continue the Loop

Files:
- `docs/knowledge/engineering/current-state.md`
- `docs/knowledge/engineering/log.md`
- `docs/knowledge/engineering/sessions/2026-06-18-2026-06-18-gallery-architecture-pass-continuation.md`

Goal:
- Record the final state, subagent findings, verification commands, and next action so later
  sessions continue from verified evidence rather than stale chat context.

## Scope Boundaries

- Do not create `open-gpui-ui-headless`.
- Do not move behavior modules between crates in this pass.
- Do not preserve compatibility fields when a resolved state getter already exists and all callers
  can move to it.
- Do not rewrite the whole gallery shell just to remove shallow duplication.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Over-deleting collection fixtures forces awkward builder reconstruction | Medium | Keep collections until helpers prove real locality |
| Subagents or stale sessions edit the same files concurrently | High | Interrupt stale writers and verify after every edit |
| Tests become snapshots of implementation details | Medium | Test public sample/state contracts and rendered selectors, not private helper names |

## Acceptance Examples

- AE1. Given a `SplitterSample`, when the shell renders it, then orientation and size come from
  `sample.state`, not duplicate sample fields.
- AE2. Given a choice or command sample, when scalar metadata is inspected, then placeholder, size,
  selected value, disabled state, query, dialog, and loading facts come from resolved state.
- AE3. Given the Components page smoke test, when a catalog selector or sample selector drifts, then
  the gallery test fails before manual dogfood.
