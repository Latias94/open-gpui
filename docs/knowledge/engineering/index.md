# Engineering Memory

## Core

* [Current State](current-state.md) - Short durable summary of the active engineering state.
* [Update Log](log.md) - Chronological history of meaningful memory updates.
* [Open GPUI native UI framework distribution strategy](decisions/open-gpui-native-ui-framework-distribution-strategy.md) - Decision memory for Cargo-first crate distribution, source inspection, typed contract checks, gallery, theme, a11y, and verification.
* [ADR 0014: Remove Open GPUI Native UI Hybrid Registry](../../adr/0014-remove-native-ui-hybrid-registry.md) - Current decision removing the generated registry manifest, scaffold recipes, artifacts, and scan command.
* [ADR 0013: Open GPUI Native UI Hybrid Registry](../../adr/0013-open-gpui-native-ui-hybrid-registry.md) - Superseded decision for the removed hybrid registry experiment.
* [ADR 0015: UI Motion Runtime Foundation](../../adr/0015-ui-motion-runtime-foundation.md) - Current renderer-neutral motion timeline and retarget boundary for Splitter and docking.
* [Native UI framework design research handoff](sessions/2026-07-02-native-ui-framework-design-research-handoff.md) - Handoff for the 28-item native UI framework design research report and next architecture step.
* [Native UI framework research report verification](verification/native-ui-framework-research-report-20260702.md) - Verification evidence for the generated research report and JSON field coverage.
* [Native UI hybrid registry architecture planning](progress/2026-07-02-native-ui-hybrid-registry-architecture-plan.md) - Superseded historical plan for the removed hybrid registry MVP.
* [Native UI hybrid registry implementation](progress/2026-07-02-native-ui-hybrid-registry-implementation.md) - Superseded implementation history for the removed registry manifest, recipes, artifacts, and scan.
* [Native UI hybrid registry implementation verification](verification/native-ui-hybrid-registry-implementation-20260702.md) - Superseded verification evidence for the removed hybrid registry work.
* [Open GPUI UI foundation first](decisions/open-gpui-ui-foundation-first.md) - Decision to prioritize accessibility, focus, overlay, tokens, sizing, density, and adaptive layout before broad component rollout.
* [Open GPUI UI component productization roadmap](decisions/open-gpui-ui-productization-roadmap.md) - Decision to treat current UI crates as the active product boundary and defer standalone headless extraction.
* [Open GPUI UI component depth roadmap](decisions/open-gpui-ui-component-depth-roadmap.md) - Decision to deepen Command, Menu, Table, and Tree before adding more shallow primitives.
* [Table and virtualizer roadmap framing](progress/2026-06-21-table-virtualizer-roadmap-framing.md) - Planning note for the next table / virtualizer series using fret and TanStack references.
* [Gallery selector unification handoff](sessions/2026-06-17-gallery-selector-unification-and-verification-handoff.md) - Session handoff for the gallery selector contract unification and verification pass.
* [Open GPUI component library planning handoff](sessions/open-gpui-component-library-handoff.md) - Session handoff for the ADR and UI foundation sequencing.
* [Gallery scroll and viewport hardening session handoff](sessions/2026-06-21-gallery-scroll-viewport-hardening.md) - Session handoff for the Components-page scroll regression slice.
* [Menu runtime focus and current repo state](sessions/2026-06-20-menu-runtime-focus-and-current-repo-state.md) - Session handoff for the focused menu/context-menu repair and current repo state.
* [Text input patterns research](subagents/text-input-patterns.md) - Subagent finding on GPUI text input primitives and the TextInput/Field boundary.
* [Text input controller research](subagents/text-input-controller-research.md) - Subagent finding on the minimal editable TextInputController slice.
* [UI component roadmap reference research](subagents/ui-component-roadmap-reference-research.md) - Reference repository findings for the next official component roadmap.
* [Gallery architecture review 2026-06-18](subagents/gallery-architecture-review-20260618.md) - Subagent finding on remaining deletion seams in the UI foundation gallery.
* [U5 focused Components Tree smoke review](subagents/u5-focused-components-tree-smoke-review.md) - Subagent finding on the focused-mode Tree gallery smoke and root click-to-focus behavior.
* [Menu runtime focus regression verification](verification/menu-runtime-focus-regression-20260620.md) - Verification evidence for the menu/context-menu runtime focus repair.
* [Gallery scroll and viewport hardening verification](verification/gallery-scroll-viewport-hardening-20260621.md) - Verification evidence for navigation rail, ScrollArea, and vertical Tabs scroll regressions.
* [Docking nested inner-edge ImGui alignment verification](verification/docking-nested-inner-edge-20260628.md) - Verification evidence for mixed-axis nested inner-edge docking staying scoped to the hit leaf.
* [Docking presentation prior art synthesis](subagents/docking-presentation-prior-art-20260630.md) - Subagent synthesis of SuperSplit, BonSplit, and current docking UI/UX capability gaps.
* [Docking runtime capability follow-up synthesis](subagents/docking-runtime-capability-followup-20260630.md) - Subagent and local synthesis for the post-merge runtime animation, tab insertion, accessibility, split primitive, and dogfood plan.
* [Docking presentation scene and motion model planning](progress/2026-06-30-docking-presentation-scene-motion-plan.md) - Planning note for the next docking presentation scene, overlay, motion, zoom/focus, divider, and accessibility refactor.
* [Docking split motion primitive refactor](progress/2026-06-30-docking-split-motion-primitives.md) - Current progress note for the shared split/motion primitive boundary and U10 cleanup.
* [Docking split motion primitive verification](verification/docking-split-motion-primitives-20260630.md) - Verification evidence for the split/motion primitive refactor gates.
* [Docking flat motion runtime framework implementation](progress/2026-07-02-docking-flat-motion-runtime-plan.md) - Implementation state for flat render authority, real-content transition reveal, overlay motion, retargeting, split motion, and zoom/focus polish.
* [Docking flat motion runtime verification](verification/docking-flat-motion-runtime-20260702.md) - Verification evidence for focused and final flat motion runtime gates.
* [UI motion runtime foundation](progress/2026-07-02-ui-motion-runtime-foundation.md) - Progress note for the shared `ui_core` motion timeline and retarget primitive used by Splitter and docking.
* [Tree renderer productization verification](verification/tree-renderer-productization-20260622.md) - Verification evidence for the official Tree renderer, gallery sample, and nested scroll smokes.
* [Tree virtualized window verification](verification/tree-virtualized-window-20260626.md) - Verification evidence for the opt-in Tree virtualized render window, API export coverage, and gallery metadata proof.
* [Table sticky pinned columns verification](verification/table-sticky-pinned-columns-20260623.md) - Verification evidence for sticky pinned Table center scrolling and nested vertical containment.
* [Table exact-size virtualizer window verification](verification/table-exact-size-virtualizer-window-20260623.md) - Verification evidence for the exact-size virtualizer window used by Table center-column virtualization.
* [Table custom aggregation callbacks completion](progress/2026-06-24-table-custom-aggregation-callbacks-plan.md) - Durable handoff for the custom aggregation callbacks slice.
* [Table row selection variants planning](progress/2026-06-24-table-row-selection-variants-plan.md) - Planning note for the next Table follow-up boundary and the durable handoff for the row-selection variants slice.
* [Table faceted filter controls completion](progress/2026-06-24-table-faceted-filter-controls-plan.md) - Durable handoff for the single-column categorical faceted filter control slice.
* [Table numeric range filter controls](progress/2026-06-24-table-numeric-range-filter-controls.md) - Durable handoff for the numeric min/max filter control slice.
* [Table global filtering and faceting planning](progress/2026-06-24-table-global-filtering-faceting-plan.md) - Durable handoff for the next Table search / global faceting boundary.
* [Table column visibility controls](progress/2026-06-24-table-column-visibility-controls.md) - Durable handoff for the active Table column visibility controls slice.
* [Table filter operators planning](progress/2026-06-24-table-filter-operators.md) - Durable handoff for the built-in predicate operator slice.
* [Table autosize by content completion](progress/2026-06-25-table-autosize-by-content-plan.md) - Completion note for the Table content-fit sizing slice.
* [Text input editor family planning](progress/2026-06-25-text-input-editor-family-plan.md) - Planning note for password display, controlled textarea, and later Table multiline editor composition.
* [Table column groups and nested headers](progress/2026-06-24-table-column-groups-nested-headers.md) - Planning note for the next Table header-depth slice.
* [Table column groups and nested headers verification](verification/table-column-groups-nested-headers-20260625.md) - Verification evidence for the nested header gallery proof and center-window scroll smoke.
* [Table cell editing completion](progress/2026-06-24-table-cell-editing-plan.md) - Durable handoff for the first text-cell editing slice.
* [Table numeric range filter controls verification](verification/table-numeric-range-filter-controls-20260624.md) - Verification evidence for the numeric range filter control slice.

## Concepts

* [Decisions](decisions/) - Durable engineering choices and rationale.
* [Progress](progress/) - Work progress tied to plans, branches, or commits.
* [Sessions](sessions/) - Compaction, interruption, and handoff summaries.
* [Subagents](subagents/) - Distilled findings from spawned agents.
* [Verification](verification/) - Test, build, lint, benchmark, and manual evidence.
* [Conventions](conventions/) - Local repo rules and reusable agent contracts.
