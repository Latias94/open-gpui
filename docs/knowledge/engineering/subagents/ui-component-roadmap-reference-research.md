---
type: "Subagent Finding"
title: "UI component roadmap reference research"
description: "Read-only reference repository research for the next official Open GPUI component roadmap."
tags: ["ui-components", "roadmap", "reference-research", "subagent"]
timestamp: 2026-06-15T15:47:00Z
subagent_id: "/root/ui_reference_research/ui_reference_research"
related_plan: docs/plans/2026-06-15-004-feat-ui-component-roadmap-plan.md
---

# Finding

The next component roadmap should use `repo-ref/gpui-component` as the primary GPUI-native
implementation reference and `repo-ref/fret/ecosystem/fret-ui-kit` as the primary policy-layer
reference for theme, overlay, headless behavior, and conformance. `fret-ui-shadcn`, shadcn/ui, and
daisyUI are useful taxonomy and theme references, but should not become runtime architecture.

# Evidence

- `repo-ref/gpui-component/crates/ui/src/input/state.rs` implements the real GPUI text input path
  through `EntityInputHandler`, with UTF-16 mapping, selected and marked ranges, IME behavior,
  bounds lookup, and character-position lookup.
- `repo-ref/gpui-component/crates/ui/src/theme/` contains a GPUI-native theme model with mode,
  registry, schema, default themes, scrollbar appearance sync, radii, fonts, and theme colors.
- `repo-ref/gpui-component/crates/ui/src/popover.rs`,
  `repo-ref/gpui-component/crates/ui/src/dialog/dialog.rs`, and
  `repo-ref/gpui-component/crates/ui/src/menu/popup_menu.rs` show GPUI-native overlay and menu
  anatomy, but mix enough concrete runtime detail that Open GPUI should port selectively.
- `repo-ref/fret/ecosystem/fret-ui-kit/src/overlay_controller.rs` and
  `repo-ref/fret/ecosystem/fret-ui-kit/src/window_overlays/` provide stronger policy separation
  for overlay stack, modal/non-modal behavior, dismissal, and focus restore.
- `repo-ref/fret/apps/fret-ui-gallery/` and its tests show a mature gallery/conformance pattern
  with stable ids, docs surface checks, and drift detection.
- `repo-ref/zed/crates/ui/src/components/popover_menu.rs` and
  `repo-ref/zed/crates/ui/src/components/context_menu.rs` provide production GPUI examples for
  popover/menu handles, submenu behavior, and keyboard navigation.

# Recommendation

Keep the roadmap sequence infrastructure-first:

1. runtime theme table;
2. real editable `TextInput` controller;
3. form controls and labels;
4. roving focus and Tabs;
5. shared overlay behavior;
6. overlay components;
7. layout and shell components;
8. gallery conformance;
9. headless extraction review.

Do not copy `fret`'s cross-runtime `UiHost`, widget tree, or immediate-mode facade into the Open
GPUI component crate. Do not copy `gpui-component`'s full `InputState` wholesale because it includes
editor-grade capabilities such as LSP, diagnostics, completion, folding, and rich popovers that
belong in optional editor extensions, not base TextInput.

# Disposition

Accepted into `docs/plans/2026-06-15-004-feat-ui-component-roadmap-plan.md`. The plan keeps
`open-gpui-ui-headless` deferred until repeated contracts exist across several component families
and keeps reference repositories as inputs rather than runtime dependencies.

# Citations

[1] [Roadmap plan](../../plans/2026-06-15-004-feat-ui-component-roadmap-plan.md)
[2] [ADR 0005](../../adr/0005-open-gpui-official-component-architecture.md)
[3] [Component contract](../../ui/component-contract.md)
