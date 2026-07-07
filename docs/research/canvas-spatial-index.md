# Canvas Spatial Index Research

**Date**: 2026-06-08
**Status**: Research complete, runtime cache landed

## Context

`open-gpui-canvas` now routes production queries through `CanvasRuntime` and its runtime query
module. `SpatialIndex` remains a deterministic dev/test oracle and simple fallback model, but app
and paint code should not bypass the runtime-owned cache.

The next performance step should be selected from real canvas workloads, not from a generic spatial
index preference. The hot paths are:

- visible AABB queries for every paint frame;
- pointer hit testing with z-order and locked/hidden filtering;
- incremental updates while dragging nodes and rerouting incident edges;
- long edge bounds and route hit areas;
- large mostly-static documents with small dynamic edit overlays;
- future tile, level-of-detail, and GPU-assisted culling paths.

## Candidate Crates

| Candidate | Current crate metadata | Fit | Notes |
| --- | --- | --- | --- |
| `rstar` | `0.13.0`, `MIT OR Apache-2.0`, Rust `1.85`, R*-tree | Best dynamic general-purpose candidate | Mature R*-tree API, supports envelope-based spatial queries and nearest-neighbor style workloads. Good first spike for replacing the current vector index when incremental inserts/removes matter. |
| `static_aabb2d_index` | `2.0.0`, `MIT OR Apache-2.0`, fast static 2D AABB index | Best minimal static AABB candidate | Closest to the canvas paint-frame query shape: many 2D boxes, mostly query-only between rebuilds. Needs a dynamic overlay strategy for active edits. |
| `packed_spatial_index` | `0.4.1`, `Apache-2.0`, Hilbert-packed static 2D/3D AABB index with parallel/SIMD defaults | Strong static packed-index candidate | Interesting for large immutable snapshots and benchmark comparison. Its Rust `1.89` requirement is above the current workspace baseline implied by dependencies, so adoption may be premature. |
| `geo-index` | `0.3.4`, `MIT OR Apache-2.0`, immutable ABI-stable spatial indexes | Good immutable index candidate | Designed for fast immutable indexes. Worth benchmarking for snapshot-heavy documents, but the geo ecosystem shape may be broader than this crate needs. |
| `parry2d` | `0.28.0`, `Apache-2.0`, 2D collision detection | Useful conceptually, too broad for core default | Its broad-phase structures are aimed at collision systems. Good reference for dynamic broad phase, but it brings collision-domain concepts we do not need in the first canvas index. |
| `kiddo` | `5.3.2`, `MIT OR Apache-2.0`, high-performance k-d tree | Poor fit for primary canvas culling | Excellent for point nearest-neighbor queries, but canvas records are AABBs and route hit areas. Could help snap-to-point or nearest-handle tools later, not the main visible-record index. |

## Algorithm Options

### Dynamic R-tree / R*-tree

Use a dynamic tree such as `rstar` and update moved records incrementally.

Pros:

- Good general fit for insert, remove, move, and AABB query.
- One structure can serve visible queries and hit testing.
- Mature Rust ecosystem support.

Cons:

- Dynamic updates during drag may churn the tree for every pointer move.
- Z-order still needs a separate sort or stable record ordering layer.
- Long edges and inflated hit areas can create broad envelopes that reduce pruning quality.

Best use: medium to large documents with frequent edits where full rebuilds are too expensive.

### Packed Static AABB Index

Build a packed immutable index from the current document snapshot, then query it for visible
records.

Pros:

- Excellent fit for paint-frame culling of large mostly-static documents.
- Build/query behavior is easier to benchmark and reason about.
- Avoids tree mutation complexity in the render path.

Cons:

- Needs rebuild or overlay when records move.
- Dragging many selected nodes requires either transient overlay records or delayed rebuilds.
- Not enough by itself for highly dynamic graph editors.

Best use: large whiteboards, mind maps, and document canvases where most frames are query-only.

### Runtime Hybrid Static Base Plus Dynamic Overlay

Keep a packed immutable base index for the last committed stable document, plus a small dynamic
overlay for currently edited or recently changed records. Queries merge base and overlay results
and deduplicate by `CanvasRecordId`.

Pros:

- Matches canvas behavior: large stable background plus small active edits.
- Keeps paint-frame queries fast while avoiding full rebuilds on every drag update.
- Gives gestures a natural path: overlay during gesture, rebuild base at commit or debounce.

