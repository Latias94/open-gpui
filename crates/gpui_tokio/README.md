# Open GPUI Tokio

An optional application bridge between Tokio futures and Open GPUI tasks. Applications opt in
by depending on `open-gpui-tokio`; the framework and its HTTP client do not initialize this bridge.

Call `open_gpui_tokio::init(app)` during application setup to create an App-owned Tokio runtime
with two worker threads. If the application already owns a runtime, call
`open_gpui_tokio::init_from_handle(app, runtime.handle().clone())` instead. The application must
keep that external runtime running; borrowing its handle does not transfer runtime ownership.

`Tokio::handle(&App)` returns a cloned handle to the selected runtime. `Tokio::spawn(&context,
future)` accepts a public `AppContext` and returns `Task<Result<T, JoinError>>`, preserving any
application-level `Result` inside `T`. `Tokio::spawn_result` accepts an `anyhow::Result<T>` future
and returns `Task<anyhow::Result<T>>`, preserving application errors and wrapping join errors.
Dropping either GPUI task aborts its Tokio task. Dropping the App global shuts down an owned
runtime with `shutdown_background()`; borrowed runtimes remain under their caller's control.
As in Tokio, already-running blocking work cannot be aborted by dropping a task.

The crate explicitly enables Tokio's `rt-multi-thread`, `net` and `time` features.
`rt-multi-thread` implies `rt`; `enable_all()` enables the selected network and timer drivers.
The root currently declares only `tokio = "1"`, so the bridge does not rely on feature
unification from an unrelated workspace consumer. Applications can select further Tokio
capabilities, such as `io-util`, `fs`, `process` or `sync`, through their own dependency.

## Source and license

