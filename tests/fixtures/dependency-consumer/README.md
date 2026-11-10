# Independent dependency consumer

This application is its own Cargo workspace. It consumes Open GPUI through path dependencies
without inheriting the repository workspace's dependency resolution or lockfile. Its own
`Cargo.lock` records the consumer resolution.

The application pins Taffy 0.15.0 and, on macOS, Core Foundation 0.10.1 to exercise
application-owned compatible dependency choices independently of the library workspace.
Its direct WGPU dependency selects upstream gfx-rs revision
`cddb2f5d80dc0009b2ef6767635140cacf90a007`, matching the renderer source, and normally requests
only `std` for the preview's API types. The pinned correction declares Rust 1.93.

The optional `wgpu-renderer` feature also includes `open-gpui-wgpu` in the consumer graph, so the
all-features gate compiles the actual renderer against the application's pinned WGPU source.
Cargo unifies the renderer's WGPU feature requests with the direct dependency's features. This
compile check does not select WGPU as the Windows platform backend. Passing public WGPU types
between application and backend requires the same source identity; a registry version with
the same package version is a distinct Rust type.

The source composes a GPUI `Render` view, a semantic button activation handler, custom canvas
painting, and a `DockSurface` panel factory. Taffy's size type supplies the preview dimensions;
WGPU's texture format type supplies the preview label. It adds no document or history model.

The platform dependency enables `x11` and `wayland` on Linux and FreeBSD, and `font-kit` on macOS.
Windows uses the shared platform dependency without additional fixture-selected features.

Run these commands from the repository root. For dependency maintenance, generate or refresh the
consumer lockfile, then review and commit it:

```text
cargo generate-lockfile --manifest-path tests/fixtures/dependency-consumer/Cargo.toml
```

The regular fixture gate checks all application targets with all features, including the WGPU
renderer, against that lockfile and reuses the repository's build directory with one build job:

```text
cargo check --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --all-targets --all-features --jobs 1 --target-dir target
```

Run the same gate separately on Windows, Linux, and macOS in CI. Record results per host;
a Windows result does not qualify compilation on macOS, Linux, or FreeBSD.

Set `CARGO_BUILD_JOBS=1` for serial compilation when running the nextest commands below.
Nextest's `--jobs` and `--test-threads` both control test concurrency; use only
`--test-threads=1` rather than supplying both options.

This is a source compilation and dependency resolution gate. It requires the normal native build
prerequisites for the host platform. No application launch or test framework is required. Passing
the gate does not qualify native window behavior, visual output, GPU execution, or the declared
minimum Rust version. Platform runtime features and renderer execution need separate validation.

The optional public text tests consume `IndentAdjustment`, Unicode wrapping, Middle truncation and
the fluent style API through this independent lockfile. Run them separately from compilation:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --all-features --test wrapping --test-threads=1 --no-fail-fast --no-tests fail
```

These tests use `NoopTextSystem` for deterministic boundary and run checks. They do not launch the
application or establish real-font, native, GPU or compiler-floor qualification. The framework's
separate wrapping consumer uses actual Cosmic fonts and public headless paint.

The optional `fonts` tests exercise public prewarming and backend-only family inventory. With
`wgpu-renderer`, the same independently locked consumer installs tracked Cosmic fonts, shapes
mixed text, installs another face and retains the old layout, glyph bytes and exact text/backend
lifetime. It uses software rasterization and does not establish native window or GPU output:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --all-features --test fonts --test-threads=1 --no-fail-fast --no-tests fail
```

The optional `text_layers` test type-checks a custom Element that delegates measurement/prepaint
to StyledText and uses public background/highlight/foreground passes. It verifies the exact API
signatures without launching that element. Actual layered primitive order, real Cosmic paint and
native source-fragment/underline submission have separate framework consumers:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --all-features --test text_layers --test-threads=1 --no-fail-fast --no-tests fail
```

The optional `window_text` test checks the public Rc window-text getter signature and directly
consumes two WindowTextSystem owners over one global Arc TextSystem. It exercises complete source,
cache isolation, width changes, reconstruction and exact final release with NoopTextSystem.
It does not launch an App or Window. The framework has separate two-window Cosmic headless and
native reconstruction consumers; these tests do not establish renderer performance:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --all-features --test window_text --test-threads=1 --no-fail-fast --no-tests fail
```

The optional `grid_layout` test consumes the shared GridTemplateMinSize type and fluent
row/column content sizing through the public style API. It remains direct API evidence;
actual measured grid/text/root painting and native submission have separate framework consumers.
The independent application does not enable stack growth unless it explicitly requests the
framework's `stacker` feature.

The optional `images` tests directly consume public `Image::to_image_data` without test-support.
They preserve animated WebP frames, delays and BGRA color/alpha, static JPEG EXIF orientation,
and malformed/truncated-input errors through this independent lockfile. Actual asynchronous
Img loading, animation clocks, atlas reuse and native Submitted/readback/retirement have
separate framework consumers; direct decoding does not qualify those paths:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --all-features --test images --test-threads=1 --no-fail-fast --no-tests fail
```

The optional `embedded_app` tests type-check consuming Application::run_embedded, shared
ApplicationHandle updates with owned generic results, and weak AsyncApp access without
test-support. They do not construct a platform, start an App, or qualify a production external
loop. The framework separately tests App lifecycle and a returning visual-test host over real
Windows windows:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --all-features --test embedded_app --test-threads=1 --no-fail-fast --no-tests fail
```

The optional `asset_cache` tests type-check the Option-returning App fetch and opaque owning
image-entry APIs without test-support. A downstream custom image cache can start an entry,
subscribe each current view with `use_image` and inspect a published result with `get`.
They do not launch an App or qualify cancellation, notification, native frames or performance;
the framework's owner and actual multi-window consumers supply that separate evidence:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --all-features --test asset_cache --test-threads=1 --no-fail-fast --no-tests fail
```

The optional `displays` tests type-check App, Window and Context display subscriptions without
test-support or engine-private types, and execute public refresh-rate conversion controls.
They do not launch an App or establish native display changes. Framework owner tests and the
separate Windows ingress/lifecycle consumer qualify those respective paths:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --all-features --test displays --test-threads=1 --no-fail-fast --no-tests fail
```

## Lifecycle and visibility consumer

