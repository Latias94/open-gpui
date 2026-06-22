---
title: "Open GPUI Table Depth Plan"
type: feat
date: 2026-06-23
execution: code
branch: main
depends_on:
  - docs/adr/0009-open-gpui-table-and-virtualizer-product-shape.md
  - docs/plans/2026-06-21-001-feat-ui-table-virtualizer-roadmap-plan.md
  - docs/knowledge/engineering/decisions/open-gpui-ui-component-depth-roadmap.md
  - docs/ui/component-contract.md
  - docs/verification.md
  - crates/ui_core/src/table.rs
  - crates/ui_components/src/table.rs
  - examples/ui-foundation-gallery/src/pages/components.rs
  - repo-ref/fret/ecosystem/fret-ui-headless/src/table/grouping.rs
  - repo-ref/fret/ecosystem/fret-ui-headless/src/table/row_expanding.rs
  - repo-ref/fret/ecosystem/fret-ui-headless/src/table/column_pinning.rs
  - repo-ref/fret/docs/adr/0100-headless-table-engine.md
---

# Open GPUI Table Depth Plan

## Summary

Deepen the official `Table` family beyond the v0 row model by adding grouped rows, expansion,
simple aggregation metadata, and pinned-column semantics while keeping the existing
`ui_core` / `ui_components` boundary. This slice should make the named row-model stages in
`TableState` real enough for application workflows without turning the component into a full data
grid or extracting a standalone headless crate.

## Problem Frame

The current table stack is useful but shallow. `TableState` has stable row ids, filtering, sorting,
pagination, selection, and a fixed-row virtualizer path. The row-model vocabulary already names
`Grouped` and `Expanded`, but those stages are not implemented. The GPUI adapter can render large
vertical lists, but it cannot yet express common production table behavior such as group headers,
collapsed groups, summary values, or pinned left/right columns.

This is the right next table slice because the foundation work is already done. Virtualization and
runtime scroll ownership should stay stable; the new work should deepen table semantics and then
prove those semantics in the focused Components gallery.

## Requirements

- R1. Keep table semantics renderer-neutral in `open-gpui-ui-core`; keep GPUI focus, events,
  scroll handles, and visual lane layout in `open-gpui-ui-components`.
- R2. Implement the full row-model order already documented for Open GPUI tables:
  core -> filtered -> grouped -> sorted -> expanded -> paginated -> final.
- R3. Add caller-owned grouping and expansion state. Expansion must use stable row identities, not
  visible indices.
- R4. Add group rows with stable ids, depth, parent metadata, leaf counts, and aggregate cell
  metadata that renderers can inspect.
- R5. Add pinned-column state that can split visible columns into left, center, and right regions
  without requiring two-dimensional virtualization.
- R6. Keep row virtualization one-dimensional for this slice. Group rows and leaf rows both
  participate in the same vertical virtual window.
- R7. Extend gallery samples and gates so grouped, expanded, aggregated, and pinned-column behavior
  is visible and testable.

## Acceptance Examples

- AE1. Given rows grouped by `team`, `TableState::resolve()` returns stable group row ids before
  the grouped stage leaves `ui_core`.
- AE2. Given a collapsed group, the expanded and final row models include the group row but exclude
  its descendant leaf rows.
- AE3. Given an expanded group, descendant rows render under the group with depth metadata, stable
  source row ids, and preserved selection state.
- AE4. Given sorted grouped rows, sibling groups and leaf rows resolve deterministically with stable
  id tie-breakers.
- AE5. Given aggregate specs for numeric columns, group rows expose aggregate cells and leaf counts
  without renderer-specific callbacks.
- AE6. Given pinned left and right columns, the render plan splits columns into left, center, and
  right regions while preserving visible column order inside each region.
- AE7. Given the grouped gallery sample is scrolled, wheel input stays inside the table viewport and
  virtualized row counts remain bounded by visible rows plus overscan.
- AE8. Given catalog conformance runs, `Table`, `TableState`, row-model stages, pinned-column
  metadata, gallery selectors, and verification docs stay aligned.

## Key Technical Decisions

- **Make grouped and expanded stages first-class now:** The current row-model vocabulary already
  promises those stages. Implementing them removes the largest semantic gap in the official table.
- **Use group rows as resolved rows, not a side channel:** Renderers should consume one final row
  stream that can contain both group and leaf rows. This keeps virtualization, accessibility row
  counts, keyboard focus, and debug selectors aligned.
- **Keep expansion caller-owned:** `TableState` stores the expansion input; the adapter may emit
  toggle payloads later, but it must not hide expansion ownership in GPUI runtime state.
