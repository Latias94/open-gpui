# Independent dependency consumer

This application is its own Cargo workspace. It consumes Open GPUI through path dependencies
without inheriting the repository workspace's dependency resolution or lockfile. Its own
`Cargo.lock` records the consumer resolution.

The application pins Taffy 0.15.0, WGPU 30.0.1, and, on macOS, Core Foundation 0.10.1 to exercise
application-owned dependency choices independently of the library workspace. Keep these exact pins in
the application fixture; library dependencies should allow compatible patch releases. The direct
WGPU dependency requests only its `std` feature for API types.

The optional `wgpu-renderer` feature also includes `open-gpui-wgpu` in the consumer graph, so the
all-features gate compiles the actual renderer against the application's pinned WGPU 30.0.1.
Cargo unifies the renderer's WGPU feature requests with the direct dependency's features. This
compile check does not select WGPU as the Windows platform backend or establish GPU resource
sharing.

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
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test lifecycle --jobs 1 --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
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
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test profiler --jobs 1 --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features profiler --test profiler --jobs 1 --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
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
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features profiler --test foreground_journal --jobs 1 --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
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
cargo nextest run -p open-gpui --features test-support,profiler --test foreground_journal --jobs 1 --test-threads=1 --no-fail-fast --no-tests fail
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
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test frame_signals --jobs 1 --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features profiler --test frame_signals --jobs 1 --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
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
cargo nextest run -p open-gpui --features test-support,profiler --test frame_signals --jobs 1 --test-threads=1 --no-fail-fast --no-tests fail
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
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test frame_scheduling --jobs 1 --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features profiler --test frame_scheduling --jobs 1 --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
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
cargo nextest run -p open-gpui --no-default-features --features test-support --test frame_scheduling --jobs 1 --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run -p open-gpui --no-default-features --features test-support,profiler --test frame_scheduling --jobs 1 --test-threads=1 --no-fail-fast --no-tests fail
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
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test refresh_measurements --test profiler --jobs 1 --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features profiler --test refresh_measurements --test profiler --test foreground_journal --jobs 1 --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
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
cargo nextest run -p open-gpui --no-default-features --features test-support,profiler --test refresh_measurements --jobs 1 --test-threads=1 --no-fail-fast --no-tests fail
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
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --test power_cadence --jobs 1 --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --locked --no-default-features --features profiler --test power_cadence --jobs 1 --target-dir target --test-threads=1 --no-fail-fast --no-tests fail
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
cargo nextest run -p open-gpui --no-default-features --features test-support --test power_cadence --jobs 1 --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run -p open-gpui --no-default-features --features test-support,profiler --test power_cadence --jobs 1 --test-threads=1 --no-fail-fast --no-tests fail
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
