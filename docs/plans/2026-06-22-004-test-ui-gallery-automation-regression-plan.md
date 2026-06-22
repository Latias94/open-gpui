---
title: Gallery Automation Regression Hardening
type: test
date: 2026-06-22
---

# Gallery Automation Regression Hardening

## Summary
The Components gallery now has enough official surface area that manual inspection is no longer a reliable way to keep regressions out. This plan adds a reusable automation matrix for focused component-family inspection, so new catalog entries inherit a focused-mode guard without adding more one-off smoke tests.

## Problem Frame
Recent work has proven that the gallery can model scroll ownership, focus resets, and family-specific inspection. The gap is that those guarantees still depend on per-family tests and ad hoc human review, which scales poorly as more component families and state contracts are added.

## Requirements
- R1. The gallery test surface must provide one reusable helper that drives a catalog card into focused mode and asserts the correct sample or readout selector is rendered.
- R2. The focused-matrix coverage must include every catalog entry that can be focused from the Components page, including official components and state-contract entries.
- R3. The automation slice must keep the existing nested scroll containment smokes intact for Table, Tree, VirtualizedList, ScrollArea, Tabs, Sidebar, and release-queue chrome.
- R4. The automation slice must preserve the full all-components page as the integration stress test and keep focused mode as an inspection path.
- R5. The verification docs and engineering memory must name the new matrix gate so future sessions can rerun it without reconstructing the intent from chat history.

## Key Technical Decisions
- Derive the matrix from `COMPONENT_CATALOG` and `focused_section_for_catalog_entry` rather than maintaining a separate automation manifest, because the gallery metadata is already the source of truth.
- Keep the first slice on debug selectors and runtime state assertions. Screenshot or image-diff tooling is deferred because it introduces a separate baseline problem and does not yet buy enough coverage over the current regressions.
- Refactor helper code where it removes duplication, but do not change gallery structure, component contracts, or component APIs as part of this slice.

## Scope Boundaries
- Deferred for later: screenshot baselines, pixel-diff regression tooling, and cross-platform visual capture infrastructure.
- Deferred for later: expanding the automation matrix to non-gallery pages or component APIs outside the gallery contract surface.
- Outside this product's identity: introducing a new framework-level visual testing system just to validate this one gallery.

## Implementation Units

### U1. Add a catalog-driven focused-mode matrix test
Goal: create one runtime smoke that iterates the focusable Components catalog entries, opens each from its card, and asserts the expected sample or state-readout selector is rendered.

Files:
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs`

Patterns:
- Reuse existing `scroll_page_until_visible`, `click`, `settle`, and selector helpers.
- Drive focus from `COMPONENT_CATALOG` and `focused_section_for_catalog_entry`.

Test Scenarios:
- Clicking a focusable catalog card updates `ComponentFocusMode` to the matching section.
- The matching sample selector or state-contract readout becomes rendered.
- The section directory remains available in focused mode.
- Returning to `All components` restores the full page.

Verification:
- `cargo nextest run -p open-gpui-ui-foundation-gallery <new-matrix-test-name>`

### U2. Reuse the helper in existing focused and scroll smoke coverage
Goal: reduce one-off focused-mode code where the same guard can be expressed through the matrix helper without weakening component-specific scroll containment tests.

Files:
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs`
- `docs/verification.md`

Patterns:
- Keep the Table, Tree, and VirtualizedList nested scroll assertions as dedicated smoke tests.
- Use the new helper for catalog-to-focus transitions where it avoids duplication.

Test Scenarios:
- Focused Table remains scroll-contained.
- Focused Tree and VirtualizedList remain scroll-contained.
- The new helper does not hide any component-specific scroll regression.

Verification:
- The focused gallery smoke commands listed in `docs/verification.md` remain green.

### U3. Record the new automation gate in docs and memory
Goal: make the new matrix gate easy to rediscover in future sessions.

Files:
- `docs/verification.md`
- `docs/knowledge/engineering/current-state.md`
- `docs/knowledge/engineering/log.md`

Patterns:
- Add the matrix command next to the gallery verification section.
- Update the current-state note to point at the new automation slice.
- Record the next likely follow-up if visual regression tooling is still deferred.

Test Scenarios:
- A fresh session can find the matrix gate from the verification doc alone.
- The engineering memory points at the completed automation slice and next follow-up.

Verification:
- `python $HOME\.codex\skills\engineering-wiki-memory\scripts\wiki_memory.py validate --root docs\knowledge\engineering`

## Risks & Dependencies
- The matrix could become slow if it tries to open too many samples per run; if so, keep the runtime helper lean and prefer selectors already present on the page.
- A helper that is too generic could blur the component-specific smoke tests, so the refactor needs to preserve the current dedicated scroll containment assertions.
- Visual screenshot tooling remains a future dependency if we decide to move beyond selector-based automation.

## Sources
- `docs/ui/component-contract.md`
- `docs/verification.md`
- `docs/knowledge/engineering/current-state.md`
- `examples/ui-foundation-gallery/src/pages/components.rs`
- `examples/ui-foundation-gallery/tests/foundation_gallery.rs`
