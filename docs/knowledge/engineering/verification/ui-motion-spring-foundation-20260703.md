---
type: Verification Evidence
title: UI motion spring foundation verification
status: verified
timestamp: 2026-07-03T23:59:00+08:00
git_branch: feat/ui-motion-spring-foundation
related_plan: docs/plans/2026-07-03-004-refactor-ui-motion-spring-foundation-plan.md
related_adr:
  - docs/adr/0015-ui-motion-runtime-foundation.md
  - docs/adr/0016-ui-motion-spring-foundation.md
verified_by:
  - cargo nextest run -p open-gpui-ui-core motion spring projection policy --no-fail-fast
  - cargo nextest run -p open-gpui-ui-core spring --no-fail-fast
  - cargo nextest run -p open-gpui-ui-core projection --no-fail-fast
  - cargo nextest run -p open-gpui-ui-core controller --no-fail-fast
  - cargo nextest run -p open-gpui-ui-core policy --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components splitter --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_transition_tests host_render_tests host_zoom_focus_tests host_interaction_tests --no-fail-fast
  - cargo nextest run -p open-gpui-docking --no-fail-fast
  - cargo check -p open-gpui-docking-native --bin open-gpui-docking-native
  - cargo nextest run -p open-gpui-docking-native runtime_status_panel_formats_platform_capabilities --no-fail-fast
  - cargo fmt --all -- --check
  - git diff --check
  - python "$HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py" validate --root docs/knowledge/engineering
tags:
  - ui-core
  - motion
  - spring
  - projection
  - splitter
  - docking
---

# Verification

Focused gates passed for the new renderer-neutral spring sampler, projection primitive, scalar
controller, and motion policy validator. Splitter coverage passed after programmatic splitter
motion moved to the shared scalar controller and default layout spring while pointer drag remained
immediate.

Docking transition/render/zoom/interaction coverage passed after the transition executor moved to a
shared scalar motion track, default specs began using spring progress, explicit custom specs kept
timeline semantics, and pane/divider move/resize samples switched to projection-derived bounds.

# Behavior Locked

- Spring samples expose value, velocity, rest completion, cancellation, retarget, and reduced-motion
  final semantics.
- Projection samples keep target bounds as the semantic layout while describing translation, scale,
  tree-scale correction, and reveal geometry.
- Motion policy rejects pointer-drag spatial smoothing, keyboard-focus spatial motion, overlong UI
  motion without continuity context, excessive bounce, missing reduced-motion final state, and
  unrelated-target preview interpolation.
- Splitter programmatic motion can animate through shared primitives; pointer drag cancels/bypasses
  active motion and keeps current fractions direct.
- Docking custom timeline specs remain honored. Default transition specs can use spring progress,
  while preview release authority stays in current facts rather than motion samples.

# Final Verification

Final automated gates passed:

- `cargo nextest run -p open-gpui-ui-core motion spring projection policy --no-fail-fast`
- `cargo nextest run -p open-gpui-ui-components splitter --no-fail-fast`
- `cargo nextest run -p open-gpui-docking --no-fail-fast`
- `cargo check -p open-gpui-docking-native --bin open-gpui-docking-native`
- `cargo nextest run -p open-gpui-docking-native runtime_status_panel_formats_platform_capabilities --no-fail-fast`
- `cargo fmt --all -- --check`
- `git diff --check`
- `python "$HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py" validate --root docs/knowledge/engineering`

# Citations

- [Plan](../../../plans/2026-07-03-004-refactor-ui-motion-spring-foundation-plan.md)
- [ADR 0016](../../../adr/0016-ui-motion-spring-foundation.md)
- [Verification docs](../../../verification.md)
