# Canvas Relation Traversal Benchmark Results

**Date**: 2026-06-12
**Status**: Initial baseline complete

## Summary

This spike measured the current vector-backed `CanvasRecordRelations` traversal through the public
selection-scope API. The goal was to decide whether parent/group relations need an immediate
read-side index after the selection scope refactor.

The current vector model remains the right default for this stage. It keeps relation ordering and
snapshot serialization simple, and the first benchmark pass does not justify adding derived parent
or group indexes before more product-shaped workloads exist.

## Benchmark Setup

Command:

```powershell
cargo bench -p open-gpui-canvas --bench relation_traversal -- --sample-size 10
```

Local notes:

- Criterion used the default 3 second warmup and a 10 sample run.
- `Gnuplot` was not installed, so Criterion used the plotters backend.
- Each benchmark calls `selection_record_scope` with
  `CanvasRecordScopeOptions::structural_with_internal_edges()`.
- Fixtures are intentionally moderate so the benchmark is useful as a fast local regression rather
  than a memory stress test.

## Initial Measurements

`Records` counts canvas nodes, edges, and shapes only; parent and group relation rows are not
included in the number.

| Workload | Records | Time | Shape |
| --- | ---: | ---: | --- |
| `parent/shallow_wide` | 1,001 | 641.59-705.56 us | One frame with 1,000 direct child nodes. |
| `parent/deep_chain` | 257 | 198.40-227.39 us | 256 nested frame records plus one leaf node. |
| `group/shallow_wide` | 1,001 | 1.0664-1.1191 ms | One group shape with 1,000 member nodes. |
| `mixed/nested_parent_group` | 1,088 | 1.5608-1.9113 ms | Root frame, nested group shapes, parent members, duplicate group memberships, and internal edges. |

## Interpretation

The mixed fixture is the slowest because it exercises both relation vectors, duplicate suppression,
and internal edge inclusion. Even there, the current public selection-scope path stays under roughly
2 ms for about one thousand records on this machine.

That does not prove vector scans will remain enough for every future canvas. It does show that an
index would be premature without a larger workload tied to a real operation such as marquee
selection, copy/cut, structural resize, group editing, paint-frame construction, or CRDT merge.

## Decision

Do not add a relation read index in this pass.

Keep `CanvasRecordRelations` as ordered vectors and serialized source of truth. If later data shows
relation traversal is hot, add a crate-private derived index that is rebuilt or updated only through
relation mutation paths. That index must stay out of snapshots and must preserve the vector order as
the replay/serialization contract.

## Follow-Up Work

- Add product-shaped benchmarks once group editing, frame layout, or mind-map expansion tools land.
- Compare traversal cost inside complete operations, not only isolated `selection_record_scope`.
- Revisit indexing if large nested canvases show relation traversal dominating selection, resize,
  copy, paint, or persistence workloads.
