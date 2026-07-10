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
- availability snapshots and the restoration priority: newer claim, saved live target, nearest
  active ancestor last-live target, registered window fallback, then preserve or clear focus.

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

# U3 Preparatory State

U3 introduces and tests the policy and GPUI adapter seams, including real keyboard dispatch,
nested restoration, stale target fallback, reentrant focus claims, conditional mounting, live-handle
rebinding, explicit unregistration, and multi-window isolation. A logical target is accepted only
when its projected scope is active and its current handle is a real rendered descendant of the
scope root. Exit-painted or otherwise inactive nested scopes are excluded from parent traversal.
The adapter is exposed only through `gpui_adapter` as a low-level composition seam. Target IDs are
canonical and unique within a window: component instances must use instance-qualified IDs, and one
live handle cannot be registered under aliases.

Initial and restoration claims are queued at the end of the state transaction, then resolved from
the next completed rendered frame. This prevents a saved target hidden in the same close transaction
from being restored out of the previous frame merely because another owner still holds its handle.

U3 does not make focus scope the production authority. Existing official overlays continue using
their current focus bookkeeping until their U4 migration. They must not layer the preparatory
runtime on top of those tails.

# U4 Completion Gate

U4 completes this decision only when:

- one window-owned overlay runtime owns layer registration, parentage, dismissal, modality,
  committed close lifecycle, focus scopes, and restoration;
- `FocusScopeRuntime` construction and registration are private to that window runtime; the
  preparatory `gpui_adapter` constructor and direct export are removed;
- the old `ui_core::overlay` stack restore resolver, `FocusRestoreResolution`, and per-layer trigger
  target are deleted so all live target selection uses the focus registry resolver;
- Dialog, Popover, and Menu first migrate as a pilot without family-specific runtime branches;
- the remaining official overlay families migrate in the same authority model;
- component-owned Escape, outside-press, initial-focus, and restoration tails are deleted;
- the shallow `OverlayLayerHost` lifecycle facade is deleted while placement, measurement,
  `anchored`, and `deferred` remain;
- controlled refusal, exit/reopen, callback reentrancy, nested topmost behavior, real focus loops,
  LIFO restoration, trigger loss, and multi-window isolation pass through `TestAppContext`.

# Consequences

- `OverlayFocusTarget` is deleted; overlay intent uses the existing `FocusTargetId` authority.
- GPUI gains contained traversal queries but no second focus manager.
- Runtime registrations use weak handles and re-check a post-transition completed frame before
  focusing.
- Only plain Tab and Shift-Tab participate in modal traversal; modified Tab chords propagate.
- Stable logical IDs survive handle replacement through rebind; unregister removes descendant
  scopes, their targets, fallbacks, and pending claims so an identity can be mounted again.
- When deferred initial and restoration claims coexist, the newest valid initial claim is committed
  first and supersedes the older restoration claim.
- A newer runtime claim or direct programmatic focus change invalidates an older deferred restore.
- Missing restoration candidates never focus arbitrary content; focus is preserved only outside the
  closing scope or safely cleared.
- The ADR must be amended during U4 with the final window ownership mechanism, lifecycle matrix,
  pilot evidence, and deletion evidence.