- **Keep aggregation declarative:** Start with built-in aggregate kinds such as count, sum, min,
  max, and average for `TableCellValue`. Do not add arbitrary closures to `ui_core` in this slice.
- **Pin columns semantically before grid virtualization:** Pinned left/right regions should be
  explicit in resolved column metadata and the GPUI render plan. Sticky horizontal scrolling,
  column resizing, and two-dimensional grid virtualization remain separate work.
- **Use `repo-ref/fret` for structure, not source cloning:** Fret's grouping, row-expanding, and
  column-pinning modules are the closest local references, but Open GPUI should keep its existing
  Rust-native API shape.

## Implementation Units

### U1. Extend the core table row type for group-aware row models

**Goal:** Represent leaf and group rows in the same resolved row stream.

**Requirements:** R1, R2, R4

**Files:**

- Modify `crates/ui_core/src/table.rs`
- Modify `crates/ui_components/tests/components.rs`

**Approach:** Replace the leaf-only `TableResolvedRow` assumptions with a resolved row kind that
can carry either a source `TableRow` or group metadata. Keep stable ids, source indices for leaf
rows, selection metadata, and rows-by-id lookup. Add group metadata for grouping column, grouping
value text, depth, parent id, first leaf id, and leaf count. The API may break existing internal
callers if that makes the contract clearer; update adapter and tests in the same slice.

**Test scenarios:**

- Leaf rows keep source row ids, source indices, and selection metadata.
- Group rows have deterministic ids derived from their grouping path.
- Rows-by-id lookup resolves both group rows and leaf rows.
- Duplicate source row ids are still reported without panicking.

### U2. Implement grouping and expansion in the core pipeline

**Goal:** Make the documented grouped and expanded row-model stages real.

**Requirements:** R2, R3, R4, R6

**Files:**

- Modify `crates/ui_core/src/table.rs`
- Modify `docs/ui/component-contract.md`

**Approach:** Add grouping state as an ordered list of column ids and expansion state as either all
rows expanded or a set of expanded resolved row ids. Resolve pipeline stages as core -> filtered ->
grouped -> sorted -> expanded -> paginated -> final. Grouping buckets should preserve first-seen
order before sorting, generate stable nested group ids, and retain all leaf rows in lookup maps even
when collapsed. Expansion should only affect the flattened visible model, not row lookup metadata.

**Test scenarios:**

- Empty grouping keeps the current v0 row order.
- Single-column grouping creates one group per distinct value.
- Multi-column grouping creates nested group rows with increasing depth.
- Collapsed groups hide descendants from final rows while preserving lookup.
- Expanded groups show descendants with stable row ids and selected state.
- Pagination applies after expansion, not before it.

### U3. Add built-in aggregation metadata

**Goal:** Give group rows useful summary cells without adding renderer callbacks to `ui_core`.

**Requirements:** R1, R4

**Files:**

- Modify `crates/ui_core/src/table.rs`
- Modify `crates/ui_components/src/table.rs`
- Modify `docs/ui/component-contract.md`

**Approach:** Add aggregate specs keyed by column id. Start with deterministic built-in aggregate
kinds for count, sum, min, max, and average over `TableCellValue`. Group rows should expose
aggregate cells through the same cell lookup path renderers already use, with sensible empty values
when an aggregate does not apply. Keep custom aggregation deferred until a real caller needs it.

**Test scenarios:**

- Count aggregates include all descendant leaf rows.
- Numeric sum, min, max, and average ignore non-numeric values deterministically.
- Aggregate cells are stable across sorting and expansion changes.
- Leaf row cells continue to return source values unchanged.

### U4. Add pinned-column state and render lanes

**Goal:** Represent pinned left/right columns in both core state and the GPUI render plan.

**Requirements:** R1, R5, R7

**Files:**

- Modify `crates/ui_core/src/table.rs`
- Modify `crates/ui_components/src/table.rs`
- Modify `crates/ui_components/tests/components.rs`
- Modify `docs/ui/component-contract.md`

**Approach:** Add `TableColumnPinning` state with left and right column id lists. Resolved visible
columns should split into left, center, and right regions after visibility, explicit ordering, and
grouped-column ordering are applied. The GPUI adapter should render stable region metadata and
debug selectors. Keep true two-axis virtualization, drag resize, and sticky horizontal scrollbars
out of scope.

**Test scenarios:**

- Pinned columns are removed from the center region.
- Moving a column between left and right regions does not duplicate it.
- Unknown or invisible pinned ids are ignored.
- Header and body cells use the same column region order.
- Render plan selectors expose left, center, and right regions for gallery tests.

