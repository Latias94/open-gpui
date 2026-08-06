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

## Theme Scope Authority

Theme selection is no longer owned by a public app-global `ThemeRuntime`. Install definitions and
select app or window authority through the explicit theme-owner API, then resolve the effective
owned context with the window-aware resolver:

```rust
use open_gpui_ui_components::theme::{
    ThemeContext, ThemeResolver, ThemeScope, install_theme_registry, set_app_theme,
    set_window_theme,
};

install_theme_registry(cx, registry, "light")?;
set_app_theme(cx, "dark")?;
set_window_theme(window, cx, "high-contrast")?;

let effective = ThemeResolver::current(window, cx);
let subtree = ThemeScope::new("settings-theme", ThemeContext::dark(), child);
```

The precedence is nearest subtree scope, window selection or explicit override, app selection,
then the built-in light fallback. `ThemeScope::new` now requires a stable element id so a context
change can invalidate cached child-view journals. Window state is owned by the window and is
dropped on close. Unknown ids and equal selections are atomic no-ops and do not refresh unrelated
windows. Replacing the complete registry does not silently demote a window whose selected id is
temporarily absent: it retains the last-known snapshot until that id returns or
`clear_window_theme` explicitly restores app inheritance.

Official overlays freeze the effective context when their lifecycle generation opens. Trigger
styling follows the current context, but surface colors and deferred children retain the opening
context until close; reopening captures again. Delayed Button, IconButton, Menu, Sidebar, and
Toolbar tooltips capture automatically. Direct GPUI tooltip attachment must make the boundary
explicit:

```rust
use open_gpui_ui_components::Tooltip;

let context = ThemeResolver::current(window, cx);
let trigger = div().tooltip(Tooltip::scoped(
    context,
    Tooltip::text("Saved"),
));
```

When both a custom tooltip builder and text fallback are present, official Button, IconButton, and
Toolbar adapters attach only the custom builder. The former `ThemeRuntime`, `ThemeRuntimeError`,
`ThemeResolver::current(&App)`, `ThemeResolver::resolve`, `init_theme_runtime`,
`current_theme_context`, `try_theme_context`, and `set_active_theme*` surfaces are deleted without
aliases. The scope mechanism remains theme-specific because no independent non-theme consumer met
the adoption gate for a public generic inherited-context API.
See [Theme scope resolution and deferred capture](../knowledge/engineering/decisions/theme-scope-resolution.md)
for the render-timing prototype and rejected generic-context decision.

### Complete Theme V1 Replacement

Theme v1 is no longer a color-only definition. `ThemeSnapshot` owns a complete color table plus
typed typography, spacing, radius, elevation, density, and motion-policy scales. `ThemeContext`
adds the runtime-owned effective revision used for invalidation. Serialized `revision` remains
source metadata and cannot force or suppress runtime refreshes.

Code-built definitions must now supply the complete payload. The most direct migration for a
derived theme starts from a complete snapshot:

```rust
use open_gpui_ui_components::theme::{ThemeDefinition, ThemeSnapshot};

let base = ThemeSnapshot::dark();
let definition = ThemeDefinition::from_snapshot("editor-dark", "Editor dark", &base);
```

Use `ThemeDefinition::design_scales(...)` and `.colors(...)` when replacing those complete values.
Registration rejects missing, duplicate, or unsupported color entries and missing design scales
before it mutates the registry. Metadata-only reloads preserve the effective revision; changed
effective content and changed app/window/subtree selection allocate a new monotonic revision.

Theme JSON remains schema version 1 but its shape is intentionally replaced in place. Files must
contain `design.typography`, `design.spacing`, `design.radius`, `design.elevation`,
`design.density`, and `design.motion_policy`, as well as the complete color table. Regenerate or
serialize complete files with `theme_json_string`. The old `fallback_mode`, partial-color fill,
`ThemeRegistrationDiagnostics`, caller-supplied runtime revision, and color-only loader are deleted
without aliases or compatibility parsing.

Component sizing now resolves as explicit `Size` first, then the theme density default. Theme and
component motion preferences merge to the stricter value: either side can request reduced motion,
and an explicit animated request cannot override a reduced theme. Adaptive device density remains
an application-shell recommendation rather than an implicit theme override.

## Dock Visual Style Authority

`open-gpui-docking` no longer chooses colors and shadows independently in each render path.
Every host resolves one complete immutable `DockVisualStyle`. Applications that do not install a
resolver receive the deterministic built-in style. Applications with a theme system install a
read-only resolver on the facade:

```rust
use open_gpui_docking::{
    DockSurface, DockVisualPalette, DockVisualStyle, DockVisualStyleResolver,
};
use open_gpui_ui_components::theme::ThemeResolver;

let resolver = DockVisualStyleResolver::new(|window, cx| {
    let theme = ThemeResolver::current_snapshot(window, cx);
    let palette = application_dock_palette(&theme);
    DockVisualStyle::from_palette(palette)
});

let surface = DockSurface::builder("main")
    .visual_style_resolver(resolver)
    // Register panels and policy as before.
    .build(cx)?;
```

The application-owned `application_dock_palette` helper must construct every
`DockVisualPalette` field deliberately. Neither `DockVisualPalette` nor `DockVisualStyle`
implements `Default`; use `built_in()` only when the application explicitly chooses the
deterministic fallback. `DockVisualStyle::from_palette` returns
the complete host, tab, splitter, floating, drag, preview, guide, route, transition, focus, and
elevation style; Docking never fills a partial application style during rendering.
`ThemeResolver::current_snapshot` is the read-only adapter boundary. It observes subtree, window,
app, and built-in precedence without initializing window state, updating entities, notifying,
dispatching, or scheduling a refresh.

Low-level multi-window integrations install the same immutable resolver with
`DockViewportRuntimeHandle::with_visual_style_resolver` or
`with_close_policy_and_visual_style_resolver`. An explicit `DockHost` may use
`from_controller_with_visual_style_resolver`. The resolver is evaluated in each host's active
window and subtree context. It must not reenter Dock rendering.

Source and destination visuals intentionally use different timing. A source-owned drag preview
freezes its `DockDragVisualStyle` for the drag session's opening generation. Target-owned guides
and previews resolve the destination host's current style. Cancellation clears the captured
metadata, and reopening the same payload captures again. No style enters `DockDragPayload`, so
payload equality, route validation, and persistence are unchanged.

`DockDropGuideStyle`, the `drop_guide_style` builders, and the
`DockHostOptions::drop_guide_style` field are deleted without aliases. The value contained only
structural dimensions and hit-test sizing, so migrate both builder calls and direct option
struct literals to `DockDropGuideMetrics` and `drop_guide_metrics`:

```rust
// Before
let builder = builder.drop_guide_style(DockDropGuideStyle::default());

// After
let builder = builder.drop_guide_metrics(DockDropGuideMetrics::default());

// Direct DockHostOptions literals
let options = DockHostOptions {
    drop_guide_metrics: DockDropGuideMetrics::default(),
    ..DockHostOptions::default()
};
```

Dear ImGui remains an interaction reference for tab, inner/outer target, accepted/rejected preview,
and tear-off behavior. This change does not copy ImGui's colors, immediate `ImGuiDockContext`,
binary node tree, builder API, or settings format. See
[ADR 0027](../adr/0027-open-gpui-dock-visual-style-authority.md).

## Dock Surface Change And Activation Authority

`DockSurface` is now a cloneable handle to one private application-level owner instead of a loose
controller/runtime facade. All clones share one monotonic committed revision and one typed change
stream. Subscribe to lightweight metadata, apply application-owned debounce, and export the
revision-consistent snapshot only when your persistence policy requires it:

```rust
use open_gpui_docking::prelude::DockSurface;
use std::{cell::Cell, rc::Rc, time::Duration};

let pending = Rc::new(Cell::new(false));
let pending_for_events = pending.clone();
let surface_for_events = surface.clone();

surface
    .subscribe_changes(cx, move |event, cx| {
        log::debug!(
            "dock revision={} categories={:?}",
            event.revision(),
            event.categories()
        );
        if pending_for_events.replace(true) {
            return;
        }

        let pending = pending_for_events.clone();
        let surface = surface_for_events.clone();
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            cx.update(|cx| {
                persist(surface.export_snapshot(cx));
                pending.set(false);
            });
        })
        .detach();
    })
    .detach();
```

The event contains only a revision and bounded categories: layout, selection, panel lifecycle,
viewport topology, and observed viewport placement. It does not contain a snapshot. Failed,
unchanged, focus-only, style-only, and native-mutation-dispatch-only work does not advance the
revision. `DockSurfaceSnapshot::revision()` identifies the committed revision paired with both its
layout and viewport-placement facts; older serialized snapshots without the field load as revision
zero.

Use stable item ids for product activation:

```rust
let (_request, completion) =
    surface.activate_panel_with_completion("editor", cx, |outcome, _cx| {
        log::debug!("editor activation settled: {outcome:?}");
    });
completion.detach();
```

`select_panel` remains selection-only. Activation may first commit selection and later settle as
`Committed`, `Rejected`, `Superseded`, `Unavailable`, `DuplicateHostConflict`, or `WindowClosed`
from exact descendant GPUI focus completion. Dropping the returned subscription stops observing
the result but does not cancel the issued intent. Product code must no longer call node-id
`DockHost::focus_pane`; that primitive is crate-private.

Applications should delete generic-notify, end-of-turn, render-count, or snapshot-diff persistence
inference. Docking intentionally has no persistence timer, built-in debounce duration, path
setting, or file writer. See
[ADR 0028](../adr/0028-open-gpui-dock-surface-change-and-activation-authority.md).

## Dock Surface Window Sessions

Facade-managed native Dock windows now belong to one exact-generation surface session. Rename
the `DockSurfaceViewportSession` facade type to `DockSurfaceViewports`, keep using the existing
`viewports()` accessor, and stop treating `open_primary_window` as a bare GPUI window-open result:

