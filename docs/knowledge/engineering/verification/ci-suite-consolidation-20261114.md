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
