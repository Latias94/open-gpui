# Zed Fork Dependency Audit

**Date**: 2026-06-06
**Status**: In progress
**Related**: [ADR 0001](../adr/0001-open-gpui-fork-strategy.md), [Verification](../verification.md)

## Problem

Open GPUI still depends on a small set of Zed-maintained external forks after the initial clean
workspace import. The remaining forks are intentionally allowed by the current import-boundary
scan, but they remain follow-up debt because they keep framework builds coupled to Zed-controlled
git repositories.

The goal is to replace, justify, or own each fork without weakening the clean license and dependency
boundary established by ADR 0001.

## Current Dependency Map

```mermaid
flowchart TD
    Scap[zed-scap] --> Gpui[gpui screen-capture feature]
    Scap --> GpuiLinux[gpui_linux screen-capture feature]
    Scap --> GpuiWindows[gpui_windows screen-capture feature]
    FontKit[zed-font-kit] --> GpuiFont[gpui font-kit feature]
    FontKit --> GpuiMacos[gpui_macos]
    FontKit --> GpuiWgpu[gpui_wgpu font matching]
    Wgpu[Zed wgpu fork] --> GpuiWgpu
```

## Resolved Forks

| Fork | Resolution | Compatibility note | Evidence |
| --- | --- | --- | --- |
| `zed-reqwest` | Replaced with crates.io `reqwest = "=0.12.15"`. | Zed's fork added per-request `RequestBuilder::redirect_policy`; `reqwest_client` now caches same-config clients with the requested upstream client-level redirect policy when a request carries `RedirectPolicy`. | `cargo check -p reqwest_client`; `cargo nextest run -p reqwest_client` |
| `zed-scap` transitive `windows-capture` drift | Kept `zed-scap`, but pinned the lockfile back to `windows-capture = 1.4.0`. | `zed-scap` declares `windows-capture = "1.3.6"`, which allowed Cargo to select `1.5.0`; `1.5.0` changed the Windows capture API and broke `gpui_windows --all-features`. Version `1.4.0` matches the fork's `Context`, cropped-buffer, and five-argument `WCSettings::new` usage. | `cargo check -p gpui_windows --all-features`; `cargo tree --workspace --all-features --target all --edges normal -i windows-capture@1.4.0` |
| `zed-xim` | Replaced with crates.io `xim = "=0.5.0"`. | Zed's fork exposed a `Client::reset_ic` helper. Upstream `xim 0.5.0` removed that helper but still exposes `ClientCore::send_req` and `Request::ResetIc`, so `gpui_linux` now sends the same XIM reset request directly. | WSL Ubuntu: `cargo check -p gpui_linux --all-features` |

## Inventory

| Fork | Current source | Reverse dependency evidence | Public candidate | Risk | Recommendation |
| --- | --- | --- | --- | --- | --- |
| `zed-scap` | `zed-industries/scap`, package `zed-scap`, version `0.0.8-zed` | `gpui`, `gpui_linux`, `gpui_windows` under `screen-capture` / all-features | `scap = 0.1.0-beta.1` from crates.io search | High | Keep the Zed fork for now. The current crates.io package fails to compile on Windows with its own `windows-capture = 1.5.0` dependency. |
| `zed-font-kit` | `zed-industries/font-kit`, package `zed-font-kit`, version `0.14.1-zed` | `gpui`, `gpui_macos`, `gpui_wgpu` | `font-kit = 0.14.3` from crates.io search | High | Defer until text/font rendering has stronger coverage; this touches font matching and platform font behavior. |
| Zed `wgpu` fork | `zed-industries/wgpu.git`, version `29.0.3` | `gpui_wgpu` | `wgpu = 29.0.3` from crates.io search | High | Defer. The version line matches crates.io, but the fork may carry unpublished patches in rendering internals. Compare lockfile and API behavior before replacing. |

## Evidence Commands

The initial audit used:

```sh
cargo tree --workspace --all-features --target all --edges normal --invert zed-reqwest
cargo tree --workspace --all-features --target all --edges normal --invert zed-scap
cargo tree --workspace --all-features --target all --edges normal --invert zed-font-kit
cargo tree --workspace --all-features --target all --edges normal --invert zed-xim
cargo tree --workspace --all-features --target all --edges normal --invert wgpu
cargo search reqwest --limit 5
cargo search scap --limit 5
cargo search font-kit --limit 5
cargo search xim --limit 10
cargo search wgpu --limit 5
```

Additional `zed-scap` migration probe on 2026-06-06:

```sh
cargo update -p zed-scap --precise 0.1.0-beta.1
cargo check -p gpui --all-features
cargo check -p gpui_windows --all-features
cargo check -p gpui_linux --all-features --target x86_64-unknown-linux-gnu
cargo info scap
cargo info windows-capture
```

Result:

- `scap = 0.1.0-beta.1` is the current crates.io release.
- On Windows, `scap` fails before Open GPUI-specific code is checked because it calls
  `windows_capture::frame::Frame::timespan()`, which is not present in `windows-capture = 1.5.0`.
