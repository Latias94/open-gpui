# Zed GPUI Import

**Date**: 2026-06-06
**Source**: `repo-ref/zed`
**Decision**: ADR 0001

## Imported Crates

The first import copied the Apache-2.0 GPUI framework closure from Zed into this workspace:

- `collections`
- `gpui`
- `gpui_linux`
- `gpui_macos`
- `gpui_macros`
- `gpui_platform`
- `gpui_shared_string`
- `gpui_util`
- `gpui_web`
- `gpui_wgpu`
- `gpui_windows`
- `http_client`
- `http_client_tls`
- `media`
- `refineable`
- `refineable/derive_refineable`
- `reqwest_client`
- `scheduler`
- `sum_tree`
- `util`
- `util_macros`

## Import Adjustments

- Rebuilt the root Cargo workspace dependency table instead of copying Zed's full workspace
  manifest. This keeps Zed editor application crates out of the dependency graph.
- Replaced `sum_tree`'s `ztracing::instrument` import with `tracing::instrument`.
- Removed `sum_tree`'s test dependency on `zlog`.
- Removed `util_macros`' dependency on Zed's `perf` tooling crate and kept the `perf` attribute
  self-contained behind the existing `perf-enabled` feature.
- Declared `cfg(rust_analyzer)` as an expected workspace cfg because GPUI uses it in debug
  inspector-related code.

## Known Follow-Ups

- Continue monitoring Open GPUI-owned fork dependencies: `font-kit` and `open-gpui-scap`.
- Clean up Zed product naming in comments, examples, window class names, and README content.
- Decide whether platform backend crates should be published as-is or renamed under the Open GPUI
  package strategy.
- Import or fork `gpui-component` only after a native Open GPUI example builds and runs from this
  workspace.

## November 5 Provenance Reconstruction

The initial import commit is `c9571c486612f1bd9194faa028709e89e3f58b7e`; it did not record the
exact Zed checkout SHA. A content comparison found Zed
`6e9465a4288c332208643892e23b9d35d7be5c79` reproduces the imported closure apart from the five
documented tracing/perf adaptations and license-file mode changes. Its parent has identical
contents throughout that closure, so the original monorepo SHA remains unknown. Use this as a
content-verified reconstruction anchor rather than an asserted source-history parent. The
[current sync queue](zed-gpui-upstream-sync-candidates.md#fixed-revisions-and-provenance) owns the
complete evidence, later comparison pins and fixed maintenance target.
