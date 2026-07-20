# ADR 0022: Open GPUI Subtree Presentation Authority

**Status**: Accepted
**Date**: 2026-07-20

## Context

Open GPUI previously expressed layout-preserving hiding through `Style::visibility`, its generated
Serde/schema `StyleRefinement::visibility` field, `Visibility::{Visible, Hidden}`, and the generated
`visibility_style_methods!` surface with `.visible()` / `.invisible()` fluent methods. The
resulting late paint branch ran after input and focus registration, while accessibility used a
separate inherited `a11y_hidden` stack. A subtree could therefore remain interactive, focused, or
present in the final AccessKit tree after it stopped painting. Components also had no coherent
inert state that retained layout and paint while suppressing every interactive and semantic
channel.

Presentation is a cross-channel lifecycle fact. It must govern paint, hit testing, pointer and
wheel dispatch, hover and cursor state, drag/drop and pointer capture, focus and IME, tooltip and
overlay intent, inspector geometry, deferred work, cached frame replay, and final accessibility
membership at the same committed-frame boundary.

## Decision

`open-gpui` owns one public `SubtreePresentation` authority with three states:

| State | Layout | Paint | Input | Focus / IME | Accessibility |
| --- | --- | --- | --- | --- | --- |
| `Visible` | yes | yes | yes | yes | yes |
| `Inert` | yes | yes | no | no | no |
| `Hidden` | yes | no | no | no | no |

`SubtreePresentationExt::with_subtree_presentation` is the application entry point. Layout includes
measurement, flex/grid order, sibling placement, and scroll extent. `Display::None` remains the
explicit layout-removing choice. Component disabled state remains a semantic component fact: a
disabled control may stay discoverable in accessibility, while an inert control is absent from the
accessibility tree and all input routing.

Nested declarations resolve to the most suppressive state. `Hidden` dominates `Inert`, which
dominates `Visible`; a descendant cannot opt back in beneath a suppressed ancestor. Decorative
leaf omission uses `omit_accessibility_node` and is not an ancestor presentation mechanism.

```rust
use open_gpui::{
    ParentElement as _, SubtreePresentation, SubtreePresentationExt as _, div,
};

let content = div()
    .child("Retains layout and paint without interaction")
    .with_subtree_presentation(SubtreePresentation::Inert);
```

## Frame And Lifecycle Contract

The resolved state is carried by a stack-balanced `Window` scope. Low-level registration APIs
check that scope, so custom elements cannot publish hitboxes, listeners, focus targets, text input,
tooltips, inspector targets, or AccessKit nodes from an inert or hidden ancestor. `Hidden` still
runs layout and the minimum retained-view reconciliation needed to discard pending interaction
state, but skips descendant prepaint and paint work whenever the child has no such reconciliation.

Frame commit first reconciles presentation membership, then publishes retained transactions.
Changing a captured, dragged, hovered, focused, or editing subtree to `Inert` or `Hidden` removes
the stale binding in that committed frame. Pointer ownership receives exactly one terminal cancel.
Focus and IME use the existing window focus authority rather than a presentation-specific restore
policy. Returning to `Visible` rebuilds current participation and never replays an old event,
activation arm, focus claim, tooltip request, scroll callback, or overlay intent.

Focus intent remains provisional until the target qualifies in the final rendered focus tree.
`on_focus_committed` observes one handle becoming the exact window-local focus, while
`on_focus_committed_in` observes committed focus entering a handle or descendant. Both work while
the platform window is inactive; `on_focus_in` remains the effective active-window event.
Retained transactions such as Dock focus restoration use typed one-shot
`focus_with_completion` and `blur_with_completion` results instead of inferring success from a
persistent observer, request submission, or next-frame timing. Exact-target and empty-focus
requests each terminate once as `Committed`, `Rejected`, or `Superseded`.

After paint resolves input handlers and accessibility focus, the candidate frame's focus authority
is sealed. A focus or blur request from a frame-commit callback is therefore queued for one later
platform generation rather than mutating already-published channels. The request leaves a
focus-only frame demand only after all current focus listeners finish; net-zero reassertions remove
that demand, and cached commit replay cannot synchronously drive an effect-loop redraw cycle. A
rejected late presentation candidate cannot consume a successful-focus transaction, and later
platform activation cannot replay an already committed local event.

Ordinary deferred descendants and cached journals inherit the resolved state. Coordinate-space
portal resets do not reset presentation. A `WindowOverlayRuntime` layer is an independently owned
window root only through its explicit registration boundary; its local state still combines with
the effective state of its overlay parent. Suppressing a parent therefore suppresses all open
descendants without affecting independent roots.

Persistent window interceptors are explicitly window-root APIs, not element-scoped listeners.
Subtree consumers use frame-scoped listeners. Docking, for example, publishes its pointer-cancel
listener from the rendered `DockHost` subtree, so suppression terminates its active interaction
once and leaves no listener authority behind.

## Consequences

- `Style::visibility`, generated Serde/schema `StyleRefinement::visibility`,
  `Visibility::{Visible, Hidden}`, `visibility_style_methods!` and its generated `.visible()` /
  `.invisible()` (`fn visible` / `fn invisible`) methods, `Element::a11y_hidden`, `aria_hidden`, and
  the inherited hidden-accessibility stack are deleted without aliases.
- `SubtreePresentation` is the only layout-preserving ancestor switch for paint, interaction,
  focus/IME, and accessibility participation.
- Overlay presence, opacity, clipping, transforms, disabled controls, and `Display::None` remain
  separate facts with separate ownership.
- Gallery, DevTools, public-surface tests, and `scan-ui-contract` expose or enforce the same state;
  production code cannot recreate the removed subtree authorities.

## Verification

Runtime coverage asserts the exact channel matrix, ancestor dominance, dynamic focus/capture/IME
and accessibility cleanup, stale action rejection, transform composition, deferred and cached
replay, portal inheritance, explicit overlay roots, semantic activation, and Dock cancellation.
The Gallery Presentation page switches identical transformed content among all three states while
holding its layout slot stable. Public-surface and source scans reject every removed entry point.

## Related Documents

- [UI framework authority convergence plan](../plans/2026-07-10-001-refactor-ui-framework-authority-convergence-plan.md)
- [ADR 0021: Open GPUI Interactive Subtree Transform Authority](0021-open-gpui-interactive-subtree-transform-authority.md)
- [Open GPUI v0.3 UI migration guide](../ui/migration-v0.3.md)
- [Verification guide](../verification.md)
