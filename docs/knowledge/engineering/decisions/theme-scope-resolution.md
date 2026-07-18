---
type: "Decision"
title: "Theme scope resolution and deferred capture"
description: "Keep theme inheritance domain-specific while app, window, subtree, cached-view, overlay, and delayed-tooltip rendering share one effective ThemeContext authority."
timestamp: 2026-07-17T22:00:00+08:00
tags: ["open-gpui", "ui", "theme", "scope", "overlay", "adr"]
status: "active"
git_branch: "refactor/ui-framework-authority-convergence"
related_plan: "docs/plans/2026-07-10-001-refactor-ui-framework-authority-convergence-plan.md"
---

# Decision

Open GPUI keeps inherited theme resolution inside `open-gpui-ui-components`. The public authority
is an owned, immutable `ThemeContext`; the resolution order is:

1. nearest `ThemeScope` subtree override;
2. window selection or explicit window override;
3. application selection;
4. built-in light fallback.

There is no public generic GPUI inherited-context API and no app-global map keyed by window or
subtree identity. A private app global owns the installed registry and app fallback selection.
`Window::use_window_state` owns each window's base authority, cached effective context, scope stack,
and app-change observer. Window close therefore drops the complete local authority without a
cleanup registry.

# Context

The removed `ThemeRuntime: Global` combined registry ownership with one app-wide active id.
`ThemeResolver::current(&App)` could not distinguish windows or nested subtrees, and deferred work
had no opening-snapshot contract. Adding another map beside it would have created two selection
authorities and made window teardown dependent on manual bookkeeping.

The U7 prototype exercised actual GPUI timing rather than assuming React-like render context.
`RenderOnce` runs during element request-layout, while prepaint and paint occur in later phases.
Deferred children leave the parent stack after request-layout. Native tooltip builders execute in a
timer task and their returned views are laid out as separate roots. Cached `AnyView` children may
replay a previous frame journal without rendering their entity again.

# Scope Mechanism

`ThemeScope::new(stable_id, context, child)` is a three-phase element wrapper. It enters the
window-local stack independently for request-layout, prepaint, and paint. Each entry creates an
RAII guard that restores the prior stack depth on normal return, early return, or unwind. Owned
contexts cross phase boundaries; registry borrows never do.

The stable element id records the previous context in GPUI element state. When the context changes,
the scope calls `Window::with_cached_view_refresh` around child prepaint. The prototype's failing
cached-child test proved this substrate is necessary: an unchanged cached view otherwise replays
colors from its old journal even though the nearest provider changed. The helper temporarily
bypasses cached-view reuse and restores the previous refresh state after success or panic. It is a
cache-control substrate only; it stores no theme value and does not generalize service lookup.

# Selection And Invalidation

The public mutation surface is `install_theme_registry`, `register_theme`, `set_app_theme`,
`set_window_theme`, `override_window_theme`, and `clear_window_theme`. Definitions are validated on
a cloned registry before installation. Unknown ids fail before mutation. Equal app or window
selections do not update state entities.

Each initialized window observes private app theme state through a weak entity handle. An app
change recomputes the base context, but mutates and refreshes only when the effective context for
that window changed. Explicit overrides are skipped. A selected window refreshes only if the
definition behind its selected id changed. If a complete registry replacement temporarily omits a
window-selected id, that window retains its last-known owned snapshot and selection authority;
app changes do not demote it. Re-registering the same id updates it, while `clear_window_theme` is
the explicit fallback operation. Resolver reads derive inheriting contexts from current app state,
so app selection is visible in the same transaction before global observer effects flush. This
avoids retain cycles, transient theme flashes, stale reads, and global redraws.

# Complete Theme V1 Payload

U8 replaces the color-only payload in place under schema version `v1`. Every immutable
`ThemeSnapshot` and `ThemeContext` carries the complete admitted `ThemeDesignScales` value beside
the required color table. The value reuses `open_gpui_ui_core::Density` and
`open_gpui_motion::MotionPreference`; there is no parallel design-scale registry, string resolver,
or mutable service lookup.

