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

The November continuation applies the
[official-baseline rule](../../.agents/skills/open-gpui-upstream-sync/SKILL.md#integrate-through-existing-owners).
Record retained differences and their consumer/verification basis here; an unimplemented
official mechanism remains queued until it receives an explicit disposition.

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
| `03a8544040` | Implemented: November 6 successor | The July classification was incorrect: Middle was absent in the inspected fork. The complete Unicode wrapping/truncation successor now adds the public style route and exact run redistribution; its qualification is linked below. |
| `a873cf402c` | No-op for this sync | The workspace no longer has a `gpui` dependency on `async-process`; only a comment references `async_process`. |

## Rejected or Deferred Candidates

| Upstream commit | Decision | Rationale |
|---|---|---|
| `2882636c06` | Rejected | The hanging-updates change was reverted upstream by `b5c2d8a13f`; do not port a reverted behavior. |
| `b5c2d8a13f` | Rejected | Revert commit confirms `2882636c06` should not enter the candidate queue. |
| `a923597341` | Split | Terminal product behavior stays outside the closure. The official shell-builder regression is imported; the fork already has the tested runtime behavior. Qualification is recorded below. |
| `479bce0995` | Split | Remote telemetry stays outside the closure. Public util::parse_os_release and official/public tests are implemented; scoped qualification is recorded below. |
| `ccf4058b7a` | Split | Picker product changes stay outside the closure. Public Rems helpers and complete key-dispatch regressions are implemented; scoped qualification is recorded below. |
| `10628c3d2c` | Split | Agent search stays outside the closure. Complete upward List normalization is implemented through the existing admitted commit; scoped qualification is recorded below. |
| `6076ce2738` | Deferred | Markdown benchmark work may be useful later but is not a correctness fix in this plan. |
| `362035d52a` | Split | File-existence fallback stays in product path derivation. The util parser already supports parenthesized positions; its official behavioral regression is imported and qualified below. |
| `83d4847462` | Split | Settings product migration stays outside the framework closure. The complete synthetic-child callback/builder/Div mechanism is implemented and qualified below. Its separate focus/keyboard mechanisms remain explicit successors; the product disposition rejects neither framework slice. |
| `c7ad65e468` | Split | Editor highlight consumers stay outside the closure. Complete measured TextLayout queries are implemented with owner/public/independent consumers; scoped qualification is recorded below. |
| `1722fe63bc` | Split | Editor multi-cursor consumers stay outside the closure. Parameterized benchmark, real text-system and dispatcher/teardown mechanisms join the existing benchmark/report successor below. |
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
source findings. The arena, TextSystem, cached-dependency, text-layout and pending-input
successors below now have executed scope in the
[verification record](../verification.md#pending-input-modifier-recognition-and-blur).
Those initial units do not exhaust the framework differences. Do not overwrite framework or
docking files from the upstream tree.

### Fixed Revisions And Provenance

The repository [upstream-sync skill](../../.agents/skills/open-gpui-upstream-sync/SKILL.md)
describes the repeatable comparison workflow. This document owns its changing revisions and
dispositions. The fork was extracted into independent history; no cross-repository merge base
establishes source parity.

| Role | Fixed revision and evidence | Meaning |
| --- | --- | --- |
| Initial Open-GPUI import | `c9571c486612f1bd9194faa028709e89e3f58b7e` | Its parent is the new workspace initialization. The import record names `repo-ref/zed` but records no upstream SHA or source-history parent. |
| Content-verified import reconstruction | Zed `6e9465a4288c332208643892e23b9d35d7be5c79` | The reference clone's creation reflog names this revision. Across the entire original import closure, both trees have 331 entries: 326 identical blobs and five import-policy adaptations in `sum_tree` and `util_macros`. Twenty-two license entries changed from symlink to ordinary-file mode without a blob change. This is a reproducible content anchor, not proof of the exact originally checked-out monorepo SHA. |
| Exact original Zed checkout | Unknown | The reconstruction anchor's parent `aeb8c1da93c45a4e06bfb1522be50a267812b1dd` has identical contents throughout the imported closure. Content equality cannot distinguish these monorepo revisions. |
| July comparison | Fork `e05eaec01a741fa9f6ffe587253c164e72f6dc43`; Zed `e3b73c6b30cdc09e820823fe44542b89850d4be1` | The frozen July queue led to U2-U8's 15 targeted correctness ports. It did not establish complete framework parity with that Zed revision. |
| Committed fork comparison used by the inventory | `229723e6205e8fc88da50ffabeb721c482d100e4` | Includes the arena and TextSystem successors and separate WinRT factory correction. The later cache implementation is not part of this fixed source inventory. Its qualification is recorded separately below. |
| Preliminary November source comparison | Zed `dd3437011469d5c2ec8c821a84cc999979625697` | The initially available November 3 source tree. It is superseded as the cycle target, not a completed integration baseline. |
| This cycle's fixed upstream target | Zed `635026fabc38440fce5e1767d125026a17b12012` | The November 5 comparison target. Completion requires disposition of framework differences plus the declared verification; the five initial correctness units alone do not close it. |
| Later remote observation | Zed `654efae441bc8eb117d0042bb28cd7cf3570bdc9` | Observed with `git ls-remote zed-upstream HEAD` on November 5. This advancing HEAD does not change the current cycle's target. |

The import comparison is reproducible from `git ls-tree -r` and blob hashes for the crates in the
[import record](zed-gpui-import.md). Compare the five changed files with `git diff` to distinguish
the recorded tracing/perf removals from upstream behavior. The reflog is local provenance evidence;
the content comparison can be repeated from the recorded revisions without that reflog.

The July implementation commits are `841913ce031ff2b6b741fda6721db070d47d11e9` (HTTP),
`a4593d250e476d7eb823319b3fa8959432da9724` (Linux),
`79088c4d4c6a7a464a680d441adc45669020e493` (Windows),
`2af24c07a43ca4437d92e9ba6ca68111b18880a8` (wake),
`2c381bcd7b0baaf9527ed91aec2e952a2eaa76e9` (list/scheduler),
`acbcfe933cdb0f06e04bf51e488d7da45069689b` (process/Darwin) and
`18162118c4a239fc4aec44f8f9979da549a639d0` (SVG). Their historical verification keeps its original
platform scope. Their presence does not prove that the July queue exhausted the earlier delta.

### Diff Inventory And Interpretation

The initial inventory compares three trees: historical upstream to the fixed target for additions,
the same historical revision to the committed fork for local changes, and the fork to the fixed target for the
remaining reconciliation. Read the upstream fix and its follow-ups after locating the owner.
The fork has moved substantial Window state into `window/` owner modules; a removed upstream line
can correspond to relocated local code. Package aliases and the independent manifest are also
intentional source differences.

A November 5 inventory of the original imported crate paths finds 210 changed Rust, manifest and
shader files between July upstream and the fixed target, with 59,309 added and 17,769 removed
lines. It excludes top-level `examples`, `benches`, `resources` and `tests` directories and counts
production source plus tests embedded under `src`. The same filter between the committed fork
and target finds 311 files, 73,381 additions and 164,111 removals. These are directional raw diff
sizes, not missing functionality, work estimates or a claim that local code should be removed.
The corresponding non-merge upstream log has 364 commits touching the imported paths. Its entries
still require semantic classification; these counts are an inventory, not a completed audit.
The content-verified import reconstruction to target, under the same filter, has 219 changed
files, 63,932 additions and 17,509 removals. Include that earlier range when reconstructing missing
behavior: July's selective ports did not establish a complete synchronized base. The incorrectly
classified `TruncateFrom::Middle` is one confirmed inherited omission, even though it was already
present in July upstream.

Expand the dependency closure when target source introduces an owner outside the original import
list. In particular, the Apple backend now shares `gpui_apple`; iOS is a separate platform choice.
The existing import-boundary policy determines which dependencies can enter the framework.

The initial ordering remains memory/lifetime, cache/deferred dependencies, text/input correctness,
then coherent text/layout/API and platform/render reconciliation. In every group, classify behavior
as missing, equivalent, intentionally divergent, superseded or outside the framework closure.
The five priority rows below are confirmed gaps, not an exhaustive statement about the 210 files.

| Priority and owning unit | Confirmed difference from the development baseline | Upstream change and required verification |
| --- | --- | --- |
| Implemented: element arena custody | The baseline stored the arena inline; TLS/raw-pointer scopes neither deferred nested clearing nor established allocation stability. | [`ec3d887507`](https://github.com/zed-industries/zed/commit/ec3d887507f272119d9fe146c685f0a941d0e798), [`2529740084`](https://github.com/zed-industries/zed/commit/2529740084edf8c736e9d316edbcf56d40c1cf0f). App now owns a separate Rc allocation, draw guards retain it and clear tokens use weak identity plus `clear(cx)`. Nested-window, unwind, wrong-App and allocator Miri tests pass. The fork retains AppCell native delivery; its Windows IME maintenance now uses the existing post-borrow command queue, and forced refresh survives throttling. See the verification record for executed cohorts and the unqualified DirectManipulation timing comparison. |
| Implemented: TextSystem lifetime | The baseline wrapper pool stored LineWrapper values that strongly retained the same TextSystem. | [`e4a3732b94`](https://github.com/zed-industries/zed/commit/e4a3732b94eb0881e1d954201ed41ffd2abf18f6). The pool now retains only CharWidthCache, preserving resolved-font/size partition and ASCII/Unicode reuse. The original leak was reproduced, then wrapper/wrap/truncation and public headless/native reconstruction regressions passed. The latter exposed an independent stale Windows UISettings factory after apartment recreation; the native query owner now acquires that factory locally. See the verification record for actual cohorts and limits. |
| Implemented: cached-view dependencies | The baseline's whole-frame set differences missed earlier reads, paint/deferred accesses and inherited opacity. Its per-round deferred vector also invalidated nested journal ranges. | [`5e982c6bdc`](https://github.com/zed-industries/zed/commit/5e982c6bdc315a6d2bb68b3edefae00ceedd35f4), [`decd2399b1`](https://github.com/zed-industries/zed/commit/decd2399b180b64dea2a4182f9ac63545a45e7a3), [`8fa085e397`](https://github.com/zed-industries/zed/commit/8fa085e397fd106166eb055f74a6e9d5e674d02f). Existing owners now isolate draw/callback reads, filter released generations, keep deferred positions stable and retain paint/deferred dependencies. Ordinary deferred content and portals inherit logical opacity; admitted hover predictions converge without changing layout latches. Upstream nested-popover and four-way subscription regressions are retained with fork-specific nested-window, refusal, clip and real HWND/Submitted controls. See the qualification for failures, final cohorts and the unchanged non-view/external-group notification limitations. |
| Implemented: text layout correctness | The baseline dropped zero-glyph tab advances, lacked single-baseline Unicode separator/default-feature behavior and could panic on a wrapped hit-test rounding gap. | [`0294cfb58e`](https://github.com/zed-industries/zed/commit/0294cfb58e8e24d12be7596f56bdb7a53e3dfd2f), [`979cd3fdf0`](https://github.com/zed-industries/zed/commit/979cd3fdf02ffa39131b33507cbd7375a68910fd), [`ac6818fb2d`](https://github.com/zed-industries/zed/commit/ac6818fb2de7aaec9ee00d4ff697e948c8b8f0a3), [`e71963a599`](https://github.com/zed-industries/zed/commit/e71963a599c64c213aca1c607e7418503a1e787d). Native advance/hit-position and distinct-font oracles, both separators, live typography and exact/closest/caret controls pass. The actual native text trajectory retains full source through paint, newer Submitted generations and teardown. Mutable callback context and nullable arrays take relevant `89e8a4b9ec` prerequisites; atlas/staging hardening remains separate below. See the qualification for red/final cohorts and pixel/input/performance limits. |
| Implemented: pending input and blur | The baseline missed standalone-modifier interception/observation and timeout, and retained pending input after empty-focus adoption. | [`bc7041c600`](https://github.com/zed-industries/zed/commit/bc7041c600e3da82713849f4d65aa4a3cac89f67), [`ba42ab9d98`](https://github.com/zed-industries/zed/commit/ba42ab9d9851f82a7d8f5ae40b941bf5c515fc10), [`f66ed399cd`](https://github.com/zed-industries/zed/commit/f66ed399cdde86092af8af3dc7b418abf45f37f8), with `dc1e815e47`, `a8cae3bd77` and `e463c5ab74` prerequisites. Recognized modifier releases now share interception/observation and singleton-prefix timeout. Existing focus/blur/candidate owners settle pending input once after admission and retain it on refusal. Active pending and replay validate the original focus/context, including user callbacks; completed live-window actions retain their observer report. See the [qualification](../verification.md#pending-input-modifier-recognition-and-blur) for red regressions, 24 focused scenarios, native handler evidence and explicit OS-input/platform limits. |

Implement each row as a complete ownership/correctness change with focused tests and the affected
presentation, focus, portal and docking regressions. Preserve the accepted contracts, while
challenging implementation complexity when a deeper local module removes it. In particular,
renderer-submitted semantics, cross-channel subtree suppression, native mutation generations and
the existing docking transaction work must not disappear during an upstream update.

Already-covered work is not new benefit: `ListState::rebase_pending_scroll` is connected locally,
and the dedicated-task scheduler no longer has the earlier strong-reference-cycle shape. Record
these equivalences instead of reapplying their July candidates.

### Source-Confirmed Follow-Ups Beyond The Initial Units

These rows come from the fixed-target source diff. Implemented successors link their executed
qualification; the remaining rows are queued reconciliation work. This is not an exhaustive
disposition of the 364 commits.

| Owning area | Target evidence and current fork difference | Required reconciliation |
| --- | --- | --- |
| Implemented: GPU primitive representation | `f28177003499ec9c5052d1883fee62b148451c59` replaces GPU-facing Rust `bool` with `PaddedBool32`. The fork now uses private initialized zero/one storage in both sprite and underline records, preserving its clip, tile-generation and transform ordering. | Shared field-width/value/offset checks and complete Core/WGPU execution cover the corrected upload ABI. DirectX now uploads the shared sprite slice without a duplicate layout or conversion vector. The [GPU qualification](../verification.md#gpu-primitive-flags-and-windows-color-transfer) owns actual Windows image/decoration/opacity/clip/cache pixels and shader-path results. Metal source/ABI assertions are updated; generated-header, shader and native execution on macOS remain unqualified. |
| Implemented: Windows color transfer | `b9419ae72814157164b09592ab2466f9f4709f09` corrects reversed linear/sRGB power directions through the existing HLSL owner. | The native old-shader regression fails on a non-endpoint Oklab pixel; the independent gamma-2.2 oracle passes with corrected directions, retaining sRGB/solid/alpha and newer-frame controls. See the [qualification](../verification.md#gpu-primitive-flags-and-windows-color-transfer) for actual compilation/execution paths and limits. This preserves the Windows approximation, selects no engine-wide color policy and establishes no performance result or byte parity with WGPU's piecewise transfer. |
| Implemented: native text/atlas hardening | `89e8a4b9ec7e0fa8a49987a3c1dbf29343dc6999` adds mutable DirectWrite callback context, nullable-array checks, mapped color-glyph staging release and atlas source-length checks. The text-layout unit takes the first two prerequisites. The native-resource successor now completes mapped staging release and shared checked atlas upload admission. | Both insertion APIs refuse malformed sources through `Err` before allocation/key/generation changes and validate the final destination before upload. A later refusal cancels only its unpublished allocation; published regions and exact-instance leases remain valid. The target's void short-buffer skip was intentionally replaced because its caller still publishes success. Color readback uses RAII Unmap and transparent target initialization through the existing raster owner. See the [native-resource qualification](../verification.md#native-atlas-upload-and-color-readback) for WARP multi-row/nonzero-origin pixels, retry/lease controls, debug-layer red/green and actual color-emoji readback. These changes establish neither GPU completion from an upload return nor full refresh completion. |
| Implemented: shared image decoding | Official `9552acc2bc` and fixed target `635026fabc` now share static/GIF/WebP decoding across encoded Image and resource loaders. The retired duplicate blocks were inherited official code, unchanged from the import anchor. | The [image qualification](../verification.md#shared-image-decoding-and-animation) records red EXIF/one-frame regressions, BGRA/alpha and asymmetric-orientation controls, real headless timelines, isolated completion notifications and native 0/1/0 Submitted/GPU-readback/retirement. Official contextual decoder errors now enter `ImageCacheError::Other`. Fork-only adaptations declare the source-confirmed image 0.25.4 API minimum, use the existing executor clock and include the exact loading deadline. Complete affected framework/public, component and independent suites pass 1,328/1,328, 2,964/2,964 and 12/12; four existing interactive Windows cases remain skipped. Locked checks, correctness Clippy, formatting and actual Windows Rust 1.92.0 affected-framework/consumer checks pass. This is not full image/platform or renderer-lifetime parity. |
| Image dependency features | Official `8e06c323ae063f3d245ea5c283d4bf373ad51db2` excludes AVIF from the image dependency's otherwise-default feature set. The fork retains the imported default features. | Reconcile this separate dependency policy with actual image-format consumers in the remaining closure audit. It is not a prerequisite for shared EXIF/WebP decoding; neither existing lockfile changes in that unit. Do not describe the full image feature surface as synchronized from decoder qualification. |
| Implemented: owning asset loads | Official `5896274168e06663568f42b5bae142162936e0a6` and fixed-target cleanup now flow through the owning CachedLoad, Option-returning App fetch, Window subscriptions and opaque ImageCacheItem. Shared task handles and per-use completion tasks are removed from App, Window, retain-all and the actual gallery LRU. | The [asset-load qualification](../verification.md#owning-asset-loads-and-view-completion) records the reproduced fork App-borrow failure and wait-before-publication adaptation. Focused owner/image/candidate/LRU/native cases pass 22/22. Each interested view receives one notification, passive reads do not redraw, eviction cancels old entries and accepted frame leases survive key removal. The dormant gallery now has an explicit platform example target and actual cache tests. Browser contention, numeric costs and complete framework/engine acceptance remain distinct unqualified scope. |
| Implemented: externally driven App ownership | Official `74798c68d5c` now supplies `Application::run_embedded` and the strong ApplicationHandle; launch reuses the existing run_platform owner. Handle update deliberately uses the fork's complete update transaction rather than official direct callback invocation. | The [application qualification](../verification.md#externally-driven-application-lifetime) records defer/batching red results, 10 owner cases and actual Windows visual-host Submitted/readback/retirement. Complete affected runtime/public, component and independent suites pass 1,339/1,339, 2,964/2,964 and 14/14; four existing interactive Windows cases remain skipped. Locked checks, correctness Clippy and actual Windows Rust 1.92.0 affected-framework/consumer checks pass. Stock Windows still owns a blocking process loop; native production external-loop support, shared GPU and separate display/hot-reload/asset owners are not implied. |
| Implemented: display and launch lifecycle | `f495dd0f77b69e287ed72c1168dc02965f08be7d` now supplies App-owned display observation, per-window association and cached refresh intervals, initialized from both launch callbacks. The fixed target retains that mechanism. | The [qualification](../verification.md#connected-displays-and-window-association) records the fork's ordered native ingress/immutable topology adaptation, official reciprocal-overflow correction, deferred-close reentry fix and ordinary required bitflags dependency. Corrected runtime/public/gallery, component and independent consumers pass 1,388/1,388, 2,964/2,964 and 18/18; four existing interactive Windows cases remain skipped. Source migration, actual Submitted frames and synthetic WM ingress retain distinct scope from physical hotplug/refresh-mode and non-Windows native qualification. |
| Implemented: refresh-aware measurement v3 | `4f6d97b9de7d9f167da8b7832eeecea22c5a9efa` and final cleanup now measure signal/dirty-to-submission work against each window's refresh interval, including view self-time and the complete v3 detector/monitor mechanism. Local implementation is `411895475e9e51a9f06954d5d676333d597b991f`. | The [qualification](../verification.md#refresh-aware-frame-and-hang-measurement-v3) retains official provenance, exact accepted/Submitted adaptations and four reproduced inherited sealer/active-use corrections. Complete framework/public/Gallery/state/native 1,809, components/docking/Canvas 2,966, disabled Core/public 1,134 and independent enabled/disabled 46/32 pass. Actual Windows Rust 1.92.0, stable/nightly production wasm and static checks pass. Numeric overhead, foreign native/browser execution and complete benchmark/report/debug-overlay parity remain separate. |
| Conditional GPUI hot reload | `670a779a5ed9e7161c97aa4f998d395c207d5631` initializes Subsecond hot reload under `cfg(gpui_hot_reload)` during both launch callbacks; the fork has no matching owner or dependency. | Compare compiler/build/runtime prerequisites and real stateful custom-panel replacement/recovery before selecting the target's mechanism. External-loop ownership is not hot reload, and source adoption alone does not qualify edit-to-result latency. |
| Atlas and renderer lifetime | Target `platform.rs` adds shared `AtlasBackend`/`AtlasState`; `96e95edac45` skips released-texture sprites, and WGPU adds texture generations and bind-group caching. The fork instead has per-instance lease generations and candidate-frame refusal. | Compare the lifetime behavior in the existing owner, including stale resource and rejected-frame controls. Preserve admitted candidate and lease semantics while evaluating deeper shared atlas code; copying upstream's skip policy would change the product contract. |
| Platform closure | Target macOS shares `gpui_apple` and uses `objc2`; Linux gains switchable headless/windowed modes and rendering; Web adds backend/focus/input changes. The native-font successor now corrects already-matched Windows selections and private fallback, including gaps retained by the fixed target. CoreText source adaptation has separate unexecuted-platform scope. | Map trait and manifest changes across the actual supported backend closure. The [native-font record](../verification.md#native-font-reselection-and-retained-faces) distinguishes actual simulation/face retention, submitted Windows frames and prepared Mac source. Treat iOS as a separate platform decision and retain unexecuted native-platform limits. Existing Windows-only evidence cannot qualify the other backends. |
| Public API evolution | The target changes event, headless-renderer, font/text and benchmark APIs. Its generic `View`/`ViewElement` path from `74b5207744ef95505fd75c56bf2c9d4736e931ee` coexists with `Render`/`RenderOnce`; AnyView embeds through ViewElement. The complete View interface, macro and cached-view migration is implemented and scoped qualification is recorded below. | Preserve ordinary public consumption and the independently locked consumer. The target's debug accessibility view-type map/dump pipeline and conditional hot reload retain separate owners; the interface migration alone does not qualify those mechanisms or complete API parity. |

A subsequent coherent text refresh should assess resolved-font fallback caches together with
font-generation and layout invalidation, UAX #14 line breaking, and text synchronization/API
changes. The source baseline also advances Taffy from 0.10 to 0.15 and selects Rust 1.98.1. These
are migration inputs, not a reason to copy the upstream manifest or silently change Open GPUI's
declared Rust 1.92 floor. Validate the actual compiler/dependency requirement per owning change.
Completing a few correctness ports does not qualify the complete upstream refresh.

The display successor adds actual wasm32-unknown-unknown production Core/WGPU/Web compilation.
Its Web test-target attempt stops in unchanged wait-timeout 0.2.1 test-support dependency before
the backend. The remaining platform/dependency closure must assess the fork's proptest 1.10.0
pin against target 1.12.0 and the supported test feature sets; a version difference alone is not
evidence that an upgrade fixes it. The [qualification](../verification.md#connected-displays-and-window-association)
keeps that failed attempt distinct from production compilation and unexecuted browser/native OS
scope.

### Profiler And Platform Prerequisite Order

The fixed-target source audit after `e5ad73a24a6` finds the following coherent dependency
order. These are framework mechanisms; Zed's GPL hang_telemetry, input_latency_ui and product
report publication remain outside the extracted closure. GPUI's Apache debug overlay is a
framework consumer and needs its own disposition rather than implicit exclusion.

1. Qualified unified profiler foundation: `48511e0b9c`, `c16d19c94d`, `a21007b7a9`, with the
   scheduler action clock `d4cd310edb`, draw-duration return `07a0bd1220` and
   dirty-to-present histogram `55007f518b`. Retain the existing Criterion API's actual Canvas
   consumer while mapping the official measurement scopes. Complete upstream benchmark report/
   metrics evolution (`7316cf7745`, `1f7eae7fb7`) remains an explicit consumer comparison. The
   local implementation `3e5a907775e9f22d0a092a0f9f27c6f1696ade0e` and its
   [qualification](../verification.md#unified-profiler-foundation) separate official baseline,
   fork candidate/input/reset/unwind adaptations, inherited trace/action corrections, complete
   public/native consumers, the unchanged-size benchmark fixture and unmeasured overhead.
2. Qualified native lifecycle observation: multi-subscriber wake `7a7c3e1d2f` and sleep/window
   visibility `3db02c2c77`. The [successor](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#sleep-wake-and-presentation-visibility-successor)
   preserves the complete official mechanism through existing native ingress and reusable public
   owners. Implementation `8b283cb151b070d7d5851a531a86e6d6d67196cb` removes the singleton
   wake slot and provides public test-context controls. Framework/public/gallery/native 1,446,
   component/docking/Canvas 2,966, disabled Core/public 1,113 and independent enabled/disabled
   28/26 pass. Actual Windows compiler-floor checks pass. The
   [qualification](../verification.md#sleep-wake-and-presentation-visibility) distinguishes
   official mechanisms, fork ingress/SetWindowPos adaptations, source-only foreign backends,
   portable production compilation and the remaining physical-power/browser/cost scope.
3. Qualified foreground journal and hang detection: `1861e58f98`, trigger rules `97b1e64a17`,
   sleep/visibility `f0fb48c71a` and unnotified suspend `7232ba04ef`. Preserve bounded writer,
   independent cursors, small-poll aggregation, explicit interval boundaries and demand deadlines.
   The [complete successor](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#foreground-journal-and-hang-monitor-successor)
   imports the official coherent checkpoint through `d284cbbc87`, including the monitor below.
   Local implementation `8493e9eb1cad7c3336fa99b4b8749c5be1d6c7c5` retains all 45 official
   test scenarios and public/module consumers. The [qualification](../verification.md#foreground-journal-and-hang-monitor)
   records complete runtime/public/gallery/native 1,531, components/docking/Canvas 2,966,
   disabled Core/public 1,114 and independent enabled/disabled 32/26 passing. Native Submitted/
   reconstruction, actual Windows Rust 1.92.0, stable wasm and existing nightly shared-memory
   compilation retain separate scopes. Measurement-v2 and later final-target signal/refresh
   changes remain distinct; accepted backlog, pending publisher tails and numeric costs stay open.
4. Qualified GPUI-owned hang monitor `0f9c923e67`, including `4c0ead4a3e` and `d284cbbc87` test
   corrections, App shutdown flush and background ownership. Source detection is post-hoc;
   it is not a watchdog for work that never returns.
5. Platform frame-signal provenance `cd4fc8de4c` and nonempty completed-skip sealing
   `36b6d0951f`: shared atomic first timestamp/source, actual backend clock conversion and
   feature-disabled zero-capture behavior. Display observation `f495dd0f77` is already qualified.
   The [complete successor](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#platform-frame-signals-and-completed-skips-successor)
   is qualified locally at `5811543bebe10657b9f99b42bfadb214e7dad999` through existing frame,
   journal and platform owners. Its [qualification](../verification.md#platform-frame-signals-and-completed-skips)
   records final Core/Windows 1,482, components/docking/Canvas 2,966, disabled Core/public 1,122
   and independent enabled/disabled 38/29 passing. Four interactive native skips and the earlier
   gallery fixture skip remain explicit; WGPU/gallery contributes 228 passing cases. Initial
   clipboard failure, unchanged rerun, interrupted run and final success retain separate records.
   Actual Windows Rust 1.92.0 and production wasm configurations compile; native foreign-platform,
   browser, numerical-cost and remaining platform-loop scope are unqualified.
   Source reconciliation exposes the inherited demand-driven prerequisite
   `eb354c8d504071bdb79110a7a5c9d374c2864113`: complete Wayland FrameLoop/ping/retry/deadline
   and the related Core scheduling contract/Web completion-hook removal must follow through
   their existing owners before complete cross-platform refresh-aware telemetry can be claimed.
   Current clock capture is not
   a disposition to reject that official mechanism. X11's direct Expose/recovery path also needs
   its source reconciliation against `ae99a867d7` and `f25434f3c5`; a delivered timer timestamp
   alone does not establish native Expose provenance.
   The [platform-loop prerequisite](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#demand-driven-platform-loop-prerequisite)
   owns the complete source/candidate/retirement reconciliation and removal of superseded hooks.
   Local implementation `b6506b48efca022c13256ef17d5ecfa26787b2ae` includes official Web/
   invalidator weak wakeups `417d03305a`, complete Wayland scheduling and X11 direct Expose/
   recovery through existing owners. Its [qualification](../verification.md#demand-driven-platform-frame-loops)
   records 62 focused Core/public, 27 actual production-state and five affected native passes.
   The complete runtime cohort passes 1,748 of 1,749, with one image-animation timeout passing
   an unchanged isolated rerun; its cause is undiagnosed. Five preexisting skips remain explicit.
   Components/docking/Canvas 2,966, disabled Core/public 1,134 and independent enabled/disabled
   40/31 pass without skips. Locked checks, correctness Clippy, formatting, actual Windows Rust
   1.92.0 and stable/nightly production wasm configurations pass. Existing completion commands
   carry Wayland post-borrow work; host state tests do not establish native Linux/browser execution
   or numeric cost qualification. Initial fixture failures and interrupted compilation remain recorded.
6. Refresh-aware frame/view/hang measurement `4f6d97b9de`: real signal/dirty work start,
   submission-start cutoff, per-refresh histograms, missed refreshes, view self-time and
   measurement-version-3 trigger/active-use rules. The
   [complete successor](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#refresh-aware-measurement-v3-successor)
   owns framework/report consumers, exact Submitted integration and enabled/disabled qualification.
   Implementation preserves the official mechanism while correcting reproduced inherited sealer
   and active-use defects. Fork raw input-activity controls follow existing dispatch lifecycle so
   nested/unwinding work and inactive gaps have bounded, chronological time accounting; they
   add no work or completion boundary. This explicit extension is not present in the fixed target.
   Local implementation `411895475e9e51a9f06954d5d676333d597b991f` has complete scoped
   qualification: framework/public/Gallery/state/native 1,809, components/docking/Canvas 2,966,
   disabled Core/public 1,134 and independent enabled/disabled 46/32 pass. The record preserves
   all red baselines, actual native submission and distinct compilation/cost limits.
   Do not infer timing fidelity or overhead from functional tests.
7. Fixed-target power/cadence follow-ups `fdcb198110`, `2580a00264`, `7c8dbadcb5`, plus
   macOS MetalFrameSource `0373366f43`. Public power modes, configurable frame limits, stable
   pacing, mandatory presentation bypass and actual platform frame sources have a
   [implemented owner](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#power-and-cadence-successor).
   Local implementation `2ceb09aa4590388b8e2d73ac1406caddc5b6c470` starts at
   `110f9c88084227eb526515167c204af3555f22a8`. The official mechanisms
   replace the fork's earlier force/callback-only throttle; platform state, observer lifetime,
   longest-limit composition, stable pacing and Metal/CV source integration share existing owners.
   The [qualification](../verification.md#power-modes-and-stable-frame-cadence) distinguishes
   executed Windows/portable consumers from foreign native compilation and physical power costs.
   Components 2,966, independent enabled/disabled 51/36, disabled Core/public 1,162 and final
   compiler/static checks pass. The final complete Windows run remains 220/221, followed by a
   passing three-case image/activation recheck. Earlier capture/activation failures remain
   unresolved qualification limits; no complete clean native gate or fixed-target parity is claimed.

Use the final target's files and follow-ups for each unit, mapping relocated ownership. Merely
adding an API name or retaining an earlier fixed-budget counter does not close its mechanism.

### View And Component Interface Reconciliation

Official `74b5207744ef95505fd75c56bf2c9d4736e931ee` unifies Render and RenderOnce through
generic View/ViewElement. The successor implemented at `652122a4e118ff2d213ae0d9392c71f49c44553b`,
starting at `a89a7200c80fc40eb073dbad37789f17153ec7b8`,
replaces the inherited direct AnyView Element and Component owners with the official shared
embedding mechanism. Typed/erased/weak handles, stateless scopes, parent props with entity identity,
the derive macro and both entity-backed cache routes move together. Shared phase helpers retain
the fork's exact candidate/resource/geometry admission, subscriptions and scoped profiler unwind.
Explicit conversion caller tracking is a reproduced fork diagnostic adaptation to the target's
internal source locations. It leaves render/cache ownership intact and has direct public controls.

The [View qualification](../verification.md#generic-views-and-reusable-component-interfaces)
records actual owner/public/native and independently locked consumer evidence: complete framework
1,856, components/docking/Canvas 2,966, disabled Core/public 1,167 and independent enabled/disabled
53/38 pass within their recorded scope. The bounded final metadata/test correction has separate
public enabled/disabled/native and compiler checks. Conditional Subsecond/build/runtime selection
and the target's absent debug accessibility
view-type map/dump pipeline remain separate complete successors. Do not claim their parity from
this API migration or introduce an unused substitute map. Source disposition remains separate
from measured timing, complete framework refresh and engine-default selection.

### Accessibility Publication Diagnostics And Synthetic Children

Source inspection at the fixed target identifies complete official mechanism
`2268045a119030735f762e5afaf59da0bda869f4`: view-name/node provenance, last-tree capture,
Window JSON access and Zed dump/copy consumers. Its synthetic-child provenance depends on the
framework slice of `83d4847462e57a6f72dc314abea05d57d3bdcfb6`: Element callback,
A11ySubtreeBuilder and Div/stateful forwarding. Settings product dependencies are unnecessary.
This owner is implemented locally at `751e563eb89eeb373dac41cf734bfe372b81d0e4`, separately
from the generic View API. Synthetic children affect runtime accessibility. Only provenance is debug-only;
the target retains tree capture and JSON access in release builds too.

Use the fork's final accepted accessibility publication, existing focus/activation authority and
prepaint rollback. Candidate geometry/modal/hidden filtering must also filter provenance; refused
or superseded candidates cannot replace the last published snapshot. The target captures before
its send decision, and its capture counter is not a native Submitted generation. Its view-name
registration is limited to AnyView; broader typed/manual attribution would be a documented fork
adaptation, not source parity.

[ADR 0023](../adr/0023-open-gpui-live-region-announcement-authority.md#explicit-published-tree-inspection)
now distinguishes automatic diagnostic artifacts from explicit developer inspection. Preserve the
official complete raw tree reader, including label/value/title and live-region content already
published in that tree. Automatic announcement diagnostics and DevTools histories/exports remain
redacted; they do not consume this reader. Keep the public Window reader and an actual
example/Gallery copy consumer independent of Zed editor and Nara types. Acceptance
must cover synthetic parentage, debug/release output, exact published-tree correspondence,
rollback/rejection/filtering, activation changes, raw canary inclusion in explicit inspection and
canary exclusion from automatic artifacts before the queued unit closes.
Map frame/title/focus metadata and valid tab-stop counting to the existing owners. Official action
enumeration uses AccessKit's enumn feature, now enabled with only its necessary dependency edge
in both lockfiles. The [qualification](../verification.md#synthetic-accessibility-and-explicit-published-tree-inspection)
records four executed failure boundaries and their owner corrections, final Core/public 1,412,
components/docking/Canvas 2,966, WGPU/affected native 96, disabled Core/public 1,203, Gallery 13,
independent type-only 2/2 in each feature mode, and an executed six-case non-debug serializer
slice. Locked checks, correctness Clippy, actual Windows Rust 1.92, production wasm, formatting
and static scans pass within their recorded scope. Full AT/native-foreign/browser, numerical
costs, engine adoption and the remaining official mechanisms below are separate work.

The bounded audit keeps both product-labeled commits split by their actual mechanisms. The
synthetic/inspection unit does not close their following surviving framework behavior:

| Mechanism | Fixed-target source and current fork disposition | Owning successor |
| --- | --- | --- |
| Active-descendant focus | `83d4847462`: descendant-side declaration, real focused-ancestor gating, self/duplicate diagnostics and final tree focus override are missing. `track_accessibility_focus` and relation repair are different contracts. | Composite-widget focus and keyboard continuity through real menu/listbox consumers. |
| Keyboard click continuity | `83d4847462`: KeyDown/focus-generation capture and one-shot KeyUp consumption are missing. Other keys and focus ABA must cancel the pending activation. | Same focus/keyboard owner; a KeyUp-only click is not equivalent. |
| Tab-group ordering and missing-node diagnostics | `2268045a11`: insert the container focus handle inside its tab group; distinguish no ID/no role with per-focus diagnostic dedup/reset. Both are missing. | Same owner, preserving scoped geometry and accepted focus. |
| Accessibility properties | The fork's `InteractivityAccessibility` already owns description/value/placeholder, equivalent in responsibility to official `AriaProperties`. `aria_keyshortcuts` and `aria_numeric_value_step` remain absent. | Complete the existing property builder/writer and public/final-tree consumers; no parallel property owner. |
| Initial title tree | `83d4847462`: per-frame root labels and title updates are present in this unit. The fork still returns no synchronous initial tree and schedules a sequenced refresh. | Reconcile activation callback, exact native authority and first-tree visibility explicitly; retain this behavior difference until qualified. |
| Later enabled/author-ID APIs | Fixed-target `9183ba4c55` separates enabled from active; `cc053a4a6f` adds author identifier. Both remain missing. | Separate complete public property/configuration paths; the existing active query does not close enabled semantics. |

These are source-confirmed queued mechanisms, not rejected official functionality or executed
equivalence. Continue the complete focus/keyboard unit before the benchmark/report and hot-reload
owners when its consumer prerequisites are available.

### Framework Slices In Product-Labeled Commits

A bounded fixed-target source audit corrects seven further product-wide rejections. Product
consumers stay outside the imported closure; their surviving framework slices have independent
dispositions. None of these findings adds runtime qualification or changes the fixed target.

| Source | Framework disposition | Complete owner/acceptance |
| --- | --- | --- |
| `a923597341` | Official non-interactive regression imported; runtime already omits the interactive flag. | Existing runtime and source regression need no terminal-product dependency. |
| `479bce0995` | Complete public util::parse_os_release and official/public tests implemented. | ID/optional VERSION_ID and quote/missing-ID behavior remain official. No telemetry dependency is needed. |
| `ccf4058b7a` | Complete Rems::ZERO/from_pixels/AddAssign and all three same-depth key load-order scenarios implemented. | Actual public Window/layout and independent value/signature consumers preserve the existing resolver; no picker/product overlay is needed. |
| `10628c3d2c` | Complete upward List normalization implemented; the old negative-offset branch is removed. | Existing PendingAutoscroll/admitted commit retains source and atlas rejection. Original/variable/zero-height/start-clamp controls and exact public recovery qualify the adaptation. |
| `362035d52a` | Official parenthesized-position regression imported; runtime parsing already present. | Existing `/root/Test (3)` path/row behavior is preserved. Product file-existence fallback stays outside util. |
| `c7ad65e468` | Complete source-order shared line queries and measurement-only indexed lookup implemented. | Owner, actual software paint and independent signature consumers preserve existing text/candidate lifetime and clipping. |
| `1722fe63bc` | Parameterized benchmark macros, real text-system injection and threaded dispatcher/timer teardown are absent. | Extend the existing benchmark/report successor, not another harness. Reconcile phase 2 `297c4a4d78`, ThreadedDispatcher `8886dcb0d4`, existing `1f7eae7fb7` support and `3135cdf8dc` platform text-system caching against final target. |

The benchmark owner includes inputs/input_name/group/sample_size validation and Criterion
bench_with_input, thread-local dispatcher reuse with per-context platform creation, timer release
outside its lock followed by waking/draining, bounded cancel/drain after App/foreground release,
and bounds evaluation before mutable App access. Macro parameters alone do not close that owner.
The source-only audit retains the explicit `2882636c06`/`b5c2d8a13f` revert pair's rejected
disposition. It is a bounded correction, not a claim that all target-to-fork omissions are exhausted.

The [query/List qualification](../verification.md#measured-text-queries-rem-units-and-list-autoscroll)
records utility `787ab55a8f68daab1dc48e2c80d12f4109028bdb` and UI
`e58b8b7687aadc91408657b4e550a1b7d723c0a9`, starting at
`18e5f46e87f919049628cd9fd1be782c0fbaf49a`. Three executed List red controls precede its
correction. The record retains the 51/52 fixture-generation failure and measured-text cache/link
failures and corrections. Final focused 54, complete Core/public 1,390, utility 115, WGPU 89,
components/docking/Canvas 2,966, disabled Core/public 1,181 and independent enabled/disabled
56/41 pass. Affected native five, locked checks, Clippy, actual Rust 1.92, production wasm,
formatting and static scans pass within their recorded scope. These entries do not close complete
benchmark/report, accessibility, hot-reload, foreign-platform/browser or engine owners.

### Text And Layout Reconciliation Units

The original fixed-target text audit maps the following complete units to fork `229723e620`.
Implemented successors link their later executed qualification; queued source findings add no
runtime qualification. Preserve `WindowTextSystem::abort_frame`, which returns
layouts moved by a refused candidate, and the fork's exact clip authority throughout the changes.

| Unit | Source-confirmed behavior and prerequisites | Owning acceptance |
| --- | --- | --- |
| Implemented: font resolution, installation and reporting | `9e6e14160c`, `3389d87577`, `b7d50b95e8`, `b68add5b37` now flow through the existing TextSystem, window cache, Cosmic and App owners. Direct/Fallback/Missing resolution and successful-installation generations preserve existing face identities. One coherent frame-cache state, pre-resolution generation capture and generation-aware width pools prevent stale reuse; exact candidate abort/retry remains authoritative. | The [font qualification](../verification.md#runtime-font-installation-and-missing-glyph-reporting) owns reproduced failures, actual Core/Cosmic/Window and native installation results, observer/task release and explicit redraw. Reporting is bounded, subscription-scoped and revalidates after App borrow waiting. Current Web obtains Cosmic's behavior directly; the target's later Canvas wrapper/post-fallback scan remains a platform-closure difference. Windows/CoreText retain the optional default sink and gain no missing-glyph reporting claim. |
| Implemented: Cosmic paragraph shaping; remaining fallback policy | `c214057e08` now splits `Bidi_Class=B` paragraphs, preserves exact clipped FontRun configuration and rebases original glyph byte/x positions with merged metrics. Missing raw indices aggregate against the complete source so CRLF reports stay whole. `d6358b2d3c`'s fallback-face preservation remains implemented. | The [paragraph qualification](../verification.md#cosmic-paragraph-shaping-and-public-paint) owns red direction/reporting regressions, the corrected width oracle, actual Hebrew glyph/raster and public headless shape/paint/reconstruction. Direct `unicode-bidi` uses the already resolved 0.3.18 in both lockfiles. The no-`m` primary filter still omits that family from explicit fallback-chain selection; automatic fallback uses the retained database face. These results do not qualify general fallback priority, full RTL caret, native GPU pixels or platform parity. |
| Implemented: font preparation, inventory and raster reuse | `f1d27d545e`, `5b4a2153a0` and `1ff7cb669b` now use explicit public prewarming, backend-only family inventory and Cosmic bounds-to-raster image reuse. Shared primary/fallback match properties preserve normal shaping. The fork bounds temporary image handoff to 256 entries and 8 MiB of buffer capacity, with eviction/recomputation rather than glyph refusal. Native alias preparation also exposes the callback-identity prerequisite fixed separately at `19a4ed3434a`. `d0f797a38c`'s retained embedded-font source storage remains implemented. | The [qualification](../verification.md#font-preparation-inventory-and-raster-reuse) retains the inventory/alias red baselines, actual query/match identity, cold pixel/Swash-generation controls and native installation/retirement. Final all-feature Core/WGPU/Windows plus public text passes 1,268/1,268 with four existing interactive cases skipped; affected components/docking/Canvas pass 2,964/2,964 and independent public API tests pass 5/5. Locked compilation, correctness Clippy, formatting and actual Windows Rust 1.92.0 affected-framework/consumer checks pass. Existing atlas/family/width-pool hits are controls; numeric frame/process costs remain unmeasured. |
| Implemented: wrapping, indentation and truncation | `f0210f1621`, `4f70d91bda` and inherited `03a8544040` now flow through the existing wrapper, shaped-layout, style and TextRun owners. UAX opportunities span fragments/elements; explicit indentation, all three truncation directions and source/visual grapheme grouping preserve font generations and candidate recovery. Additional corrections retain carried End content and measure Start's actual row budget. | The [qualification](../verification.md#unicode-wrapping-indentation-and-middle-truncation) owns reproduced baseline/review defects, public style/paint, exact native Submitted frames, reconstruction and dependency/compiler evidence. The target's non-complex-scripts segmenter remains selected; general complex-script/UAX #9 caret and logical reconstruction of reordered rows remain open. |
| Implemented: text split, layered paint and underlines | `2936989f1b`, `f792c2d72d`, `bda9c0bd43` and `0dea8c63d9` now map to raw layout split/paint, a forward cursor, public TextLayout passes and underline callbacks/exclusions. Source-index partition fixes the old visually reordered split's reproduced index underflow. Shared row geometry and existing projected clip owners retain the original glyphs and full wave phase. | The [qualification](../verification.md#source-fragments-text-layers-and-underline-exclusions) retains UTF-8/monotonic boundaries, independent visual-RTL membership, aligned/wrapped layer footprints, inserted highlights and zero-advance callbacks. Both owner defects are reproduced before correction. Focused public/native 19/19, complete Core/WGPU/Windows plus public text 1,281/1,281, affected components/docking/Canvas 2,964/2,964 and independent API tests 6/6 pass; four existing interactive Windows cases are skipped only in the aggregate. Locked compilation, correctness Clippy, formatting and actual Windows Rust 1.92.0 framework/consumer checks pass. General bidi reflow/caret, existing ink overhang, numeric costs and the remaining refresh keep separate scope. |
| Implemented: window-local layout ownership | Official `772a8a9100` supplies Rc/RefCell window caches and local font-run scratch while retaining global Arc<TextSystem>; final-target follow-up `dde76a1dca` uses Cell take/set for scratch. Official `2a74c8dff6` removes unused hash APIs/caches. The fork adopts those owners and deletes the parallel hash path. | The [qualification record](../verification.md#window-local-text-ownership-and-source-cache) separates official mechanisms from fork generation coherence, rejected-candidate recovery, backend reentry and a reproduced inherited clamp-key correction. Complete Core/WGPU/Windows plus public text passes 1,292/1,292 with four existing interactive cases skipped; affected components/docking/Canvas pass 2,964/2,964 and independent public tests pass 7/7. Full source, automatic notified frames, reentry/unwind, shared font installation and exact two-window/native lifetimes retain their own scope. Locked compilation, correctness Clippy and actual Windows Rust 1.92.0 framework/consumer checks pass. Rc/method removals are explicit breaking changes; numeric costs remain unmeasured. |
| Implemented: Taffy and intrinsic measurement | Official `91fdd55889`, `b05f40c554`, `31ceaf7908`, `cdf33ac25e`, `b488c4e814` now map to Taffy 0.15 leaf/alignment APIs, explicit relative position, separate truncation width, actual shaped-overflow checks and MaxContent list probes. Both qualification lockfiles select 0.15.0; the library permits compatible patches. | The [qualification record](../verification.md#taffy-text-measurement-and-rootgrid-sizing) retains the reproduced inherited ordinary-wrap defect and fork exact-width correction, actual kerning/source/paint consumers and initial fixture failures. Corrected Core/WGPU/Windows plus public layout/text passes 1,312/1,312 with four existing interactive cases skipped; affected components pass 2,964/2,964 and independent public tests pass 9/9. Locked compilation, correctness Clippy and actual Windows Rust 1.92.0 checks pass. No general word-based MinContent or measured speedup claim follows. |
| Implemented: root/grid sizing and stack policy | Official `b0da438545` fills auto window/prompt roots without stretching anchored/tooltip/drag roots. `5ccbbbd88f` adds grid-row min/max-content and renames the shared min-size type. `41d72f6c1d` now makes stack protection opt-in across both Taffy callbacks and Div's three phases. | Auto/explicit/percentage/minmax roots, prompt, notify/resize, shrink-wrap and actual grid geometry pass within the same qualification. The stacker-disabled owner/portal cohort passes 33/33; the all-feature complete runtime enables stacker. Portal root-inset and multi-Sidebar visible-allocation fixture corrections preserve original identity/input/transform assertions. Both feature paths and native reconstruction retain their stated workloads, not arbitrary-depth safety. |
| CoreText identity and size | `20a93f6195`, `44ec5691b5`: deduplicate PostScript identity within one family load and preserve the exact first nonempty run size. | Duplicate/missing family identity and empty/BOM leading runs on macOS. These local text corrections do not require completing the Apple renderer split first. |

The target's UAX path adds `icu_segmenter` 2.3.0 with compiled data and promotes
`unicode-segmentation` to a normal dependency. Cosmic adds direct `unicode-bidi` 0.3.18;
Cosmic 0.19.0 and Swash 0.2.6 are already present locally. Available package metadata declares
Rust 1.88 for ICU segmenter, 1.47 for unicode-bidi and 1.71 for Taffy 0.15; those declarations do
not qualify the resolved closure under Rust 1.92. Validate that compiler and the independently
locked consumer rather than adopting upstream's workspace compiler by assumption. The consumer's
direct Taffy value supplies preview dimensions rather than crossing a GPUI conversion boundary.
The current layout successor explicitly moves that application pin to 0.15.0 to exercise the
same selected backend release through its independent lockfile; it does not claim a direct
Taffy-to-GPUI geometry interoperability API or a distinct Taffy patch-version comparison.

The November 6 wrapping successor executes Rust 1.92.0 for the affected Core/WGPU/Windows and
independently locked consumer all-target/all-feature checks. It corrects existing unstable source
uses in TypeId hashing, profiler cold hints and Windows mouse dispatch while retaining the
declared floor. Root ICU 2.3 and consumer ICU 2.4 both compile. The fixed target itself pins Rust
1.98.1; its source environment is separate from this executed Windows minimum-compiler scope.
