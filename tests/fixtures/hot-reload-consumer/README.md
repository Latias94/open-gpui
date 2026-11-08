# Independent Native Hot-Reload Consumer

This independently locked workspace exercises the official GPUI/Subsecond render boundaries
without engine-private APIs. It contains a native executable and a separate `panel` workspace
member so that a successful patch must include code outside the executable crate.

The executable renders a typed cached entity, an `AnyView` constructed and stored at launch, and
a `RenderOnce` component with persistent keyed state. Observations identify the process, window,
entities, counters, render recipes and actual DX11 scene pixels. A frame is observed only when
its accepted, nonempty, rendered and Submitted generations agree. Offscreen scene readback does
not establish OS compositor scanout.

## Build

```text
cargo check --locked --manifest-path tests/fixtures/hot-reload-consumer/Cargo.toml --all-targets
```

The core `hot-reload` feature is optional. Ordinary builds can compile this consumer with
`--no-default-features`; the transport and render indirection are enabled only in native debug
builds. The pinned transport uses Dioxus/Subsecond 0.7.10 and async-tungstenite 0.33. Other
platforms can compile the consumer but report unsupported native execution.

## Real Edit, Build And Apply

On Windows, run from a Visual Studio developer shell with the MSVC/Windows SDK environment
available to child processes:

```text
python tests/fixtures/hot-reload-consumer/run.py --dx PATH_TO_DX_0_7_10 --target-dir PATH_TO_SHARED_TARGET
```

Use the official `dx` 0.7.10 executable, verifying its published archive checksum before use.
The driver does not replace an installed tool. It copies this fixture into a unique directory
inside the supplied target, rewrites only the copied path dependencies, and runs
`dx serve --hot-patch`. It preserves the maintained source files and lockfile.

The original Windows x64 dx 0.7.10 executable has a reproduced code-generation defect:
its unresolved-function stub overwrites RAX, corrupting the `__chkstk` frame-size argument.
Large render frames then overflow on their first patched invocation. Upstream issue Dioxus
`#5966` and patch `#5967` identify the same defect. The
[narrow backport](dx-windows-x64-stub.patch) applies that register-preserving jump to the
published `dioxus-cli` 0.7.10 crate source at
`57d6794ad60b949e5bd8aa282f6f8c3dc97a365e`. Build that tool separately and pass its executable
through `--dx`; record its exact source patch and binary hash with the result. This is a compiler
driver correction, not a modification to GPUI's official render boundaries. Unmodified and
backported driver outcomes must remain distinct in qualification records. Raising the application
stack reserve does not repair the corrupted argument.

For the published CLI crate extracted outside this repository, apply the backport with
`git apply --check` followed by `git apply`, then build it in the shared target:

```text
cargo +1.95.0 rustc --locked --manifest-path PATH_TO_DIOXUS_CLI_0_7_10/Cargo.toml --features disable-telemetry --bin dx -- -C opt-level=1
```

The qualified tool build sets `CARGO_BUILD_JOBS=1`, `CARGO_TARGET_DIR` to the shared target and
`CARGO_PROFILE_DEV_DEBUG=0`. Level-one optimization applies to the CLI entry crate; dependencies
retain their ordinary development settings. An entirely unoptimized CLI has separately exhausted
its own main-thread stack during `serve` startup. This tool build policy does not change the
consumer profile or its stack reserve.

After an initial Submitted frame, the consumer changes its constructor counters from 37 to root
41 and child values 43, 47 and 53 through their existing owners. The driver waits for those values
in a Submitted frame before changing the copied panel's render bodies, including colors, labels
and returned recipe values. Success requires the new pixels and values in a newer Submitted
generation with the same process-start nonce, window, entities, mutated counters and initialization
counts. After the initial state operation the polling observer does not notify or refresh.

Finally a stop file asks the App to remove its window, observe entity release, and quit normally.
The stock Windows platform exits the process; the fixture records retirement and the quit callback
before exit. The driver retains an exact process handle and requires exit code zero before writing
success. It makes no in-process assertion after `Application::run`. The launched dx process is
assigned to a kill-on-close Windows Job while still suspended, so final cleanup covers its own
descendants even if dx exits early. Only the developer server is forcibly cleaned up after a
successful application exit; forced application termination is failure cleanup.

The artifact directory retains the driver log, raw JSONL observations and a successful result
document. The single observed edit-to-result duration includes compiler, transport, redraw and
the 100 ms sampling interval; it is not a latency distribution or performance budget.

## Native Rejection And Disconnect

Using the same dx-built base executable, install the optional test dependency from
`requirements.txt` and run:

```text
python -m pip install -r tests/fixtures/hot-reload-consumer/requirements.txt
python tests/fixtures/hot-reload-consumer/failures.py --app PATH_TO_BASE_CONSUMER_EXE --artifacts PATH_TO_SHARED_TARGET
```

The mature `websockets` test server receives the actual build/PID/ASLR handshake and sends a
wrong-build patch, a wrong-PID patch, an exact-process nonexistent DLL, a full-reload command
and a graceful disconnect. Live checkpoint requests observe the existing native frame without
notification or redraw. Every checkpoint must retain the original mutated state, entity/window
identity, exact generation and pixels; expected load failure/restart/disconnect logs are checked.
Mismatch silence is observed after a protocol pong and a bounded 0.5-second settle. This does
not prove behavior under an arbitrarily stalled receiver. The process must release state,
complete shutdown and quit with exit code zero before the result is written.

Only compatible render-body edits are exercised. Arbitrary type-layout migration, custom Element
phase patching, callback replacement outside explicit Subsecond boundaries, patching framework
crates outside this consumer workspace, browser/native foreign-platform execution and an engine
Editor reload workflow require separate qualification. A rejected or disconnected development
connection is reported by the framework; a full-reload command requests an application restart.
