# Open GPUI v0.3 UI Migration

Open GPUI v0.3 is an intentionally breaking, pre-release architecture release. Replaced APIs are
deleted rather than retained as aliases. This guide records each migration in the implementation
unit that introduces it.

## Form Lifecycle Authority

`FormStore` now derives `FormStatus` and submission eligibility from active validation plus its
submission phase. Callers no longer coordinate status changes indirectly.

`FormStore` is no longer `Clone`: cloning a live authority made its outstanding tickets ambiguous.
Share `FormSnapshot` or application-owned field values when another consumer needs a copy. Tickets
are scoped to the exact store that created them and are rejected by other stores.

### Async Validation

`ValidationTicket` is opaque and includes the field value revision. Completion returns a typed
result instead of `bool`:

```rust
let ticket = form.begin_async_validation(&email)?;
let completion = form.complete_async_validation(ticket, errors);

match completion {
    ValidationCompletion::Applied => {}
    ValidationCompletion::Stale | ValidationCompletion::Cancelled => return Ok(()),
}
```

An edit, reset, synchronous validation, or newer generation invalidates old work. Starting
validation during submission now returns `FormError::CannotValidateWhileSubmitting`.

Successfully registering a new field also advances the form revision and invalidates an active or
terminal submission because the submitted data shape changed. A rejected duplicate registration
does not mutate lifecycle state.

### Submission

`begin_submit()` now returns `Result<SubmitTicket, FormError>`. Both finish methods require that
ticket and return `SubmitCompletion`:

```rust
let ticket = form.begin_submit()?;

match request_result {
    Ok(()) => {
        let _completion = form.finish_submit_success(ticket);
    }
    Err(error) => {
        let _completion = form.finish_submit_error(ticket, error.to_string());
    }
}
```

Handle `FormError::CannotSubmit { reason }` with `SubmitBlockReason::{Invalid, Validating,
AlreadySubmitting}`. The old free-form rejection reason and ticketless finish methods no longer
exist. Rejected starts do not increment `submit_count`.

### UI Projection

Use `FormProjection::resolve(&snapshot, disabled)` for form-level busy state and submit eligibility.
`FormFieldProjection` now exposes `validating()` and `busy()`, and propagates busy state to Field,
TextInput, Textarea, NumberInput, and Checkbox state. Busy validation does not imply disabled or
read-only input. When rebuilding concrete components from those states, pass `busy(state.busy())`;
all five concrete builders preserve that fact in their resolved `state()`.

### DevTools Boundary

The first-party form adapter no longer forwards caller-selected values, field paths, field ids, or
free-form errors. It emits typed redaction markers, counts, lifecycle facts, and deterministic
opaque field identities before a `DevtoolsCapture` is constructed. Do not parse DevTools payloads
for application form data; application and rendering code should consume `FormSnapshot` directly.

## GPUI Pointer Sessions

Window removal now needs application context so GPUI can deliver terminal pointer cancellation and
clear pressed-button, pointer-capture, and drag state before the window disappears. Replace
`window.remove_window()` with `window.remove_window(cx)`. Removing one window clears only sessions
owned by that window; it does not cancel a drag or capture owned by another window.

The pointer-capture API is a clean break:

| v0.2 | v0.3 |
| --- | --- |
| `window.capture_pointer(hitbox_id)` | Retain a `PointerCaptureHandle`, bind it each frame, then call `window.capture_pointer(&handle, button)?`. |
| `window.release_pointer()` | Call `window.release_pointer(&handle)?`; only that owner can release its capture. |
| `window.captured_hitbox()` | Use `window.captured_pointer()` and inspect `PointerCapture::handle()` plus `PointerCapture::button()`. |
| `hitbox.is_hovered(window)` for captured event routing | Use `hitbox.is_mouse_event_target(window)`; reserve `is_hovered(window)` for physical hover. |

