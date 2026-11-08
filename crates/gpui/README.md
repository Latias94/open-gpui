# Welcome to Open GPUI!

Open GPUI is a hybrid immediate and retained mode, GPU accelerated UI framework
for Rust, forked from Zed's GPUI framework code.

## Getting Started

Open GPUI is still in active development and is pre-1.0. There will often be breaking changes between versions. The current workspace MSRV is Rust 1.92. Add the following to your `Cargo.toml`:

```toml
open_gpui = { package = "open-gpui", version = "0.2.0" }
open_gpui_platform = { package = "open-gpui-platform", version = "0.2.0" }
```

- [Ownership and data flow](src/_ownership_and_data_flow.rs)
- [Accessibility](src/_accessibility.rs)

Everything in Open GPUI starts with an `Application`. You can create one with `open_gpui_platform::application()`, and kick off your application by passing a callback to `Application::run()`. Inside this callback, you can create a new window with `App::open_window()`, and register your first root view.

## Views And Reusable Components

`View` is the shared embedding contract for reactive views and stateless components. Continue
implementing `Render` for an entity's persistent state, or implement `RenderOnce` for a component
that consumes parent-supplied props. Derive `IntoElement` on a RenderOnce component to use it in
ordinary `.child(...)` composition.

Implement `View` directly when a reusable component combines props with an existing backing
entity. Return that entity's identity from `entity_id` and consume the props in `render`; use
`#[derive(IntoElement)]` or `ViewElement::new` to embed it. An entity identity establishes its
reactive boundary and local element-id scope. Two siblings at the same element-tree position
must not return the same identity. Nesting retains the parent scope. A View with no identity uses
the stateless type-name scope in all three drawing phases.

`Entity<T: Render>` and `AnyView` embed through `ViewElement`, with the concrete Render type
retained for profiling. AnyView remains a clonable, downcastable entity handle; its weak handle
does not keep the entity alive. Construct elements during rendering. The shared element consumes
its View once and participates in the existing frame, input, accessibility and resource lifecycle.
Explicit typed/erased/cached conversions preserve the caller's debug source location through
the shared constructor; release builds omit that metadata.

Use `Entity::cached(style)` or `AnyView::cached(style)` for entity-backed subtrees with a definite
layout size. They return ViewElement values. Notification or window refresh invalidates reuse,
alongside the existing geometry, inherited style and resource checks. Manual and stateless Views
have no public cache shortcut. When branches have different element types, finish each branch
with `into_any_element()`; the underlying entity identity and concrete profiling name survive.

## Atlas Ownership

`PlatformAtlas` insertion consumes an `AtlasKey`. Retain a caller-owned key with an explicit
clone when it is also needed for later removal. Builders run under the atlas lock and must not
re-enter that same atlas. Backend authors can use the shared `AtlasState`/`AtlasBackend` to
publish only successful inserts and keep lookup, removal and reset ordering in one owner.
Diagnostic insertion uses that same key publication path.

`HeadlessAtlas` supplies the shared CPU implementation for Test and Linux headless windows.
It allocates unique tile identities without uploading pixels. Removing a key and releasing its
last admitted texture lease are separate events: retained scenes keep their exact texture
instances, while a renderer reset invalidates old epochs. These lifetime controls do not
establish GPU submission, scanout or a numerical cache-performance result.

`Scene::clone()` records complete drawing data, including the frame-local clip arena and
paint journal. It does not acquire texture leases or transfer accepted-frame or submission
authority. A backend that retains drawing data must also retain its required resources.
Renderers validate the complete texture set and draw from frozen resource handles; a missing
instance requires refusal or fresh paint rather than a successful partial frame. WGPU's
window and headless paths share the same rendering core and exact-instance binding cache.

## Conditional Native Hot Reload

Enable `open-gpui`'s optional `hot-reload` feature and launch a native debug application through
`dx serve --hot-patch` using Dioxus CLI 0.7.10. GPUI initializes the connection during application
launch, accepts only patches for the current build and process, and applies executable code in
a foreground App update before refreshing windows. A native callback that temporarily holds the
App borrow delays application through the existing update owner. A quitting App refuses patch
application, and releasing the App-owned reload state cancels both tasks. Release builds and
WebAssembly do not enable this transport.

`View`/`RenderOnce` and stored `AnyView` render functions pass through the official Subsecond
boundaries. Existing model state and cached views can survive compatible render-body edits;
arbitrary type-layout migration and custom Element phase replacement need a restart or separate
integration. Callbacks outside these boundaries can opt into the reexported `subsecond` API.
Only load patches produced by the developer's trusted local build server.

