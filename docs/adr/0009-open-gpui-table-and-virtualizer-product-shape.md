# ADR 0009: Open GPUI Table and Virtualizer Product Shape

**Status**: Accepted
**Date**: 2026-06-22
**Updated**: 2026-07-18

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
shows a useful split between thin facades and deeper behavior modules. The reviewed TanStack Table
reference is the local `@tanstack/table-core` `9.0.0-beta.31` clone at commit
`5af79a877fa80f63703c6dc21861acc9d18baecf`; it supplies comparison vocabulary for row-model stages
and manual ownership. TanStack Virtual supplies comparison vocabulary for range, measurement,
overscan, and snapshots.

## Decision

Open GPUI will build the first table and virtualizer series inside the existing UI crates.

- `open-gpui-ui-core` owns renderer-neutral table state, row identity, row-model ordering, and
  virtualizer range calculations.
- `open-gpui-ui-components` owns the GPUI `Table` adapter, concrete scroll handles, input events,
  accessibility mapping, and visible table chrome.
- `examples/ui-foundation-gallery` owns the official table dogfood surface and regression gates.
- The executable row pipeline is core -> filtered -> grouped -> sorted -> expanded -> paginated ->
  final. Row pinning derives top/center/bottom partitions after stage resolution; it is not a row
  stage. Each stage exposes real behavior, not a version label.
- Filtering, sorting, faceting, and pagination may be client-owned or manual. Grouping is
  client-owned. Manual expansion applies only to ungrouped source-tree rows; grouped expansion
  remains client-owned. Manual stages preserve caller-supplied row order rather than pretending to
  execute a client transform.
- `TableRowModel::rows()` is Open GPUI's flat materialized order for that stage. Addressable rows
  that are not currently materialized remain available through the typed lookup model.
- The virtualizer contract is a pure input/output calculator. It accepts counts, extents, offsets,
  keys, estimates, measurements, overscan, gap, and scroll margin, and it returns ranges,
  measurements, total size, and snapshot data.
- Do not introduce a standalone `open-gpui-ui-headless` crate for this series.

## TanStack Comparison

Open GPUI intentionally preserves the useful TanStack stage ordering without adopting TanStack as
a dependency or cloning its plugin/atom architecture.

- Open GPUI uses typed source, occurrence, explicit-instance, and synthetic-group identities rather
  than a single string row id.
- `TableRowModel::rows()` is flattened materialized order; TanStack's top-level `rows` and nested
  `subRows` shape must not be used as an expected-value oracle without this translation.
- Row pinning is filter-aware, resolves exact typed targets, and uses top-wins conflict handling.
  Column ordering is global before the resolved pin partitions.
- Grouping is currently client-owned. Server/manual grouping would require an explicit caller-owned
  grouped model rather than a misleading mode flag.
- `VirtualizerState` remains an independent deterministic engine and restoration contract; Table
  consumes it but does not merge it into the row-model pipeline.

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
- Server grouping, dataset-wide autosizing, fetch/cache orchestration, and deeper two-dimensional
  virtualization still need separate product decisions.

## Follow-Up Work

- Keep stage behavior characterized with mixed filter/group/sort/expand/paginate/pin tests.
- Keep `TableBehaviorSnapshot` diagnostics under `open_gpui_ui_components::table`, while real
  restoration inputs remain in the common component surface.
- Keep `TableStateCacheKey` under `open_gpui_ui_core::table`; it is the production runtime cache
  invalidation key, not a foundation-wide default import.
- Re-audit this ADR against the local TanStack reference when adding a new row-model authority.

## Citations

[1] [ADR 0008](0008-open-gpui-ui-component-productization-roadmap.md)
[2] [Table and virtualizer roadmap plan](../plans/2026-06-21-001-feat-ui-table-virtualizer-roadmap-plan.md)
[3] `repo-ref/fret/docs/adr/0100-headless-table-engine.md`
[4] `repo-ref/fret/docs/adr/0070-virtualization-contract.md`
[5] `repo-ref/tanstack-table/docs/guide/row-models.md`
[6] `repo-ref/tanstack-virtual/docs/api/virtualizer.md`
