# Independent additive test-support consumer

The `property_path` target also executes two public `#[open_gpui::property_test]` workflows
through the reexported proptest 1.12 path, without a direct proptest dependency. Each retains
four generated cases, actual TestPlatform drawing, entity notifications/deferred effects and
release or shutdown. It is gated by `test-support`; bench-only selection excludes the target's
body. Run `cargo nextest run --locked --manifest-path tests/fixtures/test-support-consumer/Cargo.toml --test property_path --target-dir target --test-threads=1 --no-fail-fast --no-tests fail`.
These are verification instructions, not native presentation or GPU qualification.

This native consumer is its own Cargo workspace and owns an independent `Cargo.lock`. It uses
only public Open GPUI, scheduler, and utility APIs through normal path dependencies. It has no
Nara dependency and does not inherit the repository workspace's dev-dependency feature selection.
The separate `dependency-consumer` fixture remains the consumer whose all-features graph excludes
test support; do not add these features to that fixture.

The contract follows Zed `635026fabc38440fce5e1767d125026a17b12012`, including the additive
test-support change introduced by `95cd535a5fad96d649513f96c5784ceefd379e47`.

## Scope

| Feature selection | Executed public consumer behavior |
| --- | --- |
| `--no-default-features` | Checks availability of the ordinary `App`, `AsyncApp`, GPUI background executor, scheduler local/background executor identity queries, and the process hint. Runs the ordinary home-directory binary. Does not construct an App or a Test context. |
| Default, or `--no-default-features --features test-support` | Also constructs an actual `TestAppContext` through the public test macro, executes background-to-foreground entity updates, owns and closes a real Test window, and checks typed/erased/weak ownership, release callbacks, retained resources before effect cleanup, and snapshots. Forwards utility `test-support` to the ordinary binary. |
| `--no-default-features --features bench-support` | Also launches a returning `Application` and executes a real `BenchAppContext`, with neither GPUI nor utility test support enabled by this fixture. Criterion is required only for this workflow. |
| `--features bench-support` | Runs all of the above. The returning application exists before a Test context is created; it and the subsequent benchmark retain their own real executor identity after `test_scheduler_created()` becomes true in that same process. |
| `--features leak-detection` | Explicitly forwards `open_gpui/leak-detection`, independently of test support. |

The ordinary binary opens the public home directory for reading and applies the public path
compaction policy to an application settings path. It writes no home-directory files. This is
normal downstream consumption with utility test support selected, rather than another exact
comparison with `dirs::home_dir()`; the utility crate's integration test owns that comparison.

The application and benchmark use `bench_platform` with a `NoopTextSystem` and no GPU renderer.
They execute real App ownership and shutdown, entity updates, foreground tasks, worker threads,
and a real two-millisecond timer in each measured task. Worker thread identity and wall-clock
elapsed time are asserted. This does not qualify native event-loop integration, native text
shaping, GPU rendering, compositor presentation, or performance. There is no virtual-clock
benchmark path. The test contexts remain deterministic and are feature-gated separately.

The repository's `crates/gpui/tests/test_support.rs` is another external public consumer. It
additionally checks failed and successful erased downcasts, snapshot baselines that ignore
pre-existing entities, weak upgrades as new strong handles, observer-driven automatic TestApp
drawing, cascading view/model release, and a benchmark that leaves the Test clock unchanged.

## Tracking expectations

Test support alone must not enable entity-handle tracking. The dependency enables tracking when
the `leak-detection` feature is selected or when `GPUI_LEAK_DETECTION` is nonempty at build time.
The value `0` is nonempty and therefore enables tracking. An empty value leaves environment-based
tracking disabled. The core crate's own `cfg(test)` tracking is a separate case; this fixture
consumes the library as a normal dependency.

The runtime, test-only `GPUI_TEST_EXPECT_LEAK_DETECTION` can declare `on` or `off` explicitly
for the matrix below. Ordinary test commands need no extra expectation variable: both targets
default to their forwarded feature and compile-time `GPUI_LEAK_DETECTION` selection. An explicit
`off` is rejected when the leak feature is selected. The tests do not inspect a local
`cfg(gpui_leak_detection)` or infer build configuration from the runtime value of
`GPUI_LEAK_DETECTION`: dependency build-script cfgs do not propagate to this consumer. Snapshot
and lifetime behavior still verify the dependency's actual result against that declared build
selection; a runtime expectation cannot change compiled instrumentation.

In tracking mode, both a default empty snapshot and an earlier baseline reject new live entities.
Without tracking, those checks are no-ops even while a strong handle is live. In both modes,
`assert_released` rejects actual strong ownership and resources awaiting an effect-cycle flush.
Positive release assertions follow explicit App cleanup, with all owned strong handles dropped.
Expected panics are caught while the context remains alive and cleanup still runs.

## Serial verification

The main task owns lockfile generation and all Cargo execution. Generate the fixture lockfile
once from the repository root, review it, and retain it separately from the root lockfile:

```text
cargo generate-lockfile --manifest-path tests/fixtures/test-support-consumer/Cargo.toml
```

Reuse the repository `target` directory and run one Cargo command at a time. In PowerShell, the
tracking-off public-consumer check is:

```powershell
$env:CARGO_BUILD_JOBS = '1'
$env:GPUI_LEAK_DETECTION = ''
$env:GPUI_TEST_EXPECT_LEAK_DETECTION = 'off'
cargo nextest run -p open-gpui --locked --no-default-features --features test-support --test test_support --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/test-support-consumer/Cargo.toml --locked --target-dir target --no-default-features --features test-support --test public_api --test-threads=1 --no-fail-fast --no-tests fail
```

Run the following combinations serially; the tracking rows apply to both public test targets:

| Build-time `GPUI_LEAK_DETECTION` | Runtime expectation | Features | Purpose |
| --- | --- | --- | --- |
| Empty | `off` | `--no-default-features --features test-support` | Proves feature-only snapshots are no-ops and weak release checks still enforce real ownership and cleanup. |
| `1` | `on` | `--no-default-features --features test-support` | Proves build-time environment activation of tracking in a normal dependency. |
| `0` | `on` | `--no-default-features --features test-support` | Proves nonempty activation is not parsed as a Boolean. |
| Empty | `on` | `--no-default-features --features test-support,leak-detection` | Proves explicit feature activation without the environment switch. |
| Empty | `on` | `--all-features` | Includes real benchmark workflows after Test scheduler creation, with explicit tracking active. |

Also run these fixture selections with an empty `GPUI_LEAK_DETECTION`; use `off` for any
selection containing test support:

```text
cargo nextest run --manifest-path tests/fixtures/test-support-consumer/Cargo.toml --locked --target-dir target --no-default-features --test public_api --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/test-support-consumer/Cargo.toml --locked --target-dir target --no-default-features --features bench-support --test public_api --test-threads=1 --no-fail-fast --no-tests fail
cargo nextest run --manifest-path tests/fixtures/test-support-consumer/Cargo.toml --locked --target-dir target --features bench-support --test public_api --test-threads=1 --no-fail-fast --no-tests fail
```

The last selection exercises the returning application and real benchmark with test support
compiled in while tracking remains disabled. The root `--all-features --test test_support` run
covers its real benchmark scenario; the root tracking-off run deliberately selects test support
alone. Root all-features unit tests can assume their explicit tracking feature is enabled.
Continue running the existing `dependency-consumer` all-features gate independently. These
commands are acceptance instructions, not results recorded by the worker that added the fixture.
