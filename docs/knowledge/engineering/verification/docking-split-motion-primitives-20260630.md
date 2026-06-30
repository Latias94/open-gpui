---
type: Verification Evidence
title: Docking split motion primitive verification
status: active
timestamp: 2026-06-30T19:30:00+08:00
source_session: 019f09c8-a122-7d42-b250-053c40f9c513
git_branch: feat/docking-split-motion-primitives
related_plan: docs/plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md
verified_by:
  - cargo fmt --all -- --check
  - cargo nextest run -p open-gpui-ui-core --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast
  - cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests --no-fail-fast
  - cargo check -p open-gpui-docking-native
  - cargo check -p open-gpui-docking --tests
  - cargo nextest run -p open-gpui-docking geometry host_accessibility_tests --no-fail-fast
  - git diff --check
  - python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering
---

# Verification

Current focused verification proves the U10 cleanup path:

- `cargo fmt --all -- --check` passed.
- `cargo nextest run -p open-gpui-ui-core --no-fail-fast` passed with 139/139 tests.
- `cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast`
  passed with 13/13 tests.
- `cargo nextest run -p open-gpui-docking host_presentation_scene_tests host_viewport_preview_visual_tests host_transition_tests host_zoom_focus_tests host_divider_hit_map_tests host_accessibility_tests --no-fail-fast`
  passed with 28/28 tests.
- `cargo check -p open-gpui-docking-native` passed. The command still emits existing macOS/objc
  `unexpected cfg cargo-clippy` warnings plus the known `block` future-incompat warning.
- `cargo check -p open-gpui-docking --tests` passed after removing the old docking-local split
  handle-center and handle-hit geometry path.
- `cargo nextest run -p open-gpui-docking geometry host_accessibility_tests --no-fail-fast` passed
  with 37/37 tests.
- `git diff --check` passed.
- Engineering wiki validation passed.

# Covered Behavior

- Graph layout still resolves pane bounds from sanitized split fractions and central-child
  remaining-space behavior.
- Splitter accessibility descriptors expose orientation, disabled state, and increment/decrement
  actions.
- Selected tabs are marked from focus-region descriptors.
- Overlay drop targets expose custom actions while rejected targets are disabled and actionless.

# Pending Final Gates

- `ce-code-review` and any eligible follow-up fixes.
- Re-run formatting, diff whitespace, and wiki validation after review fixes or before final handoff.

# Citations

- [Plan](../../../plans/2026-06-30-003-refactor-docking-split-motion-primitives-plan.md)
- [Progress note](../progress/2026-06-30-docking-split-motion-primitives.md)
