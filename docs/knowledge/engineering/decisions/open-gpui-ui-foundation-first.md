---
type: "Decision"
title: "Open GPUI UI foundation first"
description: "Prioritize accessibility, focus, overlay, tokens, sizing, density, and adaptive layout before broad component rollout."
timestamp: 2026-06-15T04:16:47Z
tags: ["open-gpui", "ui", "a11y", "foundation", "adr"]
status: "active"
source_session: "019ec6c8-5566-7062-8458-21ebe1360573"
git_branch: "main"
git_commit: "6d1caf947e1116419a7e55a1d3636712947541d0"
---

# Decision

Open GPUI should treat accessibility, focus, overlay, tokens, sizing, density, and adaptive layout as the
foundation layer for its component ecosystem before broadening the styled component surface.

# Context

The current `open-gpui` runtime already exposes the primitives needed for a UI ecosystem, but it does not
yet have a first-party component foundation. The local `fret-ui-kit` and the existing GPUI accessibility,
focus, and overlay code show that the right next step is to harden the shared base before growing a large
component catalog.

The repository reference set now matters too: `../../../../../fret` is the strongest local
architecture reference, `../../../../../fret/ecosystem/fret-ui-kit` is the strongest
foundation-layer reference, `repo-ref/gpui-component` is the best GPUI-native implementation seed,
and Flutter / Jetpack Compose / Radix UI / React Aria / React Spectrum / shadcn/ui / Apple HIG /
SwiftUI are the broader comparative references.

For the first real consumer of `open-gpui-ui-core`, use a dedicated pure-foundation gallery example
instead of repurposing `examples/smoke-native`. That keeps the foundation signal clean and makes the
consumer itself reusable as a dogfood surface.

# Alternatives

- Build components breadth-first and add foundation helpers only when a component demands them.
- Copy a broader component surface from `gpui-component` and split it later.
- Make the foundation explicit first, then grow a component crate on top of it.

The foundation-first path is the chosen direction because it keeps a11y and keyboard behavior in the base
layer instead of scattering them through each component.

# Consequences

- The first deliverable should be a UI foundation slice, not a large catalog of visual components.
- Accessibility and keyboard contracts become part of the base contract for every later component.
- Wenli and similar apps get a reusable shell foundation instead of one-off UI code.
- The component ecosystem stays aligned with GPUI's runtime boundaries instead of becoming a second runtime.

# Citations

[1] [ADR 0004](../../../adr/0004-open-gpui-component-library-strategy.md)
[2] [Current State](../current-state.md)
