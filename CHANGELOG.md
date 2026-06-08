# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added `open-gpui-canvas` as a reusable pre-1.0 infinite canvas foundation with separated nodes,
  edges, shapes, handles, viewport transforms, GPUI batched paint, JSON Canvas import/export, and a
  native smoke example.
- Added a document mutation journal that returns committed mutations with the applied transaction,
  inverse transaction, actual document diff, and actual semantic record operation batch.
- Added `CanvasEditor` as the consistency boundary for document mutation, selection, history,
  gestures, runtime cache updates, edge router policy, and kind registry policy.
- Added first-class gesture sessions so tools can express transient update, commit, and cancel
  semantics without hand-rolling inverse transactions.
- Added `CanvasRuntime` for spatial, graph, and edge-geometry runtime caches.
- Added `CanvasGeometryResolver` so culling, hit testing, endpoint picking, connection previews,
  routing, and GPUI paint share route and geometry semantics.
- Added `CanvasKindRegistry` for per-kind defaults, migrations, validation, and geometry hooks over
  the open `kind: String` plus JSON payload model.
- Added checkpoint and transaction-log persistence boundaries, typed and byte-store adapters, and an
  in-memory persistence store.
- Added canvas spatial-index research covering dynamic R*-trees, packed static AABB indexes,
  hybrid overlays, tile indexes, quadtrees, and candidate Rust crates.

### Changed

- Tightened pre-release canvas APIs by removing compatibility constructors that accepted
  caller-supplied `SpatialIndex` values. Runtime cache ownership now stays centralized in
  `CanvasRuntime`.
- Persistent undo and redo now append the prepared mutation to the log and then apply the same
  prepared mutation in memory, avoiding a second prepare/apply pass.

## [0.1.0] - 2026-06-07

### Added

- Root-level fork attribution and licensing notes, plus per-crate `NOTICE` files that preserve
  upstream copyright notices.

- A publish-check workflow that validates leaf crate packaging first and package contents for the
  rest of the workspace.

### Changed

- Public package names and Rust import paths are standardized around the `open-gpui` /
  `open_gpui::...` branding.
- Workspace metadata is aligned to the fork author and unified version line for the first release.
- Fork dependencies now resolve from crates.io via `open-gpui-scap` and `open-gpui-font-kit`, and publishable Open GPUI crates no longer inherit the workspace root's `publish = false` guard.