Only tokens consumed by at least two distinct production component recipes enter the public
contract. Tests, Gallery examples, repeated call sites in one component, and documentation are
evidence but do not count as independent consumers. Structural component dimensions remain local,
and motion execution remains in `open-gpui-motion`.

Component size resolution is `explicit Size > theme density default`. Device-adaptive density is a
host recommendation and never enters recipes implicitly. Reduced motion is an accessibility safety
floor: a reduced theme or an explicit reduced component request wins, while an explicit animated
request cannot relax a reduced theme.

Serialized `revision` is source-file metadata. Runtime effective revisions come only from the theme
authority allocator and increase when effective content or app/window/subtree authority selection
changes. Callers cannot forge them. Identical effective reloads, metadata-only reloads, and exact
selection/override no-ops preserve the effective revision. Registry validation completes before
mutation, so failed parsing or replacement changes neither content, selection, revision, nor window
invalidation.

The old `fallback_mode`, partial color-table filling, registration diagnostics, color-only fixtures,
and compatibility parsing are deleted. Complete v1 input missing any required color or admitted
design scale fails atomically.

# Detached Rendering

Every official overlay binding stores one opening `ThemeContext` with its lifecycle generation.
Entering Open captures the effective trigger context. Rebinding within the same open generation
keeps it. Hidden clears it, and close/reopen captures a new context. Trigger styles continue to use
the current context; all surface colors, nested official components, and deferred phases use the
opening context. Synthetic Menu branch layers inherit their parent binding's opening context rather
than the outer scope visible when a branch happens to open.

UI Components tooltip attachment captures the trigger context when the hover/open generation is
scheduled. Both the delayed builder call and the returned `AnyView` run under that owned context.
A scope change after scheduling does not retheme the visible tooltip; closing and scheduling a new
tooltip captures again. Button, IconButton, Menu, Sidebar, and Toolbar apply this automatically.
Raw GPUI interactivity has no access to the exited subtree stack, so direct attachment must use
`Tooltip::scoped(context, builder)`. This explicit boundary replaces any claim of automatic ambient
inheritance.

# Generic API Gate

The prototype found no independent non-theme consumer that needs the same immutable nearest-scope
stack and detached capture semantics:

- focus scopes use explicit parent registration and live focus policy;
- overlay parentage uses a domain-local layer stack;
- accessibility scopes carry modal and hidden-tree semantics;
- text styles compose refinements instead of choosing one nearest immutable value.

Theme reads in normal elements, overlays, and tooltips are one consumer family, not three. The
two-consumer adoption gate therefore failed, and a public `InheritedContext<T>` would expose
abstraction without proven ownership. The theme-specific API remains the correct shape.

# Public Break

The following app-only compatibility authority is deleted without aliases:

- `ThemeRuntime` and `ThemeRuntimeError`;
- `ThemeResolver::current(&App)` and `ThemeResolver::resolve`;
- `init_theme_runtime`, `current_theme_context`, and `try_theme_context`;
- `set_active_theme` and `set_active_theme_mode`.

`ThemeRegistry`, `ThemeSnapshot`, `ThemeContext`, explicit `resolve_with`, built-in ids, and schema
loaders remain. Gallery renders real Dark and High Contrast sibling scopes plus a deferred Popover.
Its DevTools theme probe consumes the window-effective `ThemeContext` during shell construction and
each refresh; it no longer installs a fixed Light snapshot as runtime truth.

# Verification

Focused tests prove nested and sibling restoration, early-return and panic/unwind recovery, rerender
and cached-child invalidation, three-window isolation, precise app invalidation, unknown/no-op
atomicity, window-close isolation, complete-palette deferred capture with identical mode/revision
metadata, official Popover generation freeze, native Button and IconButton delayed tooltips,
Gallery scoped Popover rendering, and window-effective Gallery DevTools initialization.

U8 replaced the color-only payload with complete Theme v1 scales and split source metadata from the
runtime-owned effective revision. The completed implementation preserves the same precedence,
window ownership, scope identity, detached capture, and test matrix while extending canaries to
non-color scales and same-transaction redraws.
