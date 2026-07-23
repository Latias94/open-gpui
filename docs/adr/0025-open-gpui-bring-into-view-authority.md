# ADR 0025: Open GPUI Bring Into View Authority

**Status**: Accepted
**Date**: 2026-07-22

## Context

Focus, AccessKit `ScrollIntoView`, and application code previously reached scroll containers through
different paths. Component adapters also computed fixed-row or nearest-row offsets themselves. Those
paths could reveal one immediate viewport, but they could not consistently traverse nested
scrollports, arbitrate overlapping requests, convert through non-uniform subtree transforms, or
produce one deterministic completion and cancellation contract.

Virtual collections add a separate concern. A stable logical item may not currently have physical
geometry. Treating an item index or estimated offset as a GPUI target would move domain identity and
virtualization policy into the window substrate.

## Decision

Each `Window` owns one bring-into-view authority. `Window::new_reveal_target` creates an opaque
same-window `RevealTargetHandle`. Applications bind it once per candidate frame with
`RevealTargetExt::track_reveal_target` or the lower-level `Window::bind_reveal_target`. A successful
frame commits the target's final `ElementGeometry`, effective presentation, and ordered physical
scroll ancestry. Hidden, absent, rolled-back, invalid, or unmounted targets do not reuse an older
binding.

Application requests, the winning end-of-turn focus claim, and accepted AccessKit
`ScrollIntoView` actions enter the same window authority. Every request receives a monotonic
window-local sequence and an opaque chain generation. Requests whose committed chains are disjoint
may advance together. Before one request mutates a shared scroll container, it cancels older
overlapping work as `Superseded`.

Every successfully published semantic node exposes AccessKit `ScrollIntoView`. It is a geometry
action rather than an activation or focus capability, so disabled or otherwise non-activatable
nodes retain it while stale, suppressed, or unpublished nodes cannot route it.

The authority processes committed containers from inner to outer. It derives a window-space delta
from final displayed geometry, converts that vector into the owning container's local coordinates,
quantizes the delta to the device-pixel grid, applies the requested physical horizontal and vertical
policies, and waits for committed geometry before continuing outward. A saturated inner container
continues outward when the target is already nearest-visible there; a target that still cannot move
toward visibility terminates as `NoProgress`.

The public alignment vocabulary is deliberately physical: `Nearest`, `MinEdge`, `Center`, and
`MaxEdge` are selected independently for horizontal and vertical axes, with checked non-negative
physical margins. Logical block/inline and start/end names wait for a locale and direction
authority. Instant requests do not depend on Motion. Animated requests use renderer-neutral
`MotionTransition` sampling and the effective reduced-motion policy while preserving the same
geometry and cancellation rules.

Direct wheel, scrollbar, keyboard, touch, or explicit programmatic scrolling cancels affected
active requests as `ScrollOverridden`. Unlink, suppression, ancestry replacement, window close, and
no progress have distinct terminal reasons. Dropping a completion `Subscription` stops observation
only; it does not cancel the request.

An explicit portal begins a new rendered ancestry. GPUI does not guess through a portal to a source
tree. Virtualized collections use two phases: their adapter resolves a stable key and scrolls only
far enough to materialize its physical row, then binds a `RevealTargetHandle` and asks the window
authority for final nested alignment. GPUI never interprets collection keys, indices, row IDs, or
virtual ranges.
When materialization and physical submission span frames, an adapter captures an opaque
`DeferredBringIntoViewGuard` from prepaint inside the intended final scroll ancestry as soon as
logical materialization completes. Its later guarded submission, after the target binds, atomically
rejects direct-scroll interruption, a missing target, or a changed complete scroll ancestry before
a request reaches window authority. A per-handle
`ScrollHandle::direct_scroll_revision` remains low-level support for a known single handle, not a
replacement for the guard. Geometry replacement while a physical request is in flight waits for
that request's terminal outcome and retries only after completion. Every cancellation, including
`Superseded`, `ScrollOverridden`, `TargetUnlinked`, `AncestryChanged`, `TargetSuppressed`,
`NoProgress`, and `WindowClosed`, terminates the stale adapter operation.

`ScrollChainFence` is the retained input-era continuation capability shared by deferred adapters
and focus handoffs. It records the complete inner-to-outer chain, available physical axes, and
direct-scroll revisions without creating a new baseline after input. A
`focus_with_completion_and_scroll_fence` claim follows ordinary focus arbitration, but its
implicit physical reveal is withheld if the fence is interrupted or no longer matches the committed
focus target ancestry.

## Consequences

- Focus does not own a second scrolling runtime, and losing or stale focus claims cannot scroll.
- Virtual Tree focus submits its stable focus claim at input time and materializes only while that
  exact window focus revision remains current. Its physical materialization runs in a terminal
  focus-stable prepaint phase after ordinary commits; focus and blur mutations from that phase are
  rejected, so a later callback cannot invalidate the authority it observes.
- AccessKit and application requests receive the same arbitration, transform, and terminal-outcome
  behavior as focus.
- Table, Tree, Command, Listbox, and VirtualizedList retain domain identity and materialization
  policy while deleting their final container-specific reveal arithmetic.
- Low-level direct scrolling, including an accepted local `request_autoscroll`, remains available,
  but it explicitly interrupts older animated reveal work instead of masquerading as nested
  bring-into-view.
- One stable public target cannot be bound twice in a frame or used in another window.
- Portals do not inherit an implicit source scroll chain.

## Verification

GPUI tests cover every alignment, explicit axes and margins, nested and disjoint chains, shared-chain
supersession, direct-scroll cancellation, no progress, target unlink, suppression, window close,
wrong-window handles, portals, focus arbitration, real AccessKit actions, reduced motion, cached and
deferred bindings, and non-uniform transforms at target and container levels. Public-surface tests
compile the downstream capability while keeping identity and geometry fields opaque.

UI Components tests cover key-based materialization, measured and fixed rows, missing data, reorder,
filtering, stale completion and ABA replacement, Tree and Command consumers, and Table identity.
The foundation Gallery drives direct application, keyboard focus, AccessKit, animation,
direct-scroll cancellation, and virtual materialization through one nested two-axis transformed
scenario.

Focused virtual Tree coverage also proves that a later ordinary prepaint focus claim prevents
materialization, a focus-stable callback cannot introduce a competing claim, and a rejected static
handoff retries only while no newer claim has replaced it.

## Related Documents

- [UI framework authority convergence plan](../plans/2026-07-10-001-refactor-ui-framework-authority-convergence-plan.md)
- [Interactive subtree transform authority](0021-open-gpui-interactive-subtree-transform-authority.md)
- [Subtree presentation authority](0022-open-gpui-subtree-presentation-authority.md)
- [Typed committed portal anchor authority](0024-open-gpui-typed-committed-portal-anchor-authority.md)
- [UI component contract](../ui/component-contract.md)
- [Open GPUI v0.3 UI migration guide](../ui/migration-v0.3.md)
- [Bring-into-view verification ledger](../knowledge/engineering/verification/bring-into-view-authority-20260722.md)