The `lifecycle` integration test target consumes the public lifecycle and presentation visibility
APIs without `test-support`. Its source baseline is official Zed at
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012),
including the subscription mechanism from
[`7a7c3e1d2f`](https://github.com/zed-industries/zed/commit/7a7c3e1d2f03195c5fa19bc890da330ad7f3abef)
and the sleep/visibility mechanism from
[`3db02c2c77`](https://github.com/zed-industries/zed/commit/3db02c2c77659ad29810594e6249287ea1e791f7).

Three tests contain typed, deliberately unexecuted closures. They check that downstream code can
register multiple `App::on_system_wake` subscriptions and an `App::on_system_sleep` subscription
through `&App`, with owned mutable callbacks receiving `&mut App`. These return `Subscription`;
the former single-handler `Application::on_system_wake` setter is replaced by the App API.
Window consumers read `visibility()` and `is_visible()` and register a callback receiving
`(WindowVisibility, &mut Window, &mut App)`. Entity consumers register through `&Context<T>` with
callbacks receiving `(&mut T, WindowVisibility, &mut Window, &mut Context<T>)`.

The backend closure consumes `&dyn Platform` and `&dyn PlatformWindow`; it does not implement a
mock backend. Custom backends must implement `Platform::on_system_sleep`,
`PlatformWindow::visibility`, and `PlatformWindow::on_visibility_change`; none has a default.
The fork's native sleep/wake ingress uses `NativeAppNotificationHandler`, an
`Arc<dyn Fn() + Send + Sync>`, rather than upstream's boxed mutable notification callback.
The platform window callback remains `Box<dyn FnMut(WindowVisibility)>`. The backend must provide
an initial visibility value and report transitions on the main thread outside a window update.

Keep returned App, Window, and Context subscriptions alive for as long as observation is needed.
Dropping a subscription cancels its callback. `detach()` consumes the handle without cancelling
observation; a detached observer remains tied to its emitter's lifetime, and Context observers
weakly reference their entity. `Subscription::join` groups two cancellation lifetimes, including
detaching both. Sleep handlers have little time before suspension and should save state or cancel
work. The five executable controls cover `WindowVisibility::{Visible, Hidden}.is_visible()` and
real `Subscription::new` values: cancellation once on drop, no cancellation on detach, both joined
cancellations on drop, and neither joined cancellation on detach, including release of captures.
They do not prove delivery or cleanup of registered App, Window, or Context observers.

Visibility describes the platform's presentation signal, independently of native shown/mapped
state or focus. The source contracts have these limits; this consumer does not execute them:

- macOS uses `NSWindow.occlusionState` to report occlusion and other presentation changes.
- Windows uses shown/minimized state; a fully covered window stays `Visible`, and display sleep
  does not produce a visibility change.
- Wayland uses the xdg-shell v6 `suspended` state; older compositors never report `Hidden`.
- X11 uses mapped state and `VisibilityNotify`; compositing generally prevents full-occlusion
  reports.
- Web uses Page Visibility for background tabs and minimized browser windows; browsers provide
  no corresponding system sleep/wake notification.

The public `headless()` constructor still uses the native platform loop. On Windows that loop
blocks and exits the process; its returning test constructor requires `test-support`. This target
therefore does not start an App, construct a Window, or synthesize an application owner or window
identity. Run it separately with the existing lockfile and root build directory:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test lifecycle --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

Passing this target establishes public API compilation and the five standalone controls only.
It does not qualify native power ingress, observer delivery/order/deduplication, visibility
transitions, frame scheduling, renderer submission, native `Submitted`, or performance. Those
require separate framework owner and real platform consumers. No profiler feature or lockfile
change is needed for this target.

## Profiler consumer

The optional `profiler` feature forwards directly to `open_gpui/profiler`; it does not enable
`test-support`. The `profiler` tests have separate enabled and default/off cases and do not
construct a platform, start an App, or open a Window.

With the feature enabled, typed closures consume the public Window snapshot/reset APIs, the
input/submission/refresh diagnostic count fields, an owned `WindowProfiler`, and draw/present
events. The owned profiler uses a real Window's ID only inside an unexecuted closure. These
closures establish API accessibility and signatures, not runtime sampling behavior. The executed
controls toggle the public trace flag, record a task through the public runnable timing entry
points, read its snapshot and incremental collector, clear the trace buffer, and construct and
poll an owned frame-event collector. The task counter remains monotone across clear/re-enable;
the same collector must receive only the new task without resetting its cursor. No frame events
are generated by these controls.

Without the feature, attempting to enable tracing and record a task must leave the public
aggregate accessors empty, including current-thread timings and action statistics.

When introducing this feature, first refresh and review the consumer lockfile with the maintenance
command above so its resolution includes `hdrhistogram`. Run both configurations separately against
that lockfile, reusing the root build directory:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test profiler --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features profiler --test profiler --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

This consumer does not qualify Window invalidation, histogram sampling across draw/present,
input-to-Submitted latency, renderer submission outcomes, or native timing. Framework owner and
native consumers retain the separate exact-generation and Submitted diagnostic assertions.
Journal/hang telemetry and the fork's existing Criterion benchmark setup are outside this gate.

## Foreground journal and hang consumer

The `foreground_journal` integration test target uses the same optional `profiler` feature,
without `test-support`, additional fixture dependencies, or a separate application/runtime
owner. Its coherent official source is Zed
[`d284cbbc87b55d659303106ced627c8612369827`](https://github.com/zed-industries/zed/tree/d284cbbc87b55d659303106ced627c8612369827),
within the dependency-ordered refresh toward the cycle target
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012).
This gate does not claim the later frame-signal or refresh-aware measurement mechanisms.

Two tests contain explicitly named, deliberately unexecuted `_type_only` closures. They consume
`App::foreground_journal()`, cloned journal handles, independently held collectors,
`DrainedEntries`, `IntervalSealer`, and `HangDetector` polling/first-presentation access. On
non-Wasm targets, the monitor closure consumes
`App::start_hang_monitor(HangMonitorConfig, on_poll)` with an owned mutable, `Send + 'static`
callback receiving `HangMonitorPoll`, returning `Result<(), HangMonitorError>`. These are
signature checks only: they do not construct an App, record work, poll a live stream, or spawn
a monitor thread. The APIs retain their official module paths under
`open_gpui::profiler::{journal, hang}`.

Monitor intervals must be positive and form a representable native deadline; invalid values
return `HangMonitorError::InvalidInterval` without occupying the App slot. Shutdown awaits one
poll of finalized journal entries and its callback within the existing bounded shutdown wait.
A contended publisher tail can remain queued until the next foreground publication; Flush is
not a guarantee that every offered entry has finalized. The detector remains post-hoc and does
not report work that never returns. Ordinary callback completion, observer panic/reentry and
bounded waiting have separate framework tests.

The executable value controls feed public action/input values and a small-poll summary into
the public sealer, close them with an explicit idle boundary, and check nested occupancy,
threshold versus budget qualification, startup/steady conversion metadata and contributor
capping. They also check monitor error sources and distinct interval/flush reasons on non-Wasm
targets. These are constructed data controls, not measured input dispatch, actual slow work,
or JSON serialization. All journal/detector tests require the fixture's `profiler` feature;
monitor tests additionally require a non-Wasm target.

Keep the existing consumer lockfile. Compile ordinary application targets with instrumentation
disabled, then run the feature-enabled public consumer separately, serially, from the root:

```text
cargo check --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --all-targets --jobs 1 --target-dir target
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features profiler --test foreground_journal --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

The framework's separate crate-external `crates/gpui/tests/foreground_journal.rs` target requires
both `test-support` and `profiler`. It uses the existing `TestAppContext` and `VisualTestContext`,
real Window input handlers and renders, deferred platform frame requests, visibility changes,
and sleep/wake notification ingress. Its collectors, sealer and detector observe independently;
serialized incident comparison executes through `serde_json` in that framework target. The
non-Wasm shutdown scenario starts the App-owned monitor, uses shutdown to request and await
the final flush, and checks callback-thread, window/App release, channel disconnection and
collector survival across App release. It uses no fabricated Window ID, timing-buffer cursor,
or manually recorded submission.

```text
cargo nextest run -p open-gpui --features test-support,profiler --test foreground_journal --test-threads=1 --no-fail-fast --no-tests fail
```

Those framework cases use the test platform's presentation callback and outcome. They do not
establish native `Submitted` receipts, GPU output, physical power transitions, native visibility,
frame-deadline expiry, a never-returning foreground watchdog, or measured overhead. Native
input/draw/submission, power/visibility interruption, shutdown flush and retirement belong to
the separate `crates/gpui_windows/src/native_profiler_tests.rs` trajectory. The commands above
are verification instructions, not a record that this gate has been compiled or executed.

## Platform frame signal and skip consumer

The `frame_signals` integration target consumes the official frame-request source and completed
skip APIs introduced by
[`cd4fc8de4c`](https://github.com/zed-industries/zed/commit/cd4fc8de4ca8548cca2567352b87bcaaec13328f)
with the idle-interval correction from
[`36b6d0951f`](https://github.com/zed-industries/zed/commit/36b6d0951fdee409f0957294a69360ba2e8e980e).
These mechanisms remain part of the refresh toward
[`635026fabc`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012);
this target does not qualify later frame-latency measurements or refresh estimation.

`FrameRequestSource`, both `RequestFrameOptions::signal_at` and `signal_source`, and
`PlatformFrameSignal::{new, capture, record, take}` are available with instrumentation enabled
or disabled. Executable controls construct the complete options value, check defaults and
copy/equality behavior, and drain new/default accumulators without constructing a Platform.
With `profiler`, they check that capture evaluates its closure exactly once, coalescing retains
the earliest timestamp and its matching source in either source order, and draining permits
another independent request. On non-Wasm hosts, scoped producer threads exercise the shared
accumulator concurrently. Without `profiler`, capture evaluates its closure zero times,
the accumulator has zero timestamp-storage size, and recording leaves it empty.

The explicitly named `_backend_type_only` and `_journal_type_only` closures are deliberately
unexecuted. The former checks the actual platform callback signature. The latter consumes the
feature-gated public `PlatformSignal`, `ReentrantFrameSkipped`, `FrameSkipped`, skip reasons,
`IntervalBoundary::FrameSkipped`, sealer and incident conversion APIs using a Window identity
obtained only inside the closure. Its sealer and conversion calls are signature evidence,
not executed journal or serialization controls. Separate executable comparisons preserve the
three completed skip reasons. Signal and reentrant markers are diagnostic points, not work or
sealing boundaries. A completed skip seals retained work without satisfying pending demand;
idle callbacks with no retained work must not seal empty intervals.

Run both fixture configurations separately, preserving its independent lockfile and sharing
the root build directory. Neither configuration enables `test-support`:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test frame_signals --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features profiler --test frame_signals --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

The separate crate-external `crates/gpui/tests/frame_signals.rs` target requires both
`test-support` and `profiler`. It uses public TestAppContext/VisualTestContext, actual Window
input dispatch and rendering, controlled test-platform callbacks, and actual presentation
facts. It checks source metadata before completed skips, real input sealed without a new draw,
raw marker behavior and a constructed reentrant marker's sealer behavior, `frame_skipped` JSON
conversion, idle callbacks producing neither empty snapshots nor incidents, and inactive
throttling retaining pending input, next-frame callbacks and a force-render request. In the
same inactive trajectory, `require_presentation: true` with `force_render: true` must run the
queued callback and submit a new exact accepted generation while the window remains inactive.
A second pass activates that same window and consumes the retained force hint through a
default request. Ordinary requests with neither flag still skip while callbacks are throttled;
after successful presentation they leave only raw markers, without another draw or interval.
Test-mode effect flushing draws dirty windows before platform delivery; accepted and submitted
generations distinguish that real pending scene from a new callback-driven draw.
A separate scenario requests presentation of that accepted scene while inactive, observes
the actual `Deferred` attempt without a new draw, and checks that a completed no-render skip
retains demand. Closing the window releases that demand and its root entity; real noninvalidating
input on a second fully submitted window then returns the retained work to idle without another
draw. Releasing a held native terminal
callback must not publish a second Closed entry.

The current public fixture has two limits. `VisualTestContext::simulate_frame` borrows App
to resolve the platform window before invoking its real callback, so it cannot inject callbacks
while App is already borrowed to exercise queued/coalesced or draw-reentrant delivery. Those
scenarios need a retained real test-platform callback handle obtained before the busy update.
Inactive throttling uses the production wall clock and fixed 33,333-microsecond interval;
the throttle scenario delivers its second request immediately, but deterministic qualification
under scheduler stalls needs a controllable frame clock or inactive interval in test support.
The throttle request uses `force_render: true` and `require_presentation: false`, preserving
the official presentation bypass. The callback itself does not request a refresh, so the
activation/default-request pass checks force-hint retention independently of callback execution.

```text
cargo nextest run -p open-gpui --features test-support,profiler --test frame_signals --test-threads=1 --no-fail-fast --no-tests fail
```

These are test-platform runtime scenarios and independent public API/value controls, not native
frame-source timing, reentrant backend callback delivery, compositor behavior, GPU output,
native `Submitted` receipts, thermal-state transitions, performance or disabled-path codegen
qualification. The commands are verification instructions; this section records no compilation
or execution result. These controls add no fixture dependencies.

## Demand-driven frame scheduling consumer

The `frame_scheduling` integration target consumes demand-driven scheduling from official Zed
[`eb354c8d50`](https://github.com/zed-industries/zed/commit/eb354c8d504071bdb79110a7a5c9d374c2864113),
as present in the fixed cycle source
[`635026fabc`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012).
The fork keeps its existing presentation outcomes and exact accepted/submitted generation facts.
It replaces `PlatformWindow::completed_frame` with `schedule_frame`, while native frame
maintenance remains a separate backend responsibility. `schedule_frame` defaults to a no-op;
`frame_waker` defaults to `None` and returns `Option<Rc<dyn Fn()>>` when a backend supplies one.
A supplied waker must be safe to call while App is borrowed, use the existing backend callback
path without borrowing App or drawing, and avoid retaining the owning Window or App.

This independent target keeps the consumer lockfile and ordinary dependency features. It adds
no dependency, fake platform, application owner, or `test-support` requirement. Its explicitly
typed `_backend_type_only` closure is deliberately unexecuted: it consumes `&dyn PlatformWindow`,
the scheduling method, the optional shared waker, and the actual
`Box<dyn Fn(RequestFrameOptions)>` callback signature. `_window_type_only` similarly checks
`Window::on_next_frame`, a callback queued from a callback, `Window::refresh`,
`App::refresh_windows`, and `AsyncApp::refresh`. It destructures every public
`WindowPresentationFacts` and `WindowPresentAttemptFacts` field without constructing identities
or reporting a fabricated presentation. These closures establish compilation contracts only;
they do not execute the platform defaults or prove callback delivery and effect flushing.

Actual weak-backend release is verified through the framework's existing TestWindow owner, rather
than a consumer-owned closure that exercises only standard-library reference counting. Run the
signature target in both instrumentation configurations, separately, with the existing lockfile:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test frame_scheduling --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features profiler --test frame_scheduling --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

The separate crate-external `crates/gpui/tests/frame_scheduling.rs` target requires
`test-support` and runs with profiling either disabled or enabled. It uses the existing
TestPlatform through public `TestAppContext` and `VisualTestContext`, activates actual test
windows, and draws through `VisualTestContext::update` without borrowing the root view across
the draw. It defers the next window's frame requests before opening it, so hidden initial
presentation cannot consume the startup ticks automatically. The first scheduled tick submits
the accepted scene; the second delivers its compositor callback and parks the source while
preserving the initial exact presentation facts. `simulate_scheduled_frame` delivers only
existing backend demand; it must return
`false` after parking. A successful test-platform `Submitted` leaves one compositor callback,
whose delivery must park without another render or presentation. The workflows cover:

- Root notification and window refresh waking a parked source, followed by submission of the
  exact scene already drawn by test-mode effect flushing.
- `AsyncApp::refresh` flushing immediately before it returns, drawing both parked windows,
  scheduling both sources, and eventually submitting each accepted generation.
- Noninvalidating next-frame callbacks waking a parked source and queuing a callback mid-frame
  for a separate scheduled turn. Neither callback requests a new draw or submission; complete
  presentation facts and render counts must stay unchanged before the source parks again.
- Repeated `Deferred` and `Rejected` renderer outcomes retaining scheduled demand and the same
  accepted scene without advancing submitted/nonempty generations. Restoring `Submitted` must
  submit that exact generation and then park after its compositor callback.
- A real image-paint candidate refused by a controlled atlas lease failure. The existing public
  test-support atlas setter installs that CPU test atlas; refusal preserves every presentation
  fact and keeps scheduling retries. Allowing the lease must accept and submit the next exact
  generation. This tests candidate refusal rather than GPU residency or a renderer failure.
- Logical close releasing the queued callback's captures and root entity before a held native
  terminal callback is released. A later application refresh and terminal delivery must not
  redraw or restore that window.

```text
cargo nextest run -p open-gpui --no-default-features --features test-support --test frame_scheduling --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run -p open-gpui --no-default-features --features test-support,profiler --test frame_scheduling --test-threads=1 --no-fail-fast --no-tests fail
```

These framework scenarios qualify TestPlatform scheduling and its simulated compositor callback,
not native compositor commits, Wayland fullscreen behavior, GPU output, actual native
`Submitted` receipts, physical refresh timing, thermal throttling, or performance. The public
scheduled-frame helpers resolve their window by borrowing App; injecting a callback during a
busy App update therefore needs a retained real platform callback handle obtained beforehand.
This target does not expose an engine hook or claim busy/reentrant callback qualification.
Root/callback release across logical close is observable here; backend waker detachment has
separate owner coverage. App-owned initial-presentation tasks can retain backend state after
logical close; a backend weak-state probe need not expire until App itself is dropped. Retaining
or invoking a weak frame waker must not increase that state's strong owner count or reschedule
a terminal window. The commands above are verification instructions, not a record that
these targets have been compiled or executed.

## Refresh-aware measurement v3 consumers

The `refresh_measurements` integration target consumes the public frame-pacing and foreground
measurement successor from official Zed
[`4f6d97b9de`](https://github.com/zed-industries/zed/commit/4f6d97b9de7d9f167da8b7832eeecea22c5a9efa),
as present in the fixed cycle source
[`635026fabc`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012).
The measurement version is `3`. A frame's work starts at the later of its captured signal and
first invalidation and ends at the start of platform submission. Its lateness budget uses the
display refresh interval with the 120 Hz floor, or the 60 Hz fallback when the display interval
is unknown. Missed refreshes use the real interval. A missing frame signal leaves work
unmeasured even when the display interval is known; an unknown display interval can still have
measured work when a signal is available, but has no missed-refresh count.

This independent fixture keeps ordinary dependency features and its existing lockfile. It adds
no dependency or `test-support` requirement. Its timing-value controls call public work,
budget, lateness, and missed-refresh methods, including equality boundaries and the faster
display budget floor. Owned `WindowProfiler` draw scopes exercise per-refresh snapshots and
`RefreshIntervalFrames::since` without fabricating successful presentations. Public histogram
controls check inclusive duration/percentage buckets and the final overflow bucket. View values
pass through `ForegroundEvent::View`, `IntervalSealer`, threshold detection, and v3 conversion
with type names, phase names, nesting depth, and exclusive self time. The migrated
`foreground_journal` control retains occupancy, conversion, contributor caps, and nesting
assertions while replacing cumulative `Budget` qualification with single-event `BlockingWork`.
Disabled-feature controls execute nested public task guards and verify that timing snapshots,
collectors, aggregate accessors, and trace controls remain empty or no-op.

The fork's raw `InputActivity` controls describe existing dispatch lifecycle for bounded active-
time accounting, including nested input and unwind. The independent consumer supplies those
public values to the sealer and verifies that they create neither work nor a sealed interval.
Completed input events retain work/latency meaning. Actual guard lifetime, chronological union
accounting, suspend/loss and native delivery have separate owner and platform coverage.

The explicitly named `_window_type_only` and `_active_time_type_only` closures are deliberately
unexecuted. They consume actual Window/report signatures and detector active-time access,
including every per-refresh field. Their bodies are compilation contracts, not evidence of
window drawing, active use, or submission. Synthetic `FrameTiming` values use an ID only to
test arithmetic; they never construct presentation facts, successful outcomes, or submitted
generations. The separate existing `profiler` target preserves its input snapshot diagnostics
and ordinary trace/aggregate controls.

Run the independent targets separately in both instrumentation configurations:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test refresh_measurements --test profiler --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features profiler --test refresh_measurements --test profiler --test foreground_journal --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

The separate crate-external `crates/gpui/tests/refresh_measurements.rs` target uses the existing
TestPlatform through public `TestAppContext` and `VisualTestContext`. It defers startup frame
requests, activates each window, and prepares the initial scene through visual-context updates
without borrowing the typed root across a draw. It then exercises these actual workflows:

- A public next-frame callback dispatches mouse input through a rendered handler, changes the
  scene, and reaches draw, platform submission, journal collection, hang detection, v3
  serialization, and per-refresh reports. The display changes between 60 Hz, 240 Hz, unknown,
  and 30 Hz through public display helpers. Input diagnostics, all per-refresh counters,
  histograms, buckets, and exact accepted/submitted/nonempty generations are checked while
  tracing is disabled. Captured signals are explicit `LocalSchedule` simulation values.
- Scheduled callbacks with no captured timestamp submit real accepted scenes while preserving
  unmeasured frame counts and empty work histograms. A measured no-render skip followed by a
  direct draw must also remain unmeasured; the skipped callback cannot lend its signal to
  later work. A compositor callback parks without adding another frame sample.
- Deferred and rejected attempts retain the accepted scene and pending input. Changing the
  display refresh rate during retry cannot alter that scene's original refresh interval,
  signal, draw timestamps, invalidations, or phase durations. The eventual test-platform
  submission must complete the exact accepted generation once, with outcome counters intact.
- Slow nested real views become named v3 contributors and exclude child work from parent
  self time. A measured slow frame produces `LateFrame`; an unmeasured frame with one slow
  input handler produces `BlockingWork`. Deliberate input marks these detector incidents as
  occurring during active use. Draw contributors carry all three phase durations.

```text
cargo nextest run -p open-gpui --no-default-features --features test-support,profiler --test refresh_measurements --test-threads=1 --no-fail-fast --no-tests fail
```

These commands are verification instructions, not an execution or compilation record. Runtime
coverage is TestPlatform callback delivery and its own submitted scenes, not native frame-source
capture, native submitted receipts, compositor timing, GPU output, display scanout, or
performance. Short sleeps make selected real view/input work observable; exact lateness for
ordinary frames is checked against their captured timing rather than a wall-clock speed claim.
Scheduled helpers do not capture a timestamp or accept custom signal options; explicit
`simulate_frame(RequestFrameOptions)` delivers captured simulation signals through the same
public test-platform callback. Busy/reentrant backend delivery and native timing need their
own owner verification. This fixture does not start an App or monitor thread.

## Power modes and configurable cadence consumers

The `power_cadence` targets consume the complete official power and cadence successor from Zed
[`fdcb1981103408e63fff5ef47c8ae2f9995262ad`](https://github.com/zed-industries/zed/commit/fdcb1981103408e63fff5ef47c8ae2f9995262ad),
[`2580a00264a799d76401f12bde3f9ade219212c4`](https://github.com/zed-industries/zed/commit/2580a00264a799d76401f12bde3f9ade219212c4),
and
[`7c8dbadcb5c39270eba8d45bc040fd626684a748`](https://github.com/zed-industries/zed/commit/7c8dbadcb5c39270eba8d45bc040fd626684a748),
as present in the fixed cycle source
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012).
Inactive windows without sustained high-rate input, Low Power Mode, and serious or critical
thermal pressure contribute enabled intervals; the longest applicable interval wins. Defaults
are 33,333, 33,333, and 16,667 microseconds respectively. Each `WindowOptions` field accepts
`None` to disable its own limit. Required platform presentations bypass the limits. The owner
module tests cover the pacer's one-millisecond early tolerance, bounded quarter-interval
deadline correction, restart after stalls, and demand accounting; the public consumers below
do not make precise frame-time assertions.

This independently locked fixture uses ordinary dependency features and adds no dependency or
`test-support` requirement. Its `power_cadence` target executes enum and option-value assertions,
including all four `PowerMode` variants, the exact defaults, custom intervals, and independent
disabling of each limit. With `profiler`, it also checks the distinct public low-power skip
reason while preserving the existing skip-reason assertions in `frame_signals`.
The deliberately unexecuted `_platform_type_only` and `_app_type_only` closures consume the
actual object-safe Platform query, `Arc<dyn Fn() + Send + Sync>` notification callback, App
query, and App observer returning `Subscription`. Those closures qualify signatures only;
they do not deliver notifications, test subscription replacement, or start an App or Window.

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test power_cadence --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features profiler --test power_cadence --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

The separate `crates/gpui/tests/power_cadence.rs` target exercises real TestPlatform workflows
through public App, Window, `TestAppContext`, and `VisualTestContext` APIs:

- Mode changes update the query before any observer is registered; retained observers read
  the committed mode, independent subscriptions survive another subscription's drop, dropped
  callbacks release their captures, and detached subscriptions remain active.
- Two actual windows receive different creation options. Under the same inactive, low-power,
  and critical-thermal conditions, each individual enabled limit defers callback demand while
  the window with all limits disabled runs immediately. Required presentation releases the
  capped callback, and the simulated compositor callback parks the source afterward.
- An active window retains a deferred force-render request in Low Power Mode. Each non-low
  mode releases it without another force flag or an App observer. A separately accepted scene
  with only pending-presentation demand is also paced, then submitted by a required frame
  without drawing again.
- A next-frame callback dispatches real mouse input through the rendered handler and queues
  another callback. Throttling preserves their captures and input state. Deferred and rejected
  renderer attempts preserve the exact accepted scene and previous submitted generations;
  leaving Low Power Mode submits that same scene, releases captures, and retains the handler
  for later platform input.
- Closing a throttled window releases its queued captures and root before a held native
  terminal callback. The App's power query and observer and the other window continue to work;
  releasing the late terminal callback cannot revive the closed window.
- With `profiler`, actual deferred callbacks emit the effective limit's skip reason. Each
  limit wins when its interval is longest, with the inactive reason winning an equal tie.

```text
cargo nextest run -p open-gpui --no-default-features --features test-support --test power_cadence --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run -p open-gpui --no-default-features --features test-support,profiler --test power_cadence --test-threads=1 --no-fail-fast --no-tests fail
```

These commands are verification instructions, not an execution or compilation record. The frame
pump uses scheduler `Instant::now()` (the native monotonic clock); advancing the executor's
`TestClock` does not advance it. The public tests deliberately use hour-scale intervals and
immediate callback delivery, with no sleeps or assertions about physical frame timing. Exact
pacer deadlines and high-rate input thresholds belong to owner-module tests. Presentation
generations and outcomes are read from real test-platform draws and submissions, never
constructed as successful receipts. Native power notification sources, physical mode changes,
native compositor timing, GPU output, energy savings, and performance remain unqualified by
these consumers.

## Generic View and component consumers

The `view_components` targets consume the official generic View/ViewElement unit from Zed
[`74b5207744ef95505fd75c56bf2c9d4736e931ee`](https://github.com/zed-industries/zed/commit/74b5207744ef95505fd75c56bf2c9d4736e931ee),
including its final-target changes through
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012).
`Render` and `RenderOnce` share the public `View` contract and embed through `ViewElement`.
The derive supports both `RenderOnce` components and ordinary manual `View` implementations,
preserves generic parameters and where clauses, and retains the conversion caller's source
location. Only `Entity<T: Render>` and `AnyView` expose cached embedding; a stateless or manual
View has no public frozen-cache shortcut.

This independently locked fixture uses ordinary dependency features without `test-support` or
new dependencies. Its `view_components` target constructs a generic stateless component and a
generic manual enum, converts both to their actual `ViewElement` types, and checks stateless
identity and debug source locations. The deliberately unexecuted `_composition_type_only` and
`_root_type_only` closures consume manual backing identity, direct `View::render`, public local
state, typed/erased/weak conversion, entity-backed cached return types, pointer/keyboard/canvas
composition, and the typed root creation signature. Those closures establish compilation
contracts only; they do not run an App, render a frame, dispatch input, or prove cache reuse.

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test view_components --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features profiler --test view_components --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

The separate crate-external `crates/gpui/tests/view_components.rs` target exercises actual
TestPlatform windows through public `TestAppContext` and `VisualTestContext` APIs:

- Two instantiations of one generic `RenderOnce` component use the same local-state call site
  with distinct type scopes. Pointer and keyboard input mutate exact independent counts, and
  custom canvas painting reads the resulting values. Changed initialization props preserve
  existing state; removal and reconstruction release and recreate it exactly once.
- Manual props-bearing Views use ordinary data entities as backing identities. Parent props
  updates and sibling reordering preserve each entity and local state, callbacks use the new
  props, and removal/reconstruction resets only local state. Closing and dropping the remaining
  entity owners checks exact release counts.
- Typed, erased, and upgraded weak children render in a real typed root. Backing notifications
  rebuild the canvas, and root replacement and close check weak release. Actual test-platform
  accepted/submitted generations and valid primitives come from each window's presentation facts.
- `Entity<RenderedLeaf>::cached` and `AnyView::cached` embed two independent entities in one
  window. Parent notifications reuse both caches; notifying each backing entity rebuilds only
  its own subtree and paints its new value. A window refresh rebuilds both, and closing releases
  both entity owners exactly once.
- Explicit typed, erased, cached and upgraded-weak conversions retain the caller's file/line in
  debug builds and omit it in release builds. This is a fork diagnostic adaptation to the target's
  internal conversion locations. The manual props workflow directly invokes the derive's
  into_any_element override; generic child composition invokes into_element.
- Typed entities, erased downcasts, and upgraded weak handles also become actual window roots.
  Closing and reopening windows retains the caller-owned backing entity, while final owner
  release and reconstruction check the new identity, rendered values, and exact release count.

```text
cargo nextest run -p open-gpui --no-default-features --features test-support --test view_components --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run -p open-gpui --no-default-features --features test-support,profiler --test view_components --test-threads=1 --no-fail-fast --no-tests fail
```

These commands are verification instructions, not an execution or compilation record. Framework
tests activate their windows, disable creation-time frame limits, and draw through the visual
context without holding a typed root borrow. Their submission facts belong to TestPlatform;
native compositor delivery, GPU output, foreign-platform execution, hot reload, and measured
performance require separate qualification.

## Measured text queries and rem units

The ordinary `measured_text` target constructs public StyledText/TextLayout values and checks query
signatures without enabling test-support. Its `_measured_queries_type_only` closure is deliberately
unexecuted. The separate WGPU `measured_text` consumer executes measurement before prepaint,
source-order and UTF-8/newline/wrap queries, then paints the same StyledText using Cosmic and Swash.
It also exercises notification, remeasurement, source updates, refresh, reconstruction and retained
layout lifetime. That is software/TestPlatform qualification, not native GPU presentation.

The ordinary `rems` target exercises ZERO, arithmetic and style storage. Its pixel-conversion
closure is typechecked only. The separate Core public `rems` target executes current Window rem
size changes, two-window isolation and actual committed layout dimensions. Rems conversion stores
a value using the current rem size; it does not introduce another reactive scale owner.

```text
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all-features --test measured_text --test rems --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked -p open-gpui --all-features --test rems --test frame_scheduling --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked -p open-gpui-wgpu --all-features --test measured_text --test-threads=1 --no-fail-fast --no-tests fail
```

These commands describe the intended gates; actual results and limits are recorded in the
[query/List qualification](../../../docs/verification.md#measured-text-queries-rem-units-and-list-autoscroll).

## Synthetic accessibility and explicit tree inspection consumer

The `accessibility_tree` integration target consumes `A11ySubtreeBuilder`, the custom
`Element::a11y_synthetic_children` hook, the `Stateful<Div>` callback, and
`Window::debug_a11y_tree_json(&self) -> Option<String>` through the independent consumer's
existing dependency graph. AccessKit types come from `open_gpui::accesskit`; this target adds no
direct AccessKit dependency and does not enable `test-support`.

Both fixture tests are explicitly type-only. Their custom element and deliberately unexecuted
closures check parent-scoped hash keys, `NodeId` results, Boolean insertion results, mutable parent
nodes, prepared text runs and selections, an owned `FnOnce + 'static` capture, and inspection
through a shared Window reference. They do not start an App, activate accessibility, invoke the
synthetic hook, publish a tree, or execute the inspection closure.

The separate crate-external framework target uses the existing `TestAppContext` and
`VisualTestContext` activation, final platform tree history, and real accessibility action ingress.
It exercises the custom Element and Div/Stateful hook delegation, UTF-8 text runs and relations,
source order, parent metadata, release-build duplicate rejection, stable IDs, notification-driven
changes, deactivation/reactivation, root replacement/reconstruction, and weak owner release. Its JSON checks
compare the complete exported node set and child order with the actual accepted platform tree;
repeated inspection must not render, activate accessibility, schedule frames or publish updates.
Creator provenance is required only with `cfg(debug_assertions)` and absent in release builds.
Additional public controls exercise invalid own-overflow clips, late paint invalidation and
irreversible quiescence during accepted publication. Complete parent/deferred rollback and
actual atlas refusal/recovery retain their existing internal owners and separate evidence.

The JSON schema follows official Zed
[`2268045a11`](https://github.com/zed-industries/zed/commit/2268045a11), within the cycle target
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012),
with accepted `frame.frame_generation` and activation `frame.activation_generation` added.
`frame.frame_number` counts captured publications. This API is an explicitly requested raw developer
export, including accessible text and window title. It returns `None` before publication and after
deactivation or close. Automatic announcement diagnostics and DevTools captures retain their
separate redacted contracts.

Run the independent signature target and the framework runtime target separately, reusing the
existing lockfiles and root build directory, with one Cargo process at a time:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test accessibility_tree --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run -p open-gpui --features test-support --test accessibility_tree --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run -p open-gpui --release --features test-support --test accessibility_tree --test-threads=1 --no-fail-fast --no-tests fail
```

These commands are verification instructions, not execution results. The framework runtime tests
use TestPlatform and do not establish native screen-reader behavior or native GPU presentation.
Actual results and configuration limits belong to the
[qualification record](../../../docs/verification.md#synthetic-accessibility-and-explicit-published-tree-inspection).

## Composite accessibility focus and keyboard continuity consumers

The `composite_focus` targets consume the composite focus mechanisms from official Zed
[`83d4847462`](https://github.com/zed-industries/zed/commit/83d4847462) and
[`2268045a11`](https://github.com/zed-industries/zed/commit/2268045a11), as present in the fixed
source [`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012).
`StatefulInteractiveElement::aria_active_descendant()` is declared on the selected descendant
of a composite widget. Its container retains actual keyboard focus. The final AccessKit
`TreeUpdate.focus` follows the selected descendant only while a proper ancestor is actually
focused. Explicit inspection preserves this distinction through `gpui_focus` and
`active_descendant_focus`.

This independently locked fixture enables no `test-support` feature and adds no dependency.
Both fixture tests are explicitly type-only: their deliberately unexecuted closures check
composition, the no-argument fluent method, synthetic child callbacks, ordinary key handlers,
the targeted keyboard click event, focus manipulation, container tab ordering, and shared
Window inspection. AccessKit types come from `open_gpui::accesskit`. These checks do not start
an App, deliver input, publish an accessibility tree, or qualify runtime behavior.

The separate crate-external `crates/gpui/tests/composite_focus.rs` target uses real TestPlatform
windows through public `TestAppContext` and `VisualTestContext` APIs:

- A menu trigger opens a composite menu whose actual keyboard FocusHandle stays on its
  container. Arrow actions and notifications from a separately owned source entity change
  the selected descendant. Final platform tree focus and inspection JSON agree, stable item
  identities persist, and synthetic detail children remain children of the selected items.
- An outside control retains actual and accessibility focus while the inactive menu changes
  its declaration. Returning focus to the menu restores its current selected descendant.
  Removing all items clears the descendant focus, and rebuilding restores navigation.
- An explicit `track_accessibility_focus` display override does not authorize a selected
  descendant without real input ancestry. With no actual input focus, or with input focus on
  an outside control, the override keeps its own display focus and the descendant is ignored.
- Enter and Space share the official activation arm and require an uninterrupted activation
  keydown/keyup pair. Orphan releases, intervening other key events, and an actual focus ABA
  do not click. Reasserting the
  same focus handle still permits a clean click. A real menu accept action closes on keydown
  and returns focus to its trigger; the subsequent keyup cannot reopen the menu.
- Tab from the second non-tab-stop group container enters that group's first child and then
  advances to its second child. Root replacement and window close release exact weak owners,
  and retained source notifications cannot render the released root.

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test composite_focus --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run -p open-gpui --no-default-features --features test-support --test composite_focus --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run -p open-gpui --release --no-default-features --features test-support --test composite_focus --test-threads=1 --no-fail-fast --no-tests fail
```

These commands are verification instructions, not execution or compilation results. Run one
Cargo process at a time and reuse `target`. The runtime target qualifies public consumers on
TestPlatform; native screen-reader behavior, native GPU presentation, rejected candidates,
atlas refusal, and late geometry invalidation require their owning verification.

## Accessibility property and configuration consumer

The `accessibility_configuration` target consumes the public shortcut, numeric-step and
author-identifier builders, plus the enabled-versus-active Window queries, against official Zed
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012).
Its two tests contain deliberately unexecuted `_type_only` closures. They type-check spinbutton
and menu composition with `aria_keyshortcuts`, `aria_numeric_value_step` and `accessibility_id`,
including borrowed strings, owned strings and `SharedString`. A composite consumer gates its
accessibility-only source subscription with `Window::is_a11y_enabled()`, independently of
`Window::is_a11y_active()`. This fixture adds no dependency, enables no `test-support`, constructs
no App or Window, and does not publish or inspect a live accessibility tree.

The author identifier is distinct from GPUI's internal element ID. It is visible outside the
process through supported AccessKit platform adapters; use stable identifiers unique within the
accessibility tree. Keyboard shortcuts describe existing bindings and do not install a keymap.
The enabled query describes configuration, including force-disabled windows, while the active
query describes the current frame's accessibility work. Gate subscriptions by enabled and gate
render-time semantic computation by active, so attaching assistive technology can request a
fresh tree without missing earlier source or focus notifications.

The separate framework `crates/gpui/tests/accessibility_configuration.rs` runtime target uses
public `TestAppContext` and `VisualTestContext` APIs to cover:

- Source-notified spinbutton and menu property replacement and removal, exact complete author
  identifiers and shortcut strings, numeric steps, stable distinct node/element identities,
  removed menu children, and agreement between the final platform tree and inspection JSON.
- Passive enabled/active and inspection queries before attachment, after accepted publication
  and after deactivation, with unchanged rendering, scheduling, focus and publication history.
- Real composite focus and source subscriptions registered before attachment, source selection
  updates while inactive, repeated activation/deactivation and full-refresh requests, and
  selected-descendant focus with stable item identities.
- `TestAppContext::disable_accessibility()` affecting subsequently created windows only;
  earlier windows remain eligible for activation, while disabled menus never register
  accessibility subscriptions, select, activate or publish, including after root replacement.
- Root replacement, reconstruction and window removal releasing exact weak view owners and
  preventing further deliveries from a retained source entity.

Run the independent type checks and framework runtime target separately with the existing
lockfiles, one Cargo process at a time, reusing `target`:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test accessibility_configuration --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run -p open-gpui --no-default-features --features test-support --test accessibility_configuration --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run -p open-gpui --release --no-default-features --features test-support --test accessibility_configuration --test-threads=1 --no-fail-fast --no-tests fail
```

These commands are verification instructions, not compilation or execution results. The runtime
target inspects the latest complete accepted tree after the public activation helper drains its
work; it does not assert the synchronous initial callback's root-only title snapshot or its
sequenced refresh. Native initial callbacks, actual screen readers, native `Submitted` frames,
GPU output and platform-specific identifier exposure require their separate owners and gates.

## Maintained benchmark harness consumer

The `maintained_harness` benchmark binary uses the public benchmark implementation from the fixed
Zed source
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012),
adapted through Open GPUI's platform and presentation owners. Enable this fixture's `bench-support`
feature to forward Core and facade benchmark support and activate Criterion 0.8.2 and rand 0.9.
This feature is independent of `test-support`; the consumer does not use `FakeCounter`, test
contexts, virtual timers or a no-op text system.