```rust
use open_gpui_docking::prelude::{
    DockSurfacePrimaryWindowOpenOutcome, DockSurfaceViewportOpenOutcome,
    DockSurfaceViewportUnavailable,
};

let primary = match surface.open_primary_window(primary_options, cx) {
    DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened,
    DockSurfacePrimaryWindowOpenOutcome::Unavailable(reason) => {
        return Err(format!("Dock anchor unavailable: {reason:?}"));
    }
};

let viewport = match surface.viewports().open("preview", preview_options, cx) {
    DockSurfaceViewportOpenOutcome::Opened(opened) => opened,
    DockSurfaceViewportOpenOutcome::Unavailable(
        DockSurfaceViewportUnavailable::SessionInactive { status },
    ) => {
        return Err(format!("Dock session is not active: {:?}", status.phase()));
    }
    DockSurfaceViewportOpenOutcome::Unavailable(reason) => {
        return Err(format!("Dock viewport unavailable: {reason:?}"));
    }
};
```

The primary outcome exposes the exact committed window and session generation. Managed viewport
open, restore, registration, activation, drag, route, mutation, and observation work is admitted
only for that generation. `DockSurface::window_session_status` reports `Vacant`, `Opening`,
`Active`, `ShuttingDown`, or `Closed`, plus the anchor, rollback/shutdown reason, terminal ticket
counts, and runtime convergence.

Delete application close observers that call `App::quit` when the Dock primary closes. The first
ordinary anchor close request freezes the surface, force-retires that surface's dependent windows
before removing the anchor, and bypasses per-viewport `Prevent` or `MergeBack` policy. It does not
close another `DockSurface`, an unmanaged low-level runtime, or an unrelated application window.
GPUI's last-window policy remains the application-exit authority. Reopening a surface is rejected
until the previous exact generation reaches `Closed`. Do not treat a close dispatch or disappearance
from GPUI's logical window registry as native completion. Ordinary teardown settles from the exact
platform native-terminal callback. App shutdown may clear the logical registry first, but GPUI
retains every detached platform owner and Dock remains `ShuttingDown` until those exact native
terminals arrive.

An embedded `surface.host_view(cx)` remains outside this managed lifecycle and never invents an
anchor or registers managed route and activation authority. It is a rendering path for content in
an application-owned window; facade activation reports `Unavailable` until a managed host exists.
Use `surface.open_primary_window(...)` before `surface.viewports()` when Dock should own the native
window tree. Applications that need custom window ownership must explicitly construct the low-level
runtime and host from `open_gpui_docking::runtime` rather than borrowing a facade session. See
[ADR 0030](../adr/0030-open-gpui-dock-surface-window-session-authority.md).

## Platform Window Mutation Authority

Already-open native windows now use a capability-specific request and observation contract.
`WindowMutationSupport::Live` means the backend can both dispatch the property and later supply
readable facts for it. `CreationOnly` means the property belongs in `WindowOptions` when opening a
window; it is not a valid live request. `Unsupported` is explicit.

Use `WindowPlacementRequest` for live placement rather than inferring success from a setter:

```rust
use open_gpui::{
    WindowMutationDispatch, WindowPlacementRequest, px, size,
};

let dispatch = window.request_window_placement_request(WindowPlacementRequest {
    size: Some(size(px(1280.0), px(800.0))),
    ..Default::default()
});

match dispatch {
    WindowMutationDispatch::Queued(ticket) => {
        // Retain the ticket and any ticket subscription in application state when the
        // terminal outcome matters.
        pending_window_mutation = Some(ticket);
    }
    WindowMutationDispatch::Unchanged => {}
    WindowMutationDispatch::Unsupported
    | WindowMutationDispatch::Rejected
    | WindowMutationDispatch::WindowClosed => {}
}
```

`Queued` means only that GPUI handed the request to the backend. It does not update
`Window::platform_facts`, `bounds`, `window_bounds`, fullscreen/minimized state, any independent
flag, or Dock placement. A retained ticket later settles once as `Exact`, `Adjusted`,
`Superseded`, `Rejected`, `Unsupported`, or `WindowClosed`; the terminal observation includes the
committed `WindowPlatformFacts`. Every backend terminal carries its domain and generation. GPUI
rejects a stale generation before committing its facts, so an older callback cannot settle a newer
ticket or roll the public cache backward. Dropping a subscription stops callback delivery but
never cancels the request or its terminal record.

Position, size, windowed/maximized/fullscreen/minimized state, and restore bounds are one
placement conflict domain. Pointer input, coherent activation policy, alpha, topmost, and taskbar
visibility are five independent domains. A newer legal request supersedes only the older
request in its own domain, while an invalid placement request does not disturb a pending one.
Closing a window first invalidates every queued backend generation, then settles retained tickets
as `WindowClosed`. `WindowBounds` remains the compatibility projection for windowed, maximized,
and fullscreen creation or requests; use `WindowPlacementRequest` for partial updates and
minimized state.

`Window::request_window_mutation` accepts the complete `WindowMutationRequest` vocabulary.
`request_pointer_input`, `request_activation_policy`, `request_topmost`,
`request_taskbar_visibility`, `set_background_appearance`, `resize`, `zoom_window`,
`minimize_window`, and `toggle_fullscreen` are ergonomic typed wrappers over that same authority
and now return a `must_use` `WindowMutationDispatch`. The state helpers request only the target
state; they do not copy restore geometry into the request. Handle the dispatch or explicitly bind
it to `_` when the terminal result is intentionally ignored.

`PlatformViewportCapabilities::live_window_move`, `PlatformViewportFlagCapabilities`, and
`App::viewport_flag_capabilities()` have been deleted. Inspect
`Window::window_capabilities()` instead. The `mutations` matrix covers placement, pointer input,
the two-field activation policy, alpha, topmost, taskbar visibility, and coordinate space. Windows
currently exposes live size, windowed/maximized/fullscreen, and pointer-input mutation, but reports
window-local coordinates and keeps position plus restore bounds creation-only until mixed-DPI
desktop coordinates are comparable. Other native backends conservatively report only their
creation-time or unsupported properties. Capability lookup is per `WindowKind`: Wayland
LayerShell windows do not inherit XDG windowed/maximized/fullscreen/restore claims.
When code has only an opened `AnyWindowHandle`, use `App::window_profile(handle)` to read
the immutable profile captured for that window's actual creation kind and target display. Do not call
`App::window_capabilities()` and assume its normal-window matrix describes floating,
popup, or LayerShell windows.

The former `WindowOptions::focus` field and live focus-on-appearing/click mutation requests have
been deleted. Use `WindowOptions::focus_on_appearing` only for the first native appearance. It
never means permanent non-activation. Set `WindowOptions::activation_policy` for lifetime
`accepts_activation` and `focus_on_click` behavior; both fields share one mutation generation and
terminal observation. Pointer-input acceptance remains independent.

To establish a native top-level owner relationship, create a token with
`App::transient_window_owner(live_handle)` and assign it to `WindowOptions::transient_for`. The
token is bound to the exact live window generation and application. Self, stale, closed, or
foreign-app owners are rejected before native creation. Native ownership assists grouping,
activation, minimization, and z-order only; applications must still close subordinate windows
explicitly.

Creation and presentation history no longer leak through lifetime flags. Read
`Window::creation_facts()` for the immutable applied first-appearance and owner facts. Read
`Window::presentation_facts()` to distinguish native creation, accepted frame, submitted present,
first non-empty present, the exact latest present attempt, bounded initial-presentation settlement,
and current native visibility.

### Custom platform backends

Third-party `Platform` implementations must accept the target display when projecting creation and
live support:

```rust
fn window_capabilities(
    &self,
    kind: &WindowKind,
    display_id: Option<DisplayId>,
) -> PlatformWindowCapabilities;
```

`None` means the backend's primary or default display. `App` normalizes an unavailable requested
display id to `None` before both capability lookup and window creation, so restoring a stale display
does not leave a profile and native constructor disagreeing. Use the supplied display for
capabilities that depend on native resources, such as an X11 screen's transparent visual.
`WindowParams` now contains the mandatory canonical `window_bounds`, `focus_on_appearing`,
`activation_policy`, and validated `transient_for`; use them as independent creation inputs while
`bounds` remains a compatibility projection.

`PlatformWindow::creation_facts` must report the exact applied immutable creation facts,
`is_visible` must report native visibility, and `draw` must return a truthful
`PlatformWindowPresentOutcome`. `PlatformWindow::platform_facts` must return one coherent observed
snapshot. Set `initial_presentation_order` to `BeforeVisibility` only when a frame can be submitted
while hidden, `AfterVisibility` only when native visibility precedes submission, or
`PresentationEstablishesVisibility` when the first submission itself maps the native surface. A
property may be
reported as `Live` only when the backend implements the typed `prepare_window_mutation`,
`request_window_mutation`, and `invalidate_window_mutation` paths and can emit a terminal
generation-bound observation through `on_window_mutation_observation`. Use
`on_window_state_change` to refresh committed external facts; it does not settle a queued ticket.
The former parallel resize, pointer-input, minimize, zoom, and fullscreen backend methods were
deleted rather than retained as bypasses.

Return `PlatformWindowPresentOutcome::Deferred` only when retrying the same scene can still
succeed. Return `RepaintRequired` after renderer-local resource identity changes, including an
atlas reset or successful device recovery. GPUI then invalidates that exact scene generation and
keeps a fresh-paint request authoritative across inactive and thermal throttling; the backend must
not maintain a competing one-shot redraw flag. A hidden initial window remains gated while this
bounded recovery runs and closes if no newer non-empty submitted generation can be produced.
Fresh-generation attempts and same-generation `Deferred` retries have independent finite budgets
during this pre-visibility recovery; ordinary visible-window deferred presentation keeps its
existing retry semantics.

Dock status now records requests and dispatches separately from terminal observations, and each
observation preserves its typed request plus committed facts. Only committed Window facts can
create an observed viewport-placement revision. Terminal rejected, adjusted, unsupported, or
closed requests are not retried every frame, including when a viewport reuses an existing GPUI
window; Dock retries only after the target or relevant committed facts change. Placement retry
fingerprints deliberately exclude focus, pointer, and unrelated flag facts. DevTools preserves
per-window kind/capability profiles and queued requests as structured payloads rather than unstable
debug strings. See
[ADR 0029](../adr/0029-open-gpui-platform-window-mutation-capabilities.md).

## Native Window Callback And Command Boundary

