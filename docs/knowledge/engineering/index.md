# Engineering Memory

## Core

* [Current State](current-state.md) - Short durable summary of the active engineering state.
* [Update Log](log.md) - Chronological history of meaningful memory updates.
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
* [Tree renderer productization verification](verification/tree-renderer-productization-20260622.md) - Verification evidence for the official Tree renderer, gallery sample, and nested scroll smokes.
* [Table sticky pinned columns verification](verification/table-sticky-pinned-columns-20260623.md) - Verification evidence for sticky pinned Table center scrolling and nested vertical containment.
* [Table exact-size virtualizer window verification](verification/table-exact-size-virtualizer-window-20260623.md) - Verification evidence for the exact-size virtualizer window used by Table center-column virtualization.
* [Table custom aggregation callbacks completion](progress/2026-06-24-table-custom-aggregation-callbacks-plan.md) - Durable handoff for the custom aggregation callbacks slice.
* [Table row selection variants planning](progress/2026-06-24-table-row-selection-variants-plan.md) - Planning note for the next Table follow-up boundary and the durable handoff for the row-selection variants slice.
* [Table faceted filter controls planning](progress/2026-06-24-table-faceted-filter-controls-plan.md) - Planning note for the next Table follow-up boundary and U1 completion handoff.

## Concepts

* [Decisions](decisions/) - Durable engineering choices and rationale.
* [Progress](progress/) - Work progress tied to plans, branches, or commits.
* [Sessions](sessions/) - Compaction, interruption, and handoff summaries.
* [Subagents](subagents/) - Distilled findings from spawned agents.
* [Verification](verification/) - Test, build, lint, benchmark, and manual evidence.
* [Conventions](conventions/) - Local repo rules and reusable agent contracts.
