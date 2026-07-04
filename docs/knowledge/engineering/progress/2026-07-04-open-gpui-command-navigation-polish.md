---
type: Work Progress
title: Open GPUI command navigation polish
status: active
timestamp: 2026-07-04T09:15:19+08:00
git_branch: feat/command-navigation-polish
tags:
  - open-gpui
  - command
  - ui-components
  - keyboard-navigation
---

# Summary

`Command` now has explicit palette navigation behavior instead of inheriting every keyboard move
from the underlying `ListboxState`.

The new public `CommandNavigationBehavior` controls two stable policies:

- `loop_navigation`: Up/Down wrap across the first and last focusable command rows by default; apps
  can disable it for bounded palettes.
- `group_navigation`: Alt+Up/Alt+Down jump to the first focusable command in the previous/next
  rendered group by default.

Home/End are handled as command-surface navigation keys, PageUp/PageDown keep their viewport-based
nearest-focusable behavior, and Vim-style Control aliases remain supported.

# Implementation Notes

- `CommandNavigationBehavior` lives with the resolved command model and is exported through the
  crate root, prelude, and default public API surface.
- `Command` exposes `navigation_behavior(...)`, `loop_navigation(...)`, and
  `group_navigation(...)` builders. The resolved `CommandState` exposes matching getters so gallery
  readouts and callers can inspect the active policy.
- Runtime keyboard navigation now resolves command-specific targets before dispatching selection,
  while `ListboxState` stays a reusable lower-level option model. Up/Down adjacent navigation
  reuses the shared roving index helper so single-focusable loop behavior stays aligned with the
  rest of the choice surfaces.
- The component API inventory, public-surface export tests, component contract docs, verification
  docs, and foundation gallery state readouts were updated together.

# Review Notes

Read-only review found and the implementation fixed:

- A single-focusable command list in loop mode now returns the current row instead of ignoring
  Up/Down, so the command surface still consumes the navigation event.
- Focused tests now cover End, bounded Up, Alt+Up event normalization, and
  `group_navigation(false)` for both group-jump directions.
- Engineering memory frontmatter and body branch labels were aligned.

# Next Action

After this slice is committed or merged, the next command ecosystem hardening step should be either
async provider UX polish or real app-shell dogfood around `CommandPaletteController` and
`CommandCenter` integration.

# Citations

- [Command ecosystem docs](../../../ui/command-ecosystem.md)
- [Component contract docs](../../../ui/component-contract.md)
- [Verification docs](../../../verification.md)
- [Verification evidence](../verification/open-gpui-command-navigation-polish-20260704.md)
