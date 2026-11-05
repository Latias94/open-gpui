# Zed GPUI Upstream Sync Candidates

**Date**: 2026-07-05
**Plan**: `docs/plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md`
**Open GPUI baseline**: `e05eaec01a741fa9f6ffe587253c164e72f6dc43`
**Planning commit**: `da849a0779fe278e706559c4c947e49a523faf3a`
**Zed reference**: `repo-ref/zed`
**Zed reference HEAD**: `e3b73c6b30cdc09e820823fe44542b89850d4be1`

This document freezes the upstream candidate queue for the 2026-07-05 GPUI sync.
Implementation units consume this list instead of expanding scope from a moving `repo-ref/zed`
HEAD.

## Selection Rules

- Port only Apache-2.0 GPUI framework closure behavior that belongs in this workspace.
- Preserve Open GPUI package names, workspace aliases, `open-gpui-scap`, the Open GPUI-owned
  `font-kit` fork, and the crates.io `wgpu` migration.
- Do not import Zed editor/product crates or retired Zed dependency forks.
- Every accepted candidate needs an owning unit and a verification path.

## Accepted Candidates

| Upstream commit | Owning unit | Upstream intent | Open GPUI mapping | Test source |
|---|---|---|---|---|
| `552fc9f3c3` | U2 | Fix streamed reqwest request bodies being truncated after `Poll::Pending`. | `crates/reqwest_client/src/reqwest_client.rs`. Keep public `reqwest = 0.12.15` migration. | Upstream streamed-body regression adapted to local inline tests. |
| `485aeabff3` | U2 | Drop stale reqwest connections with keepalives. | `crates/reqwest_client/src/reqwest_client.rs`. Use only public reqwest builder APIs available in this workspace. | Focused client construction and existing request tests. |
| `bda5ac3626` | U3 | Prevent Wayland clipboard reads from blocking indefinitely. | `crates/gpui_linux/src/linux/platform.rs` and Wayland clipboard call sites. | Timeout and chunked-read tests where host support allows. |
| `c56646ffdf` | U3 | Keep IME candidate window following cursor in TUI/composition paths. | `crates/gpui_linux/src/linux/wayland/client.rs`. | Wayland IME candidate-position behavior, plus compile gate if runtime unavailable. |
| `f4364d870e` | U3 | Add `open_window` support to Linux headless client. | `crates/gpui_linux/src/linux/headless/*`. | Headless platform-window creation test. |
| `dae3e574e4` | U3 | Improve missing X11/Wayland feature error. | `crates/gpui_linux/src/linux.rs` and feature matrix docs. | Linux feature matrix, including no-default and single-backend combinations. |
| `9ac117693b` | U4 | Keep caption button hit testing for immovable Windows windows. | `crates/gpui_windows/src/events.rs`. | Focused hit-test behavior or compile-backed test if native event test is impractical. |
| `5aa6e8a0b3` | U4 | Avoid Windows Credential Manager blob-size overflow. | `crates/gpui_windows/src/platform.rs`. | Oversized credential error test with redaction assertions. |
| `ee571d3c69` | U5 | Add GPUI system wake callback. | Shared `Platform` trait and all Open GPUI backends. | Test-platform wake callback regression. |
| `eb87750323` | U5 | Add missing Windows dependency feature for wake support. | `crates/gpui/Cargo.toml`, `crates/gpui_windows/Cargo.toml`, or root workspace dependency features as needed. | Windows all-features check plus import-boundary scan. |
| `7fd5ea4bf3` | U6 | Rebase pending `ListState` scroll after remeasurement. | `crates/gpui/src/elements/list.rs`. | Upstream list scroll regression adapted to local tests. |
| `b14229f1a0` | U6 | Fix `TestScheduler::spawn_dedicated` leak due to cycle. | `crates/scheduler/src/test_scheduler.rs`. | Scheduler leak regression. |
| `c642b422de` | U7 | Use Windows Job Objects to reap spawned process trees. | `crates/util/src/process.rs` and manifest features. | Process-tree cleanup and containment tests. |
| `d1f500edf1` | U7 | Resolve binary names against custom `PATH` on macOS. | `crates/util/src/command/darwin.rs`. | Compile-backed Darwin path-resolution review; runtime gate needs macOS. |
| `f791aa57d7` | U8 | Bump `resvg`/`usvg` and add SVG panic regression. | `crates/gpui/Cargo.toml`, `Cargo.lock`, `crates/gpui/src/svg_renderer.rs`. | Split-glyph SVG regression plus renderer smoke. |

