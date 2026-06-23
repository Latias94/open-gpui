---
title: "Open GPUI Table Manual Expansion and Async Children Plan"
type: feat
date: 2026-06-23
execution: code
branch: main
depends_on:
  - docs/plans/2026-06-23-005-feat-ui-table-tree-data-plan.md
  - docs/ui/component-contract.md
  - docs/verification.md
  - crates/ui_core/src/table.rs
  - crates/ui_components/src/table.rs
  - crates/ui_components/tests/components.rs
  - examples/ui-foundation-gallery/src/pages/components.rs
  - examples/ui-foundation-gallery/src/pages/components/render.rs
  - examples/ui-foundation-gallery/tests/foundation_gallery.rs
  - repo-ref/tanstack-table/docs/framework/react/guide/expanding.md
  - repo-ref/tanstack-table/docs/reference/index/interfaces/TableOptions_RowExpanding.md
  - repo-ref/tanstack-table/docs/reference/static-functions/functions/table_getExpandedRowModel.md
  - repo-ref/fret/ecosystem/fret-ui-kit/src/tree.rs
  - repo-ref/fret/ecosystem/fret-ui-kit/src/declarative/tree.rs
---

# Open GPUI Table Manual Expansion and Async Children Plan

## Summary

Extend the shipped tree-data Table contract so source rows can represent server-owned children:
expandable rows may start without loaded descendants, expose loading or error metadata, and emit
controlled expansion requests without forcing the component to own fetch tasks.

## Problem Frame

The current tree-data slice assumes child rows are already present in `TableRow::children`. That is
correct for local dependency trees, but server-backed trees often know that a row can expand before
its children are fetched. TanStack Table handles this through manual expanding: the table can emit
expanded state changes while the caller owns the data model that appears after expansion. Fret's
tree/list helpers follow the same boundary by flattening the current snapshot from app-owned
models and leaving async/caching work outside the UI helper.

Open GPUI should add the missing row metadata and rendering contract while keeping the component
renderer-neutral. The Table should not start async tasks, hold request handles, or invent a global
data source API.

---

## Requirements

**Core Table Contract**

- R1. A source `TableRow` can be marked expandable even when it has no loaded child rows.
- R2. A source `TableRow` can carry child-loading metadata for idle, loading, and failed states.
- R3. Resolved tree-row metadata distinguishes loaded children from expandable capability.
- R4. Client expansion remains the default and continues to hide collapsed descendants.
- R5. Manual expansion mode lets callers provide the visible source-tree snapshot without the core
  row model pruning descendants from `TableExpansionState`.

**Component Contract**

- R6. The GPUI `Table` renders disclosure affordances for expandable unloaded rows.
- R7. Expansion payloads include enough metadata for the caller to decide whether to fetch,
  retry, or only update expanded state.
- R8. Loading and error states are visible and accessible from the row affordance without changing
  selection or row activation semantics.

**Gallery and Verification**

- R9. The Components gallery includes a deterministic server-tree sample that starts with an
  unloaded expandable branch and simulates an app-owned child load after expansion.
- R10. Focused tests prove the first click emits a load/expand request, loaded children appear
  after the app updates the sample snapshot, and row activation remains separate from disclosure.
- R11. Contract, verification docs, and engineering memory describe manual expansion as shipped
  behavior and keep real async task orchestration deferred to applications.

---

## Acceptance Examples

- AE1. Given a row is marked expandable with no children, `TableState::resolve()` exposes it as a
  tree branch and reports zero loaded children.
- AE2. Given a row is marked loading, the resolved row carries loading metadata and the component
  renders a busy disclosure state.
- AE3. Given a row has a failed child load, the resolved row carries the failure message and the
  component emits a retry-capable expansion payload.
- AE4. Given client expansion mode, collapsed loaded descendants stay hidden exactly as they do in
  the previous tree-data slice.
- AE5. Given manual expansion mode, supplied descendants remain visible as caller-owned snapshot
  data even when the expansion set would otherwise hide them.
- AE6. Given a gallery server-tree branch is clicked, the expansion log records the unloaded branch
  state and the sample app inserts loaded children into the next render snapshot.

---

## Key Technical Decisions

- **Represent remote child state on `TableRow`:** A row descriptor is already the durable,
  cloneable input object. Adding expandable and child-load metadata there keeps cache keys,
  equality, tests, and serialization-friendly state in one place.
- **Separate `has_children` from `can_expand`:** Loaded children are data shape; expandable is UI
  capability. Rows with unloaded, loading, or failed children need a disclosure even when
  `children()` is empty.
- **Add manual expansion as a row-model mode, not a data-source abstraction:** TanStack's
  `manualExpanding` skips local expanded-row pruning. Open GPUI can mirror that with a compact
  mode on `TableState`; applications still own fetching, cancellation, caching, and retry policy.
- **Keep expansion callbacks semantic:** `on_row_expansion_request` remains the event surface. The
  payload should grow with row child-load metadata rather than adding a separate `on_load_children`
  callback that would duplicate the same user gesture.
- **Do not render synthetic loading rows in the core model:** Loading and error are row metadata in
  this slice. Placeholder child rows, skeleton rows, or detail panels can be modeled later if a
  concrete product flow needs them.

---

## Implementation Units

### U1. Add expandable and child-load metadata to core rows

- **Goal:** Let source rows describe unloaded, loading, and failed child states without losing flat
  row compatibility.
- **Requirements:** R1, R2, R3
- **Files:**
  - Modify `crates/ui_core/src/table.rs`
  - Modify `crates/ui_components/src/lib.rs`
  - Modify `crates/ui_components/src/prelude.rs`
  - Modify `crates/ui_components/tests/components.rs`