Custom controls that must keep receiving mouse move and button events after the pointer leaves their
hitbox now use a stable, window-owned `PointerCaptureHandle`:

```rust
let capture = window.new_pointer_capture_handle();

// Bind the retained handle while rendering every frame.
let owner = div().track_pointer_capture(&capture);

// Start capture from the owner's mouse-down path after GPUI records the pressed button.
window.capture_pointer(&capture, event.button)?;

// End capture explicitly when custom interaction logic completes.
window.release_pointer(&capture)?;
```

Create and retain the handle once for the control instance, then bind that same handle with
`track_pointer_capture` in every rendered frame. `capture_pointer` requires both a current-frame
binding and an already pressed initiating button; creating a fresh handle during each render or
capturing before the mouse-down dispatch is rejected. Custom elements may use
`Window::bind_pointer_capture` after inserting their hitbox instead of the standard element helper.

`window.release_pointer(&capture)` returns `Result<bool, PointerCaptureError>` for explicit release.
GPUI also releases capture when the initiating button goes up, the owner is absent from the next
frame, the window deactivates, pointer cancellation occurs, or `remove_window(cx)` closes the owner
window. Use
`window.captured_pointer()` when diagnostics need the active handle and initiating button; it
returns `Option<PointerCapture>`, whose `handle()` and `button()` accessors preserve that ownership
identity.

Pointer capture separates event routing from visual hover. Mouse event handlers use
`Hitbox::is_mouse_event_target(window)` (or the equivalent `HitboxId` method) so the captured owner
continues receiving routed events outside its bounds. Visual hover, cursors, tooltips, and drag-over
feedback continue to use `is_hovered(window)`, which follows the physical pointer and input
modality. Do not use physical hover to gate a captured mouse-up, and do not use the captured event
target to paint hover outside the hitbox.

## Window Overlay Runtime

Focus target identity is now owned by `FocusTargetId`. The duplicate `OverlayFocusTarget` type was
deleted without an alias. Update explicit overlay focus policies directly:

```rust
use open_gpui_ui_core::{FocusTargetId, InitialFocusIntent};

let initial_focus = InitialFocusIntent::TargetOrFirstFocusable(
    FocusTargetId::new("dialog.primary-action"),
);
```

An explicit target ID now names a real handle registered by the component adapter. Dialog, Sheet,
and Popover expose the same declaration API:

```rust
use open_gpui_ui_components::dialog::Dialog;
use open_gpui_ui_components::gpui_adapter::FocusTargetRegistration;

let dialog = Dialog::element("confirm", "Open", "Confirm", content)
    .initial_focus_intent(initial_focus)
    .focus_target(FocusTargetRegistration::new(
        "dialog.primary-action",
        &primary_action_focus,
    ));
```

The ID passed to `InitialFocusIntent` and `FocusTargetRegistration` must match exactly and is local
to that overlay layer. Do not include a component instance or layer prefix. `WindowOverlayRuntime`
owns canonical window identity, live availability, rerender rebind, and stale-target removal.

### Canonical Overlay Presence

`OverlayPresence::from_parts(open, present, interactive)` now returns `Option<OverlayPresence>` and
accepts only the three canonical lifecycle states:

- `Hidden`: `(false, false, false)`
- `Open`: `(true, true, true)`
- `Closing`: `(false, true, false)`

Every other flag combination returns `None`; callers must reject inconsistent lifecycle input
instead of relying on normalization. Prefer `OverlayPresence::{hidden, open, closing}` when the
semantic state is already known, and use `OverlayPresence::from_open` only for an instant open/hidden
surface with no exit-presence phase.

### Focus Restore Inputs

`FocusRestoreInput::ancestor_last_targets` was renamed to `ancestor_targets` without a compatibility
field. Update struct literals and helper parameters to the new name. The new name is intentional:
the nearest-first slice may contain both an ancestor scope's last live target and its surface
fallback, rather than exactly one last-live target per ancestor.

