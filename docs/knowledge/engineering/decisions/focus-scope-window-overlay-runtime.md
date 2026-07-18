---
type: "Decision"
title: "Focus scope and window overlay runtime ownership"
description: "Keep focus policy renderer-neutral while one per-window overlay runtime owns live GPUI focus and layer arbitration."
timestamp: 2026-07-10T17:00:00+08:00
tags: ["open-gpui", "ui", "focus", "overlay", "adr"]
status: "active"
git_branch: "refactor/ui-framework-authority-convergence"
related_plan: "docs/plans/2026-07-10-001-refactor-ui-framework-authority-convergence-plan.md"
---

# Decision

Open GPUI will use one focus-scope and overlay runtime per GPUI window. Renderer-neutral focus
identity, scope policy, initial and restoration intent, and restoration priority belong to
`open-gpui-ui-core`. Live `FocusHandle` ownership, current-frame initial-target realization and
availability, real Tab and Shift-Tab dispatch, and end-of-turn focus claims remain GPUI adapter
responsibilities.

The runtime must be owned by the window overlay authority. It must not be an application global,
an app-global map keyed by `WindowId`, or a separate runtime created by each component family.
Logical scope and target IDs may repeat in different windows without sharing registrations or
claims.

# Context

Before this decision, `ui_core::overlay` described initial and restoration intent, while Dialog,
Popover, Menu, AlertDialog, Sheet, and choice overlays each stored their own strong focus handles
and executed local focus tails. GPUI tab groups only affected ordering; they did not trap focus.
The final result was locally plausible but had no authority for nested scopes, stale target
filtering, callback reentrancy, or deterministic restoration.

Two reference approaches were rejected as ownership models:

- an app-global focus-trap map cannot deterministically choose nested scopes and is not window
  ownership;
- moving focus through the whole window and scanning until it returns to the trap briefly focuses
  invalid underlay targets and fails for empty scopes.

# Ownership Split

`open-gpui-ui-core` owns:

- `FocusScopeId` and `FocusTargetId`;
- passive and modal-loop scope policy;
- initial and restoration intent plus restoration resolution;
- availability snapshots and the restoration priority: newer claim, saved live target, ordered
  active ancestor targets, registered window fallback, then preserve or clear focus. The public
  `FocusRestoreInput::ancestor_targets` slice is nearest-ancestor first and may contain both that
  scope's last live target and its surface fallback; the narrower `ancestor_last_targets` field was
  removed.

GPUI owns:

- the current frame's tab-stop order and rendered focus tree;
- initial-target realization and contained traversal using `TabStopMap` order plus `DispatchTree`
  ancestry and adapter-projected availability;
- opaque focus-claim revisions used to reject stale deferred claims, including a newer request that
  reasserts the current handle or explicitly clears an already-empty focus;
- opaque rendered-frame revisions used to resolve initial and restoration claims only after the
  activation or close transaction reaches a completed rendered tree.

The concrete adapter owns:

- weak GPUI handles for stable logical IDs;
- the explicit scope parent stack and innermost active modal selection;
- end-of-turn initial and restoration claims;
- per-window application fallback registration;
- atomic scope/target rebinding and explicit unregistration for mount, unmount, and remount.

# Window Ownership Mechanism

U3 introduced and tested the renderer-neutral policy and GPUI focus seams. U4 internalizes those
seams in `WindowOverlayRuntime`: `WindowOverlayRuntime::for_window` obtains one state entity through
`Window::use_window_state`, installs the window input interceptors, and returns another handle to
that same authority on later calls for the window. It is neither an app-global registry nor a map
whose entries must be cleaned up by `WindowId`.

Each layer registration carries a stable ID, immutable parent identity, owner mode, policy, focus
mode, focus-restore condition, and callbacks. `OverlaySurface` contributes live inside-region
geometry and an ambient parent for overlays rendered from its subtree. Entity-bound leases schedule
leaf-to-root cleanup after owner release; a lease, binding, surface, or focus target from another
window is rejected.

Official components use the same ambient-parent mechanism for non-visual composition.
`Menu::overlay_child` wraps each supplied overlay in a Menu-root parent scope, allowing a deferred
Dialog to register as the Menu's logical child without exposing layer IDs or adding a Menu/Dialog
branch to the runtime.