The binary executes its workloads through `#[open_gpui::bench]`, a
`Criterion<BenchMeasurement>` group, and `bench_main!` with the default `CountingAllocator`:

- `simple_native_shaping` calls `bench_text_system()` and measures actual native `layout_line`
  calls for multilingual text. After execution it checks UTF-8 length, positive native metrics,
  character-boundary glyph indices and nonempty glyph output.
- `maintained-renderer/rows/{4,16}` constructs source-observed, labeled views outside timing and
  measures source updates through `bench_renderer`. Executed render/paint counters and the final
  painted source value must agree. Accepted frame generations advance, and the final submitted
  generation and nonempty presentation facts must match the accepted frame. Closing the window
  and explicitly dropping the window context, view and source releases their weak owners once.
- `maintained-seeded-tasks/items/{64,256}/seed-{7,23}` puts the by-value `StdRng` between the input
  and context arguments. Each Criterion routine checks the fresh RNG stream against the selected
  seeds, clones input in untimed `bench_batched_task` setup, sorts on the background executor, and
  verifies completed output in its destructor after timing and tracing finish. The next iteration
  requires the preceding output to have been released.
- `seeded_task/seed-11` puts `StdRng` first and executes a background-to-foreground `bench_task`
  workflow with verified completion and released task captures.
