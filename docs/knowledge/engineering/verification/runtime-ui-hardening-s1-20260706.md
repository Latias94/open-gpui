---
type: Verification Note
title: Runtime UI Hardening S1-S2 Verification
date: 2026-07-06
---

# Runtime UI Hardening S1-S2 Verification

This note records the first execution slice from
`docs/plans/2026-07-06-001-refactor-runtime-ui-hardening-plan.md`.

## Scope

- Windows platform lifecycle fallback for `hide_other_apps` and `unhide_other_apps`.
- Web dispatcher mode facts for stable single-threaded fallback and optional multithreaded
  shared-memory execution.
- Stable wasm package checks for the web backend surface.
- Docking capability parity audit for platform-window unsupported routes and runtime diagnostics.

## Behavior

- `WindowsPlatform::hide_other_apps` and `WindowsPlatform::unhide_other_apps` no longer call
  `unimplemented!()`. Windows does not claim macOS-style hide-other-apps behavior; the methods are
  debug diagnostic/no-op fallbacks under the existing `Platform` trait shape.
- `WebDispatcherMode` is a web-backend-local typed fact. `WebDispatcher::mode()` reports the mode,
  and `WebPlatform::dispatcher_mode()` caches the same fact for diagnostics and tests without
  changing the generic `PlatformDispatcher` trait.
- Stable web builds report a single-threaded fallback reason when the `multithreaded` feature is
  absent. Feature-enabled builds still require caller opt-in and browser shared-memory support
  before workers start.
- If worker startup fails after capability checks pass, Web dispatcher initialization degrades to
  `SingleThreaded { reason: WorkerStartupFailed }` and reports the actual worker count when only a
  subset starts.
- Browser scheduling still preserves only the realtime-vs-deferred distinction on the
  single-threaded path. Full priority-queue draining and dedicated realtime worker execution remain
  named limitations rather than implicit TODOs.

## Verification

Observed local gates:

```sh
cargo fmt --all
cargo check -p open-gpui-web --target wasm32-unknown-unknown --tests --locked -j 1
cargo check -p open-gpui-web --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-platform --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-wgpu --target wasm32-unknown-unknown --locked -j 1
cargo check -p open-gpui-windows --all-features --locked
cargo nextest run -p open-gpui-docking host_viewport_route host_viewport_platform_capability viewport_runtime --no-fail-fast
cargo check -p open-gpui-ui-core --tests --locked
cargo check -p open-gpui-ui-components --locked
cargo check -p open-gpui-ui-foundation-gallery --locked
cargo run -p xtask -- scan-ui-contract
cargo run -p xtask -- scan-import-boundary
cargo check --workspace --locked
(cd crates/gpui_web/examples/hello_web && cargo check --target wasm32-unknown-unknown -j 1)
```

Results:

- `open-gpui-web` stable wasm check passed.
- `open-gpui-web` stable wasm test-target check passed, including dispatcher mode-selection
  regression coverage.
- `open-gpui-platform` stable wasm check passed.
- `open-gpui-wgpu` stable wasm check passed.
- `open-gpui-windows --all-features` passed on the local host as a crate-configuration smoke.
- Docking focused capability/route/runtime nextest ran 191 tests: 191 passed, 707 skipped.
- `open-gpui-ui-core --tests` passed as the local motion-boundary compile gate.
- `open-gpui-ui-components` and `open-gpui-ui-foundation-gallery` ordinary checks passed.
- `scan-ui-contract` and `scan-import-boundary` passed.
- `cargo check --workspace --locked` passed.
- `hello_web` nightly shared-memory/multithreaded compile check passed with the expected
  `-Ctarget-feature=+atomics` warning, covering the feature-gated worker startup path.

The Windows GitHub Actions job remains the final owner for Windows API-path coverage because the
local host was macOS.

## Local Nextest Limitation

The local macOS host repeatedly stalled in test-binary list stage for:

```sh
cargo nextest run -p open-gpui-ui-core motion spring projection policy --no-fail-fast
cargo nextest run -p open-gpui-ui-components --test public_surface --no-fail-fast
cargo nextest run -p open-gpui-ui-foundation-gallery component overlay --no-fail-fast
```

Direct `cargo test -p open-gpui-ui-core motion -- --list --format terse` stalled in the same
`open_gpui_ui_core` test-binary listing path. This matches the repository's existing macOS
dyld/Gatekeeper list-stage limitation. The session interrupted those stuck runs and used compile,
scan, docking nextest, stable wasm, and workspace checks as local coverage; CI remains responsible
for completing platform-hosted test execution.

## Follow-Up

- S3 and S4 remain audit-first: add code only when motion or component-contract drift is found.
