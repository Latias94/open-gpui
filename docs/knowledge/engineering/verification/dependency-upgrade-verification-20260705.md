---
type: "Verification Evidence"
title: "Workspace dependency upgrade verification"
description: "Verification evidence and follow-up findings for the 2026-07-05 workspace dependency upgrade."
timestamp: 2026-07-05T20:24:03+08:00
tags: ["dependencies", "verification", "nextest", "macos", "xtask"]
status: "verified"
git_branch: "refactor/ui-framework-non-overlay-depth"
git_commit:
  - "50f7cfc build(deps): upgrade workspace dependencies"
  - "05f63de test(verification): restore gates after dependency upgrade"
---

# Summary

The workspace dependency upgrade landed in `50f7cfc` and was followed by verification-gate fixes in
`05f63de`.

The upgrade intentionally keeps `core-graphics = 0.24` and `core-text = 21` because
`open-gpui-font-kit 0.14.3` still exposes native CoreText/CoreGraphics FFI types from those
versions. Upgrading those two crates causes `open-gpui-macos` type mismatches, so they remain the
only direct outdated root dependencies reported by `cargo outdated --workspace --root-deps-only`.

# Verification

- `cargo check -p open-gpui-macos --all-targets -j 1`: passed.
- `cargo check -p open-gpui-scheduler -p open-gpui-wgpu -p open-gpui-ui-components -p xtask --tests -j 1`: passed.
- `cargo check -p open-gpui-wgpu --target wasm32-unknown-unknown -j 1`: passed.
- `cargo check -p open-gpui-web --target wasm32-unknown-unknown -j 1`: passed after making the
  stable wasm default single-threaded.
- `cargo check -p open-gpui-platform --target wasm32-unknown-unknown -j 1`: passed.
- `(cd crates/gpui_web/examples/hello_web && cargo check --target wasm32-unknown-unknown -j 1)`:
  passed on nightly with the example's shared-memory / atomics configuration.
- `cargo nextest run -p open-gpui-scheduler --no-fail-fast -j 1`: passed 27/27 after rebuilding the stale scheduler test binary.
- `cargo nextest run -p open-gpui-scheduler -p open-gpui-ui-components -E 'test(/public_surface|overlay|choice|navigation|scheduler/)' --no-fail-fast -j 1`: passed 97/97.
- `cargo nextest run -p open-gpui-ui-foundation-gallery --test foundation_gallery --no-fail-fast -j 1`: passed 103/103.
- `cargo run -p xtask -- scan-theme-drift`: passed.
- `cargo run -p xtask -- scan-import-boundary`: passed after switching xtask TOML parsing to `toml::from_str`.
- `cargo run -p xtask -- scan-ui-contract`: passed.
- `cargo fmt --all --check`: passed.
- `cargo check -p xtask`: passed.
- `cargo test -p xtask`: passed 29/29.
- `git diff --check`: passed.

# Follow-Up Fixes

- `examples/ui-foundation-gallery/tests/foundation_gallery/foundation_contracts.rs` now accepts
  `open_gpui_platform = { workspace = true, features = ["font-kit"] }` while still proving the
  gallery stays foundation-scoped.
- `xtask/src/import_boundary.rs` now uses `toml::from_str::<toml::Value>(...)` for full TOML
  documents. `contents.parse::<toml::Value>()` fails against `toml 1.1` for workspace manifests and
  lockfiles.
- `open-gpui-web` now defaults to the stable single-threaded wasm path. The `multithreaded` feature
  still exists for the SharedArrayBuffer / `wasm_thread` path, but it requires nightly because
  `wasm_thread 0.3.3` enables `stdarch_wasm_atomic_wait`.
- `open-gpui-platform` exposes `web-multithreaded` to opt back into
  `open_gpui_web/multithreaded`; the `hello_web` example enables that feature because its local
  toolchain and rustflags are already configured for shared-memory wasm.
- `open-gpui-web` no longer assumes absent local font assets at compile time. Until web font assets
  or a runtime font registration API are added, `WebPlatform` starts without bundled fonts and logs
  that state.

# Environment Notes

- Several apparent nextest hangs were traced to macOS dyld/Gatekeeper validation. Sampled test
  binaries were stopped at `_dyld_start` with tiny resident memory before Rust test harness entry.
- Rebuilding the affected test binary restored normal execution. The same Mach-O copied to `/tmp`
  ran immediately, which supports an inode/path/system-validation issue rather than a Rust code
  defect.
- Avoid running many first-time nextest discovery jobs concurrently on macOS when `syspolicyd` is
  busy. If a test binary appears stuck in `--list --format terse`, sample the process before
  treating it as a test failure.

# Remaining Constraints

- `open-gpui-web --features multithreaded --target wasm32-unknown-unknown` still requires nightly
  because of `wasm_thread 0.3.3`.
- The nightly shared-memory path emits Rust's expected warning that `-Ctarget-feature=+atomics` is
  not stably supported.
- Browser runtime verification is still separate from compile verification; `hello_web` has only
  been compile-checked, not driven through Trunk/WebGPU in a browser.
- Web text rendering still needs a follow-up decision: commit bundled font assets, expose a runtime
  font registration path, or integrate browser font loading with the text system.
- Windows and Linux cross-target checks require platform toolchains not present in the local macOS
  environment.