### U5. Add grouped and pinned table gallery samples

**Goal:** Make the new table semantics visible and regression-testable.

**Requirements:** R6, R7

**Files:**

- Modify `examples/ui-foundation-gallery/src/pages/components.rs`
- Modify `examples/ui-foundation-gallery/src/pages/components/render.rs`
- Modify `examples/ui-foundation-gallery/tests/foundation_gallery.rs`
- Modify `docs/verification.md`

**Approach:** Add a focused Table sample that groups release rows by team or status, starts with a
mix of expanded and collapsed groups, exposes aggregate counts / numeric summaries, and pins one
identifier column plus one status/action column. Keep the sample within the existing Components
gallery, preserving focused-mode inspection and nested scroll containment.

**Test scenarios:**

- Focused Table mode renders the base virtualized sample and the new grouped sample.
- Gallery state summaries expose grouped, expanded, aggregate, and pinned-column metadata.
- Runtime smoke proves the grouped sample scrolls locally inside the table viewport.
- Row debug selectors show group rows and leaf rows with stable ids.
- Catalog conformance keeps Table signals and sample selectors aligned.

### U6. Verification, memory, and review pass

**Goal:** Close the slice with durable evidence and next-step boundaries.

**Requirements:** R7

**Files:**

- Modify `docs/verification.md`
- Modify `docs/knowledge/engineering/current-state.md`
- Modify `docs/knowledge/engineering/log.md`

**Approach:** Run focused `nextest` gates for `ui_core table`, `ui_components table`, and
foundation-gallery Table tests before running broader component/gallery checks if the change
touches shared exports or catalog metadata. Update memory with what shipped, what remains deferred,
and which gallery selectors prove the new behavior.

**Verification commands:**

- `cargo fmt -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-ui-foundation-gallery`
- `cargo nextest run -p open-gpui-ui-core table`
- `cargo nextest run -p open-gpui-ui-components table`
- `cargo nextest run -p open-gpui-ui-foundation-gallery table`
- `cargo check -p open-gpui-ui-components --tests`
- `cargo check -p open-gpui-ui-foundation-gallery --tests`
- `git diff --check`
- `python $HOME\\.codex\\skills\\engineering-wiki-memory\\scripts\\wiki_memory.py validate --root docs\\knowledge\\engineering`

## Scope Boundaries

### Active Scope

- Grouped rows, expansion state, aggregate metadata, and pinned-column state for the official
  `Table` family.
- GPUI render-plan and gallery support for those semantics.
- One-dimensional vertical virtualization over the final row stream.
- Focused Components gallery and state-summary verification.

### Deferred

- Two-dimensional grid virtualization.
- Sticky horizontal scroll implementation and custom column resize handles.
- Tree-data tables with arbitrary nested source rows.
- Custom aggregation callbacks in `ui_core`.
- App-wide data loading, server pagination, faceting UI, and row-editing workflows.
- Standalone headless crate extraction.

## Risks and Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Resolved row API changes ripple through adapter tests | Medium implementation churn | Break deliberately inside this slice and update all direct callers together |
| Group rows accidentally lose leaf lookup metadata | Selection, expansion, and future keyboard behavior drift | Keep rows-by-id over both visible and collapsed rows, and test lookup directly |
| Aggregation grows into a custom engine too early | Scope expansion and unclear callback ownership | Start with built-in deterministic aggregate kinds only |
| Pinned columns imply a full data grid | Rendering complexity and scroll bugs | Ship semantic lanes and stable selectors first; defer 2D virtualization and resize |
| Large grouped samples become expensive per redraw | Gallery frame cost regresses | Reuse current table runtime cache and lazy sample storage; keep virtualizer one-axis |

## Sources and Research

- `docs/adr/0009-open-gpui-table-and-virtualizer-product-shape.md`
- `docs/plans/2026-06-21-001-feat-ui-table-virtualizer-roadmap-plan.md`
- `docs/knowledge/engineering/decisions/open-gpui-ui-component-depth-roadmap.md`
- `crates/ui_core/src/table.rs`
- `crates/ui_components/src/table.rs`
- `examples/ui-foundation-gallery/src/pages/components.rs`
- `repo-ref/fret/ecosystem/fret-ui-headless/src/table/grouping.rs`
- `repo-ref/fret/ecosystem/fret-ui-headless/src/table/row_expanding.rs`
- `repo-ref/fret/ecosystem/fret-ui-headless/src/table/column_pinning.rs`
- `repo-ref/fret/docs/adr/0100-headless-table-engine.md`
