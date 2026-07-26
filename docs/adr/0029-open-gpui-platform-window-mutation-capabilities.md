# ADR 0029: Open GPUI Platform Window Mutation Capabilities

**Status**: Accepted
**Date**: 2026-07-25

## Context

The former viewport-level `live_window_move` boolean grouped together several independent
questions: whether a backend reports shared-desktop coordinates, can dispatch a position or size
change, can observe the resulting placement, and can change a particular window state or input
flag. A single boolean could therefore advertise move support when only resize was usable, or
allow callers to infer a native commitment from a request that was only queued.

Native backends also differ materially. Windows can read native window state and perform typed
live size, state, and pointer-input requests, but its existing logical desktop coordinates are
scaled per window and are not comparable across mixed-DPI monitors. macOS consumes initial bounds,
state, and pointer-input configuration. X11 can observe global configure events and consume initial
placement configuration, but its move and resize notifications are not one atomic global
placement observation. Wayland intentionally leaves placement to the compositor: an XDG surface
can consume and observe surface-local size and state configuration, but no Wayland surface can
offer a global position contract.

## Decision

GPUI exposes one backend-neutral, property-specific window mutation capability contract. The
single `PlatformWindowMutationCapabilities` matrix separates coordinate authority from support for
position, size, windowed, maximized, fullscreen, minimized, restore bounds, pointer input,
focus-on-appearing, focus-on-click, alpha, topmost, and taskbar visibility. Each property is
explicitly unsupported, creation-only, or live; a creation-only claim requires a native creation
path, and a live claim also requires typed dispatch plus a readable native observation path for
the resulting fact.

The coordinate contract is explicit:

- macOS and X11 report `GlobalScreen` because their current facts use one shared desktop coordinate
  space.
- Windows reports `WindowLocal` and leaves live position unsupported until it has a mixed-DPI-safe
  shared desktop coordinate contract. Position and restore-bounds hints remain available at
  creation.
- Wayland reports `WindowLocal` and does not advertise global position.
- A coordinate claim does not imply atomic placement mutation. In particular, X11 must not claim
  one coherent global move-and-resize observation from separate configure handling.
- Wayland XDG creation can request maximized or fullscreen state before its first commit. LayerShell
  windows have no equivalent state request and remain unsupported for those kind-specific
  operations.

Windows advertises `Live` size, windowed, maximized, fullscreen, and pointer-input support.
Position, restore bounds, focus-on-appearing, and alpha are `CreationOnly`; minimized,
focus-on-click, topmost, and taskbar visibility are unsupported. Its live backend paths return
typed queued dispatch, guard each placement or pointer-input generation, read the resulting native
facts, roll back failed multi-step native writes, and emit one domain-and-generation-bound
terminal observation. Fullscreen rollback includes style, bounds, `WINDOWPLACEMENT`, restore
state, display/scale facts, and the `NonRudeHWND` taskbar property; pointer-input rollback restores
both the native style and the internal hit-test fact.

The remaining native projections advertise creation-only support where the backend consumes the
canonical creation request. They do not upgrade legacy resize, toggle, or boolean setters to
`Live`: those paths do not yet provide typed dispatch, generation ownership, and coherent observed
facts. Windowed, maximized, fullscreen, and minimized are distinct capability properties, so a
backend that cannot create or observe minimized state leaves that property unsupported.
X11 window-manager and Wayland compositor state requests remain requests: the resulting creation
facts may be adjusted and are authoritative only after native observation.

Position, size, each placement state, and restore bounds remain one GPUI placement conflict domain.
Pointer input, focus-on-appearing, focus-on-click, alpha, topmost, and taskbar visibility each own
an independent domain. The common GPUI authority owns all seven monotonic generation streams,
queued versus terminal outcomes, close handling, and the committed fact cache. Every backend
terminal observation carries the exact domain and generation supplied at dispatch. `Window`
rejects a stale generation before committing its facts, so a delayed callback cannot settle a
newer ticket or roll the public cache backward. Before a new request is classified as unchanged,
unsupported, or queued, the backend invalidates older queued work in that domain. Window close
invalidates every backend domain before retained tickets settle as `WindowClosed`.

