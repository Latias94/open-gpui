# ADR 0027: Open GPUI Dock Visual Style Authority

**Status**: Accepted
**Date**: 2026-07-25

## Context

Dock rendering previously selected colors and shadows inside individual host, tab, splitter,
floating, drag, guide, preview, and transition paths. Those local choices could not follow the
effective theme of a window or subtree without either copying theme logic into Docking or adding a
reverse dependency from `open-gpui-docking` to `open-gpui-ui-components`. The public
`DockDropGuideStyle` name also mixed structural guide dimensions with visual styling.

Cross-window dragging adds a timing distinction that an ordinary render lookup cannot express.
The source-owned deferred drag visual must retain the style visible when that drag generation
opened, while target-owned guides and previews must follow the target host's current context.
Putting visual data in `DockDragPayload` would make presentation affect payload equality, routing,
validation, and persistence.

The local Dear ImGui checkout remains useful evidence for mature docking interactions. In
particular, `DockNodePreviewDockSetup`, tab-bar state, inner versus outer target zones, accepted
versus rejected previews, and viewport tear-off behavior inform the interaction vocabulary.
ImGui's immediate-mode `ImGuiDockContext`, binary node identity, builder API, settings format, and
default colors do not match Open GPUI's retained graph and theme ownership.

## Decision

`open-gpui-docking` owns one complete immutable `DockVisualStyle`. It covers host and diagnostic
surfaces, tab and close-action states, splitter states, floating chrome, source drag visuals,
accepted and rejected target previews, inner and outer guides, route previews, transition
affordances, focus rings, and elevation. `DockVisualPalette` is a complete semantic input used to
derive that style. There is no partial style merge.

The only production color and shadow literals live in `DockVisualPalette::built_in`,
`DockVisualStyle::from_palette`, and their private color/shadow construction helpers. Neither
`DockVisualPalette` nor `DockVisualStyle` implements `Default`, so a partial struct update cannot
silently merge application input with fallback values. The deterministic `built_in()` fallback is
used when an application installs no resolver. Structural layout and hit-test values remain in Dock
options. `DockDropGuideStyle` is deleted without an alias and replaced by `DockDropGuideMetrics`.

`DockVisualStyleResolver` is the named render-time boundary. Its callback has the read-only
signature:

```rust
Fn(&Window, &App) -> DockVisualStyle
```

The resolver is an immutable value installed on a `DockSurface`, a
`DockViewportRuntimeHandle`, or an explicit low-level `DockHost`. A host resolves it in its active
window and subtree render context for each relevant render generation. Read-only callback
arguments prevent entity updates, notifications, dispatch, registration changes, and refresh
scheduling; a separate guard rejects reentrant Dock style resolution and restores its state on
unwind.

Theme integration is application-owned. A consumer that depends on both crates may map
`ThemeResolver::current_snapshot(window, cx)` into a `DockVisualPalette` and return
`DockVisualStyle::from_palette`. The snapshot accessor is read-only and observes the same subtree,
window, app, and built-in precedence as the mutable theme resolver without initializing or
mutating runtime state. `open-gpui-docking` has no production dependency on UI Components.

Each host render resolves one style and shares that immutable value through every paint path.
Presentation-only snapshots contain no style. A source drag freezes only the source-owned
`DockDragVisualStyle` in viewport runtime metadata keyed by the drag session and opening
generation; the source's host, tab, splitter, floating, guide, and preview styles cannot leak into
the target. Target guides and previews resolve the current target-host style. Cancel or close
removes the source snapshot before another drag can begin, and reopening the same payload captures
a new generation. `DockDragPayload` contains no visual facts, so its equality, identity, route
validation, and persistence remain unchanged.

`scan-ui-contract` rejects Dock production dependencies on UI Components, the retired guide-style
name, `Default` implementations for the complete visual inputs, visual literals outside the exact
built-in definition scopes, and competing production theme lookups. Transparent routing constants
remain narrowly allowlisted.

Dear ImGui is an interaction-state reference only. Open GPUI retains `DockGraph`, n-ary same-axis
split normalization, stable application item identities, explicit transactions, viewport session
generations, retained GPUI views, and application-owned persistence. It does not port
`ImGuiDockContext`, `DockBuilder`, pointer-addressed or binary nodes, `.ini` settings, `PlatformIO`,
or ImGui's pixel palette.

## Consequences

- A theme or subtree change updates Dock paint without mutating `DockGraph`, selection, focus
  history, or surface revision.
- Different hosts and windows may render different styles while sharing one retained controller.
- Source and target drag visuals have explicit generation ownership rather than depending on
  whichever window happens to render last.
- Applications own brand and theme mapping, while Docking owns visual completeness and rendering
  semantics.
- Adding a new Dock visual state requires extending the complete style, its state lookup tests,
  the application adapter, and the source gate.
- The resolver intentionally cannot perform mutable lazy initialization. Applications must prepare
  registries and theme state before rendering.

## Rejected Alternatives

- A mutable app-global Dock theme registry would duplicate application theme authority and make
  window or subtree isolation ambiguous.
- A production UI Components dependency would invert the intended crate boundary and prevent
  theme-independent Dock use.
- Per-render-path optional colors or partial style merging would recreate competing fallbacks and
  make completeness unprovable.
- Capturing target style at source-drag start would paint destination guides with the wrong
  window's context.
- Resolving the source drag visual live would let an out-of-band deferred surface change appearance
  during one opening generation.
- Storing style in `DockDragPayload` would couple presentation to domain identity and routing.
- Copying ImGui's context, binary tree, settings, or colors would replace retained Open GPUI
  authorities rather than merely learning from its interaction behavior.
