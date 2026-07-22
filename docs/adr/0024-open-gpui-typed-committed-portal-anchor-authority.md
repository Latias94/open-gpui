# ADR 0024: Open GPUI Typed Committed Portal Anchor Authority

**Status**: Accepted
**Date**: 2026-07-21

## Context

Trigger-bound overlays previously reconstructed live placement from estimated local rectangles or
retained a raw submenu row bound after prepaint. Those values had no window ownership, could miss a
subtree transform or scroll delta, and could survive Hidden state, unmount, rollback, or a failed
frame. A raw `OverlayAnchorInput` is a valid renderer-neutral placement value, but it cannot prove
that the element which produced it still exists.

GPUI already has one committed post-layout geometry authority and transactional frame journals. The
missing boundary was a narrow capability that could name one target without exposing a generic DOM
node, raw transform matrix, mutable rectangle, selection API, or cross-window transport.

## Decision

`Window::new_portal_anchor` creates an opaque `PortalAnchorHandle` owned by that window. A target
binds the handle once per frame with `PortalAnchorExt::track_portal_anchor` or the lower-level
`Window::bind_portal_anchor`. Duplicate binding and foreign-window use return typed errors. The
handle may have any number of followers, but it never mutates follower state itself.

A candidate binding is recorded during target prepaint and becomes committed only when the frame
finishes successfully. During prepaint or paint, `Window::resolve_portal_anchor` consults only the
candidate frame; outside a draw it consults only the completed frame. No binding in the completed
frame means unlinked. Hidden, absent, unmounted, rolled-back, and numerically invalid targets never
reuse an older snapshot. Inert targets remain linked and publish `SubtreePresentation::Inert` so
each follower can apply its own eligibility policy.

`PortalAnchorSnapshot` publishes window identity, frame generation, opaque `ElementGeometry`,
effective presentation, and effective clip AABB. It exposes neither a resolved matrix nor mutable
target state. Cached target journals may replay under an unchanged geometry/presentation cache key,
updating only their frame generation. Views that resolve an anchor are cross-view dependencies:
GPUI records the resolving view and its cache ancestors, so their deferred journals rebuild on the
next frame rather than replaying a captured linked result after the target changes elsewhere.
Target-root presentation and transform wrappers compose by layout identity, so moving
`track_portal_anchor` before or after those wrappers cannot change the published target facts.
Cached `AnyView` roots explicitly alias their outer cache layout to the rendered root layout. This
keeps root wrappers authoritative across the retained-view boundary while excluding transforms or
presentation scopes that belong only to ordinary descendants. An outer tracker forces fresh
prepaint for that cached semantic root because replaying the inner journal cannot update the active
outer capture.

`portal_anchor_follower` is the standard same-frame deferred consumer. It resolves after ordinary
prepaint and emits content through a named window-space portal. The target snapshot is already
projected to window space. The portal resets geometry and clip deliberately while preserving theme
and presentation inheritance; ordinary deferred descendants still inherit their current transform
and clip.

UI Components requires a Visible snapshot for interactive overlay followers. Missing or Inert
targets force the layer Hidden and noninteractive with `DismissReason::AnchorUnlinked`.
Uncontrolled owners commit closed state. Controlled owners receive exactly one close intent while
the runtime immediately releases keyboard, pointer, and focus authority; an owner that commits
Hidden clears the pending intent, while an owner that remains Open retains it without duplicate
dispatch. Reappearance and a renewed Open request create a new opening generation.

Official trigger-bound Popover, Select, Combobox, HoverCard, Menu, and submenu paths use runtime-
owned handles. Standalone Tooltip accepts an external stable handle and an open-change callback.
ContextMenu's explicit window point, GPUI-native pointer tooltip points, and full-window modal
surfaces remain intentionally distinct. They use named window-space paths and do not pretend a
point or viewport is a live element target.
Overlay inside regions are likewise committed from checked displayed `ElementGeometry` intersected
with the effective content mask; raw or clipped-away layout bounds cannot become a second
outside-press coordinate authority under subtree transforms.

## Consequences

- Menu no longer stores `submenu_trigger_bounds` or reuses a previous row rectangle.
- Estimated `(0, 0)` trigger rectangles and the local relative overlay helper are removed from
  official trigger-bound components.
- One target can drive multiple differently placed followers without reopening when geometry moves.
- A follower in a cached view reruns its resolver whenever the window draws; ordinary unrelated
  cached views remain reusable.
- Portal output intentionally escapes the target's effective clip. The clip remains a source fact
  for follower policy and diagnostics, not a second clipping authority for the portal surface.
- Applications needing a standalone Tooltip retain one handle, bind it to exactly one target per
  frame, pass it to every follower, and handle `AnchorUnlinked` like any other controlled intent.

## Verification

GPUI tests cover current versus committed ordering, multiple followers, duplicate and wrong-window
errors, Hidden/Inert/absent/unmount/rebind, rollback, late numeric failure, effective clip, explicit
portal reset, wrapper-order invariance, cached target replay, and independent cached follower
invalidation. Cached-view tests also cover a tracker outside the view with transform and
presentation wrappers inside its rendered root across consecutive frames. UI Components
tests cover every official family, Menu branches, controlled and uncontrolled unlink, exactly-once
intent, focus teardown, opening generations, transformed inside regions, point/full-window reset,
and stable external Tooltip registration. The Gallery drives transform, page scroll, two followers,
Hidden, unmount, owner
commit, and controlled reopen through real window runtime paths.

## Related Documents

- [UI framework authority convergence plan](../plans/2026-07-10-001-refactor-ui-framework-authority-convergence-plan.md)
- [Interactive subtree transform authority](0021-open-gpui-interactive-subtree-transform-authority.md)
- [Subtree presentation authority](0022-open-gpui-subtree-presentation-authority.md)
- [UI component contract](../ui/component-contract.md)
- [Open GPUI v0.3 UI migration guide](../ui/migration-v0.3.md)
- [Portal-anchor verification ledger](../knowledge/engineering/verification/portal-anchor-authority-20260721.md)