This imports the complete Apache-2.0 `crates/gpui_tokio` implementation from fixed Zed target
[`6ceb6fee8738274589b59139df27efc191ec2231`](https://github.com/zed-industries/zed/tree/6ceb6fee8738274589b59139df27efc191ec2231/crates/gpui_tokio).
Its source blob is `6ef71b4890106bdf733c873a52c0cf076ab28477` and its manifest blob is
`7b2d180ecd5fe58733111acdfb8b6ffcf0635e98`. Both match the available historical endpoint
`78f5d7446c55f6c1aa9b66e8111f6b3f4625ebb0`; this crate has no change between those endpoints.
The target was checked through official fixed-SHA raw sources without moving the reference
checkout or fetching objects into it.
The separately observed upstream remote HEAD on 2026-11-13 was
`4dff6f8eef9548d297073845fecb6c41de9f4c36`; it does not advance the fixed comparison target.

The bridge already exists at content reconstruction anchor
`6e9465a4288c332208643892e23b9d35d7be5c79` (source blob
`d32843ae86f428a3f47f54e3387a75c5aedbcddc`). Its absence from the original extracted closure
is an inherited omission. This import explicitly adopts that independent framework owner;
it is not excluded by an earlier bug-fix-only sync boundary.

The imported source history includes initial owner `bd213340133f23add7d5b1d4bc250d1880b6ff95`,
generic-context support `517e519bdcc868d94a130e913403e5b2db4a59a3`, and subsequent source changes
`8a307e7b89f50a22fc497df0ff06736eb704837c`, `1b2ceae7efb2b871d19025582cabc4619eee1bdc`,
`fd3ca0303ff1dfc552119878724eb151bc94b49c` and
`97c35c084b9a1058b2a57ada953a20eb8af95665`. The following mechanisms and manifest changes
are also adopted:

| Upstream commit | Adopted behavior |
| --- | --- |
| `158ebdc5803f9f69dfbfddcd216f04e5bc6001d9` | Caller-provided Tokio handles and separate owned/borrowed runtime lifetimes. |
| `101083e7f9c92c52464551d3f888dfa5894b44e8` | Explicit `shutdown_background()` when the App global releases its owned runtime. |
| `a873cf402c8d5ffa13ab54efd29ccd4df59c7e46` | Use the small GPUI utility owner for `defer`, rather than the general `util` dependency. |
| `aec7395e30feba9139c9ab8b9444d672882ddf41` | Disable the empty library unit-test harness; public integration tests remain enabled. |
| `d544470d1d3770f0024651dbf121525564a2657c` | Select `rt-multi-thread` without a redundant explicit `rt` feature. |

Production source changes are limited to the `open_gpui` and `open_gpui_core_util` import
aliases. Package metadata follows this workspace, and explicit Tokio driver features make
the standalone dependency contract complete. The upstream Apache license is retained as a
regular [LICENSE-APACHE](LICENSE-APACHE) file, including its existing attribution.

## Workspace integration

`crates/gpui_tokio` is registered in the root `[workspace].members`. Consumers declare
the optional dependency directly; the root does not define an unused workspace alias.
For an application under `examples/my-app`, use:

```toml
open_gpui_tokio = { package = "open-gpui-tokio", version = "0.2.0", path = "../../crates/gpui_tokio" }
```

An external application can use
`open_gpui_tokio = { package = "open-gpui-tokio", path = "/path/to/open-gpui/crates/gpui_tokio" }`.
The library name is `open_gpui_tokio`. Upstream consumer migration changes `gpui_tokio` to
that alias; `init`, `init_from_handle`, `Tokio::{spawn, spawn_result, handle}` and `JoinError`
retain their upstream signatures and behavior. No existing framework consumer must migrate.

## Verification

`tests/bridge.rs` uses the public GPUI `test-support` API and real Tokio workers. It covers owned
and borrowed handles, network and timer drivers, nested and flattened application errors,
panic and cancellation join errors, task-drop cancellation before and after the GPUI wrapper
is polled, and App-global shutdown inside an asynchronous context. The GPUI test scheduler
allows parking for real Tokio wakeups; it does not create another native platform thread pool.

All seven public bridge tests pass in the final affected **3,615/3,615** run
(`a5c5f0e9-7fcf-45bc-b489-5393662e1e27`) on macOS. The earlier **111/111** focused
run and **3,595/3,598** intermediate run retain their own scope; three unrelated fixture
failures are repaired before the final cohort. Three pre-existing AppKit-main-thread tests
in the platform package remain ignored. The exact package/features and platform matrix are
recorded in the [refresh matrix](../../docs/imports/zed-gpui-commit-reconciliation-2026-11-12.md#platform-and-functional-unit-verification-matrix)
and [verification guide](../../docs/verification.md#november-13-framework-closure-completion).

Focused verification entry points, run serially with the existing target directory, are:

```text
cargo check -p open-gpui-tokio --lib --no-default-features --locked
cargo nextest run -p open-gpui-tokio --test bridge --locked
cargo run -p xtask -- scan-import-boundary
```

Workspace and independently locked all-target compilation pass. Windows all-target
cross-compilation uses real MSVC headers and clang-cl, including this package. Actual
independent consumer runtime, compiler-floor and other native-platform results retain their
separate receipts in the verification record; host bridge tests do not qualify those runners.

Completed checks include byte comparison of the complete production source after the two import
aliases, exact upstream license comparison, TOML parsing, direct `rustfmt --check`, whitespace
checks and `rustc --print cfg --target aarch64-unknown-linux-ohos`. The last command confirms
the OHOS target also reports `target_os = "linux"`; it does not compile a backend.

The companion `484bcbfea6ec20b8a973404d2899879deec2bf20` routing slice excludes
`target_env = "ohos"` from this fork's Linux selector and backend dependencies, including its
headless-renderer path. `current_platform` explicitly panics on that target and directs the
consumer to `open_gpui::Application::with_platform`; no OpenHarmony backend is supplied or
added as a dependency. OHOS target compilation and execution remain separate verification.
The platform's macOS visual-test module additionally requires `test-support`, because its Core
visual helper is unavailable in a benchmark-only build. `bench-support` remains independent.