`WindowMutationRequest` is the complete executable request vocabulary, not merely a diagnostic
matrix. The public placement, pointer-input, focus, alpha, topmost, taskbar, resize, zoom,
minimize, and fullscreen helpers are thin typed wrappers over it. `WindowPlatformFacts` carries
the corresponding committed independent-flag facts. A creation-only value is seeded from window
creation and preserved when a generic native geometry refresh cannot observe that property.
Native backends project their observable capabilities into this authority; they do not introduce
an optimistic parallel fact source.

The public `zoom_window`, `minimize_window`, and `toggle_fullscreen` helpers are state-only typed
wrappers over that placement authority. They return a `must_use` dispatch and do not attach stale
restore geometry to a state transition. The backend trait no longer exposes parallel legacy
commands that can bypass placement generations.

Moved, resized, and external window-state callbacks refresh the committed facts cache. A generic
state-change callback never settles a mutation ticket: only the generation-bound terminal
observation callback can do that.

The legacy `live_window_move`, `PlatformViewportFlagCapabilities`, and
`viewport_flag_capabilities` surfaces are deleted. Docking and diagnostics consume the one GPUI
matrix rather than mirroring backend-specific booleans. Dock retains terminal failures per window,
domain, request, and relevant committed facts so a rejected or unsupported request is not retried
every frame; a changed target or changed relevant facts permit a new attempt. Placement retry
fingerprints exclude activity, pointer input, and unrelated flag facts. DevTools serializes queued
requests and terminal observations as structured request/fact payloads rather than unstable debug
strings.

Capability projection before opening a window uses its actual creation kind and target
`Option<DisplayId>`; `None` selects the backend's primary or default display. This lets a backend
report display-dependent creation support exactly, including whether an X11 screen exposes a
transparent visual. An unavailable display id is normalized to `None` before both projection and
creation, so a stale saved target falls back to the current default rather than creating a profile
for one screen and a window on another. GPUI captures one immutable
`PlatformWindowMutationProfile` containing the `WindowKind` and resolved matrix when the window is
registered, keeps it readable while a window update temporarily removes mutable window state from
the registry, and removes it on close. Dock runtime status resolves every viewport window through
that profile instead of applying the backend's `WindowKind::Normal` or primary-display matrix to
heterogeneous windows.

Owning-platform CI both checks and tests each native backend package. Windows integration tests
exercise every advertised live domain against native readback, inject a frame-change failure after
the first pointer-style write to prove rollback and rejection, compare creation-time cache seeding
with independent Win32 readback, defer hidden-window placement, and use an external `WM_SIZE`
callback to refresh committed facts without a GPUI mutation request. macOS, X11, and Wayland
currently advertise no live domains. Their package tests assert exact kind-specific
creation-only/unsupported matrices and exercise pure creation projections that are consumed by the
production native constructors, including Wayland's XDG-versus-LayerShell split. Any future `Live`
upgrade must add an owning-runner dispatch, failure, and observation test in the same change.

## Consequences

- Callers can distinguish unsupported, creation-only, and live behavior before dispatching a
  request.
- A queued native request is not reported as an applied or committed platform fact.
- A delayed terminal from an older generation cannot replace committed facts or settle a newer
  ticket.
- Backends can expose creation-only or live resize, state, or pointer-input support independently
  without claiming unrelated position or restore-bound guarantees.
- Independent flags can dispatch, supersede, and settle without cancelling unrelated flag work.
- A capability consumer sees one tri-state matrix instead of combining placement facts with a
  second viewport-flag table.
- Wayland remains usable for observable local size and state changes without inventing a
  compositor-global coordinate model.
- Wayland capability lookup is window-kind-specific, so LayerShell never inherits XDG placement
  claims.
- X11 alpha creation support is target-display-specific and becomes unsupported when that screen
  has no transparent visual.
- Multi-viewport diagnostics retain each opened window's actual kind and display-resolved
  capability matrix rather than projecting one backend-wide normal-window answer.
- Windows does not claim cross-window global geometry until mixed-DPI coordinates are comparable.
- Native backend changes must revise both the capability projection and owning-platform
  observation evidence when a claim changes.

## Rejected Alternatives

- Keeping `live_window_move` would continue to conflate coordinates, dispatch availability, and
  observation.
- Treating every existing setter as live would advertise requests whose resulting native facts
  cannot be read back.
- Normalizing Wayland to a fabricated global desktop origin would make placement-dependent
  consumers incorrect under compositor control.
- Letting Docking infer capabilities from direct backend getters would duplicate and drift from
  GPUI's committed platform-fact authority.
