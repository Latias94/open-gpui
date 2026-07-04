---
type: "Verification Evidence"
title: "UI framework non-overlay depth verification"
description: "Verification Evidence for UI framework non-overlay depth verification."
timestamp: 2026-07-04T12:07:44Z
tags: ["ui-components", "gallery", "motion", "public-surface", "non-overlay"]
status: "verified"
related_plan: "docs/plans/2026-07-04-002-refactor-ui-framework-non-overlay-depth-plan.md"
git_branch: "refactor/ui-framework-non-overlay-depth"
---

# Verification

Focused gates were run per tranche, using `nextest` where it completed and plain `cargo test`
where the requested `nextest` filter stalled during listing.

# Result

Verified for the non-overlay scope. The committed code and docs prove component module splits,
choice/search behavior, default export narrowing, private motion scalar internals, and gallery
state-readout evidence. Overlay adapter/runtime behavior was intentionally not changed.

# Evidence

- `cargo nextest run -p open-gpui-ui-components --test choice --no-fail-fast`: passed 52/52 after
  the choice/search characterization and Select/Combobox module split work.
- `cargo check -p open-gpui-ui-components --tests`: passed after default-surface migration.
- `cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast`: passed after
  source mapping, default export, and owner-crate import changes.
- `cargo check -p open-gpui-ui-core --tests`: passed after making `motion_value` private.
- `cargo nextest run -p open-gpui-ui-core motion motion_controller motion_value motion_policy motion_projection --no-fail-fast`:
  passed 44/44 after the motion scalar cleanup.
- `cargo test -p open-gpui-ui-components splitter -- --nocapture`: passed the splitter-focused
  fallback gate after the equivalent `nextest` filter stalled while listing.
- `cargo test -p open-gpui-docking host_transition_tests -- --nocapture`: passed 12/12 as the
  docking transition fallback gate after the equivalent `nextest` filter stalled while listing.
- `cargo test -p open-gpui-docking host_zoom_focus -- --nocapture`: passed 13/13 as the docking
  zoom/focus fallback gate after the equivalent `nextest` filter stalled while listing.
- `cargo check -p open-gpui-ui-foundation-gallery --tests`: passed after gallery owner imports and
  choice/search state readout contracts were updated.
- `cargo nextest run -p open-gpui-ui-foundation-gallery component --no-fail-fast`: passed 69/69
  with the new choice/search story state-readout coverage.
- `cargo fmt --check`: passed before the U8 docs/memory pass.
- `cargo fmt --all -- --check`: passed after U8 docs/memory updates.
- `cargo run -p xtask -- scan-ui-contract`: passed after the docs described the narrowed default
  surface, choice/search story readouts, and private motion scalar boundary.
- `python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering`:
  passed with pre-existing warnings for stale large rollups and local-path citations outside the
  new sharded memory files.
- `git diff --check`: passed after U8 docs/memory updates.

# Follow-up

No code follow-up remains in this scope. If a later branch wants broader motion ergonomics, it
should start from real Splitter/docking or application consumers rather than reopening
`MotionValue` as a public primitive by default. Overlay adapter/runtime follow-up belongs to the
user's separate overlay branch.

# Citations

- `docs/plans/2026-07-04-002-refactor-ui-framework-non-overlay-depth-plan.md`
- `crates/ui_components/tests/choice.rs`
- `crates/ui_components/tests/public_surface/exports.rs`
- `crates/ui_core/tests/headless_contracts.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery/component_catalog_contracts.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery/component_smoke_shell.rs`