Cons:

- More implementation complexity than a single R-tree.
- Requires careful result deduplication and stale-record suppression.
- Needs benchmark coverage for overlay size thresholds.

Best use: Open GPUI canvas default direction. The current runtime implementation keeps the hybrid
contract internal: stable base records, overlay records for changed items, stale suppression by
`CanvasRecordId`, and final filtering/order in `CanvasRuntimeQuery`.

### Uniform Grid / Tile Index

Bucket records into fixed-size or zoom-aware tiles.

Pros:

- Natural bridge to tile rendering, level-of-detail, and GPU-assisted culling.
- Predictable query behavior for dense, evenly distributed documents.
- Easy to make dirty regions and tile invalidation explicit.

Cons:

- Large records and long routed edges cross many cells.
- Bad cell-size choices cause either over-querying or high insertion cost.
- Less precise than R-tree-like structures without secondary filtering.

Best use: future renderer-level tiling and large-scene invalidation, not the first replacement for
`SpatialIndex`.

### Quadtree

Recursively partition 2D space into quadrants.

Pros:

- Simple mental model for canvas space.
- Can work well for clustered point-like records.

Cons:

- AABBs that straddle quadrant boundaries are awkward.
- Degenerate distributions and large edges need special handling.
- Rust crate maturity is less compelling than R-tree/AABB-index options for this use case.

Best use: reference concept only unless a benchmark proves it beats the packed/R-tree options.

## Recommendation

Do not expose concrete index strategy knobs. The runtime query boundary is the important architecture
seam and it has landed: `CanvasRuntimeQuery` owns final filtering, z-ordering, precise hit tests,
and stale suppression over an internal spatial cache.

Future work should be a focused internal benchmark pass with this order:

1. Run the same documents through the current runtime cache and candidate base/overlay builders.
2. Compare `rstar` as a possible overlay implementation without making it public API.
3. Compare `static_aabb2d_index` or another packed AABB structure as a possible base implementation.
4. If the workspace Rust version can move high enough, compare `packed_spatial_index`.
5. Measure rebuild time, incremental update time, visible query time, hit-test time, memory, and
   overlay compaction thresholds.
6. Keep the current internal strategy if results are close; prefer the simpler cache internals.

The likely long-term architecture is a hybrid:

- static packed AABB index for committed document snapshots, if benchmarks justify replacing the
  current internal base;
- dynamic overlay for active gesture edits and committed diffs, if benchmarks justify replacing the
  current internal overlay;
- tile/LOD layer above that for renderer invalidation and very large scenes;
- optional nearest-point structure only for handle snapping or alignment tools.

## Current Runtime Cache Contract

- Keep `CanvasRuntime` as the cache owner.
- Keep the runtime query module as the final query boundary.
- Do not expose concrete index choices in `CanvasEditor` or `CanvasPaintModel`.
- Preserve z-order hit-test behavior and locked/hidden filtering.
- Suppress stale base records by semantic `CanvasRecordId` when overlay records replace them.
- Refresh incident edge records when node diffs can change routed edge geometry.
- Use focused parity tests before changing base, overlay, compaction, or ordering internals.

## Sources Checked

- `cargo info rstar`: `0.13.0`, `MIT OR Apache-2.0`, R*-tree spatial index.
  Docs: <https://docs.rs/rstar/latest/rstar/>
- `cargo info static_aabb2d_index`: `2.0.0`, `MIT OR Apache-2.0`, fast static 2D AABB index.
  Docs: <https://docs.rs/static_aabb2d_index/latest/static_aabb2d_index/>
- `cargo info packed_spatial_index`: `0.4.1`, `Apache-2.0`, Hilbert-packed static AABB index.
  Docs: <https://docs.rs/packed_spatial_index/latest/packed_spatial_index/>
- `cargo info geo-index`: `0.3.4`, `MIT OR Apache-2.0`, immutable ABI-stable spatial indexes.
  Docs: <https://docs.rs/geo-index/latest/geo_index/>
- `cargo info parry2d`: `0.28.0`, `Apache-2.0`, 2D collision detection.
  Docs: <https://docs.rs/parry2d/latest/parry2d/>
- `cargo info kiddo`: `5.3.2`, `MIT OR Apache-2.0`, k-d tree nearest-neighbor library.
  Docs: <https://docs.rs/kiddo/latest/kiddo/>