## Already Covered or No-Op

| Upstream commit | Decision | Rationale |
|---|---|---|
| `35eaeb94a7` | Already covered | Wayland app-id first-commit behavior is already present locally. |
| `7c3160b7bf` | Already covered | Wayland startup activation token handling is already present locally. |
| `0d8a4d4292` | Already covered | Atlas tile-space cleanup is already present locally. |
| `34cd17ff5e` | Already covered | `BoxShadow` builder API is already present locally. |
| `c899d5b590` | Already covered | `Rgba::alpha` is already present locally. |
| `03a8544040` | Already covered | Middle-truncation behavior is already present locally or outside this sync's framework surface. |
| `a873cf402c` | No-op for this sync | The workspace no longer has a `gpui` dependency on `async-process`; only a comment references `async_process`. |

## Rejected or Deferred Candidates

| Upstream commit | Decision | Rationale |
|---|---|---|
| `2882636c06` | Rejected | The hanging-updates change was reverted upstream by `b5c2d8a13f`; do not port a reverted behavior. |
| `b5c2d8a13f` | Rejected | Revert commit confirms `2882636c06` should not enter the candidate queue. |
| `a923597341` | Rejected | Agent terminal sandbox behavior is Zed product/app scope, not Open GPUI framework scope. |
| `479bce0995` | Rejected | Remote-server telemetry is outside the GPUI framework import boundary. |
| `ccf4058b7a` | Rejected | Picker preview and resizing are Zed UI/product behavior, not framework closure sync. |
| `10628c3d2c` | Rejected | Agent UI in-thread search is outside Open GPUI's imported framework closure. |
| `6076ce2738` | Deferred | Markdown benchmark work may be useful later but is not a correctness fix in this plan. |
| `362035d52a` | Rejected | Folder-opening suffix behavior is editor/workspace product scope. |
| `83d4847462` | Rejected | Settings UI accessibility is product UI scope. |
| `c7ad65e468` | Rejected | Editor highlight optimization is outside the GPUI framework closure. |
| `1722fe63bc` | Rejected | Editor multi-cursor optimization is outside the framework closure. |
| `138139f830` | Deferred | macOS traffic-light hitbox behavior appears mostly absorbed locally; revisit only if a focused diff proves a remaining framework regression. |
| `60ed56b372` | Deferred | Miscellaneous macOS process spawning fixes need a separate utility audit after U7. |
| `e1bfcf85db` | Deferred | macOS process file-descriptor leak fix needs a separate utility audit after U7. |
| `137e677a05` | Deferred | Multiple-window Wayland IME handling may overlap U3, but it needs a focused diff before expanding the accepted queue. |
| `8036a3c74b` | Deferred | Non-breaking glue character text behavior is not in the high-priority correctness queue; revisit after accepted fixes land. |

## Verification Baseline

- `cargo run -p xtask -- scan-import-boundary` passed before implementation started.
- `git diff --check` passed after writing the plan.

## November 5 Maintained-Framework Successor

The owner is resuming development for a game-engine Editor consumer. The inspected development
baseline is `89cb76ba1d59904d5c670f770aa8e30fe0346178` on
`optimize/docking-graph-transaction`; its canonical-docking cutover is still in progress. The July
queue above retains its original scope and is not a statement that the fork contains all later
upstream work.