Mounted ownership is rebindable rather than part of layer identity. Dialog, Popover, and Menu may
switch between controlled and uncontrolled ownership on the same lease. The rebind atomically
reconstructs lifecycle state from the newly committed presence, clears a pending intent from the
old owner mode, and preserves monotonic intent revision allocation.

An entity-bound component ID may also be reused before a stale owner's deferred release callback
settles. If the previous owner is no longer live, or belongs to an older frame with no current
inside geometry, component binding cancels every pending focus claim for the old subtree with
restoration disabled, removes that subtree leaf-to-root, and atomically registers a new lease. Lease
tokens, generations, and registration revisions fence late callbacks and cached geometry from the
replaced incarnation.

`FocusScopeRuntime` is now crate-private and constructed only by the window runtime. The former
`gpui_adapter::FocusScopeRuntime` constructor and direct methods are not public migration paths.
Applications and components use the official overlay adapters or the exported
`gpui_adapter::WindowOverlayRuntime` registration API. Target IDs remain canonical within one
window: component instances qualify their IDs, and one live handle cannot be registered under
aliases.

Initial and restoration claims are queued at the end of the state transaction, then resolved from
a completed rendered frame. A logical target is accepted only when its projected scope is active
and its current handle is a rendered descendant of the scope root. Exit-painted and inactive nested
scopes are excluded. A child surface focus also marks its runtime ancestors as entered, so a passive
parent can restore only after its subtree actually owned focus.

# Lifecycle Matrix

The public runtime exposes four committed phases: `Open`, `CloseRequested`, `Closing`, and
`Hidden`. They implement the plan's five observable lifecycle cases as follows:

| Observable case | Phase / presence | Paint and input authority | Modal and focus authority |
| --- | --- | --- | --- |
| Owner is open | `Open` / `Open` | Painted and interactive; eligible as the topmost Escape/outside target | Modal pointer barrier and active focus scope apply; no restore is pending |
| Controlled close requested while owner remains open | `CloseRequested` / `Open` | Still painted and interactive; duplicate intent is suppressed | Registration, modality, focus scope, and saved focus remain unchanged until owner commit or matching rejection |
| Owner commits closed | `Closing` / `Closing` | Exit paint may remain, but surface actions and keyboard dismissal are ineligible | Accessibility/focus authority is removed; a modal pointer barrier may remain until presence ends; one end-of-turn restore is queued |
| Same logical layer reopens during exit | `Open` / `Open` | The existing registration becomes interactive again; a stale `finish_exit` generation is rejected | Pending restore is cancelled and the newer initial-focus claim wins |
| Layer finishes or owner unmounts | `Hidden` / `Hidden` while a quiescent registration is retained, then absent after unregister | No paint, hit testing, keyboard handling, or snapshot entry after unregister | No active trap, pointer barrier, or pending claim survives lease cleanup |

The `CloseRequested` phase is intentional: a controlled callback emits intent against the owner's
currently committed state. It cannot perform semantic close cleanup or restoration. Uncontrolled
state commits before its observer callback. Reentrant callbacks and newly opened layers therefore
produce newer focus claims that an older restore cannot override. Changing ownership on a mounted
layer is a rebind transaction, not an implicit close: the newly committed presence replaces the old
mode's lifecycle and pending intent in one transition.

# U4 Fleet Result

Dialog, Popover, Menu, ContextMenu, Sheet, AlertDialog, HoverCard, Tooltip, Select, Combobox, and
Command overlay mode register through the same `WindowOverlayRuntime` with no family-specific
branch in the runtime. Their old `OverlayLayerHost` forwarding path and component-owned
Escape/outside/focus tails have been removed. Dialog, Sheet, and AlertDialog use modal focus and
pointer barriers; Popover restores only after its subtree owned focus; Menu and ContextMenu register
explicit branches; choice/search overlays preserve their editor/trigger policy; passive Tooltip and
HoverCard layers never claim or restore focus and are transparent to outside-press arbitration.

