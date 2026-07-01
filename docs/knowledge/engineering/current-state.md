---
type: Current State
title: Open GPUI main integration state
status: active
timestamp: 2026-07-01T16:05:00+08:00
git_branch: main
related_plan:
  - docs/plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md
  - docs/plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md
  - docs/plans/2026-06-30-001-refactor-ui-architecture-deepening-plan.md
  - docs/plans/2026-06-30-002-refactor-ui-deep-modules-plan.md
  - docs/plans/2026-07-01-001-refactor-ui-contract-test-modules-plan.md
related_adr:
  - docs/adr/0010-docking-presentation-scene-motion-model.md
  - docs/adr/0011-docking-split-motion-primitive-boundary.md
  - docs/adr/0012-docking-runtime-capability-alignment.md
verified_by:
  - cargo fmt --all -- --check
  - cargo nextest run -p open-gpui-ui-core split motion --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests host_interaction_tests workspace_resize_policy_tests --no-fail-fast
  - cargo check -p open-gpui-docking-native
  - git diff --check
  - python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering
---

# Current State

- Branch: `main`; resolving integration of the local docking runtime capability commits with the newer `origin/main` UI architecture/test-module refactors.
- Done: `docs/plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md` is implemented and committed locally. The docking runtime now has sampled transition execution, scene-owned tab insertion/payload preview descriptors, zoom/unzoom and focus presentation, GPUI-facing accessibility descriptors, shared split primitive consumption, visible corner drag, routed overlay cleanup, native dogfood proof, ADR 0012, and U11 cleanup.
- Done: Public docking focus commands were corrected after `$emil-design-eng` / `$review-animations` review: `focus_pane` and `focus_neighbor_pane` use immediate focus-ring semantic feedback because they are high-frequency and keyboard reachable; explicit internal/test `MotionSpec` entry points remain available for lower-frequency focus-ring transition proofs.
- Done: The newer UI architecture line from `origin/main` splits `ui_components` contract tests into focused modules, moves public-surface inventory helpers under `tests/support`, centralizes overlay/choice/text/table/theme seams, and keeps `components.rs` deleted. Docking splitter coverage should live in `crates/ui_components/tests/layout.rs` and public inventory coverage in `crates/ui_components/tests/support/public_surface.rs`.
- In progress: Local `main` is being merged with `origin/main`. Conflict resolution should preserve the remote UI test-module split and explicit `ui_core` root export vocabulary while keeping the local docking `MotionSpec` and split primitive exports.
- Blocked: None.
- Next action: finish the merge, run focused split/motion/docking gates plus wiki/diff checks, then push `main`.

# Citations

- [Runtime capability follow-up plan](../../plans/2026-06-30-004-refactor-docking-runtime-capability-alignment-plan.md)
- [ADR 0012](../../adr/0012-docking-runtime-capability-alignment.md)
- [Runtime capability verification evidence](verification/docking-runtime-capability-alignment-20260701.md)
- [UI contract module refactor plan](../../plans/2026-07-01-001-refactor-ui-contract-test-modules-plan.md)
