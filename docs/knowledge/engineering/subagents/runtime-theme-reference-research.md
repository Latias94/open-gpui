---
type: "Subagent Finding"
title: "Runtime Theme Table Reference Research"
description: "Reference guidance for the Open GPUI runtime theme table slice."
tags: ["engineering-memory", "ui", "theme", "subagent"]
timestamp: 2026-06-15T16:20:00Z
status: "active"
---

# Runtime Theme Table Reference Research

The runtime theme table slice should stay intentionally small. The useful boundary is a
theme snapshot that maps `(TokenKey, ColorState)` to concrete colors, with an explicit
mode and revision. Missing entries should fall back to the `ColorIntent` RGB so existing
components keep rendering while theme coverage grows.

## Reference Takeaways

- `repo-ref/gpui-component/crates/ui/src/theme/` is the GPUI-native reference for a
  broader future registry, default theme loading, and theme schema shape.
- `repo-ref/fret/crates/fret-ui/src/theme/mod.rs` is the reference for immutable
  `ThemeSnapshot` values and revision-based cache invalidation.
- U1 should not copy full JSON schemas, filesystem watching, editor/highlight theme
  support, or Fret's broad metric/text-style token engine.

## Decision

This research records the earlier color-snapshot slice. The later U7 authority-convergence
decision deletes the direct default-light compatibility resolver and is authoritative in
`docs/knowledge/engineering/decisions/theme-scope-resolution.md`.

Implement the U1 slice as immutable snapshots plus a resolver API:

- `ColorIntent` keeps `TokenKey`, `ColorState`, and fallback RGB in resolved component state.
- `ThemeSnapshot` exposes `ThemeMode`, `revision`, and color entries.
- `ThemeResolver::resolve_with(intent, snapshot)` resolves from the snapshot first and
  falls back to the intent RGB.
- At this checkpoint, `ThemeResolver::resolve(intent)` remained a default-light compatibility
  path; U7 later removed it without an alias.

App-level theme registries, user theme files, JSON schemas, and hot reload belong in a
later slice after more component contracts exist.
