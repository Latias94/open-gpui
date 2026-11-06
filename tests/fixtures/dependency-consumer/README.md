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