- The same Windows path also calls `WCSettings::new` with the display branch argument order from an
  older `windows-capture` API.
- The Linux cross-check did not run because `x86_64-unknown-linux-gnu` is not installed in this
  Windows toolchain, but the Windows compile failure is already sufficient to block this migration.
- The probe was reverted; `Cargo.toml` and `Cargo.lock` continue to use `zed-scap`.

Follow-up lockfile check:

- The existing `zed-scap` fork also fails with `windows-capture = 1.5.0`, but for a different API
  shape: its Windows path still uses the five-argument `WCSettings::new`.
- `windows-capture = 1.3.6` is too old for the fork because it lacks `capture::Context` and exposes
  `FrameBuffer::as_raw_nopadding_buffer()` instead of `as_nopadding_buffer()`.
- `windows-capture = 1.4.0` matches the fork and restores `cargo check -p gpui_windows --all-features`.

Additional `zed-xim` migration probe on 2026-06-07:

```sh
cargo info xim
cargo update -p zed-xim --precise 0.5.0
cargo tree --workspace --all-features --target all --edges normal --invert xim
wsl -d Ubuntu -- bash -lc 'cd /mnt/f/SourceCodes/Rust/open-gpui; export CARGO_TARGET_DIR=/tmp/open-gpui-target-linux; cargo check -p gpui_linux --all-features'
```

Result:

- `xim = 0.5.0` is the current crates.io release and preserves the `x11rb-client` / `x11rb-xcb`
  feature split used by `gpui_linux`.
- The only compile break was the fork-only `Client::reset_ic` helper. Sending
  `Request::ResetIc` through `ClientCore::send_req` preserves the same protocol request.
- WSL Ubuntu verifies the Linux all-features build. It still emits two pre-existing
  `nightly_coverage` `unexpected_cfgs` warnings from `gpui_linux/src/linux/dispatcher.rs`.

## Alternatives Considered

### Option A: Replace all forks immediately

Pros:

- Removes Zed git dependency debt quickly.
- Forces incompatibilities into the open.

Cons:

- High regression risk across rendering, fonts, screen capture, and Linux input methods.
- Current CI only verifies Windows plus Cargo checks; it does not exercise Linux X11, Wayland,
  macOS font behavior, or screen capture runtime paths.

Decision: rejected for now.

### Option B: Replace forks in risk order

Pros:

- Keeps each change reviewable.
- Lets verification coverage grow around each subsystem before changing higher-risk dependencies.
- Preserves momentum while protecting core rendering and platform behavior.

Cons:

- Zed fork debt remains temporarily.
- Requires several small migration commits instead of one broad cleanup.

Decision: recommended.

### Option C: Keep the forks indefinitely and document them as supported dependencies

Pros:

- Lowest immediate engineering cost.
- Preserves imported behavior exactly.

Cons:

- Keeps long-term dependency control outside Open GPUI.
- Weakens the clean-framework positioning from ADR 0001.
- Makes future crates.io publication and supply-chain review harder.

Decision: rejected as a long-term strategy.

## Recommended Work Order

1. **`zed-scap`**: blocked on the current public crate. Revisit after an upstream `scap` release fixes
   the Windows `windows-capture` API mismatch, or after deciding to own a small Open GPUI patch/fork.
   Any retry should verify all-features builds for `gpui`, `gpui_linux`, and `gpui_windows`.
2. **`zed-font-kit`**: defer until text/font tests are stronger because it affects font matching and
   renderer behavior.
3. **Zed `wgpu` fork**: defer until a dedicated renderer compatibility lane can compare behavior
   against crates.io `wgpu = 29.0.3`.

## Success Metrics

| Metric | Target | Measurement |
| --- | --- | --- |
| Zed fork count | Reduced one fork at a time | `rg 'zed-scap|zed-font-kit|zed-industries/wgpu' Cargo.toml Cargo.lock` |
| Verification gate | Still passes after each migration | `cargo run -p xtask -- verify` |
| Focused package checks | Targeted crate builds after migration | `cargo check -p <crate>` |
| Runtime risk | No migration without relevant platform/runtime evidence | feature-specific smoke checks or documented limitation |

## Risks and Mitigations

| Risk | Severity | Likelihood | Mitigation |
| --- | --- | --- | --- |
| Public crate API differs from Zed fork | High | Medium | Compare with a focused manifest switch before changing code broadly. |
| Platform-specific regression is missed on Windows CI | High | Medium | Add Linux/macOS or feature-specific CI before migrating platform-sensitive forks. |
| Lockfile churn hides the actual dependency change | Medium | Medium | Commit one fork migration at a time with focused verification evidence. |
| Fork has unpublished behavioral fixes | High | Medium | Inspect fork delta or run compatibility tests before replacement. |

## Open Questions

- Should Open GPUI publish temporary internal forks under its own organization when upstream crates
  are not compatible yet?
- Which runtime checks should be added before touching `font-kit` or `wgpu`?
- Should Linux all-features CI be added as a permanent gate for future Linux dependency changes?
