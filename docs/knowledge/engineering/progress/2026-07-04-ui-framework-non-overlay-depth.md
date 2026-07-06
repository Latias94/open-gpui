---
type: "Work Progress"
title: "UI framework non-overlay depth"
description: "Work Progress for UI framework non-overlay depth."
timestamp: 2026-07-04T12:07:44Z
tags: ["ui-components", "gallery", "motion", "public-surface", "non-overlay"]
status: "active"
related_plan: "docs/plans/2026-07-04-002-refactor-ui-framework-non-overlay-depth-plan.md"
git_branch: "refactor/ui-framework-non-overlay-depth"
---

# Summary

Completed the non-overlay UI framework deepening pass on
`refactor/ui-framework-non-overlay-depth`. The pass deliberately kept overlay adapter/runtime
behavior out of scope while tightening choice/search component ownership, default public exports,
motion public surface, and foundation-gallery evidence.

# Details

- U1 characterized choice/search behavior before movement in
  `crates/ui_components/tests/choice.rs`: Listbox navigation skips disabled/separator rows, Select
  trigger and popup state preserve selected/active intent, Combobox filtering preserves hidden
  selected values, and shortcut labels use the platform display helper.
- U2 split Select into `select/{mod,model,style,render_plan,runtime}.rs`. Public Select types still
  flow through the curated component surface, while GPUI rendering and existing overlay calls stay
  in the runtime owner.
- U3 split Combobox into
  `combobox/{mod,descriptor,model,style,render_plan,runtime}.rs`. Query/filter/keyboard semantics
  moved behind model and render-plan owners, with TextInput and popup rendering left in runtime.
- U4 tightened the internal choice boundary by removing the public default
  `listbox_navigation_target` helper and forcing consumers through `ListboxState` or existing
  component-specific navigation helpers.
- U5 narrowed `open_gpui_ui_components` default exports. Command infrastructure now imports from
  `open_gpui_command`, and broad renderer-neutral table/virtualizer/grid infrastructure imports
  from `open_gpui_ui_core`. Component-facing public types remain available from
  `open_gpui_ui_components`.
- U6 made `open_gpui_ui_core::motion_value` private and reduced `MotionValue` to an internal
  sanitized scalar used by `MotionScalarTrack`. Public motion contracts remain
  `MotionScalarTrack`, `MotionScalarController`, `MotionFrameDemand`, `MotionModel`,
  `MotionPreset`, motion policy types, and `MotionProjectionClip`.
- U7 strengthened non-overlay foundation-gallery evidence for Listbox, Select, Combobox, and
  Command. Official story contracts now expose state readout selectors in addition to sample
  selectors, and focused smoke coverage checks that those readouts render.
- Commits landed for each tranche:
  `61517a7`, `4d844a2`, `d0320d5`, `5f8f53e`, `4779460`, `180fd3e`,
  `1522cfb`.

# Next Action

Finish U8 by validating engineering memory, running the final contract/format gates, and committing
the documentation and memory update. Overlay adapter work remains excluded for the user's separate
branch.

# Citations

- `docs/plans/2026-07-04-002-refactor-ui-framework-non-overlay-depth-plan.md`
- `docs/ui/component-contract.md`
- `docs/architecture/native-ui-framework-strategy.md`
- `docs/verification.md`
