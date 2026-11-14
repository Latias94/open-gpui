# CI Suite Consolidation — November 14, 2026

## Reproduced Failure

The starting framework revision is `2890cd30b7737b9d3316c6c99c172044ecacae44`.
In [Verify run 43557556065](https://github.com/Latias94/open-gpui/actions/runs/43557556065),
all three stable-platform jobs exhaust their 45-minute job limit. The Windows
check annotation explicitly reports `The job has exceeded the maximum execution
time of 45m0s`. Each platform reports a cache miss.

Windows passes production FXC compilation, then times out inside the nested
`xtask verify` before the targeted native animation gate. Linux passes the full
browser smoke and reaches independent windowing compilation. macOS reaches Apple
activity compilation. These are incomplete jobs, not complete verification
passes or evidence that the unexecuted native tests failed. Both Windows MSRV
jobs, Dependency Health and Publish Check pass at this exact revision.

## CI Ownership

The stable workflow replaces per-fix filter accumulation and the nested local
rollup with ordinary package suites. `cargo run -p xtask -- verify` remains a local
convenience command; CI directly names its owning package and repository gates.
No new runner script, source parser or generated workflow is introduced.

| Suite | Scope |
| --- | --- |
| Windows compiler floors | Keep the independently locked Rust 1.93 framework and Rust 1.95 docking/tooling checks. |
| Workspace and libraries | Formatting, default workspace targets, default Core, default and full component/DevTools/docking libraries, doctests, repository contracts and release embedded assets. |
| Core/platform matrix | Linux/macOS/Windows complete Core and utility tests; separate instrumentation-free and conditional hot-reload builds; host Web/font fallback, component platform/motion consumers and owning backend checks. |
| Independent consumer matrix | Separate manifest/lockfile, minimal API, benchmark support without framework test support, complete features, logging without Tracy and additive support. Windows additionally runs the existing benchmark smoke workloads. |
| Windows native | Production FXC and retained-SVG native tests run first. Complete ordinary backend tests, the explicit desktop-only cohort and all three handle-tracking environment controls remain separate. |
| Windows GPU hosts | Complete WGPU tests, the application-supplied renderer and all seven ordinary/external/deferred/logical-root host trajectories. |
| Browser | Stable wasm production/test compilation and the complete required browser smoke, independently of native package builds. |

The reviewed rewrite reduces the workflow from 710 to 393 lines, its step
definitions from 134 to 77, and literal nextest invocations from 135 to 38.
These are configuration counts, not measured speedups. Full suites intentionally
discover new tests without adding a workflow row for each regression. Shared
feature selection must still preserve a distinct default docking build and the
instrumentation-free Core/consumer builds.

Native/package jobs retain one Cargo build worker and one test thread. Their
90-minute limits allow cold dependency compilation; browser and compiler-floor
jobs retain their existing 45-minute limits. Different hosted jobs have their own
ordinary target directory and cache. The local development checkout continues
to serialize Cargo and reuse its original target directory.

Dependency audit and packaging retain their existing workflows. Registry source
qualification and the dedicated physical-input workflow keep their separate
contracts. A hosted DirectX result does not establish the physical-input gate.

## Qualification

`actionlint .github/workflows/verify.yml` and the existing document-link, API,
import, release, theme and UI contract checks pass. The exact consolidated
Core/utility command passes **2587/2587**, zero skipped, on the Mac host in
373.789 seconds including compilation; nextest execution is 136.138 seconds,
run `6c8c9baa-28f8-4492-9230-392103b55e5c`. Its log is
`/tmp/open-gpui-ci-core-batch-20261114.log`.

The independently locked `bench-support,profiler` complete test cohort passes
**155/155**, zero skipped, in 175.143 seconds including compilation; execution
is 1.783 seconds. The log is
`/tmp/open-gpui-ci-independent-bench-batch-20261114.log`. Both commands use one
build worker, one test thread and the original target directory.

The instrumentation-free Core command additionally passes **1917/1917**, zero
skipped, in 215.103 seconds including compilation (51.270 seconds execution).
The Apple/macOS `font-kit,screen-capture` cohort passes **199/199**, zero skipped,
in 43.467 seconds including compilation (3.834 seconds execution), retaining the
offline Metal library. Logs are
`/tmp/open-gpui-ci-{core-minimal,apple-offline}-batch-20261114.log`.

Independent review identifies and corrects three consolidation errors before
commit: the fixture's profiler flag must match bench-support's enabled Core
implementation; profiler/encoder/debug-embed/logging retain minimal feature
checks; and macOS backend execution retains the embedded Metal library instead
of silently switching to runtime shaders. The real independent GPU-atlas test
belongs to the Windows GPU job, not the three-host CPU consumer matrix. Final
review reports no remaining P1/P2. Owning-platform CI execution remains required
before claiming that the new full suites pass remotely.

### Independent Loopback Follow-up

At workflow checkpoint `04acc73022a5f4f78b551f0f52ca6b8219383b7f`, the complete
minimal consumer passes **120/120**, zero skipped, in 91.723 seconds including
compilation. The complete-feature consumer initially passes 158/159: its HTTP
redirect fixture receives `WouldBlock` while reading request headers, then
resets the connection. The nonblocking listener's accepted stream can retain
nonblocking mode on macOS; setting a read timeout does not change that mode.
The production Reqwest implementation is unchanged, and its existing unit-test
loopback helper already resets accepted streams correctly.

The independent fixture now likewise calls `set_nonblocking(false)` before
blocking reads. The original redirect/header assertions remain intact. A
20-iteration targeted stress run reproduces one failure before the fix and
passes all 20 afterward. The final complete-feature consumer passes **159/159**,
zero skipped, in 2.809 seconds including the incremental build; test execution
is 1.903 seconds. Logs are
`/tmp/open-gpui-ci-http-{before,after}-fix-20261114.log` and
`/tmp/open-gpui-ci-independent-complete-after-fix-20261114.log`.

The exact timeout job logs are retained as
`/tmp/open-gpui-ci-timeout-{windows,macos,linux}-20261114.log`; the GitHub job and
annotation API snapshots use `/tmp/open-gpui-continuation-*-20261114.json`.

### Consolidated-suite Regressions

At `10254c62684d1f7f69dcbb4bcb1735f2fa768415`,
[Verify run 43564500440](https://github.com/Latias94/open-gpui/actions/runs/43564500440)
reaches both previously unexecuted Windows gates. Production FXC passes, followed
by the generic native animation scenario in 5.674 seconds. The retained-SVG
scenario fails at its fixed render-count assertion: two renders instead of one.
Its log is `/tmp/open-gpui-ci-native-failure-20261114.log`. This result does not
qualify the rest of that job's unreached native and backend suites.

Core initially draws before showing the native window; resize and completion
callbacks can legitimately request another refresh. The fixture's first visible
Submitted therefore need not be a stable animation-only baseline. The correction
waits for an exact-registration post-show `WM_PAINT` to return, requests one
initialization refresh and starts measurements at its newer Submitted generation.
The existing fixed render-count and generation assertions, reduced-motion phases
and pending-frame retirement checks remain unchanged. No production draw policy
changes. The original failure has no message trace identifying which callback
requested that extra render; owning Windows execution must confirm the correction.
The correction `d4648a1e` passes real-SDK `cargo xwin check --cross-compiler
clang-cl --locked --target x86_64-pc-windows-msvc -p open-gpui-windows
--all-targets --all-features` in 163.099 seconds, with one build worker. Independent
review finds no blocking issue. This is compiler qualification, not Windows
execution or FXC qualification. Log:
`/tmp/open-gpui-ci-native-svg-msvc-after-fix-20261114.log`.

The complete Windows WGPU cohort passes **148/149**. Its grayscale-fallback
comparison reports 147 differing pixels, all at most one channel level apart,
while comparing an R8 mask against an independently allocated color-format mask.
The shader uses linear filtering across the crop's transparent border. This
supports isolating the comparison's sampling conditions; it does not establish
a production driver or shader defect. The fixture now keeps the R8 prelude and
uses the same color tile for the reference monochrome records and the fallback
subpixel records. Transparent borders, both independent 144/120-byte record
layouts, both transports, five-texel-wide cross-row decoding, one/three instances,
fixed pixel expectations and zero-tolerance full-image comparison remain intact.
Independent source review finds no blocking issue. The failed Windows log is
`/tmp/open-gpui-ci-gpu-failure-20261114.log`; new Windows execution remains required.
The corrected complete Mac WGPU suite passes **148/148**, zero skipped, in 155.481
seconds including compilation (15.286 seconds execution), run
`24baa56d-71f5-4948-a2d7-d5ed55974c9f`. Its log is
`/tmp/open-gpui-ci-wgpu-after-fix-20261114.log`; the fixture correction is
`03261233`.

The local combined library command first stops during linking because the disk
is full. No assertion result follows from that attempt. After the owner frees
space, the same command builds successfully and executes 3,639 tests in 894.629
seconds including compilation, with 478.320 seconds of test execution: **3,638
pass, one fails, two are skipped** (run
`e70348fd-3cf1-4657-8347-a9d9e7970d36`). It exposes an outdated Gallery assertion
left behind by `f2407cda`: tree accessibility set positions now count siblings,
including disabled nodes, whereas keyboard focus
continues to skip disabled nodes. The Gallery assertion now checks both that
distinction and the disabled node's position 2 of 3. Production tree behavior is
unchanged. The original and resumed logs are
`/tmp/open-gpui-ci-full-libraries-{batch,resumed}-20261114.log`.
At `d5995867`, all five Gallery catalog contracts pass with the original combined
package/feature graph, in 71.267 seconds including compilation (1.015 seconds
execution), run `de83cc9c-5801-4d08-9558-1a46799d1c9f`. The targeted filter excludes
3,634 other cases and retains the two pre-existing ignored cases; this is not a
second complete-library pass. The log is
`/tmp/open-gpui-ci-gallery-after-fix-20261114.log`.

The remote Windows default-library suite independently reproduces the Gallery
failure and one additional lifecycle-fixture error: after closing the only window,
the repeated-mount listener test tries to open another window in an application
already shut down by the non-Mac default `QuitMode::LastWindowClosed`. Its
multi-cycle fixture must explicitly select `QuitMode::Explicit`, as the existing
cross-platform close tests do. That setup preserves every mount, listener weak
count and error assertion. The original Windows default-library result is
**1,631/1,633**, one ignored, after the default Core passes **1,780/1,780**. Log:
`/tmp/open-gpui-ci-libraries-windows-failure-20261114.log`.
The correction `40ff32b6` passes the complete local default docking library
**66/66**, zero skipped, in 122.278 seconds including compilation (1.006 seconds
execution), run `b24561c9-7d69-4069-9ef5-e7aa913d8a09`. Log:
`/tmp/open-gpui-ci-docking-quit-after-fix-20261114.log`.

The same remote revision passes both Windows compiler-floor jobs and the complete
Linux consumer and browser jobs. Linux consumer cohorts pass **117/117** minimal,
**151/151** benchmark API, **154/154** complete API, **1/1** logging and **7/7**
dedicated support, all with zero skipped. Browser smoke passes real WebGL and
WebGPU pixels, resize, input, clipboard, text and font fallback, plus its declared
unavailable-context and Auto-fallback controls. Logs are
`/tmp/open-gpui-ci-{consumers-linux,browser}-20261114.log`.

The complete Linux platform job and Mac consumer job subsequently pass at the
same revision. Linux full Core/utility passes **2,567/2,567**, instrumentation-free
Core **1,932/1,932**, and its backend cohort **196/196** with six pre-existing
ignored cases. Mac independent cohorts pass **120/120**, **155/155**, **159/159**,
**1/1** and **7/7**, all with zero skipped, independently qualifying the earlier
loopback-fixture correction. Logs are
`/tmp/open-gpui-ci-{core-linux,consumers-macos}-20261114.log`. Other jobs remain
pending qualification at this checkpoint; a successful step is not a complete
Verify pass.

### Implementation Checkpoints

| Fork commit | Correction | Executed qualification before publication |
| --- | --- | --- |
| `04acc730` | Split complete suites by package, feature contract and native host. | Workflow lint, repository contracts and local Core/consumer cohorts. |
| `10254c62` | Reset accepted loopback sockets to blocking mode. | Red/green stress, complete local and remote Mac/Linux consumers. |
| `03261233` | Compare fallback record layouts against one sampled atlas tile. | Complete Mac WGPU **148/148**; Windows GPU rerun remains required. |
| `d5995867` | Preserve disabled-tree sibling metadata in the Gallery contract. | Reproduced failure, then **5/5** related contracts with the same package graph. |
| `d4648a1e` | Establish a post-show initialization frame before SVG measurements. | Real MSVC SDK all-target/all-feature compilation; native rerun remains required. |
| `40ff32b6` | Keep the repeated-close test application alive across cycles. | Complete default docking library **66/66**; owning Windows rerun remains required. |

## Consumer Follow-up

Read-only inspection finds Nara at `8666a83371f4511cdb0943c47459966997b5bfbf`,
with its independent GPUI fixture still pinned to `68d52b009dde3c784b01268208cb1d0ed690e99c`.
Its full Dialogue/native View trajectory still requires Windows; Mac Session
Open remains unsupported by NaraFS's declared identity guarantees.

The sibling dockspace checkout contains release-retirement correction
`5ff5c2c2df0bcbe823771eef985e8d0eed1ec66a`. It adopts the already-issued exact
preview outputs when pointer release starts awaiting presentation, allowing all
original outputs becoming Dropped to cancel the pending release. Open-GPUI's
pin `c200977d0f34943284a125c492919791650e1bff` lacks this correction.

At 13:39 UTC+8 on November 14, the GitHub commit API reports `No commit found`
for the correction, and remote dockspace `main` is
`475e1e36b64f4a6ee33073797f013a10aa7574f9`. Updating the pin now would make CI
depend on unavailable objects. The local pin-to-correction range contains 23
commits and changes 38 core files. Qualify that explicit range and add the adapter
regression after the source is published: emit an exact child-window preview,
release before Submitted, drop all original preview outputs without emitting
replacement previews, preserve the source child/View, then allow another drag.
This is a source gap with no new native reproduction receipt. No sibling files
or unpublished objects are changed or pushed by this CI task.

### Independent Mac Workbench Close

At Open-GPUI `10254c62684d1f7f69dcbb4bcb1735f2fa768415` and sibling dockspace
`3622f0369bcae96a8ecff5210c6550f04133b98c`, the independently locked
`integration/gpui-harness` rebuilds in 69.756 seconds with one build worker and
the existing Open-GPUI target. Compiler JSON identifies the actual executable;
the receipt records its SHA-256 and both source heads, unchanged after build.

One `gpui-workbench --smoke-close` launch through the existing bounded signed-App
helper completes in **2.421 seconds**, with actual OS wait status **0**. The
visible workbench accepts exact Submitted/core artifacts, keeps retained button
and tab entities, moves the splitter, completes 24 continuous floating-window
motion samples and closes the native window normally. The final inventory has
zero workbench processes and the temporary App is removed. Logs and receipt are
`/tmp/open-gpui-independent-workbench-{build,native}-20261114.log` and
`/tmp/open-gpui-independent-workbench-receipt-20261114.json`.

The smoke dispatches framework events inside one native window; it does not
qualify physical mouse capture or native multiwindow dragging. It uses sibling
dockspace directly and does not establish that Open-GPUI's older Git dependency
pin contains the pending release-retirement correction. No sibling source or
lockfile is modified.