A full-reload command reports that an application restart is required. Disconnects and failed
loads are reported; restart `dx serve` to reconnect. See the independently locked
[native consumer](../../tests/fixtures/hot-reload-consumer/README.md) for the real compiler,
state, exact Submitted-frame and pixel acceptance procedure and its platform limits. Windows x64
qualification uses the documented upstream dx jump-stub backport; the published 0.7.10 executable
has a reproduced stack-probe code-generation defect.

## Application Lifetime Under An External Loop

Use `Application::run_embedded` when the supplied Platform's `run` method returns control to
the host. Retain its `ApplicationHandle` for application ownership and enter it with
`handle.update(|cx| ...)`. An update batches nested entity work and flushes effects before
returning, using the same transaction as async App updates. `handle.to_async()` supplies a
weak AsyncApp for work across await points; it does not retain the application or cancel
tasks automatically when the last strong owner disappears.

The handle stays on its owning thread. Reentering it while that App is borrowed panics;
callbacks should use their current App context. A panic restores transaction bookkeeping,
but does not roll back already queued work or user state. Finish shutdown and drive native
window retirement before releasing the last application owner. Public `App::shutdown`
settles the existing shutdown fence; dropping the handle neither invokes shutdown nor pumps
an external event loop. Shutdown barriers during preparation and terminal native quit remain
the existing lifecycle contracts.

Launch can complete asynchronously after `run_embedded` returns. Its pending platform
callback may itself retain the App, so handle drop is not a launch-cancellation protocol.
The API still calls the supplied `Platform::run`: stock desktop implementations can block,
and the production Windows implementation owns process exit. Select an explicitly returning
platform for an external loop. This interface supplies no shared GPU or native window handle
attachment. Test-support's VisualTestPlatform returns after launch and delegates native
windows to its supplied backend; its host drives both the simulated dispatcher and native
messages. That test facility is not a production Windows external-loop backend.