The real-component integration test
`popover_menu_dialog_escape_is_lifo_and_restores_focus_through_real_components` proves a Popover ->
Menu -> controlled Dialog hierarchy: Menu points to Popover and the Dialog supplied through
`Menu::overlay_child` points to Menu. Successive outside presses close only Dialog, then Menu, then
Popover; reopening the same component instances and pressing Escape restores focus to the Menu
surface, Menu trigger, and Popover trigger in LIFO order. Gallery renders the same real component
hierarchy and asserts the same parent snapshot and Escape restore targets.

Focused runtime pointer tests separately prove that child surface geometry is inside every ancestor,
that an outside press is offered only to the topmost eligible layer, and that Ignore or Consume does
not leak to an underlay. Real Menu tests retain the root consumption boundary while submenu branches
pass eligible presses through for ancestor resolution. Additional focused tests cover controlled
Dialog refusal, mounted ownership changes, callback reentrancy, modal Tab loops, owner
release/remount, stale-owner same-ID replacement, stale branch cleanup, delimiter-safe public Menu
path keys, trigger loss, and two-window isolation.

The pilot established the shared lifecycle without runtime family branches. The U4B fleet reuses
that authority for every remaining official family. `OverlayLayerHost` and the unused runtime
request forwarding helpers are deleted from the repository.

# Fleet Completion Evidence

U3/U4 fleet completion is established by the following invariants and focused gates:

- all official overlay families use the same per-window registration, parentage, lifecycle, input,
  and focus authority;
- caller-declared focus target IDs are layer-local; the runtime alone canonicalizes them and
  reconciles their live handles, availability, and removal across renders;
- every migrated family has deleted its component-owned Escape, outside-press, initial-focus, and
  restoration tail;
- the shallow `OverlayLayerHost` lifecycle facade is absent, while placement, live measurement,
  `anchored`, and `deferred` remain;
- choice/search and passive-overlay deviations follow their family rows in the U4 migration matrix;
- controlled refusal, exit/reopen, callback reentrancy, nested topmost behavior, real focus loops,
  LIFO restoration, trigger loss, and multi-window isolation stay green through `TestAppContext`;
- Gallery, redacted DevTools inspection, migration notes, contracts, and tests move with each fleet
  slice rather than describing a future or parallel authority.

# Consequences

- `OverlayFocusTarget` is deleted; overlay intent uses the existing `FocusTargetId` authority.
- GPUI gains contained traversal queries but no second focus manager.
- Runtime registrations use weak handles and re-check a post-transition completed frame before
  focusing.
- Only plain Tab and Shift-Tab participate in modal traversal; modified Tab chords propagate.
- Stable logical IDs survive handle replacement through rebind; unregister removes descendant
  scopes, their targets, fallbacks, and pending claims so an identity can be mounted again.
- Dialog, Sheet, Popover, and AlertDialog retain only an opaque target-lease set; none rewrites IDs
  or stores a component-owned target registry.
- Stale-owner same-ID replacement cancels the old pending restore before installing a new lease, so
  the previous incarnation cannot steal focus from the remounted component.
- When deferred initial and restoration claims coexist, the newest valid initial claim is committed
  first and supersedes the older restoration claim.
- A newer runtime claim or direct programmatic focus change invalidates an older deferred restore.
- Missing restoration candidates never focus arbitrary content; focus is preserved only outside the
  closing scope or safely cleared.
- `WindowOverlaySnapshot` is a read-only diagnostic projection. DevTools replaces raw layer and
  parent IDs with snapshot-local opaque ordinals before serialization.

# Related Decisions

- [Semantic accessibility and final-tree authority](semantic-accessibility-final-tree-authority.md)
- [Semantic activation authority](semantic-activation-authority.md)
- [Theme scope resolution and deferred capture](theme-scope-resolution.md)
- [ADR 0008: Open GPUI UI Component Productization Roadmap](../../../adr/0008-open-gpui-ui-component-productization-roadmap.md)
- [ADR 0009: Open GPUI Table and Virtualizer Product Shape](../../../adr/0009-open-gpui-table-and-virtualizer-product-shape.md)
- [ADR 0014: Remove Open GPUI Native UI Hybrid Registry](../../../adr/0014-remove-native-ui-hybrid-registry.md)
