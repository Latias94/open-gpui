# Zed GPUI Import

**Date**: 2026-06-06
**Source**: `repo-ref/zed`
**Decision**: ADR 0001

## Imported Crates

The first import copied the Apache-2.0 GPUI framework closure from Zed into this workspace:

- `collections`
- `gpui`
- `gpui_linux`
- `gpui_macos`
- `gpui_macros`
- `gpui_platform`
- `gpui_shared_string`
- `gpui_util`
- `gpui_web`
- `gpui_wgpu`
- `gpui_windows`
- `http_client`
- `http_client_tls`
- `media`
- `refineable`
- `refineable/derive_refineable`
- `reqwest_client`
- `scheduler`
- `sum_tree`
- `util`
- `util_macros`

## Import Adjustments

- Rebuilt the root Cargo workspace dependency table instead of copying Zed's full workspace
  manifest. This keeps Zed editor application crates out of the dependency graph.
- Replaced `sum_tree`'s `ztracing::instrument` import with `tracing::instrument`.
- Removed `sum_tree`'s test dependency on `zlog`.
- Removed `util_macros`' dependency on Zed's `perf` tooling crate and kept the `perf` attribute
  self-contained behind the existing `perf-enabled` feature.
- Declared `cfg(rust_analyzer)` as an expected workspace cfg because GPUI uses it in debug
  inspector-related code.

## November 12 Analytic Path Dependencies