- **Approach:** Add a renderer-neutral child-load enum and expandable flag to `TableRow`. Expose
  `can_expand`, `loaded_child_count`, and child-load state through `TableTreeRow` and
  `TableResolvedRow`. Default rows remain leaves unless they have children.
- **Patterns to follow:** Existing `TableTreeRow` metadata, `CommandLoadingState` naming style in
  `crates/ui_components/src/command.rs`, and Fret `TreeEntry` capability metadata in
  `repo-ref/fret/ecosystem/fret-ui-kit/src/tree.rs`.
- **Test scenarios:**
  - Existing flat rows remain non-tree rows.
  - Rows with children are expandable and report loaded child count.
  - Rows marked expandable with no children resolve as tree branches.
  - Loading and failed rows preserve metadata through row lookup.
- **Verification:** `cargo nextest run -p open-gpui-ui-core table`

### U2. Add manual expansion row-model mode

- **Goal:** Let applications supply the visible tree snapshot when server-side expansion owns child
  materialization.
- **Requirements:** R4, R5
- **Files:**
  - Modify `crates/ui_core/src/table.rs`
  - Modify `crates/ui_components/tests/components.rs`
- **Approach:** Add a `TableExpansionMode` with client default and manual mode. Client mode keeps
  the current expansion flattening behavior. Manual mode flattens the supplied source-tree snapshot
  and uses `TableExpansionState` only as expanded-state metadata.
- **Patterns to follow:** TanStack `manualExpanding` docs in
  `repo-ref/tanstack-table/docs/framework/react/guide/expanding.md` and the current
  `TableExpansionState` resolver.
- **Test scenarios:**
  - Client mode still hides descendants behind collapsed parents.
  - Manual mode keeps supplied descendants visible even when no row id is expanded.
  - Manual mode still reports explicit expanded state on branch metadata.
  - Grouped rows keep existing behavior.
- **Verification:** `cargo nextest run -p open-gpui-ui-core table`

### U3. Render expandable unloaded rows and enrich expansion payloads

- **Goal:** Make the GPUI `Table` affordance reflect can-expand, loading, and failed states while
  keeping expansion controlled by the caller.
- **Requirements:** R6, R7, R8
- **Files:**
  - Modify `crates/ui_components/src/table.rs`
  - Modify `crates/ui_components/tests/components.rs`
- **Approach:** Render disclosure controls when `can_expand` is true. Add child-load metadata to
  `TableRowAction` or `TableRowExpansionToggle` so the callback can distinguish first load,
  retry, and normal expand/collapse. Preserve the runtime expansion override for immediate UI
  feedback only when the event is a normal client toggle.
- **Patterns to follow:** Existing row activation payloads, existing tree disclosure renderer, and
  Command loading metadata accessors.
- **Test scenarios:**
  - Render plans expose child-load metadata for unloaded, loading, and failed branches.
  - Disclosure click on an unloaded branch emits expansion payload with child-load metadata.
  - Disclosure click does not emit row activation.
  - Keyboard Right/Left keeps working for loaded tree branches.
- **Verification:** `cargo nextest run -p open-gpui-ui-components table component_api_inventory`

### U4. Add gallery server-tree proof and docs

- **Goal:** Prove the contract in the official Components gallery and record the shipped boundary.
- **Requirements:** R9, R10, R11
- **Files:**
  - Modify `examples/ui-foundation-gallery/src/pages/components.rs`
  - Modify `examples/ui-foundation-gallery/src/pages/components/render.rs`
  - Modify `examples/ui-foundation-gallery/tests/foundation_gallery.rs`
  - Modify `docs/ui/component-contract.md`
  - Modify `docs/verification.md`
  - Modify `docs/knowledge/engineering/current-state.md`
  - Modify `docs/knowledge/engineering/log.md`
- **Approach:** Add a `server-tree` Table sample whose app-owned runtime starts with an unloaded
  expandable branch. On expansion request, the gallery updates its sample snapshot with loaded
  children and records the request metadata. Update documentation to say async tasks remain
  application-owned.
- **Patterns to follow:** Existing `dependency-tree` sample and Fret's app-owned model snapshot
  approach in `repo-ref/fret/ecosystem/fret-ui-kit/src/declarative/tree.rs`.
- **Test scenarios:**
  - Metadata test covers unloaded branch count, loading/error metadata, and manual expansion mode.
  - Smoke test clicks the server branch, sees child rows after the app-owned snapshot update, and
    verifies the expansion log includes child-load metadata.
  - Existing dependency-tree smoke still passes.
- **Verification:** `cargo nextest run -p open-gpui-ui-foundation-gallery table`

---

## Scope Boundaries

- Do not implement real network fetching, cancellation, retry backoff, cache invalidation, or a
  global table data source API.
- Do not introduce synthetic loading child rows or detail panels in the core row model.
- Do not compose source-tree manual expansion with grouped-row aggregation beyond preserving the
  existing grouped-row path.
- Do not add checkbox/range selection, row pinning, cell editing, or full two-axis grid
  virtualization in this slice.

---

## Sources

- `docs/plans/2026-06-23-005-feat-ui-table-tree-data-plan.md`
- `repo-ref/tanstack-table/docs/framework/react/guide/expanding.md`
- `repo-ref/tanstack-table/docs/reference/index/interfaces/TableOptions_RowExpanding.md`
- `repo-ref/tanstack-table/docs/reference/static-functions/functions/table_getExpandedRowModel.md`
- `repo-ref/fret/ecosystem/fret-ui-kit/src/tree.rs`
- `repo-ref/fret/ecosystem/fret-ui-kit/src/declarative/tree.rs`