```rust
let input = FocusRestoreInput {
    newer_claim,
    saved_target,
    ancestor_targets: &ancestor_candidates,
    window_fallback,
    current_target,
};
```

GPUI elements that combine `track_focus` with `tab_stop` or `tab_index` now apply the element's
declared tab configuration to the explicit handle in either builder order. Code no longer needs to
preconfigure a separate cloned handle merely to enter the rendered tab order.

The preparatory `gpui_adapter::FocusScopeRuntime` constructor and its direct registration methods
were deleted from the public surface. There is no compatibility alias. Focus-scope construction is
now crate-private to the window-owned overlay authority.

Dialog, Popover, and Menu formed the U4A pilot. The completed fleet also includes ContextMenu,
Sheet, AlertDialog, HoverCard, Tooltip, Select, Combobox, and Command overlay mode. Every official
family now obtains the unique runtime for its GPUI window, registers stable layer and parent
identities, and lets that runtime arbitrate Escape, outside press, modal barriers, controlled close
intent, focus claims, and restoration. Applications using official components do not install or
forward an additional runtime.

Dialog's default outside policy changed from `OutsidePressPolicy::Consume`, which only blocked the
press, to `OutsidePressPolicy::DismissAndConsume`, which requests close and prevents underlay
activation. Code that deliberately needs a sticky Dialog must now set `Consume` or `Ignore`
explicitly. Popover keeps pass-through outside dismissal, but it restores its trigger only after
focus entered the Popover surface or a registered descendant. Closing a Popover that never owned
focus therefore preserves the application's newer focus claim.

Mounted Dialog, Popover, and Menu instances may now switch between controlled and uncontrolled
ownership without changing their element ID or forcing a remount. Adding or removing the `.open(...)`
builder on a later render atomically adopts that render's committed presence and clears any pending
intent from the previous ownership mode. In particular, switching away from a controlled
`CloseRequested` state does not leave a stale close request suppressing the new owner.

### Menu Logical Children

Use `Menu::overlay_child` when another overlay must remain a logical child of the Menu root. The
Menu installs a parent scope around each supplied element, so a deferred Dialog or other official
overlay registers with the Menu layer as its parent instead of becoming the Menu's sibling under an
outer Popover. This is the supported composition path for Popover -> Menu -> Dialog dismissal,
ancestor inside-region handling, subtree teardown, and LIFO focus restoration; do not reconstruct
component-generated layer IDs in application code.

### Menu Path-Key Encoding

All six public Menu path-key accessors now percent-encode each path segment before joining segments
with `/`:

- `MenuItemState::path_key`
- `MenuSelection::path_key`
- `MenuSubmenuNavigation::{open_path_key, focused_path_key}`
- `MenuState::{open_path_key, focused_path_key}`

Within a segment, `%` becomes `%25` and `/` becomes `%2F`, in that order. For example, the path for
values `parent/%` and `child/%` is now
`0:parent%2F%25/0:child%2F%25`. This is a breaking public output-format change: update persisted
keys, selector fixtures, caches, and equality assertions. Code that parses a compact key may split
on `/` only as the encoded segment boundary and must percent-decode each segment before treating it
as the prior unescaped path-segment representation. Prefer the slice-returning `path`, `open_path`,
and `focused_path` accessors when unencoded segment strings are required.

Custom GPUI overlay adapters that genuinely need the low-level boundary migrate to the window
runtime registration API:

```rust
use open_gpui_ui_components::gpui_adapter::{
    OverlayLayerRegistration, OverlayOwnership, WindowOverlayRuntime,
};

let runtime = WindowOverlayRuntime::for_window(window, cx);
let registration = OverlayLayerRegistration::new(
    "app.settings-dialog",
    policy,
    OverlayOwnership::Controlled,
);
let binding = runtime.register_layer_for_entity(registration, &owner, window, cx)?;
```