See the [external-loop qualification](../../docs/verification.md#externally-driven-application-lifetime)
for actual consumers and platform limits.

## What This Crate Owns

`open-gpui` owns the application, window, element, entity, input, text, accessibility, and rendering-facing API that applications use directly. It does not choose a backend by itself; use `open-gpui-platform` for target-specific application construction.

Native backends cover macOS, Windows, Linux, and FreeBSD through platform crates. WebAssembly uses `open-gpui-web` through `open-gpui-platform` on `wasm32-unknown-unknown`. Backend capabilities such as WebGPU, screen capture, platform viewport windows, and compositor features remain runtime facts.

For higher-level controls, use `open-gpui-ui-components`. For deterministic UI motion primitives, use `open-gpui-motion`. For retained dock spaces, use `open-gpui-docking`.

## Platform Window Lifecycle

Window creation, presentation, and already-open mutations have separate observed-fact contracts.
Query `Window::window_capabilities()` before opening or mutating a window. Its `creation` half
reports support for a non-activating first appearance, typed transient ownership, and first-frame
ordering. Its `mutations` half reports position, size, each placement state, restore bounds,
pointer input, coherent activation policy, alpha, topmost, and taskbar visibility as
`Unsupported`, `CreationOnly`, or `Live`. Only `Live` properties can be requested on an existing
window.
For app-level diagnostics that only retain an `AnyWindowHandle`, use
`App::window_profile(handle)`: the immutable profile records the opened window's actual
`WindowKind` and target-display-resolved creation and mutation capabilities and is removed when
the window closes. Before opening a window, `App::window_capabilities_for(kind, display_id)`
projects the same support; `None` selects the backend's primary or default display. An unavailable
display id is normalized to `None`, so capability projection and window creation use that same
fallback.

`WindowOptions::focus_on_appearing` is one-shot appearance policy. It does not disable later
activation. Lifetime activation and click-focus behavior live together in
`WindowOptions::activation_policy`, while pointer acceptance remains independent. Use
`App::transient_window_owner` to create an application-bound token for an exact live owner
generation; native ownership assists grouping and z-order but never owns window teardown.
`Window::creation_facts` records the immutable applied creation relationship, and
`Window::presentation_facts` distinguishes native creation, accepted frames, submitted presents,
non-empty presents, the exact latest submission outcome, bounded initial-presentation settlement,
and current native visibility. Initial-presentation capabilities distinguish a frame submitted
while hidden, a frame submitted after native visibility, and protocols where the first frame
submission itself establishes native visibility.

Use `WindowPlacementRequest` with `request_window_placement_request`, a named flag helper, or
`request_window_mutation` with a `WindowMutationRequest` when the result matters. A
`Queued(WindowMutationTicket)` reports that GPUI dispatched intent, not that the operating system
committed it. Retain the ticket to observe its one terminal `Exact`, `Adjusted`, `Superseded`,
`Rejected`, `Unsupported`, or `WindowClosed` outcome. Every backend terminal carries its domain
and generation; a stale terminal is rejected before it can replace committed facts.
`Window::platform_facts`, bounds/state/flag getters, and Dock integrations update only from the
backend's committed facts snapshot.

Position, size, window state, and restore bounds share one placement conflict domain. Pointer
input, coherent activation policy, alpha, topmost, and taskbar visibility each own an independent
generation. Closing a window invalidates queued backend generations before settling their tickets
as `WindowClosed`. `WindowBounds` remains a compatibility value for windowed, maximized, and
fullscreen creation or complete requests; `WindowPlacementRequest` additionally supports partial
updates and minimized state. `resize`, `zoom_window`, `minimize_window`, `toggle_fullscreen`,
`set_background_appearance`, `request_activation_policy`, and the named flag helpers return the
same `must_use` typed dispatch rather than bypassing mutation generations. See
[ADR 0029](../../docs/adr/0029-open-gpui-platform-window-mutation-capabilities.md).

## Interactive Subtree Geometry

Use `SubtreeTransform` when axis-aligned scale or translation must apply consistently to an
interactive subtree. The transform runs after layout, so measurement, flex/grid placement, scroll
extent, and sibling flow do not change:

```rust
use open_gpui::{
    ParentElement as _, SubtreeTransform, SubtreeTransformExt as _, SubtreeTransformOrigin, div,
    point, px, size,
};

let transform = SubtreeTransform::try_new(
    size(1.2, 0.9),
    point(px(8.0), px(-4.0)),
    SubtreeTransformOrigin::CENTER,
)
.expect("finite positive transform");

let content = div().child("Interactive content").with_subtree_transform(transform);
```

The supported public contract is finite positive normal axis scale, finite logical-pixel translation, and
a post-layout origin. Rotation, skew, arbitrary affine matrices, and 3D are intentionally absent.
Use `TargetedEvent` helpers for target-local input coordinates and `ElementGeometry` or
`measured_element` for committed layout/displayed geometry. Raw platform event positions remain in
window coordinates.

See [ADR 0021](../../docs/adr/0021-open-gpui-interactive-subtree-transform-authority.md) for the
cross-channel and numeric-failure contract.

## Layout-Preserving Subtree Presentation

Use `SubtreePresentation` when a whole subtree must retain its layout slot while changing its
participation in rendering and interaction:

```rust
use open_gpui::{
    ParentElement as _, SubtreePresentation, SubtreePresentationExt as _, div,
};

let inert_content = div()
    .child("Painted, but excluded from input, focus, IME, and accessibility")
    .with_subtree_presentation(SubtreePresentation::Inert);
```

`Visible` participates in every channel. `Inert` keeps layout and paint but suppresses input,
focus, IME, tooltips, overlay intent, and accessibility. `Hidden` keeps layout only. A suppressive
ancestor always wins, including through transforms, deferred elements, and cached views. Returning
to `Visible` rebuilds current participation without replaying stale input or focus claims.

Use `Display::None` when the subtree must leave layout. Use component disabled state when a control
should remain discoverable with disabled semantics. See
[ADR 0022](../../docs/adr/0022-open-gpui-subtree-presentation-authority.md) for the full matrix and
committed-frame cleanup contract.

### Focus Observation

`on_focus_in` observes effective focus entering a handle or descendant while the platform window
is active. `on_focus_committed` observes one handle becoming the exact committed local focus;
`on_focus_committed_in` observes committed focus entering a handle or descendant. Both committed
observers work while the platform window is inactive, and later platform activation does not
replay them. `Window::focused` reads current intent and may be provisional during a candidate
render; `Window::committed_focus` reads the exact leaf from the last committed window-local tree.

Use `focus_with_completion` or `blur_with_completion` for a retained transaction that must settle
one exact or empty focus-authority request. The callback receives `Committed`, `Rejected`, or
`Superseded` after final rendered membership is known. A request issued after the current frame
seals input and accessibility authority is qualified in one later platform frame, so late
presentation failure cannot produce stale success bookkeeping and cached commit replay cannot
recursively redraw inside one effect cycle. `Window::blur` and `Window::disable_focus` take `cx`
so superseded completion callbacks are always scheduled.

## Bring Into View

Retain a same-window `RevealTargetHandle` when an application, focus transition, or accessibility
action needs to reveal an element through nested scrollports. Bind the handle in every rendered
frame and submit explicit physical-axis options:

```rust,ignore
use open_gpui::{
    BringIntoViewAlignment, BringIntoViewOptions, RevealTargetExt as _,
};

let target = content.track_reveal_target(&state.reveal_target);
window.bring_into_view(
    &state.reveal_target,
    BringIntoViewOptions::vertical(BringIntoViewAlignment::Nearest),
    cx,
)?;
```

Create the handle once with `window.new_reveal_target()`, not during every render. A completed frame
commits the target's checked geometry and inner-to-outer scroll ancestry. Application requests,
the winning focus claim, and AccessKit `ScrollIntoView` share one request sequence, overlap
arbitration, transform conversion, and cancellation contract. `bring_into_view_with_completion`
observes the terminal outcome of one exact request; dropping its subscription does not cancel the
request.

The public axis vocabulary is physical: horizontal and vertical policies use `Nearest`, `MinEdge`,
`Center`, or `MaxEdge`, with checked physical margins. `BringIntoViewOptions::vertical` preserves
the horizontal offset. Virtual collections must materialize stable logical identity before binding
the physical target; GPUI does not accept indices or collection keys. See
[ADR 0025](../../docs/adr/0025-open-gpui-bring-into-view-authority.md) for arbitration, transforms,
portals, Motion, and cancellation behavior.

Custom two-phase adapters capture `Window::capture_deferred_bring_into_view_guard` from prepaint
inside the intended final scroll ancestry as soon as logical materialization completes, then call
`Window::try_bring_into_view_with_guard_and_completion` after the physical target binds. The
opaque guard atomically checks the target, its complete scroll ancestry, and relevant direct-scroll
interruption before a request enters window authority. `ScrollHandle::direct_scroll_revision()` remains available for a
known-single-handle low-level adapter; it is not a scroll offset or a second bring-into-view API.

For a focus handoff that spans frames, retain the opaque `ScrollChainFence` captured from the
committed anchor chain at input time or the current prepaint ancestry after materialization, then
use `focus_with_completion_and_scroll_fence`. Focus still settles through ordinary arbitration;
only its implicit physical reveal is suppressed when direct input or a chain/axis change invalidates
the fence.

### Dependencies

Open GPUI has various system dependencies that it needs in order to work.

#### macOS

On macOS, Open GPUI uses Metal for rendering. In order to use Metal, you need to do the following:

- Install [Xcode](https://apps.apple.com/us/app/xcode/id497799835?mt=12) from the macOS App Store, or from the [Apple Developer](https://developer.apple.com/download/all/) website. Note this requires a developer account.

> Ensure you launch Xcode after installing, and install the macOS components, which is the default option.

- Install [Xcode command line tools](https://developer.apple.com/xcode/resources/)

  ```sh
  xcode-select --install
  ```

- Ensure that the Xcode command line tools are using your newly installed copy of Xcode:

  ```sh
  sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
  ```

## The Big Picture

Open GPUI offers three different [registers](<https://en.wikipedia.org/wiki/Register_(sociolinguistics)>) depending on your needs:

- State management and communication with `Entity`'s. Whenever you need to store application state that communicates between different parts of your application, you'll want to use Open GPUI's entities. Entities are owned by Open GPUI and are only accessible through an owned smart pointer similar to an `Rc`. See the `app::context` module for more information.

- High level, declarative UI with views. A window root is an `Entity` implementing `Render`. Its tree can compose entity-backed views, props-bearing manual `View` implementations and stateless `RenderOnce` components through `ViewElement`. Views build a tree of elements, lay them out and style them with a tailwind-style API, and then give them to Open GPUI to turn into pixels. See the `div` element for general layout, styling and interaction.

- Low level, imperative UI with Elements. Elements are the building blocks of UI in Open GPUI, and they provide a nice wrapper around an imperative API that provides as much flexibility and control as you need. Elements have total control over how they and their child elements are rendered and can be used for making efficient views into large lists, implement custom layouting for a code editor, and anything else you can think of. See the `element` module for more information.

Each of these registers has one or more corresponding contexts that can be accessed from all Open GPUI services. This context is your main interface to Open GPUI, and is used extensively throughout the framework.

## Other Resources

In addition to the systems above, Open GPUI provides a range of smaller services that are useful for building complex applications:

- Actions are user-defined structs that are used for converting keystrokes into logical operations in your UI. Use this for implementing keyboard shortcuts, such as cmd-q. See the `action` module for more information.

- Platform services, such as `quit the app` or `open a URL` are available as methods on the `app::App`.

- An async executor that is integrated with the platform's event loop. See the `executor` module for more information.,

- The `[open_gpui::test]` macro provides a convenient way to write tests for your Open GPUI applications. Tests also have their own kind of context, a `TestAppContext` which provides ways of simulating common platform input. See `app::test_context` and `test` modules for more details.

Currently, the best way to learn about these APIs is to read the Open GPUI examples and the framework source. This repository is a fork of Zed GPUI, but it is maintained as an independent Open GPUI workspace.

## Power Modes And Frame Cadence

Read `App::power_mode` and retain an `App::on_power_mode_change` subscription to observe the
platform's energy preference. `PowerMode::Unknown` means the platform cannot determine or
distinguish the mode; it is not a guessed performance setting. Power observation does not change
the host's system settings.

Mappings follow the native API: Windows maps battery saver on/off to LowPower/HighPower, while
macOS reports LowPower or Unknown. Linux maps recognized power-saver/balanced/performance profiles
to the corresponding mode. An unsuccessful or unrecognized query retains Unknown. These values
describe those reports rather than measured performance or energy use.

Set `WindowOptions::inactive_frame_interval`, `low_power_frame_interval` and
`thermal_frame_interval` independently when opening a window. Defaults are approximately
30 fps inactive, 30 fps in Low Power Mode and 60 fps under serious/critical thermal pressure.
`None` disables an individual limit. The longest applicable interval wins; sustained high-rate
input bypasses the inactive limit while power/thermal limits still apply. Mandatory native
presentations and existing recovery transitions bypass limits.

Dirty work, a scene awaiting presentation, next-frame callbacks and high-rate-input presentation
share the same pacing decision. The frame source retries deferred work without consuming its
force-render hint. Pure idle callbacks neither consume a cadence slot nor wake a parked source.
Small callback delays retain stable deadlines; a longer stall restarts cadence instead of
producing a catch-up burst. These rules do not replace accepted-frame or native submission facts.

The [power/cadence qualification](../../docs/verification.md#power-modes-and-stable-frame-cadence)
records backend sources, executed platform/consumer scope and remaining physical timing/energy
limits.

## Frame And Hang Measurement

Enable the `profiler` feature to read `Window::frame_duration_snapshot` and
`Window::input_latency_snapshot`. Aggregates are collected independently of trace retention.
Frame-duration reports include `by_refresh_interval`, grouping the window's accepted draws and
submitted frames by the display interval captured for each draw. Each group reports on-time,
late and unmeasured frames, missed refreshes, draw/work histograms and input-response statistics.
Use `histogram_bucket_counts` with the public duration/work bounds when formatting a report.

Frame work runs from the later of the captured frame signal and first invalidation to the start
of platform submission. A missing signal leaves work unmeasured. Lateness uses the display
interval with the official 120-Hz lower bound, or the 60-Hz fallback when refresh is unknown;
missed refreshes use the actual known interval. This differs from input latency, which continues
through submission completion. A successful retry uses its accepted scene's original timing;
refused candidates and unsuccessful submissions do not complete a frame measurement.

Read `App::foreground_journal` for independent event collectors and the public
`profiler::hang::HangDetector`, or retain the handle from `App::start_hang_monitor` for background
polling. Measurement version 3 separates threshold, late-frame and blocking-work incidents and
reports active-use time. View contributors identify the actual view type and request-layout,
prepaint or paint phase, excluding nested view time. Detection reports completed intervals;
the journal is bounded and exposes discontinuities to readers.

Raw input-activity controls follow the existing dispatch lifecycle, including nested dispatch,
rollback and unwind. They let active-time accounting distinguish inactive gaps from an input
handler still running. They do not add work or seal an interval; completed input events keep their
ordinary work and latency meaning.

The [measurement qualification](../../docs/verification.md#refresh-aware-frame-and-hang-measurement-v3)
records executed consumer/platform scope separately from source inspection and numeric overhead.

## Verification

For focused changes in this crate, run:

```sh
cargo check -p open-gpui --tests --locked
cargo run -p xtask -- verify-release-docs
```

For end-to-end workspace confidence, use the root `cargo run -p xtask -- verify` gate.

## License and Attribution

Open GPUI is licensed under Apache-2.0 and is forked from Zed's Apache-2.0 GPUI framework code. See this package's `LICENSE-APACHE` and `NOTICE` files for fork attribution, and the repository root `README.md` for full dependency attribution and release license guidance.
