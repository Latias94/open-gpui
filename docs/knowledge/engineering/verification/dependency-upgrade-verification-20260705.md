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

- `open-gpui-web` default wasm checks still require a nightly-capable `wasm_thread` path.
- `open-gpui-web --no-default-features` still needs the expected local font assets and existing
  feature-cfg cleanup before it can be a reliable gate.
- Windows and Linux cross-target checks require platform toolchains not present in the local macOS
  environment.

