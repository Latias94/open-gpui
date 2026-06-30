---
type: Current State
title: Docking split motion primitive refactor state
status: active
timestamp: 2026-06-30T19:30:00+08:00
source_session: 019f09c8-a122-7d42-b250-053c40f9c513
git_branch: feat/docking-split-motion-primitives
related_plan: docs/plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md
related_adr:
  - docs/adr/0010-docking-presentation-scene-motion-model.md
  - docs/adr/0011-docking-split-motion-primitive-boundary.md
verified_by:
  - cargo check -p open-gpui-docking --tests
  - cargo nextest run -p open-gpui-docking geometry host_accessibility_tests --no-fail-fast
  - cargo fmt --all -- --check
  - cargo nextest run -p open-gpui-ui-core --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests --no-fail-fast
  - cargo check -p open-gpui-docking-native
  - git diff --check
  - python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering
---

# Current State

- Goal: Complete `docs/plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md`
  through its Definition of Done using `compound-engineering:ce-work`.
- Branch: `feat/docking-split-motion-primitives`.
- Done: U1-U9 are committed through `8373b99` (`refactor(docking): unify divider resize
  transactions`).
- Done: Shared split and motion primitives now live in `open_gpui_ui_core`; `ui_components::Splitter`
  renders through core state; docking presentation, overlay, motion, zoom/focus, divider hit maps,
  and resize transactions consume those primitives.
- In progress: U10 accessibility, documentation, deletion cleanup, final verification, and review.
- Latest cleanup: docking removed the old split handle-center / handle-hit geometry path; graph
  layout now keeps only pane-bound calculation while divider and junction hit testing flows through
  presentation scene data plus `SplitterHitMap`.
- Last verified: the plan's required focused gates pass locally, including full
  `open-gpui-ui-core`, focused `open-gpui-ui-components` splitter/API inventory, docking
  presentation/preview/motion/zoom/divider/a11y, native docking check, formatting, diff whitespace,
  and engineering wiki validation. `cargo check -p open-gpui-docking-native` still reports existing
  macOS/objc `unexpected cfg cargo-clippy` warnings.
- Blocked: None.
- Next action: finish U10 docs and memory, run the plan's required gates, perform `ce-code-review`,
  apply eligible findings, commit, and mark the goal complete only after the final gates pass.

# Citations

- [Plan](../../plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md)
- [ADR 0010](../../adr/0010-docking-presentation-scene-motion-model.md)
- [ADR 0011](../../adr/0011-docking-split-motion-primitive-boundary.md)
- [Progress note](progress/2026-06-30-docking-split-motion-primitives.md)
- [Verification evidence](verification/docking-split-motion-primitives-20260630.md)