- `module_cases::public_routine` registers a benchmark from a child module through its preserved
  restricted visibility, proving that reusable benchmark modules can join the public group.

Every executed Criterion routine installs fresh App state. A weak owner and exact release count
are checked before the next routine and after each generated benchmark returns, including the
last benchmark. Local handles and context clones are dropped before the macro's teardown. Filters
and `--list` can skip workloads; listing or compiling the binary is not runtime qualification.
`SEED` and `ITERATIONS` select reproducible seed cases using the same rules as the benchmark macro.

The Core crate-external `benchmark_harness` target is gated solely on `bench-support`. It executes
short Criterion measurements through raw public APIs and checks source notifications, resize and
frame callbacks, root reconstruction, real timer deadlines, an external callback waking a Task,
untimed batched input/output destruction, metric samples, App/TextSystem lifetime, timer cleanup
and the next routine's isolation on the shared dispatcher. Finite renderer sessions use fresh
untimed setup per iteration and require the final source value to be accepted and submitted before
session destruction. Those Core windows use `NoopTextSystem` without a GPU renderer; native text
shaping belongs to this independent binary. Candidate refusal and native GPU output retain their
separate verification owners.

Run the focused public tests, the independently locked binary's execution smoke check, and an
actual measurement separately, one Cargo process at a time, reusing the existing `target`:

```text
cargo nextest run --locked -p open-gpui --no-default-features --features bench-support --test benchmark_harness --test-threads=1 --no-fail-fast --no-tests fail
cargo bench --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features bench-support --bench maintained_harness --jobs 1 --target-dir target -- --test
cargo bench --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features bench-support --bench maintained_harness --jobs 1 --target-dir target
cargo bench --locked -p open-gpui-canvas --features bench-support --bench large_canvas --jobs 1 -- paint_frame_render_labels --test
```

Set `BENCH_MEASUREMENT=wall-time` when the smoke check should omit hardware counters; the default
measurement reports supported counters and allocator observations. Measurement selection and
`bench_main!(allocator = none; ...)` are owned by the public harness, rather than a consumer-local
measurement or allocator shim. The Canvas consumer preserves its 200-by-100 labeled grid,
viewport and culling options while moving only setup-excluded notification/render measurement to
the maintained context and native text factory.

These commands are verification instructions, not compilation, execution or performance results.
The consumer's presentation assertions describe the benchmark platform's accepted/submitted facts.
GPU rendering, OS-native window delivery, cross-platform qualification and rejected-candidate
controls require their owning checks and recorded execution evidence.

## Shared atlas ownership runtime consumers

The `atlas` targets consume the owned-key insertion API and shared atlas owners introduced by
official Zed `0ef92145ff87aab5f19f6b44c9c55a486f4f064f`, as present in the fixed source
`635026fabc38440fce5e1767d125026a17b12012`. Insertions transfer an `AtlasKey`; tests clone a key
only when retaining it for a later lookup or removal. Existing exact texture-instance leases,
generation fields and shader padding remain part of the fork's contract.

The crate-external `crates/gpui/tests/atlas.rs` consumer implements `AtlasBackend` with a concrete
CPU texture owner and consumes `AtlasState` through public exports. It executes unavailable and
decoder-error paths, rejects a short pixel buffer in the actual custom backend, retries the same
unpublished key, and verifies independently owned image bytes after the caller changes its buffer.
Cache hits skip decoding and upload. Monochrome and polychrome allocations share the typed backend.
Removing the image releases exactly its allocation while preserving the live icon; the default
backend removal diagnostics report the removed texture. A typed reset callback releases the icon,
and a reconstructed allocation is released when the state drops.

This independently locked fixture's `tests/atlas.rs` executes `HeadlessAtlas::default()` through
both the concrete owner and `Arc<dyn PlatformAtlas>`. Removing a key while leased retains its exact
instance for further acquisition. Reinserting the same key creates a distinct instance. Dropping
one lease preserves the other holders; after the final actual old lease drops, acquisition of the
old instance fails with `TextureUnavailable`, while the replacement remains cached and retainable.
Removing the replacement after its lease drops frees that replacement's texture.

Both targets use ordinary `#[test]` functions and public APIs. The fixture enables no
`test-support`, adds no dependencies, and uses inferred API results with `expect` rather than
requiring a direct `anyhow` dependency. Run the focused targets from the repository root, with
`CARGO_BUILD_JOBS=1`, one Cargo process at a time, reusing `target` and the existing lockfiles:

```text
cargo nextest run --locked -p open-gpui --no-default-features --test atlas --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked -p open-gpui --all-features --test atlas --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --test atlas --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all-features --test atlas --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

These commands are verification instructions, not compilation or execution results. The feature
off/on fixture commands exercise the ordinary consumer's existing feature graph without adding
`test-support`. These are CPU backend and headless lifetime checks; they construct no App or Window,
upload no GPU pixels, and establish neither native presentation, scanout, renderer submission nor
performance. The separately ported official owner regressions and real renderer/resource tests
retain their own verification scope.

## Application-owned AVIF encoding opt-in

The consumer's optional `image` dependency requests version `0.25.4` with
`default-features = false` and only `features = ["avif"]`. Its `avif-encoding` feature enables
that dependency through `dep:image`; no GPUI feature or wrapper is involved. Other image
formats remain governed by the workspace dependency policy. This follows the encoding opt-in
boundary from official Zed `8e06c323ae063f3d245ea5c283d4bf373ad51db2`, retained in the fixed
source `635026fabc38440fce5e1767d125026a17b12012`.

With `avif-encoding` enabled, the ordinary `images` test
`downstream_opts_into_avif_encoding_alongside_gpui_decoding` executes public
`Image::from_bytes` and `Image::to_image_data` to decode the existing animated WebP fixture.
It converts the first one-pixel frame from BGRA to RGBA and actually encodes it with the public
`image::codecs::avif::AvifEncoder`. It uses speed 10 and `with_num_threads(Some(1))`, checks
that output is nonempty, and requires `image::guess_format` to identify the bytes as AVIF.
The existing JPEG, animated WebP and malformed/truncated-image tests remain enabled in both
feature configurations; only the additional encoder test is gated by `avif-encoding`.

Run these commands from the repository root with `CARGO_BUILD_JOBS=1` and one Cargo process at a time.
The first command runs the image tests with the encoding opt-in off; the second enables it.
Both reuse the repository's `target` directory:

```text
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --test images --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features avif-encoding --test images --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

Inspect the corresponding image feature requests separately:

```text
cargo tree --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --edges features --invert image
cargo tree --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features avif-encoding --edges features --invert image
```

With the workspace's default-minus-AVIF image configuration, the off graph must omit the
`image` crate's `avif` feature, and the on graph must show that feature requested by the
consumer. The fixture's `--all-features` gate also includes this opt-in.

