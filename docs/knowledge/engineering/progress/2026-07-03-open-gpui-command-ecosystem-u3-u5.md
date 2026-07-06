---
type: Work Progress
title: Open GPUI command ecosystem U3-U5
status: verified
timestamp: 2026-07-03T23:20:00+08:00
git_branch: feat/open-gpui-command-ecosystem
related_plan: docs/plans/2026-07-03-001-feat-open-gpui-command-ecosystem-plan.md
---

# Summary

Implemented the GPUI-facing command ecosystem slice after the initial registry work.

- `open_gpui_ui_components::gpui_adapter::GpuiCommandAction` stores one stable command id plus a
  GPUI `Action` prototype.
- `GpuiCommandActionMap` maps command ids to GPUI actions, projects shortcut labels from app
  `Keymap` or focused `Window` precedence, builds registry-backed `CommandIndexSnapshot` values,
  and dispatches `CommandSelection` values through `App` or `Window`.
- Public-surface contract rows mark the new command adapter APIs as adapter-only under
  `open_gpui_ui_components::gpui_adapter`.
- The foundation gallery now has a fifth command sample, `registry-dispatch`, that uses a
  registry snapshot plus keymap projection and records the resolved command id intended for
  dispatch.
- `docs/ui/command-ecosystem.md` documents the boundary between GPUI action/keymap execution,
  core command metadata/registry snapshots, and component adapter projection.

# Design Notes

The UI crates still do not own a global command runtime. Apps remain responsible for:

- focus/window routing;
- modal editing or Vim-mode state;
- chord policy;
- command enablement and async execution.

The shared join key is the command id. Registry descriptors, shortcut projection, palette
selection, menu projection, and dispatch adapters all meet on that id.

# Verified

The U3-U5 slice passed focused component, core, gallery, public-surface, formatting, wiki, and
diff-whitespace gates. See the paired verification evidence for exact commands.

# Next Action

Commit the U3-U5 slice.

# Citations

- [Plan](../../../plans/2026-07-03-001-feat-open-gpui-command-ecosystem-plan.md)
- [Command ecosystem docs](../../../ui/command-ecosystem.md)
- [Adapter implementation](../../../../crates/ui_components/src/command/gpui_adapter.rs)
- [Gallery sample](../../../../examples/ui-foundation-gallery/src/pages/components/samples/choice.rs)
