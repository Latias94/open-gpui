---
type: Verification Evidence
title: UI motion value foundation verification
status: verified
timestamp: 2026-07-04T05:45:00+08:00
git_branch: feat/ui-motion-value-foundation
related_plan: docs/plans/2026-07-04-001-refactor-ui-motion-value-foundation-plan.md
related_adr:
  - docs/adr/0017-ui-motion-value-foundation.md
verified_by:
  - cargo nextest run -p open-gpui-ui-core motion_value motion_controller motion_policy --no-fail-fast
  - cargo nextest run -p open-gpui-ui-core motion_projection motion_controller --no-fail-fast
  - cargo nextest run -p open-gpui-ui-components runtime_panel_identity_changes_sync_immediately --no-fail-fast
  - cargo test -p open-gpui-ui-components splitter::tests::runtime --lib
  - cargo test -p open-gpui-ui-components --test public_surface public_surface::inventory::component_api_inventory_tracks_public_method_surface -- --exact --test-threads=1
  - cargo test -p open-gpui-ui-components --test public_surface public_surface::exports::root_and_prelude_exports_match_contract_default_surface_intent -- --exact --test-threads=1
  - cargo test -p open-gpui-ui-components --test public_surface public_surface::docs::component_contract_docs_match_current_public_surface_vocabulary -- --exact --test-threads=1
  - cargo nextest run -p open-gpui-docking host_transition_tests host_render_tests host_zoom_focus_tests host_viewport_preview_visual_tests host_accessibility_tests --no-fail-fast
  - cargo check -p open-gpui-docking-native --bin open-gpui-docking-native
  - cargo nextest run -p open-gpui-docking-native runtime_status_panel_formats_platform_capabilities --no-fail-fast
  - cargo check -p open-gpui-ui-core -p open-gpui-ui-components -p open-gpui-docking
  - cargo fmt --all -- --check
  - python "$HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py" validate --root docs/knowledge/engineering
  - git diff --check
tags:
  - ui-core
  - motion
  - value
  - projection
  - splitter
  - docking
---

# Verification

Focused gates passed for the UI motion value foundation. The code now separates explicit motion
model resolution, run state, scalar value state, scalar samples, frame demand reasons, policy gates,
and projection visual bounds.

`cargo nextest run -p open-gpui-ui-components splitter component_api_inventory --no-fail-fast` was
interrupted after the test binary startup stalled without failure output; the equivalent Splitter
runtime and public-surface inventory/export/docs checks above were run directly and passed.

# Behavior Locked

- `MotionPreset` resolves default committed-layout, continuity, and affordance motion explicitly.
- `MotionValue` is proof-gated behind the scalar controller path and is not exported through
  root/prelude defaults.
- `MotionScalarSample` is model-neutral and can represent either timeline or spring-backed scalar
  motion.
- Splitter programmatic motion uses an explicit `motion_preference` render-path entry point; panel
  identity/count changes are immediate until insert/remove transitions are genuinely implemented.
- Docking moving/resizing panes render final-size content through projection clip/occlusion layers,
  while the final presentation scene remains the semantic authority.
- Docking custom timeline specs remain timeline-backed, default continuity paths use explicit
  spring presets, and invalid policy reports downgrade to immediate final-state samples.

# Citations

- [Plan](../../../plans/2026-07-04-001-refactor-ui-motion-value-foundation-plan.md)
- [ADR 0017](../../../adr/0017-ui-motion-value-foundation.md)
- [Verification docs](../../../verification.md)
