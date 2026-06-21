# ADR 0009: Open GPUI Table and Virtualizer Product Shape

**Status**: Accepted
**Date**: 2026-06-22

## Context

ADR 0008 keeps the current UI crates as the active product boundary. Table and virtualization are
the next pressure points for that boundary because they combine renderer-neutral state, large data
sets, nested scrolling, accessibility metadata, and concrete GPUI input wiring.

The project already has relevant primitives: `open-gpui-ui-core` owns neutral sizing, geometry,
accessibility, overlay, and token vocabulary; `open-gpui-ui-components` owns concrete GPUI
components and adapter helpers; `examples/ui-foundation-gallery` is the conformance surface. The
existing `crates/gpui/examples/data_table.rs` proves GPUI can draw tabular content, but it is not an
official component contract.

Local references shape the design without becoming source-level dependencies. `repo-ref/fret`
shows a useful split between thin facades and deeper behavior modules. TanStack Table supplies the
row-model vocabulary and stable row identity semantics. TanStack Virtual supplies the range,
measurement, overscan, and snapshot vocabulary for virtual lists.

## Decision

Open GPUI will build the first table and virtualizer series inside the existing UI crates.

- `open-gpui-ui-core` owns renderer-neutral table state, row identity, row-model ordering, and
  virtualizer range calculations.
- `open-gpui-ui-components` owns the GPUI `Table` adapter, concrete scroll handles, input events,
  accessibility mapping, and visible table chrome.
- `examples/ui-foundation-gallery` owns the official table dogfood surface and regression gates.
- The first table slice implements the row-model subset core -> filtered -> sorted -> paginated.
- The full row-model vocabulary keeps grouped and expanded stages named, but those transforms stay
  deferred.
- The virtualizer contract is a pure input/output calculator. It accepts counts, extents, offsets,
  keys, estimates, measurements, overscan, gap, and scroll margin, and it returns ranges,
  measurements, total size, and snapshot data.
- Do not introduce a standalone `open-gpui-ui-headless` crate for this series.

## Rationale

Table state and virtualized scrolling need to be testable without a GPUI window, but a separate
headless package would freeze API boundaries before the first GPUI table surface proves the real
behavior. Keeping the contracts in `open-gpui-ui-core` makes the reusable rules explicit while
keeping the product surface in the crates users already consume.

The split also matches how scroll bugs usually occur. The neutral virtualizer should prove range
math, measurement idempotence, key stability, and total size. The GPUI adapter should prove scroll
ownership, wheel containment, focus, keyboard, pointer behavior, and accessibility metadata.

## Consequences

Positive:

- Row identity and virtualization behavior can be tested as pure Rust contracts.
- The first official table can ship without inventing a new crate boundary.
- Future adapters have a stable semantic vocabulary if extraction becomes useful later.
- Gallery regression tests can distinguish row-model failures from scroll and layout failures.

Negative:

- `open-gpui-ui-core` gains more domain vocabulary and must stay disciplined about renderer-neutral
  types.
- Full TanStack parity remains out of scope and must be reopened intentionally.
- Complex table features such as grouped rows, tree expansion, pinned columns, and two-dimensional
  virtualization need follow-up decisions.

## Follow-Up Work

- Add `open_gpui_ui_core::table` with stable row ids, lookup, selection, sorting, filtering, and
  pagination.
- Add `open_gpui_ui_core::virtualizer` with deterministic range and measurement behavior.
- Add an official `open_gpui_ui_components::Table` adapter backed by those contracts.
- Add Components gallery samples and conformance tests for long scrollable tables.
- Update verification and engineering memory when the first slice ships.

## Citations

[1] [ADR 0008](0008-open-gpui-ui-component-productization-roadmap.md)
[2] [Table and virtualizer roadmap plan](../plans/2026-06-21-001-feat-ui-table-virtualizer-roadmap-plan.md)
[3] `repo-ref/fret/docs/adr/0100-headless-table-engine.md`
[4] `repo-ref/fret/docs/adr/0070-virtualization-contract.md`
[5] `repo-ref/tanstack-table/docs/guide/row-models.md`
[6] `repo-ref/tanstack-virtual/docs/api/virtualizer.md`