Native window callbacks no longer own ad hoc `AsyncApp::update_window(...).log_err()` retry or
drop behavior. GPUI classifies every callback before wiring it: asynchronous facts and events enter
the typed `AppCell` ingress, synchronous queries read committed snapshots or conservatively prevent
native action, and `on_input` returns the exact handler-derived native disposition immediately.
The canonical callback-by-callback table and all coalescing, FIFO, barrier, stale, and terminal
rules are in
[ADR 0029](../adr/0029-open-gpui-platform-window-mutation-capabilities.md#callback-delivery-and-reentrancy).

For custom platform backends:

- Continue storing and invoking the `PlatformWindow` callbacks supplied by GPUI. Do not call back
  into `App` through a backend-specific side channel.
- Implement `command_dispatcher()` with a weak reference to native window state. Dispatch only
  `PlatformWindowCommand::{CompleteInitialPresentation, RevealDeferredInitialPresentation,
  Activate, ShowWindowMenu, StartWindowMove, StartWindowResize}` and return
  `PlatformWindowCommandOutcome::Rejected` without side effects after that native state is gone.
  Return `Accepted` only when the backend accepted the requested operation. A provisional reveal
  must validate the exact session and presentation generations, retain the same native window,
  avoid activation, preserve native hit transparency, and call
  `WindowProvisionalSession::record_native_reveal` with observed visibility, foreground, hit-test,
  identity, and z-order facts before reporting acceptance.
- If the backend can own native pointer capture, construct the dispatcher with
  `PlatformWindowCommandDispatcher::new_with_pointer_capture_release`. Its preparer may only
  snapshot the exact native owner for the supplied release generation; the retained operation
  performs release after the `App` borrow is gone. Report `Released` only after capture is absent,
  `NativeWindowTerminal` only for the exact terminal native window, and otherwise `Rejected` so
  GPUI retains the same snapshot and retries. Backends that never own native capture may use
  `PlatformWindowCommandDispatcher::new`.
- Implement the mandatory `PlatformWindow::prepare_presentation_shutdown` seam. Preparation must
  only capture backend-owned state. Its retained operation runs after the `App` borrow is gone,
  drains renderer and surface work for the exact ticket, calls
  `WindowPresentationShutdownTicket::acknowledge_quiesced`, and only then returns
  `PlatformPresentationShutdownOutcome::Quiesced`. Keep `retire_native_window` rejected until
  quiescence is proven; a fallible native destroy remains retryable while GPUI retains the owner.
- Keep `map_window` synchronous and unable to pump `on_input`. A backend may keep the native target
  hidden, or perform only a backend-proven non-activating map/commit whose callbacks cannot enter
  the hybrid input path. Any show, focus, or activation work that can pump input belongs to
  `CompleteInitialPresentation`. Rejection must preserve pending show/placement intent where the
  backend owns it. GPUI performs one bounded retry, for two diagnosed attempts total, and publishes
  initial-presentation completion only after acceptance.
- Treat `on_input` as an idle-only synchronous contract. Every `PlatformInput` variant must return
  the current `DispatchEventResult`; a default result, delayed replay, or callback-local retry is
  a correctness bug even if one operating-system message currently ignores the result.
- Keep `on_hit_test_window_control` read-only. It returns the committed window-control snapshot and
  must not traverse the live element tree.
- When `on_should_close` returns `false`, leave the native window alive. GPUI may have queued an
  ordered close intent because the application was busy; its later ordered delivery owns removal.
- Permit callbacks during native construction. GPUI has already reserved the full generational
  `WindowId`; callbacks wait for commit and are discarded against that exact ID after rollback.
- Do not consume native paint invalidation without either handing GPUI an accepted frame request
  with a guaranteed wake or explicitly re-invalidating and scheduling another callback. Merge
  repeated requests by OR-ing `force_render` and `require_presentation`.

For application, Dock, and component code, replace direct pump-sensitive backend calls with the
existing `Window` methods. They enqueue the closed command set and dispatch only after the outer
application borrow and older callback barriers are released. Operations that hold an entity,
controller, or viewport-runtime guard should first compute typed effects, drop the guard, and then
apply those effects through the current `&mut App`. Do not migrate such code to a
`Box<dyn FnOnce>`, arbitrary task queue, or asynchronous open-window outbox.
Direct `PlatformWindow` activation, window-menu, move, and resize methods have been removed. Use
the corresponding `Window` methods so every pump-sensitive operation crosses the same post-borrow
FIFO.

The observable ordering rules are intentionally strict:

- `AppCell` allocates one application-wide native-event sequence before any borrow attempt,
  inline-delivery decision, or coalescing.
- Fact domains coalesce only at the adjacent queue tail for the same full `WindowId` and relevant
  generation. Close, pointer cancellation, accessibility actions, system-tab commands, and
  mutation terminals remain FIFO and non-droppable.
- A drain handles at most 64 events before yielding and scheduling another foreground wake.
- A close barrier settles retained mutation tickets and retires the full ID before a reused slot
  can receive callbacks.
- Platform-command FIFO sequencing is separate for diagnostics, but dispatch waits for older
  ingress barriers. Commands enqueued by a command append rather than recurse, and each backend
  attempt terminates as accepted or rejected.

The component-side constraints are also recorded in the
[Open GPUI Component Contract](component-contract.md#native-window-callback-boundary).

## Platform Atlas Texture Lease Contract

`PlatformAtlas` no longer supplies compatibility defaults for renderer texture leasing. Every
platform backend must implement `atlas_texture_lease_epoch`, `acquire_atlas_texture_leases`, and
`release_atlas_texture_leases`. An implementation that previously omitted these methods compiled,
but every retained visual referencing atlas textures failed to obtain a lease and was repainted
without its glyph, icon, emoji, or image sprites.

Use one renderer-local lifetime epoch and one exact instance generation per reusable texture slot:

- Keep `atlas_texture_lease_epoch()` stable for the current renderer and advance it whenever a
  reset invalidates all prior texture instances.
- Treat each `AtlasTextureInstanceId` as the complete slot-plus-generation identity. Reusing the
  same numeric slot must mint a new instance generation.
- Validate the entire deduplicated input set before incrementing any lease count. Return
  `AtlasTextureLeaseError::TextureUnavailable` or `LeaseCountOverflow` without partially retaining
  earlier entries.
- On successful acquisition, return the epoch observed under the same atlas lock. Keep every exact
  instance resident until the matching release consumes that lease.
- Release only the same deduplicated instance set and epoch. A release from an obsolete epoch must
  not mutate the replacement atlas lifetime.

The methods remain an unsafe backend seam because GPUI transfers one exact release obligation into
the non-cloneable visual lease. Backends must not expose another path that can forge, duplicate, or
prematurely consume that obligation. A compile error for an incomplete backend is intentional and
safer than a successful build that renders blank text or images.

`AtlasTile` also changes its public `repr(C)` layout. Update every Rust structure literal and every
host/shader mirror together. The generation identifies the current occupant of a reusable texture
slot; the explicit padding is ABI-owned and must remain zero:

```rust
let tile = AtlasTile {
    texture_id,
    tile_id,
    padding,
    bounds,
    texture_generation: slot_generation,
    texture_generation_padding: 0,
};
```

This is an ABI break, not only a Rust source break. The old 32-byte record is replaced by the
following 40-byte, 4-byte-aligned host layout:

| Field | Byte offset | Byte size | External representation |
|---|---:|---:|---|
| `texture_id.index` | 0 | 4 | unsigned 32-bit slot |
| `texture_id.kind` | 4 | 4 | unsigned 32-bit `AtlasTextureKind` discriminant |
| `tile_id` | 8 | 4 | unsigned 32-bit tile id |
| `padding` | 12 | 4 | unsigned 32-bit content padding |
| `bounds` | 16 | 16 | four signed 32-bit values: origin x/y, width, height |
| `texture_generation` | 32 | 4 | non-zero unsigned 32-bit slot generation |
| `texture_generation_padding` | 36 | 4 | zero |

Recompile every downstream Rust, C, Metal, and WGSL consumer; do not reinterpret persisted or
cached 32-byte records as the new type. External Rust backends should assert `size_of`, `align_of`,
and `offset_of!` for their own `#[repr(C)]` mirror. Shader mirrors must apply their language's
layout rules while preserving the byte offsets and 40-byte array stride above.

Increment `slot_generation` before publishing a replacement occupant under the same
`AtlasTextureId`. Do not derive it from `tile_id`, leave it at a permanent constant, or omit the two
fields from a C, Metal, or other renderer-side mirror. `AtlasTile::texture_instance()` is the
canonical slot-plus-generation identity used by leasing and scene validation.

## Native Point-Hit Stack Contract

`PlatformWindowHit` adds `ProvisionalPassThrough`, so exhaustive matches in platform backends and
native integrations must handle the new variant. The variant is legal only for an exact gated
provisional window and carries its non-zero immutable session generation plus same-observation
coverage and client geometry.

Custom `Platform::window_hit_stack_at` implementations must return entries in native front-to-back
order using this grammar:

```text
ProvisionalPassThrough* (RegisteredApplication | OpaqueBarrier)
```

An empty available observation means the backend positively verified open desktop space. Missing
windows, incomplete enumeration, stale generation identity, a point outside any reported coverage,
or any ordering ambiguity must return `PlatformWindowHitStack::Unavailable` instead. The trait's
default implementation already returns `Unavailable`, so backends that do not support classified
point-hit stacks may keep that fail-closed behavior.

Update consumers as follows:

```rust
match hit {
    PlatformWindowHit::ProvisionalPassThrough { .. } => {
        // Continue only after validating the exact provisional session.
    }
    PlatformWindowHit::RegisteredApplication { .. } => {
        // This is the terminal application target.
    }
    PlatformWindowHit::OpaqueBarrier { .. } => {
        // Stop routing at ordinary, foreign, or unknown native coverage.
    }
}
```

Use `PlatformWindowHitObservation::try_new` or `PlatformWindowHitStack::try_available` rather than
constructing unchecked observations. They reject malformed coverage, zero provisional generations,
non-prefix pass-through entries, and non-empty observations without exactly one terminal entry.

## Deterministic Collection Typeahead

Collection typeahead now uses one crate-private, instance-owned session with GPUI executor time.
Tree, VirtualizedList, Menu, ContextMenu, and standalone Listbox share its printable-input,
composition, modifier, timeout, and repeated-character rules. Select receives the same behavior
only from its mounted popup Listbox; printable input on a closed Select trigger remains inert.
Combobox and Command keep their persistent editor-owned query and filtering paths and do not use
the collection session.

The timeout remains 700ms. Input at exactly the boundary refines the existing prefix; later input
starts a new session. A first character and repeated equal characters scan after the current stable
key and wrap. A different character refines the current prefix while including the current match.
Matching, disabled or structural row filtering, focus, reveal, and selection remain owned by each
component model. Sessions never store row indexes or target keys, so reorder resolves from the
latest stable key and removal falls back safely.

Tree values are stable row identities and must be unique within one resolved tree. Duplicate
values now fail closed: ambiguous rows remain visible but cannot receive focus, selection, or
typeahead targeting. Assign distinct values instead of relying on source order to disambiguate
rows.

`Listbox::typeahead_query`, `ListboxState::typeahead_query`, and the corresponding
`ListboxState::resolve` argument were deleted without aliases. They copied an editable owner's
query into inert Listbox metadata and were not the runtime buffer. Read `ComboboxState::query` or
`CommandState::query` when displaying or inspecting editable search, and call
`ListboxState::typeahead_target` directly when a pure caller-owned query needs model resolution.
There is no public collection-session API.

`Listbox::active`, `Select::active`, `Combobox::active`, and `Command::active` were also deleted.
They behaved like caller-owned values but exposed no active-change intent, so every redraw could
silently overwrite keyboard or typeahead progress. Use `default_active` to seed the adapter-owned
keyed runtime once. Renderer-neutral state requests still accept an `active_value`, and the
editor-owned Combobox adapter projects its current runtime value into its private embedded
Listbox; neither path recreates a public controlled-active API.

## Interactive Subtree Transform And Coordinates

The SVG-only `Transformation`, `TransformationMatrix`, and `Svg::with_transformation` APIs are
deleted without aliases. They changed raster output while leaving descendants, input, IME,
accessibility, and diagnostics at the old geometry. Use the checked layout-neutral subtree
authority for supported axis-aligned scale and translation:

```rust
use open_gpui::{
    SubtreeTransform, SubtreeTransformExt as _, SubtreeTransformOrigin, div, point, px, size,
};

let transform = SubtreeTransform::try_new(
    size(1.25, 0.9),
    point(px(8.0), px(-4.0)),
    SubtreeTransformOrigin::CENTER,
)?;

let element = div().child(content).with_subtree_transform(transform);
```

The contract accepts only finite, strictly positive normal axis scale with a representable reciprocal,
finite logical-pixel translation, and a finite post-layout origin. `TOP_LEFT` and `CENTER` are
built in; `SubtreeTransformOrigin::try_new(anchor, offset)` resolves
`anchor * child_size + offset` after layout. Rotation, skew, arbitrary matrices, reflection, and
3D have no replacement in this API. A numeric or backend conversion failure suppresses the whole
affected subtree transaction; it never clamps or substitutes identity.

Retained geometry or other state published outside the frame journal must use a stable
`PrepaintPublicationId` with `Window::record_prepaint_window_transaction`. GPUI commits it only
after the valid candidate has become the accepted rendered frame and invokes its discard callback
when that accepted frame contains an invalid publication or omits the previous one. A candidate
rejected before the frame swap preserves the previous publication and runs neither callback. Reuse
the ID across renders of one logical producer; do not publish directly from render/prepaint or
preserve last-known state after an accepted unmount or rollback.

The transaction callbacks now receive an `AcceptedFrameFence` instead of a bare frame revision.
Use `fence.generation()` when diagnostics need the accepted generation, and
`fence.is_satisfied_by(window)` when an internal lifecycle must distinguish work already represented
by the rendered frame from ordinary event-driven work that must wait for a future accepted frame.
The fence is window-bound and cannot be constructed by applications. A rejected candidate produces
no fence, so consumers must not synthesize an equivalent from `Window::rendered_frame_revision()`.

For a top-left-relative pixel origin, use the checked
`SubtreeTransformOrigin::try_pixels(offset)` constructor. The origin type has no unchecked pixel
conversion or `From<Point<Pixels>>` implementation; invalid input must be handled at the call site.

Raw platform events remain window-space. Interactive listeners now receive `TargetedEvent<E>`
where target geometry is required. Read the original event through `window_event()` and use the
checked target-local or target-layout helpers for control logic:

```rust
div().on_mouse_move(|event, _window, _cx| {
    let raw_window_position = event.window_event().position;
    let Ok(local_position) = event.target_local_position() else {
        return;
    };
    update_selection(local_position, raw_window_position);
})
```

`on_click` and `on_aux_click` receive `TargetedEvent<ClickEvent>`. Drag construction receives
`DragStartGeometry`; `on_drag_move` receives `DragMoveEvent<T>` whose `drag()` reads the captured
payload; and `on_drop` receives `DropEvent<T>` with `value()` plus a targeted `pointer()`. Use
`window_preview_offset()` only for the window-space drag preview. Pixel wheel deltas can be read in
target-local units; line deltas preserve their semantic unit.

For committed geometry, use `Hitbox::geometry()` or wrap content with `measured_element`. Its
listener is now `Fn(MeasuredElementSnapshot, &mut App)`: the snapshot carries frame generation,
semantic/global identity, and one immutable `ElementGeometry` with layout bounds, displayed
bounds, and checked local/layout/window conversions. Failed transform scopes publish no snapshot.
Custom prepaint channels that already receive layout bounds may call `Window::try_element_geometry`
and publish only its opaque result. Do not compare raw layout bounds with window-space input.

Complex custom consumers must keep visual stacking and pointer ownership in the same committed
coordinate model. Docking now resolves divider junctions separately for the root and each floating
surface and gives every floating container a blocking boundary across its complete bounds. Raw
splitter and composite-floating drags retain the Dock host capture owner. Standard GPUI payload
drags use the source element's stable owner and acquire it only after crossing the drag threshold,
so `on_drag` sources must call `.id(...)` first and removal of that binding produces terminal
cancellation without capturing ordinary clicks. A frame-scoped listener inside each rendered
`DockHost` receives terminal cancellation from the old committed frame and clears only that host's
matching runtime session, preview, anchor, and captured native route. It does not leave a
window-global observer after the host is suppressed or removed.

Delete integrations with `host_outside_release`, `Platform::mouse_button_is_pressed`, and any 16 ms
release polling task. A native Dock payload drag now prepares one exact-generation consumer from
`DragStartGeometry`; GPUI activates it with the source drag and delivers immutable physical move,
release, or cancellation facts after application and window borrows are released. Cross-window
routing requires a point-scoped `window_hit_stack` observation with coherent target geometry. A
target window does not need to receive raw pointer input, and its local listener is not
cross-window release authority. Missing capability, an opaque barrier, a foreign Dock surface,
stale scene or session generations, and malformed physical facts fail closed and clear or reject
the current route instead of reusing a prior preview. The source `MouseUp` locks the route's point,
observation, generation, and ingress sequence; later capture-change cleanup cannot replace that
terminal result. Establish component payload state only after policy and checked geometry accept
the underlying drag session, and defer rollback when the framework writes its active drag after the
constructor returns. Do not flatten transformed overlays into one hit list, treat only dividers as
occluding, or leave component drag state dependent on target-window mouse delivery.

Motion remains below GPUI. Sample `MotionProjection::try_transform_sample` and convert it in a
consumer that depends on both crates, such as
`open_gpui_ui_components::gpui_adapter::subtree_transform_from_motion_projection`. Do not add a
GPUI type or identity fallback to `open-gpui-motion`.

See [ADR 0021](../adr/0021-open-gpui-interactive-subtree-transform-authority.md) for composition,
cache/deferred, renderer, and failure-ordering details.

## Exact Subtree Clips

`ContentMask`, `Window::with_content_mask`, and element-local descendant clip flags are deleted
without aliases. They could reduce rounded clips to rectangles or let paint and input use different
geometry. Use the checked public subtree authority instead:

```rust
use open_gpui::{
    Corners, SubtreeClip, SubtreeClipExt as _, div, px, size,
};

let radius = size(px(12.0), px(12.0));
let clip = SubtreeClip::try_own_rounded_border_box(Corners {
    top_left: radius,
    top_right: radius,
    bottom_right: radius,
    bottom_left: radius,
})?;

let element = div().child(content).with_subtree_clip(clip);
```

`SubtreeClip::own_border_box()` and `.clip_to_border_box()` are the rectangular equivalents. For
an explicit child-local region, use `SubtreeClip::try_rect` or
`SubtreeClip::try_rounded_rect`; bounds are zero-origin logical pixels relative to the child's
post-layout border box, not window coordinates. Radii must be finite and non-negative. GPUI
normalizes elliptical radii before any subtree transform, then preserves those ellipses under
non-uniform scale.

The wrapper changes no layout, measurement, sibling flow, or scroll extent. It applies one exact
nested clip stack to paint and initial pointer hit testing, including hover/click/wheel/drag/drop,
deferred/cache replay, focus/IME, and Inspector. Public `SubtreeClip` also excludes fully clipped
accessibility descendants. Do not model a rounded descendant clip with a background radius or an
AABB test. Style overflow enters the same visual/pointer stack; one-axis overflow preserves the
other axis and two-axis overflow uses the padding-box ellipse. `Overflow::Hidden` and
`Overflow::Clip` also exclude semantics, whereas `Overflow::Scroll` deliberately keeps off-viewport
nodes available for AccessKit `ScrollIntoView`.

`Hitbox::hit_test_snapshot` is the frame-committed capability for integrations that need to carry
exact target eligibility outside the rendering callback. Its public queries are intentionally
read-only. Do not retain a raw clip stack, reconstruct window-space geometry, or use the published
accessibility AABB as rounded coverage. Public subtree clips and non-scrolling overflow remove fully
semantic-clipped nodes; root and scroll viewports do not remove off-viewport targets. A published
non-empty node uses a conservative AABB, while built-in fallback click separately needs an internal
point proven inside the complete visual/pointer stack. A zero-area semantic node remains only when
its anchor is inside the semantic stack, and it receives neither a pointer witness nor a fallback
click.

Adapters that register a logical region without a target hitbox, such as an Overlay inside region,
must capture `Window::hit_test_snapshot(layout_bounds)` during prepaint. Do not replace that
snapshot with its displayed AABB.

Canvas custom prepaint code must pass the Canvas element's post-layout border box to
`prepare_canvas_frame`; the same bounds must be passed to `paint_canvas_frame` later in that frame:

```rust
// Before
let prepared = prepare_canvas_frame(frame, theme, window);

// After
let prepared = prepare_canvas_frame(frame, canvas_bounds, theme, window);
```

Ordinary deferred and cached descendants inherit clipping. Named window-space portals intentionally
reset it and start their AccessKit subtree at the window root. An invalid transform, clip, or
backend conversion suppresses the affected subtree across paint, input, focus/IME, debug,
deferred/cache, and accessibility; there is no rectangle fallback.
Arbitrary path clips, fill rules, stencil/tessellation controls, and silent native-surface fallbacks
remain unsupported. See [ADR 0026](../adr/0026-open-gpui-rounded-subtree-clip-authority.md) for the
renderer and failure contract.

## Layout-Preserving Subtree Presentation

`Style::visibility`, the generated Serde/schema `StyleRefinement::visibility` field,
`Visibility::{Visible, Hidden}`, `visibility_style_methods!` and its generated `.visible()` /
`.invisible()` (`fn visible` / `fn invisible`) methods, `Element::a11y_hidden`, and `aria_hidden`
are deleted without aliases. They independently suppressed paint or accessibility and could leave
a hidden subtree interactive or focused. Replace ancestor-level hiding with one three-state
authority:

```rust
use open_gpui::{
    ParentElement as _, SubtreePresentation, SubtreePresentationExt as _, div,
};

let element = div()
    .child("Content")
    .with_subtree_presentation(SubtreePresentation::Inert);
```

The exact contract is:

| State | Layout | Paint | Input | Focus / IME | Accessibility |
| --- | --- | --- | --- | --- | --- |
| `Visible` | yes | yes | yes | yes | yes |
| `Inert` | yes | yes | no | no | no |
| `Hidden` | yes | no | no | no | no |

All three states retain measurement, flex/grid ordering, sibling placement, and scroll extent.
Use `SubtreePresentation::Inert` when the subtree must remain painted but must leave every input,
focus, IME, and accessibility route; use `SubtreePresentation::Hidden` when paint must also stop.
Use `Display::None` when layout participation must be removed. Keep component `disabled` state for
controls that remain discoverable with disabled semantics; an inert subtree is absent from the
final AccessKit tree and cannot be activated by pointer, keyboard, AccessKit, or an
`ActivationHandle`.

Nested declarations choose the most suppressive ancestor state, so a descendant cannot restore
itself to `Visible`. Ordinary deferred elements, cached views, transforms, and coordinate portals
inherit that state. Independently owned window overlays start at an explicit
`WindowOverlayRuntime` root and still inherit suppression from their overlay parent.

Dynamic `Visible` to `Inert` or `Hidden` transitions revoke hover/cursor state, pointer capture,
drag/drop, focus, IME, tooltip and overlay intent, inspector targets, and AccessKit membership at
the committed-frame boundary. The old pointer owner receives one terminal cancel. Restoring
`Visible` does not replay pressed keys, pointer releases, focus claims, pending scroll callbacks,
tooltips, or activation arms; require fresh input.

Code that must settle one programmatic focus restoration or clearance should use
`Window::focus_with_completion`, `Context::focus_with_completion`,
`Window::blur_with_completion`, or `Context::blur_with_completion` and handle
`FocusClaimOutcome::{Committed, Rejected, Superseded}`. `Window::blur` and
`Window::disable_focus` now require `cx`; migrate `window.blur()` to `window.blur(cx)` and
`window.disable_focus()` to `window.disable_focus(cx)`. Use `on_focus_committed` to observe one
handle becoming the exact committed local focus, or `on_focus_committed_in` to observe committed
focus entering a handle or descendant. These committed observations work while the platform window
is inactive; keep `on_focus_in` for effective active-window focus entry. Do not infer focus success
from calling `focus`, `blur`, an `on_next_frame` callback, or platform activation: a late
inert/hidden candidate can still be rejected, and platform activation may expose an already
retained local focus without creating a new committed-focus event. Sealed requests receive one
later platform candidate generation; they do not recursively redraw inside the current effect
cycle. `Window::focused` exposes current intent and may be provisional during a candidate render;
use `Window::committed_focus` when a synchronous read specifically needs the last committed local
leaf.

Decorative leaf elements that intentionally emit no accessibility node use
`omit_accessibility_node(true)`. That method omits only the current leaf projection and must not be
used as an ancestor presentation switch. Renderer-neutral component adapters now express that
same narrow operation as
`SemanticDescriptor::with_omit_accessibility_node(true)`. The ambiguous `with_hidden` builder and
`hidden` getter are deleted without aliases because their names incorrectly implied descendant
suppression. Strict DevTools payload consumers must likewise rename the semantic state key from
`hidden` to `omit_accessibility_node`.

See [ADR 0022](../adr/0022-open-gpui-subtree-presentation-authority.md) for frame ordering,
low-level registration gates, deferred/cache behavior, and overlay-root ownership.

## Live Regions And Window Announcements

Accessibility status updates now use the renderer-neutral live-region contract:

```rust
use open_gpui_ui_core::{LivePoliteness, Role, SemanticDescriptor};

let status = SemanticDescriptor::new(Role::Status)
    .with_live_text("Indexing complete")
    .with_live(LivePoliteness::Polite)
    .with_live_atomic(true)
    .with_busy(false);
```

`Role::Status` defaults to polite and atomic; `Role::Alert` defaults to assertive and atomic.
`with_live_text` writes the same value to the descriptor label and value for cross-platform
AccessKit adapter behavior. `LivePoliteness::Off` is explicit and remains present in the final
tree, which is useful for illustrative catalog examples. `StatusCue::live`,
`StatusCue::live_atomic`, and `StatusCue::busy` expose the same policy without creating a second
semantic assembly path. `EmptyState` remains a structural `Section` and is not a live region; use
`StatusCue` or an explicit window announcement when an empty/error event should be spoken.
Components must not call the window announcement queue from render, mount, remount, or timeout code.

For a notification that is intentionally independent of an element lifecycle, use the window-owned
queue:

```rust
use open_gpui::AccessibilityAnnouncement;

let outcome = window.announce(AccessibilityAnnouncement::polite("Workspace saved"), cx);
```

The queue accepts at most 32 pending or one-generation-retained requests per window. It preserves
order and gives repeated equal text a new sequence and node identity. At call time, `Accepted` means
only that the request entered this queue. If its accessibility activation generation remains
current, the node is committed in the final AccessKit tree, kept until one later matching generation
commits its removal, and never moves focus or calls native speech. Deactivation, activation
replacement, or window close can instead clear an accepted request before publication; the
metadata-only diagnostic records that typed `Cleared` lifecycle and the request never replays.
Requests submitted while accessibility is inactive or the window is closing, or rejected at
capacity, return a typed `Dropped` outcome. GPUI guarantees that publication uses a committed tree
update, not that every accepted request reaches publication or that a screen reader speaks it.
Diagnostics never expose announcement text.

The following public migrations are part of this contract:

- `CommandStatusItem::new`, `info`, `warning`, and `error` now require a stable caller-provided id;
  status node identity must not be derived from an array index. Duplicate or empty identities are
  omitted from resolved `CommandState` with metadata-only diagnostics.
- `ResourceCollectionProjection::resolve` and `ResourceMutationProjection::resolve` now require a
  caller-owned `ResourceAdapterNamespace`; resource and mutation status IDs derive only from that
  diagnostic-safe namespace and remain stable across lifecycle transitions. Query keys and
  mutation IDs must never be used as status identities.
- `StatusCue` ordinary intents resolve to `Status`, danger resolves to `Alert`.
- Field validation errors resolve to an assertive atomic `Alert`; help text remains a `Label`.
- VirtualizedList empty/exhausted rows resolve to `Status`, and inline error/retry rows resolve to
  `Alert` rather than `AlertDialog`.

See [ADR 0023](../adr/0023-open-gpui-live-region-announcement-authority.md) for lifecycle,
privacy, and final-tree verification details.

## Semantic Activation Authority

Official controls normalize pointer, allowed key-up, AccessKit Click, and programmatic requests
through one activation transaction. `Activation` carries the typed source, while domain payloads
carry only the item or value facts required by the callback. Use an `ActivationHandle` for
application-driven activation instead of synthesizing pointer coordinates or key events.

### Listbox Selection And Activation

`Listbox::selected` now accepts `Option<String>` and is always caller-owned. Use
`Listbox::default_selected` when the Listbox adapter should own subsequent selection commits. Bind
programmatic selection by stable option value with `Listbox::activation_handle`:

```rust
use open_gpui_ui_components::{ActivationHandle, Listbox};
use open_gpui_ui_components::listbox::ListboxOption;

let beta = ActivationHandle::new();
let listbox = Listbox::new("frameworks", "Frameworks")
    .default_selected("alpha")
    .option(ListboxOption::new("alpha", "Alpha"))
    .option(ListboxOption::new("beta", "Beta"))
    .activation_handle("beta", &beta)
    .on_select(|selection, window, cx| {
        record_selection(selection.value(), window, cx);
    });
```

An option-level `ListboxOption::on_select` replaces the Listbox-level public fallback; the two are
not both delivered. When the option is rendered inside Select or Combobox, the owning adapter still
commits its controlled/uncontrolled transaction, input projection, and overlay close before it
delivers the chosen item or family callback. Disabled, structural, and duplicate-value rows reject
pointer, keyboard, AccessKit, and programmatic activation. Separators no longer use reserved String
sentinels, so every caller-provided option value remains legal.

### Select, Combobox, And Command Selection Ownership

`Select::selected`, `Combobox::selected`, and `Command::selected` now match Listbox: they accept
`Option<String>` and always represent the caller-owned render-frame value. Use each component's
`default_selected` builder when its adapter should own later single-selection commits. Command
multi-selection keeps `selected_values(...)` as its caller-owned input and adds
`default_selected_values(...)` for adapter-owned state.

```rust
let controlled = Select::new("framework", "Framework")
    .selected(selected_framework.clone())
    .option(ListboxOption::new("alpha", "Alpha"));

let uncontrolled = Command::new("tools", "Tools")
    .default_selected("search")
    .item(CommandItem::new("search", "Search"));
```

A controlled selection callback emits one intent without changing hidden adapter state. If the
owner refuses it, selection and Combobox input text remain unchanged; the new value or label is
projected only after a later caller prop commit. Selection callbacks may synchronously redraw.
The matching overlay close observer is still delivered exactly once after an ordinary rebind,
while a genuinely superseding request, owner commit, ownership change, or unregister invalidates
the older dispatch.
Command multi-select callbacks toggle the raw caller/runtime value set. Values that are currently
missing, disabled, or filtered out stay in the emitted set even though they do not project as chips
or selected options. Toggling an existing value removes every duplicate occurrence of that value,
so the emitted collection cannot report deselection while retaining the same logical member.

### Toolbar Activation

`ToolbarSelection` and both `Toolbar::on_select` and `ToolbarItem::on_select` are deleted. Replace
them with `ToolbarActivation` and `on_activate`, whose callback also receives `Activation`:

```rust
use open_gpui_ui_components::{ActivationHandle, Toolbar};
use open_gpui_ui_components::toolbar::ToolbarItem;

let save = ActivationHandle::new();
let toolbar = Toolbar::new("editor-toolbar", "Editor actions")
    .item(ToolbarItem::action("save", "Save"))
    .activation_handle("save", &save)
    .on_activate(|item, input, window, cx| {
        dispatch_toolbar_action(item.value(), input.source(), window, cx);
    });
```

Action items activate on unmodified Enter or Space key-up. Toggle items activate on Space key-up
only, and their `pressed()` payload is the caller-owned state from before activation; Toolbar does
not mutate it. Disabled items share the same gate for every source. An item-level `on_activate`
handler overrides the toolbar-level fallback, so registering both does not execute an action twice.
Roving Arrow/Home/End navigation remains a separate focus transaction. Item values must be unique
within one Toolbar. Duplicate values remain visible but fail closed as disabled and reject every
activation source rather than letting render order choose a programmatic target.
Diagnostic selectors for duplicate occurrences end in `:snapshot:<opaque-token>`. Treat them as
current-snapshot probes: do not construct or persist the token, and reacquire the selector after an
authored item reorder.
Custom view tooltip closures are not mounted for duplicate Toolbar values because closure contents
cannot provide stable authored identity. Use a text tooltip or unique item values instead.

### Sidebar Activation

`SidebarSelection`, `Sidebar::on_selection_change`, and `SidebarItem::on_select` are deleted.
Replace them with `SidebarActivation` and `on_activate`; bind application-driven activation by
stable item value:

```rust
use open_gpui_ui_components::{ActivationHandle, Sidebar, SidebarSection};
use open_gpui_ui_components::sidebar::SidebarItem;

let settings = ActivationHandle::new();
let sidebar = Sidebar::new("app-sidebar", "Application navigation")
    .section(
        SidebarSection::new("account", "Account")
            .item(SidebarItem::new("settings", "Settings")),
    )
    .activation_handle("settings", &settings)
    .on_activate(|item, input, window, cx| {
        navigate(item.value(), input.source(), window, cx);
    });
```

Pointer, Enter/Space key-up, AccessKit Click, and programmatic requests now share one transaction.
The callback observes caller-owned `selected()` state from before activation. Item-level handlers
override the Sidebar fallback. Item values must be globally unique across sections; duplicates stay
visible but are disabled, non-focusable, and blocked for programmatic activation. Offcanvas and
disabled Sidebars bind handles as blocked rather than dispatching or selecting an arbitrary item.
Duplicate section and item selectors likewise carry an opaque current-snapshot suffix so a stale
selector, AccessKit node id, or activation state key cannot silently retarget after reorder.

## Federated Component Contract Authority

The component product table now owns only official id, revision, family, and required scenario ids.
The former API inventory, source mapping, public-surface owner map, Gallery/docs status rows,
accessibility evidence rows, and central conformance gates are deleted without compatibility
aliases. Use `ComponentContractEntry` / `ComponentContractMetadata` for product identity and use the
natural owner for every downstream fact:

- public exports are declared together with typed `PublicApiExport` facts;
- Components and Overlay Gallery rows own presentation status, selectors, and Story probes;
- native integration targets own exact coordinates in sibling `*.scenarios.toml` artifacts;
- DevTools semantic payloads carry `contract_id`, `contract_revision`, and `family` from canonical
  metadata;
- `scan-ui-contract` joins and executes these owners but is not another registry.

`ComponentContractEntry` is now immutable canonical metadata rather than a public struct-literal
inventory row. Replace `entry.name` with `entry.id().as_str()`, `entry.family: Option<_>` with
`entry.family().as_str()`, and read the new `entry.revision()` and
`entry.required_scenarios()` projections. Owner, Gallery/docs status, export, and source fields have
no replacement on this row; query their natural owners instead.

Delete imports of `CallbackApi`, `DefaultSeedApi`, `ComponentApiInventoryEntry`,
`ComponentA11yEvidence`, `ComponentConformanceGate`, `PublicSurfaceOwnerClass`,
`PublicSurfaceOwnerEntry`, `SurfaceGalleryStatus`, and `SurfaceDocsStatus`. Also delete uses of
`COMPONENT_API_INVENTORY`, `COMPONENT_A11Y_EVIDENCE`, `COMPONENT_CONFORMANCE_GATES`,
`PUBLIC_SURFACE_OWNER_MAP`, `component_public_methods`, `component_render_inputs`,
`public_owner_for_component_inventory`, `component_recipe_component_rows`,
`default_surface_rows`, `gallery_surface_rows`, `official_component_rows`,
`official_overlay_component_rows`, and `component_source_inputs`. These APIs have no aliases or
method/source manifest replacement; inspect the owning Rust API and use executable tests.

## Accessibility Semantic Projection

Official semantic producers now derive an ephemeral `SemanticDescriptor` from their resolved
render state. The descriptor is projected into GPUI with `UiA11yElementExt::ui_semantics`; it is
not stored as a second component state model. Final `TreeUpdate` assertions and real AccessKit
action dispatch are the executable authority. Static `COMPONENT_A11Y_EVIDENCE` rows, Gallery
`COMPONENT_A11Y_CLAIMS`, and their consumers were deleted rather than retained as transitional or
substitute runtime evidence.

See
[Semantic accessibility and final-tree authority](../knowledge/engineering/decisions/semantic-accessibility-final-tree-authority.md)
for the projection, lifecycle, executable-evidence, and DevTools redaction decision.

IconButton keeps its accessible name separate from its description. Slider and NumberInput expose
`SetValue` only when a change callback exists, consume only numeric AccessKit payloads, and reject
non-finite values. Disabled controls reject focus and mutation actions; read-only NumberInput keeps
focus but rejects value changes. Indeterminate Progress omits the current numeric value while
retaining its supported range.

`NumberInputStepAction` now includes `SetValue`, so exhaustive downstream matches must handle the
new variant. This breaking addition preserves the real source of a `NumberInputChange`; mapping an
explicit accessibility value request to Increment or Decrement would lose behaviorally relevant
information.

TextInput and Textarea now publish their value, required, invalid, busy, read-only, disabled, and
available text actions from an ephemeral descriptor. Textarea uses the multiline text-input role;
password inputs use the password role and expose only a masked value in the final tree. AccessKit
`SetValue` and `ReplaceSelectedText` enter the same controlled text-editing path as platform input.
Plain TextInput publishes one stable `(control id, "text-run")` child. Textarea publishes one stable
TextRun per logical line: its first run uses `(control id, "text-run")`, later runs use stable
`text-run:{line index}` identities, a hard line break belongs to the preceding run, and a trailing
line break creates an empty final run. Directional selection may span Textarea runs. AccessKit
character indices follow Unicode grapheme boundaries, including combining sequences, emoji ZWJ
sequences, and normalized LF line breaks. Read-only controls remain focusable and selectable but
reject value mutation; disabled controls reject both selection and mutation. Password inputs
intentionally publish neither a TextRun child nor text-selection actions, so masked content cannot
be recovered through fine-grained accessibility metadata. If any single grapheme exceeds
AccessKit's `u8` character-length limit, the control omits TextRun and selection metadata and
retains only whole-value accessibility actions.

`Label::for_control` and `LabelState` control-association metadata have been removed because they
never created a real accessibility relation. Compose labels and support text through `Field`, whose
typed control adapter resolves `labelled_by`, `described_by`, and validation error relations from
the actual GPUI element path. Standalone `Label` remains a visible-text semantic primitive.
`Field::new(id, control_id, label)` is replaced by `Field::new(id, label)`, and
`FieldState::control_id()` is removed; the composed control continues to own its own element id.
Custom field controls implement `open_gpui_ui_components::gpui_adapter::FieldControl` rather than
depending on the renderer-neutral prelude.

### Table Public Paths

Table restoration inputs remain common application APIs, while diagnostic readouts now require
their owner modules:

| Removed v0.2 import | v0.3 import |
| --- | --- |
| `open_gpui_ui_components::TableBehaviorSnapshot` and companion behavior snapshot types | `open_gpui_ui_components::table::TableBehaviorSnapshot` and the named companion types in `open_gpui_ui_components::table` |
| `open_gpui_ui_components::common::TableBehaviorSnapshot` and companions | Import the explicit `open_gpui_ui_components::table` types; they are not common-facade APIs |
| `open_gpui_ui_components::prelude::TableBehaviorSnapshot` and companions | Import the explicit `open_gpui_ui_components::table` types; they are not prelude APIs |
| `open_gpui_ui_core::TableStateCacheKey` or `open_gpui_ui_core::table::prelude::TableStateCacheKey` | `open_gpui_ui_core::table::TableStateCacheKey` |

`Table` and `TableVirtualizerSnapshot` remain available from the component root/common prelude.
`TableState`, row-model inputs, and the Table engine remain under `open_gpui_ui_core`; no behavior
contract was deleted merely to narrow an import path.

`TABLE_ROW_MODEL_PIPELINE`, `TABLE_ROW_MODEL_V0_PIPELINE`, and
`TableRowModelStage::implemented_in_v0` were deleted without aliases because they only restated a
version label. Use `TableResolvedState::{core_model, filtered_model, grouped_model, sorted_model,
expanded_model, paginated_model, final_model}` when code needs executable stage output, and use
`TableRowModelStage::as_str()` only for a stable label.

### Table Logical Identity

Table row pinning, expansion, rendering, editing, callbacks, virtualization, and accessibility now
share `TableRowIdentity` as the authoritative logical-row identity. Source rows with duplicate
business `TableRowId` values resolve to distinct source-instance identities, and synthetic group
rows use a separate typed namespace. Pinning and virtual-window movement no longer add region or
slot identity, so the same logical row and cell keep their final AccessKit node ids when they move.

`TableRowPinning::pinned_top` and `pinned_bottom` no longer accept strings or `TableRowId` values.
Pass `TableRowIdentity` for one exact source instance or group row. Use
`TableRowPinTarget::all_source_rows(row_id)` only when pinning every resolved source instance with
the same business id is intentional. Targets retain caller order; bulk matches retain current
model order; top wins logical overlap after target expansion. This is a clean public-contract
replacement with no compatibility alias for the old business-id-only pin state.

`Table::virtualizer_snapshot` now accepts `TableVirtualizerSnapshot` instead of the generic
`VirtualizerSnapshot`. Build retained measurements with `TableVirtualizerSnapshotItem` and a
`TableRowIdentity`; string keys are no longer accepted because they bypass duplicate-source and
group-row identity rules. The Table adapter owns the private conversion to generic virtualizer
keys, so application code should not encode row identities itself.

`TableRowId` remains the application business key, but it is no longer an exact target. Choose the
source identity form deliberately:

```rust
use open_gpui_ui_core::{
    TableRow, TableRowIdentity, TableRowPinTarget, TableSourceRowIdentity, TableState,
};

let state = TableState::new([
    TableRow::new("unique-row"),
    TableRow::new("duplicate"),
    TableRow::new("duplicate"),
    TableRow::new("duplicate").with_instance_id("retained-instance"),
]);
let unique = TableSourceRowIdentity::unique("unique-row");
let occurrence = state
    .source_row_identity_at("duplicate", 1)
    .expect("the current source snapshot contains a second occurrence");
let retained = TableSourceRowIdentity::explicit("duplicate", "retained-instance");
let selected_state = state
    .clone()
    .with_selected_rows([unique.clone(), retained.clone()]);
let exact_pin = TableRowPinTarget::exact(TableRowIdentity::source_instance(
    "duplicate",
    "retained-instance",
));
let bulk_pin = TableRowPinTarget::all_source_rows("duplicate");
```

`TableSourceRowIdentity::unique` means that the business id must be unique in the current source
snapshot; lookup returns `TableSourceRowLookup::Ambiguous` when it is not. An occurrence returned
by `source_row_identity_at` is valid through `TableState` clones and row-model transforms, but any
`with_rows` replacement or reorder creates a different snapshot and lookup returns
`TableSourceRowLookup::StaleSnapshot`. Use `TableRow::with_instance_id` and
`TableSourceRowIdentity::explicit` for identity retained across source replacement or reorder.
Row selection now follows the same rule: `TableState::with_selected_rows` accepts exact
`TableSourceRowIdentity` values, `selected_rows()` returns that exact set, and
`TableRowSelectionChange::current_selection()` returns caller-owned explicit selection roots in
source-model order. Derived descendants are not promoted into explicit state. With
`TableSubRowSelectionPolicy::Descendants`, canceling a selected parent removes its explicit subtree;
canceling an inherited selected descendant removes the explicit ancestor that covers it, so
committing the callback payload makes that descendant unselected. Duplicate business ids no longer
select or deselect every occurrence together. The unimplemented
`TableSelectionScope` surface has been removed; any future business-id bulk selection must use a
separately named target rather than an implicit conversion. `Table::on_row_expansion_request` is
also strictly controlled: a pointer or keyboard request does not render an expanded branch until
the caller commits a changed `TableState`.

Cell edits now target the exact `(TableRowIdentity, TableColumnId)` pair. Construct an
application-owned edit with `TableCellEditRequest::new(TableSourceRowIdentity, ...)`; the
`TableCellEditChange` emitted by `Table::on_cell_edit_change` is reserved for renderer-resolved
callbacks and therefore always carries real `TableRowAction` metadata. `source_row_id()` is a
business-id readout, not target authority. Applying a unique-assumption request to duplicate rows returns
`TableCellEditApplyOutcome::AmbiguousRowId`, and applying an occurrence edit to a newer source
snapshot returns `StaleRowIdentity`. Both outcomes leave the current rows and Table cache identity
unchanged; an exact unique, explicit-instance, or current-snapshot occurrence edit updates only the
intended source row.

The identity-sensitive accessor migration is explicit. A `source_row_id()` result is a business-key
readout and must not be passed back as though it uniquely identified the resolved row:

| Removed or changed v0.2 surface | v0.3 replacement |
| --- | --- |
| `TableResolvedRow::id()` | `identity()` for exact lookup and targeting; optional `source_row_id()` for source-backed display/diagnostics |
| `TableResolvedRow::parent_id()` | `parent_identity()` |
| `TableGroupRow::parent_id()` | `parent_identity()` |
| `TableGroupRow::first_leaf_row_id()` | `first_leaf_identity()` |
| `TableTreeRow::parent_id()` | `parent_identity()` |
| `TableRowModel::rows_by_id()` | No business-id map replacement. Use `rows()` for materialized model order, `lookup_rows()` for every addressable row, or `row(&TableRowIdentity)` for one exact row. |
| `TableRowModel::row(&TableRowId)` | `row(&TableRowIdentity)` for an exact row; `source_rows(&TableRowId)` for every matching source instance; `unique_source_row(&TableRowId)` only when exactly one match is acceptable |
| `TableResolvedState::duplicate_row_ids()` | `row_identity_diagnostics()`, including typed `DuplicateRowId` and `DuplicateSourceInstance` diagnostics |
| `TableRowAction::row_id()` | `identity()` for exact targeting; `source_row_id()` only for display/diagnostics |
| `TableRowActivation::row_id()` | `identity()`; optional `source_row_id()` readout |
| `TableRowExpansionToggle::row_id()` | `identity()`; optional `source_row_id()` readout |
| `TableRowSelectionChange::row_id()` | `identity()`; optional `source_row_id()` readout |
| `TableState::with_selected_rows(TableRowId...)` / `selected_rows() -> BTreeSet<TableRowId>` | Pass exact `TableSourceRowIdentity` values; `selected_rows()` returns `BTreeSet<TableSourceRowIdentity>`. |
| `TableRowSelectionChange::current_selection() -> &[TableRowId]` | `current_selection() -> &[TableSourceRowIdentity]` as caller-owned explicit roots in source-model order. |
| `TableSelectionScope` | Removed. Only exact row selection exists; add an explicitly named bulk target if a real consumer requires one. |
| `TableExpansionState::rows(TableRowId...)` / `is_expanded(&TableRowId)` | `rows(TableRowIdentity...)` / `is_expanded(&TableRowIdentity)` |
| `TableCellEditChange::for_row(...)` / `for_source_identity(...)` | `TableCellEditRequest::new(TableSourceRowIdentity, ...)` for programmatic edits; runtime callbacks receive `TableCellEditChange` |
| `TableCellEditChange::row_id()` | `identity()` for the exact callback row; optional `source_row_id()` readout |
| `TableBehaviorSnapshot::row(&TableRowId)` | `row(&TableRowIdentity)` for one exact rendered row; `source_rows(&TableRowId)` or `unique_source_row(&TableRowId)` for rendered business-id lookup |
| `TableRowBehaviorSnapshot::id()` | `identity()` for the exact rendered row; optional `source_row_id()` readout |
| `TableResolvedHeaderCell::id()` | `identity()` for the typed resolved fragment; `logical_identity()` when pinning-region fragmentation must be ignored |
| `TableResolvedHeaderCell::source_id()` | `source_column_id()` for a leaf, `source_group_path()` for a group, or inspect `logical_identity()` when handling every header kind |
| `TableResolvedHeaderCell::placeholder_id()` | No string-id replacement. Match `TableHeaderIdentity::Placeholder` through `logical_identity()`. |
| `TableResolvedHeaderCell::sub_header_ids()` | `sub_header_identities()` |
| `TableResolvedHeaderGroup::id()` | `identity()` returning `TableHeaderRowIdentity`; read `region()` separately because row identity is region-independent |
| `TableRowPinning::pinned_top(TableRowId...)` / `pinned_bottom(TableRowId...)` | Pass exact `TableRowIdentity` targets, or explicit `TableRowPinTarget::all_source_rows(TableRowId)` bulk targets. |
| `TableRowPinning::top()` / `bottom()` | `top_targets()` / `bottom_targets()` returning ordered `TableRowPinTarget` slices |
| `TableRowAction::render_key()` / `TableCellEditChange::render_key()` | No like-for-like string accessor. Retain `identity()` as authority, use `identity().key()` only when a typed `TableRowIdentityKey` is required, and use `TableDebugSelector` builders for official selectors. |
| `TableDebugSelector::select_editor_option(...)` | Removed without replacement. Table owns the cell-editor identity, while the nested Listbox owns rendered option selectors. Render the editor and query the unique Listbox option within that editor owner instead of synthesizing a cross-component selector. |
| `Table::default_focused_row(TableRowId)` | `Table::default_focused_row(TableRowIdentity)` |
| generic `VirtualizerSnapshot` string keys | `TableVirtualizerSnapshotItem::new(TableRowIdentity, size)` |
| `TableColumnOrderChange::apply_to_order(column_order)` | `apply_to(TableState) -> TableState`; the state supplies the full source-column authority used to normalize partial order before the move |

Column order no longer owns visibility. A partial `TableState::column_order()` puts its known ids
first, and `normalized_column_order()` appends every unlisted source column in source order while
ignoring unknown and duplicate ids. `TableColumnOrderChange::apply_to(TableState)` normalizes that
complete source order before moving either a listed or previously unlisted column, then stores the
full order without changing visibility or pinning.

Virtual Table focus remains logical when a row leaves the rendered overscan window. The stable
Table root becomes the physical and AccessKit focus proxy, publishes no stale row actions, and
continues real Up, Down, Home, End, Enter, and Space behavior against the complete final model. A
row remount reclaims physical focus only while that proxy still owns the claim. If the exact row
leaves the final model, focus falls back to its first remaining row or clears for an empty model;
focus already moved outside the Table is never stolen.

### VirtualizedList Render Identity

`VirtualizedListRowBehaviorSnapshot::render_key()` and
`VirtualizedListRowRenderContext::render_key()` are opaque adapter identities, not domain keys.
Use `VirtualizedListItemDescriptor::key()` for application identity. Duplicate source keys now use
a collision-checked, length-prefixed occurrence encoding so they cannot alias any legal unique
source key. Do not parse, format, or persist the encoding; only round-trip a render key to the
adapter-owned measurement path for the same ordered descriptor snapshot. Reordering duplicate
items creates a new occurrence authority.

### DevTools Semantic Probes

The old `a11y_evidence_probe_snapshot` and `a11y_contracts_probe_snapshot` entry points are deleted
without compatibility aliases. They projected contract evidence and claim rows rather than the
resolved semantics used by the renderer.

Construct the replacement probe from canonical component metadata, an app-assigned opaque identity,
and the ephemeral resolved descriptor:

```rust
use open_gpui_devtools::ui_components::{
    ComponentSemanticIdentity, OpaqueSemanticNodeId, ResolvedSemanticNode,
    resolved_semantics_probe_snapshot,
};

let component = ComponentSemanticIdentity::for_component("TextInput")
    .expect("TextInput must have a canonical component contract row");
let node = ResolvedSemanticNode::new(
    component,
    OpaqueSemanticNodeId::new(42),
    semantic_descriptor,
);
let snapshot = resolved_semantics_probe_snapshot([node]);
```

`OpaqueSemanticNodeId` must not encode a renderer node id, accessible text, or application data.
The new payload replaces `contract_count` and claim rows with a root `node_count` plus typed,
redacted semantic nodes. Each node retains canonical contract/family metadata, role, state, actions,
structural counts, and presence facts; accessible text and numeric values are represented only by
typed redaction markers.

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

## Typed Committed Portal Anchors

Trigger-bound overlays no longer estimate a local rectangle at `(0, 0)` or retain raw submenu row
bounds as live geometry. Popover, Select, Combobox, HoverCard, Menu, and submenu paths now bind a
window-owned `PortalAnchorHandle` internally. Their public component APIs keep the same trigger
shape, but placement now follows committed transform and scroll geometry in the same frame.

Custom element followers retain one handle for the target instance, bind it exactly once in every
frame where the target is present, and resolve it through the named follower helper:

```rust
use open_gpui::{
    PortalAnchorExt as _, PortalAnchorHandle, portal_anchor_follower,
};

struct ViewState {
    anchor: PortalAnchorHandle,
}

let target = trigger.track_portal_anchor(&state.anchor);
let follower = portal_anchor_follower(&state.anchor, |snapshot, _window, _cx| {
    snapshot.map(|snapshot| {
        build_surface(snapshot.geometry().displayed_bounds()).into_any_element()
    })
});
```

Create the handle once with `window.new_portal_anchor()`; do not create it during each render. One
handle may feed multiple followers, but binding it to two targets in one frame is an error. A handle
cannot be used by another window. `PortalAnchorSnapshot` deliberately exposes opaque element
geometry rather than a raw matrix or mutable rectangle.
Target-root `with_subtree_transform` and `with_subtree_presentation` wrappers may appear before or
after `track_portal_anchor`. GPUI resolves them by tracked layout identity, so builder order does
not change displayed geometry, presentation, or Hidden unlink behavior. The same rule crosses a
cached `AnyView` boundary: wrappers on the view's rendered root remain part of the target, while
wrappers on ordinary descendants do not become anchor-wide facts.

During a draw, resolution reads only the current candidate. Outside a draw it reads only the last
completed frame. Hidden, absent, unmounted, rolled-back, or invalid targets resolve as `None`; do
not add an application-owned last-known fallback. Inert is still a linked GPUI source fact, although
interactive UI Components followers require Visible. Views that resolve an anchor are rebuilt on a
later frame instead of replaying a cached linked surface after another view removes the target.
Overlay inside regions now use the same checked displayed geometry, so transformed trigger or
surface bounds are not misclassified as outside presses.
The low-level `WindowOverlayRuntime::set_inside_region` entry point is replaced by
`set_element_inside_region` without an alias. Pass element layout bounds during prepaint; the
runtime captures `Window::hit_test_snapshot` with the exact active clip stack. The standard runtime
surface wrapper remains preferred.

Standalone Tooltip is the official component whose target and surface may be authored separately.
Pass the same retained handle to both sides and observe controlled unlink through the new callback:

```rust
let target = trigger.track_portal_anchor(&state.anchor);
let tooltip = Tooltip::new("save-help", "Save the current document")
    .portal_anchor(state.anchor)
    .open(state.tooltip_open)
    .on_open_change(|intent, _window, _cx| {
        // Commit intent.desired_open() in application state.
        // intent.reason() may be DismissReason::AnchorUnlinked.
    });
```

An initially closed Tooltip without a handle creates no overlay registration. Once a retained
Tooltip ID binds an external handle, later renders reuse that exact capability; changing it is a
typed `PortalAnchorModeChanged` error. Keep passing the stable handle whenever practical. Native
GPUI tooltip builders retain their intentional pointer-point anchor and do not call
`Tooltip::portal_anchor`.

`DismissReason` now includes `AnchorUnlinked`; update exhaustive matches. The runtime hides a
controlled follower immediately and emits one close intent. If the owner remains Open, the pending
intent stays visible without being emitted again. If the owner commits Hidden, the pending intent
and revision clear. Uncontrolled owners commit closed state automatically. Reopening after the
target returns starts a new overlay generation.

ContextMenu's explicit anchor point remains window-space. Dialog, AlertDialog, Sheet, and other
viewport surfaces use a named full-window portal. Neither inherits an ancestor transform or clip,
and neither should be converted into a fake element handle.

The removed `gpui_relative_overlay_layer`, Menu `submenu_trigger_bounds`, and component-local raw
trigger bounds have no compatibility aliases. Keep `OverlayAnchorInput` only as a pure placement
snapshot after a live target has been resolved.

## Window-Owned Bring Into View

Final reveal is no longer component-owned fixed-row or nearest-row arithmetic. Application
requests, winning focus claims, and AccessKit `ScrollIntoView` now share one window-owned authority
that walks committed scroll ancestry from inner to outer.

For an ordinary physical target, retain one handle and bind it in every rendered frame:

```rust
use open_gpui::{
    BringIntoViewAlignment, BringIntoViewOptions, RevealTargetExt as _,
};

let target = row.track_reveal_target(&state.reveal_target);
window.bring_into_view(
    &state.reveal_target,
    BringIntoViewOptions::vertical(BringIntoViewAlignment::Nearest),
    cx,
)?;
```

Create `state.reveal_target` once with `window.new_reveal_target()`. Do not recreate it during
render, bind it to two elements in one frame, use it in another window, or retain a raw rectangle as
fallback geometry. Use `bring_into_view_with_completion` when the exact `Completed` or `Cancelled`
outcome matters. Dropping the returned subscription only stops observation.

Every successfully published accessibility node now exposes `ScrollIntoView`. It is a geometry
action, not a Click or Focus alias, so disabled nodes may retain it while stale, suppressed, or
unpublished nodes cannot route it.

`BringIntoViewOptions` names physical horizontal and vertical axes. The alignments are `Nearest`,
`MinEdge`, `Center`, and `MaxEdge`; margins are finite non-negative physical edges. The vertical
convenience constructor preserves horizontal position. Do not translate these into logical
block/inline or start/end semantics until the framework has a direction authority.

VirtualizedList now exposes a strict two-phase contract. Replace final reveal math such as
`scroll_target_for_key` and `scroll_target_for_key_with_snapshot` with the complete builder path:

```rust
let list = VirtualizedList::new("results", "Results", items)
    .bring_key_into_view(
        selected_key,
        BringIntoViewOptions::vertical(BringIntoViewAlignment::Nearest),
    );
```

The list resolves a `VirtualizedListMaterializationResult`, uses private estimated geometry only to
mount the keyed row, then asks GPUI to reveal its bound physical target. Custom adapters may call
`materialization_target_for_key` or `materialization_target_for_key_with_snapshot`, but the returned
index and estimated flag are not final scroll coordinates. Duplicate, missing, disabled, status,
and structural keys fail closed. Reorder or filtering between materialization and reveal re-resolves
the stable key; do not persist an index.

Direct `ScrollHandle` operations remain valid explicit scrolling. Wheel, scrollbar, keyboard,
touch, or programmatic direct scrolling cancels affected in-flight reveal instead of silently
continuing an older animation. An explicit portal begins a new rendered ancestry; source-tree reveal
requires an explicit application request rather than implicit anchor traversal.

Custom two-phase adapters that schedule a physical request after mounting must capture
`Window::capture_deferred_bring_into_view_guard` from prepaint inside the intended final scroll
ancestry as soon as logical materialization completes, then later use
`Window::try_bring_into_view_with_guard_and_completion` after the target binds. The guard
atomically rejects an interrupted direct scroll, an unbound target, or a changed nested ancestry
without entering window authority. `ScrollHandle::direct_scroll_revision()` remains suitable only when an
adapter intentionally owns one known handle; neither capability is an offset, viewport snapshot,
or replacement reveal engine.

When a focus handoff itself spans frames, retain its `ScrollChainFence` from the original input or
materialization boundary and call `Window::focus_with_completion_and_scroll_fence`. This keeps
ordinary focus arbitration intact while suppressing only the automatic physical reveal when direct
scrolling, scroll-axis capability, or committed ancestry has changed. Do not recapture a fence
after user input to make an old focus operation eligible again.

`ListState::scroll_to_reveal_item` is removed without an alias; bind the rendered item to a
`RevealTargetHandle` when final nested reveal is intended. `UniformListScrollHandle` keeps
`scroll_to_item*` as explicit index-based direct scrolling and adds `base_handle()` for low-level
viewport access. Its tuple field, `UniformListScrollState`, and `DeferredScrollToItem` are now
private implementation state; use the named methods instead of mutating pending scroll records.

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
let binding = runtime.register_layer(registration, window, cx)?;
runtime.bind_layer_to_entity_release(&binding, &owner, window, cx)?;
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
policy or handles, and use the matching unregister methods only for explicit manual lifetime. For
entity-owned layers, call `register_layer` and then `bind_layer_to_entity_release` so subtree
cleanup follows owner release.

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

### DevTools Window Focus Projection

Construct the GPUI runtime focus projection from the rendered window instead of assembling focus
facts beside the runtime:

```rust
use open_gpui_devtools::gpui::GpuiRuntimeFocusSnapshot;

let focus = GpuiRuntimeFocusSnapshot::from_window(window_id, window, cx);
```

The snapshot now carries optional `focused_element_rendered`, opaque `focus_claim_revision`, and
opaque `rendered_frame_revision` facts. `focus_scope_count` and `focus_handle_count` are also
optional: a producer that cannot prove a fact must emit `None`, not a guessed false or zero. This
keeps older imported JSON distinguishable from a live negative observation. An inactive window may
retain a logical `FocusHandle`, but `from_window` reports no `focused_window_id` until that window
again owns keyboard focus. Downstream JSON consumers must accept null for every unavailable fact.
