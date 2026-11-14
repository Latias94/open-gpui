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

This is a correctness follow-up, not completion of the newer source endpoint.
The SVG timed-transformation contribution remains explicitly queued. Its
upstream performance measurements are not measurements of this fork.

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
| `aff99ecc3249cfa95e720c4d6c1eb4a72105a3ee` | Animate SVG transformations without re-rendering views (#66896) | **Queued M-SVG**: applicable GPUI optimization and public API. Adapt Scene animation entries, original transforms, accepted/replayed journal ranges, reduced motion and submission scheduling together; port the complete upstream regressions and ordinary example. Product `ui` call sites are reference consumers. No equivalent implementation or performance pass is claimed. |
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

The resulting code checkpoint is
`eef07bd089871b2d93845591973910debd8791c5`. Every implementation has its own
Conventional Commit; subsequent documentation does not change the qualified
runtime source.

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

## Remaining qualification and next work

Complete M-SVG before claiming source closure at the newer fixed endpoint.
Advance its transformations through the original Scene/candidate owners; do
not mutate accepted source transforms cumulatively or manufacture a submitted
frame. Retained replay, exact clips and atlas residency remain fork contracts.

Physical capture/drag cancellation, stationary-pointer/native hit-stack
arbitration, mixed-DPI movement and actual foreign-platform shutdown retain
their existing open gates. The ordinary Mac editor smoke and native callback
injection do not supply those input or platform receipts. Notification command
tests likewise do not claim live authorization, OS delivery or an induced
notification-service stall. A stalled service can still delay subsequent
notification commands while the UI remains independent.

Windows and Linux notification implementations and native test call sites are
source-migrated to `Task<()>`; their owning-target compilation and native
notification/retirement runs still need those platforms. The Mac checks do not
grant that execution credit or repeat iOS qualification. Web's real wasm
compilation covers its inherited default notification method.
