# Canvas Spatial Index Benchmark Results

**Date**: 2026-06-08
**Status**: Initial spike complete

## Summary

This spike compared the current sorted-vector `SpatialIndex` with dev-only `rstar`,
`static_aabb2d_index`, and a simulated static-base plus dynamic-overlay candidate. The candidates
were built only in tests and benches, not in the production runtime.

The current index remains the correct 0.1 public oracle because it is simple and deterministic.
The follow-up runtime prototype has now landed behind `CanvasRuntime`: production queries use an
internal stable-base plus overlay cache, while third-party index crates remain confined to tests and
benches.

## Correctness Coverage

`crates/canvas/tests/spatial_index_strategies.rs` compares each candidate against `SpatialIndex` as
the oracle.

Covered semantics:

- visible query parity across grid, dense-overlap, clustered, long-edge, and mixed-kind fixtures;
- hit-test ordering parity for overlapping records;
- `HitOptions` parity for hidden records, locked records, handles, and margin expansion;
- custom edge-router bounds;
- registered kind geometry bounds;
- bench-only hybrid overlay stale-record suppression for dragged nodes, node handles, incident
  edges, and deleted nodes;
- runtime-owned hybrid cache parity for rebuilds, hit tests, custom routers, kind geometry, and
  diff-fed drag updates.

Verification on this branch:

```text
cargo nextest run -p open-gpui-canvas
198 tests run: 198 passed

cargo check -p open-gpui-canvas --benches
passed

cargo check -p open-gpui-canvas
passed
```

## Benchmark Setup

Representative command examples:

```powershell
cargo bench -p open-gpui-canvas --bench spatial_index_strategies "spatial_index/grid/rebuild"
cargo bench -p open-gpui-canvas --bench spatial_index_strategies "spatial_index/grid/query"
cargo bench -p open-gpui-canvas --bench spatial_index_strategies "spatial_index/grid/hit_test"
cargo bench -p open-gpui-canvas --bench spatial_index_strategies "spatial_index/grid/paint_frame_culling"
cargo bench -p open-gpui-canvas --bench spatial_index_strategies "spatial_index/drag_grid/drag_overlay/hybrid/10"
cargo bench -p open-gpui-canvas --bench spatial_index_strategies "spatial_index/drag_grid/drag_update/runtime/10"
```

Machine-local notes:

- Criterion was configured for short spike runs: 10 samples, 250 ms warmup, 2 second target
  measurement.
- `Gnuplot` was not installed, so Criterion used the plotters backend.
- The first `grid/drag` attempt was too broad because the filter matched all grid drag cases and
  exceeded 5 minutes. The benchmark was then split so drag runs use a dedicated medium `drag_grid`
  fixture instead of every workload.

## Initial Measurements

Grid fixture:

- document shape: 120 columns x 80 rows;
- indexed records: 19,120;
- viewport: 1,280 x 720 px around `(2400, 1400)`.

| Operation | Current vector | `rstar` candidate | Static AABB candidate | Observation |
| --- | ---: | ---: | ---: | --- |
| Rebuild | 5.36-6.13 ms | 8.15-8.73 ms | 1.87-2.03 ms | Static AABB build is much cheaper than both current vector rebuild and R-tree bulk load for this fixture. |
| Visible query | 25.64-30.41 us | 7.71-9.52 us | 6.19-6.69 us | Both candidates prune better than vector scan; static AABB is fastest here. |
| Sparse hit test | 37.01-40.43 us | 116.96-119.89 ns | 126.07-142.40 ns | AABB candidates avoid the full reverse scan when the point touches few records. |
| Paint-frame culling, current path | 26.34-28.18 us | n/a | n/a | Current paint-frame cost tracks the vector visible-query cost. |

Dedicated drag fixture:

- document shape: 40 columns x 25 rows;
- drag duration: 120 frames;
- note: the simulated candidates still rebuild the oracle index each frame to materialize overlay
  records. These numbers are a correctness-spike upper bound, not the expected runtime cache cost.

| Operation | 10 selected nodes | 100 selected nodes | Observation |
| --- | ---: | ---: | --- |
| Static AABB rebuild each frame | 483.76-554.47 ms | 4.87-5.40 s | Full per-frame rebuild is too expensive for gesture loops. |
| Hybrid overlay simulation | 458.46-486.31 ms | 4.75-5.45 s | Similar upper bound because the spike rebuilds the oracle each frame to extract overlay records. A real runtime overlay must consume document diffs directly. |

## Interpretation

The rebuild and query data favor a static AABB base for stable documents. Static AABB has the best
build time and visible-query time on the grid fixture, which is close to paint-frame culling.

`rstar` remains useful as the dynamic overlay candidate. It is slower to bulk-build than the current
vector in the grid fixture, but point hit tests and sparse spatial queries are excellent. It should
be judged on incremental overlay mutation, not on replacing the entire base snapshot.

The hybrid drag simulation proved the result-merging and stale suppression semantics, but it did
not yet prove final performance. The current spike constructs overlay records by rebuilding the
oracle each frame; the production design must instead receive the actual `CanvasDocumentDiff` /
gesture dirty set and refresh only affected node, handle, shape, and incident-edge records.

## Decision Recommendation

Do not expose public index strategy knobs before the 0.1 release.

The implementation phase following this spike landed these pieces:

1. `CanvasRuntime` owns an internal `CanvasSpatialCache`.
2. `CanvasRuntime::spatial_index()` and `CanvasEditor::index()` were removed so callers use runtime
   query methods instead of bypassing cache ownership.
3. The cache keeps stable base records, overlay records, stale IDs keyed by `CanvasRecordId`, and
   canvas-owned filtering and z-ordering.
4. Runtime diffs materialize changed records through `CanvasGeometryResolver`; node updates also
   refresh current incident edges without rebuilding an oracle index.
5. The parity suite now checks `CanvasRuntime` directly against `SpatialIndex`.

## Risks

| Risk | Severity | Likelihood | Mitigation |
| --- | --- | --- | --- |
| The grid fixture overstates static AABB wins | Medium | Medium | Keep dense-overlap, clustered, long-edge, and mixed workloads in the bench harness; run full results before swapping defaults. |
| Hybrid overlay adds complexity before data is complete | Medium | Medium | Keep it internal and covered by parity tests; do not expose public strategy knobs yet. |
| Drag benchmark is misread as final performance | Medium | High | Treat current drag numbers as upper bounds; implement diff-fed overlay before making a performance claim. |
| Third-party dependency becomes public API by accident | High | Low | Keep candidates dev-only until the runtime strategy is selected and hidden behind semantic runtime internals. |

## Follow-Up Work

- Add benchmark output for clustered, dense-overlap, long-edge, and mixed workloads to this file.
- Measure memory and overlay size thresholds.
- Replace the internal stable-base record vector with a packed static AABB base only after runtime
  benchmark data justifies the dependency and complexity.
- Decide whether semantic presets such as `Auto`, `Simple`, `Dynamic`, `StaticSnapshot`, and
  `Hybrid` are needed after the internal runtime prototype has real data.
