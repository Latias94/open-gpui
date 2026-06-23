---
title: Table manual row-model controls
type: feat
date: 2026-06-23
---

# Table manual row-model controls

## Summary

Table already owns client-side filtering, sorting, grouping, expansion, and pagination. This plan adds manual control points for the row-model stages that applications commonly source from the server, plus the row-count metadata needed to render those states honestly.

## Problem Frame

The current Table slice can render server-owned child loading, but it still assumes local row-model transforms for the filtering, sorting, and pagination stages. Real data-fetching tables need to keep those stages app-owned so the component can render the current snapshot without pretending it has already computed the next page or filter result.

TanStack Table treats manual filtering, sorting, and pagination as first-class opt-ins and pairs manual pagination with total row-count metadata. That shape matches the product direction here better than extracting a separate headless data-source crate.

## Requirements

- R1. TableState can mark filtering, sorting, and pagination as manual independently.
- R2. Manual stages preserve the caller-supplied row snapshot instead of reapplying the local transform.
- R3. Pagination metadata can report a server-known total row count and derived page count.
- R4. Manual row-model modes keep row ids, selection, grouping, expansion, and lookup behavior stable.
- R5. The GPUI Table adapter exposes the manual-mode metadata through its public API and render-plan summaries.
- R6. The Components gallery includes a server-paged Table sample that proves page changes, total counts, and stable row identities under manual pagination.
- R7. Contract docs, verification docs, and engineering memory are updated together with the implementation.

## Key Technical Decisions

- Separate stage-level manual flags are better than one global server mode. Filtering, sorting, and pagination often move independently in real apps.
- Keep the data source app-owned. `ui_components` should describe the snapshot and expose metadata, not fetch rows or manage cache lifecycles.
- Use TanStack-style `rowCount` and `pageCount` semantics for pagination metadata. That keeps the API familiar and avoids inventing a fetch abstraction.
- Defer faceting value caches, row pinning, and any standalone headless extraction to later slices.

## Scope Boundaries

### Deferred for later

- Faceted value caches and server facet result payloads.
- Row pinning.
- Cell editing.
- Two-axis grid virtualization.
- Standalone headless extraction.

### Outside this plan

- Real network fetching, retries, cancellation, or cache invalidation.
- A shared query client or data-source abstraction.
- Compatibility shims for older pagination APIs.

## Implementation Units

### U1. Add manual row-model policy and pagination totals in `ui_core`

- Extend the pagination and row-model state so filtering, sorting, and pagination can be manual independently.
- Preserve client-side defaults so existing tables keep current behavior.
- Add row-count / page-count metadata for manual pagination.
- Update the row-model cache key and resolver tests to cover manual and client paths.

### U2. Surface the new metadata in `ui_components`

- Re-export the new policy and metadata through the crate root and prelude.
- Extend the Table adapter and render-plan summaries so manual pagination states are visible in gallery readouts.
- Keep the existing sort/filter/pagination callbacks and current control surface intact.

### U3. Add a server-paged Components gallery sample

- Add a focused Table sample that simulates app-owned page snapshots and total row counts.
- Prove the table renders the supplied page snapshot without local slicing in manual mode.
- Cover page navigation, stable row ids, and total-count readouts in a runtime smoke.

### U4. Update contracts, verification, and memory

- Update the Table contract and verification docs to describe manual row-model controls and pagination totals as shipped behavior.
- Refresh engineering memory with the new slice and its verification results.
- Keep the plan, docs, and tests aligned on the same manual/server vocabulary.

## Sources / Research

- `crates/ui_core/src/table.rs`
- `crates/ui_components/src/table.rs`
- `examples/ui-foundation-gallery/src/pages/components.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs`
- `repo-ref/tanstack-table/examples/preact/with-tanstack-query/src/main.tsx`
- `repo-ref/tanstack-table/examples/react/mantine-react-table/src/mantine-react-table/hooks/useMRT_TableOptions.ts`
- `repo-ref/tanstack-table/packages/table-core/src/features/row-sorting/rowSortingFeature.types.ts`
- `repo-ref/tanstack-table/examples/react/lib-react-aria/src/main.tsx`