Build `registration` from a stable, instance-qualified layer ID, an `OverlayLayerPolicy`, and
`OverlayOwnership`. Declare parentage on `OverlayLayerRegistration`, retain or entity-bind the
returned `OverlayLayerBinding`, wrap rendered overlay content with `WindowOverlayRuntime::surface`,
and use the binding's runtime-owned trigger/surface handles or explicit focus-target registration.
Rebind the same lease when policy or committed presence changes. Do not recreate a runtime to model
mount, unmount, or rerender; repeated `for_window` calls return handles to the same window state.

The runtime validates both logical ownership and the current GPUI render tree. A named target that
is outside its declared scope, unavailable, stale, unmounted, or owned by an inactive nested scope
is ignored. Initial focus requested before conditional content mounts is retried after the next
completed frame. Use `rebind_layer` and `rebind_focus_target` when stable identities gain current
policy or handles, and use the matching unregister methods only for explicit manual lifetime. The
preferred `register_layer_for_entity` path binds subtree cleanup to owner release.

Official component adapters also handle a new entity mounting with the same stable component ID
before the old owner's deferred release cleanup has settled. When the old owner is gone or stale for
the current frame, the runtime cancels the old subtree's pending restore, tears down its leases and
focus registrations without restoring through stale state, and atomically installs the replacement
binding. The replacement receives a new lease token, so callbacks, geometry journals, and focus
work captured by the old incarnation cannot affect it. This replacement behavior belongs to the
component binding path; low-level manual registrations still report duplicate live IDs.

Target IDs are canonical within one window. Component instances must qualify their IDs so two live
instances do not collide, and one live handle cannot be registered under multiple aliases. Modal
containment intercepts only plain Tab and Shift-Tab; Tab chords with Control, Alt, platform, or
function modifiers continue through normal dispatch. If initial-focus and restoration work become
pending in the same turn, the newest valid initial-focus claim wins so reopening cannot be undone by
an older close. Initial and restoration targets are resolved after the state transition reaches a
completed rendered frame, so a target hidden in the same transaction is treated as unmounted even
when another owner still holds its handle.

The old stack-only `FocusRestoreResolution` and per-layer trigger target bookkeeping were removed.
They could select an unmounted trigger without consulting live registrations. The renderer-neutral
resolver is now exported as `resolve_focus_scope_restore`; the window runtime supplies its
live-handle validation and applies the returned resolution rather than resolving a focus target
from an `OverlayLayer` snapshot.

Controlled close callbacks are intent notifications. Keeping the controlled value open keeps the
layer registered, modal, and focused; accepting the request requires the owner to commit closed
presence on a later render. Do not restore focus or remove a barrier from the callback itself.
Uncontrolled components commit framework state before notifying observers. In either mode, a newer
focus claim made by a callback wins over an older deferred restore.

`Sheet::on_close` was removed without a compatibility alias. Replace it with
`Sheet::on_open_change`, whose callback receives an `OverlayOpenIntent`; run close-request behavior
only when `intent.desired_open()` is false. The typed callback also identifies the dismissal reason
and, for controlled state, represents an intent rather than proof that the owner committed closed
presence.

The crate-private `OverlayLayerHost`, `OverlayOpenRuntimeRequest`, and their forwarding helpers were
deleted after the final fleet caller moved to `WindowOverlayRuntime`. There is no compatibility
facade. Official adapters no longer own parallel Escape, outside-press, initial-focus, or
focus-restoration tails.

HoverCard and Tooltip retain component-owned delay/epoch policy but register every visible surface
with the window runtime. They never claim or restore focus. Outside press is transparent and maps to
`OutsidePressPolicy::Ignore`; it is not a HoverCard or Tooltip ownership event. HoverCard still
allows Escape dismissal, while Tooltip retains its descriptive default Escape policy. The removed
HoverCard `.outside_press_policy(...)`, `.initial_focus_intent(...)`, and
`.focus_restore_intent(...)` builders must not be replaced with local event handlers.