The newly recorded Zed baseline is `635026fabc38440fce5e1767d125026a17b12012`, observed from the
upstream remote on November 5. Source inspection first used the cached November 3
`dd3437011469d5c2ec8c821a84cc999979625697` tree and checked the relevant latest blobs. These are
source findings. The arena and TextSystem successors below now have the executed scope in the
[verification record](../verification.md#textsystem-pool-lifetime); the other three units remain
queued. Do not overwrite framework or docking files from the upstream tree.

| Priority and owning unit | Confirmed difference from the development baseline | Upstream change and required verification |
| --- | --- | --- |
| Implemented: element arena custody | The baseline stored the arena inline; TLS/raw-pointer scopes neither deferred nested clearing nor established allocation stability. | [`ec3d887507`](https://github.com/zed-industries/zed/commit/ec3d887507f272119d9fe146c685f0a941d0e798), [`2529740084`](https://github.com/zed-industries/zed/commit/2529740084edf8c736e9d316edbcf56d40c1cf0f). App now owns a separate Rc allocation, draw guards retain it and clear tokens use weak identity plus `clear(cx)`. Nested-window, unwind, wrong-App and allocator Miri tests pass. The fork retains AppCell native delivery; its Windows IME maintenance now uses the existing post-borrow command queue, and forced refresh survives throttling. See the verification record for executed cohorts and the unqualified DirectManipulation timing comparison. |
| Implemented: TextSystem lifetime | The baseline wrapper pool stored LineWrapper values that strongly retained the same TextSystem. | [`e4a3732b94`](https://github.com/zed-industries/zed/commit/e4a3732b94eb0881e1d954201ed41ffd2abf18f6). The pool now retains only CharWidthCache, preserving resolved-font/size partition and ASCII/Unicode reuse. The original leak was reproduced, then wrapper/wrap/truncation and public headless/native reconstruction regressions passed. The latter exposed an independent stale Windows UISettings factory after apartment recreation; the native query owner now acquires that factory locally. See the verification record for actual cohorts and limits. |
| Cached-view dependencies | Whole-frame entity-set differences miss earlier reads; paint/deferred accesses and inherited opacity are absent from the retained dependencies. | [`decd2399b1`](https://github.com/zed-industries/zed/commit/decd2399b180b64dea2a4182f9ac63545a45e7a3), [`8fa085e397`](https://github.com/zed-industries/zed/commit/8fa085e397fd106166eb055f74a6e9d5e674d02f). Adapt scoped accesses, released-entity filtering, deferred/paint replay and opacity with accessed-scope, deferred subscription and inherited-opacity regressions. Preserve the fork's additional presentation/transform/clip/reveal validity. |
| Text layout correctness | DirectWrite drops zero-glyph advances and lacks the line-separator/default-feature behavior; wrapped hit testing can unwrap an out-of-line floating-point result. | [`0294cfb58e`](https://github.com/zed-industries/zed/commit/0294cfb58e8e24d12be7596f56bdb7a53e3dfd2f), [`979cd3fdf0`](https://github.com/zed-industries/zed/commit/979cd3fdf02ffa39131b33507cbd7375a68910fd), [`ac6818fb2d`](https://github.com/zed-industries/zed/commit/ac6818fb2de7aaec9ee00d4ff697e948c8b8f0a3), [`e71963a599`](https://github.com/zed-industries/zed/commit/e71963a599c64c213aca1c607e7418503a1e787d). Use native DirectWrite advances as the tab oracle, test both separator forms and default/explicitly-disabled ligatures, and retain the wrapped-line trailing-position regression. |
| Pending input and blur | Standalone modifier keystrokes miss interception/timeout handling, and explicit empty-focus adoption lacks the same pending-input settlement as focus replacement. | [`bc7041c600`](https://github.com/zed-industries/zed/commit/bc7041c600e3da82713849f4d65aa4a3cac89f67), [`ba42ab9d98`](https://github.com/zed-industries/zed/commit/ba42ab9d9851f82a7d8f5ae40b941bf5c515fc10), [`f66ed399cd`](https://github.com/zed-industries/zed/commit/f66ed399cdde86092af8af3dc7b418abf45f37f8). Port recognized-keystroke and focus-commit behavior with standalone-modifier, observation suppression and pending-input focus/blur regressions, including refused candidate and closed-window controls. |

Implement each row as a complete ownership/correctness change with focused tests and the affected
presentation, focus, portal and docking regressions. Preserve the accepted contracts, while
challenging implementation complexity when a deeper local module removes it. In particular,
renderer-submitted semantics, cross-channel subtree suppression, native mutation generations and
the existing docking transaction work must not disappear during an upstream update.

Already-covered work is not new benefit: `ListState::rebase_pending_scroll` is connected locally,
and the dedicated-task scheduler no longer has the earlier strong-reference-cycle shape. Record
these equivalences instead of reapplying their July candidates.

A subsequent coherent text refresh should assess resolved-font fallback caches together with
font-generation and layout invalidation, UAX #14 line breaking, and text synchronization/API
changes. The source baseline also advances Taffy from 0.10 to 0.15 and selects Rust 1.98.1. These
are migration inputs, not a reason to copy the upstream manifest or silently change Open GPUI's
declared Rust 1.92 floor. Validate the actual compiler/dependency requirement per owning change.
Completing a few correctness ports does not qualify the complete upstream refresh.
