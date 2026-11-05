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
| `03a8544040` | Missing: November 5 correction | The July classification was incorrect. The current fork's `TruncateFrom` has Start/End only; Middle and its run redistribution remain absent. Reconcile this earlier gap with the complete truncation/wrapping unit. |
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
source findings. The arena, TextSystem and cached-dependency successors below now have the
executed scope in the [verification record](../verification.md#cached-dependencies-deferred-replay-and-opacity);
the other two units remain queued. Do not overwrite framework or docking files from the upstream
tree.

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

### Source-Confirmed Follow-Ups Beyond The Initial Units

These rows come from the fixed-target source diff. They are queued reconciliation work, not
executed regressions or an exhaustive disposition of the 364 commits.

| Owning area | Target evidence and current fork difference | Required reconciliation |
| --- | --- | --- |
| GPU primitive representation | `f2817700349` replaces GPU-facing Rust `bool` with `PaddedBool32`; the fork's `scene.rs::PolychromeSprite` still has a `bool` before `opacity`. | Adopt an explicitly initialized GPU representation through every shader/backend and constructor, accounting for the fork's additional transform/clip fields. Verify layout and real renderer output; a struct declaration alone does not qualify buffer writes. |
| Windows color transfer | `b9419ae7281` corrects reversed linear/sRGB power directions in `gpui_windows/src/shaders.hlsl`; the current fork retains the old directions. | Port the transfer correction with shader/gradient color evidence before treating an engine color comparison as valid. This source finding is not a color-space performance measurement. |
| Image decoding consistency | Target `635026fabc` shares animated WebP decoding between `Image::to_image_data` and the image element loader; the fork's former path still returns a single frame. The target also relies on `9552acc2bc` static decoding/EXIF helpers absent locally. | Reconcile both decode paths and EXIF behavior, preserve the two-frame/delay regression and static/BGRA/alpha controls, then verify actual window frame progression. Decoding alone does not qualify native playback or atlas/cache replay. No source prerequisite currently establishes a need to raise the compiler floor. |
| External run loop | `74798c68d5c` adds `Application::run_embedded` and `ApplicationHandle` in target `app.rs`; these public APIs are absent from the fork. | Reconcile the actual externally driven App lifetime, callback/borrow rules and shutdown through public consumers before inventing a separate engine attachment. This does not itself provide shared GPU ownership. |
| Atlas and renderer lifetime | Target `platform.rs` adds shared `AtlasBackend`/`AtlasState`; `96e95edac45` skips released-texture sprites, and WGPU adds texture generations and bind-group caching. The fork instead has per-instance lease generations and candidate-frame refusal. | Compare the lifetime behavior in the existing owner, including stale resource and rejected-frame controls. Preserve admitted candidate and lease semantics while evaluating deeper shared atlas code; copying upstream's skip policy would change the product contract. |
| Platform closure | Target macOS shares `gpui_apple` and uses `objc2`; Linux gains switchable headless/windowed modes and rendering; Web adds backend/focus/input changes. | Map trait and manifest changes across the actual supported backend closure. Treat iOS as a separate platform decision and retain unexecuted native-platform limits. Existing Windows-only evidence cannot qualify the other backends. |
| Public API evolution | The target changes event, headless-renderer, font/text and benchmark APIs. A historical commit temporarily unified `Render`/`RenderOnce`, but the fixed target still defines both traits. | Compare final target blobs and follow-up/revert history before migrating consumers. A commit title alone is insufficient evidence for an API change. Keep the independently locked consumer in the verification path. |

A subsequent coherent text refresh should assess resolved-font fallback caches together with
font-generation and layout invalidation, UAX #14 line breaking, and text synchronization/API
changes. The source baseline also advances Taffy from 0.10 to 0.15 and selects Rust 1.98.1. These
are migration inputs, not a reason to copy the upstream manifest or silently change Open GPUI's
declared Rust 1.92 floor. Validate the actual compiler/dependency requirement per owning change.
Completing a few correctness ports does not qualify the complete upstream refresh.

### Text And Layout Reconciliation Units

The fixed-target text audit maps the following complete units to fork `229723e620`. These source
findings add no runtime qualification. Preserve `WindowTextSystem::abort_frame`, which returns
layouts moved by a refused candidate, and the fork's exact clip authority throughout the changes.

| Unit | Source-confirmed behavior and prerequisites | Owning acceptance |
| --- | --- | --- |
| Font resolution, installation and reporting | `9e6e14160c`, `3389d87577`, `b7d50b95e8`, `b68add5b37`: cache Direct/Fallback/Missing resolution, invalidate resolution and layout generations after successful installation, retain only direct reverse lookup, and preserve loaded Cosmic FontId identity. Missing-glyph reporting includes bounded delivery, generation deduplication and subscription replacement/release across core/Cosmic/Web. Target Windows/CoreText still use the default empty sink. | Installation, explicit redraw, fallback-to-direct change, stale `LineLayoutIndex`, rejected candidate recovery and released observer tests. Generation invalidation alone does not arrange redraw; do not claim all-platform missing-glyph reporting. |
| Cosmic shaping and fallback | `c214057e08` handles `Bidi_Class=B` separators with FontRun slicing, glyph-byte/x rebasing and merged metrics. `d6358b2d3c` retains non-Latin fallback faces that cannot serve as a Latin primary. | Mixed separators/directions, source byte mapping and fallback-only fonts. Add the direct `unicode-bidi` dependency; existing Cosmic user fallback chains already exist locally. |
| Font work reuse | `f1d27d545e`, `5b4a2153a0`, `d0f797a38c`, `1ff7cb669b`: font prewarming hook, truthful available-family names, shared embedded-font bytes and glyph raster reuse after bounds measurement. | Count actual font resolution/raster/byte ownership work with generation and cleanup controls. Existing family caching and the independent CharWidthCache pool are not new benefits. |
| Wrapping, indentation and truncation | `f0210f1621` supersedes earlier punctuation patches with UAX #14 segmentation across fragments, inline elements and shaped clusters. `4f70d91bda` adds `IndentAdjustment` and the third `wrap_line` parameter. Earlier `03a8544040` Middle truncation remains missing locally. | Preserve exact text/run redistribution, grapheme and shaped-cluster boundaries, RTL workaround behavior and wrapped truncation. Target uses a non-complex-scripts segmenter; this does not establish complete complex-script or UAX #9 behavior. |
| Text split, layered paint and underlines | `2936989f1b`, `f792c2d72d` add raw LineLayout split/paint and ShapedLineCursor; ShapedLine split is already present. `bda9c0bd43` separates text background/foreground with correct aligned/wrapped bounds. `0dea8c63d9` adds underline-handler byte/geometry callbacks and exclusion painting. | UTF-8 and monotonic cursor boundaries, visually reordered split fallback, aligned/wrapped layer bounds, opaque run background with inserted highlights and zero-advance underline geometry. Import framework primitives, not Zed's editor diagnostics owner. |
| Window-local layout ownership | `772a8a9100` changes window text state from Arc/locks to Rc/RefCell while retaining the global Arc<TextSystem>; `2a74c8dff6` removes unused hash APIs/caches. | Update all public consumers and borrowed lifetimes, preserve refused-candidate cache recovery, and test nested draws/reentry. No local call site is not evidence that deleting a public method is nonbreaking. |
| Taffy and intrinsic measurement | `91fdd55889`, `b05f40c554`, `31ceaf7908`, `cdf33ac25e`, `b488c4e814`: Taffy 0.15 leaf measurement/alignment APIs, explicit relative position, separate truncation width from intrinsic measurement, shape-width truncation and MaxContent list probes. | Constrained/remeasured text, flex/grid/list widths and independent consumer. A dependency bump without the measurement corrections preserves known regressions. |
| Root/grid sizing and stack policy | `b0da438545` fills auto window/prompt roots without stretching anchored/tooltip/drag roots. `5ccbbbd88f` adds grid-row min/max-content and renames the shared min-size type. `41d72f6c1d` makes stack protection opt-in. | Exact root classes and explicit sizes, grid-row sizing and all affected public style consumers. Decide default stack protection explicitly; apply any gate across both Taffy and Div's three phases. |
| CoreText identity and size | `20a93f6195`, `44ec5691b5`: deduplicate PostScript identity within one family load and preserve the exact first nonempty run size. | Duplicate/missing family identity and empty/BOM leading runs on macOS. These local text corrections do not require completing the Apple renderer split first. |

The target's UAX path adds `icu_segmenter` 2.3.0 with compiled data and promotes
`unicode-segmentation` to a normal dependency. Cosmic adds direct `unicode-bidi` 0.3.18;
Cosmic 0.19.0 and Swash 0.2.6 are already present locally. Available package metadata declares
Rust 1.88 for ICU segmenter, 1.47 for unicode-bidi and 1.71 for Taffy 0.15; those declarations do
not qualify the resolved closure under Rust 1.92. Validate that compiler and the independently
locked consumer rather than adopting upstream's workspace compiler by assumption. The consumer's
separate Taffy 0.10.0 value does not currently cross a GPUI conversion boundary; evaluate its
purpose before mechanically changing it.