The [rendering successor](zed-gpui-upstream-sync-candidates.md#november-12-rendering-successor)
selects Zed `6ceb6fee8738274589b59139df27efc191ec2231`, including the complete path
mechanism introduced at `ffe0fa29e41ee16c948ccbc312edd304b9bf7a3f`. Core replaces the
`lyon` facade/tessellator dependency with crates.io `lyon_path = "1.0.19"` and
`tiny-skia-path = "0.11"`; development examples use `lyon_extra = "1.1.0"`. Existing locks
already contain the selected path releases, so this migration removes unused tessellation
packages rather than upgrading the closure. The upstream GPUI sources retain their Apache-2.0
license, and the winding shader's Eric Lengyel attribution and MIT source notice are preserved.
Zed product changes remain excluded. Numerical performance and foreign-platform execution
require the separate [qualification](../verification.md#november-12-rendering-successor).

## Known Follow-Ups

- Continue monitoring Open GPUI-owned fork dependencies: `font-kit` and `open-gpui-scap`.
- Clean up Zed product naming in comments, examples, window class names, and README content.
- Decide whether platform backend crates should be published as-is or renamed under the Open GPUI
  package strategy.
- Import or fork `gpui-component` only after a native Open GPUI example builds and runs from this
  workspace.

## November 5 Provenance Reconstruction

The initial import commit is `c9571c486612f1bd9194faa028709e89e3f58b7e`; it did not record the
exact Zed checkout SHA. A content comparison found Zed
`6e9465a4288c332208643892e23b9d35d7be5c79` reproduces the imported closure apart from the five
documented tracing/perf adaptations and license-file mode changes. Its parent has identical
contents throughout that closure, so the original monorepo SHA remains unknown. Use this as a
content-verified reconstruction anchor rather than an asserted source-history parent. The
[current sync queue](zed-gpui-upstream-sync-candidates.md#fixed-revisions-and-provenance) owns the
complete evidence, later comparison pins and fixed maintenance target.

## November 11 Mac AppKit And Capture Binding Successor

The fixed target replaces handwritten Cocoa shims with `objc2` across the Mac input, AppKit
and display owners. Official `4b47ceb9d328035bde5ef99009e674d9fee07fb7` replaces the initially
imported Media crate and its bindgen path with the published ScreenCaptureKit and CoreMedia
0.3.2 bindings. The native capture producer emits the same retained CoreVideo buffer already
used by Core and the Apple surface renderer. These dependencies are selected only by Mac
`screen-capture`; other backends retain their existing capture paths. The historical import
set above records Media's original provenance, not a current workspace member. Unrelated
legacy transitive bindings in other platform dependencies are outside this replacement.
ScreenCaptureKit requires macOS 12.3. Capture-enabled application binaries that support earlier
macOS 12 releases weak-link the framework, as Zed does in its final application build script.
The [ordinary consumer](../../tests/fixtures/dependency-consumer/README.md) records that link
configuration and the retained buffer interface separately from native runtime qualification.

## November 8 Benchmark Measurement Dependency

The complete benchmark successor additionally imports `crates/bench_metrics` from fixed Zed
`635026fabc38440fce5e1767d125026a17b12012`, source blob
`1ec628b0e943e34ca6c0f87e5def8f86a63e967f`. Its manifest explicitly licenses this independent
Criterion measurement package under Apache-2.0; its license is retained as a regular file rather
than the monorepo's license symlink. The extracted package is `open-gpui-bench-metrics`, with the
workspace alias `open_gpui_bench_metrics`. It depends on Criterion, anyhow, Unix libc and Linux
`perf-event2`, without any GPUI or Zed product dependency.

Core consumes it only through `bench-support`; ordinary test support remains independent. The
official Criterion 0.8.2 API is retained, including all throughput variants, primary measurement
selection and secondary reports. This import does not add the retired GPL `perf` tooling crate.
Executed behavior and platform limits belong to the existing verification record.

## November 10 Windowing Event-Loop Prerequisite

The fixed target selects `zed-industries/calloop` at
`3759371fee14c40066d64777e2a36b6ffcc22590` through a crates.io patch. Its idle budget,
reentrant-idle handling and zero-timeout continuation are required for the complete Linux
windowing successor's apply-between-dispatch behavior. Registry 0.14.4/0.14.5 are not equivalent
to those two official changes; wake persistence and source removal are already present in
registry. The Git pin is not a superset of later registry channel/timer corrections.

The root and independently maintained platform-consumer/browser-example roots explicitly
select the same source, including calloop-wayland-source's calloop types. A dependency library
cannot export its root patch to downstream Cargo workspaces; ordinary native consumers must
declare that source selection themselves, as the independent fixture documents. This is an
upstream dependency prerequisite, not a second scheduler or an engine-specific loop owner.
Actual runtime scheduling, native switching and remaining source differences retain the
scope recorded in the existing plan and verification guide.

## November 10 Unified Darwin Process Reaper

The Darwin command owner follows fixed-target `e1bfcf85db`, `60ed56b372` and `d4cc8d2409`,
including owned stdio cleanup, Async<File> pipes and adopted PIDs. It requires official
`zed-industries/async-process` revision `0b6d6713570af61806e1e5cb40e0f757cb93fd9d` with
`Child::adopt_raw_pid`; the registry package is not an equivalent implementation. The workspace
and five independent application roots select that revision through `[patch.crates-io]`, so
the direct Darwin dependency and `smol` share one process/reaper type. Cargo does not inherit
this root patch from a library dependency; downstream roots must declare it explicitly.

The [support-primitives qualification](../verification.md#process-reaping-dedicated-execution-and-shared-strings)
records the actual resolved graph, target compilation and retained Windows process-tree owner.
It does not claim Darwin process execution or a second engine process service.

## November 10 HTTP And Release Download Dependencies

The complete HTTP/release-download successor selects the fixed Zed target's `zed-reqwest`
revision `33bc764aa15ff7b200bf7c93bd96e24878d53e14`, declared MIT OR Apache-2.0 in its manifest
with both license texts present. The exact source exposes request-local redirect policies while
retaining the caller's original client configuration. The previous registry adaptation rebuilt
clients for redirects, requiring a parallel configuration/cache and still failing to preserve
arbitrary imported-client settings. That historical source disposition is superseded under
ADR 0001; remove the cache rather than maintain the difference for its own sake. Reqwest is an
ordinary dependency of the native HTTP implementation, not a new engine transport owner.

The separate Apache-2.0 `github_releases` owner is imported as `open-gpui-github-releases` from
fixed target `635026fabc38440fce5e1767d125026a17b12012`. It consumes Core's BackgroundExecutor,
HTTP and utilities, leaving the HTTP abstraction independent of those higher-level owners.
The official archive dependency is MIT/Apache-2.0 async-tar
`bd3ad6f89df9a9da7a8535958756d6bf465936a0`; its length-driven PAX records and futures-based
runtime replace the registry parser/Tokio bridge. Platform TLS initialization follows official
rustls-platform-verifier 0.7 and bundled WebPKI roots on initialization failure. Source selection
and license evidence do not establish native TLS, browser execution or engine adoption.

## November 10 Portable Path Package

The path successor imports the complete Apache-2.0 singular `crates/path` package from fixed
target `635026fabc38440fce5e1767d125026a17b12012` as `open-gpui-path`. It preserves PathStyle,
checked normalized relative paths, local absolute paths, component/prefix iteration and native
byte/WTF-8 conversion. The plural Zed product `crates/paths` remains excluded. Source commits
include `f181a2f47b`, `2610332077`, `a84689073d`, `51a3a0e971`, `f929b3f7ce`, `a4a3a968d1`
and `9521840152`; the package retains full Apache text and the component parsers' attribution.

The utility successor consumes the canonical package with serde instead of implementing another
RelPath/RelPathBuf or PathStyle. Home-directory presentation, file matching/ordering, position
parsing and SanitizedPath remain utility responsibilities with their actual native consumers.
The complete fixed-target utility file also carries lenient globs, extension ordering and
UNC/position parsing; earlier public APIs and duplicate relative-path sources are removed.
Native byte decoding forwards to the path package. One `&Path` emptiness spelling uses the
equivalent stable `as_os_str().is_empty()` to preserve the declared compiler floor. Fixed
home-directory tests retain Open-GPUI names while ordinary consumers still see the real home.
Qualification and any remaining platform scope belong to the existing plan/verification guide.

## November 10 Utility Composition And Native Commands

The complete utility successor follows fixed Zed target
`635026fabc38440fce5e1767d125026a17b12012`: shell construction/capture from `ce48461eaa`,
`c578f4d12b`, `7ca96c2a58`, `18f9d7e731`, `b41a8b60a4`, `4c6c4750d3` and complete target
follow-ups; helper-owner moves `a873cf402c`, `c9d1d0ddfe`, `bcf6582ce3`, `d544470d1d`;
JSON replacement/union `d61edb0825`/`b47d8ac455`; development assets `ee6badf4b0`,
`d3e54c97f4`, `2551721adb`; fragment positions `54f2b5df1d`/`1102219f81`; environment names
`1057c2cf3d`; and explicit Unix file-limit setup `d12e456be7`.

Low-level command construction, Windows shell discovery and UTF-8/bounded-sort helpers move
to open-gpui-core-util. Higher utilities forward those canonical implementations; duplicate
definitions are removed. The existing injected shell-discovery test entry point retains the
official strategy, priority, cache and return type. Qualified restart arguments, process
reaping, RAII/hash/log behavior and product-neutral executable helper names remain with their
current consumers. Markdown/redaction and the complete shell builder match the target source;
other differences are extracted dependency names, retained consumer vocabulary and test seams.

The complete official native/wasm module boundaries and canonical path reexports are adopted.
JSON replacement and deduplicating union are separate policies. Development assets retain
official executable-first runtime repository discovery, matching filters and get/iter/trait
composition. Release and debug-embed use the dependency's derive. Macro crate paths name the
extracted open_gpui_util owner; no build-time checkout is baked into development lookup.

Compression is carried by the actual rust-embed owner. An ordinary consumer resolves 8.13 and
exposes a required trait method absent in the root's original 8.11 family. The supported
dependency floor becomes 8.13; the macro reexports its compressed-file/include-flate symbols
and implements the same dynamic compressed(None) behavior as that version's own derive.
This is one bounded dependency adaptation, without version-specific compatibility branches
or direct pins of proc-macro implementation internals. Required compression dependencies keep
their permissive licenses. The actual dependency-health gate finds four advisories in the
initial compression selection. The root and independent locks select patched zstd 0.14.3,
zstd-safe 8.2.0 and zstd-sys 2.1.2 without new advisory ignores; no source fork or replacement
asset mechanism follows. Actual results and execution limits belong in the verification guide.

## November 10 Shared Apple Package

The shared Apple successor adopts the Apache-2.0 `gpui_apple` framework closure as
`open-gpui-apple`, using fixed target `635026fabc38440fce5e1767d125026a17b12012` and extraction
sources `40180d9c40e2d20eb63d388bff920818f2910b53` /
`95d35c6efddfa6dc34f99bb1873620bd74af806a`. It retains the existing license and NOTICE,
Open-GPUI aliases and concrete fork clip/atlas/presentation and font-selection adaptations.
AppKit application/window/input and the CoreVideo display registry remain in macOS; shared
Metal display links, dispatch, rendering, CoreText/OpenType, Keychain and process information
have one Apple owner. Core no longer depends on unused Media/font/shader build packages.
The new generated CoreText binding is registry `objc2-core-text` 0.3.2; existing dependencies
keep their locked versions. Qualification and remaining Apple execution scope belong to the
[shared Apple record](../verification.md#shared-apple-framework-topology).

## November 11 Logging And Tracing

Zed `ac5af8b9e1` relicensed `zlog`, `ztracing` and `ztracing_macro` as Apache-2.0. The source
files and their Apache notices come from fixed `635026fabc38440fce5e1767d125026a17b12012`.
Their standalone package names are `open-gpui-log`, `open-gpui-tracing` and
`open-gpui-tracing-macro`; the GPL `zlog_settings` application package is excluded. The old
import boundary still disallows the unpublished monorepo package names.

SumTree restores the official `instrument` and its conditional test logger. The application
chooses whether to install the standard `log` facade's process logger; the primary environment
variable becomes `OPEN_GPUI_LOG` and retains `RUST_LOG` fallback. Native `tracy` and browser
`web` features select the official tracing behavior without ambient Zed build variables.
The Tracy allocator remains available as a type an application can select. No dependency
installs it on the application's behalf. Neither tracing feature controls GPUI's foreground
profiler. The actual checks are in the [verification record](../verification.md#standalone-logging-and-tracing).
