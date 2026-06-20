# ADR 0008: Open GPUI UI Component Productization Roadmap

**Status**: Accepted
**Date**: 2026-06-17

## Context

ADR 0005 chose an adapter-first, headless-ready component architecture and deferred a standalone
`open-gpui-ui-headless` crate until repeated behavior contracts existed and the public boundary was
clean enough to move.

ADR 0006 and ADR 0007 later confirmed that the current UI core and component boundaries are clean
enough to keep extraction possible. That remains useful history. It is not the active roadmap for
the next phase.

The current component stack is already broad enough to function as a product surface:
runtime theme resolution, editable text input, overlay behavior, focus and accessibility state,
scroll viewports, splitter constraints, shell/navigation, and choice/search families are all part
of the current crates. The remaining work is to finish and harden those crates, not to introduce a
new package boundary first.

## Decision

Treat the current UI crates as the product boundary for the next phase.

- `open-gpui-ui-core` owns neutral vocabulary, policy, and shared state.
- `open-gpui-ui-components` owns the concrete official GPUI components and adapter helpers.
- `examples/ui-foundation-gallery` remains the conformance and dogfood surface.
- Do not create a standalone `open-gpui-ui-headless` crate in the active roadmap.
- Keep renderer-neutral state and adapter classification as hygiene, not as the primary product
  objective.

The next sequence should therefore be:

1. Recenter roadmap and memory docs on current-crate productization.
2. Finish runtime foundations that every component depends on.
3. Harden shell/navigation and choice/search families.
4. Keep the gallery, verification docs, and engineering memory aligned with the active product
   story.

## Rationale

The project already has enough repeated behavior to justify future extraction research, but the
largest current risk is product quality inside the existing crates, not the existence of a separate
headless package. Finishing the current crates first keeps the component system coherent, keeps the
gallery honest, and avoids freezing abstractions before the surface has stabilized.

## Consequences

Positive:

- The roadmap now matches the current implementation reality.
- The docs and memory trail point future work at the product surface instead of the old extraction
  story.
- Runtime foundations, shell/navigation, and choice/search can be improved as one coherent product
  line.

Negative:

- A future standalone headless crate remains deferred and must be revisited explicitly if the
  product direction changes.
- Some historical docs will still mention extraction because they record the earlier boundary work.
  Those references should be treated as history unless a new extraction plan is opened.

## Relationship to ADR 0006 and ADR 0007

ADR 0006 remains the extraction checkpoint that proved the UI core boundary is clean enough to
consider future moves.

ADR 0007 remains the design gate that classifies which behavior families would move first if the
project later revisits extraction.

This ADR does not invalidate either document. It changes the active roadmap that should guide the
next implementation phase.

## Follow-Up Work

- Update the roadmap and series-plan docs so productization is the active story.
- Update the engineering wiki memory bundle so later sessions resume from the productization
  narrative.
- Keep ADR 0006 and ADR 0007 available as historical boundary references.

## Citations

[1] [ADR 0005](0005-open-gpui-official-component-architecture.md)
[2] [ADR 0006](0006-open-gpui-ui-headless-extraction-checkpoint.md)
[3] [ADR 0007](0007-open-gpui-ui-headless-boundary-design.md)
[4] [Productization roadmap plan](../plans/2026-06-17-003-feat-ui-component-productization-roadmap-plan.md)
