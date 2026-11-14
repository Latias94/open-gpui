# GPUI Maintenance Follow-up — November 14, 2026

## Fixed sources and scope

This bounded follow-up starts at Open-GPUI
`6017e2a3c069656f73294e91192428bb699b6ccd` and checks native lifecycle,
shared docking cleanup, persistence and newly published official fixes. The
preceding [November 13 ledger](zed-gpui-incremental-sync-2026-11-13.md) retains
its original endpoint and executed qualification.

The excluded upstream endpoint and read-only reference checkout are both
`27bbbc1cc480a7500cec122f120e796d1fb8500c`. The separately observed official
remote HEAD is `5bd59e2e439aff9b9203b9dd3c9bcb04159f701c`; this document fixes
that endpoint for its fourteen-row inventory. The reference checkout is complete,
not shallow, and was not fetched or moved. Shared dockspace remains
`c200977d0f34943284a125c492919791650e1bff`.

The official [comparison patch](https://github.com/zed-industries/zed/compare/27bbbc1cc480a7500cec122f120e796d1fb8500c...5bd59e2e439aff9b9203b9dd3c9bcb04159f701c.patch)
contains fourteen commit headers and all changed paths: 282,554 bytes, SHA-256
`ff6346824919c56bb06b9a1292ccccb358bcf86024e40df83cd446e66a3f4778`.
The GitHub API comparison returned HTTP 403 rate-limit exceeded; the official
patch endpoint succeeded without changing either checkout. These fourteen rows
extend the earlier 574-SHA inventory to 588 distinct official commits.

The applicable framework source contributions at this fixed endpoint are now
reconciled, including the SVG timed-transformation contribution below. The
fourteen-row range contains no remaining queued framework commit. Source
completion does not qualify every platform or reproduce upstream performance
measurements; the executed and outstanding gates remain separate below.

## Per-commit inventory

| Full official SHA | Official contribution | Disposition and owner |
| --- | --- | --- |
| `42fa909546bf85d92f3ddeac0690e4ca69776cc7` | corgi: Support building on NixOS (#66827) | Outside the extracted framework: Zed's `corgi.toml` build configuration. |
| `48741fd17d0dec28be6ad664b0a06dc7202086be` | which_key: Show popup above open modals (#66838) | Outside the extracted framework: which-key and Workspace modal-layer composition; no GPUI platform or portal implementation changes. |
| `5a40bc8bd78da92ea3b1ed143dd1208a0e4a3567` | gpui_web: Fix safe-area insets under a strict style CSP (#66907) | M-CSP: adopt the complete per-property CSSOM probe configuration in `gpui_web::viewport`; retain existing geometry and removal ownership. |
| `4fd48c02acb46500b832a3a90f89ceea4735f9f8` | Miscellaneous improvements to prepare for Rust 1.100 (#66901) | M-RUST: move the unused fallback cache key instead of cloning it. Window image removal already consumes its key. Preserve equivalent older-compiler constant/atomic APIs and the host-test dependency configuration described below. Product dependency/version and editor cleanups remain outside the framework. |
| `af7a1adb50a9a37dbb12d3ccfddec1c18bdc6a6e` | Use full context window for Copilot models without long-context surcharge (#64302) | Outside the extracted framework: Copilot provider model limits. |
| `cb48e27cebc9c77f0046e0976076d3d2fa011a08` | Prefer musl builds of ACP registry agents on musl Linux hosts (#66914) | Outside the selected framework closure: ACP/language-server binary download policy. The new `util::libc_type` helper serves those product consumers; GPUI does not select or download those binaries. |
| `faf97b0a0c1249f64b29bbc3f26d4503fa9bddf2` | Preserve structured model refusal details (#66929) | Outside the extracted framework: model providers, agent thread and generated-code UI. |
| `69239eb54067293f1b646a7dd7f83fc4f62af728` | Add an icon-only variant to the toggle button component (#66937) | Outside the extracted framework: Zed's product `ui` component. Open-GPUI's own controls retain their separate public contracts. |
| `563319c3f47e2bf82d132942c6b5f5860b18a1e2` | Sync Neovim default key mappings (#66759) | Outside the extracted framework: Vim product keymap. |
| `aff99ecc3249cfa95e720c4d6c1eb4a72105a3ee` | Animate SVG transformations without re-rendering views (#66896) | M-SVG: adopt the official static/timed SVG APIs, easing, heartbeat, Scene animation table, FPS/reduced-motion policy and ordinary example. Adapt cache replay, candidate admission, exact clips and asynchronous accepted-scene presentation through their existing owners. Migrate Metal/WGSL/HLSL records together; keep the 120-byte subpixel path distinct. Product `ui` call sites remain reference consumers. |
| `83df5df635ed97ca6425d16659ad0307865ed93b` | Add corgi to the Nix dev shell (#66940) | Outside the extracted framework: Zed's Nix/devshell build environment. |
| `dc93abce2fa6c0bf349d425310db4884aa953467` | Keep macOS notification service calls off the UI thread (#66934) | M-NOTIFY: adopt one lazy FIFO notification worker, native-call completion tasks, retained delegate and foreground responses; migrate the public trait, backend implementations, relocated tests/example and independent consumer. |
| `06999050d632c0c6ee224fb3955a7bcda9afc462` | Stream thread saves into zstd instead of buffering JSON (#66683) | Outside the extracted framework: agent database persistence. |
| `5bd59e2e439aff9b9203b9dd3c9bcb04159f701c` | Add GPT-6.1 Terra (#66659) | Outside the extracted framework: OpenAI model-provider definitions. |

M-RUST preserves Core's Rust 1.93 floor and docking's Rust 1.95 floor. The
existing `1.618_034_f32` golden-ratio value and atomic `fetch_update` operations
provide the same behavior without requiring newly stabilized standard-library
names. iOS frame-demand host tests and Windows color-glyph host tests use Core
outside their native target configuration, so their dependency is retained.
The X11 license-comment spacing has no runtime contribution. Neither root
lockfile churn nor product `open` dependency removal selects a new framework
dependency here.

## Independently found lifecycle repairs

| Finding | Provenance and trigger | Correction and qualification boundary |
| --- | --- | --- |
| Global close-listener accumulation | Fork-owned docking mount integration `68d52b00` detached two application-wide subscriptions per mount. Ordinary closure and initial-presentation rollback removed the mount but retained both callback captures. | `03f6652d` stores one joined `Subscription` with the original mount and releases it through existing removal. Keep managed native terminal/capture/shutdown custody in its independent original owners. |
| Close-callback panic crossing the macOS ABI | Inherited direct calls exist in the fixed official `window_should_close` and native close implementation. An application callback or retired callback destructor can panic before the native close tail finishes. | Reuse `MacWindowCallbackPanicBoundary`: a query panic vetoes that close; individual terminal cleanup effects remain isolated so superclass close and the existing owner reach completion. No second lifecycle state machine is introduced. |

The docking regressions first fail on the unfixed implementation, each retaining
two weak callback captures instead of zero: nextest
`ecd4149b-3113-416e-b382-8d6460032c0b`,
`/tmp/open-gpui-listener-lifetime-red-20261114.log`. The corrected default
cohort passes 66 cases and the all-feature cohort passes 1,542 cases, including
the explicit legacy engine. Native close failure injection retains a private
typed expected payload; unexpected assertion panics still fail the process.

Independent close review found two additional pre-existing unwind gaps before
publication: user callback captures were dropped inside window locks before the
new terminal boundary, and a query panic could overlap the retirement panic of
its own replaced callback. The latter is independently reproduced in a CPU-only
test process: nextest `39944d9b-a464-48d0-82fd-805d32af8b2d` terminates its one
selected regression with SIGABRT and `panic in a destructor during cleanup`.
Log: `/tmp/open-gpui-close-checkout-red-20261114.log`. The correction separates
query execution from checkout retirement, preserves the first panic and avoids
dropping an arbitrary secondary payload. Mac callback retirement moves outside
the window lock and shares the existing panic boundary.

Persistence review finds no additional defect in strict envelope/core decoding,
closed-key validation, save-generation admission or one-way legacy migration.
Restoration constructs an unpublished candidate; it is not qualification of a
new live-workspace replacement API.

## Delivery and executed evidence

The preceding correctness code checkpoint is
`eef07bd089871b2d93845591973910debd8791c5`. Every implementation has its own
Conventional Commit. Those receipts retain that source scope; the subsequent
SVG implementation and its qualification are recorded separately below.

| Owner | Full local commit | Source linkage |
| --- | --- | --- |
| Docking mount subscriptions | `03f6652d0dea75f71642c7b4d44714d353962de0` | Fork-only correction to `68d52b00`; repeated close and initial-presentation rollback controls. |
| Close-query checkout | `f87370c1dc916fa813958f0ed1966be806c2cd06` | Fork slot correction; first panic, arbitrary secondary payload and replacement callback remain correctly owned. |
| macOS native close boundary | `a353ce1dcc70e00bd887c1bfd157af8cf67281ad` | Inherited direct-call gap and fork cleanup adaptation; shared lock-free callback retirement and native fault injection. |
| M-CSP | `06d650dff7f048cde039427159b68e2625cd9b44` | Official `5a40bc8bd78da92ea3b1ed143dd1208a0e4a3567`. |
| M-RUST | `7d489567eb69d1b6a6ae1d967833c8d598a193a6` | Applicable cache cleanup from official `4fd48c02acb46500b832a3a90f89ceea4735f9f8`; equivalent floor-compatible operations retained. |
| M-NOTIFY | `eef07bd089871b2d93845591973910debd8791c5` | Official `dc93abce2fa6c0bf349d425310db4884aa953467`; includes public API, backend, relocated consumer and migration updates. |

Cargo runs serially with `CARGO_BUILD_JOBS=1` and the existing root target.
Test execution uses the existing isolated nextest 0.9.151 runner; the shared
installation is unchanged. Independent reviewers cover subscription removal,
official notification/CSP equivalence and final close panic boundaries. The two
additional close findings above are corrected and independently re-reviewed
without remaining P1/P2 findings.

| Gate | Actual result and scope |
| --- | --- |
| Default shared docking | **66/66**, zero skipped; `9dac18f3-1156-447a-9226-1f5f3567c813`. |
| Docking with all features | **1542/1542**, zero skipped; `c0f75166-bb7c-4c26-9c91-81432a83e0ab`. Includes the explicit compatibility engine. |
| Final Core/Apple/macOS libraries | **1904/1904**, zero skipped; `fae34c63-30a0-4996-a36e-010d9262758a`, 54.958 s test execution, `test-support,runtime_shaders`. Includes both notification queue regressions and final query/checkout behavior. The earlier 1903-case run precedes the additional review correction and is not the final receipt. |
| Focused Core query/notification controls | **15/15**, 1709 unrelated cases filtered; `d3e0b888-77ed-4113-b202-8615097218b6`. Includes the formerly aborting combination. |
| Public notification workflow | **2/2**, zero skipped; `2029ac8b-4162-49a5-970c-615229b7db84`. Real CPU TestPlatform App delivery, view notification and handler replacement, not OS notification delivery. |
| Native ordinary close | **Passed**, runner and actual OS exit 0 in 1.532 s; original shutdown, reentrant close, exact terminal and retired backing-layer fallback. |
| Native close panic mode | **Passed**, runner and actual OS exit 0 in 0.918 s; real Objective-C query invocation, replacement plus combined callback/checkout panic, frame/resize retirement, terminal callback/body destruction, exact shutdown and final owner phase. Exact isolated-log counts are query **3**, surface quiescence **2**, terminal close **2**. |
| Native editor | **Passed**, runner and actual OS exit 0 in 1.845 s; two submitted windows, retained Entity and local/shared edits, dependent/anchor shutdown and isolated sibling survival/retirement. |
| Rust 1.93 | Locked Core/macOS/WGPU all-target check with `test-support,runtime_shaders` passes in 49.852 s, including the native example's compilation. |
| Independent Rust 1.95 consumer | **3/3**, zero skipped; `aecf6ab1-ee23-4e80-b60e-ede276b455a4`, using its own locked dependency graph. Notification values and traits execute; the App/`dyn Platform` task-return signature closures compile but are not invoked in this ordinary consumer. The public CPU workflow above supplies separate App execution. |
| Web compilation | Independently locked browser smoke compiles for `wasm32-unknown-unknown` on Rust **1.95** in 8.912 s. This adds CSP source/compilation evidence, not a new browser or mobile safe-area execution claim. |
| Correctness Clippy | Core/macOS/WGPU/platform/default docking all-target Clippy passes with `test-support,runtime_shaders` and both `clippy::correctness` / `clippy::suspicious` denied, in 119.052 s. Existing advisory warnings remain; this is not a warning-free claim. |
| Repository gates | Root and independent-consumer formatting, document links, public API tiers, import boundaries, release/breaking inventory and whitespace checks pass. |

Native executables come from Cargo's JSON artifact paths and run serially through
the existing signed-bundle runner. It requires the success marker and actual OS
normal exit status 0. Each temporary app is removed before the next case starts.
The final inventory contains **zero live probe processes and zero temporary
apps**, and the bounded display-wake assertion process is terminated and reaped.
The native receipt records all seventeen modified source hashes, verifies their
match to the final code checkpoint and keeps its original build-time base.

Logs are `/tmp/open-gpui-maintenance-*-20261114.log`,
`/tmp/open-gpui-maintenance-native-20261114-*.log` and
`/tmp/open-gpui-csp-wasm-check-20261114.log`. Native source, actual exit and cleanup
facts are in `/tmp/open-gpui-maintenance-native-20261114-receipt.json`;
serial compiler/test commands and results are in
`/tmp/open-gpui-maintenance-validation-20261114.json`.

## SVG retained-scene adaptation

The implementation checkpoint is
`b95b84ff5e70570ae672243f605c57fc0af3af82`. Its full official/local mapping is:

| Owner | Full official commit | Full local implementation commit |
| --- | --- | --- |
| M-SVG, including fork journal/submission and renderer adaptations | `aff99ecc3249cfa95e720c4d6c1eb4a72105a3ee` | `b95b84ff5e70570ae672243f605c57fc0af3af82` |

M-SVG restores the official SVG-only raster pose APIs alongside the checked
interactive subtree authority. It does not extend arbitrary element hit testing
to rotation. Scene entries retain original matrices/colors, rebind IDs during
cache replay and publish animation demand only with the accepted candidate.
Animation-only FPS limits leave dirty frames, callbacks, input and required
submissions under the existing scheduling rules. Reduced motion accepts a static
initial pose and removes animation demand.

The first real Mac attempt exposed an integration gap: native delivery is
asynchronous, and the incumbent retained-present fallback rebuilt the view on
every frame (59 to 73 renders in 220 ms). Accepted animated scenes now receive
an independently leased, immutable presentation snapshot and a fresh attempt
under the existing submission authority. Only its actual Submitted completion
publishes the raster sample, preserving the semantic frame generation. A new
candidate, shutdown, stale geometry, invalid scope, font change or failed lease
cannot publish the old sample. This path adds no second semantic acceptance
transaction.

Independent review also reproduces a journal rebuild losing an already delivered
pose when an invalid sibling is removed. The formerly failing deferred regression
now passes; rebuilding preserves surviving raster samples while keeping journal
base values unchanged. Renderer review finds that the old WGPU grayscale fallback
would decode 120-byte subpixel instances as the new 144-byte monochrome record.
Storage and packed-texture fallback now use the dedicated subpixel vertex entry;
the monochrome fragment retains its existing grayscale behavior. DirectX has the
same separate subpixel vertex/record boundary.

| SVG gate | Actual result and scope |
| --- | --- |
| Final Core/Apple/macOS/WGPU libraries | **2269/2269**, zero skipped; nextest `b3702b22-3d7d-4de1-879c-dd72fb8660e4`, 66.315 s execution, `test-support,runtime_shaders,profiler`. Includes nine accepted/deferred SVG window regressions, exact clips, lifecycle and real GPU readbacks. |
| Public presentation surface | **4/4**, zero skipped; nextest `1e6bdcd2-3ac9-4323-b7cf-929096b3f4ab`. SVG poses compose with the checked subtree API; the new compile-fail case rejects poses on arbitrary `Div`. |
| Independent Rust 1.95 consumer | **2/2**, zero skipped; nextest `e3e6f0aa-9288-4a98-93db-a77f97c29268`, its own locked graph and no optional features. Public API composition and heartbeat execute without starting an App. |
| Rotated masks and subpixel grayscale fallback | **2/2** focused actual-GPU controls; nextest `d548bd85-b25b-4923-871e-414bb2007a82`. Both storage and packed-texture transports preserve UVs, parent projection, exact clips and record strides; the final library cohort above includes both tests. |
| Offline Metal and native SVG example | Offline Xcode Metal build passes in 22.974 s. Signed-bundle `svg_animation --smoke` passes in 2.918 s with actual OS exit 0: all 200 animations remain live, pixels advance over 220 ms, and both render count and accepted generation remain unchanged. Final inventory: zero live probe processes and zero temporary apps. |
| Compiler and wasm owners | Final Rust **1.93.0** locked Core/macOS/WGPU/platform all-target check passes in **33.57 s**. Web wasm `--tests` passes in **1.83 s** and the production WGPU wasm library passes in **18.30 s**. An exploratory WGPU wasm test build retains its existing Criterion/Rayon restriction; it is not a supported passing gate. |
| Shared iOS compilation | `open-gpui-ios --target aarch64-apple-ios-sim --all-features --lib` passes in **84.214 s** with deployment floor 15.0. Shared Apple/Core source is compiled; no simulator or device launch is claimed. |
| Windows owning-target compilation | Real MSVC SDK/LLVM `cargo xwin check` passes Windows/WGPU/platform all targets with `test-support` in **91.215 s** for `x86_64-pc-windows-msvc`. This cross-check does not run FXC, DirectX, native windows or GPU output. |
| Correctness Clippy | Core/Apple/macOS/WGPU/platform/xtask all-target check passes `-D clippy::correctness -D clippy::suspicious` in **56.52 s**. Existing advisory warnings remain. |
| Complete real browser workflow | Chromium **151.0.7922.34**, final source, **exit 0 in 236.660 s** without unavailable skips. Explicit WebGL, Auto/WebGPU, explicit WebGPU and forced-no-WebGPU Auto/GL each pass **22/22** PNG samples at both viewport sizes. Platform identity, trusted clipboard/text, fonts, touch and deterministic backend refusal/cleanup controls all pass in the same invocation. |
| Repository gates | Root/consumer formatting, document links, public API tiers, import boundaries, release/breaking inventory and whitespace checks pass. The first release-doc check catches a missing literal ABI symbol in the changelog; the release-facing migration entry is added before the final pass. |

The first library run's tiny-atlas fixture rejected the SVG supersampling extent;
its corrected atlas retains ordinary raster bounds. The later grayscale regression
initially compared masks with different filtering neighborhoods. Both now use the
same transparent border, preserving the original whole-image zero-difference
oracle; neither production coverage nor pixel tolerance is weakened. These
fixture failures are separate from the reproduced deferred/journal defects.

Native source hashes, build-time base, Cargo-reported executable and actual exit
are retained in `/tmp/open-gpui-svg-native-final-receipt-20261114.json`. Logs are
`/tmp/open-gpui-svg-*-20261114.log`. The existing runner removes each signed app
before another launches. No upstream speedup ratio or renderer submission count
is inferred from view counters.
The native pixel oracle uses GPU readback of the accepted scene through
`Window::render_to_image`. That method does not advance animation time; the
changing sample is published by the original native Submitted completion. This
is not a screen recording or scanout measurement.

The first browser invocation selects the host's default Edge and exits before
graphics preflight on a CDP handshake timeout. Its log remains
`/tmp/open-gpui-svg-browser-edge-handshake-failed-20261114.log`. The complete
passing invocation explicitly selects the already qualified Chromium above and
the task-local matching wasm-bindgen **0.2.130** CLI. The shared CLI is unchanged.
Browser output and execution metadata are retained in
`/tmp/open-gpui-svg-final-browser-20261114.log` and its companion JSON.

## CI dependency and browser repairs

| Full local commit | CI contribution and source boundary |
| --- | --- |
| `f483eed100cd8535ec8560972cfa25c31afe0d7d` | Vulnerable QUIC updates, explicit package versions for retained Git pins and target-correct wasm test support. |
| `813139bb3ed01c4d19d047e62d9b783cc6052efd` | Reject lossy Git/patch dependency substitution before any registry upload. |
| `299a91dc2461535a826d7d80624be22e3c19db14` | Initialize direct SwiftShader Vulkan without ANGLE Vulkan reimport. |
| `df39f150aa2cec0a4a02ba2fd7e135e315d5e0dd` | Deterministic WebGL initialization refusal, exact candidate cleanup and independent test controls. |

`f483eed1` updates the vulnerable QUIC packages, supplies required package versions
alongside development Git pins and restores target-correct wasm test support.
Dependency Health and Publish Check both pass for `299a91dc`; the local audit
reports zero vulnerabilities and all 39 packageability checks pass. These are
build/packaging receipts, not authorization or qualification of registry uploads.
`813139bb` rejects publishable Git dependencies and root-only patches before the
first upload, because Cargo packaging can otherwise replace required fork fixes
with registry code. The [dependency audit](zed-fork-dependency-audit.md) owns those
unresolved registry successors.

The Linux browser initially crashes its GPU process before preflight. `299a91dc`
keeps direct SwiftShader Vulkan without importing Vulkan from ANGLE; the real
Linux rerun then passes WebGL, Auto/WebGPU and explicit WebGPU at both pixel
sizes, plus platform/input/touch checks. Its later failure exposes an obsolete
test assumption: requesting a WebGL software adapter succeeds on that browser.
`df39f150` replaces that assumption with a deterministic unavailable-context
control, requiring the exact backend error, no launched App and removed canvas
candidate. The control is removed before WebGPU failure/Auto fallback checks.
Its fourteen existing host tests and independent read-only review pass. The
complete local Chromium invocation above executes the new control and final SVG
source together; remote Verify status remains separate from that host receipt.

## Remaining qualification and next work

The fixed fourteen-row source inventory is complete. Continue qualifying the
changed renderer and native paths on their owning platforms; retain exact clips,
atlas residency and original submission authority when adding future consumers.

Physical capture/drag cancellation, stationary-pointer/native hit-stack
arbitration, mixed-DPI movement and actual foreign-platform shutdown retain
their existing open gates. The ordinary Mac editor smoke and native callback
injection do not supply those input or platform receipts. Notification command
tests likewise do not claim live authorization, OS delivery or an induced
notification-service stall. A stalled service can still delay subsequent
notification commands while the UI remains independent.

Windows and Linux notification implementations and native test call sites are
source-migrated to `Task<()>`. The subsequent Windows target cross-check above
compiles its backend and call sites; Linux target compilation still needs its
own result. Native notification/retirement execution remains open on both
platforms. The Mac checks do not grant that execution credit or repeat iOS native
qualification. Web's real wasm compilation covers its inherited default
notification method.