The [qualification](../../../docs/verification.md#image-dependency-feature-policy) records
executed tests and compiler scope separately from these reusable commands. This consumer
enables no `test-support` or `avif-native` feature and uses no engine-private API.
Encoding and format detection do not establish AVIF decoding or a decoded AVIF round trip.
The test constructs no App or Window and does not qualify native image loading, rendering,
GPU presentation, cross-platform runtime behavior or performance.

## Application-owned GPU and native Vulkan controls

The `wgpu_atlas` target runs with `wgpu-renderer` and no GPUI test support. It creates a real
application-owned adapter, device and queue through the direct WGPU dependency, then passes
those public types to `WgpuAtlas::new`. It exercises malformed-source refusal, a complete
same-key retry, actual upload and bounded queue completion, public texture leasing and final
backend-owner release. This proves the device/atlas boundary rather than full Nara renderer
or native Window integration.

The application-owned `vulkan-surface` feature enables `wgpu/vulkan` and `wgpu/wgsl`.
Its Windows `vulkan_surface` tests create and retire an actual HWND and Vulkan surface while
an acquired texture remains alive, with and without submitted clear work. They are ignored
in generic runs because they require a real presenting Vulkan adapter and desktop. Missing
support fails explicit execution; it is not silently treated as success. Validation scopes
and uncaptured errors remain checked. The tests do not assume a minimum GPU execution time
or simulate physical device loss.

Combining `vulkan-surface,wgpu-renderer` adds an ordinary public `WgpuRenderer` trajectory.
It creates the first real HWND surface/context with a zero requested drawable size, resumes
to a nonempty submitted Scene,
shares the same device/queue with a second real surface, suspends/resumes that surface and
quiesces both exact shutdown tickets before destroying either HWND. It uses borrowed handle
values without an unsafe Send/Sync implementation. This qualifies initialization, sharing and
native retirement on the executed adapter, not the number of configuration calls, hybrid-GPU
selection, compositor pixels or physical device loss.

Run each command serially from the repository root:

```text
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features wgpu-renderer --test wgpu_atlas --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features vulkan-surface --test vulkan_surface --run-ignored only --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features vulkan-surface,wgpu-renderer --test vulkan_surface --run-ignored only --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

The [WGPU qualification](../../../docs/verification.md#wgpu-dependency-shaders-and-webgl)
records executed results, including the registry 30.0.1 failure control and restored fixed
source. Reusable commands are not an execution claim for another host or backend.

## Frame debug overlay public consumers

The `debug_overlay` target consumes the official frame overlay introduced by Zed `07a0bd1220`,
as present in fixed source `635026fabc38440fce5e1767d125026a17b12012`. The existing `profiler`
feature forwards the Core opt-in. With it disabled, this integration target compiles empty;
it adds no dependencies, GPUI test-support, feature or lockfile changes.

The ordinary independently locked consumer executes the public `DebugFrameOverlayMode`
default and Hidden/Minimal/Full cycle. A separately named signature test uses exact function
pointer types and an explicitly unexecuted Window closure to consume
`Window::debug_frame_overlay_mode`, `set_debug_frame_overlay_mode`,
`cycle_debug_frame_overlay_mode` and `reset_debug_frame_overlay_stats`. These Window checks
are type evidence; they do not construct an App or Window or execute those methods.

The separate crate-external `crates/gpui/tests/debug_overlay.rs` target requires both
`test-support` and `profiler`. Its headless consumer executes a real public App/Window workflow
with `NoopTextSystem`, the shared public `HeadlessAtlas` and a custom CPU recording
`PlatformHeadlessRenderer` installed through `HeadlessAppContext::with_platform`.
The renderer clones each complete received `Scene` through public `Scene::clone`.
The test reads actual root quads, translucent panel bounds, viewport clip shapes and green
bitmap glyph runs, reconstructing the displayed text from those quads. It checks mode
appearance/removal, hidden accepted-draw history, reset placeholders with the lifetime count
preserved, repopulated samples, bounded idle callbacks and exact App/view/renderer/atlas
release across window reconstruction at two viewport sizes. The view uses no text elements,
and the captured scenes must contain no font sprites.

A second public runtime test uses `TestAppContext` to execute an actual resize and root
replacement, checking the rendered root's viewport, preserved overlay mode and exact view
release. It also checks that the public mode/reset controls cause automatic redraw through
the test context's dirty-effect flush. Complete panel/glyph Scene inspection belongs to the
headless consumer; this second test does not expose a Scene or attach a renderer.

The configured CPU renderer's presentation facts do not establish GPU execution, actual
native submission, compositor pixels or performance. Pixel capture is explicitly unsupported
by the recorder. The native overlay consumer and owner regressions separately qualify their
own rendering and accepted/refused/unwinding histories.

Run the focused targets from the repository root with `CARGO_BUILD_JOBS=1`, one Cargo process
at a time, reusing `target` and the existing lockfiles:

```text
cargo nextest run --locked -p open-gpui --no-default-features --features test-support,profiler --test debug_overlay --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features profiler --test debug_overlay --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo check --locked -p open-gpui --no-default-features --features test-support --test debug_overlay --jobs 1
cargo check --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --test debug_overlay --target-dir target --jobs 1
```

These commands are verification instructions, not compilation or execution results.
The main verification owner records actual results and host/compiler scope separately.

## Generic animation public consumers

The `animation` target consumes the official generic animation controls in fixed Zed source
`635026fabc38440fce5e1767d125026a17b12012`: App-wide reduced motion, shared-phase repeating
animations, per-animation FPS caps, finite easing overshoot and `ParentElement` composition.
It excludes the subsequent spring APIs and introduces no dependencies, features or lock changes.
These APIs do not require the `profiler` feature.

The ordinary independently locked target uses no GPUI test-support. Its executed tests check
public builder fields and cloning, ordinary versus synced repetition, FPS-cap value storage,
finite overshoot, public easing functions and construction of animated parents with children,
mapping and chains. The parent construction consumes actual element values but does not perform
layout or paint. A separately named signature test checks exact function pointers and an
explicitly unexecuted App/Window closure. That closure is type evidence only; it does not
construct an App or Window or execute their methods.

The separate crate-external `crates/gpui/tests/animation.rs` target requires only `test-support`.
Its real `HeadlessAppContext` consumer mounts cached entity views at different times and in
different windows, compares synced phases against an ordinary independently started loop,
and reconstructs a window without resetting the shared App epoch. Its configured CPU renderer
receives complete Scenes, checks actual animated-parent and appended-child quads, and verifies
accepted-generation and `Submitted` presentation facts. It checks cached sibling reuse, exact
view/App/renderer/atlas release and absence of timer-driven resurrection after window removal.
The recorder does not render pixels and supplies no GPU, native or performance qualification.

Two further public `TestAppContext` workflows use the deterministic executor clock and ordinary
App/View APIs. One checks timer-driven capped animation, overlapping redraw timer deduplication,
retirement with a pending timer and actual callback-driven uncapped animation. The other changes
the App reduced-motion preference after cached views mount, checks oneshot, repeating and chain
static endpoints plus finite overshoot, confirms idle parking, and re-enables motion with the
existing animation clock. Test window options disable unrelated inactive, low-power and thermal
limits; per-animation cadence is exercised through its normal owner.

Run these targets serially with the existing lockfiles and one Cargo process:

```text
cargo nextest run --locked -p open-gpui --no-default-features --features test-support --test animation --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked -p open-gpui --no-default-features --features test-support,profiler --test animation --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --test animation --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features profiler --test animation --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

These commands define the consumer gates. The
[qualification record](../../../docs/verification.md#generic-animation-controls-and-clock)
records executed host/compiler modes, initial harness corrections and native limits.

## Stateful Spring public consumers

The `spring` targets consume the complete official Spring API from fixed Zed source
`635026fabc38440fce5e1767d125026a17b12012`. GPUI retains typed targets, interpolation,
builders, playback and element identity. The pure `SpringConfig`, `SpringState` and
`sampled_easing` implementation belongs to `open-gpui-motion` and is reexported by GPUI.
These tests introduce no features, dependencies or lockfile changes.

The ordinary independently locked target enables no GPUI test-support. Executed tests consume
public solver values across damping regimes, explicit elapsed-time advancement and retargeting,
the propagator, moving-target ramp, velocity-aware settling and undamped nontermination.
The public easing adapter is evaluated and installed in an ordinary duration-animation builder,
including overshoot. Typed target and interpolation tests execute scalar, Pixels, Rems, bool,
AnimationPhase, a consumer-defined target, RGBA interpolation and the shortest HSLA hue path.
Builder tests actually construct and drop Spring parents with appended children, map the
underlying element, and consume a move-only animator capture through normal destruction.
They perform no layout or paint. A separately named signature test contains an explicitly
unexecuted App/Window closure; that closure establishes only public type compatibility.

The crate-external `crates/gpui/tests/spring.rs` target requires only `test-support`. Its ordinary
type-identity test assigns the exact same configuration and state values across the existing
direct motion dependency and GPUI's reexports, compares both easing paths, and feeds that
configuration into the GPUI builder. The independent fixture has no direct motion dependency
and does not claim to establish that cross-package type identity itself.

Real public TestApp workflows mount cached entity views, advance the deterministic executor
clock and deliver scheduled frame callbacks. They check stateful retargeting after elapsed
time under the old target and configuration, preserved velocity, pause/resume without counting
paused time, stop with discarded velocity, cancellation to the first initial value, completion
to the latest target, default mounting at the target and `.from` applying only to new state.
Changing ancestor identity and unmounting/remounting establish fresh state. A reduced-motion
workflow checks updates to mounted cached views, static parking, preference idempotence and
subsequent retargeting from the snapped endpoint. An unchanged sibling must reuse its cache.
Window options disable unrelated inactive, low-power and thermal limits; close/reopen uses
explicit App quit ownership.
An additional real TestApp view runs built-in Pixels and bool targets, preserving overshooting
AnimationPhase output while explicitly clamping its color projection, then snapping both typed
values under reduced motion.

A separate HeadlessAppContext workflow supplies NoopTextSystem, public HeadlessAtlas owners and
a concrete CPU recording renderer. It checks actual typed-target quad widths and interpolated
colors, the mapped parent height and appended-child quads in complete received Scenes. It
compares accepted and Submitted generations through public presentation facts, preserves cached
sibling reuse, reconstructs a window, and checks exact App/view/renderer/atlas release without
post-close animation resurrection. Configured CPU submission facts do not establish GPU
execution, native presentation, scanout or performance. Candidate-refusal, unwind and native
Spring trajectories retain their separate owning tests.

Run the focused targets serially from the repository root with `CARGO_BUILD_JOBS=1`, using the
existing lockfiles and the caller's shared `CARGO_TARGET_DIR`:

```text
cargo +1.95.0 nextest run --locked -p open-gpui --no-default-features --features test-support --test spring --test-threads=1 --no-fail-fast --no-tests fail
cargo +1.95.0 nextest run --locked -p open-gpui --no-default-features --features test-support,profiler --test spring --test-threads=1 --no-fail-fast --no-tests fail
cargo +1.95.0 nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --test spring --test-threads=1 --no-fail-fast --no-tests fail
cargo +1.95.0 nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features profiler --test spring --test-threads=1 --no-fail-fast --no-tests fail
```

These commands define verification instructions, not compilation or execution results. The main
verification owner records actual results and compiler/platform scope separately.

## System notification public consumers

The `system_notifications` targets consume the official notification API in fixed Zed source
`635026fabc38440fce5e1767d125026a17b12012`. They introduce no dependencies, features or lockfile
changes. The ordinary independently locked target enables no GPUI test-support.

Its executed value tests construct owned notifications, action lists and body/action responses,
check cloning and equality, exercise the action's Hash implementation and preserve arbitrary
caller action IDs such as `default` and `__closed`. These are value checks, not transport tests.
A separately named signature test checks exact App function pointers and explicitly unexecuted
App/Platform closures, including owned mutable callbacks and the shared `&App` receiver.
Those closures do not construct an Application, start an App or execute backend notification
methods. The public controls return unit; absence of an error is not a delivery receipt.

The separate crate-external `crates/gpui/tests/system_notifications.rs` requires only
`test-support`. It constructs actual App/Window consumers through public `TestAppContext` and
the existing CPU TestPlatform. Its full workflow sets identity before opening a window,
posts two tags, replaces one without losing the other, delivers a response while the App is
actively borrowed, updates a real entity, observes `Context::notify` and checks redraw.
It reconstructs the root view, routes the next action response to the replacement, dismisses
only the matching tag and verifies exact view/App release. Another workflow replaces a handler
from within its own delivery, enqueues a nested response through the public simulator and checks
ordered delivery plus release of the old and replacement callback captures. The first response
is queued under an active App borrow so the simulator restores its platform callback before
that handler starts and simulates the nested response. These consumers complement the
separately ported official Core cases; they do not duplicate those cases.

The `open-gpui-platform` example adapts the official `system_notifications.rs`, including its
final startup font initialization through the existing checked-in font fixtures on Web. It
sets identity before posting, provides Show or replace and Dismiss controls and updates its view
from body/action callbacks. Its status reports requests, because delivery and dismissal are
best-effort. macOS requires an app bundle and notification authorization. The fixed Linux and
FreeBSD backend requires a session D-Bus service and does not replace or retract delivered
notifications. The Web backend does not deliver system notifications. Native Windows uses its
existing package/AppUserModelID notification path; this example adds no separate installer or
permission UI.

Run the focused gates serially from the repository root with `CARGO_BUILD_JOBS=1`, reusing
the existing lockfiles and build directory:

```text
cargo nextest run --locked -p open-gpui --no-default-features --features test-support --test system_notifications --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --test system_notifications --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo check --locked -p open-gpui-platform --all-features --example system_notifications --jobs 1
```

Launch the native example separately with
`cargo run -p open-gpui-platform --example system_notifications` and the platform features
required on the host. These commands define verification instructions, not execution results.
CPU TestPlatform delivery and request history establish neither actual operating-system
notification delivery, authorization, action routing, native rendering nor performance.
The main verification owner records executed commands and remaining native limits separately.

## Sendable foreground consumers

The `send_foreground` targets consume the official sendable foreground mechanism from Zed
[`2c2f737ab2f3db793b7a313bc8e5307f1708d2e8`](https://github.com/zed-industries/zed/commit/2c2f737ab2f3db793b7a313bc8e5307f1708d2e8),
checked against fixed source
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012).
These test targets require no additional test-only dependencies or feature declarations.
The framework dependency-source changes have their separate
[qualification record](../../../docs/verification.md#sendable-foreground-submission-and-web-fetch).
Both targets use the same test cases with the existing `profiler` feature enabled or disabled;
native thread tests are excluded on Wasm.

The ordinary independently locked target does not enable `test-support`. Its six runtime tests
use `ForegroundExecutor::to_send`, cloned `SendForegroundExecutor` handles and the existing
doc-hidden `PlatformDispatcher` bridge. A small test dispatcher captures the originating thread
and forwards actual runnables through a standard-library channel. Only that thread drains the
channel, using the public `profiler::task_timing` guard around each actual poll. This exercises
the production `PlatformScheduler` dispatch path, without constructing an Application or App,
fabricating a window identity, starting a native event loop or introducing another library
scheduler. Background, timer and realtime dispatch fail explicitly if unexpectedly invoked.

The runtime scenarios cover:

- Worker submission with no eager polling; the first owner poll becomes Pending. A worker wakes
  the original stored waker, the second poll returns a `Cell`, and the registered Task waiter
  is woken. The task and its `Send` but non-`Sync` output are then consumed on a worker. The
  future also captures a `Cell`, so neither the future nor its output requires `Sync`.
- Two distinct captured destinations: a public facade handle and a handle obtained through
  `SchedulerLocalExecutor::to_send` both submit from one worker. Draining the first destination
  completes only its task; the second task remains pending until its own destination is drained.
- The final cloned Send handle retaining both the scheduler and dispatch destination after
  releasing the local executor, then releasing both when dropped on a worker. This case uses
  the existing `SchedulerLocalExecutor::new` constructor with the same dispatcher and a closure
  that does not retain the scheduler, independently testing the Send handle's own retention.
- Worker cancellation before the first poll, followed by owner cleanup without executing the
  future body; the captured drop probe is released exactly once on the owner thread.
- Worker cancellation after a Pending poll, owner cleanup without another user poll, and safe
  use of the saved waker after cancellation without resurrection or duplicate capture release.
- Detached Pending work retaining its scheduler and destination after all executor handles are
  gone, resuming on a worker wake, then releasing its captures, scheduler and destination on
  completion. Channel disconnection independently checks final destination release.

The separate crate-external `crates/gpui/tests/send_foreground.rs` requires `test-support` and
uses the existing public `TestAppContext` and CPU TestPlatform. Its first workflow creates two
independent Apps with real windows. A worker submits and wakes a Send future through the first
App's handle, and driving the unrelated App cannot advance it. A local consumer awaits the
`Cell` result, updates the originating root entity, calls `Context::notify`, and returns an
`Rc` result through the unchanged local executor. An entity observer checks delivery and the
view's render counter checks automatic dirty drawing. The unrelated root and its render count
remain unchanged. The test closes both windows and checks exact root/App release. It never
calls draw while a typed root update holds the root borrow.

The second workflow cancels worker-submitted tasks before and after their first poll and checks
capture release on the owner, receiver cancellation, unchanged window state and no redraw.
The third retains cloned Send handles through window close and App release, checks that neither
root nor App remains alive, and explicitly drives the original test scheduler to finish a later
submission. Dropping the last Send handle then releases the scheduler. A retained dispatch
handle does not promise that an event loop is still running.

These are runtime test bodies, with no deliberately unexecuted signature closures. The commands
below define verification instructions, not a record that the new targets have compiled or run.
Run them serially from the repository root with `CARGO_BUILD_JOBS=1`, reusing the existing
lockfiles and build directory:

```text
cargo nextest run --locked -p open-gpui --no-default-features --features test-support --test send_foreground --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked -p open-gpui --no-default-features --features test-support,profiler --test send_foreground --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --test send_foreground --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features profiler --test send_foreground --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

The ordinary dispatcher keeps its receiver alive until accepted work has been drained. Its
defensive closed-channel fallback follows the erased runnable's thread-affine drop policy;
it can forget a refused off-owner runnable. Admission refusal alone therefore proves neither
awaitable Task settlement nor capture release, and these tests make no such assertion. Native
queue refusal/accounting, stalled-loop behavior and teardown retain their separate owner tests.
Neither target establishes OS-native wake delivery, Windows dispatcher behavior, native
`Submitted`, GPU output, native resource retirement, Web Fetch behavior, measured profiler
overhead or a minimum supported compiler. The actual native App-update-to-Submitted workflow
and its qualification belong to the separate native consumer and verification record.

## Randomized tree public consumers

The `randomized_tree` test and benchmark targets consume the production randomized-tree API
from fixed Zed source
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012).
Both require this fixture's existing `bench-support` feature, which forwards Core and platform
benchmark support and enables Criterion 0.8.2 and rand 0.9. They use the independent consumer
lockfile and add no dependencies, test-support, leak-detection or measurement framework.

The five value tests execute configuration builders and density normalization, seeded Bounds
sampling, deterministic topology and snapshot recreation, stable IDs and seeded mutation replay.
Insertion and subtree removal check counts and preservation of the next ID across reconstruction.
The public mutation inventory must contain all sixteen controls, including `RemoveLeaf`.
These test bodies contain no deliberately unexecuted App or Window closures. They do not launch
windows; the companion benchmark executes the App and Window workflow.

The benchmark registers `randomized-public/mutation/{ordinary,cached}/{kind}/seed-{7,23}` through
`#[open_gpui::bench]`, `Criterion<BenchMeasurement>`, `bench_group!` and `bench_main!`. Every
executed mutation case builds a fresh seeded, wide tree with persistent entities and handlers
using `new_with_config`, warms both frame buffers and applies its explicitly selected mutation
with notifications. The existing `bench_batched_task` harness measures the mutation and scheduled
frame delivery. Setup, snapshot comparisons, reference construction and destruction stay outside
timing and tracing. This is a small public integration workload; the platform's separate tree
benchmark owns its full topology/family and workload matrix.

After measurement, the accepted generation must have advanced and `Window::rendered_scene`
supplies the actual accepted scene. The consumer compares it with freshly rendered plain
`from_snapshot` and entity-preserving `from_snapshot_with_entities` reconstructions. The scene
comparison follows renderer batch order and retains primitive geometry, colors, transforms,
shadow/path/text attributes and every exact clip shape in each primitive's ancestry. It normalizes
frame-local clip offsets, path IDs and per-window atlas allocation identities and coordinates,
while preserving which primitives share each texture and tile. Positive and negative
normalization controls distinguish legal allocation renumbering from incorrect glyph aliasing;
it does not read back glyph textures or compare pixels. `RootVisibility` additionally restores
the original root and checks the restored scene against both fresh references. Each original
and reference root must release its weak handle and invoke its release callback exactly once
after replacement or window close; no window may remain before the next measured setup.
Reference windows become active while mounted. After closing them, the consumer reactivates its
persistent window and asserts active state so restoration is not left behind the inactive frame
rate limit. Every mutation, including visibility restoration, must advance the accepted generation.

Run these commands serially from the repository root with `CARGO_BUILD_JOBS=1`, reusing `target`.
The first command exposes the resolved feature graph for review: the benchmark-only graph must
not activate `open-gpui/test-support` or `open-gpui/leak-detection`.

```text
cargo tree --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features bench-support -e features -i open-gpui
cargo nextest run --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features bench-support --test randomized_tree --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo bench --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features bench-support --bench randomized_tree --jobs 1 --target-dir target -- --test
cargo bench --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --no-default-features --features bench-support --bench randomized_tree --jobs 1 --target-dir target
```

Leave `SEED` unset to execute both declared seeds 7 and 23. The shared harness lets `SEED`
override explicit seeds, so a platform smoke's single-seed setting must not carry into this
consumer unnoticed. Set `BENCH_MEASUREMENT=wall-time` to omit hardware counters during smoke execution. The
ordinary benchmark macro supplies `bench_platform`, `bench_text_system` and
`current_headless_renderer`; this consumer does not replace their renderer selection or lifetime
owners. The current-platform factory returns no GPU renderer on Windows. Other hosts may create
their ordinary headless renderer, but accepted-scene equality alone does not prove GPU execution.
These commands are runnable verification instructions, not a record of successful compilation,
benchmark execution, native `Submitted`, capture, pixel correctness or measured performance.
Actual GPU submission/capture and numeric performance retain their separate qualification owners.

## Async clipboard and paste consumer

The `clipboard` integration test consumes the official async clipboard and platform paste APIs
from Zed
[`d70c45e5bb1d8b7b3926fba5a6d1c69c23bbc405`](https://github.com/zed-industries/zed/commit/d70c45e5bb1d8b7b3926fba5a6d1c69c23bbc405),
within target
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012).
It uses the existing independent lockfile and requires no new dependencies or manifest changes.

Without optional features, two executed controls check all three exact `ClipboardReadError`
variants (`Unavailable`, `Denied(String)`, `UnsupportedContent`), their `Display` and standard
error contracts, and mixed clipboard entry order, text metadata, image format/bytes, paths and
text projection. These error values are constructed controls, not permission-denial injection.

With the fixture's existing `bench-support` feature, three additional tests execute actual App
and Window calls without `test-support`. They construct the existing public
`bench_platform(None, Arc::new(NoopTextSystem))` and start it through
`Application::run_embedded`; this platform returns to its caller. `HeadlessAppContext` is gated
by `test-support` and is not used here. No Criterion benchmark or native platform loop runs.

The runtime cases call `App::read_from_clipboard_async` through a shared App, retain its ready
task, overwrite the clipboard, and only then poll the task. Both an initially empty `Ok(None)`
and a full mixed-item snapshot must remain independent of later writes. The paste case directly
executes `InputHandler::paste` on `ElementInputHandler` against a live Window, comparing the
default `EntityInputHandler` text replacement with a full-item override. Mixed text/image/path
items preserve all entries and metadata in the override; the default concatenates the text.
Image-only and empty items perform no text replacement, and path-only items retain the existing
path-to-text fallback. The lifetime case closes a real Window, verifies its weak root releases,
retains the ready read and both async contexts across App release, and checks that late window
updates fail and the App-owned drop marker releases exactly once.

Run the default controls and then the runtime cases serially, with `CARGO_BUILD_JOBS=1` and the
repository build directory. Both commands use the independent lockfile without `test-support`:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test clipboard --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features bench-support --test clipboard --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

The companion crate-external `crates/gpui/tests/clipboard.rs` target uses `test-support` and the
single-use `TestAppContext::set_next_clipboard_read` fixture for pending completion, each exact
platform error and cancellation. It also drives an actual mouse callback on an accepted frame
to await a read and paste the full item through the public entity adapter, and rejects a late
paste after window/entity release. Its direct default-handler case separately covers
`InputHandler::paste` text fallback. Run that target separately:

```text
cargo nextest run -p open-gpui --features test-support --test clipboard --test-threads=1 --no-fail-fast --no-tests fail
```

These commands are verification instructions, not a record of successful execution. Direct
handler calls do not establish delivery through an installed platform input-handler slot.
Neither consumer qualifies browser permissions, DOM paste events, native system clipboard
access, image decoding, renderer output, native `Submitted`, or measured performance. The
framework owner, browser and native consumers retain those separate responsibilities.

## Credential tasks and macOS add-only consumer

The `credentials` integration test consumes the credential APIs and macOS-only add-only method
from official Zed
[`79fe6a645f68f8c9230578f154bf99852d9f716f`](https://github.com/zed-industries/zed/commit/79fe6a645f68f8c9230578f154bf99852d9f716f),
as present at fixed target
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012).
`App::write_credentials_if_not_exists(&self, url, username, password)` returns
`Task<Result<bool>>` only on macOS. `true` means creation and `false` means an existing item was
left unchanged. The Platform default immediately returns
`this platform does not support atomic credential writes`; it has no read-then-write fallback.
Ordinary `write_credentials`, `read_credentials` and `delete_credentials` retain their contracts.

With the fixture's existing `bench-support` feature, two runtime tests execute public App,
Platform and AsyncApp calls through `bench_platform(None, Arc::new(NoopTextSystem))` and
`Application::run_embedded`, without enabling `test-support`. The platform supplies ready
credential stubs: read returns `Ok(None)`, and write/delete return `Ok(())`. Reads remain empty
after a successful stub write. The tests poll tasks after their borrowed input buffers leave
scope; a separate lifetime case retains tasks and AsyncApp across shutdown and App release,
then verifies that the App-owned drop marker releases exactly once and tasks still complete.
These are executed call paths when the tests run, rather than uncalled signature closures.

On macOS, `bench-support` enables a third runtime test that executes the add-only unsupported
default through both `Rc<dyn Platform>` and a shared `&App`. Ordinary stub writes still succeed,
but neither add-only call may report creation or a duplicate. The lifetime case also retains an
add-only task beyond input and App release and checks the exact unsupported error.

Three macOS-only error controls require no optional fixture feature. They consume the backend's
public `open_gpui_macos::keychain::{KeychainError, KeychainOperation}`, matching official Keychain
source blob `a40eafcf2032a3cae4282e822a0354fb63aaf51c`. These tests require the fixture's
target-specific `open_gpui_macos` dependency. Constructed `Security`, `UserCanceled` and
`InvalidResponse` errors check exact operations, OSStatus values, official secret-free messages,
and borrowed and owned downcasting after application context is added through `open_gpui::Result`.
The controls construct errors directly; they do not inject OS statuses, query Keychain, or prove
native cancellation, absence classification, duplicate handling or persistence. Every credential
assertion uses a boolean condition and static failure message so failures do not print read tuples
or test credential buffers.

Run the independent runtime target serially with `CARGO_BUILD_JOBS=1` and the repository build
directory. It contains two tests on Windows/Linux and six on macOS, including the error controls:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features bench-support --test credentials --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

On macOS, run the three standalone error controls without `bench-support` as well. On other
targets, this feature-free credentials target intentionally contains no tests:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test credentials --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

The companion crate-external `crates/gpui/tests/credentials.rs` uses `test-support` without any
backend dependency. Its two portable tests execute the ready stub contract and App/task/async
context lifetime; a third test on macOS checks the exact unsupported add-only default. Run it
separately:

```text
cargo nextest run -p open-gpui --features test-support --test credentials --test-threads=1 --no-fail-fast --no-tests fail
```

These commands are verification instructions, not execution results. Windows results do not
compile or execute the macOS-only API or error controls. Neither consumer qualifies native
credential storage, interprocess atomicity, native platform loops, renderer output or `Submitted`.
The native backend tests and platform qualification retain those separate responsibilities.

## Runtime operating system consumer

The `platform_info` integration target consumes official Zed
[`389c22e1c747d1a433c3b51c176936166c9cc1cf`](https://github.com/zed-industries/zed/commit/389c22e1c747d1a433c3b51c176936166c9cc1cf),
as present at fixed target
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012).
`PlatformInfo { os, target }` distinguishes host conventions from application delivery.
The two feature-free controls cover all seven `OperatingSystem` variants and both
`PlatformTarget` variants, dynamic `parse_with_os`, explicit `display`/`unparse`, static
`parse` rejection of `secondary`, explicit-modifier preservation, binding labels and OS defaults
in `KeyContext`. Only `Mac` resolves `secondary` to Command; the other variants use Control.

With the existing `bench-support` feature, one additional test runs `Application::run_embedded`
over `bench_platform(None, Arc::new(NoopTextSystem))` without enabling `test-support`.
It checks the platform and App's actual native host identity, loads owned dynamic bindings
through `KeyBinding::load(..., &App)`, draws a real Render tree, dispatches single keys, a held
repeat, explicit Control/platform keys and a chord to registered action listeners, and compares
rendered labels with `Window::keystroke_text_for`. An OS-scoped action consumes the default key
context. Activation drains the existing threaded dispatcher and requires the exact ticket's
`Activated` result before checking Window activity. Focus changes cancel a pending prefix before
the sibling receives its suffix. Closing
with a pending prefix releases the exact root and its observation, and retained async contexts
cannot update the retired Window or released App. The benchmark platform has no public identity
injection API: other OS conventions are exercised by the explicit pure controls.

Run both independent configurations serially with `CARGO_BUILD_JOBS=1`, the existing lockfile
and the shared build directory:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test platform_info --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features bench-support --test platform_info --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

The companion public target uses `TestAppContext::set_platform_info` to execute all fourteen
OS/target combinations in one App on one host. Its actual window input callbacks exercise
single/repeated actions, chords, explicit-modifier controls and labels, same-focus prefix
preservation, sibling/blur cancellation, the 999 ms / 1000 ms prefix fallback boundary, and
exact entity release with no timer or observer delivery after retirement:

```text
cargo nextest run -p open-gpui --features test-support --test platform_info --test-threads=1 --no-fail-fast --no-tests fail
```

One bounded Windows native test uses the actual `WindowsPlatform`, App, registered HWND,
DirectWrite and DirectX. Public binding loading uses the installed keyboard mapper. Synthetic
keys enter the native input callback; an explicit `WM_CHAR` enters the focused native text slot.
Shortcut labels, input text and an action/source marker must belong to the same visible,
accepted scene with a nonempty `Submitted` record. `capture_frame` re-renders that scene for
GPU readback; it does not read the earlier swapchain submission. Exact text is checked in
shaped source; pixel checks establish row ink and marker colors, not exact glyph images.
The test changes focus between real text handlers, executes an actual prefix-timeout fallback,
then closes with another live timed prefix, checks exact registration/generation retirement and
handler/root release, opens a successor that cannot inherit that prefix, and retires the App,
platform message window and OLE owner. This test is ignored by default because it needs an
interactive Windows desktop and a DirectX GPU:

```text
cargo nextest run -p open-gpui-windows --lib -E 'test(=platform::native_test_support::native_platform_info_tests::native_runtime_os_loads_routes_labels_text_and_exact_window_retirement)' --run-ignored only --test-threads=1 --no-fail-fast --no-tests fail
```

These commands are verification instructions, not execution results. Independent scenes are
discarded and use `NoopTextSystem`; public test-platform frames do not establish native
`Submitted` or GPU output. Even the native case does not establish physical key translation,
foreground acquisition, live IME composition, browser user-agent detection, other native OS
backends, compositor scanout or performance. Injected `Web` identity exercises shared host
conventions and does not launch a browser.

On Windows, `mapped_keybindings` uses ordinary dependencies and the actual native App mapper,
without `test-support` or `bench-support`. A child process enters the backend's blocking
`Application::run` and checks shifted single-key and chord installation, string preflight,
pending/full palette resolution and already-normalized event input. The parent requires a
completion marker and successful exit within thirty seconds. It opens no native Window and
does not qualify a returning external loop. The fixture adds only the existing command package's
Windows dev-dependency edge to its independently maintained lockfile:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test mapped_keybindings --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

## Native restart arguments consumer

The Windows `restart` target runs the ordinary public Application/App restart path from official
Zed `715076597926d566d560b30b806749e1a2166080`, with fixed-target PowerShell discovery
corrections. It needs no `test-support` or optional fixture feature. The `restart_probe` binary
has explicit worker and recorder roles; it never restarts the test runner or a user's application.
The parent starts the build-output worker, assigns it to a kill-on-close Job before releasing
stdin, and selects a distinct copied recorder in a path containing Unicode and shell characters.

The actual backend launches PowerShell, waits for the worker to exit and starts that selected
recorder. The recorder writes native UTF-16 arguments, its PID and transport-environment absence.
The parent requires successful worker exit, exact native payload values, selected-executable
identity, restart-before-quit callbacks and zero active Job processes. PowerShell may normalize
the executable's verbatim path prefix; its file identity is checked separately from the exact
argument payload. The complex-argument case independently observes the retired worker PID before
recording. The no-argument case checks the other launcher branch. A third control explicitly
terminates a blocked Job-owned worker and observes completed cleanup.

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test restart --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --all-features --test restart --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

These commands are instructions, not results. The fixture uses an explicit existing target,
checks configured arguments rather than automatically retaining the original process argv,
and opens no native Window. It does not qualify an updater, a default-target restart, native
renderer retirement, physical input or foreign OS launchers. Linux and Mac tests retain their
separate backend gates. The fixture promotes its existing target-specific windows dependency
for the recorder binary and enables only its native process/Job feature closure; no framework
production dependency or compiler-floor change follows.

## Scoped activity consumer

The `idle_sleep` target consumes the public ActivityGuard, App/Platform idle-sleep acquisition
and BackgroundExecutor/PlatformDispatcher App Nap interfaces from official Zed
`450775244fade36db70af2447be24e74edb43368`, reconciled with the fixed target. Ordinary Rust
cases retain Send ownership, one-shot release, noop and unwind without test support.

On Windows, the dedicated `idle_sleep_probe` binary creates an ordinary headless native App.
Its worker warms the existing scheduler, acquires eight real power-request guards, releases
four independently, releases the remainder on another thread and observes process handle
counts. The non-Apple App Nap default creates no native power request. The parent requires a
completed report, the actual App quit callback and successful process exit within thirty
seconds. Failure cleanup kills and observes only its owned worker with a bounded deadline;
temporary report files belong to its exclusively created directory. It starts no user app,
opens no visible window, changes no persistent power setting and never forces sleep.

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test idle_sleep --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --all-features --test idle_sleep --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

These commands are instructions rather than results. Process resource observations do not
measure OS power-policy enforcement or energy use. Mac activity and Linux portal acquisition/
Request::close retain their own native gates; a Windows run cannot qualify them.

## Window appearance consumer

The `window_appearance` target consumes `App::set_window_appearance(&self, Option<WindowAppearance>)`
and the Platform setter/getter without optional features or `test-support`. Its official source is
Zed `8e4e5a39eed932d943cb7a3e7078885b75d1ea54` at fixed target
`635026fabc38440fce5e1767d125026a17b12012`. The signature control runs on every host. On Windows,
an actual `current_platform(true)` exercises all four variants, repeated overrides and `None`
requests against the default no-op setter while preserving the native system appearance getter.
It drops the platform without launching an App, opening a Window or entering the native loop.

With the existing `bench-support` feature, an ordinary App and hidden headless Window execute the
same requests on the returning benchmark platform with `NoopTextSystem` and no renderer. App,
Platform and Window getters retain their baseline values through replacement, repetition and
clear requests, and the Window is explicitly closed before App shutdown. This is the benchmark
platform's no-op behavior, including on macOS; it does not establish native App/Window behavior.

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test window_appearance --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features bench-support --test window_appearance --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

These are verification instructions, not recorded results. The Windows runtime case requires the
host's normal native platform initialization prerequisites; it changes no persistent OS setting.
The root `test-support` consumer separately checks unchanged appearance observers and rendered
frame revisions. macOS override/clear behavior, system tracking and native titlebars require the
separate Mac backend gates; neither this Windows control nor the benchmark platform qualifies them.

## Startup and live windowing consumer

Linux windowing uses the fixed upstream calloop idle-dispatch corrections. Cargo does not
inherit dependency workspaces' root patches, so a standalone consumer that includes the native
platform must select the same source in its own workspace root:

```toml
[patch.crates-io]
calloop = { git = "https://github.com/zed-industries/calloop", rev = "3759371fee14c40066d64777e2a36b6ffcc22590" }
```

This fixture declares that patch and independently locks the Git revision. The direct Linux
dependency and calloop-wayland-source must resolve to the same calloop source; matching
package names or compatible version numbers do not make their Rust types interchangeable.
The ordinary and browser example roots each declare their own patch.

The `windowing` target consumes `WindowingRequest`, the host's `GraphicalEnvironment`, application
`ActivationPolicy`, `Application::with_windowing`, and the App/Platform mode-request and environment
interfaces. Linux and FreeBSD also consume `WindowingModes` and the public
`open_gpui_platform::linux(allowed_modes)` factory. These APIs come from official Zed
[`f37989fbdf840b308892af66516c2b9c3ef3853a`](https://github.com/zed-industries/zed/commit/f37989fbdf840b308892af66516c2b9c3ef3853a),
as present at fixed target
[`635026fabc38440fce5e1767d125026a17b12012`](https://github.com/zed-industries/zed/tree/635026fabc38440fce5e1767d125026a17b12012).
The port also includes the launch/display-refresh integration from
[`f495dd0f77b69e287ed72c1168dc02965f08be7d`](https://github.com/zed-industries/zed/commit/f495dd0f77b69e287ed72c1168dc02965f08be7d);
its exact forwarding, refresh and native-retirement coverage belongs to the Core/backend gates.
Application policy remains distinct from the existing per-window `WindowActivationPolicy`.

Feature-free cases check the ordinary public signatures and graphical environment behavior.
Linux/FreeBSD inspect child `Command` overrides for replacement, removal, repeated application,
unrelated-variable preservation and the child's unchanged activation token. No command is started
and no process-global environment variable is written. Windows detects the actual process session
and checks that applying a different session to a child command leaves its environment unchanged.
An actual `current_platform(true)` on Windows repeatedly refuses both live requests, including
after startup-mode and activation-policy calls, while retaining its text system and reporting no
graphical connection. It never launches an App or enters the native loop on a test-runner thread.

With `bench-support`, one ordinary App runs on the existing returning benchmark platform with
`NoopTextSystem` and no renderer. It consumes actual unsupported Tasks from both App and Platform,
changes and reads retained entity state after failures, and opens/closes/reopens the same retained
root three times. Both requests fail while each Window is open; after closure and dispatcher
drain, requests reach the platform's unsupported default again. Initial-mode and activation-policy
setters retain their documented no-op behavior on this platform. These are failure/recovery and
consumer-lifetime controls; they do not simulate a successful display-connection switch.

Run the independent configurations serially with `CARGO_BUILD_JOBS=1`, the existing lockfile and
the shared build directory:

```text
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test windowing --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features bench-support --test windowing --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
```

The companion crate-external target checks the same public environment contract and executes
default unsupported Tasks and open-window refusal with a retained root through `TestAppContext`:

```text
cargo nextest run -p open-gpui --features test-support --test windowing --test-threads=1 --no-fail-fast --no-tests fail
```

The ordinary `toggle_headless` example is registered in `open-gpui-platform` and requires neither
`test-support` nor `bench-support`. Run it from a terminal on Windows/macOS, or enable the desired
compiled display backends on Linux/FreeBSD:

```text
cargo run -p open-gpui-platform --example toggle_headless
cargo run -p open-gpui-platform --example toggle_headless --features wayland,x11
```

It starts headless through the switchable application factory, retains Todo entities, and uses the
same App for `create`, `ls`, `open`, `close`, another `create`, and `open` again. The printed entity
IDs and addresses let a human inspect retained state. `close` waits for the exact window's native
terminal notification before requesting headless mode; a bounded timeout reports pending
retirement without bypassing readiness. Closing via the window manager follows the same tracked
retirement path. Backend errors are reported while the command loop stays available. `quit` and
stdin EOF request ordinary graceful shutdown. Linux/FreeBSD `open` accepts explicit `DISPLAY`,
`WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR` and `XDG_ACTIVATION_TOKEN` assignments; Windows accepts
`SESSION=id`. Plain `open` detects the process environment. Use `close` before choosing another
connection. The ordinary `headless()` facade stays fixed headless and cannot attach later.

The [windowing qualification record](../../../docs/verification.md#startup-and-live-windowing-lifecycle)
distinguishes executed results from these instructions. Native Windows controls require normal
platform initialization; the example additionally needs an interactive graphical
session, fonts and graphics support, and Linux/FreeBSD require an enabled reachable display
backend. macOS must run the example on the actual main thread. Benchmark `run_embedded` does not
establish a returning native engine loop, an external Host/View adapter, shared GPU ownership,
native rendering/input, successful detach/reattach, compositor presentation or performance.
Backend-specific session refusal, display freshness, activation, GPU retirement and switching
qualification retain their separate native gates.
