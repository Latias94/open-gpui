# ADR 0008: Open GPUI UI Component Productization Roadmap

**Status**: Accepted
**Date**: 2026-06-17
**Updated**: 2026-07-19

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

When this ADR was accepted, the next sequence was framed as splitting a growing component contract
registry, adding accessibility contract gates, and adding a theme loader. That framing is retained
as history, but its central-registry workflow is superseded by ADR 0014 and the completed U1-U10
component-authority slice of the ongoing authority-convergence series. That slice produced:

1. `ComponentContractEntry` owns only product id, revision, family, and required scenario ids.
   Public export declarations, Gallery stories/selectors, native scenario coordinates, DevTools
   projections, and documentation remain with their natural owners.
2. Accessibility semantics derive from resolved component state and are proven at final AccessKit
   tree and real action-dispatch boundaries, not by static evidence rows.
3. Theme v1 is a complete portable payload with validated JSON loading, app/window/subtree
   resolution, and opening-generation capture for detached surfaces.
4. `scan-ui-contract` joins federated typed facts and executes owner-provided scenarios. It is a
   verification gate, not a registry, generated manifest, or replacement source of truth.

Broad splitting of every remaining 1k+ component file is not part of this sequence. A large file
should move only when a concrete contract, runtime, accessibility, or theme ownership problem makes
the split valuable. Standalone headless extraction also remains out of scope.

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
- The completed authority-convergence work hardened shared product contracts before another large
  visible component or package boundary was opened.
- The original registry-splitting sequence is complete and superseded by federated ownership; it
  must not be revived as an API inventory, generated manifest, or central conformance table.

Negative:

- A future standalone headless crate remains deferred and must be revisited explicitly if the
  product direction changes.
- Some large component implementation files remain large until a concrete ownership problem makes
  splitting them worthwhile.
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

- Preserve the completed authority split through the focused gates in
  [the verification guide](../verification.md).
- Use the [v0.3 UI migration guide](../ui/migration-v0.3.md) as the downstream entry point for the
  breaking callback, accessibility, overlay, theme, Table identity, and typeahead changes.
- Keep ADR 0006 and ADR 0007 available as historical boundary references.

## Citations

[1] [ADR 0005](0005-open-gpui-official-component-architecture.md)
[2] [ADR 0006](0006-open-gpui-ui-headless-extraction-checkpoint.md)
[3] [ADR 0007](0007-open-gpui-ui-headless-boundary-design.md)
[4] [Productization roadmap plan](../plans/2026-06-17-003-feat-ui-component-productization-roadmap-plan.md)

## Related Decisions

- [ADR 0009: Open GPUI Table and Virtualizer Product Shape](0009-open-gpui-table-and-virtualizer-product-shape.md)
- [ADR 0014: Remove Open GPUI Native UI Hybrid Registry](0014-remove-native-ui-hybrid-registry.md)
- [Focus scope and window overlay runtime ownership](../knowledge/engineering/decisions/focus-scope-window-overlay-runtime.md)
- [Semantic accessibility and final-tree authority](../knowledge/engineering/decisions/semantic-accessibility-final-tree-authority.md)
- [Semantic activation authority](../knowledge/engineering/decisions/semantic-activation-authority.md)
- [Theme scope resolution and deferred capture](../knowledge/engineering/decisions/theme-scope-resolution.md)
