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
  `font-kit` fork, and the original July crates.io `wgpu` migration. The November WGPU
  successor below explicitly replaces that dependency source with the official pinned
  upstream correction after its real registry failure control.
- Do not import Zed editor/product crates. Retired dependency forks stay excluded unless a
  source-specific November successor explicitly selects the required official mechanism under
  ADR 0001, as for request-local reqwest redirects.
- Every accepted candidate needs an owning unit and a verification path.

The November continuation applies the
[official-baseline rule](../../.agents/skills/open-gpui-upstream-sync/SKILL.md#integrate-through-existing-owners).
Record retained differences and their consumer/verification basis here; an unimplemented
official mechanism remains queued until it receives an explicit disposition.

## Accepted Candidates

| Upstream commit | Owning unit | Upstream intent | Open GPUI mapping | Test source |
|---|---|---|---|---|
| `552fc9f3c3` | U2 | Fix streamed reqwest request bodies being truncated after `Poll::Pending`. | July used registry reqwest in `crates/reqwest_client/src/reqwest_client.rs`; the November HTTP successor supersedes that source/cache choice with the complete official owner. | Upstream streamed-body regression adapted to local inline tests. |
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
| `34cd17ff5e` | Implemented and qualified at `b42295d0e53` | The July classification was incorrect: the complete builders were absent at `f6f3d98d156`. The randomized-tree unit restores the official `BoxShadow` construction, blur, spread and inset builders consumed by its tree. |
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
| `6076ce2738` | Split | The framework's ready-foreground draining is present in the shared threaded dispatcher and BenchAppContext, with its official nonwaiting-handoff regression retained. The [complete benchmark qualification](../verification.md#complete-benchmark-context-and-measurements) owns that mechanism. Zed Markdown/edit-file benchmark workloads remain product consumers outside this closure. |
| `362035d52a` | Split | File-existence fallback stays in product path derivation. The util parser already supports parenthesized positions; its official behavioral regression is imported and qualified below. |
| `83d4847462` | Split | Settings product migration stays outside the framework closure. The complete synthetic-child callback/builder/Div mechanism is implemented and qualified below. Its separate composite-focus/keyboard owner is now implemented with scoped qualification below; the product disposition rejects neither framework slice. |
| `c7ad65e468` | Split | Editor highlight consumers stay outside the closure. Complete measured TextLayout queries are implemented with owner/public/independent consumers; scoped qualification is recorded below. |
| `1722fe63bc` | Split | Editor multi-cursor consumers stay outside the closure. Parameterized benchmark, real text-system and dispatcher/teardown mechanisms join the existing benchmark/report successor below. |
| `138139f830` | Implemented: typed Mac successor | `0c1118e2f1a` carries the official titlebar-container/button frames and tracking-area updates for both normal and reset positioning. The [Mac qualification](../verification.md#macos-native-input-appkit-and-screen-capture) records foreign compilation; actual AppKit hitbox behavior still requires an Apple host. |
| `60ed56b372` | Implemented: complete Darwin process successor | The inherited omission found by the November 10 audit is superseded at `5b1810abbcc`: owned descriptors, inherited stdio under CLOEXEC_DEFAULT and SIGPIPE restoration use the unified official process owner. The [qualification](../verification.md#process-reaping-dedicated-execution-and-shared-strings) separates source compilation from native Darwin execution. |
| `e1bfcf85db` | Implemented: complete Darwin process successor | The complete pipe/dev-null and reaper owner at `5b1810abbcc` removes the former waitpid/Unblock implementation. Its fixed async-process selection is qualified with actual consumers; native Darwin process execution remains unqualified. |
| `137e677a05` | Implemented: inactive-window IME gate | `c0a36ddce01` restores the fixed target's early return before querying the leased handler or changing shared IME state. The [qualification](../verification.md#wayland-inactive-window-ime) records 13/13 affected cache and actual native-window lifecycle controls. Physical IME/text-input protocol delivery remains separate. |
| `8036a3c74b` | Superseded | The implemented shared UAX wrapping mechanism from `f0210f1621` supersedes the old glue-specific policy. Do not restore the obsolete branch. |

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

The November 11 [fixed-cycle exit](../verification.md#fixed-cycle-source-adoption-exit)
reconciles the named source-adoption dispositions at Open GPUI
`424a3a0f7c2e71f6ce5a933de00138919c50893d`. Read earlier queued findings with their
implemented successors and explicit exclusions below. This bounded source disposition does not
grant complete macro/API parity, full platform qualification, Nara R11 or consumer acceptance,
or a framework-maintenance handoff.

The subsequent [external WGPU prerequisites](../verification.md#retained-gpu-images-and-supplied-win32-renderers)
are fork-owned framework improvements after that fixed source audit. They add retained texture
import and separate Win32 native services from concrete renderer construction; they do not change
the fixed Zed target or retroactively describe these extensions as official implementations.
Ordinary DirectX remains a control. An application-owned HWND/loop, delayed Scene/submission
publication and the actual Nara Host/View consumer retain separate incomplete gates in the plan.

The following [supplied-window/external-Win32 unit](../verification.md#supplied-platform-windows-and-external-win32-driving)
extends those fork-owned boundaries with App's explicit platform-window factory and a synchronous
application-created HWND/loop composition. Exact binding detachment remains distinct from OS
destruction, and native service/resource cleanup uses the original retirement authority. The
unit's actual results and failures remain in its qualification; delayed Scene/input publication,
broader surface hosting and the Nara consumer keep separate gates. These are framework extensions
after the fixed-source audit, not additional official source-adoption credit.

The [deferred-delivery successor](../verification.md#deferred-scene-submission-and-submitted-publications)
implements original candidate custody at `797372013ab` and native/ordinary WGPU driving at
`69d0992f2a0`, with standalone feature correction `ec576a20dc1`. Submitted interaction and
publications remain authoritative until exact host submission returns through ordered ingress.
The record owns final focused 57/57, affected shared 626/626, native 22/22, independent 1/1,
actual producer pixel readbacks and synchronous controls. These are fork-owned extensions;
logical roots, physical/platform/numeric qualification and the Nara consumer remain open.

The next fork-owned [hosted-surface unit](../verification.md#host-driven-logical-surfaces-without-native-windows)
adds a production no-native-window adapter through the original App/Window, with its own captured
capability profile and host renderer/input/frame services. Core production compilation and 45/45
affected CPU/public controls pass, including retained composition cleanup on close. All four
ordinary GPU trajectories, profiler-disabled capture 1/1, native lifecycle 14/14 and independent
interfaces 5/5 pass. Core is `2910b52424e`; ordinary consumers are `8809f12929e`. Final static
qualification stays in the record. This work does not add official-adoption credit or select an
engine UI dependency.

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
qualification; later platform and API owners below resolve the broader source findings. The
fixed-cycle exit records their bounded disposition without claiming an exhaustive per-commit or
API-parity audit of the 364 commits.

| Owning area | Target evidence and current fork difference | Required reconciliation |
| --- | --- | --- |
| Implemented: GPU primitive representation | `f28177003499ec9c5052d1883fee62b148451c59` replaces GPU-facing Rust `bool` with `PaddedBool32`. The fork now uses private initialized zero/one storage in both sprite and underline records, preserving its clip, tile-generation and transform ordering. | Shared field-width/value/offset checks and complete Core/WGPU execution cover the corrected upload ABI. DirectX now uploads the shared sprite slice without a duplicate layout or conversion vector. The [GPU qualification](../verification.md#gpu-primitive-flags-and-windows-color-transfer) owns actual Windows image/decoration/opacity/clip/cache pixels and shader-path results. Metal source/ABI assertions are updated; generated-header, shader and native execution on macOS remain unqualified. |
| Implemented: Windows color transfer | `b9419ae72814157164b09592ab2466f9f4709f09` corrects reversed linear/sRGB power directions through the existing HLSL owner. | The native old-shader regression fails on a non-endpoint Oklab pixel; the independent gamma-2.2 oracle passes with corrected directions, retaining sRGB/solid/alpha and newer-frame controls. See the [qualification](../verification.md#gpu-primitive-flags-and-windows-color-transfer) for actual compilation/execution paths and limits. This preserves the Windows approximation, selects no engine-wide color policy and establishes no performance result or byte parity with WGPU's piecewise transfer. |
| Implemented: native text/atlas hardening | `89e8a4b9ec7e0fa8a49987a3c1dbf29343dc6999` adds mutable DirectWrite callback context, nullable-array checks, mapped color-glyph staging release and atlas source-length checks. The text-layout unit takes the first two prerequisites. The native-resource successor now completes mapped staging release and shared checked atlas upload admission. | Both insertion APIs refuse malformed sources through `Err` before allocation/key/generation changes and validate the final destination before upload. A later refusal cancels only its unpublished allocation; published regions and exact-instance leases remain valid. The target's void short-buffer skip was intentionally replaced because its caller still publishes success. Color readback uses RAII Unmap and transparent target initialization through the existing raster owner. See the [native-resource qualification](../verification.md#native-atlas-upload-and-color-readback) for WARP multi-row/nonzero-origin pixels, retry/lease controls, debug-layer red/green and actual color-emoji readback. These changes establish neither GPU completion from an upload return nor full refresh completion. |
| Implemented: shared image decoding | Official `9552acc2bc` and fixed target `635026fabc` now share static/GIF/WebP decoding across encoded Image and resource loaders. The retired duplicate blocks were inherited official code, unchanged from the import anchor. | The [image qualification](../verification.md#shared-image-decoding-and-animation) records red EXIF/one-frame regressions, BGRA/alpha and asymmetric-orientation controls, real headless timelines, isolated completion notifications and native 0/1/0 Submitted/GPU-readback/retirement. Official contextual decoder errors now enter `ImageCacheError::Other`. Fork-only adaptations declare the source-confirmed image 0.25.4 API minimum, use the existing executor clock and include the exact loading deadline. Complete affected framework/public, component and independent suites pass 1,328/1,328, 2,964/2,964 and 12/12; four existing interactive Windows cases remain skipped. Locked checks, correctness Clippy, formatting and actual Windows Rust 1.92.0 affected-framework/consumer checks pass. This is not full image/platform or renderer-lifetime parity. |
| Implemented: image dependency features | Official `8e06c323ae063f3d245ea5c283d4bf373ad51db2` is adopted at `b4355a36824f0709f1b73f7480ec88a884a489bc`, retaining fourteen formats, Rayon, the 0.25.4 API minimum and resolved image 0.25.10 while excluding default AVIF encoding. Neither the previous nor current graph provides the separate avif-native decoder. | The [qualification](../verification.md#image-dependency-feature-policy) records exact three-platform before/after/default/downstream graphs, final Core/public/native image 25/25 and ordinary application-owned encoder off/on 3/3 and 4/4. Locked checks, correctness Clippy, actual Windows Rust 1.92 framework/consumer checks and production wasm pass. The separately refreshed smoke lock passes its own locked wasm check. No GPUI codec-selection owner is added. TGA resource guessing, native AVIF decoding, foreign execution, full refresh and numeric costs retain their distinct scope. |
| Implemented: owning asset loads | Official `5896274168e06663568f42b5bae142162936e0a6` and fixed-target cleanup now flow through the owning CachedLoad, Option-returning App fetch, Window subscriptions and opaque ImageCacheItem. Shared task handles and per-use completion tasks are removed from App, Window, retain-all and the actual gallery LRU. | The [asset-load qualification](../verification.md#owning-asset-loads-and-view-completion) records the reproduced fork App-borrow failure and wait-before-publication adaptation. Focused owner/image/candidate/LRU/native cases pass 22/22. Each interested view receives one notification, passive reads do not redraw, eviction cancels old entries and accepted frame leases survive key removal. The dormant gallery now has an explicit platform example target and actual cache tests. Browser contention, numeric costs and complete framework/engine acceptance remain distinct unqualified scope. |
| Implemented: externally driven App ownership | Official `74798c68d5c` now supplies `Application::run_embedded` and the strong ApplicationHandle; launch reuses the existing run_platform owner. Handle update deliberately uses the fork's complete update transaction rather than official direct callback invocation. | The [application qualification](../verification.md#externally-driven-application-lifetime) records defer/batching red results, 10 owner cases and actual Windows visual-host Submitted/readback/retirement. Complete affected runtime/public, component and independent suites pass 1,339/1,339, 2,964/2,964 and 14/14; four existing interactive Windows cases remain skipped. Locked checks, correctness Clippy and actual Windows Rust 1.92.0 affected-framework/consumer checks pass. Stock Windows still owns a blocking process loop; native production external-loop support, shared GPU and separate display/hot-reload/asset owners are not implied. |
| Implemented: display and launch lifecycle | `f495dd0f77b69e287ed72c1168dc02965f08be7d` now supplies App-owned display observation, per-window association and cached refresh intervals, initialized from both launch callbacks. The fixed target retains that mechanism. | The [qualification](../verification.md#connected-displays-and-window-association) records the fork's ordered native ingress/immutable topology adaptation, official reciprocal-overflow correction, deferred-close reentry fix and ordinary required bitflags dependency. Corrected runtime/public/gallery, component and independent consumers pass 1,388/1,388, 2,964/2,964 and 18/18; four existing interactive Windows cases remain skipped. Source migration, actual Submitted frames and synthetic WM ingress retain distinct scope from physical hotplug/refresh-mode and non-Windows native qualification. |
| Implemented: refresh-aware measurement v3 | `4f6d97b9de7d9f167da8b7832eeecea22c5a9efa` and final cleanup now measure signal/dirty-to-submission work against each window's refresh interval, including view self-time and the complete v3 detector/monitor mechanism. Local implementation is `411895475e9e51a9f06954d5d676333d597b991f`. | The [qualification](../verification.md#refresh-aware-frame-and-hang-measurement-v3) retains official provenance, exact accepted/Submitted adaptations and four reproduced inherited sealer/active-use corrections. Complete framework/public/Gallery/state/native 1,809, components/docking/Canvas 2,966, disabled Core/public 1,134 and independent enabled/disabled 46/32 pass. Actual Windows Rust 1.92.0, stable/nightly production wasm and static checks pass. Numeric overhead, foreign native/browser execution and complete benchmark/report/debug-overlay parity remain separate. |
| Implemented: additive test support | Official `95cd535a5fad96d649513f96c5784ceefd379e47` now separates leak tracking from test support, adds build-time instrumentation selection and identifies deterministic execution through runtime queries in existing owners. Local implementation is `c6ffb32bd1499c9e8fb46783a3d1a4de626707a0`. | The [qualification](../verification.md#additive-test-support-and-explicit-leak-tracking) records actual inherited and native-default failures before corrections, focused 130, util 116, complete public/independent feature/environment matrix, selected framework 1,918-case union, complete Windows 224 and components 2,966. Ordinary enabled/disabled 62/47, locked checks, correctness Clippy, actual Windows Rust 1.92, production wasm, formatting and static gates pass. The initial native extra-event source, foreign execution, numeric costs and hot reload retain separate scope. |
| Implemented: conditional GPUI hot reload; remaining native focus gate | `670a779a5ed9e7161c97aa4f998d395c207d5631` and the final target now supply the optional dependency/build selection, native launch initialization, matched patch transport, foreground application and View/AnyView HotFn boundaries. The fork waits through its existing AsyncApp update owner when native callbacks hold an active borrow. | The [qualification](../verification.md#conditional-native-hot-reload) records an independently locked actual edit/build/apply/native-pixel consumer with retained runtime-mutated state and normal exit, plus five native rejection/disconnect controls. The identified dx 0.7.10 x64 code-generation backport and separate fork terminal-settlement correction retain distinct provenance. One repeated foreground-activation test still fails with an external game foreground; no complete final all-green native run is claimed. Compatible render-body success does not qualify arbitrary state-layout migration or Nara Editor adoption. |
| Implemented: shared atlas and retained renderer bindings | Official `0ef92145ff87aab5f19f6b44c9c55a486f4f064f`, released-resource access `96e95edac45` and the complete WGPU preupload/cache/shared-core chain are integrated at `67e1360709ca48be74bf986f116b9c004ff5bd59`. Shared key publication and HeadlessAtlas replace duplicate owners; exact leases, native frozen handles and complete-frame refusal remain fork adaptations. | The [qualification](../verification.md#shared-atlas-ownership-and-retained-renderer-bindings) preserves actual GPU fixture failures, inherited upload correction and the reproduced local prepared-region overwrite before its lease fix. Complete WGPU/public 110, final focused 21, components 2,966, independent enabled/disabled 63/48, compiler-floor/wasm/static gates pass. The earlier framework 1,934/1,935 retains the known foreground failure and four interactive skips. Metal/Linux foreign execution, replacement-Device/fractional-DPI/subpixel execution and numeric costs remain distinct open scope. |
| Implemented: WGPU dependency, shaders and WebGL | Official `4601ead416`, `d544470d1d`, `402675e805`, `24a2823456` and native configure-once `e20d19d32f` are adopted at `38d3041acab90cc01cbc9984a8b738a8b2c3a194`, with matching upstream WGPU/Naga `cddb2f5d80dc0009b2ef6767635140cacf90a007` and an explicit Rust 1.93 floor. | The [qualification](../verification.md#wgpu-dependency-shaders-and-webgl) records Vulkan registry red/restored-source green, the fork's corrected 48-byte surface uniform, packed clip/transform pixels and refusal/retry, complete WGPU/public 123, native 3, components 2,966 and independent enabled/disabled 65/48. Actual WebGPU/WebGL/Auto fallback pass both PNG sizes, text and original interaction controls; forced adapter/backend refusals remain explicit. Locked checks, correctness Clippy, actual Windows minimum-compiler checks, production/shared-memory wasm compilation, four-target graphs, formatting and existing static gates pass. This is scoped renderer qualification, not foreign execution, physical loss, DPI/performance, complete refresh or Nara GPU/View adoption. |
| Implemented: frame-time debug overlay | Official `07a0bd1220` and fixed-target `debug_overlay.rs` now provide the complete public mode/history/bitmap/Window consumer at `73472ca230bac117f8cb417d0cd6b0c1364c980e`. Exact clipping, whole-candidate refusal and accepted-draw-only sampling reuse fork owners; the separate resource-retirement correction is `d928c5459a8ed174d1a6835abd096b3e7a996b5a`. | The [qualification](../verification.md#frame-time-debug-overlay) preserves all six original tests, focused 20, complete framework/public 1,970, final actual native 5, components 2,966, disabled Core/public 1,350 and independent enabled/disabled 67/48 plus additive 4. Actual Windows Rust 1.93, locked checks, correctness Clippy, production/shared-memory wasm, formatting and static gates pass. Recorded failures and strengthened native generation observation remain explicit. This does not establish numeric overhead, foreign overlay execution, complete refresh or Nara adoption. |
| Implemented: animation controls and clock | Fixed-target `elements/animation.rs` supplies reduced motion, synchronized repeats, per-animation frame limits and executor time through `d06ebb7c03`, `4ed3738c40`, `dd04a229dd` and `99e70ee4a0`. Local `4ed461f2b591d6e44d84abc742094740fbd607af` replaces wall-clock progression and unconditional frame requests with the complete non-Spring controls, App shared epoch and image reduced-motion behavior. | The [qualification](../verification.md#generic-animation-controls-and-clock) records all five official tests, focused 26, final image 3, actual native Submitted/pixels/retirement 1, complete framework/public/Gallery 2,138, components 2,966, disabled Core/public animation 1,310 and independent enabled/disabled 68/52 passing. Seven aggregate skips retain separate scope. Reproduced rejected-first-mount timers and App-borrow notification require bounded fork adaptations. Locked checks, correctness Clippy, actual Windows Rust 1.93, production/shared-memory wasm, formatting and static gates pass. Spring, foreign execution, numerical costs and actual engine integration remain separate. |
| Implemented: stateful Spring | Official `8b1497dbd22fb06f5838a7c0b84a1e54fafa71bc`, with executor-clock correction, supplies typed targets/playback and Element-ID state through `with_spring` at `ecbbfa361ae4489b2de4121c5f0ca8b2a1d7aebb`. The existing motion crate owns the single pure solver, reexported by GPUI; duplicate fork damping formulas and `MotionSpringPhysics` are removed. | The [qualification](../verification.md#official-spring-and-shared-domain-sampling) preserves all official cases, complete framework/components 2,171/2,974, final motion 94, affected paths 43, actual native 1, disabled Core/public 1,340 and final independent Spring 6/6 in both profiler selections. Earlier broader consumers pass 74/58 at their recorded checkpoint. Executed numeric/candidate failures justify bounded fork adaptations; App reduced motion remains an adapter floor. Locked checks, expanded correctness Clippy, actual Windows Rust 1.93, production/shared-memory wasm, doctests, formatting and static gates pass; all thirteen lanes close. Numeric costs, foreign execution, remaining Core mechanisms and actual engine integration retain separate scope. |
| Implemented: system notifications | Official `de827bce2ff1cbb40040bea0ef57c8ac56afd726` supplies Core values, App/Platform identity/show/dismiss/response APIs, TestPlatform observation/simulation, native transports and the ordinary example at local `26639a9715aff09ffbc61b92af45014728f02f94`. | The [qualification](../verification.md#system-notifications-and-ordered-app-responses) preserves all four official tests, lazy initialization and platform limits through existing ordered App updates and handler replacement. Windows transport retires with its native owner. Corrected Core/public 12, Windows 11 including actual Submitted pixels/retirement, complete framework/public/Gallery 2,194, independent enabled 77, ordinary disabled notification 3 and disabled Core/public notification 12 pass. Locked checks, expanded correctness Clippy, actual Windows Rust 1.93, production/shared-memory/independent-smoke wasm and formatting pass. Actual OS notification delivery, foreign native execution, numeric costs and complete refresh remain separate scope. |
| Implemented: named platform mechanisms | The shared Apple, startup/live-windowing and Web text/viewport successors below resolve these source findings through current owners. Native-font work also corrects already-matched Windows selections and private fallback, including gaps retained by the fixed target. | The [shared Apple record](../verification.md#shared-apple-framework-topology) supplies actual foreign compilation; the [native-font record](../verification.md#native-font-reselection-and-retained-faces) retains executed Windows and prepared Mac behavior separately. iOS remains unselected. Native execution and complete platform qualification retain the limits recorded in the fixed-cycle exit. |
| Public API evolution | The target changes event, headless-renderer, font/text and benchmark APIs. Its generic `View`/`ViewElement` path from `74b5207744ef95505fd75c56bf2c9d4736e931ee` coexists with `Render`/`RenderOnce`; AnyView embeds through ViewElement. View, accessibility and conditional hot reload now have their separate scoped qualifications. | Preserve ordinary public consumption and the independently locked consumer. The November 9 bounded residual pass identifies animation, spring and notification mechanisms above; display, missing-glyph and pointer-capture paths are relocated rather than absent. This pass is not an exhaustive residual inventory or complete API parity. |

The subsequent November 9 [bounded Core residual inventory](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#bounded-core-residual-inventory)
finds five additional concrete fixed-target mechanisms: gesture recognition/axis locking,
randomized element-tree workloads and full-render equivalence, sendable foreground submission,
asynchronous clipboard/error classification, and macOS add-only credential insertion with Apple error
semantics. Their exact fixed-target owners, complete regression scope and platform limits are
recorded in the plan. Sendable submission and randomized trees now have their complete
qualifications below; asynchronous clipboard and credentials have their separate qualifications.
The [gesture/scroll successor](../verification.md#gestures-scroll-sources-and-hover) now qualifies
its complete portable owner and selected producers with focused fork controls. Previously
qualified benchmark, scheduler and input units retain their own scope; these scoped results
do not establish exhaustive fixed-target parity.

The [sendable foreground/Web Fetch successor](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#sendable-foreground-submission-and-web-fetch-successor)
is implemented at local `ff4b3f4b95d48410c7c1d3820e2b2a98444aacab`. Its official
`2c2f737ab2` handles and complete worker/main/stream/backpressure/cancellation consumer reuse
existing owners. Actual Fetch/queue browser workflows, stable WebGL/WebGPU/Auto smoke,
independent enabled/disabled 83/67, disabled Core/public 1,351 and components 2,974 pass.
The [verification](../verification.md#sendable-foreground-submission-and-web-fetch) records
the original 2,233/2,235 runtime aggregate with two topology-related failures and nine skips,
the unchanged cases/native Send 4/4 rerun, and actual compiler-floor/wasm/static checks.
Queue prerequisite `72676731163` and exact dependency selections retain distinct provenance.
The randomized-tree/platform-tree and subsequent residual successors below resolve the later
named source findings. This is not complete API parity or engine adoption.

The [randomized-tree/platform-tree successor](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#randomized-tree-and-platform-workload-successor)
is implemented and qualified at `b42295d0e535e4105470e80873a6ad7832e81863`, after `f6f3d98d156`.
Complete official model/mutations, seventeen tree groups and ordinary consumers retain their scope. Its two render
visibility declarations deliberately use the existing SubtreePresentation owner: fork
`112798a1d13` retired the upstream late-paint API under Accepted ADR 0022. This preserves layout
and all sixteen mutation controls while adding the fork's admitted cross-channel suppression;
it is not a missing dependency or a claim of source parity. The [record](../verification.md#randomized-trees-and-platform-tree-workloads)
retains actual compile/review corrections and final-source focused 181, complete framework/
public/Gallery 2,014, components/docking/Canvas 2,974, dev/stacker and release/non-stacker
platform 115 each, independent benchmark 64 and final eight-node native/animation 2 passes.
The fork wake prerequisite is separately committed at `1756e8f79aa`, and keyboard-only native
fixture isolation at `515e4d77c16` and `36e2567b16c`. Final complete Windows passes 242/242
with nine skips, preserving both original 241/242 aggregates. Independent consumers, actual
Windows Rust 1.93, production wasm, exact feature graphs and static/format gates pass.
Other residual mechanisms and full refresh remain open.

The [asynchronous clipboard/Web paste unit](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#asynchronous-clipboard-and-web-paste-successor)
maps official `d70c45e5bb` plus text-write/paste and key-default prerequisites to existing
Platform/App/input-handler/Web listener owners. It is implemented and qualified at
`5209f4e82c7cef3c095807fcf13cd16fe860b5c6`, after `770463485ac9`: focused 34, complete framework/
public/Gallery 2,028, components/docking/Canvas 2,985, complete Windows 243, ordinary enabled/
disabled consumers 96/69 and disabled Core/public clipboard 1,379 pass.
The complete browser gate executes real text/image reads and trusted paste plus distinct error,
cancellation and current-handler controls. The [record](../verification.md#asynchronous-clipboard-and-web-paste)
retains the actual fixture failures, review corrections, Windows Rust 1.93, locked checks,
correctness Clippy, production wasm and final format/static gates. All nine lanes close. The
[desktop successor](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#desktop-clipboard-normalization-and-owned-pasteboard-successor)
qualifies Windows NUL/hash correction at `a7e47b311aa6a721c5dd0991c36893694db13043` and prepares
complete typed Mac pasteboard source at `fe5bb458f2ef23c0def7f8e19aa2327e404504b4`. Windows 244,
public clipboard 7 and independent clipboard 5 pass. Mac native compilation and twelve tests
remain open after the actual foreign-host check stops at earlier generated media bindings.
The [record](../verification.md#desktop-clipboard-normalization-and-owned-pasteboard) preserves
platform limits, exact lock edges, review and final Windows/compiler/static gates. The default
desktop ready adapter does not add OS-error classification, and the target does not provide
HTML/RTF content or browser-request abortion.

The [credential successor](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#credential-storage-and-add-only-keychain-successor)
is implemented at `3f943ea373f4da87a4c8b0f7d5a84e7101874824`, after the separate fork clipboard
fixture correction `510cfc5613dd02d38d30f2497587cf6abb2995d9`. It adopts complete official
add-only/cancellation, Windows copying/background I/O and the exact Linux prompt-stream source.
Framework atomic insertion is macOS-only with an unsupported default; application-provider
read-then-write policy remains outside the Apache framework closure. Focused Windows 10,
final complete Windows 255, public enabled/disabled 2 each and ordinary independent 2 pass.
The [record](../verification.md#credential-storage-and-add-only-keychain) retains the native
red baseline, actual status/Submitted/resource retirement, two full clipboard failures,
fixture corrections, locked/compiler/static gates and exact dependency changes. Mac source
and tests are prepared, but Apple compilation/storage/atomicity and Linux Secret Service
termination remain open. This is scoped qualification, not an exhaustive refresh audit.

### Remaining Public Platform Mechanisms

A bounded November 10 comparison of the fixed target's Platform trait, App entry points and
public exports identified these additional missing mechanisms, now resolved by the successors
in this table. It accounts for fork renames and split owners; it is not an exhaustive comparison
of every backend or framework file.
The credential successor alone did not close this list. The later owners use complete official
mechanisms as the baseline, adapting fork-owned lifetime and admission contracts around them.

| Mechanism | Fixed-target provenance | Current difference and consumer consequence | Disposition |
| --- | --- | --- | --- |
| Implemented: runtime platform identity and keybindings | `389c22e1c747d1a433c3b51c176936166c9cc1cf`, `platform/platform_info.rs`, keystroke/keybinding parsing, formatting and dispatch. | `70b993c9660772b157546f46c4a65c0ebd4375e6` carries backend/App identity, official static/dynamic parsing and host-explicit labels through fork command/component/DevTools consumers. Browser identity is cached once per WebPlatform; source-label click/invalidation corrections retain separate fork provenance. | The [runtime identity record](../verification.md#runtime-platform-identity-and-keybindings) qualifies final focused 92, framework/public/Windows 2,130, command/components/DevTools/Gallery 1,230, UI core/docking/Canvas 2,077, independent all/profiler/off 102/88/72 and disabled Core/public 1,393. Actual native timeout/retirement and complete browser controls pass; Windows compiler-floor/static gates pass. Foreign native execution, numeric budgets and the real engine seam remain separate. |
| Implemented: startup and live windowing mode, scoped qualification | `f37989fbdf840b308892af66516c2b9c3ef3853a`, display refresh `f495dd0f77b`, activation `d86aa6e6b9f` / `4d847859f550`, stale URI reads `87a1ea30e8`. | `c1d8204f2da7069f40ea8e0420a37a5c0f210474` supplies official APIs, persistent Linux runtime/replaceable connection, Windows delayed GPU/session attach and Mac application policy. Concrete validation retains exact AppCell/native retirement custody; owned startup sockets preserve a running Host's environment. The official calloop idle source stays pinned, with explicit owned timer shutdown and independent-executor lifetime preserved. | The [record](../verification.md#startup-and-live-windowing-lifecycle) retains Core/public all/off 126 each, final Windows 283 plus ten skips, Linux library 162, final Lavapipe native 4, components 2,990 and independent all/off 115/83 passes. Locked/compiler-floor/Clippy/wasm/fmt/static gates pass. Failed aggregates, fixture corrections, default WSLg thread-exit SIGSEGV and uncompiled Apple native scope stay explicit. Continue native quit/Restart Manager and shared Apple topology; this is not external-loop/shared-GPU support. |
| Implemented: scoped activity, scoped qualification | `450775244fade36db70af2447be24e74edb43368`, Windows lifetime fix `d2074f4e15d289345a9583ef973470b8bdfad74a`, Apple activity placement after `40180d9c40e2d20eb63d388bff920818f2910b53`. | `2459aebecc90fb2bd2d5fefd04b7b4548f3652bc` carries ActivityGuard, App/Platform acquisition and executor/dispatcher App Nap through existing owners. Windows retains the exact reason allocation through closing; Linux ports complete portal acquisition/timeout/close and a typed headless predicate. Mac adapts the typed activity to its current local module; shared Apple topology remains separate. | The [activity record](../verification.md#scoped-activity-and-idle-sleep-prevention) qualifies focused 31, ordinary native 3, complete ordinary all/off 108/78, native follow-up 8 and disabled Core/public 24. Complete runtime 2,064/2,065 retains the unchanged generic-view topology refusal. Actual Linux protocol 11, final headless 1, Core/input 22 and public 9 pass; both windowing features compile. Windows compiler-floor/Clippy, production wasm, fmt/static gates pass. Linux all-feature screen capture lacks PipeWire; Mac native execution and OS power enforcement remain separate. |
| Implemented: application restart arguments, scoped qualification | `715076597926d566d560b30b806749e1a2166080`, Windows PowerShell corrections `4c6c4750d3f87f0b93c937c9bda7b50639c0d07d` / `bcf6582ce3500df93a8a39366640173e6786cea6`. | `175a82b47f10b0bc2bb4b5920d54e1be5d70f977` forwards one-shot native OsString arguments through Application/App/Platform and the official launchers. Exact Env deletion corrects a reproduced official PowerShell defect; the fork's queued native quit keeps Windows pumping through its existing terminal fence. | The [record](../verification.md#restart-arguments-and-native-relaunch) separates Core/public 1,765, utility 127, ordinary all/off 105/75, disabled selected 59 and final native follow-up 10 passes from complete aggregate failures 2,160/2,161 and 268/269 on unchanged clipboard/keyboard controls. Locked checks, Clippy, actual Windows Rust 1.93, wasm profiler off/on and fmt pass. Native argc/argv/target/environment/quit/cleanup and the target negative control execute. Failure causes, Linux/Mac execution and Restart Manager's separate return contract remain open. |
| Implemented: app-wide native appearance, scoped qualification | `8e4e5a39eed932d943cb7a3e7078885b75d1ea54`, typed setter `d86aa6e6b9f52046fb299ab57ec884b300190813`, conversion `10cfb459449a`. | `0b0ea5416656c2c43c242c01e2a665e21d3814ba` carries official Some/None Mac override and non-Mac no-op through App/Platform. Typed retained appearance and the exact native window are queried outside the window state lock; the dedicated native probe uses a real main-thread entry. | The [record](../verification.md#native-window-appearance-and-platform-no-op) qualifies Windows focused/disabled 4 each, callbacks 23, corrected App owner 43, independent all/off 3/2 and actual Linux 4, plus locked/compiler-floor/host Clippy/wasm/fmt/static gates. Foreign Mac compilation stops before this owner at inherited media binding generation. Mac GUI execution, titlebar pixels and actual system-theme following remain open; no whole Apple topology or engine UI default follows. |
| Implemented: native quit completion and Restart Manager | `7f2a2c3c3ee2f23f28772dee7661fb98d3910990`, synchronous on_quit result and Windows session shutdown. | `10f14f08be8572eaa802e4fb1cfcaf2d05e268c3` uses the existing ordered Quit event, bounded post-borrow convergence and exact retirement predicate. Windows preserves registered-generation routing, terminal acknowledgement, callback-panic containment and its existing ExitProcess policy. | The [record](../verification.md#native-quit-completion-and-restart-manager) qualifies Core 7, corrected Windows 6 and independent all-target compilation, retaining initial Windows 4/6 and corrections. Remaining sync trusts unchanged official coverage and focuses on affected fork seams. Foreign signatures add no termination wait policy/execution; shared Apple topology and final residual closure remain ahead of the real engine consumer. |
| Implemented: shared Apple framework topology | `40180d9c40e2d20eb63d388bff920818f2910b53`, `95d35c6efddfa6dc34f99bb1873620bd74af806a` and the fixed-target typed-binding, font-size, color-space, shader and screenshot follow-ups. | `8bd39cef78dbb7c477df08f8fc2e1ee386f22851` owns shared Metal/atlas/shaders, dispatch/activity, CoreText/OpenType, Keychain, process information and Metal display links in open-gpui-apple. Mac retains AppKit and the CoreVideo registry. Old compatibility/duplicate owners and unused dependencies are removed; exact fork clip/atlas/presentation and font-selection contracts remain. | The [record](../verification.md#shared-apple-framework-topology) qualifies affected host/ordinary compilation, portable 2/2, actual Apple/Mac production and all-target foreign correctness Clippy, and the independent Apple Keychain API. Earlier failures/type corrections remain explicit. Native Apple execution, iOS, numerical costs and the real engine seam remain separate. Continue final fixed-target residual reconciliation. |

The original comparison identified resolved-font fallback caches, font-generation/layout
invalidation, UAX #14 and text synchronization as coherent work, together with Taffy 0.10 to
0.15. Their subsequent text/layout qualifications above own the implemented scope; these
historical inputs do not reopen those completed units. The fixed target selects Rust 1.98.1.
The WGPU successor explicitly advances this fork's declared floor from 1.92 to 1.93 with actual
Windows qualification. Validate compiler/dependency requirements per owning change rather than
copying the upstream manifest. Selected qualified units still do not complete the whole refresh.

The earlier display successor qualifies production wasm compilation and records its failed Web
test-target attempt in wait-timeout 0.2.1 before the backend. Later proptest/randomized-tree and
browser qualifications above own their updated source and actual execution evidence. Preserve
the [original failed attempt](../verification.md#connected-displays-and-window-association)
without treating the old proptest pin as a current pending task or claiming a version difference
alone fixed it. Foreign-native execution retains its separate platform gates.

### November 10 Residual Source Reconciliation

The post-Apple audit compares committed fork `b7eb45c0762b70d080af5a09f64536fe1bf22809`
against the fixed target and the content-verified import anchor. It finds inherited omissions as
well as later additions. The following is a source inventory, not executed qualification, an
exhaustive completion claim or a new upstream target. Adopt complete official mechanisms by
default; reconcile fork adaptations through their current owners. Existing qualifications remain
valid in their recorded scope. The [owning plan](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#final-fixed-target-residual-reconciliation)
orders implementation and applies targeted upstream-trust verification.

| Core/layout unit | Official source | Disposition and integration boundary |
| --- | --- | --- |
| Cached element identity and diagnostics | `1dd9bbfd71`, `d94887e08e`, `c408f8efc4`, `fc952d52da`, `20fa2fa809`, `dc682ef120` | Implemented at `1a14b2076ed`; the [qualification](../verification.md#cached-element-identity-diagnostics-and-list-precision) records final Core/public 32, official logging 1, affected compilation/Clippy and reviewed fork borrow/cache/deferred/unwind seams. Complete ID paths, current AppCell convergence and caller/error formats remain authoritative. |
| Uniform-list large-offset precision | `679ad7c304` | Implemented at `1a14b2076ed` in the same focused qualification. The complete official visible-range and local f64 subtraction replace old f32 arithmetic; actual large-offset rows, existing clip and public admitted-scroll controls pass. |
| Gesture recognition and scroll source | Complete fixed-target `gestures.rs`, touch/long-press/tooltip/axis-locking successor already identified in the bounded Core inventory | Implemented at `1b0aee8b400`; the [qualification](../verification.md#gestures-scroll-sources-and-hover) records focused Core 33, portable Web 4, independent API 6 and one actual WebGL browser trajectory, plus affected compilation/Clippy and locked wasm. Official recognition, physics, prediction and canonical scroll source share current Window/Div/backend owners. Fork executor-clock, terminal listener custody, callback generation/ingress and committed handler bounds retain focused controls; unchanged official algorithms are trusted. Complete Web IME and standalone iOS remain separate. |
| Image element geometry | `59b2ebf103`, `58df5a14ce` | Implemented at `ae10ff73c3e`; the [layout/drawing qualification](../verification.md#element-layout-svg-and-provisional-list-measurement) records final Core/public 90, WGPU 3, native Windows 1, independent 1 and affected compilation/static gates. Explicit aspect ratios and separate element/image boxes use the official crop/corner mechanism with fork clip/transform/texture-lease custody. |
| Provisional List measurements and anchoring | `f41ac64812`, `d9e35975c2`, `00cba838ad` | Implemented at `ae10ff73c3e` in the same focused qualification. Readiness/provisional/pending-end, uniform height hints/reset and pause-following-tail share one anchor owner. Indexed direct scrolling is scroll_to_item under ADR 0025; final nested reveal remains Window-owned. Admitted autoscroll and direct revisions retain their reviewed fork scope. |
| Container queries | `49ad06c1b4` | Complete official ContainerQuery is implemented at `ae10ff73c3e`. Actual same-frame flex/resize, deferred scopes and failure/recovery pass. The record retains both new fixture errors/corrections; children cannot resize the container. |
| Attached anchoring | `ce6c5413f1` | Complete Placement::Attached, corners, offset flips and client-inset overflow are implemented at `ae10ff73c3e`. Official scenarios and an actual transformed-target/portal consumer pass through the existing geometry owner. |
| SVG data, parse reuse and raster bounds | `ff9f114cd3`, `00cba838ad`, `03c9c4e707`, `fbd911ed3e` | Svg::data, ParsedSvg/render_parsed, ExactSize and the 8192px cap are implemented at `ae10ff73c3e`. The production renderer matches the fixed target. Actual byte-source/exact-clip/transform and ordinary raster consumers pass; rotation retains the ADR 0021 gap. |
| Padding and hover policy | `b1a7ef0cf6`, `f0d8b0b0cd` | Implemented at `1b0aee8b400` through existing layout, presentation and admission owners. Focused fractional-padding, stationary-pointer layout, keyboard modality and listener-only policy controls pass in the same qualification. No unrelated component/compiler matrix or numeric-performance claim follows. |
| Cursor and external-drag listeners | `c7bea34383`, `693962917b`, `1a84d5d92b`, `f52fd9ac44` | Implemented at `fc6c9dc4403`; the [qualification](../verification.md#cursor-and-complete-native-file-drag) records final Core 18, Linux URI 5, independent API 3 and affected host/Mac all-target compilation/Clippy, Linux production compilation and format/static gates. Official lazy payload, source reentry and cursor timing retain per-window output, targeted geometry, candidate listeners and exact native generation/ingress. Both reviewed session defects are corrected; real Mac/Wayland cross-application execution remains unqualified. |
| Cosmic selected face and Web Canvas fallback | `7960b2a7c9`, `87f65de697` | Implemented at `f2526df594a`: complete official policy/eligibility/measurement/raster/cache/run replacement and selected-face reporting use existing TextSystem/atlas owners. The [qualification](../verification.md#web-canvas-font-fallback) records Cosmic 52/52, final Web 47/47, ordinary independent 1/1 and complete actual WebGL/WebGPU/Auto smoke, affected host/wasm correctness Clippy, optional shared-memory wasm compilation and format/static gates. Reproduced Inter family and UTF-16 import defects are corrected; obsolete textarea-blur controls follow official browser activity. All five lanes close without reviewed P1/P2. Canvas algorithms retain official source; sink release, stock fonts and input fixes retain fork provenance. Browser font coverage elsewhere, OS IME, numeric costs, the explicit fallback-chain limitation and engine adoption remain separate. |

| Supporting owner | Official source | Disposition and dependency boundary |
| --- | --- | --- |
| Darwin process and unified reaper | `e1bfcf85db`, `60ed56b372`, `d4cc8d2409` | Implemented complete owned File/stdio/SIGPIPE/Async<File>/adopt_raw_pid mechanism. The [qualification](../verification.md#process-reaping-dedicated-execution-and-shared-strings) records one actual async-process `0b6d6713` shared by smol across all six roots, utility/public-consumer Darwin source compilation and retained Windows Job cleanup. Darwin process execution remains unqualified. |
| HTTP deadlines and TLS initialization | `992c7d469c`, `865318723e`, `c595091b0a` | Implemented complete RequestTimeout/read deadlines and platform verifier/WebPKI fallback. Official reqwest `33bc764a` replaces the registry reconstruction cache with request-local redirects on the original client. The [qualification](../verification.md#http-deadlines-release-downloads-and-pax-archives) records native HTTP 9/9 and independent public 4/4; originating Tokio body polling is the bounded runtime adaptation. Actual TLS fallback and foreign execution retain separate scope. |
| Release-download helper | `ead2d9eac0`, `02d47bf64f`, `992c7d469c` | Implemented separate Apache open-gpui-github-releases with release deadlines, digest normalization, status validation and staged archive/raw Task downloads. Superseded lower-HTTP modules and native dependencies are removed. The same qualification records release 10/10, Windows file-close/error cleanup and actual public consumers; active Task-drop cancellation remains unexecuted. |
| PAX archive parser | `87324045be`; async-tar `bd3ad6f89df9a9da7a8535958756d6bf465936a0` | Implemented actual fixed Git dependency and retired the Tokio bridge. A real verified TarGz download extracts the correct following path after an embedded-newline PAX value. The download package carries Windows async-std/unstable support directly; browser/Core-only roots shed the old native archive closure. |
| SharedString and executor helpers | `47825fe00a`, `9dd6993e2e`; `2040e0de59`, `98c6c1403b` | Implemented complete From<char>/SmolStr serde and DedicatedExecutor/nonblocking rendezvous/wasm-threads/native-only blocking successor. The same qualification records affected fork cancellation/admission/shutdown and public-consumer passes. One reproduced official queued-task sender cycle uses bounded weak dispatch ownership; other official runtime mechanisms remain intact. Browser worker/shutdown execution is not implied by production wasm compilation. |
| Portable path owner | `f181a2f47b`, `2610332077`, `a84689073d`, `51a3a0e971`, `f929b3f7ce`, `a4a3a968d1`, `9521840152` | Implemented at `a4e6fd03642` as the complete Apache open-gpui-path package, with actual utility/public consumers and duplicate util types removed. The [qualification](../verification.md#portable-paths-and-utility-consumers) records focused 89/89, independent 5/5, actual Rust 1.93/wasm package compilation and affected Windows compilation/Clippy. Exact source differs only in Rustdoc links and one stable empty-Path spelling. This is distinct from the GPL product paths crate. |
| Path-position utilities | `a51d23c93d`, `72d073d642`, `347f171a3f`, `e21ff1b368` | Complete fixed-target UNC/position parsing and public parse_path_with_position are implemented at `a4e6fd03642` through the actual util owner. The same qualification includes independent matching/order/navigation composition; product file-existence fallback remains outside the framework closure. |
| Shell command construction | `ce48461eaa`, `c578f4d12b`, `7ca96c2a58`, `18f9d7e731`, `b41a8b60a4`, `4c6c4750d3` | Complete target shell/builder/environment mechanisms are implemented at `8e0517f5650`, with canonical low-level native command/Windows discovery owners and duplicate definitions removed. The [qualification](../verification.md#utility-composition-shells-and-development-assets) records final focused 47/47, existing restart 11/11, affected compilation/Clippy and retained fork test/consumer seams. Unix real-shell/descriptor execution remains unqualified on this Windows host. |
| JSON merge and development assets | `d61edb0825`, `b47d8ac455`; `ee6badf4b0`, `d3e54c97f4`, `2551721adb` | Implemented at `8e0517f5650`: replacement merge, separate deduplicating union and complete dev_repo_root/fs_embed/debug-embed share the official owners. Final independent dev 4/4, embedded 4/4 and release 1/1 pass. A real 8.13 trait error receives one bounded macro/dependency adaptation; patched compression dependencies pass existing health/audit gates without ignores. Both core-util and complete higher-util wasm compilation pass; the latter uses installed LLVM 22.1.6 after the initial missing-clang failure. |
| Remaining lightweight helpers | `887149bbdd`, `d7adf3e7e2`, `54f2b5df1d`, `1102219f81`, `1057c2cf3d`, `d12e456be7` | Lenient globs/extension sorting are implemented at `a4e6fd03642`; fragment positions, environment-name validation and explicit Unix soft-FD setup at `8e0517f5650`. Canonical core-util helper moves remove duplicates. Actual public composition is qualified; Unix file-limit execution remains unqualified. LocalExecutor::spawn_with_dispatch was already adopted by the support-primitives unit. |
| Logging dependency provenance | `ac5af8b9e1` | Implemented at `9e7b9c3a36f`: the Apache zlog/ztracing/ztracing_macro owners and SumTree instrumentation supersede the earlier ordinary-tracing adaptation. ADR 0001 corrects its historical GPL description; GPL application settings remain excluded. The [logging qualification](../verification.md#standalone-logging-and-tracing) retains explicit instrumentation choices and host-owned globals. |

| Backend/render unit | Official source | Disposition and existing owner to preserve |
| --- | --- | --- |
| Native state restoration and attention | `33c6212b055`; `905e955a702`, `50e399332c1`, `4d1935b8d00` | Implemented at `9b671ed5148`: complete Mac archive/restoration rules and Core facade, Windows single flash, Mac informational attention and X11 urgency lifecycle share existing native owners. The [qualification](../verification.md#native-window-state-and-attention) records Core/public 25/25, actual Windows lifetime/no-op 5/5, independent default/runtime 1/1 and 2/2, affected host/consumer compilation, Mac foreign correctness Clippy, Linux X11/Wayland and Web production compilation. Two reviewed fork lifecycle defects and binding/toolchain failures are corrected. Four lanes close without remaining P1/P2; actual desktop restoration/attention and numerical costs remain unqualified. |
| Window regions, layer-shell and fullscreen | `a29c0d41f47`, `166f044fd04`; `2893b86b048`, `242fe31a399` | Implemented at `77b56a6af1b`: complete official Wayland None/empty/rect input regions, runtime zone/edge validation and Mac saved frame/style/multi-window presentation custody use current owners. The [qualification](../verification.md#native-regions-layer-shell-fullscreen-and-popups) records Core/public 124/124, disabled close 6/6, Linux focused 26/26, Windows native 3/3, independent default/runtime 2/2 and 4/4, affected host/Linux/consumer checks, Mac foreign/Web compilation and format/static gates. Nine scoped lanes close without P1/P2; actual Apple and layer/input-region behavior retain explicit native gates. |
| Web text and viewport | `224373ab888`, `8fc1a8a0c6f`, `d84e5d4992b`, final target follow-ups; `6ad3c7f278e` | Implemented at `7a79a113733`: complete official configuration, textarea/beforeinput/selection/composition/editable range/keyboard and visual-viewport/safe-area owners use accepted sessions, leased handlers and the existing post-borrow FIFO. The [qualification](../verification.md#web-text-sessions-and-viewport) records final focused 16/16, affected Core/public/Windows 53/53, independent default/runtime 1/1 and 2/2, actual text/clipboard/identity/touch browser passes, affected compilation/Clippy, Linux compilation and format/static gates. Nine lanes close without remaining reviewed P1/P2; original failures remain explicit. The Canvas successor above separately corrects inherited UTF-16 import and old activity-test expectations. Actual OS IME/mobile keyboard/safe areas, Apple execution and numeric costs retain separate scope. |
| Native popup and file drag-out | `546a16d64fa`; `f52fd9ac44b`, `a8491e63b54`, `72c53bf0aae`, `c7aea6cbbd4` | Complete Mac/Wayland file source/session/URI/complete/cancel/reentry is implemented at `fc6c9dc4403`, with target UTType icons and exact native sources. Complete popup options, Wayland XDG/layer/nested parent roles, configure/reposition and child-before-parent destruction are implemented at `77b56a6af1b`. The same control qualification records actual nested-Wayland popup 1/1 and existing X11/Wayland round trips 2/2. A reproduced premature native-terminal retention defect uses a required-close request through the existing retirement owner; reserved creation and quiesced dismissal corrections retain red/green controls. Explicit grabs, actual compositor dismissal, cross-application drag and physical GPU/input behavior remain unqualified. |
| Source-over alpha and native child layering | `e18de65b759`, `bc6fc117403` | Implemented at `37d211ea030`: complete DirectX ordinary/path-sprite and WGPU path-sprite source-over state, plus the shared DirectComposition startup/recreation setting below child HWNDs. Apple settings were already adopted. The [qualification](../verification.md#source-over-alpha-and-native-child-composition) records affected compilation, existing actual native/WGPU consumers 2/2 and bounded review without P1/P2. Independent translucent-destination alpha, desktop child/WebView visibility and real device-loss visual proof remain unexecuted. |
| Windows dialogs and host/input safety | `a0f8fa7bdbb`; `9cb5f90e3c4`, `e462f873c62`, `23c0080d1dd` | Implemented at `34e6b6a963d`: complete dedicated STA/per-owner serialized dialogs, cancellation/error policy, duplicate MouseMove correction, registered-host accelerator routing and nullable composition lookup share their current owners. The [qualification](../verification.md#windows-dialog-threads-and-native-host-safety) records 66/67 affected passes plus the corrected full recovery consumer 1/1, actual worker/STA/retirement/input seams, affected compilation/Clippy and independent all-feature compilation. The consumer allocator edge matches official Windows 0.62.2 without package/source updates. Four lanes close without P1/P2; live modal focus/overwrite/timer cancellation and numeric costs remain unqualified. |
| Wayland IME/effects/XKB | `5079b33d657`, `14b7b0ef376`; `c43e2d97348`, `a7c7219d5bf`, `afc13dc8e05` | Implemented at `cf4bd2c6db1`: official cursor/serial dedup, preferred ext/KDE effects, rounded regions, shared XKB/null handling and single-backend feature separation reuse current owners. The [qualification](../verification.md#linux-ime-background-effects-and-keyboard-contexts) records affected 48/48, final 4/4, independent consumers and capability/observed-fact corrections. Actual KWin/Fcitx and compositor blur remain separate. |
| X11 metadata and dependency minimum | `5dd0666dfb6`, `dd68454633a` | Implemented at `cf4bd2c6db1`: exact WM_CLASS readback, keyboard-driven submission and retirement pass in the existing native trajectory. The manifest requires wayland-backend 0.3.15 and wayland-protocols 0.32.10; selected locks are unchanged. |
| Mac native input, blur and typed AppKit | `0a9eb81e6c`, `d86aa6e6b9`, `10cfb45944`, `d01f1258376`, `41126668013`, `db7f9ceed78`, `f6838a7c4af`, `382ff7a64b3`, `90d8514059`, `4b47ceb9d3` | Implemented at `0c1118e2f1a`: official keyboard/event, application/menu, window/display and ScreenCaptureKit owners replace cocoa_compat and Media. The [qualification](../verification.md#macos-native-input-appkit-and-screen-capture) records Apple-target checks, affected correctness Clippy, the independently locked capture-to-surface consumer and nullable view/retirement fixes. Native Apple execution and weak-linked startup remain separate. |
| Renderer batching and overdraw | `be8c6f9fb35`, `b4ecc454225`, `0c51c7fd248`; `e91b82c106`, `e20d19d32f` | Implemented at `36362708133`: whole-view DirectX offsets, Metal video-surface bounds, exact atlas grouping, shared sprite ABI and hollow border raster strips. WGPU skips non-rasterized path composites and encodes at the acquired extent; configure-once was already covered. The [qualification](../verification.md#renderer-batching-and-hollow-borders) records Windows WARP and WGPU pixel evidence, Apple cross-compilation and ordinary consumer gates. Exact clip and atlas leases, uncertain sprite order and native retirement retain their existing owners. Native Metal execution and numeric costs remain separate. |

The [logging, Window API and Windows dispatch successor](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#logging-window-api-and-windows-dispatch-successor)
reconciles the additional method and dependency findings:

| Mechanism | Fixed-target source | Disposition |
| --- | --- | --- |
| Logger and instrumentation | `ac5af8b9e1`, `crates/{zlog,ztracing,ztracing_macro}` | Implemented at `9e7b9c3a36f`: Apache packages and SumTree instrumentation use explicit Cargo selections and host-owned globals. GPL `zlog_settings` remains outside the framework. See the [logging qualification](../verification.md#standalone-logging-and-tracing). |
| Window controls and geometry | `e99616cdd4`, `058f01fa93`, `d85eade7ab`, `cf4deb270f`, `25b5569dd2` | Implemented at `655e9610c77`: public creation and prompt facts, outside-click suppression, owned chord pause, visible-area centering and bottom anchors use current owners. Windows already reports its work area. See the [Window qualification](../verification.md#window-controls-lazy-inspector-and-win32-dispatch). |
| Lazy Inspector | `b0f53ad23b` | Implemented at `655e9610c77`: per-window lazy identities, renderer factories and frame-map release retain selection getters and accepted-frame geometry, including factory-panic recovery. Zed Inspector UI, LSP and application benchmarks remain excluded product integrations. The same Window qualification records final Gallery controls. |
| Win32 pool and timer | `2c4e44704c`, `72656afa6d`, `d544470d1d` | Implemented at `9d9602bb46d`: the official Win32 background owner replaces the WinRT path and unused feature, retaining the exact foreground queue, callback generation and profiler. See the [Window and Win32 qualification](../verification.md#window-controls-lazy-inspector-and-win32-dispatch). |
| Keystroke input preference | `86c20cc911` | Implemented at `571484691ea`: public interceptors/observers receive the official `InputPreference`, computed before interception and recomputed after replay/interceptor effects before binding dispatch. Existing admitted input-handler custody remains. See the [input/focus record](../verification.md#input-priority-and-focus-lifetime) for final profiler on/off 113/113 and native execution. |
| Dropped focus-handle queue | `e463c5ab74` | Implemented at `571484691ea`: the private FocusMap module owns the final-strong-reference queue and App returns when it is empty. The old full-map scan is removed; existing Window cleanup still covers pending/provisional/fallback/candidate focus after releasing the map lock. No measured speedup is claimed. |
| Focus-lost ancestor restoration | `b2d9c2e122` | Implemented at `571484691ea`: the public query uses the callback-scoped old committed path and new accepted dispatch tree, restoring scope across nested callbacks/unwind. Actual public child-removal and callback-failure consumers pass. Zed workspace/agent-panel consumers remain outside this framework unit. |

iOS remains an unselected application backend; shared Core/Web mobile behavior is assessed in
its own unit. This inventory does not establish native coverage everywhere or exact source
identity with the monorepo.

Already covered mechanisms include shared image decoding, Taffy 0.15/grid-row intrinsic sizing,
owning CachedLoad, animation/Spring parent composition and relevant collections/refineable/
bench_metrics/macros/sum_tree behavior. Visibility is intentionally represented by Accepted
ADR 0022 SubtreePresentation. Do not reopen these units for renamed files or test placement.

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
is qualified by the [hot-reload successor](../verification.md#conditional-native-hot-reload);
the formerly absent debug accessibility view-type map/dump pipeline is qualified by the
[synthetic/inspection successor](../verification.md#synthetic-accessibility-and-explicit-published-tree-inspection).
Neither is inferred from the View API migration alone. Source disposition remains separate
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
| Active-descendant focus | `83d4847462`: descendant-side declaration, real focused-ancestor gating, self/duplicate diagnostics and final tree focus override are implemented at `53decdc64a2d182fad1fbf00ababcd2b55a906e7`. Final surviving semantic parentage and the fork's distinct explicit display override retain their own authority. | [Composite-focus qualification](../verification.md#composite-focus-and-keyboard-continuity) records full scoped runtime, ordinary-consumer and compiler gates, with explicit AT/cost limits. |
| Keyboard click continuity | `83d4847462`: KeyDown/focus-generation capture and one-shot KeyUp consumption are implemented. The official shared Enter/Space arm is retained. Actual focus ABA and intervening dispatch cancel it; candidate refusal preserves the admitted arm. | Same focus/keyboard owner, with actual atlas refusal and accepted retirement controls. |
| Tab-group ordering and missing-node diagnostics | `2268045a11`: the container focus handle is inserted inside its tab group; missing ID/role uses accepted-publication per-focus diagnostic dedup/reset. | Same owner, preserving scoped geometry, actual focus and irreversible quiescence. |
| Accessibility properties | The existing `InteractivityAccessibility` now owns the complete official shortcut/numeric-step/author-ID builders and writer; no parallel `AriaProperties` is introduced. | [Property/configuration/initial-tree qualification](../verification.md#accessibility-properties-configuration-and-initial-trees) records complete scoped runtime, public/final-tree, ordinary-consumer and compiler gates. |
| Initial title tree | `83d4847462`: the official synchronous root-only creation-title snapshot is implemented before the existing sequenced full refresh. Retained callbacks read original admission and native-close facts; shutdown uses the existing shared sequencer fence. | Same complete qualified owner; synchronous response, accepted full tree, Submitted frame and real AT retain distinct evidence requirements. |
| Later enabled/author-ID APIs | Official `9183ba4c55` enabled-versus-active and test configuration, plus `cc053a4a6f` author identifiers, are implemented. The fork's active-query name migrates to the official API without an alias. | Same complete public/native/ordinary-consumer unit; configuration queries remain passive. |

The first three mechanisms share the qualified focus/keyboard unit. The following three are
implemented at `d9608b1a3174f17df270d6b92894539ec32654a8`. Their qualification records complete
Core/public/WGPU/Windows 1,787, components/docking/Canvas 2,966, disabled affected 182, ordinary
type-only 6/6 in each mode and Gallery 13 passing, together with locked checks, actual Windows
Rust 1.92, production wasm, formatting and static gates. Four existing interactive skips and
the earlier undiagnosed extra native mouse event retain their recorded scope. The later
benchmark/report and hot-reload successors resolve those source owners; real AT, numerical costs
and engine adoption remain open. Product-level migration stays outside the imported closure without rejecting its
surviving official framework mechanisms.

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
| `1722fe63bc` | Complete context, measurement, parameterized macro, real text-system and renderer successor is implemented at `5f3a99fad9284c17a8eaac508ab8b4f47018be44` after the qualified threaded-dispatcher prerequisite. | Phase 2 `297c4a4d78`, existing `1f7eae7fb7`, shared seeds `2440236055` and `3135cdf8dc` are reconciled against the fixed final target. The [qualification](../verification.md#complete-benchmark-context-and-measurements) records complete scoped runtime, benchmark-only/injected GPU, independent-consumer and compiler gates, with explicit platform/performance limits. |

The benchmark owner includes inputs/input_name/group/sample_size validation and Criterion
bench_with_input, thread-local dispatcher reuse with per-context platform creation, timer release
outside its lock followed by waking/draining, bounded cancel/drain after App/foreground release,
and bounds evaluation before mutable App access. Macro parameters alone do not close that owner.

The November 8 pre-implementation audit maps the final target beyond those introductions. The
concrete official `ThreadedDispatcher` is implemented by the prerequisite below. At that audit,
`BenchAppContext` still used the virtual `TestDispatcher`; native text-system injection/cache,
scheduled-frame driving and isolated per-routine platforms were missing. `1f7eae7fb7`'s blocked
HTTP/trace foundations were present, but independent `bench-support` and benchmark-only prompt
cancellation were missing. `2440236055` adds seeded benchmark macros and shared seed calculation;
the former private test helper was not equivalent when `SEED` was combined with multiple
iterations. The final macro also depends on `Criterion<BenchMeasurement>` rather than the fork's
former plain Criterion forwarders.

`3135cdf8dc` also owns queue/dispatcher teardown fixes: final-sender drop must wake every waiting
receiver, receivers must recheck closure after waking, and the Linux timer loop must exit on a
closed channel. The prerequisite below now implements these fixes in the existing queue and
Linux dispatcher. Its qualification keeps Linux active execution separate from source review.
The audit also requires the official no-readback render operation and fallible optional renderer
factory; TestWindow draw counts and simulated outcomes are not actual GPU submission. The complete
benchmark/report successor below owns these harness/render findings, full report/allocator
semantics and actual injected headless GPU consumers. Real device-loss and platform execution
retain their separate evidence requirements.

The [threaded-dispatcher prerequisite](../plans/2026-07-05-001-refactor-zed-gpui-upstream-sync-plan.md#threaded-dispatcher-and-queue-closure-prerequisite)
starts from `1670c373e34751ac520237b7d04a480b06ab7b4a` and implements the complete
concrete dispatcher and final-target readiness/cancellation hooks alongside queue/Linux closure,
with public executor execution and existing admission/profiler contracts. Its test-support exposure
does not itself close the successor's independent benchmark features, context replacement, real
font/GPU injection or complete measurement/report consumers.

Local implementation `018fbdc5f708d91f0ae16a7fb43ac04280ebb680` and its
[qualification](../verification.md#threaded-dispatcher-and-queue-closure) record complete
Core/public/WGPU/Windows 1,810, components/docking/Canvas 2,966, disabled affected 35 and
independent enabled/disabled 62/47 passing. Four original interactive skips remain. Locked
checks, actual Windows Rust 1.92, correctness Clippy, stable/nightly production wasm,
formatting and static gates pass. The Linux fix/test is source-reviewed without active Linux
compilation or execution. That prerequisite retains ordinary dependencies' prior feature graph
and lockfile; complete context/dependency replacement belongs to the benchmark successor.

The complete benchmark successor is implemented at
`5f3a99fad9284c17a8eaac508ab8b4f47018be44`. It replaces the virtual context and forwarding
macros, adds the independent Apache-2.0 measurement package, isolates native text/platform state,
and separates optional no-readback rendering from screenshots. Independent review reproduces and
corrects inherited seed, wrapper-visibility and teardown/unwind edges separately from fork
ingress/candidate/terminal-quit adaptations. Focused actual consumers pass 60/60, the independent
benchmark executes nine cases, Canvas passes five smoke cases, and complete runtime/public/
Gallery/native passes 2,040/2,040 with five documented skips. Components pass 2,964 plus two
separately gated Docking controls, preserving the previous 2,966-test set. Disabled affected
Core/public 112 and independent enabled/disabled 62/47 pass. Locked checks, correctness Clippy,
actual Windows Rust 1.92, stable/nightly production wasm, formatting and existing static gates
are recorded in the owning qualification. Numerical costs, foreign-platform execution, additive
test support, conditional hot reload and engine adoption remain separate.

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
formatting and static scans pass within their recorded scope. Benchmark/report, accessibility and
hot reload have their separate later qualifications above; this query/List evidence does not
extend their scope or qualify foreign-platform/browser execution or engine adoption.

### Text And Layout Reconciliation Units

The original fixed-target text audit maps the following complete units to fork `229723e620`.
Implemented successors link their later qualification; original source findings alone add no
runtime qualification. Preserve `WindowTextSystem::abort_frame`, which returns
layouts moved by a refused candidate, and the fork's exact clip authority throughout the changes.

| Unit | Source-confirmed behavior and prerequisites | Owning acceptance |
| --- | --- | --- |
| Implemented: font resolution, installation and reporting | `9e6e14160c`, `3389d87577`, `b7d50b95e8`, `b68add5b37` now flow through the existing TextSystem, window cache, Cosmic and App owners. Direct/Fallback/Missing resolution and successful-installation generations preserve existing face identities. One coherent frame-cache state, pre-resolution generation capture and generation-aware width pools prevent stale reuse; exact candidate abort/retry remains authoritative. | The [font qualification](../verification.md#runtime-font-installation-and-missing-glyph-reporting) owns reproduced failures, actual Core/Cosmic/Window and native installation results, observer/task release and explicit redraw. Reporting is bounded, subscription-scoped and revalidates after App borrow waiting. That Web checkpoint uses Cosmic directly; the subsequent [Canvas qualification](../verification.md#web-canvas-font-fallback) adopts the complete wrapper and post-fallback scan. Windows/CoreText retain the optional default sink and gain no missing-glyph reporting claim. |
| Implemented: Cosmic paragraph shaping; remaining fallback policy | `c214057e08` now splits `Bidi_Class=B` paragraphs, preserves exact clipped FontRun configuration and rebases original glyph byte/x positions with merged metrics. Missing raw indices aggregate against the complete source so CRLF reports stay whole. `d6358b2d3c`'s fallback-face preservation remains implemented. | The [paragraph qualification](../verification.md#cosmic-paragraph-shaping-and-public-paint) owns red direction/reporting regressions, the corrected width oracle, actual Hebrew glyph/raster and public headless shape/paint/reconstruction. Direct `unicode-bidi` uses the already resolved 0.3.18 in both lockfiles. The no-`m` primary filter still omits that family from explicit fallback-chain selection; automatic fallback uses the retained database face. These results do not qualify general fallback priority, full RTL caret, native GPU pixels or platform parity. |
| Implemented: font preparation, inventory and raster reuse | `f1d27d545e`, `5b4a2153a0` and `1ff7cb669b` now use explicit public prewarming, backend-only family inventory and Cosmic bounds-to-raster image reuse. Shared primary/fallback match properties preserve normal shaping. The fork bounds temporary image handoff to 256 entries and 8 MiB of buffer capacity, with eviction/recomputation rather than glyph refusal. Native alias preparation also exposes the callback-identity prerequisite fixed separately at `19a4ed3434a`. `d0f797a38c`'s retained embedded-font source storage remains implemented. | The [qualification](../verification.md#font-preparation-inventory-and-raster-reuse) retains the inventory/alias red baselines, actual query/match identity, cold pixel/Swash-generation controls and native installation/retirement. Final all-feature Core/WGPU/Windows plus public text passes 1,268/1,268 with four existing interactive cases skipped; affected components/docking/Canvas pass 2,964/2,964 and independent public API tests pass 5/5. Locked compilation, correctness Clippy, formatting and actual Windows Rust 1.92.0 affected-framework/consumer checks pass. Existing atlas/family/width-pool hits are controls; numeric frame/process costs remain unmeasured. |
| Implemented: wrapping, indentation and truncation | `f0210f1621`, `4f70d91bda` and inherited `03a8544040` now flow through the existing wrapper, shaped-layout, style and TextRun owners. UAX opportunities span fragments/elements; explicit indentation, all three truncation directions and source/visual grapheme grouping preserve font generations and candidate recovery. Additional corrections retain carried End content and measure Start's actual row budget. | The [qualification](../verification.md#unicode-wrapping-indentation-and-middle-truncation) owns reproduced baseline/review defects, public style/paint, exact native Submitted frames, reconstruction and dependency/compiler evidence. The target's non-complex-scripts segmenter remains selected; general complex-script/UAX #9 caret and logical reconstruction of reordered rows remain open. |
| Implemented: text split, layered paint and underlines | `2936989f1b`, `f792c2d72d`, `bda9c0bd43` and `0dea8c63d9` now map to raw layout split/paint, a forward cursor, public TextLayout passes and underline callbacks/exclusions. Source-index partition fixes the old visually reordered split's reproduced index underflow. Shared row geometry and existing projected clip owners retain the original glyphs and full wave phase. | The [qualification](../verification.md#source-fragments-text-layers-and-underline-exclusions) retains UTF-8/monotonic boundaries, independent visual-RTL membership, aligned/wrapped layer footprints, inserted highlights and zero-advance callbacks. Both owner defects are reproduced before correction. Focused public/native 19/19, complete Core/WGPU/Windows plus public text 1,281/1,281, affected components/docking/Canvas 2,964/2,964 and independent API tests 6/6 pass; four existing interactive Windows cases are skipped only in the aggregate. Locked compilation, correctness Clippy, formatting and actual Windows Rust 1.92.0 framework/consumer checks pass. General bidi reflow/caret, existing ink overhang, numeric costs and the remaining refresh keep separate scope. |
| Implemented: window-local layout ownership | Official `772a8a9100` supplies Rc/RefCell window caches and local font-run scratch while retaining global Arc<TextSystem>; final-target follow-up `dde76a1dca` uses Cell take/set for scratch. Official `2a74c8dff6` removes unused hash APIs/caches. The fork adopts those owners and deletes the parallel hash path. | The [qualification record](../verification.md#window-local-text-ownership-and-source-cache) separates official mechanisms from fork generation coherence, rejected-candidate recovery, backend reentry and a reproduced inherited clamp-key correction. Complete Core/WGPU/Windows plus public text passes 1,292/1,292 with four existing interactive cases skipped; affected components/docking/Canvas pass 2,964/2,964 and independent public tests pass 7/7. Full source, automatic notified frames, reentry/unwind, shared font installation and exact two-window/native lifetimes retain their own scope. Locked compilation, correctness Clippy and actual Windows Rust 1.92.0 framework/consumer checks pass. Rc/method removals are explicit breaking changes; numeric costs remain unmeasured. |
| Implemented: Taffy and intrinsic measurement | Official `91fdd55889`, `b05f40c554`, `31ceaf7908`, `cdf33ac25e`, `b488c4e814` now map to Taffy 0.15 leaf/alignment APIs, explicit relative position, separate truncation width, actual shaped-overflow checks and MaxContent list probes. Both qualification lockfiles select 0.15.0; the library permits compatible patches. | The [qualification record](../verification.md#taffy-text-measurement-and-rootgrid-sizing) retains the reproduced inherited ordinary-wrap defect and fork exact-width correction, actual kerning/source/paint consumers and initial fixture failures. Corrected Core/WGPU/Windows plus public layout/text passes 1,312/1,312 with four existing interactive cases skipped; affected components pass 2,964/2,964 and independent public tests pass 9/9. Locked compilation, correctness Clippy and actual Windows Rust 1.92.0 checks pass. No general word-based MinContent or measured speedup claim follows. |
| Implemented: root/grid sizing and stack policy | Official `b0da438545` fills auto window/prompt roots without stretching anchored/tooltip/drag roots. `5ccbbbd88f` adds grid-row min/max-content and renames the shared min-size type. `41d72f6c1d` now makes stack protection opt-in across both Taffy callbacks and Div's three phases. | Auto/explicit/percentage/minmax roots, prompt, notify/resize, shrink-wrap and actual grid geometry pass within the same qualification. The stacker-disabled owner/portal cohort passes 33/33; the all-feature complete runtime enables stacker. Portal root-inset and multi-Sidebar visible-allocation fixture corrections preserve original identity/input/transform assertions. Both feature paths and native reconstruction retain their stated workloads, not arbitrary-depth safety. |
| Implemented: CoreText identity and size | `20a93f6195`, `44ec5691b5`: shared Apple `text_system.rs` deduplicates PostScript identity within one family load and preserves the exact first nonempty run size, including empty/BOM leading runs. | The [shared Apple successor](../verification.md#shared-apple-framework-topology) supersedes this queued row; fixed-tree source comparison at `424a3a0f7c2` confirms both mechanisms. Production/all-target foreign compilation is recorded separately from unexecuted native CoreText/font tests. |

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
