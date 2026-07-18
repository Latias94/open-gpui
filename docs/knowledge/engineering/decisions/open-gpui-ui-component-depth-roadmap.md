---
type: "Decision"
title: "Open GPUI UI component depth roadmap"
description: "Prioritize deeper interaction families over adding more shallow UI primitives."
timestamp: 2026-06-22T00:00:00+08:00
tags: ["open-gpui", "ui-components", "roadmap", "command", "menu", "table", "tree"]
status: "active"
source_session: "019ec6c8-5566-7062-8458-21ebe1360573"
git_branch: "main"
git_commit: "0d82a9d"
updated: 2026-07-19T00:00:00+08:00
---

# Decision

The next UI component series should deepen existing complex component families instead of adding
more shallow leaf primitives. The current baseline has enough breadth that the highest product
risk is interaction depth, state ownership, nested scroll behavior, keyboard semantics, and gallery
conformance.

Priority order:

1. `Command`: fuzzy ranking, multi-select chips, virtualized results, app-wide indexing hooks, and
   stronger controlled query / selection ergonomics.
2. `Menu` / `ContextMenu`: submenu support, menu bar shape, checkbox and radio menu items, richer
   keyboard navigation, and application-menu integration points.
3. `Table`: pinned columns, grouped and expanded rows, aggregation, and later two-dimensional
   virtualization if the current one-axis virtualizer proves insufficient.
4. `Tree`: async loading, typeahead, drag-and-drop hierarchy editing, and virtualized tree data
   after the current renderer and `VirtualizedList` contract settle.
5. Polish track: `Sidebar`, `ScrollArea`, `TextInput`, `Avatar`, and `Overlay` should receive
   targeted ergonomic and regression-hardening work when real usage exposes gaps.

2026-07-01 update: the first deepening pass for `Command`, `Menu`, `ContextMenu`, `Tree`, and
Table behavior snapshots landed. At that point the next risk was described as component registry
ownership, accessibility gates, and theme loading.

2026-07-19 update: U1-U10 of the ongoing authority-convergence series completed that component
shared-contract slice without retaining the proposed central inventory. Product metadata is now
intentionally narrow, public exports live with their declarations, Gallery owns stories and
selectors, native targets own scenario coordinates, final AccessKit trees own semantic evidence,
and `xtask` joins those owners without becoming a manifest. Later GPUI substrate units remain open;
this update supersedes only the earlier registry workflow while preserving the decision to deepen
existing component families before adding shallow breadth.

# Context

The component library now has official coverage for the core layout, selection, form, feedback,
overlay, table, virtualized list, and tree families. The focused Components gallery also makes
individual families inspectable without losing the full all-components integration stress test.

Earlier reference research already concluded that `repo-ref/gpui-component`, `repo-ref/fret`,
shadcn/ui, daisyUI, and TanStack are useful references, but they should inform taxonomy,
contracts, and interaction policy rather than force an immediate standalone headless crate.

# Alternatives

- Add more leaf components first, such as badges, skeleton variants, banners, and small display
  primitives.
- Reopen standalone headless extraction before the complex family contracts harden.
- Put visual screenshot or image-diff regression tooling before the next product component slice.

The chosen path is to deepen complex families first. Leaf additions are easy to schedule later,
but Command, Menu, Table, and Tree determine whether the library can support real application
workflows.

That path produced the first family-boundary baseline. The next chosen path was to harden the
shared product contract layer before opening another large component-family split, and the
2026-07-19 authority-convergence update above records its completion.

# Consequences

- Future component-depth planning starts from the federated authorities already in place rather
  than reopening registry, inventory, or generated-manifest work.
- A new component-depth slice updates the narrow component row only when product identity,
  revision, family, or required scenarios change. Public exports, Gallery stories/selectors, and
  native scenario coordinates move with their natural owners.
- Final accessibility trees/actions, semantic activation tests, per-window overlay runtime tests,
  theme scope/schema gates, and focused `cargo nextest` scenarios remain executable evidence; none
  is mirrored into a central conformance table.
- Visual regression tooling remains valuable, but it should follow a concrete rendering pain point
  rather than block the next product slice.
- Standalone headless extraction remains deferred until repeated contracts across several complex
  families prove a stable boundary.

# Citations

[1] [Current State](../current-state.md)
[2] [UI component productization roadmap](open-gpui-ui-productization-roadmap.md)
[3] [UI component roadmap reference research](../subagents/ui-component-roadmap-reference-research.md)
[4] [Component contract](../../../ui/component-contract.md)
[5] [API and overlay productization plan](../../../plans/2026-06-22-003-feat-ui-api-overlay-productization-plan.md)
[6] [ADR 0014: Remove Open GPUI Native UI Hybrid Registry](../../../adr/0014-remove-native-ui-hybrid-registry.md)
[7] [Semantic accessibility and final-tree authority](semantic-accessibility-final-tree-authority.md)
[8] [Theme scope resolution and deferred capture](theme-scope-resolution.md)
