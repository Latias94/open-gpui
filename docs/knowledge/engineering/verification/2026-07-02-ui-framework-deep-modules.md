---
type: Verification Evidence
title: UI framework deep modules verification
status: verified
timestamp: 2026-07-03T01:57:49+08:00
git_branch: refactor/ui-framework-deepening
related_plan: docs/plans/2026-07-02-003-refactor-ui-framework-deep-modules-plan.md
---

# Verified

- `cargo fmt --all`
- `cargo fmt --all --check`
- `cargo check -p open-gpui-ui-core --tests`
- `cargo check -p open-gpui-ui-components --tests`
- `cargo check -p open-gpui-ui-foundation-gallery --tests`
- `cargo nextest run -p open-gpui-ui-core overlay grid_viewport command --no-fail-fast`
- `cargo nextest run -p open-gpui-ui-components theme a11y menu context_menu command --no-fail-fast`
- `cargo nextest run -p open-gpui-ui-components command_descriptors --no-fail-fast`
- `cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast`
- `cargo nextest run -p open-gpui-ui-foundation-gallery component --no-fail-fast`
- `cargo run -p xtask -- scan-theme-drift`
- `cargo run -p xtask -- scan-theme-schema`
- `cargo run -p xtask -- scan-ui-contract`
- No production matches from `rg -n "ThemeResolver::resolve\(" crates/ui_components/src -g "*.rs"`
- Only `focus.rs` compatibility matches from `rg -n "focus_ring_shadow\(|ThemeContext::light\(\)" crates/ui_components/src -g "*.rs"`
- `cargo run -p xtask -- verify`
- `python $HOME/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering`
- `git diff --check`

# Evidence Scope

- Theme rendering: production component render paths now use `ThemeResolver::current(cx)` /
  `ThemeContext` or an explicit snapshot; direct `ThemeResolver::resolve(...)` is retained only as
  a default-light compatibility path and has no hits under `crates/ui_components/src`.
  Focus-ring painting now uses `focus_ring_shadow_with_theme` from production render paths; the
  default-light `focus_ring_shadow` compatibility helper is fenced to `focus.rs` by
  `production_render_paths_do_not_use_default_light_focus_ring_helper`.
- Overlay placement: `open_gpui_ui_core::overlay::resolve_overlay_placement` owns neutral
  side/alignment/fit/safe-bounds resolution for explicit placement inputs, while
  `open_gpui_ui_components::overlay` owns GPUI layer host mapping. Trigger-anchored components still
  rely on GPUI for final live measured placement until a measured overlay runtime exists.
- Viewport projection: `open_gpui_ui_core::grid_viewport::RowWindow` and `RowWindowItem` are the
  shared renderer-neutral row-window projection used by Table, VirtualizedList, and Tree.
- Gallery contract: component focus, selector, sample, and state-readout traversal derives from
  `StoryContract` through `component_story_contract_for(name)` and
  `component_story_contracts_for_focus(mode)`.
- Historical command contract: `open_gpui_ui_core::CommandDescriptor` projected one-item
  app-command metadata into Command, Menu, and ContextMenu without adding a global command registry
  or dispatch runtime. Current ownership has moved to `open_gpui_command::CommandDescriptor`.
  Its `menu_path` remains app-owned grouping metadata, not an automatic submenu builder.

# Notes

- `cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast` initially
  exposed stale documentation vocabulary from the pre-refactor row-window and theme contract. The
  public-surface doc sentinel now tracks `ThemeRuntime`, `RowWindow`, shared overlay placement,
  `CommandDescriptor`, and gallery story-contract helpers.
- Code review found one real runtime-theme regression: production focus-ring rendering still used
  the default-light `focus_ring_shadow` helper. The fix migrated component and gallery render paths
  to `focus_ring_shadow_with_theme`, exported and classified that helper as an adapter-only surface,
  and added a public-surface source guard.
- Code review also found two P2 boundary overclaims. Documentation now states that the neutral
  overlay solver covers explicit placement inputs while trigger-anchored components still depend on
  GPUI live measurement, and that `CommandDescriptor.menu_path` is app-owned grouping metadata
  rather than an automatic submenu builder.
- `origin/main` was fetched during closeout; `git rev-list --count HEAD..origin/main` returned `0`,
  so the feature branch did not need another merge from `main`.

# Citations

- [UI framework deep modules plan](../../../plans/2026-07-02-003-refactor-ui-framework-deep-modules-plan.md)
- [Component contract docs](../../../ui/component-contract.md)
- [Verification docs](../../../verification.md)
