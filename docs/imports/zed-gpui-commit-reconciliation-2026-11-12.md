# Zed GPUI Commit Reconciliation — November 12, 2026

**Evidence cutoff:** 2026-11-13; final local implementation map, native simulator/runtime and static-gate receipts. The exact source checkpoint is `138c6ef22c4ce8bdacad800bd1ae2cebd2cf0c44`; execution claims retain their individual scopes below.

This is the canonical, static per-commit ledger for the selected framework refresh.
Each upstream commit has one row in the ledger, including mixed-owner commits,
root-only product changes, retained source contracts and the final product commit.
The source evidence and verification ownership needed to interpret those rows are
recorded here; no temporary audit directory is required to use this document.

The later [November 13 incremental ledger](zed-gpui-incremental-sync-2026-11-13.md)
reviews the next ten official commits through `27bbbc1cc480a7500cec122f120e796d1fb8500c`.
Its source decisions and newer native/browser receipts supplement this ledger;
the 564 rows below retain their original `6ceb6fee` boundary.

The selected implementation is **complete**, with **zero unexplained source gaps**.
The final affected host cohort passes **3615/3615**, with three existing AppKit
main-thread tests ignored. EGL's formerly open dependency correction is adopted and
its real-binding positive and negative controls are executed. Independent source
re-review closes the List inset findings and all seven prior iOS findings.

Source adoption and platform qualification remain separate. `implemented-local-verified`
means the changed owner passed the specific local compilation or execution gate in
the matrix; `source-reconciled` records an evidenced baseline, supersession, retained
contract or source-only adoption. Full local implementation hashes are recorded below and remain local; no push occurred. Device, simulator-native, Windows/Linux/browser-native,
MSRV, clippy and static gates receive credit only from their own receipts. No row
asserts unexecuted physical input, native submitted frames or numerical performance.

## Fixed provenance and scope

- Fork comparison baseline: `a0e5a818e0a3628114e37362bc57a0bc240e66c0`. Concurrent working-tree edits are not attributed to this revision.
- Content-verified reconstruction anchor: Zed `6e9465a4288c332208643892e23b9d35d7be5c79`. This is the excluded left endpoint, not an asserted original import checkout or a cross-repository ancestor.
- Available upstream object-history endpoint: Zed `78f5d7446c55f6c1aa9b66e8111f6b3f4625ebb0`. The local reference is non-shallow; the actual objects for the anchor and this endpoint are available.
- Selected final source target: Zed `6ceb6fee8738274589b59139df27efc191ec2231`. At the initial reconciliation the seven later objects were unavailable, so six official rendering patches plus the final official product patch supplied their full SHA/title/path provenance. The November 13 continuation can read the exact target objects and independently verifies the same inventory directly from Git history; it does not change the fixed target.
- Existing whole-framework historical pin: `635026fabc38`. The earlier bounded ports do not establish a fully synchronized base. This ledger includes the entire anchor-to-endpoint history.

The [import record](zed-gpui-import.md#november-5-provenance-reconstruction),
[fixed candidate queue](zed-gpui-upstream-sync-candidates.md#fixed-revisions-and-provenance)
and [sync skill](../../.agents/skills/open-gpui-upstream-sync/SKILL.md) define provenance
and the source-comparison method. [ADR 0001](../adr/0001-open-gpui-fork-strategy.md)
owns the extracted dependency/license boundary. The rendering successor is recorded
in the [candidate queue](zed-gpui-upstream-sync-candidates.md#november-12-rendering-successor)
and [qualification record](../verification.md#november-12-rendering-successor).

The exact historical pathspec is the original imported closure, its selected successor
packages, and the root manifest/toolchain files listed below. `refineable` includes its
nested derive package. Deleted Media and moved pre-extraction owners remain in scope.
Root `Cargo.lock` is used as dependency evidence, not as an independent pathspec:
lockfile-only monorepo/product history is not part of these source/manifests/toolchain
counts. This is an explicit inventory boundary, not an outside-framework disposition
for commits that were never inspected.

```text
crates/collections
crates/gpui
crates/gpui_linux
crates/gpui_macos
crates/gpui_macros
crates/gpui_platform
crates/gpui_shared_string
crates/gpui_util
crates/gpui_web
crates/gpui_wgpu
crates/gpui_windows
crates/http_client
crates/http_client_tls
crates/media
crates/refineable
crates/reqwest_client
crates/scheduler
crates/sum_tree
crates/util
crates/util_macros
crates/gpui_apple
crates/gpui_ios
crates/gpui_tokio
crates/path
crates/github_releases
crates/bench_metrics
crates/zlog
crates/ztracing
crates/ztracing_macro
Cargo.toml
rust-toolchain.toml
```

The `crates/gpui` directory pathspec covers the **entire package**: examples, benches, resources, all package-local integration tests, build scripts, manifests, licenses and documentation. It is not a production-src filter. The 246-row Core audit is a semantic lane inventory, not the filter used for the 557-commit historical union.

The newly selected independent Apache `gpui_tokio` bridge already existed at the content anchor but was not imported at the fork baseline. [TOKIO-ANCHOR](#tokio-anchor) records **anchor-level adopted-now** source selection, implemented and locally verified; it does not invent an introduction commit at the left endpoint. Its three range commits already occur in the canonical ledger. Adding the full `crates/gpui_tokio` directory yields **557 historical SHAs, zero newly discovered SHAs**; the later seven entries keep the canonical total at 564.

Titles come from the actual historical objects or official patch headers.
The title links open the official commit; classification follows its changed source
and surviving final mechanism, never the title. For absent local objects the permanent
evidence is the official `.patch` URL of that same commit plus the recorded selected
source and integration owners below. No reference checkout was fetched or moved.

## Inventory and state counts

The November 13 independent continuation rechecks all 31 pathspec entries, including the
21 original imported entries, nested derive packages and deleted Media. The exact
anchor-to-`78f5d744` history has **557 non-merge commits**; the strict pathspec through
`6ceb6fee` has **563**, with the explicitly retained final product-only commit making
the canonical set **564**. Expected entries, ledger rows and unique full SHAs are
**564 / 564 / 564**; missing, extra and duplicate entries are **0 / 0 / 0**. The
SHA-256 of the sorted full-SHA set, joined with newlines and a final newline, is
`cf53075acfb33590d870994c30590bdaa0df9bf3c623e68ff81e0815e18132ef`.
All **149 / 149** root/toolchain entries retain their normalized official titles;
125 changed root keys/member entries and all 81 outside-framework source slices
were inspected. The original eleven queued entries and one partial renderer entry
map to current owners and local commits that are ancestors of `68d52b00`.

A separate source lane rechecks iOS activation/interaction/composition/frame-demand/
retirement, shared Apple, complete Tokio source, all five EGL root selections and
the final Windows/WGPU sprite/COLR mechanisms against the exact fixed target.
It confirms those source owners without adding execution credit. The continuation's
native Mac activation finding and repair are recorded in the
[editor-framework evidence](../knowledge/engineering/verification/editor-framework-evolution-20261113.md).
These checks add no upstream row, do not count the ten later incremental commits
again, and do not turn docking U7 or Nara qualification into upstream omissions.

| Inventory | Commit count | Counting rule |
| --- | ---: | --- |
| Historical source/manifests/toolchain union | 557 | Open-left anchor-to-78f5 history; zero merge commits |
| Core audit | 246 | 246 Core/shared/example rows; native-only slices belong to platform groups |
| Platform audit | 210 | 206 primary commits plus 4 Darwin process commits |
| Support / selected iOS audit | 101 | H01–H17; mixed commits retain applicable framework slices |
| Newly included Tokio bridge history | 3 | Already-recorded mixed commits; inherited anchor-level owner is separately pinned, no extra SHA |
| Root manifest / toolchain history | 149 | 33 support-relevant entries plus 116 residual entries, all recorded individually |
| Windows / WGPU primary audit | 81 | Full path history; no title-based exclusions |
| Windows / WGPU supplemental evidence | 12 | Disjoint from its 81 primary entries; overlaps other lanes |
| Later rendering patches | 6 | One entry per saved official patch header |
| Final git_ui product patch | 1 | Only branch_picker.rs changes |
| Canonical ledger total | 564 | 557 historical commits + 6 rendering commits + 1 product commit |

The lane inventories overlap. Their arithmetic sum is not the ledger count.

The 149 root entries comprise 33 support-relevant entries and 116 residual entries.
The residual set contains 83 individually evidenced product/tooling slices, 32 slices
with an adopted or retained source disposition, and one initially missing EGL
dependency correction, now adopted. The former 30 D-OPEN root slices were checked against
their actual Cargo/toolchain patch, surviving owner, dependency use and lock source:
17 adopted, 3 retained-contract, 9 product-only and 1 real-missing at the audit
checkpoint. With the exact EGL fix, the current root-slice decisions are **18 adopted,
3 retained-contract and 9 product-only**. There are **zero unexplained root slices**;
[D-EGL](#d-egl) preserves the original defect and executed repair evidence.
These are scope counts; mixed commits retain every applicable framework contribution.

| Aggregate row disposition | Rows | Meaning |
| --- | ---: | --- |
| `source-reconciled` | 429 | 425 baseline/source dispositions plus four completed source-only adoptions: two P04 rows, OHOS routing and the Windows RC/include fix. Their foreign/native qualification remains separate. |
| `implemented-local-verified` | 54 | Current corrections have the scoped local compile/test receipts in the matrix. Full implementation hashes are supplied in the owner map below. |
| `open` | 0 | Every selected source contribution has an implemented, superseded, retained-contract or outside-framework disposition. |
| `outside-framework` | 81 | Inspected source/root slice belongs only to excluded product/tooling behavior; evidence is retained per row. |

A mixed commit records its changed, locally verified owner without granting execution
credit to its other slices. An excluded product slice never closes a framework slice.
Historical findings below describe the fixed baseline; their current implementation
and verification are recorded separately. Unexecuted foreign/native gates do not
reopen a reconciled source decision.

## Local implementation revision map

Every canonical row maps to local source through its linked owner group(s). The
group's baseline checkpoint or the current owner key below is its **per-row local
SHA mapping**. Mixed commits can map to several local implementations. The owner map supplies full implementation hashes for every current key; several
keys intentionally share an atomic integration commit. The exact slice named by each
group determines the credit, rather than every file in a mixed commit.
Baseline credit applies only to the unchanged slices described by a group.
For an outside-framework row, its owning group records the inspected exclusion;
there is no local implementation SHA to assign to that excluded behavior.

| Local owner key | Evidence groups / exact slice | Local implementation SHA(s) |
| --- | --- | --- |
| L-BASE | Pre-existing slices with an explicit baseline checkpoint | `a0e5a818e0a3628114e37362bc57a0bc240e66c0` |
| L-CORE | G-M1/M2/M3/M5/M7/M11, G-C1/C2, G-IOS-CORE, Q-LIST/TAB/TEXT/ELEMENT/EXAMPLE, H02 Core static text | `300c05d28a7b50ec90c296850aad01224e7bb0a4`; `138c6ef22c4ce8bdacad800bd1ae2cebd2cf0c44` |
| L-WINDOW | G-M4/M6/M8/M9/M10/M12 and P27 | `300c05d28a7b50ec90c296850aad01224e7bb0a4` |
| L-GPU | G-GW1/GW3/GW4/GW5, Q-COSMIC/COLR, current W-WGPU/W-WINDOWS and RENDER-2/W-RENDER corrections | `4f8299386f8143cd0ff8fc3f5f1e21eef555f74a`; `d99e96a50714022d9dfb25b04763cf249654a714` |
| L-PLATFORM | P04/P25/P26, H05 and U-RC | `300c05d28a7b50ec90c296850aad01224e7bb0a4`; `4f8299386f8143cd0ff8fc3f5f1e21eef555f74a`; `69b5338ad1c2488e5d8052a0d6487df584b15e80`; `ca5f3abf148c1c9234ef4f09876da55b0956bd86` |
| L-IOS | H17/D07 and native UIKit/shared Apple integration | `ca5f3abf148c1c9234ef4f09876da55b0956bd86` |
| L-BENCH | P22, G-M13, G-BENCH, G-GW2 and the complete 17-mutation ChildText chain | `10878654b465c4f2f4e4fd4781f712952439b79e`; `d99e96a50714022d9dfb25b04763cf249654a714` |
| L-TOKIO | TOKIO-ANCHOR and T-TOKIO crate/member/alias/ordinary consumer | `ca5f3abf148c1c9234ef4f09876da55b0956bd86` |
| L-SHARED | H02 package equality/hash | `1660391ab1fbdd99a21336ef5e74bb9191ee80a6` |
| L-DEPS | D-EGL root patches/real-binding fixture and applicable independent-root dependency/lock integration, including D07 UIKit/Tokio entries | `ca5f3abf148c1c9234ef4f09876da55b0956bd86` |
| L-EXAMPLES | U-SETUP, U-WASM, U-POSITION and platform example/dev-dependency integration | `138c6ef22c4ce8bdacad800bd1ae2cebd2cf0c44`; `ca5f3abf148c1c9234ef4f09876da55b0956bd86` |
| L-RENDER | Historical six-renderer adoption; RENDER-2's current WGPU correction maps additionally to L-GPU | `0ed315daa37e94d6f80b357339ab8eff6b9b5afc` |

## Platform and functional-unit verification matrix

The upstream column references mechanism groups whose canonical rows contain every
**full upstream SHA**. TOKIO-ANCHOR separately pins the pre-range source snapshot;
its excluded-left-endpoint SHA is not a new introduction/history row. The owner keys
are the local SHA map above, not another upstream commit inventory.
The main thread owns actual commands, nextest receipts, reruns and limits in
[docs/verification.md](../verification.md); this document worker ran no Cargo command.

The host is Apple Silicon/macOS 27.0, Rust 1.99.1 and SDK 26.5, with zero active
displays. Main serialized Cargo with two jobs and nextest with one test thread,
reusing the checkout target. These are the completed local receipts:

| Receipt | Executed scope and result |
| --- | --- |
| F-ALL | Final affected nextest: **3615/3615 passed**, 93 binaries, 125.510 s; `a5c5f0e9-7fcf-45bc-b489-5393662e1e27`. Core/WGPU/Windows/shared Apple/macOS/Tokio/iOS/UI Core/UI components/platform with `test-support,bench-support,runtime_shaders,screen-capture`. Three pre-existing AppKit main-thread tests remain ignored. |
| F-PUBLIC | Independently locked consumer all-target check passes with `bench-support,wgpu-renderer,mac-screen-capture,open_gpui_platform/runtime_shaders`. Its ordinary public runtime cohort passes **8/8**, four binaries, zero skipped; `a7159fe9-15dd-487b-9d4f-061cb549ce2b`. |
| F-WORKSPACE | Locked workspace all-target check passes with `open-gpui-apple/runtime_shaders`; includes the registered platform examples. |
| F-WINDOWS | Real cargo-xwin MSVC sysroot and default MSVC-driver `clang-cl` all-target check passes for Core/WGPU/Windows/platform/util/Tokio, `x86_64-pc-windows-msvc`, with `test-support,bench-support,runtime_shaders,screen-capture`. No fake headers, forced target cfg or native execution. |
| F-IOS | `IPHONEOS_DEPLOYMENT_TARGET=15.0` locked `open-gpui-ios`/platform **all-target, all-feature** check passes for `aarch64-apple-ios-sim`. The new independently locked iOS consumer also resolves and passes its locked simulator check. Device `aarch64-apple-ios` library/all-feature compilation and the Python native-probe package build/codesign pass; the native iPhone 17/iOS 26.3 simulator probe passes with its explicit success marker after the recorded fixture/key-window/oracle corrections. |
| F-WASM | Production Core/Web/platform compilation passes for `wasm32-unknown-unknown`; the separate `cargo +nightly check --locked -p open-gpui-platform --example a11y --target wasm32-unknown-unknown` passes after native-only Criterion/dev-dependency gating. No browser execution. |
| F-BENCH | Final locked platform/WGPU bench compilation passes with `bench-support,font-kit,inspector,runtime_shaders`, after the ChildText chain completion. Python fixture-path tests pass **5/5**. Five-target native Criterion test smoke also passes; the 500-frame churn diagnostic completes in 93.465 s after an initial 55-s timeout. These unoptimized smoke durations are not performance comparisons or speedup evidence. |
| F-EGL | Exact official binding unit tests **8/8** and real dynamic-loader consumer **3/3** pass. Identical consumer against registry 6.0.0 is **2/3**, reproducing `Instance::initialize`'s `unwrap(None)`. Native symbols are fixture mocks, not a driver/compositor. |
| F-MSRV | Real Rust **1.93.0** locked affected host all-target check passes in **132.180 s** for Core/WGPU/shared Apple/macOS/platform/Tokio with `test-support,bench-support,runtime_shaders,screen-capture`. The separate iOS/platform simulator **all-target/all-feature** check passes in **104.546 s** on the same real compiler. |

The three existing ignored AppKit cases are `test_foreground_tasks_run_with_run_until_parked`,
`test_advance_clock_triggers_delayed_tasks` and `test_window_spawn_uses_test_dispatcher`.
A single Rust worker does not provide the process main thread they require.

Historical failures remain evidence of the changes that made these receipts pass:

- Initial WGPU/text **9/10**, with 114 skipped: `combining_marks_are_positioned_over_their_base`
  reports `mark.x=7.872px, next.x=7.872px`; shader/ordinary-image controls pass.
  The glyph-offset repair passes in the focused **111/111** run
  `eda96ec4-eec7-425d-b9a0-ec43792b2298` and F-ALL.
- Initial Core compilation exposes twelve `E0405` missing parent `IntoElement`
  imports after blanket conversion, followed by the missing `Image::is_asset_cached`
  facade and AppContext import. The integrated intermediate run
  `c4d81bfd-21a3-4cc7-a877-3c363cb35e51` is **92/96**: a 1,616-byte AnyElement
  exceeds the 1,024-byte fixture arena, and three Window probes update an already
  borrowed root. A 16-KiB arena and probes that avoid duplicate updates preserve
  the assertions; the expanded 111/111 and F-ALL runs pass.
- Complete intermediate run `e8eae75e-66db-4bfb-a813-7d4bc0e583bd` is **3595/3598**
  across 87 binaries. The wrapper-lifetime fixture must request real truncation
  after lazy wrapper acquisition; the public prompt fixture must expect official
  `OK`; the command-palette fixture must project the TestPlatform's actual OS.
  Nonempty-cache/weak-release, native-versus-window prompt and explicit Windows
  shortcut assertions are retained. The targeted repair is **3/3**
  (`32722f8c-3da4-41e9-8e5a-a931dd4ef6de`), followed by F-ALL.
- Independent List review finds two vertical-inset defects shared with upstream.
  The real red test is **0/1**, `0a846f67-8dfa-48ee-8952-46b805ec3d3b`:
  item y=0/range=0 instead of y=10/range=20. `bounds_for_item` now includes
  `padding.top`; `max_scroll_offset` includes top and bottom insets in the same
  List owner, preserving frozen drag heights. The same test, including
  bring-into-view reaching -20, passes in F-ALL; independent re-review closes both.
- The first bench-only check exposes an ungated test visual module; its corrected
  feature guard passes. The final F-BENCH additionally compiles the full
  17-mutation ChildText state/render/benchmark chain, independently source-reviewed.
- iOS compilation first catches missing Metal protocol imports, ordinary-example
  dev-dependencies and raw-window-handle UFCS. F-IOS follows those repairs.
  All seven prior independent iOS findings are fixed and independently re-reviewed
  closed; native activation, interaction quiescence, idle/in-flight composition
  cleanup, demand scheduling and exact renderer-before-native retirement have
  current source owners. The later native simulator probe executes those paths with its explicit success marker; physical-device/system-input qualification remains separate.
- The initial ordinary Tokio consumer imports `HeadlessAppContext` without
  `test-support`, producing E0432. It now uses public `bench_platform` and
  `Application::run_embedded`; F-PUBLIC passes without adding test support.
- Windows GNU-driver Clang fails in aws-lc HRSS vector operators/MSVC intrinsics;
  the unchanged production graph passes with MSVC-driver `clang-cl` in F-WINDOWS.
  Cross-host build-script cfg does not execute FXC or qualify RC/link output.
- Initial offline independent-lock refresh proposes unrelated downgrades. Main
  replaces only its resolver edits and refreshes online, preserving prior selected
  versions while adding required UIKit/iOS/Tokio entries. D-EGL records exact sources.
- The first a11y wasm dev-target check exposes native Criterion threading. Platform
  native-only dev-dependency gating yields the F-WASM rerun; production compilation
  alone would not have established that dev-target boundary.
- The first actual iOS simulator probe panics from a duplicate root borrow in
  `InteractionProbe::observe_activation`: root-leased `window.update` quiescence
  synchronously delivers the inactive observer. The fixture is changed to public
  `AsyncApp::update_window(window.into(), ...)` for window-only idle quiescence,
  rejection and close, changing only the three fixture calls, with production
  unchanged by this repair. The launch CLI returns zero despite the app panic, so native
  qualification requires the explicit **`IOS NATIVE WINDOWING OK`** marker. The later bounded native run passes as recorded in the follow-up receipt below.

The simulator-native probe, strict host/iOS Clippy, formatting and existing import/API/docs/release/dependency-health gates now pass. Physical devices/input and actual two-revision numerical benchmark measurements retain their owning runner. Historical Windows `97341cbb` exercised the old renderer/text path;
F-WINDOWS compiles the CPU COLR successor, while real DirectWrite/COLR fonts,
WARP/readback, IME/caption/PopUp and Job execution still require Windows.

| Functional unit / upstream SHA map | Local SHA owner key | Completed evidence / current disposition | Remaining platform or measurement qualification |
| --- | --- | --- | --- |
| Core integration: [C-R](#c-r), [C-V](#c-v), [C-E](#c-e), [C-I](#c-i) and current Q/G groups | L-BASE, L-CORE, L-WINDOW | F-ALL/F-WORKSPACE/F-PUBLIC; current corrections implemented-local-verified | Native geometry/input/submitted frames remain tied to their runners; three existing AppKit ignored cases remain explicit |
| Keymap precedence / continuations: [G-M1](#g-m1), [G-M2](#g-m2) | L-CORE | `keymap.rs` retains disabling bindings, source rank/depth/load order and full-continuation dispatch resolution; F-ALL and public command-palette controls pass | No numerical cost reduction claimed |
| Div drag-over and debug selectors: [G-M3](#g-m3), [G-M5](#g-m5) | L-CORE | Style-only drag hitbox and `debug_assertions` source gates adopted; affected host regressions pass in F-ALL | A separate release-mode selector execution receipt is not supplied |
| Window pointer, IDs, scale, asset/atlas, accepted paint and touch getter: [G-M4](#g-m4), [G-M6](#g-m6), [G-M8](#g-m8), [G-M9](#g-m9), [G-M10](#g-m10), [G-M12](#g-m12) | L-WINDOW | Actual-ingress, scale/cache/paint/menu/touch controls pass in F-ALL; public `core_window_api` is compiled by F-PUBLIC | Native pointer/scale callbacks on foreign platforms require their own execution |
| TestPlatform menus: [P27](#p27) | L-WINDOW | Detached owned snapshots, replacement and clearing regression pass in F-ALL | Native menu publication is a separate pre-existing owner |
| Const colors / touch vocabulary / prompt / image extension: [G-M7](#g-m7), [G-M11](#g-m11), [G-C1](#g-c1), [G-C2](#g-c2) | L-CORE | Complete public source present; F-ALL/F-PUBLIC pass, including corrected `OK` expectation | API vocabulary alone does not claim native Touch-variant synthesis |
| Blanket IntoElement: [Q-ELEMENT](#q-element) | L-CORE | Blanket trait/coherence and ordinary custom-element/component consumers compile; corrected arena fixture passes in F-ALL | No outstanding selected source contribution |
| List / Edges / tab order / lazy wrapper / async example: [Q-LIST](#q-list), [Q-TAB](#q-tab), [Q-TEXT](#q-text), [Q-EXAMPLE](#q-example) | L-CORE | Horizontal padding unit and both additional vertical-inset fixes; batched tab order, truncation-only wrapper and both notify calls; F-ALL/F-WORKSPACE pass; List re-review closed | No native or performance result inferred from portable tests |
| SharedString and static text: [H02](#h02) | L-SHARED, L-CORE | Package equality/hash **3/3** at `1660391a`, run `3617aa39-45b0-4245-9530-d2d1b0160d7d`; Core static-text entry implemented and covered by F-ALL/F-PUBLIC compilation | Both complete package/Core hashes are in the owner map |
| WGPU ordinary sprite branch: [RENDER-2](#render-2), [W-RENDER](#w-render) | L-RENDER, L-GPU | Early flat/simple return before instance load; retained opacity, clip envelope and scene-color conversion; shader/image controls pass in F-ALL; F-WASM compiles | Browser/WebGL execution and measured shader cost require separate receipts |
| Cosmic offsets, sort and corpus: [Q-COSMIC](#q-cosmic), [G-GW1](#g-gw1), [G-GW2](#g-gw2) | L-GPU, L-BENCH | Offset red-to-green, paragraph/source mapping and host tests pass in F-ALL; `sort_unstable`/dedup and separator-free/U+001C-Hebrew workloads adopted; F-BENCH compiles | No shaping or benchmark speedup claimed |
| Windows CPU COLR: [Q-COLR](#q-colr) | L-GPU | Complete DirectWrite/CPU owner, zeroed planes/layer union/correction/straight BGRA and GPU removal; portable arithmetic passes in F-ALL; real-sysroot F-WINDOWS all-target check passes | Real DirectWrite fonts, COLR layers, WARP/GPU readback and native controls unexecuted |
| Windows caption / IME / PopUp: [G-GW3](#g-gw3), [G-GW4](#g-gw4), [G-GW5](#g-gw5) | L-GPU | Live style checks before hit/release and after callback, focus-only shared IME completion, ownerless non-provisional PopUp TOPMOST; F-WINDOWS compiles; independent source review closed | Actual disabled buttons, sibling composition and native z-order require Windows |
| Windows Job activity: [H05](#h05) | L-PLATFORM | Existing owned-job accounting, truthful cleanup docs and unmanaged/live-grandchild fixtures implemented; F-WINDOWS compiles | Execute None/Some(true)/Some(false)/Err and process teardown scenarios on Windows |
| Hidden Mac activation: [P26](#p26) | L-PLATFORM | `activate_mac_window` invalidates a hidden view before `makeKeyAndOrderFront`; F-ALL/F-WORKSPACE compile the native owner | Zero active displays; visible reveal/reactivation and startup measurements unexecuted |
| Shared Apple / AppKit / Darwin: [P01](#p01), [P02](#p02), [P03](#p03), [P05](#p05), [P07](#p07), [P08](#p08), [P09](#p09), [P10](#p10), [P17](#p17), [P18](#p18), [P29](#p29) | L-BASE; L-IOS/L-BENCH for changed slices | Baseline source reconciled; affected host owners participate in F-ALL/F-WORKSPACE, iOS compilation in F-IOS | Existing exact native close/sheet/resize/capture/font/process receipts retain their own revisions and limits |
| Wayland receipt timing / desktop identity: [P04](#p04) | L-PLATFORM | Complete current source: receipt `Instant::now`, Unix desktop detection and non-Unix None route; source-reconciled | No current Linux build/compositor receipt supplied |
| Linux native baseline: [P13](#p13), [P14](#p14), [P15](#p15), [P16](#p16) | L-BASE | Source-reconciled; EGL successor tracked separately below | X11/Wayland/headless/IME/clipboard/drag scheduling retain native runner limits |
| Web / mobile shared baseline: [P12](#p12), [P19](#p19), [P20](#p20), [P21](#p21), [P28](#p28) | L-BASE; L-CORE/L-IOS for selected successors | Existing source dispositions retained; production graph compiles in F-WASM; native iOS now selected separately | Browser WebGPU/WebGL/input/clipboard/RAF and stock-font runtime controls remain distinct |
| Six-renderer integration: [RENDER-1](#render-1), [RENDER-3](#render-3), [RENDER-4](#render-4), [RENDER-5](#render-5), [RENDER-6](#render-6) | L-RENDER; L-GPU for RENDER-2 | Historical adopted source/receipts retained; current affected host tests pass in F-ALL, Windows and wasm compile in F-WINDOWS/F-WASM | Foreign native submitted frames, fractional clips/UVs, depth and GPU completion require owning runners |
| Benchmarks and ChildText: [P22](#p22), [G-BENCH](#g-bench), [G-M13](#g-m13), [G-GW2](#g-gw2) | L-BENCH | Four additional targets, five non-tree workloads, `with_paths`, 17-way mutation/state/render chain and comparison script adopted; independent source review closed; F-BENCH passes, Python **5/5**, and native Criterion test smoke completes across all five targets, including the full 500-frame churn heap control | Unoptimized smoke supplies no numerical comparison; two-revision measurement invocation and Windows script process cleanup unexecuted |
| iOS Core lifecycle: [G-IOS-CORE](#g-ios-core) | L-CORE | Ordered ingress, subscription custody, Active/Inactive/Background/Foreground/Disconnected and memory-pressure tests pass in F-ALL; existing `PlatformInputHandler::focus_id` is public across crates | Core FIFO/subscription behavior passes locally; physical-device lifecycle/background/reconnection and memory-pressure delivery remain separate |
| iOS backend / examples: [H17](#h17), [D07](#d07) | L-IOS | F-IOS passes all targets/features and independently locked simulator consumer; seven historical review findings fixed and re-review closed; device lib/all-feature compilation and native probe build/codesign pass | Native iPhone 17/iOS 26.3 probe passes: public activation, both composition paths, six pixel oracles, resized drawable, idle guards and exact retirement. Physical devices/input/performance unexecuted |
| OHOS routing: [P25](#p25) | L-PLATFORM | All three Cargo/public-factory/current-platform guards present; source-reconciled, desktop selector compiles in F-WORKSPACE/F-PUBLIC | No OHOS target/backend execution; official source supplies the injection guard, no in-tree backend |
| Independent Tokio bridge: [TOKIO-ANCHOR](#tokio-anchor), [T-TOKIO](#t-tokio) | L-TOKIO | Crate/member/alias/locks complete; seven real public bridge tests pass in F-ALL and focused run; ordinary consumer passes F-PUBLIC | Foreign runtime/platform execution retains its own scope |
| Standalone setup guide: [U-SETUP](#u-setup) | L-EXAMPLES | Correct extracted aliases/factory/native text/OS features documented; ordinary public consumer and selector compile in F-PUBLIC/F-WORKSPACE/F-WINDOWS; real host/simulator compiler floor passes F-MSRV | Linux feature-specific build/runtime remains separate |
| Taffy/text prerequisites: [U-TAFFY](#u-taffy), [D-TEXT](#d-text) | `11d9a96033ea8425b07f64fdf2bad6503f1720dc`, L-BASE; L-BENCH for current corpus | Complete compute_layout source comparison and exact lock sources retained; affected layout/wrapper tests pass in F-ALL; corpus compiles in F-BENCH | No complete third-party-release equivalence or numerical performance claim |
| Windows RC/include: [U-RC](#u-rc) | L-PLATFORM | Both Core `ParamsIncludeDirs` and RC-relative manifest hunks present; Windows-manifest route cross-compiles in F-WINDOWS; source-reconciled | Cross-host build-script cfg does not execute Windows RC/link; manifest-resource output unqualified |
| a11y / native dev scope / testing examples: [U-WASM](#u-wasm), [Q-EXAMPLE](#q-example) | L-EXAMPLES, L-CORE | `no_main`, native-only proptest and platform Criterion/dev gates, explicit examples; F-WORKSPACE/F-WASM and testing-example F-ALL pass | Browser UI/native accessibility transport requires execution |
| Rewritten Web/Trunk instructions: [U-DOCS](#u-docs) | L-BASE | Evidenced supersession: old URL/gallery paragraph absent; source-reconciled | Existing Web smoke owner retains runtime qualification |
| Display positioning example: [U-POSITION](#u-position) | L-EXAMPLES | Complete five-formula patch, matching reverse-check and actual `window_positioning` platform target; F-WORKSPACE compiles | Multiple displays with nonzero/negative origins/work areas not exercised |
| EGL dependency correction: [D-EGL](#d-egl) | L-DEPS | Exact `42214b36347b108fd9630ca6254b96dde59e04fe` root/consumer pins and locks; F-EGL **8/8 + 3/3**, registry **2/3** with actual panic; implemented-local-verified | Native Linux/FreeBSD GLES, Wayland/X11 and ANGLE driver execution unqualified |
| Support/dependency boundary: [H01](#h01), [H03](#h03), [H04](#h04), [H06](#h06), [H07](#h07), [H08](#h08), [H09](#h09), [H10](#h10), [H11](#h11), [H12](#h12), [H13](#h13), [H14](#h14), [H15](#h15), [H16](#h16), [D01](#d01), [D02](#d02), [D03](#d03), [D04](#d04), [D05](#d05), [D06](#d06), [D-ROOT30](#d-root30) | L-BASE plus current owning keys | All 30 root decisions explained; EGL source gap closed; F-WORKSPACE/F-PUBLIC and real host/simulator Rust 1.93 F-MSRV pass | Strict host/iOS Clippy, formatting and existing static gates pass; full local hashes recorded above. Native TLS/process/browser scope stays explicit |


## Other-platform execution handoff

Each row below reaches the full official SHA through its evidence group and the
full local SHA through its L-* key. These are **commands to run on the named
runner**; command text alone grants no execution credit. The later incremental
ledger supplies explicitly scoped Chromium and final AppKit receipts without
changing this historical source inventory. Use one Cargo process, retain logs/run IDs, and add
the actual platform/compiler/feature/device facts to the verification record.

| Runner / source map | Starting command | Required observation and remaining scope |
| --- | --- | --- |
| Active macOS GUI: P26 / L-PLATFORM | `cargo build --locked -p open-gpui-macos --features test-support,runtime_shaders --examples`; `python3 scripts/run_macos_native_probe.py target/debug/examples/native_windowing -- sheet`, repeat with `native-close`; run `native_layer_shutdown` through the same helper with `--exercise-window` and `--ancestor-backed` variants | The later incremental ledger records the final **14/14** native matrix after a display returns, including hidden activation, sheets, native/reentrant close, both submitted-resize and layer-retirement variants, appearance and actual quit. Every process has both its receipt and normal OS exit 0. Physical input, fullscreen/multiple-display pixels and separate forced-close contention remain unqualified. |
| Windows CPU COLR: Q-COLR / L-GPU | `cargo nextest run --locked -p open-gpui-windows --features test-support,runtime_shaders --lib -E 'test(direct_write::readback_tests::)' --test-threads=1 --no-fail-fast` | Real DirectWrite recommended modes, extending color layers, straight BGRA and headless emoji readback on installed color fonts. Portable bitmap tests and cross-compilation do not replace this. |
| Windows input/window state: G-GW3/GW4/GW5 / L-GPU | `cargo nextest run --locked -p open-gpui-windows --all-features --lib --test-threads=1 --no-fail-fast` | Disabled caption hit/release and live style changes; sibling-window IME focus ownership; ordinary topmost PopUp versus owned/provisional cases. Run the interactive native workflow where focus input needs a desktop. |
| Windows Job activity: H05 / L-PLATFORM | `cargo nextest run --locked -p open-gpui-util --lib -E 'test(process::windows_tests::)' --test-threads=1 --no-fail-fast` | Unassigned None, direct-child exit with live grandchild Some(true), terminated job Some(false), real query errors and teardown. |
| Windows renderer/resources: W-RENDER/U-RC / L-GPU/L-PLATFORM | `cargo check --locked -p open-gpui-windows -p open-gpui-platform --all-targets --all-features`; repeat the Windows library tests with runtime shaders enabled and disabled | Actual FXC entries, embedded shaders, WARP/readback, resource compiler/link and manifest output. Mac cross-host build-script cfg does not execute these native compiler paths. |
| Linux X11/Wayland/GLES: P04/P13-P16/D-EGL / L-PLATFORM/L-DEPS | `cargo check --locked -p open-gpui-linux --all-targets --features x11,wayland,test-support`; then `cargo nextest run --locked -p open-gpui-linux --features x11,wayland,test-support --lib -E 'test(native_windowing_tests::)' --run-ignored only --test-threads=1 --no-fail-fast` | Supply live DISPLAY/WAYLAND_DISPLAY or the documented GPUI_LINUX_TEST_* overrides. Check native switching, callback receipt time/profiler, GLES failure propagation, IME, clipboard/drag and renderer teardown. FreeBSD retains its own build/compositor runner. |
| Browser: P12/P19-P21/W-RENDER / L-BASE/L-GPU | `cargo run --locked -p xtask -- web-smoke`, with the matching wasm toolchain/CLI and an available local Chromium binary | The later incremental ledger records the complete actual Chromium WebGL/WebGPU pixel/input/clipboard/text/font/touch/viewport workflow passing in 222.274 s after its fixes; explicit WebGPU touch also passes. Safari, physical input and hardware-failure fallback retain separate qualification. An unavailable skip and wasm compilation alone grant no browser pass. |
| Physical iOS: H17/G-IOS-CORE / L-IOS/L-CORE | `cargo check --locked -p open-gpui-ios -p open-gpui-platform --target aarch64-apple-ios --lib --all-features`; package/sign the ordinary iOS example for an owned device | Device build already passes locally; signed device execution still needs hardware. Check IME/session changes, rotation/iPad resize, lifecycle/reconnection/memory warning, safe-area/keyboard, Universal Links, gestures and real GPU retirement. Diagnose the recorded UIKit appearance warning. |
| OHOS: P25 / L-PLATFORM | Inspect `cargo tree --locked -p open-gpui-platform --target aarch64-unknown-linux-ohos`; check the package with an actual OHOS SDK/toolchain | Verify desktop Linux exclusion and explicit `Application::with_platform` injection. No in-tree OHOS backend is selected or claimed. |
| Numeric comparison: G-BENCH / L-BENCH | `python3.13 scripts/compare_benchmarks.py BASELINE CANDIDATE --measurement wall-time` | Choose two committed revisions already exposing the same manifest/API fixtures. Retain the created branches/worktrees, actual features/fonts/device and raw samples; use optimized bench settings for numerical claims. The host five-target smoke is not such a comparison. |

## Evidence group definitions

Owner paths are checkout-relative unless a permanent link is supplied. The upstream
introduction/follow-up is available through the official link on each canonical row;
the final historical source is pinned at 78f5, and the six renderer groups use the
official fixed patches/final selected source. `W/`, `G/` and `C/` in Windows/WGPU row
notes mean `crates/gpui_windows/`, `crates/gpui_wgpu/` and `crates/gpui/` respectively.

These groups retain the source audit's mechanism reasoning. Findings explicitly
labelled historical describe the fixed baseline, with current source and completed
gates in the matrix. Every row's group links map its local SHA through the baseline
checkpoint or the [current owner map](#local-implementation-revision-map).
A matching symbol alone
does not establish adoption. Candidate acceptance, subscription/focus publication,
exact clips/transforms, atlas generations/leases, ordered native commands and real
GPU submission remain fork contracts wherever the route touches them. Retained
differences need the cited consumer/Accepted contract and their owning verification.

### C-R

**Runtime, App, arena, executor, profiler and benchmark harness.**

App owns stable Rc element-arena allocation with nested clear/access scopes; queue/executor and benchmark owners preserve real readiness, bounded draining, teardown and injected native resources. Profiler/frame admission keeps real signal, dirty and submission boundaries. Owners: crates/gpui/src/{app.rs,arena.rs,executor.rs,queue.rs,profiler.rs}, app/bench_context.rs and window/frame_admission.rs. The target effect-flush guard removal is intentionally superseded by AppCell unwind restoration, nested update transactions and shutdown fences.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [arena](../verification.md#element-arena-and-native-frame-maintenance), [profiler](../verification.md#unified-profiler-foundation), [benchmark](../verification.md#complete-benchmark-context-and-measurements). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### C-V

**Views, element identity and cached dependencies.**

ViewElement and generic View/Render/RenderOnce embedding preserve typed/erased identity and actual draw/prepaint/paint/deferred dependency reads. Reads belong to the admitted candidate, released generations are filtered and opacity survives cached/deferred replay. Owners: crates/gpui/src/view/{element,traits}.rs, element.rs, app/entity_map.rs and Window frame/deferred scopes. A similar API name or relocated source file alone is not the source proof.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [views](../verification.md#generic-views-and-reusable-component-interfaces), [cached dependency/replay](../verification.md#cached-dependencies-deferred-replay-and-opacity). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### C-E

**Elements, assets, image/SVG, lists and animation.**

Div interactivity, List provisional anchors/admitted scrolling, owning CachedLoad and shared image decoding have surviving source owners in crates/gpui/src/elements/{div,list,uniform_list,img,image_cache,animation}.rs. ParsedSvg/exact-size/8192px handling remains in svg_renderer.rs. Spring uses one solver in open-gpui-motion through the Core reexport. Direct indexed List scrolling maps to scroll_to_item under Accepted ADR 0025; SVG placement maps to the general projected subtree owner under ADR 0021. Arbitrary SVG rotation is an existing documented limit, not inferred equivalence.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [element/SVG/List](../verification.md#element-layout-svg-and-provisional-list-measurement), [assets](../verification.md#owning-asset-loads-and-view-completion), [animation](../verification.md#generic-animation-controls-and-clock). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### C-I

**Input, focus, gestures and dispatch.**

Keymap and Window dispatch preserve source/context/load ordering; current precedence/continuation gaps are separately tracked below. Existing gestures, long-press/scroll sources, focus lifetimes and modifier/IME settlement live in keymap.rs, window.rs, window/touch_gestures.rs, elements/div.rs and app/native_event_ingress.rs. Arbitrary-point front-to-back hit testing uses HitboxId::window_point_target_rank(...).is_some(), rather than raw geometric containment. Focus/input subscriptions commit only with the admitted frame.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [pending input](../verification.md#pending-input-modifier-recognition-and-blur), [input/focus](../verification.md#input-priority-and-focus-lifetime), [gestures](../verification.md#gestures-scroll-sources-and-hover). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### C-A

**Accessibility publication and synthetic children.**

The source owners in window/a11y.rs, elements/div/accessibility.rs and view/element.rs preserve synthetic parentage, active descendants, author/property publication and view provenance. The raw explicit published-tree reader is distinct from redacted automatic diagnostics under Accepted ADR 0023. Rejected/modal/hidden candidates cannot replace accepted snapshots or focus authority. iOS native transport is a separate selected unit.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [synthetic/inspection](../verification.md#synthetic-accessibility-and-explicit-published-tree-inspection), [properties](../verification.md#accessibility-properties-configuration-and-initial-trees). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### C-T

**Text, wrapping, font ownership and layout.**

TextSystem pools only CharWidthCache and owns font generation; window-local Rc/RefCell transactional layout caches recover after refusal/reentry. Generic source lookup, layered paint/source splitting and UAX #14 wrapping retain original source positions. Taffy 0.15, explicit leaf/alignment/position, truncation budgets, MaxContent List probes and root/grid policies are reconciled through text_system.rs, text_system/{line,line_layout,line_wrapper}.rs, taffy.rs, style.rs and font_features.rs. Cosmic/DirectWrite native/backend slices and current wrapper/mark corrections retain their own groups.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [text ownership](../verification.md#window-local-text-ownership-and-source-cache), [wrapping](../verification.md#unicode-wrapping-indentation-and-middle-truncation), [layout](../verification.md#taffy-text-measurement-and-rootgrid-sizing). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### C-F

**Shared facade, values and geometry.**

crates/gpui/src/{platform,color,geometry}.rs own portable value/trait contracts. Native/render implementations are classified by their actual platform groups. The baseline prompt spelling, image format extension, const-constructor and additive test-API gaps are now implemented through their G/Q owners; neighboring values or low-level seams were not used as substitutes.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [Window/facade](../verification.md#window-controls-lazy-inspector-and-win32-dispatch), [layout](../verification.md#taffy-text-measurement-and-rootgrid-sizing). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### C-X

**Core audit scope boundary.**

The Core audit excluded native backends, GPU algorithms, support and some example/documentation slices from its own lane. This is not a whole-framework rejection. The same canonical row links the actual owning platform/support/current-work group; if no source disposition exists, it stays open. The six former lane-only source entries now have actual introduction/final-owner evidence in [U-SOURCE](#u-source).

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [candidate inventory](zed-gpui-upstream-sync-candidates.md). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P01

**Diagnostics.**

Fork evidence: `crates/gpui/src/app.rs:2128; crates/gpui_macos/src/{platform,window,display_link}.rs; crates/gpui_apple/src/{display_link,metal_renderer}.rs`.

Read-only platform/window/frame-source snapshots exist. App-owned snapshots preserve checked-out window identities and do not take pending signals. Product CLI/socket plumbing is excluded.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#macos-source-reconciliation-on-november-12). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P02

**Mac titlebar, native appearance and fullscreen.**

Fork evidence: `crates/gpui_macos/src/window.rs; crates/gpui_macos/src/window/{simple_fullscreen,sheet}.rs; crates/gpui_macos/src/{platform,window_appearance}.rs`.

Native titlebar-container visibility, custom drag authority, button tracking-area restoration, fullscreen failure/exit state, native Space restoration, system Fill selector and app appearance override are present. Explicit sheet owners and native retirement are retained. The c855/fca revert/reintroduction sequence is reconciled to final 23bb behavior.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#macos-native-input-appkit-and-screen-capture). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P03

**Display topology and placement.**

Fork evidence: `crates/gpui_macos/src/display/{native,topology}.rs; crates/gpui_macos/src/display.rs; crates/gpui_linux/src/linux/{wayland,x11}/window.rs; crates/gpui/src/app.rs`.

Connected-display observation and window association use immutable native observations and ordered App ingress. Active placement and explicit display precedence are present. Existing centered API remains supported alongside centered_on_display.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#connected-displays-and-window-association). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P04

**Desktop environment and Wayland receipt timing — source adoption complete.**

Fork evidence: `crates/gpui/src/platform.rs:180; crates/gpui_linux/src/linux/wayland/client.rs:2137`.

Current `platform.rs` provides Unix current_desktop/desktop_environment observation and None on other platforms. Wayland `client.rs` captures `Instant::now()` when the frame callback is received rather than interpreting the undefined callback clock. These are current corrections, not baseline a0e5a818 credit. Local SHA mapping: L-PLATFORM. Disposition: source-reconciled; no Linux target/compositor execution receipt is supplied.

Verification owner: [owning record](../verification.md#platform-frame-signals-and-completed-skips). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P05

**Mac clipboard, typed retained pasteboards.**

Fork evidence: `crates/gpui_macos/src/pasteboard.rs; crates/gpui_macos/src/platform.rs`.

Optional retained native owners handle unavailable general/find pasteboards. Empty data, metadata, image and external paths remain supported across autorelease boundaries.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#desktop-clipboard-normalization-and-owned-pasteboard). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P06

**Common keystroke and cursor seams.**

Fork evidence: `crates/gpui/src/window.rs:13305; crates/gpui/src/platform/keystroke.rs; crates/gpui/src/platform.rs`.

Drag cursor precedence and post-listener cursor resolution are present; modifier-prefix recognition/timeout and pre-IME pending binding settlement use existing input transactions. AsKeystroke reference forwarding and normalized OK button spelling are present.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#pending-input-modifier-recognition-and-blur). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P07

**Mac keyboard, pointer, typed native bindings.**

Fork evidence: `crates/gpui_macos/src/{events,keyboard,platform,window}.rs; crates/gpui_macos/src/window/{hover,pointer_button}.rs`.

Typed objc2 event/application/menu/window bindings, Backspace/Delete and Spanish-ISO mappings, inactive hover and physical-button synthetic-drag cancellation survive. Native handlers deliver through post-borrow ingress instead of borrowing App inside native callbacks.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#macos-native-input-appkit-and-screen-capture). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P08

**Apple Metal rendering, build and shared topology.**

Fork evidence: `crates/gpui_apple/{build.rs,Cargo.toml}; crates/gpui_apple/src/{gpui_apple,metal_renderer,display_link,dispatcher,text_system,keychain,process_info}.rs; crates/gpui_apple/src/shaders.metal; crates/gpui_macos/src/gpui_macos.rs`.

Shared Apple owners, typed Metal resources, cached main-thread marker, source-over/P3, per-batch surface bounds, offscreen screenshot storage and autorelease pool are present. Build stages shaders into OUT_DIR, selects SDKROOT, compiles for macOS 12 and invokes tools from OUT_DIR. The six later renderer changes intentionally make the current renderer differ from the 78f5 source. The initially omitted 1808 atlas_upload benchmark is now adopted under G-BENCH/L-BENCH and compiles in F-BENCH.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#shared-apple-framework-topology). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P09

**Apple font and keychain services.**

Fork evidence: `crates/gpui_apple/src/{text_system,open_type,keychain}.rs; crates/gpui_macos/src/platform.rs`.

CoreText PostScript identity deduplication and first nonempty-run exact size are present. Fallback descriptors carry native weight/slant and preserve privately retained installed faces. Keychain add-only insertion preserves duplicate, missing, cancellation and authentication distinctions. Graphical NoopTextSystem fallback emits the warning.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#credential-storage-and-add-only-keychain). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P10

**Frame policies and visibility.**

Fork evidence: `crates/gpui/src/platform.rs:8237; crates/gpui/src/window/frame_journal.rs; crates/gpui_macos/src/{display_link,window}.rs; crates/gpui_apple/src/{display_link,process_info}.rs; crates/gpui_web/src/frame_loop.rs`.

Inactive/low-power/thermal intervals and platform frame signals/skips are present. Mac per-display CVDisplayLink/M5 Metal display links, no-screen fallback and one-shot signal ownership survive. Visibility and profiler publication follow accepted candidates and actual submission; a renamed owner is not an omission.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#power-modes-and-stable-frame-cadence). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P11

**Platform test support and image API boundary.**

Fork evidence: `crates/gpui/src/platform.rs; crates/gpui/src/platform/test/{platform,window}.rs; crates/gpui/src/elements/img.rs`.

Animated WebP/EXIF/static image decoding is shared; clipboard image values remain available. Deterministic test-support selection, scale-factor simulation and NotSupported raw handles survive. These commits touch the platform boundary; unrelated Core layout/text algorithms remain another lane.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#additive-test-support-and-explicit-leak-tracking). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P12

**Web foreground dispatch and HTTP.**

Fork evidence: `crates/gpui_web/src/{dispatcher,http_client,platform}.rs; crates/gpui_web/Cargo.toml; crates/gpui/src/executor.rs:43`.

Fetch uses SendForegroundExecutor, original headers/credential policy and streamed response bodies with asynchronous channel sends. Worker dispatch and mailbox wake/unlock behavior retain cancellation and shutdown ownership. Idle scheduling and wasm-threads dependency selection are present. Async clipboard image/text reads use accepted native sessions and ordered App delivery.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#sendable-foreground-submission-and-web-fetch). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P13

**Linux activation and serial custody.**

Fork evidence: `crates/gpui_linux/src/linux/wayland/{client,window,serial}.rs`.

Activation requests carry their own PendingActivation payload; latest eligible keyboard/pointer press records the requesting surface. Selection ownership is press-only. Startup token/app-id behavior survives. Fork window payload additionally carries exact handle identity, retaining stale-target rejection.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#native-regions-layer-shell-fullscreen-and-popups). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P14

**Linux headless windowing and X11/Wayland scheduling.**

Fork evidence: `crates/gpui_linux/src/linux/{platform,dispatcher,headless}.rs; crates/gpui_linux/src/linux/headless/window.rs; crates/gpui_linux/src/linux/wayland/{client,frame_state,window}.rs; crates/gpui_linux/src/linux/x11/client.rs; Cargo.toml`.

Headless open-window/display API and selected rendering hook exist. Wayland demand-driven scheduling retains frame-loop/ping/retry/deadline work and post-borrow drain. X11 drains buffered events after foreground work, requests/full-renders Expose recovery and releases client state before close callbacks. Timer shutdown retains task semantics. Missing-feature diagnostics and official calloop patch prerequisite are present. WGPU implementation internals are excluded.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#startup-and-live-windowing-lifecycle). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P15

**Linux IME, XKB, metadata, effects and regions.**

Fork evidence: `crates/gpui_linux/src/linux/{platform,text_system}.rs; crates/gpui_linux/src/linux/wayland/{client,window,popup}.rs; crates/gpui_linux/src/linux/x11/{client,window}.rs; crates/gpui_linux/Cargo.toml`.

Inactive/no-handler IME gates, cursor rectangle/serial dedup and server-keymap XKB context are present. WM_CLASS NUL termination, urgency clearing and scaled window-relative pointer coordinates survive. ext-background-effect is preferred over KDE fallback; rounded client regions, layer shell zone/edge, native popups and child-before-parent retirement are present. wayland-backend 0.3.15/log and protocols minimum are selected. Cosmic font-cache internals are another lane.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#linux-ime-background-effects-and-keyboard-contexts). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P16

**Linux clipboard and file drag sessions.**

Fork evidence: `crates/gpui_linux/src/linux/platform.rs:1715; crates/gpui_linux/src/linux/wayland/{client,clipboard}.rs`.

Nonblocking bounded pipe reads retain partial/chunked/EINTR behavior. Drag URI reads carry generations and reject stale completions; outbound wl_data_source transfer and completion/cancellation reuse exact native sessions.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#cursor-and-complete-native-file-drag). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P17

**Mac native dialogs, capture and external drag.**

Fork evidence: `crates/gpui_macos/src/{platform,screen_capture,window}.rs; crates/gpui_macos/src/window/{file_drag,native_close,sheet}.rs; crates/gpui_macos/Cargo.toml`.

Nullable open/save factories report errors. Typed screens/capture and retained CoreVideo surfaces are present. Native file source reentry, UTType icons and explicit sheet begin/end survive. Accessibility adapter is taken during disposal to break its native view cycle. Native terminal events retain exact shutdown/retirement authority.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#macos-native-input-appkit-and-screen-capture). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P18

**Shared native lifecycle, restart, idle sleep and notifications.**

Fork evidence: `crates/gpui/src/platform.rs; crates/gpui/src/window.rs; crates/gpui_macos/src/platform/{termination,termination/tests}.rs; crates/gpui_macos/src/{platform,system_notifications}.rs; crates/gpui_linux/src/linux/{platform,system_notifications}.rs; crates/gpui_apple/src/dispatcher.rs`.

Quit callbacks return cancellation decisions; restart forwards OS-string arguments. Sleep/wake, activity guard and system notification APIs/selected transports are present. Native close and notifications queue App access after native borrow release; terminal tickets and exact retirement are retained. Product AI/telemetry/CLI consumers are excluded.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#system-notifications-and-ordered-app-responses). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P19

**Web text, keyboard, viewport and native default behavior.**

Fork evidence: `crates/gpui_web/src/{events,window,ime_mirror,viewport,text_edit,frame_loop,interaction,pointer_session}.rs`.

Textarea/beforeinput/selection/composition, UTF-16 source imports, editable range, iOS software keyboard pass-through and actual document focus are present. visualViewport/safe-area sampling, prevent-scroll focus and canvas selection/callout policy survive. Weak frame wake/RAF scheduling and event listener cleanup use current owners. ed4d conditional native-context cancellation was reverted by eeac; the final unconditional cancellation survives. Input handlers are leased and app delivery follows accepted sessions.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#web-text-sessions-and-viewport). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P20

**Web Canvas font fallback and ordinary font consumer.**

Fork evidence: `crates/gpui_web/src/{text_system,canvas_text,canvas_fallback,glyph_cache,run_replacements,platform}.rs; crates/gpui_web/Cargo.toml`.

Canvas fallback includes text-default emoji candidate replacement and post-fallback missing-glyph reporting. Dynamic application font installation forwards to the existing generation owner. Unlike ef075, the fork still preloads damascene-fonts Inter/JetBrains Mono in WebPlatform::new_with_options (platform.rs:196). Existing Canvas qualification explicitly retains stock fonts as fork provenance, and the actual hello/smoke consumers depend on them. Record this policy difference explicitly rather than claim an empty database or missing application font API. Core/WGPU text implementation is outside this lane.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#web-canvas-font-fallback). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P21

**Web touch and mobile shared API slices.**

Fork evidence: `crates/gpui/src/{platform,interactive}.rs; crates/gpui_web/src/{events,window,touch_session,pointer_session,platform_info}.rs`.

The baseline already supplies ScrollEvent/source, Touch/LongPress/BackGesture values, corner_radii and focus-aware leased input/selection geometry for ordinary Core/Web consumers. Its historical native-iOS exclusion and absent lifecycle callback API are superseded by complete selected adoption in H17 and G-IOS-CORE. Current Core callbacks and UIKit producers include Disconnected and memory warnings. F-ALL tests portable delivery; F-IOS compiles the native producer. Existing Core/Web behavior alone supplies no native UIKit execution credit.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#gestures-scroll-sources-and-hover). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P22

**Benchmark runtime and completed selected workloads.**

Fork evidence: `crates/gpui/src/platform/threaded_dispatcher.rs; crates/gpui/src/app/bench_context.rs; crates/gpui_platform/benches/randomized_element_tree.rs; crates/gpui_platform/Cargo.toml; tests/fixtures/dependency-consumer/benches`.

Threaded dispatcher readiness, cancel/drop-outside-lock, quiescence, frame/task bounds and real native text/renderer injection are present. The historical audit found adopted tree/family runtime but omitted inspector_render, primitives, atlas_upload and focus_cleanup targets, five non-tree workloads and comparison tooling. Runtime Inspector/focus cleanup was not used to close those missing benchmark slices.

Current manifests register all four additional targets; native factories, workloads, with_paths and comparison tooling are implemented under G-BENCH/L-BENCH. The full 17-mutation ChildText state/render chain is reconciled and independently source-reviewed. Final platform/WGPU bench compilation F-BENCH passes after that completion, and Python fixture-path controls pass 5/5. No benchmark timing or native performance improvement is claimed. Product benchmark consumers remain outside scope.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#randomized-trees-and-platform-tree-workloads). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P23

**Shared atlas/cache access and retained resource lifetime.**

Fork evidence: `crates/gpui/src/platform/atlas.rs; crates/gpui_apple/src/metal_atlas.rs:211; crates/gpui_apple/src/metal_renderer.rs; crates/gpui/src/platform/test/window.rs`.

Shared atlas key publication, contains/image-cache observation, checked byte-length uploads and released-texture handling are present. Published regions remain allocated until the exact texture has neither keys nor leases; only unpublished allocations can cancel immediately. This intentionally supersedes 0d8 immediate tile reclamation for retained Scenes/native submissions. It is not missing.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#shared-atlas-ownership-and-retained-renderer-bindings). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P24

**Dependency/features and extracted package boundaries.**

Fork evidence: `crates/gpui_platform/Cargo.toml; crates/gpui_{apple,macos,linux,web}/Cargo.toml; crates/gpui_platform/src/gpui_platform.rs; docs/imports/zed-gpui-import.md`.

Open-GPUI packages and explicit feature selections preserve the extracted license/dependency closure. Web threading is opt-in through web-multithreaded. WGPU/WebGL selected hooks are present; renderer algorithms are another lane. 74490 only changes Windows test feature forwarding here, and Windows capture is excluded. raw-window-handle/util dependency owner moves and declaration cleanup do not establish functional omissions.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#independent-dependency-consumer). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P25

**Selected complete OHOS routing guard.**

The initial audit correctly found the missing guard but its desktop-lane exclusion did not close the portable framework slice. The current user selection adopts the complete official 484bcbfea6ec routing change. Current crates/gpui_platform/Cargo.toml excludes target_env=ohos from the Linux/FreeBSD gpui_linux dependency. The public linux factory has the same exclusion. current_platform emits the explicit OHOS diagnostic directing callers to Application::with_platform and excludes OHOS from automatic Linux selection. All three official routing contributions are present in current source. The official in-tree source has no OHOS backend: guard adoption requires no import of an external product/backend and no historical external-ecosystem rejection contract.

Local SHA mapping: L-PLATFORM. Disposition: **source-reconciled**. Desktop selector/ordinary consumer compilation passes in F-WORKSPACE/F-PUBLIC; no OHOS target/backend execution is claimed.

### P26

**Hidden Mac activation view invalidation.**

Fork evidence: `crates/gpui_macos/src/window.rs:4261; crates/gpui_macos/src/window.rs:2066; crates/gpui/src/window.rs:14267`.

Historical baseline finding: activate_mac_window reached makeKeyAndOrderFront without the official hidden-view invalidation, and the display-link fallback did not supply that operation on arbitrary reactivation. Current `activate_mac_window` captures the native view, checks `isVisible`, and sends `setNeedsDisplay: YES` before ordering a hidden window. Exact activation/native custody and admitted first presentation remain with their existing owners. L-PLATFORM maps the current implementation; F-ALL/F-WORKSPACE compile it. With zero active displays, visible reveal/reactivation and startup timing remain unexecuted.

Verification owner: [owning record](../verification.md#macos-source-reconciliation-on-november-12). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P27

**TestPlatform menu snapshots — current source implementation.**

Fork evidence: `crates/gpui/src/platform/test/platform.rs:1371; crates/gpui/src/platform.rs:3942; crates/gpui/src/app.rs:3729`.

The initial audit found an empty set_menus and no saved menu owner or get_menus override, causing App::get_menus to receive default None after set_menus. The Window/test-support implementation lane now adds RefCell<Vec<OwnedMenu>>, converts Menu through Menu::owned on every replacement, and returns detached cloned snapshots. MacPlatform retains its existing menu owner. The product disable_ai setting remains outside the framework closure.

New `crates/gpui/src/app/test_context/menu_tests.rs` exercises App::set_menus/get_menus through the actual TestPlatform: initial empty menus, action identity, OS action, checked/disabled flags, separators, nested disabled menus, system menu roles, snapshot independence, replacement and clearing. `test_platform_menus_preserve_owned_snapshots_and_replace_contents` passes in F-ALL. No menu-specific public testing wrapper was introduced. Local SHA mapping: L-WINDOW.

Verification owner: [owning record](../verification.md#additive-test-support-and-explicit-leak-tracking). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P28

**Web ordinary example.**

Fork evidence: `crates/gpui_web/examples/hello_web/main.rs:410; crates/gpui/src/app.rs:355`.

The ordinary example still calls application().run rather than run_embedded. The fork run implementation explicitly retains App in RUNNING_WEB_APPLICATIONS on wasm, preventing the premature application drop addressed by the official example migration. run_embedded and explicit ApplicationHandle remain available. This example uses the existing packaged font defaults, not application font registration. Do not report a lifetime omission from the method spelling alone.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#externally-driven-application-lifetime). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### P29

**Darwin process owner (supplemental full-history group).**

Fork evidence: `crates/util/src/command/darwin.rs`, `crates/util/src/command.rs`, root and independent `[patch.crates-io] async-process` selections. Owned stdio/dev-null descriptors, CLOEXEC_DEFAULT inheritance, SIGPIPE reset, Async<File> pipes, adopted PID reaping and environment-local PATH resolution are present. Existing process-reaper tests are source evidence only in this review.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#process-reaping-dedicated-execution-and-shared-strings). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H01

**scheduler / execution (7).**

**Equivalent/adopted.** `scheduler/src/{scheduler,clock,test_scheduler,executor}.rs` retains lazy weak-scheduler dedicated-task construction, injected local dispatch, dedicated/rendezvous task semantics, native-only blocking, shared scheduling closures, runtime test detection and sendable foreground wake identity. Scheduler and clock match target after aliases. Executor's only production delta is the documented weak sender in DedicatedExecutor, preventing its queued-runnable ownership cycle; local tests prove retained work and late-wake behavior. Tree-sitter consumers stay outside, not their executor slice.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#process-reaping-dedicated-execution-and-shared-strings). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H02

**SharedString (3).**

**Complete current adoption.** From<char>, direct SmolStr serde and pointer/length fast equality with content hashing retain final target production source in `gpui_shared_string/gpui_shared_string.rs`. Content/hash/static/independent-allocation tests preserve value semantics.

The package-owner equality/hash tests pass **3/3** at local `1660391a`, nextest `3617aa39-45b0-4245-9530-d2d1b0160d7d`. Official 415d310e5260 also contributes the now-implemented Core static-text entry; F-ALL/F-PUBLIC compile/test the affected Core integration. The package receipt alone was not used as whole-row evidence. Local SHA mapping: **L-SHARED and L-CORE**; both full hashes are supplied in the owner map.

Verification owner: [owning record](../verification.md#process-reaping-dedicated-execution-and-shared-strings). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H03

**Darwin child process (4).**

**Adopted with superseding correctness fix.** `util/src/command/darwin.rs` keeps owned File descriptors, explicit inherited stdio, SIGPIPE restoration, Async<File> pipes and adopted PID reaper. Direct async_process::Child and smol's reexport resolve to the exact same patched package. Explicit child PATH handling intentionally supersedes the target's already-diagnosed parent-PATH fallback defect; it preserves argv0, relative/empty entries and EACCES. Import and Mac follow-up qualification own that difference.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#process-reaping-dedicated-execution-and-shared-strings). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H04

**existing Windows process cleanup (1).**

**Adopted.** `util/src/process.rs` owns job assignment/kill/drop and existing descendant cleanup tests. Root already provides JobObjects. Does not close H05.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#process-reaping-dedicated-execution-and-shared-strings). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H05

**Windows job observation (1).**

**Historical finding S-G1 — fixed.** The baseline lacked owned-job accounting, truthful best-effort cleanup docs and the two official scenarios. Current `crates/util/src/process.rs` queries the existing owned job's `ActiveProcesses`, preserving None/Some(true)/Some(false)/Err, and adds unmanaged-child/live-grandchild tests without a second process service. L-PLATFORM maps the change; F-WINDOWS compiles production and fixtures. Windows execution of Job accounting/cleanup remains unqualified.

Verification owner: [owning record](../verification.md#process-reaping-dedicated-execution-and-shared-strings). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H06

**complete shell construction/discovery (10).**

**Equivalent/adopted.** `util/src/shell_builder.rs` is exact target; `shell.rs` differs only in a product-neutral comment and `shell_env.rs` only in naming/formatting. `gpui_util/src/lib.rs` keeps official ProgramFiles/MSIX/preview/Scoop/.NET/PATH/SystemRoot ordering and OnceLock-style cache through the injected discovery seam. Includes syntax-error stdin redirection, noninteractive regression, malformed variables, literal arguments, spaced Windows paths, noisy environment capture and case handling. Agent/remote launchers outside do not exclude these framework helpers.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#utility-composition-shells-and-development-assets). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H07

**canonical portable paths (14).**

**Equivalent/adopted through final successor.** Complete `path/src/{abs_path,rel_path,path,path_component,windows_prefix}.rs`, reexported by util. Earlier ad hoc join/normalize/strip-prefix implementations are superseded by final std-derived component owner. RelPath::empty_arc survives the multi_buffer commit. Trust-scope's heterogeneous AsRef inputs and titlebar's file_stem/extension survive the product titles. All production/test code matches after aliases except stable `path.as_os_str().is_empty()` spelling and a rustdoc link. Apache license correction is adopted.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#portable-paths-and-utility-consumers). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H08

**path consumers and parsing (7).**

**Equivalent/adopted.** `util/src/paths.rs` matches final target including lenient per-pattern errors, verbatim UNC, parenthesized positions, repeated filename/parent components, separator preservation, extension comparison and canonicalizing path:line helper. Only test-home naming and an added home test differ. Product file existence/opening/titlebar policy remains outside, not these parser/comparator mechanisms.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#portable-paths-and-utility-consumers). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H09

**lightweight utility owners (13).**

**Equivalent/adopted.** Canonical core-util owns command/UTF-8/bounded-sort/log formatting; higher util forwards it and adopts target native/wasm module boundaries. Final exact `markdown.rs`, `redact.rs`, and util JSON policies retain fragment positions and replacement versus deduplicating union; parse_os_release and Unix soft-FD raising are present. `gpui_util` log path preserves caller location and outlined fmt::Arguments; IntoElement's outlined conversion is present. Stable first_chunk::<8>() replaces the target's newer as_array spelling. Product telemetry/LSP/remote policy stays outside.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#utility-composition-shells-and-development-assets). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H10

**HTTP/release download (8).**

**Equivalent/adopted.** reqwest keeps keepalive, Pending-safe reader, per-read idle timeout, total deadline and exact request-local redirect owner; fork additionally reenters original Tokio context on body polls. `http_client` request/response abstractions match except intentionally removed Zed endpoint builders. Moved `github_releases` owns metadata, deadline, case-insensitive/prefix-normalized digest, staging, archive extraction and raw binaries. Its sole production delta closes/drops the output before executable permission handling; releases.rs is exact after aliases. ACP consumers are outside, checksum mechanism is not.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#http-deadlines-release-downloads-and-pax-archives). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H11

**explicitly reverted HTTP experiment (2).**

**Superseded/reverted, intentionally not ported.** Follow the final target, which reverts the sleep-time workaround; later idle timeout remains adopted. Existing queue explicitly records this pair.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#http-deadlines-release-downloads-and-pax-archives). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H12

**benchmark macros/measurement (5).**

**Equivalent/adopted complete successor.** `gpui_macros/src/bench.rs`, `bench_metrics/src/bench_metrics.rs`, Core bench context/report and platform factories implement typed measurement, native text/GPU injection, per-input/per-seed routine isolation, heap metrics and teardown. Metric production source matches target; proc_macro2 test seam and visibility preservation are deliberate corrections. Criterion 0.8.2, Linux perf-event2 0.7.4 and Unix libc are present; no retired GPL perf owner is imported. Editor optimization title does not exclude its benchmark slice.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#complete-benchmark-context-and-measurements). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H13

**public/derive/style macros (3).**

**Equivalent/adopted.** BoxShadow builder exists; generated shadow methods use equivalent field literals. View/IntoElement derive and proptest macro implementation match final target after aliases/formatting. Removed visibility_style_methods and inspector expansion are **intentionally replaced under Accepted ADR 0022**, not missing APIs: SubtreePresentation governs paint/input/focus/accessibility at admitted frames. Upstream integration tests map to `gpui/tests/{context_macros,render_macros,inspector_reflection_macros}.rs`; no circular dev dependency is needed in gpui_macros.

The intentional visibility replacement is backed by [Accepted ADR 0022](../adr/0022-open-gpui-subtree-presentation-authority.md).

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#generic-views-and-reusable-component-interfaces). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H14

**development/embedded assets (4).**

**Equivalent/adopted with qualified dependency successor.** `util/src/util.rs` owns executable-first runtime repository discovery, dynamic development get/iter, debug-embed and release derive. Compression is explicitly enabled by util; the actual 8.13 trait requires the added compressed(None) adapter/reexports. Root/consumer locked patched compression versions and existing verification own that integration. Product corgi tooling and rodio are outside, not fs_embed/compression.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#utility-composition-shells-and-development-assets). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H15

**logging/tracing (4).**

**Equivalent/adopted; explicit embedding policy.** Local Apache zlog/ztracing/ztracing_macro owners retain scoped filters including pet/simplecss, browser performance implementation (exact web.rs), native instrumentation and disabled macro (exact lib.rs). Fork fixes standard-record target filtering/config max level and file-init retry. ADR 0001/import record intentionally replace ambient Zed build cfg with explicit tracy/web features and leave logger/subscriber/global allocator installation to the application. ZED_LOG becomes OPEN_GPUI_LOG with RUST_LOG fallback. GPL zlog_settings remains outside.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#standalone-logging-and-tracing). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H16

**manifests/tooling/minimum compiler (6).**

**Equivalent source prerequisites / intentional extraction differences.** Flume 0.12, TLS verifier fallback and exact reqwest, target-gated native utility modules, test support and required proc-macro/serde/regex/log features are present. Local testing remains enabled rather than adopting monorepo test=false suppression. cargo-shear metadata is outside runtime; existing workspace cargo-machete policy remains. Rust 1.93 is the declared qualified fork floor, not upstream's 1.98.1. Dependency versions/default features are reconciled separately below; no unproved equality of third-party releases is asserted.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [owning record](../verification.md#independent-dependency-consumer). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### H17

**selected independent iOS backend (9).**

All nine native iOS commits are selected and implemented. Owners: `crates/gpui_ios/**`, the platform selector/root dependency graph and shared Apple/Core interfaces. The unit includes UIApplication/UIScene lifecycle and launch-once/frame demand; native window/display/Metal retirement; UTF-16 UITextInput, session identity and stale-selection rejection; safe-area/IME insets; touch/trackpad/back gestures and display radii; initial/continued Universal Links with query/fragment; platform/power identity, cached main-thread marker and block2 bindings; and the Cargo ios example/native text_input harness. G-IOS-CORE supplies the separate ordered subscription/ingress prerequisite, including the public existing `PlatformInputHandler::focus_id` across crates. Historical lane exclusions are superseded by this selection.

Current `ios/application.rs` delivers active/inactive status with visibility changes and revisits the live window registry after reentrant observers. `ios/window/lifecycle.rs`, `native_window.rs` and `text_input.rs` fence interaction before teardown and preserve exact shutdown authority; `PlatformInputHandlerSlot` settles idle/in-flight composition termination. `window/frame_demand.rs` retains parked demand and schedules only ready surfaces. Native retirement requires the renderer to be quiesced for the same ticket before detaching UIKit owners. All **seven prior independent review findings are fixed**, and independent source re-review closes them. This source receipt is distinct from UIKit execution.

F-IOS passes simulator **all targets/all features**, the independently locked iOS consumer, and device **library/all features**. The native Python package build and codesign pass. Initial simulator compilation failures (Metal protocol imports, ordinary example dependencies and raw-window-handle UFCS) are preserved above. The first actual native probe fails with the root-borrow fixture error recorded above; the corrected native probe passes with `IOS NATIVE WINDOWING OK` after the real key-ownership and drawable-oracle follow-ups recorded above. The launch CLI's zero exit alone is never a pass. Physical device/input and native/performance qualification remain separate.

The existing shared Apple build owner selects iphonesimulator / `-mios-simulator-version-min=15.0` for simulator and iphoneos / `-mios-version-min=15.0` for device. No second Metal build owner is added. Current platform capability implementations, rather than earlier unsupported-method notes, own service support. Local SHA mapping: **L-IOS**, with **L-CORE** for portable prerequisites; both full hashes are supplied in the owner map.

Verification owner: [owning record](../verification.md#shared-apple-framework-topology). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### TOKIO-ANCHOR

**Inherited Apache Tokio bridge — anchor-level adopted-now.**

`crates/gpui_tokio` exists at the excluded-left content anchor `6e9465a4288c332208643892e23b9d35d7be5c79` but is absent from fork baseline `a0e5a818e0a3628114e37362bc57a0bc240e66c0`. The original import list did not include this applicable independent Apache bridge. Its initial source blob is `d32843ae86f428a3f47f54e3387a75c5aedbcddc`; the final 78f5 source blob is `6ef71b4890106bdf733c873a52c0cf076ab28477`. The target manifest explicitly declares Apache-2.0, with the package Apache license entry. [Anchor source](https://github.com/zed-industries/zed/blob/6e9465a4288c332208643892e23b9d35d7be5c79/crates/gpui_tokio/src/gpui_tokio.rs) and [target manifest](https://github.com/zed-industries/zed/blob/78f5d7446c55f6c1aa9b66e8111f6b3f4625ebb0/crates/gpui_tokio/Cargo.toml) are permanent provenance.

The adopted mechanism stores a Tokio handle in an App Global, either owning a two-worker enable_all runtime or borrowing an application-supplied handle. Owned-runtime drop calls shutdown_background; a supplied runtime is not owned or shut down. Tokio::spawn and spawn_result run the actual future on Tokio, return through a GPUI background Task, and retain a defer/abort handle so dropping the GPUI task cancels the Tokio task. JoinError and nested anyhow results retain their separate public result routes; handle returns a clone of the current handle. This is an explicit bridge, not a replacement executor or a product transport service.

Adoption decision: **adopted-now at the anchor level**, implemented-local-verified. Current bridge source, root workspace member, consumer-side open_gpui_tokio alias and ordinary consumer/lock integration are complete. Seven real public bridge tests pass in the focused and F-ALL cohorts, covering owned/supplied runtime custody, Task-drop cancellation, result/JoinError propagation, handle reuse and nonblocking owned-runtime shutdown; F-PUBLIC compiles and runs the ordinary bridge consumer without test-support. L-TOKIO maps the local implementation hash above. The unused root workspace dependency alias is later removed in `68d52b00`; direct ordinary consumers remain, and `98fb59fe` corrects the README accordingly. This grants no baseline credit, asserts no introduction by the anchor commit and adds no synthetic history row.

### T-TOKIO

**Tokio bridge follow-ups within the full historical range.**

All three full SHAs already have canonical mixed-owner rows: a873cf402c8d moves util::defer to gpui_util and its manifest dependency; aec7395e30fe adds monorepo test=false metadata; d544470d1d37 removes redundant explicit Tokio rt while retaining rt-multi-thread. The actual three patches and complete final bridge were read. The executable bridge source is otherwise unchanged after its util-owner move. The Tokio package retains upstream `doctest=false` and adopts this commit's `test=false` for its empty unit harness; seven public integration tests remain enabled and execute the bridge. Other packages retain their documented standalone test/doc policy. L-TOKIO records current crate/member/alias/public-consumer integration and runtime cancellation/shutdown verification.

Local SHA mapping: **L-TOKIO**. Disposition: **implemented-local-verified**, through F-ALL/F-PUBLIC and the seven focused bridge tests. Tokio retains both upstream disabled empty harness settings and its seven enabled public integration tests. This documentation lane executes no bridge command.

### D01

**Root dependency and toolchain source.**

Exact async-process `0b6d6713570af61806e1e5cb40e0f757cb93fd9d`, scheduler async-task `015845a19fc31752c02367567d3b8900a406f9da`, and calloop `3759371fee14c40066d64777e2a36b6ffcc22590` exist in root and ordinary-consumer manifests/locks. async-process/calloop root patches cannot be exported by a dependency library; existing consumer declares them. The product-labeled ACP SDK commit 67b41bf976 contains the applicable async-task patch switch and is **not discarded by title**. A separate registry async-task copy also exists transitively; it is not the scheduler's selected owner.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [import source selections](zed-gpui-import.md) and [independent consumer](../verification.md#independent-dependency-consumer). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### D02

**Root dependency and toolchain source.**

Initial async-tar 0.6 update is superseded by exact PAX fork `bd3ad6f89df9a9da7a8535958756d6bf465936a0`, adopted. Exact zed-reqwest `33bc764aa15ff7b200bf7c93bd96e24878d53e14`, verifier 0.7 and bundled WebPKI fallback exist. Root/consumer roots are 1.0.11, newer than target 1.0.10. Which 6 versus target 8 is a retained package-version difference; official source discovery strategy and its local source regressions are present, not a missing source mechanism.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [import source selections](zed-gpui-import.md) and [independent consumer](../verification.md#independent-dependency-consumer). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### D03

**Root dependency and toolchain source.**

Flume 0.12 is selected locally and its wasm-thread spin feature is forwarded. Futures root resolves 0.3.32 versus target/independent consumer 0.3.34; no new API requirement occurs in the unchanged support source. Ctor root resolves 1.0.7 versus target 1.0.12 and independent consumer 1.0.13; this is **not claimed exact third-party parity**. The later ctor priority-section fixes are package-level changes, not absent GPUI methods. Util uses existing nix 0.29 resource/user APIs and mach2 0.5 constants; upstream chooses nix 0.30/mach2 0.6. Keep version/feature differences visible to root owner; do not infer equivalence of entire dependency releases from successful symbol lookup.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [import source selections](zed-gpui-import.md) and [independent consumer](../verification.md#independent-dependency-consumer). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### D04

**Root dependency and toolchain source.**

Existing shared Apple/macOS owners and binding family already carry objc2, Foundation, block2, CoreText/Security, QuartzCore display-link and Windows Job/Power features. Local broad defaults are a source of extra dependency features, not a demonstrated missing required feature. I-G1 and new UIKit dependency are separate; already shared Apple substrate must not be reimported. Native/foreign qualification remains with those owners.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [import source selections](zed-gpui-import.md) and [independent consumer](../verification.md#independent-dependency-consumer). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### D05

**Root dependency and toolchain source.**

Canonical local path/serde, raw-window-handle 0.6.2 (per-crate), explicit JS/web dependency declarations, compressed rust-embed 8.13 successor, Criterion 0.8.2/perf-event2 and proptest 1.12 are present. Syn/serde/log/regex required per-crate features are supplied by the retained root default/explicit superset. Monorepo-wide feature trimming is not an independent framework runtime mechanism. Shlex 1.3 local versus target 2.0 is source-supported for used try_quote/split: cached 2.0 changelog says deprecated quote/join removal and removal of Shlex's unsound DerefMut; this closure uses neither removed interface. Full dependency-version parity is not asserted.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [import source selections](zed-gpui-import.md) and [independent consumer](../verification.md#independent-dependency-consumer). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### D06

**Root dependency and toolchain source.**

Upstream toolchain 1.97 -> 1.98.1 is intentionally not the extracted workspace compiler contract. Existing Rust 1.93 qualification and stable first_chunk / Path::as_os_str adaptations preserve the owning fork floor. No build was run by this audit.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [import source selections](zed-gpui-import.md) and [independent consumer](../verification.md#independent-dependency-consumer). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### D07

**Root dependency and toolchain source.**

**Selected root integration complete.** Actual UIKit features, inherited dependency keys and selector member/feature forwarding are present. Root and new iOS-independent locks include the required native dependency owners. F-IOS passes simulator all-target/all-feature and device lib/all-feature compilation; the independently locked simulator consumer and real 1.93 simulator F-MSRV also pass. Local SHA mapping: L-IOS/L-DEPS. This prerequisite is part of the selected framework closure.

Verification owner: [import source selections](zed-gpui-import.md) and [independent consumer](../verification.md#independent-dependency-consumer). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### D-PRODUCT

**Individually inspected root-only product/tooling slices.**

The support audit inspected all 149 root commits and separated 33 directly relevant support selections from 116 residual entries. Residual is not synonymous with product. A row uses this group only where its actual parsed before/after root delta is limited to the shown product dependency/patch keys or excluded workspace members, and those keys are not inherited by the selected target crate manifests. Explicit support exclusions include notify, tree-sitter, LiveKit, crash-context, scratch and cpal patches. Product ACP/agent/CLI/editor/audio/LSP selections are recorded by their actual changed keys, not their titles. The row lists that root evidence even when no framework source changes. 83 of the 116 residual root entries meet this narrow source test, including the nine additional source-inspected entries in [D-ROOT-PRODUCT](#d-root-product); mixed framework contributions keep their other groups. Independent Apache owners such as watch/proxy_handshake are excluded here because no selected source consumes them, not because of their license.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [extraction boundary](../adr/0001-open-gpui-fork-strategy.md) and the per-row official source link. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### D-SOURCE

**Residual root slices with an explicit owning source audit.**

These residual entries are closed only for the concrete root contribution named in their row and its linked Core/platform/Windows/support owner. Evidence includes the exact final WGPU/Naga source after the registry intermediate, source-confirmed image/SVG/accesskit features, recorded Windows adapter/registry package-version differences, github_releases ownership, Web feature forwarding and the selected iOS example conversion. The product crate rename/member order remains outside scoped runtime. No entire third-party release is called equivalent merely because used methods compile.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [candidate source/dependency dispositions](zed-gpui-upstream-sync-candidates.md) and [dependency consumer](../verification.md#independent-dependency-consumer). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### D-ROOT30

**Completed source audit of the former 30 unresolved root slices.**

Each of the 30 introduction commits was inspected with its complete changed-path list and actual `Cargo.toml` / `rust-toolchain.toml` patch. The decisions below use the current manifest declarations, applicable baseline source, selected owners' real calls and independently locked package sources. None of these 30 changes modifies `rust-toolchain.toml`; the existing compiler-floor decisions remain in D06. No Cargo command was run for this audit.

The historical root-slice decisions were **17 adopted, 3 retained-contract, 9 product-only and 1 real-missing**. The exact EGL correction closes the last missing source slice, giving **18 adopted, 3 retained-contract and 9 product-only**. These are different from aggregate row states: c214057e0865's benchmark compiles in F-BENCH; 1808df02c0ff's benchmark/iOS slices compile in F-BENCH/F-IOS; 956a49e4ca8a retains its reconciled Core/Web slices despite its product-only root change. These 30 rows now comprise **19 source-reconciled, 3 implemented-local-verified and 8 outside-framework**. Each full SHA remains only in its canonical row.

The owning evidence is split across [AppKit features](#d-appkit), [Apple package/extraction choices](#d-apple-packages), [dependency cleanup](#d-extract), [product/tooling keys](#d-root-product), [text/layout prerequisites](#d-text), [Linux keyring](#d-keyring) and the [EGL repair](#d-egl). There are no unexplained root decisions. The matrix supplies current scoped build/test evidence; native execution and whole dependency-release parity are separate claims.

### D-APPKIT

**Nine adopted root binding-feature changes, relocated to the macOS package.**

The root `objc2-app-kit` declaration uses 0.3 with default features disabled and NSGraphics. [The macOS manifest](../../crates/gpui_macos/Cargo.toml) supplies the full per-package feature union explicitly, including block2; the root and independent-consumer locks select the typed 0.3.2 binding owner. Root feature placement alone would miss this extracted owner. The actual routes at the baseline are:

- 0a9eb81e6ced adds NSEvent: `events.rs` reads typed event/key-code data and converts optional native character strings before creating framework input.
- 10cfb459449a removes direct cocoa/objc keys and adds NSColor, NSDragging and NSVisualEffectView: `window.rs` uses typed color/appearance/view construction, while `window/file_drag.rs` owns typed native drag initiation. No selected direct cocoa/objc declaration survives; unrelated legacy transitive copies are not claimed removed.
- 72c53bf0aae2 adds NSDraggingItem, NSImage and NSPasteboard: `window/file_drag.rs` builds typed dragging items and a pasteboard writer, resolves a UTType for the path, and begins the native drag session.
- 86b2cf96623a adds NSScreen and the ScreenCaptureKit dependency: `screen_capture.rs::get_sources` uses typed SCShareableContent completion, validates the native result/error and returns foreground display/window capture sources; it does not substitute an inventory symbol for the callback route.
- 907b55f7eabe adds NSWorkspace: `platform.rs::register_url_scheme` uses the typed URL-registration completion block, oneshot result and background executor.
- a60addb9a96b adds NSAlert, NSApplication and block2: `window.rs` creates the alert and uses the explicit sheet/completion owner for prompts.
- d3865b099eae adds NSOpenPanel, NSPanel and NSSavePanel: `platform.rs` selects native open/save panels with the existing typed completion and foreground result delivery.
- db7f9ceed788 adds NSTrackingArea: normal windows disable acceptsMouseMovedEvents and install an area with MouseEnteredAndExited, MouseMoved, ActiveAlways and InVisibleRect; floating windows keep their native mouse-move setting. The actual creation branch calls `add_mouse_tracking_area`.
- f14592c4a5bd adds the UTI std/UTCoreTypes/UTType features: the macOS member supplies those exact features on its 0.3 declaration; the locks resolve 0.3.2. The file-drag route calls typeWithFilenameExtension and falls back to the folder/data types.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, source-reconciled only. Current unrelated hidden-activation edits do not supply this baseline claim. Native OS/SDK floors, dialogs, drag, capture and tracking retain [P07](#p07), [P08](#p08), [P17](#p17) and their existing verification limits. No new execution or platform completion is granted.

### D-APPLE-PACKAGES

**Two adopted package decisions and two retained extraction contracts.**

b89471c52d22 removes direct `block = "0.1"`: the current root and Apple owners have no direct block declaration. Apple completion blocks use block2 0.6.2/RcBlock through actual prompt/capture/URL routes. This adopts the direct-owner cleanup without claiming that every transitive block package disappeared.

53a3874a32ac relaxes `core-foundation = "=0.10.0"` to a compatible 0.10 requirement. The fork's `"0.10.0"` is a caret requirement, not an exact pin: the root lock selects 0.10.0 while the independently locked consumer actually resolves 0.10.1. The Apple CoreFoundation owner inherits this declaration; the separate legacy 0.9.4 transitive copy does not reinstate an exact pin.

1808df02c0ff removes metal 0.33 and advances official scap to b5c86ec58dd4b6b7732bf6ac8be03d6b1f190a82, whose optional macOS dependencies remove the old block tree. The fork has removed the direct metal crate and uses objc2-metal. It deliberately retains **open-gpui-scap 0.1.0-beta.1**, default-features=false, under the [existing owned-dependency selection](zed-gpui-upstream-sync-candidates.md#selection-rules), [ADR 0001](../adr/0001-open-gpui-fork-strategy.md) and the [Mac capture successor contract](zed-gpui-import.md#november-11-mac-appkit-and-capture-binding-successor). The latter preserves other backends' capture owners and excludes unrelated legacy transitive bindings from the Apple replacement. `crates/gpui/Cargo.toml` gates scap to Linux/FreeBSD/Windows, and `platform.rs` gates `scap_screen_capture` off Apple; Apple uses native ScreenCaptureKit and its CoreVideo pixel buffer. The owned scap manifest still declares legacy macOS cocoa/objc dependencies, and the locks still contain block/cocoa. This is an explicit **retained-contract** difference, not adoption of the official scap revision or proof of a block-free whole lock graph. The same commit's atlas_upload/iOS slices are implemented and compile in F-BENCH/F-IOS through L-BENCH/L-IOS; native measurement and UIKit execution retain separate qualification.

2e2fb0a21811 moves block/cbindgen/core-graphics/core-text/etagere/foreign-types/pathfinder_geometry/swash declarations to the monorepo root; it has no new renderer algorithm. The extracted owners retain explicit per-package declarations: Apple cbindgen 0.29 (official intermediate 0.28) generates the Scene header consumed by Metal compilation; core-graphics 0.24, optional font-kit core-text 21, foreign-types 0.5 and pathfinder_geometry 0.5 serve the actual native geometry/font owner. Typed block2 supersedes old Apple block. The shared atlas uses etagere 0.3 versus official 0.2, under its existing AtlasState/lease contract. Linux and WGPU request swash 0.2.6; the root lock resolves **0.2.9** and the independent consumer **0.2.10**, rather than both resolving 0.2.6. This is a **retained-contract** declaration/extraction decision; no equality of complete third-party releases is asserted.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`** for the existing package/owner mechanisms. Verification belongs to P08/P17, shared atlas/font owners and the [independent consumer](../verification.md#independent-dependency-consumer). Current atlas_upload/iOS source maps to L-BENCH/L-IOS and compiles in F-BENCH/F-IOS; native timing/input remains unqualified.

### D-EXTRACT

**Adopted dependency cleanup with retained standalone test/tooling policy.**

98f75a23bc40 replaces the root crossbeam alias with crossbeam-queue 0.3.12 and removes json_dotpath, portable-pty and take-until. Its relevant source hunks remove unused x11-clipboard from the Linux manifest and remove the take_until dependency/reexport from util. The current selected manifests and util source contain none of those removed direct declarations/reexports. The audio/terminal/json consumers are product owners; adding a new direct crossbeam-queue dependency to GPUI would not adopt their changes. The root/independent locks already include crossbeam-queue 0.3.12/0.3.14 transitively through postage. This is **adopted** scoped cleanup, not an assertion that every package is absent transitively.

282f47a544b7 removes unused root zed/brotli/cfg-if/cocoa-foundation/futures-util/pathdiff aliases and switches monorepo dependency-lint metadata from machete to shear. All six direct root aliases are absent. The fork retains the existing standalone test/doc policy and cargo-machete metadata described in [H16](#h16); `crates/sum_tree/Cargo.toml` retains its explicit cargo-machete tracing exception. Fork headless/browser owners actually consume pollster/wasm dependencies, so those are retained. This is a **retained-contract** tooling/extraction decision, not missing runtime source or proof that the removed aliases' transitive packages vanished.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, source-reconciled only. Verification remains with H09/H16, P24 and the independent consumer's documented dependency/test scope; this ledger grants no fresh package or lint-tool execution.

### D-ROOT-PRODUCT

**Nine additional root product/tooling decisions, established from actual consumers.**

These are narrow root-slice **product-only** decisions. Selected manifests and source were checked for direct inheritance/use; lock presence alone was not used as adoption evidence. Their scope is established by the real changed consumers:

- 9ee3c503a4bb adds futures-util 0.3.32 for extension_builder, extension_cli and the extension-compilation benchmark. No selected owner adds that direct key/use. Existing futures/HTTP/keyring dependencies still bring futures-util 0.3.32 in the root and 0.3.34 in the independent lock; that transitive package is not rejected.
- 2df74932bc66 adds mime 0.3.17 for acp_thread embedded resources and agent conversation UI. There is no selected direct mime key/use; MIME remains available transitively through the HTTP/mime_guess owner. This is not a missing framework HTTP MIME implementation.
- 30730a305ae2 adds rustix 1.1/fs for the product `crates/fs` eager read_dir/closedir failure policy. No selected direct rustix key/use is introduced. Root transitive rustix 0.38.44 and 1.1.6 serve async-io/calloop/Wayland, not the absent product RealFs patch.
- a7d74150ac7a adds derive_more's `from` feature for settings/editor/gutter models. The actual patch has no selected owner hunk. Core uses its existing Add/Deref/FromStr/Sub family, not those new From derives; its existing derive_more features are retained.
- f9a1fc898bae adds arc-swap 1.9.0 for the new `crates/watch/src/snapshot.rs` implementation. Watch is independently Apache licensed, but no selected owner consumes the snapshot API or arc-swap and the root lock contains no arc-swap. Its exclusion is a source-closure choice, not a GPL classification.
- dd3a94e2e51f adds the proxy_handshake member/key and piper 0.2 while removing direct tokio-socks. The independently Apache proxy_handshake owner is consumed by the Zed client collaboration/proxy transport, not by the selected HTTP/native proxy owners. Existing piper 0.2.5 via blocking and tokio-socks 0.5.3 via the HTTP dependency remain legitimate transitive packages. The product transport extraction is not imported solely because it is independently licensed.
- 4b7369481dcf adds only root `[workspace.metadata.dylint]` discovery for `tooling/lints`. The actual implementation is a separate nightly plugin with UI fixtures, deliberately outside the upstream workspace; the metadata is not a GPUI runtime mechanism.
- 4c841aaf1c4f changes AWS config/Bedrock HTTPS defaults and wasmtime-wasi p2/default features. Its consumers are context-server/project/LiveKit/xtask product services. No selected manifest/source uses these direct keys, and the current root/independent locks contain none of those three packages. No framework TLS/rendering capability is removed by excluding this product feature trim.
- 956a49e4ca8a only changes root settings_json to default-features=false. That GPL product settings owner is not consumed by the selected closure. Its **separate** Core/Web touch and gesture source contributions remain adopted in C-I/P21; this mixed SHA is therefore source-reconciled, not outside-framework as a whole.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`** for the existing extraction boundary; these decisions add no implementation or execution claim. The official per-row patch and [ADR 0001](../adr/0001-open-gpui-fork-strategy.md) own provenance/scope. No product-only title inference is used, and excluded product behavior receives no framework test requirement.

### D-TEXT

**Four adopted root text/layout prerequisites with actual source consumers.**

d71f1461045c upgrades stacksafe from 0.1 to 1.0. The current root declaration/lock use 1.0.2 and Core selects it through the optional stacker feature. The real `taffy.rs` StackSafe measurement callbacks and compute_layout wrapper, Div phase callbacks and randomized-tree owner consume that mechanism. This is an adopted prerequisite, not unused version/name matching.

b488c4e81451 upgrades the official exact Taffy pin from 0.13.0 to 0.15.0. The extracted Core owner requests compatible 0.15, default-features=false, with explicit std/taffy_tree/flexbox/grid/block_layout/float_layout/content_size features. Root and independent locks select 0.15.0; the Web smoke lock selects 0.15.1. `taffy.rs` creates measured leaves with NodeContext and runs the complete compute_layout_with_measure/compute_leaf_layout path with device-scale conversion and zero baseline handling; `style.rs` maps Visible/Clip/Hidden/Scroll one-to-one. The complete current compute_layout function matches final official source apart from three blank lines. [U-TAFFY](#u-taffy) records the prior historical pin's supersession and actual local adoption revision. The compatible requirement is a retained library packaging choice, not an exact pin or proof of every possible future release.

c214057e0865 adds unicode-bidi 0.3.18 with default-features=false/hardcoded-data. The fork's 0.3.18 default feature set supplies std and hardcoded-data, and the inspected locks select 0.3.18. The actual Cosmic owner uses bidi_class==BidiClass::B for separator detection, paragraph splitting, exact FontRunCursor partitioning and source-index/x rebasing. This root prerequisite is adopted. Its normalized corpus and mixed_direction_paragraphs benchmark are complete in [G-GW2](#g-gw2) and compile in F-BENCH; the aggregate row is implemented-local-verified through L-BENCH.

f0210f1621c6 adds ICU segmenter 2.3.0, default-features=false/compiled_data. The fork uses the same declaration and `text_system/line_wrapper.rs` constructs LineSegmenter::new_for_non_complex_scripts and segments the source string rather than relying on an old generic break predicate. The root lock selects 2.3.0; the independent/smoke locks select compatible 2.4.0. No equivalence of entire ICU releases, complex-script coverage or numerical performance is asserted.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, source-reconciled for these prerequisites. Verification belongs to [C-T](#c-t), its layout/wrapping regressions and the independent consumer. Current Cosmic corpus/offset changes map to L-BENCH/L-GPU and pass their scoped F-ALL/F-BENCH gates; these results do not establish complete dependency-release parity or numerical benchmark qualification.

### D-KEYRING

**Adopted exact Linux oo7 prerequisite.**

91d3b962ae32 replaces registry oo7 with the official 0.6 Git source **731c1215edb6905876c6137ab300428c7b8d7c13**. The root declaration and root/independent-consumer/Web-smoke locks all select that exact source. The Linux member enables async-std/native_crypto, and its platform credential owner calls new/create_item/search_items/delete through this oo7 owner. This carries the intended stream-termination error handling dependency into the actual selected Secret Service route, rather than merely declaring an unused key.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, source-reconciled only. Verification belongs to the Linux credential owner in [P17](#p17), [P24](#p24) and the [independent consumer](../verification.md#independent-dependency-consumer). No Secret Service runtime or fresh Linux build was executed here.

### D-EGL

**Exact EGL dependency correction — implemented and locally verified.**

19c69782eb3c adds the root khronos-egl patch at **[42214b36347b108fd9630ca6254b96dde59e04fe](https://github.com/zed-industries/khronos-egl/commit/42214b36347b108fd9630ca6254b96dde59e04fe)** for failed EGL calls that report EGL_SUCCESS. The historical source audit found only registry 6.0.0 at the root, independent consumer and Web smoke; that was the sole open source slice. Current applicable roots and locks select the exact official Git correction.

The actual registry 6.0.0 source confirms the gap: `Instance::initialize` returns `Err(self.get_error().unwrap())` after a failed native call, while `get_error` returns None for EGL_SUCCESS. `crates/gpui_wgpu/Cargo.toml` enables GLES for Linux/FreeBSD; the exact pinned WGPU cddb2f5d80dc0009b2ef6767635140cacf90a007 `wgpu-hal/src/gles/egl.rs::initialize_display` calls `egl.initialize(display)`, and its context path also uses binding failure wrappers. The real selected call chain is therefore exposed to the missing correction. This is not inferred from a different dependency name or an unrelated product title.

Current `get_call_error()` reads the native error once and uses `Error::Unreported` for failed/EGL_SUCCESS; all 39 former failed-call `get_error().unwrap()` branches use it, while successful branches avoid querying the error. Real EGL errors and the production `extern "system"` signatures are preserved. Manifest/dependencies/features/build/license texts are unchanged from the pin's registry-identical parent. The fixture record includes the full wrapper, ABI and downstream WGPU audit.

The exact patch is present at the workspace root, independent dependency consumer, smoke-Web and legacy hello-Web roots, and the new iOS consumer root. Their applicable lock entries resolve Git revision 42214b36347b108fd9630ca6254b96dde59e04fe. The legacy ignored hello-Web lock was regenerated locally from its old 0.1/WGPU-29 graph and is not a tracked/published lock artifact. Core-only roots with no EGL edge intentionally receive no unused patch. Libraries do not export root patch settings; each real downstream root owns its selection. Main's online targeted refresh preserves earlier package versions after rejecting unrelated offline resolver downgrades.

Executed F-EGL uses the real pinned crate: upstream wrapper tests **8/8**, plus public `DynamicInstance<EGL1_4>`/1.5-upcast tests **3/3** through the production dynamic loader and native fixture symbols. Failed/SUCCESS, genuine errors and success/no-error-query controls are exercised for initialize/window-surface/platform-window-surface. Identical source against registry 6.0.0 passes **2/3** and actually panics in `Instance::initialize` at `unwrap(None)`. Exact commands and permanent source/license/ABI evidence are in the [fixture record](../../tests/fixtures/egl-errors/README.md).

Disposition: **implemented-local-verified**; local SHA mapping: **L-DEPS**. The missing source slice is closed by the exact source/lock selection and executed positive/negative controls, not by assignment or a title. Linux/FreeBSD compositor/GLES driver and Windows ANGLE execution remain unqualified. F-WORKSPACE/F-PUBLIC/F-WASM provide their own scoped integration compilation evidence.

### W-WINDOWS

**Windows native services, DirectWrite and DirectX source audit.**

The complete 81-entry Windows/WGPU history was read, with per-row surviving mechanisms and final-owner evidence. Windows rows describe actual native command/ingress, input/IME, capabilities, UTF-16 text/DirectWrite layout, composition, dialogs/credentials, display/visibility/power, atlas/resource and shader paths. Scene/refusal, exact registrations, HWND generations and shutdown custody remain fork contracts. ExitProcess is intentionally retained only for standalone production exit after terminal convergence; ExternalHost/returning tests return, as [documented](../verification.md#native-quit-completion-and-restart-manager). Existing CPU-COLR supersession and GW3–GW5 gaps are explicit row overrides.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [native text](../verification.md#native-text-layout-and-wrapped-hit-testing), [Windows safety](../verification.md#windows-dialog-threads-and-native-host-safety), [native state](../verification.md#native-window-state-and-attention). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### W-WGPU

**WGPU, packed WebGL and Cosmic source audit.**

Rows preserve final source for paragraph splitting/rebasing, font match/install/inventory/raster reuse and fallback-face retention; native/storage and WebGL transports/decoders, actual device feature selection, error generations, optional headless no-readback rendering, initial configure-once and later surface recreation; atlas binding caches key exact texture instances and prune against admitted receipts. WGPU retains host encoder/callback/submission authority and recording resources through real GPU completion. Cosmic sorting, benchmark corpus and mark offsets have separate current groups; source adoption is not a numerical performance result.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [Cosmic paragraphs](../verification.md#cosmic-paragraph-shaping-and-public-paint), [WGPU/WebGL](../verification.md#wgpu-dependency-shaders-and-webgl), [atlas](../verification.md#shared-atlas-ownership-and-retained-renderer-bindings). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### W-SHARED

**Twelve supplemental shared-render and dependency source entries.**

These per-row source notes retain root prerequisites and shared owners absent from the Windows/WGPU primary path history. They cover JobObjects, accesskit/enumn, image/SVG dependency policy, Win32 Power, the superseded registry WGPU selection, windows-registry version differences, product member ordering, retained shared Apple/CoreVideo surface ownership and PrimitiveTransform cleanup. Local accesskit_windows 0.32.1 versus target 0.34, windows-registry 0.5 versus target 0.6 and etagere 0.3 versus target 0.2 are visible dependency differences, not proof of whole-release equality.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [import selections](zed-gpui-import.md) and [existing dependency qualification](../verification.md#independent-dependency-consumer). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### W-PATHS

**Final Windows/WGPU source-path reconciliation.**

The GPU audit covers 99 distinct commits (81 primary, six rendering, 12 supplemental) and all 37 final reference paths (25 Windows, 12 WGPU). This path inventory adds no commit row. GW omissions describe the historical audit snapshot; all five current corrections are implemented. Host WGPU/portable tests pass F-ALL, and native Windows sources/tests cross-compile in real-sysroot F-WINDOWS. Paths use the W/G aliases defined for the canonical ledger; current hashes map to L-GPU/L-BENCH.

| Final reference path | Actual final-owner source evidence |
| --- | --- |
| `W/Cargo.toml` | Required native bindings and test/bench/capture gates exist; extracted aliases, standalone test defaults and explicit dependency-version differences are recorded. |
| `W/LICENSE-APACHE` | Extracted regular Apache license text replaces monorepo license symlink. |
| `W/build.rs` | DirectX quad/opaque/path-tile/shadow/text entries compile through existing release FXC owner; CPU COLR removes its obsolete extra shader module. Default Cargo rerun policy remains broader than upstream's explicit build.rs hint. |
| `W/src/alpha_correction.hlsl` | Exact unchanged source; CPU helper mirrors the same contrast/gamma equations. |
| `W/src/clipboard.rs` | Native image/text/metadata formats retained; embedded NUL normalization/hash is adopted. Optional format registration adds qualified graceful failure behavior. |
| `W/src/destination_list.rs` | Exact unchanged native JumpList owner. |
| `W/src/dialog.rs` | Complete final official worker/STA/serialization/cancellation implementation; native retirement adds try_lock_idle, with when_idle retained for its tests. |
| `W/src/direct_manipulation.rs` | Complete pointer/viewport/scroll producer retained; explicit stopped/removed/abandoned/deactivated retirement state strengthens native lifetime without replacing its physics. |
| `W/src/direct_write.rs` | Full final shaping/typography/tab/separator/CPU color mechanism present, around documented font-generation/native-face fixes. Current Windows runtime qualification remains main-thread work. |
| `W/src/directx_atlas.rs` | AtlasState, checked admission, exact instances, leases and retained published-region policy reconciled. Missing texture is fallible rather than panic or partial accepted paint. |
| `W/src/directx_devices.rs` | Target device/adapter creation retained; extracted utility path and standalone GPU ownership remain. No missing device feature was found. |
| `W/src/directx_renderer.rs` | Whole-view/batch-offset uploads, source-over blend, native child layer setting and all six final rendering mechanisms present; text-specific GPU color path removed. Exact preflight/shutdown remain fork owners. |
| `W/src/dispatcher.rs` | Final Win32 pool/timer implementation, raw-runnable ownership and priorities present; relative delays round up and avoid zero absolute FILETIME, callback timing/unwind and foreground wake identity remain qualified adaptations. |
| `W/src/display.rs` | Monitor identity, work area, per-monitor scale and refresh interval survive in immutable topology/snapshot owners. |
| `W/src/events.rs` | Complete changed native input/display/frame/foreign-message routes reconciled. GW3/GW4 isolate the surviving capability/IME omissions. |
| `W/src/gpui_windows.rs` | Complete module wiring plus standalone target gates, test exposure and split native owner exports. |
| `W/src/keyboard.rs` | Production source matches after extracted collection alias. |
| `W/src/platform.rs` | Menus, notifications, credentials, broadcasts, session shutdown, mode selection, power and renderer/device owners reconciled. Standalone ExitProcess remains an explicitly recorded intentional policy. |
| `W/src/shaders.hlsl` | Corrected transfer directions and sprite ABI plus geometry/pixel/path/batching/depth/Levien successors; exact clip stack and local transform adaptations retained. |
| `W/src/system_notifications.rs` | Complete target notification creation/delivery plus explicit callback/WinRT retirement and test seams. |
| `W/src/system_settings.rs` | Production source matches after extracted utility alias. |
| `W/src/util.rs` | Native system/cursor helpers retained; product-neutral credential namespace and locally acquired UISettings factory are documented fork corrections. |
| `W/src/vsync.rs` | Final target source matches after extracted aliases. |
| `W/src/window.rs` | Native creation, physical placement, visibility, capture, presentation and activation use canonical fork authorities. Historical GW5 PopUp default is now implemented; GW3 capability checks belong to events/caption actions. F-WINDOWS compiles both. |
| `W/src/wrapper.rs` | Exact unchanged native wrapper owner. |
| `G/Cargo.toml` | Exact final WGPU/Naga source, Cosmic/Swash/Unicode and native/WebGL/optional-image feature requirements; standalone aliases/defaults and font-kit source policy retained. |
| `G/LICENSE-APACHE` | Extracted regular Apache license text replaces monorepo license symlink. |
| `G/benches/layout_line.rs` | Env/file font discovery retained; historical GW2 corpus/paragraph omissions now adopted and compiled in F-BENCH. |
| `G/src/cosmic_text_system.rs` | Complete paragraph/font/install/prewarm/raster/selected-face/missing-report/mark-offset paths and bounded scratch cache. GW1 unstable-sort correction adopted; host tests pass F-ALL. |
| `G/src/gpui_wgpu.rs` | Final public context/renderer/headless exports plus fork retained atlas/image/recording interfaces are wired. |
| `G/src/shaders.wgsl` | All six later mechanisms and the now-present ordinary sprite fast branch; exact clips, local transforms, imported flags and working-domain conversion retained. |
| `G/src/shaders_storage.wgsl` | All current primitive loaders, analytic tile/curve/paint storage bindings and the additional exact-clip arena are present. |
| `G/src/shaders_subpixel.wgsl` | Colocated sprite ABI, native dual-source entry points and original gamma correction survive; clipping and scene-color conversion follow the shared fork owners. |
| `G/src/shaders_webgl.wgsl` | Every current packed primitive and analytic-path record decoder present, including fixed quad texel loading and odd-record alignment. Fork clip/transform/instance-generation additions are included. |
| `G/src/wgpu_atlas.rs` | Complete final AtlasState, upload, swizzle, generation/cache and device-loss mechanisms through retained receipts/leases and imported-image ownership; split owner is `wgpu_atlas/imported_image.rs`. |
| `G/src/wgpu_context.rs` | Full native/browser/headless adapter/device/error and final configure-once mechanisms. Browser transient support check moved to adapter_supports_transient_attachments for host recording reuse. |
| `G/src/wgpu_renderer.rs` | Final renderer, headless render/readback, storage/WebGL transport, surface lifecycle and all six rendering successors. Split owners are gpu_data, scene_renderer and surface_lifecycle; resource/callback/submission authority remains explicit. |

The matrix records current F-ALL shader/text controls and F-WINDOWS/F-BENCH compilation. Source comparison alone supplies no execution credit for native IME, PopUp layering, Windows text readback or numerical benchmarks. L-GPU/L-BENCH map the current implementation hashes for main to fill.

### W-RENDER

**Windows/WGPU final six-patch algorithm reconciliation.**

The final source audit follows actual vertex clipping/fallback and exact fragment clips, ordinary sprite early return, analytic quadratic tiles/curves/paint through storage and WebGL decoders/HLSL slots, kind-aware actual-bounds batching, private depth recreation and host retention, and the Levien shadow implementation. It confirms integration paths rather than matching function names. The current ordinary sprite branch passes vertex opacity/clip envelope and applies scene color conversion before returning ahead of load_poly_sprite; imported pad!=0, grayscale and rounded cases retain the slow branch. L-GPU maps this current correction, whose shader/image controls pass F-ALL. Source inspection alone adds no native/pixel/performance receipt.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [rendering successor](../verification.md#november-12-rendering-successor) and the current platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-M1

**Historical Core source finding M1 (P2).**

- Official: `8c7811ea7291`, `crates/gpui/src/keymap.rs:227` at fixed target (except later rendering snapshot where stated).
- Baseline fork: `crates/gpui/src/keymap.rs:193`.
- Base-keymap null does not suppress equal/weaker binding sources. The fork retains the old continue branch for non-user NoAction. Preserve source meta order, context depth and same-depth load order. Carry official source-precedence regressions.

Current implementation: `keymap.rs` tracks the effective `no_action_meta` and suppresses equal/weaker sources after depth/load ordering. Existing resolver and source-precedence regressions pass F-ALL. Local SHA mapping: L-CORE.

Verification owner: [current affected Core/public consumer gates](../verification.md#independent-dependency-consumer) and platform-matrix scenario. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-M2

**Historical Core source finding M2 (P2).**

- Official: `71456c40f397`, `crates/gpui/src/keymap.rs:285` at fixed target (except later rendering snapshot where stated).
- Baseline fork: `crates/gpui/src/keymap.rs:252`.
- Possible-next bindings can advertise a chord that full dispatch disables using null or Unbind. Final official resolver replays each complete continuation with disabling bindings retained. Adapt existing resolver, including typed-character versus physical-key matching.

Current implementation: `possible_next_bindings_for_input` replays complete continuations through `resolve_bindings_for_input` while retaining disabling entries. Dispatch-winner and logical/physical matching regressions pass F-ALL. Local SHA mapping: L-CORE.

Verification owner: [current affected Core/public consumer gates](../verification.md#independent-dependency-consumer) and platform-matrix scenario. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-M3

**Historical Core source finding M3 (P2).**

- Official: `a496bf0ff6c0`, `crates/gpui/src/elements/div.rs:2527` at fixed target (except later rendering snapshot where stated).
- Baseline fork: `crates/gpui/src/elements/div.rs:3506`.
- A Div with only drag_over styling receives no hitbox: the style builder at :1537 only pushes drag_over_styles and should_insert_hitbox omits that condition. The source fix is not cosmetic product work. Preserve candidate, transformed geometry and exact clipping; carry a standalone style-only target regression.

Current implementation: `should_insert_hitbox` includes nonempty `drag_over_styles`; the style-only target participates in the same admitted transformed/exact-clip hitbox path. Affected regression passes F-ALL. Local SHA mapping: L-CORE.

Verification owner: [current affected Core/public consumer gates](../verification.md#independent-dependency-consumer) and platform-matrix scenario. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-M4

**Historical Core source finding M4 (P2).**

- Official: `961f4f202465`, `crates/gpui/src/window.rs:3088` at fixed target (except later rendering snapshot where stated).
- Baseline fork: `crates/gpui/src/window.rs:6150`.
- Bounds changes refresh size/scale facts but leave cached mouse_position stale. Native Resized/Moved/WindowStateChanged all reach this owner via app/native_event_ingress.rs:1799; commit_platform_facts at :6162 cannot refresh pointer position because WindowPlatformFacts has no pointer field. Query the existing platform mouse-position owner at the admitted native-delivery boundary; respect active captured-pointer geometry and quiescence. No backend changes are prescribed by this lane.
- Implementation follow-up: `Window::bounds_changed` now samples `PlatformWindow::mouse_position` after coherent platform facts are refreshed, before bounds observers. It retains post-borrow native ingress and skips sampling while interaction is quiesced, a pressed/captured/drag pointer session is active, or this window owns a native captured drag. New actual-ingress and admitted-capture tests are in `window/upstream_api_tests.rs`. Source implemented; affected ingress/capture tests pass in F-ALL.

Current implementation: complete under **L-WINDOW**. F-ALL passes the affected Window/test-context regressions; F-PUBLIC compiles the ordinary public core_window_api consumer.

Verification owner: [current affected Core/public consumer gates](../verification.md#independent-dependency-consumer) and platform-matrix scenario. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-M5

**Historical Core source finding M5 (P2/P3).**

- Official: `506beb34de3f`, `crates/gpui/src/elements/div.rs:908` at fixed target (except later rendering snapshot where stated).
- Baseline fork: `crates/gpui/src/elements/div.rs:1126`.
- Release + test-support still executes debug_selector closures and records maps/journal entries. The official debug_assertions gate is missing in builder, field and recording paths (:3044/:3657). The fork docs describe selectors as test-only observations, not production authority. Preserve admitted-frame replay and rollback when applying the gate.

Current builder, field and recording gates require `debug_assertions` with the existing observation feature scope. Host affected regressions pass F-ALL; no separate release-mode execution receipt is supplied. Local SHA mapping: L-CORE.

Verification owner: [current affected Core/public consumer gates](../verification.md#independent-dependency-consumer) and platform-matrix scenario. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-M6

**Historical Core source finding M6 (P3).**

- Official: `e9ddcdaf9829`, `crates/gpui/src/window.rs:8041` at fixed target (except later rendering snapshot where stated).
- Baseline fork: `crates/gpui/src/window.rs:15368`.
- ElementId lacks From<u64>; From<usize>, From<i32> and named tuples are not equivalent for generic id(42_u64) consumers. Additive API gap.
- Implementation follow-up: official `From<u64>` is added directly to `window.rs`, preserving every bit in `ElementId::Integer`. The independent consumer `tests/fixtures/dependency-consumer/tests/core_window_api.rs` exercises IDs above 32 bits, `u64::MAX`, and the actual `div().id(u64)` builder. Source implemented; F-ALL/F-PUBLIC provide the scoped current evidence.

Current implementation: complete under **L-WINDOW**. F-ALL passes the affected Window/test-context regressions; F-PUBLIC compiles the ordinary public core_window_api consumer.

Verification owner: [current affected Core/public consumer gates](../verification.md#independent-dependency-consumer) and platform-matrix scenario. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-M7

**Historical Core source finding M7 (P3).**

- Official: `63b52a7ee354`, `crates/gpui/src/color.rs:424 and :464` at fixed target (except later rendering snapshot where stated).
- Baseline fork: `crates/gpui/src/color.rs:362 and :402`.
- hsla and opaque_grey remain ordinary functions; official const construction is unavailable. Runtime bodies otherwise match.

Current implementation: complete under **L-CORE**; affected source and regressions pass F-ALL/F-PUBLIC compilation. The earlier fixture failures are historical and preserved in the matrix.

Verification owner: [current affected Core/public consumer gates](../verification.md#independent-dependency-consumer) and platform-matrix scenario. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-M8

**Historical Core source finding M8 (P3).**

- Official: `547a6cd599a1; b95b188b5945`, `crates/gpui/src/window.rs:3426; app/test_app.rs:487; app/test_context.rs:532/:1034` at fixed target (except later rendering snapshot where stated).
- Baseline fork: `crates/gpui/src/window.rs:7256; app/test_app.rs; app/test_context.rs:1090`.
- Public Window::set_scale_factor and test-context/window scale-factor simulation are missing. Fork set_platform_window_physical_client_geometry updates the lower-level native physical frame and does not implement the complete official logical-bounds-preserving callback/pump APIs. Adapt coherent physical geometry rather than installing an inconsistent cached scale.
- Implementation follow-up: all four official simulation surfaces now delegate to `TestWindow::simulate_scale_factor_change`; resize and scale simulation share one private geometry update. Logical bounds and physical desktop origin are preserved, existing physical client size is recomputed at the new scale, and physical display facts are updated before the resize callback is invoked outside the state lock. Unknown physical geometry remains unknown. The existing native ingress/pump delivers resize after App borrows. Finite positive scales are required before mutation.
- `Window::set_scale_factor` uses that same platform simulation and refreshes coherent Window facts immediately. Unlike the upstream cache-only override, it persists across a later resize. It explicitly requires a TestWindow; native windows continue to observe actual OS scale rather than receiving a cache-only fictional scale. Resize ingress still owns observer delivery and deferred-candidate cancellation. New context/VisualTestContext/TestAppWindow tests preserve upstream's 1.0/1.25/2.0 and subsequent-resize scenarios, plus physical geometry, busy-App ingress and invalid-scale controls. Source implemented; F-ALL/F-PUBLIC provide the scoped current evidence.

Current implementation: complete under **L-WINDOW**. F-ALL passes the affected Window/test-context regressions; F-PUBLIC compiles the ordinary public core_window_api consumer.

Verification owner: [current affected Core/public consumer gates](../verification.md#independent-dependency-consumer) and platform-matrix scenario. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-M9

**Historical Core source finding M9 (P3).**

- Official: `a473ea63a8bc`, `crates/gpui/src/elements/img.rs:578; platform.rs:3532; window.rs:5535` at fixed target (except later rendering snapshot where stated).
- Baseline fork: `crates/gpui/src/elements/img.rs:568; app.rs:3885; window.rs:6489`.
- Partial test-support asset inspection: App::has_asset exists; ImageSource::is_asset_cached, Image::is_asset_cached and Window::has_image_atlas_entry do not. Paint/access/removal diagnostics only report observed operations and are not a present-membership query after cache/atlas eviction. Native atlas backend work belongs to its owner.
- Implementation follow-up: `ImageSource::is_asset_cached` now queries the matching existing App asset owner without loading, and `Window::has_image_atlas_entry` requires at least one frame and actual present membership for every frame. `PlatformAtlas::contains` and Headless/Metal/WGPU/DirectX implementations already exist; no new trait/backend work is required. Tests cover pending, successfully decoded, cached-error and removed assets; Custom/Render observations stay false without calling loaders. Actual multi-frame paint/removal verifies per-window membership while accepted sprite leases remain alive.
- `Image` is owned by `crates/gpui/src/platform.rs`; there is no `src/image.rs`. Main supplied the official `Image::is_asset_cached(self: &Arc<Self>, cx: &App)` counterpart after the initial integration compile exposed its absence. The new asset test consumes that real method; the full facade/Window unit passes F-ALL and public-consumer compilation.

Current implementation: complete under **L-WINDOW**. F-ALL passes the affected Window/test-context regressions; F-PUBLIC compiles the ordinary public core_window_api consumer.

Verification owner: [current affected Core/public consumer gates](../verification.md#independent-dependency-consumer) and platform-matrix scenario. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-M10

**Historical Core source finding M10 (P3).**

- Official: `eb5483528be6; 0dea8c63d9a5`, `crates/gpui/src/window.rs:3199/:3205` at fixed target (except later rendering snapshot where stated).
- Baseline fork: `crates/gpui/src/window.rs:6446`.
- painted_quads and painted_underlines test-support accessors are absent. rendered_scene is bench-support only and is not available to ordinary test-support consumers. Retain accepted scene validity; exact clip diagnostics must not be weakened to satisfy source spelling.
- Implementation follow-up: both official test-support APIs read only `rendered_frame.scene`. A detached Scene clone is finished through the existing validity journal before returning its quads/underlines, preserving transforms and exact clip envelopes without mutating accepted state. Real candidate atlas refusal retains the preceding Accepted output; a late invalid transform scope is excluded while its valid sibling survives. No access to `next_frame` or test-only acceptance wrapper is added to production APIs. Source implemented; F-ALL/F-PUBLIC provide the scoped current evidence.

Current implementation: complete under **L-WINDOW**. F-ALL passes the affected Window/test-context regressions; F-PUBLIC compiles the ordinary public core_window_api consumer.

Verification owner: [current affected Core/public consumer gates](../verification.md#independent-dependency-consumer) and platform-matrix scenario. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-M11

**Historical Core source finding M11 (P3 API).**

- Official: `9c3273201ce6`, `crates/gpui/src/interactive.rs:275/:381` at fixed target (except later rendering snapshot where stated).
- Baseline fork: `crates/gpui/src/interactive.rs:273/:329`.
- Portable TouchClickEvent / ClickEvent::Touch and ClickEvent::is_secondary are absent. This is a public vocabulary gap. Both final dispatchers currently synthesize ordinary mouse events for recognized taps, so no claim is made that upstream already delivers Touch variants or fixes long-press aux-click dispatch.

Current implementation: complete under **L-CORE**; affected source and regressions pass F-ALL/F-PUBLIC compilation. The earlier fixture failures are historical and preserved in the matrix.

Verification owner: [current affected Core/public consumer gates](../verification.md#independent-dependency-consumer) and platform-matrix scenario. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-M12

**Historical Core source finding M12 (P3 API).**

- Official: `76b1096cbd83`, `crates/gpui/src/window.rs:6168` at fixed target (except later rendering snapshot where stated).
- Baseline fork: `crates/gpui/src/window/touch_gestures.rs:30`.
- Touch prediction state and setter exist, but the public passive touch_prediction_enabled getter is absent.
- Implementation follow-up: the passive public getter is added at the moved touch owner, `window/touch_gestures.rs`, beside the existing setter. A real Window test checks the default, toggles and absence of new frame wakes; the independent ordinary consumer checks both public method signatures. Source implemented; F-ALL/F-PUBLIC provide the scoped current evidence.

Current implementation: complete under **L-WINDOW**. F-ALL passes the affected Window/test-context regressions; F-PUBLIC compiles the ordinary public core_window_api consumer.

Verification owner: [current affected Core/public consumer gates](../verification.md#independent-dependency-consumer) and platform-matrix scenario. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-M13

**Historical Core source finding M13 (P3 harness).**

- Official: `26abd1c95908`, `crates/gpui/src/randomized_element_tree.rs:110` at fixed target (except later rendering snapshot where stated).
- Fork: `crates/gpui/src/randomized_element_tree.rs:59`.
- RandomizedElementTreeConfig::with_paths and its paths flag are absent although the path workload itself is painted at :1459. Official configurable workload selection is missing; the six-rendering-update snapshot retains it. This is Core benchmark harness scope, not a GPU algorithm gap.

Current implementation: paths/with_paths and associated workloads are complete, including all manifest targets and the 17-mutation ChildText chain. F-ALL exercises affected host controls and final F-BENCH compiles platform/Cosmic benches after that completion. Local SHA mapping: L-BENCH; no timing claim.

Verification owner: [current affected Core/public consumer gates](../verification.md#independent-dependency-consumer) and platform-matrix scenario. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-C1

**Shared prompt label.**

The surviving shared PromptButton conversion must normalize the default label to OK, rather than Ok. Product/example context does not exclude this portable facade change. Owner: crates/gpui/src/platform.rs.

Current implementation: complete under **L-CORE**; affected source and regressions pass F-ALL/F-PUBLIC compilation. The earlier fixture failures are historical and preserved in the matrix.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-C2

**Shared ImageFormat extension API.**

The product-labeled image paste commit has a portable ImageFormat::extension slice in crates/gpui/src/platform.rs. The baseline native clipboard owner did not supply this public API; current platform.rs implements the complete extension mapping.

Current implementation: complete under **L-CORE**; affected source and regressions pass F-ALL/F-PUBLIC compilation. The earlier fixture failures are historical and preserved in the matrix.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-IOS-CORE

**Selected iOS lifecycle and memory-pressure prerequisite.**

The 9c3273201c introduction and a47f721832 successor require AppLifecyclePhase including Disconnected, default Platform callback registration, observer storage, public App subscriptions and actual UIKit forwarding. Existing desktop sleep/wake is a different OS event. Adapt through AppCell ordered ingress/update transactions and subscription lifetime; do not copy direct native RefCell borrowing.

Current implementation: lifecycle/touch vocabulary and forwarding are complete, with actual TestPlatform lifecycle/memory callbacks and callback-replacement custody through AppCell ingress. F-ALL passes the portable lifecycle/observer tests; public existing `PlatformInputHandler::focus_id` is available across the backend boundary. All prior UIKit review findings are fixed and source re-review closed; F-IOS compiles native delivery. The failed first native probe and its fixture-only repair remain separate from these source/portable results. Local SHA mapping: L-CORE/L-IOS.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-BENCH

**Completed selected benchmark workload sources.**

Historical workload omissions were separate from adopted runtime Inspector/focus/typed Metal/tree behavior. Current sources include b0f53 inspector_render, e463c5 focus_cleanup, 1808df atlas_upload and 26abd1 extended non-tree/primitives/two-revision comparison. Platform manifests register the actual targets with bench-support; native factories/teardown, five additional workloads, with_paths and comparison tooling are implemented. `randomized_element_tree.rs` carries ChildText through the complete 17-way kind/enum/generation/mutation/state/render path; independent source review closes that chain.

Final F-BENCH checks platform/WGPU benches after ChildText completion, with `bench-support,font-kit,inspector,runtime_shaders`, and passes. Python comparison fixture-path controls pass 5/5. Native workload timings, two-revision numerical comparison and Windows process cleanup have no execution receipt. Local SHA mapping: L-BENCH; no speedup claim.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-GW1

**Cosmic unstable-sort adoption.**

78658778 changes all_font_names from stable sort to sort_unstable followed by dedup. The Core copy was adopted; Cosmic at crates/gpui_wgpu/src/cosmic_text_system.rs retained stable sort at audit. Equal output does not prove adoption of the selected allocation/performance mechanism.

Current `all_font_names` sorts with `result.sort_unstable()` before dedup, preserving the independent glyph-offset correction. Cosmic host regressions pass F-ALL, and the owner compiles in F-BENCH/F-WINDOWS/F-WASM/F-MSRV. Local SHA mapping: L-GPU. Equal output was not used to claim a measured performance gain.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-GW2

**Cosmic benchmark corpus and paragraph workload.**

c214057e changes benches/layout_line.rs to replace source-corpus newlines with spaces, asserts the common corpus has no separator and registers mixed_direction_paragraphs with U+001C/Hebrew. The runtime Bidi_Class=B fix is adopted, but its benchmark corpus/scenario contribution was omitted.

Current normalized one-line corpus and mixed_direction_paragraphs match the official corpus/benchmark block, including U+001C/Hebrew and original UTF-8 FontRun length. Optional environment/file font loading is retained. Final F-BENCH compiles this actual `layout_line` target. Local SHA mapping: L-BENCH; no numerical benchmark execution is claimed.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-GW3

**Disabled Windows native caption controls.**

7194f987 guards both Max/Min hit testing and final non-client release actions with is_resizable/is_minimizable. Clearing creation style flags and checking resize borders do not guard explicit ShowWindowAsync. Preserve native capture/cancel and attached-HWND registration; inspect actual native style where required.

Current caption hit/release policy reads live WS_MAXIMIZEBOX/WS_MINIMIZEBOX, including a fresh read after the input callback. Disabled matching releases remain consumed and suppress native actions. Production-policy fixtures cover callback capability changes and movable/immovable disabled controls. F-WINDOWS compiles the native owner/tests; independent source re-review is closed. Windows execution remains unqualified. Local SHA mapping: L-GPU.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-GW4

**Focused ownership of thread-local IME completion.**

826f28eb requires GetFocus()==handle before ImmNotifyIME(CPS_COMPLETE). Per-window association and accepted input-handler leases do not imply native focus. An inactive non-text popup frame can otherwise commit a sibling composition. Keep ordered frame/command retirement and restrict the shared completion operation itself.

Current GetFocus()==handle guard precedes shared IME-context acquisition and CPS_COMPLETE; per-window ImmAssociateContextEx teardown retains its own custody. F-WINDOWS compiles this source; independent source re-review is closed. Sibling-window composition execution remains a Windows-native gate. Local SHA mapping: L-GPU.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### G-GW5

**Ordinary Windows PopUp topmost default.**

The same 826f28eb commit selects WS_EX_TOOLWINDOW | WS_EX_TOPMOST for ordinary WindowKind::PopUp. The fork still uses only WS_EX_TOOLWINDOW at audit; later flags do not supply TOPMOST. Existing read/restore of native z-order does not set the creation default. ADR 0029 unsupported generic live Topmost mutation does not explicitly reject this kind-specific default. Preserve provisional z-order compensation, first accepted presentation, nonactivation and attached-host custody.

Current ordinary ownerless, non-provisional PopUps receive WS_EX_TOPMOST. Explicit/resolved transient owners and provisional sessions retain their existing z-order profile; temporary detach does not promote. Attached-host custody and Normal/Floating/Dialog defaults are preserved. F-WINDOWS compiles the actual owner/provisional policy fixtures and ordinary-profile expectation; independent source re-review is closed. Native z-order execution remains unqualified. Local SHA mapping: L-GPU.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### Q-EXAMPLE

**Testing example asynchronous notification.**

Both asynchronous entity updates in `crates/gpui/examples/testing.rs` now notify. The actual platform example target is registered with test=true and required test-support, alongside a11y/window. F-WORKSPACE compiles those targets and F-ALL includes the testing example. Local SHA mapping: L-CORE. The old example omission is not treated as a production runtime defect.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### Q-LIST

**Edges Along and complete List padding unit.**

The 7bb565 Edges:Along prerequisite and 78fde List successor are implemented as one horizontal origin/row-width/scrollbar/width-cache unit. Current `elements/list.rs` preserves provisional measurement and admitted publication. `padding_tests.rs` exercises actual padded rendering, inferred width, cache invalidation and scrollbar geometry.

Independent review additionally finds **two vertical-inset defects shared with upstream**: `bounds_for_item` omitted top padding/border, and `max_scroll_offset` omitted both insets. The genuine pre-fix run `0a846f67-8dfa-48ee-8952-46b805ec3d3b` is **0/1**, reporting y=0/range=0 instead of painted y=10/range=20. Current item bounds add `padding.top`, and the same List owner's range adds top/bottom padding while retaining frozen drag heights. The identical test also proves bring-into-view reaches -20; it passes in final F-ALL. Independent re-review closes both findings. Local SHA mapping: L-CORE.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### Q-TAB

**Batched tab-order construction.**

62682ebde9's actual accepted dispatch-tree tab order is constructed in a batch when moving focus. Focus group placement, node ordering and missing-node diagnostics are retained; affected tests pass F-ALL. Local SHA mapping: L-CORE; no measured cost reduction.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### Q-TEXT

**Delay acquisition of text wrapper.**

4023e3c88e's wrapper acquisition now occurs only on the actual truncation branch of `elements/text.rs`. Font generation, source cache, rejected-candidate recovery and shaping remain with existing owners. The intermediate lifetime fixture failed because ordinary wrapping no longer takes a truncation wrapper; it now requests real truncation and keeps its nonempty-cache/final-weak-release assertions. The targeted repair and F-ALL pass. Local SHA mapping: L-CORE.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### Q-ELEMENT

**Blanket Element to IntoElement conversion.**

39467cf4's `impl<T: Element> IntoElement for T` is present in `element.rs`, with overlapping handwritten implementations removed across Core/components. Ordinary independent custom elements remain usable. Initial missing parent imports and the undersized AnyElement fixture arena are repaired without removing assertions. F-ALL/F-WORKSPACE/F-PUBLIC compile and execute their scoped consumers/regressions. Local SHA mapping: L-CORE.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### Q-COSMIC

**Combining-mark glyph offsets.**

985bbf42 is present in actual Cosmic glyph construction: `x + x_offset*font_size`, `y - y_offset*font_size`, retaining paragraph and source-byte mapping. The initial combining-mark regression fails with identical old mark/next x positions. The repaired case passes in the expanded **111/111** focused cohort and final F-ALL. Local SHA mapping: L-GPU; the red result remains in the historical receipt.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### Q-COLR

**Complete CPU DirectWrite COLR successor.**

4a6121bb's complete unit is implemented in `direct_write.rs`, `color_glyph_bitmap.rs`, build.rs and platform/device owners: recommended render/grid-fit modes, outline/bitmap fallback, shared subpixel origin, native color translation, union of every nonempty layer bound, transparent CPU source-over accumulators, contrast/gamma correction and straight BGRA. GPUState/color shader/recovery callbacks are removed; malformed coverage is rejected. The old staging Unmap/clear path is superseded.

Portable bitmap arithmetic/regressions pass F-ALL and all native production/test targets compile in real-sysroot F-WINDOWS. Independent source review is closed. Historical `97341cbb` qualified the old renderer/text path, not this CPU successor; actual DirectWrite fonts, COLR layers, WARP/readback and Windows execution remain unqualified. Local SHA mapping: L-GPU.

Verification owner: [main-thread verification record](../verification.md) and the platform matrix. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### RENDER-1

**Vertex-stage geometry clipping.**

Metal, WGSL and HLSL clip original geometry in the vertex stage while retaining local coordinates, safe-domain rounding, original-vertex fallback, half-open exact fragment clips and fractional/tiny-image UV handling.

Official fixed patch: [commit patch](https://github.com/zed-industries/zed/commit/ba490197cfb8.patch). Observed patch SHA-256: `16a889d267146eb3b6496c2f235e66161b8853183383f5215f72a0dfa4e36066`. Historical atomic local adoption: `0ed315daa37e94d6f80b357339ab8eff6b9b5afc`; the baseline is a0e5a818. Current follow-up implementations and their target-specific qualification remain separate.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [rendering successor](../verification.md#november-12-rendering-successor). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### RENDER-2

**Move ordinary color/background work out of fragments.**

Vertex/background color preparation is adopted across renderers. The baseline WGPU ordinary sprite instance-read optimization was partial; the current complete correction returns before load_poly_sprite while retaining exact clips, imported flags and LinearSrgb. Shader/ordinary-image/slow-path controls pass in final F-ALL, and the production graph compiles in F-WASM/F-WINDOWS. Local SHA mapping: L-RENDER for historical adoption and L-GPU for the current WGPU correction. Browser/WebGL execution and numerical cost remain unqualified.

Official fixed patch: [commit patch](https://github.com/zed-industries/zed/commit/caead827d77c.patch). Observed patch SHA-256: `1357cf9ab32f8fc6baacf56422ed6618e103a5bd9ee5d7a09446f1c2a9da3d1e`. Historical atomic local adoption: `0ed315daa37e94d6f80b357339ab8eff6b9b5afc`; the baseline is a0e5a818. Current follow-up implementations and their target-specific qualification remain separate.

Verification owner: [rendering successor](../verification.md#november-12-rendering-successor). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### RENDER-3

**Analytic quadratic path rendering.**

PathBuilder/PathWinding produce monotone quadratic outlines, CPU bins and winding tiles. Core and all renderers replace tessellation/intermediate path MSAA together. Checked projection/refusal and local paint bounds/inverse transform preserve gradients/patterns under nonuniform scale. Path becomes nongeneric; lyon_path 1.0.19, tiny-skia-path 0.11 and development lyon_extra 1.1.0 replace the facade; ordinary examples and consumers migrate.

Official fixed patch: [commit patch](https://github.com/zed-industries/zed/commit/ffe0fa29e41e.patch). Observed patch SHA-256: `d934769b6dd9c8a654f4b83ac2847deb3bd047c10b935239c8e34d4ae68d562c`. Historical atomic local adoption: `0ed315daa37e94d6f80b357339ab8eff6b9b5afc`; the baseline is a0e5a818. Current follow-up implementations and their target-specific qualification remain separate.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [rendering successor](../verification.md#november-12-rendering-successor). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### RENDER-4

**Kind-aware actual-bounds batching.**

BoundsTree and Scene order overlap by primitive kind and actual clipped union bounds, including text layers and shadows. Exact texture-instance compatibility and candidate-validity journal reconstruction/retained replay remain authoritative. Public bounds-taking paint_layer/push_layer conveniences retain compatibility while owners use actual bounds.

Official fixed patch: [commit patch](https://github.com/zed-industries/zed/commit/1d8eda94bd4d.patch). Observed patch SHA-256: `5e605f8275a8eabbc4766017a956b9f453bc43a059225b53f0547d646ec00bbd`. Historical atomic local adoption: `0ed315daa37e94d6f80b357339ab8eff6b9b5afc`; the baseline is a0e5a818. Current follow-up implementations and their target-specific qualification remain separate.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [rendering successor](../verification.md#november-12-rendering-successor). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### RENDER-5

**Opaque private-depth overdraw reduction.**

Reverse opaque quad prepass writes private depth; regular paint-order blending uses Greater depth testing. Safe quad partitions preserve rounded/exact clips. Backends recreate depth with their surface; host WGPU recording retains depth/globals/uploads through real host completion without taking submission authority.

Official fixed patch: [commit patch](https://github.com/zed-industries/zed/commit/be07e43ea0ad.patch). Observed patch SHA-256: `d1af16207e1cd1e0fc81f31d5f8e1d19bdd2dc2642557e2700caeaa94a3bf120`. Historical atomic local adoption: `0ed315daa37e94d6f80b357339ab8eff6b9b5afc`; the baseline is a0e5a818. Current follow-up implementations and their target-specific qualification remain separate.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [rendering successor](../verification.md#november-12-rendering-successor). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### RENDER-6

**Levien shadow approximation.**

Metal/WGSL/HLSL retain the rounded-rectangle approximation, original algorithm/source references, separate shape/raster extents and inset clipping.

Official fixed patch: [commit patch](https://github.com/zed-industries/zed/commit/8a5cf7e23a7c.patch). Observed patch SHA-256: `1d4534ac715b1be9c4720aaf7ef9d3ba1a999327770dcfee6be65f644949d5a6`. Historical atomic local adoption: `0ed315daa37e94d6f80b357339ab8eff6b9b5afc`; the baseline is a0e5a818. Current follow-up implementations and their target-specific qualification remain separate.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: [rendering successor](../verification.md#november-12-rendering-successor). Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### PRODUCT-FINAL

**Final git_ui task-lifetime product patch.**

The official 6ceb patch changes only crates/git_ui/src/branch_picker.rs. It stores the default-branch/remote-URL tasks on the picker so closing it drops the tasks, instead of detaching them and updating a released entity. This is inspected product ownership, not a framework task primitive change. It is recorded as the final target row and is outside the extracted closure.

Local source checkpoint: **`a0e5a818e0a3628114e37362bc57a0bc240e66c0`**, limited to the pre-existing mechanism described here.

Verification owner: official product patch path/hunk inspection; no framework test required. Earlier results keep their recorded revision/platform scope and are not fresh execution of this ledger.

### U-SOURCE

**Six former source-open entries now audited against complete patches and real owners.**

The three entries absent from the initial semantic inventories and three lane-excluded slices all have source dispositions. 31ceaf7908ef/d62802d456ac are superseded and source-reconciled; 170b2dbc9621/9a992ed33c14/0a39cdb4f3ec are implemented-local-verified through F-PUBLIC/F-WORKSPACE/F-WASM. Both c24e309d935d RC/include hunks are adopted and source-reconciled, with the actual Windows resource-compiler/link output remaining unexecuted. Lane exclusion was not treated as a framework decision.

The native-only Core proptest dev gate and platform Criterion/dev gates are present. **a11y, window, window_positioning and testing** are actual registered platform targets; testing requires test-support and participates in F-ALL. F-WORKSPACE compiles the native targets; the separate a11y wasm dev-target check passes F-WASM after the initial Criterion/Rayon failure. Window positioning still has no native multi-display execution receipt.

### U-SETUP

**Standalone setup documentation adopted now.**

170b2dbc9621 changes the Core README's platform factory, dependencies, OS feature guidance and minimal runnable entry. The newly completed [Core README](../../crates/gpui/README.md) uses the fork package aliases, open_gpui_platform::application().run, font-kit for the macOS text owner, wayland/x11 for Linux/FreeBSD, and feature-independent native DirectWrite on Windows. Those statements follow the actual selector and native text/renderer factories; the pre-existing factory name alone was not the complete documentation patch. The existing Rust 1.93 and host-supplied-services contracts remain intact.

Local SHA mapping: **L-EXAMPLES**. Disposition: **implemented-local-verified** through ordinary public-consumer/selector F-PUBLIC/F-WORKSPACE/F-WINDOWS and real compiler-floor F-MSRV. Linux-specific feature/runtime qualification remains separate; no fresh visible-window result is implied by documentation.

### U-TAFFY

**Historical 0.12.2 pin superseded by the adopted complete 0.15 owner.**

31ceaf7908ef changes only Core's manifest and the lock entry from exact Taffy 0.12.1 to 0.12.2, including its checksum. It does not add a measurement or Clip source hunk. The final historical target instead selects 0.15.0 via b488c4e81451. Root and independent-consumer locks resolve that exact version with checksum `3c8480887e8803986d96a7173729565520d4bbb721ad6b7ad6941a22897207aa`.

The actual fork adoption is **11d9a96033ea8425b07f64fdf2bad6503f1720dc**, already before the fixed baseline. `taffy.rs` creates NodeContext-bearing measured leaves and its complete compute_layout function matches the final official function except for three blank lines. The callback handles missing context, known dimensions, finite available space, scale conversion and compute_leaf_layout's zero baseline. `style.rs` preserves the one-to-one Clip conversion; the locked leaf source subtracts content-box insets before measuring and distinguishes ComputeSize/PerformLayout. Visible/Clip retain their non-scroll-container automatic minimum semantics. This is actual behavioral source comparison, not a version/symbol guess.

Disposition: superseded, **source-reconciled at `a0e5a818e0a3628114e37362bc57a0bc240e66c0`**. The compatible library requirement is broader than an official exact pin; entire third-party release parity is not claimed. Existing `taffy/layout_tests.rs` constraint/scale/repeated-measurement coverage and [C-T](#c-t) own qualification; this worker read but did not rerun those tests.

### U-RC

**Both Windows resource-location hunks adopted; native resource output unqualified.**

c24e309d935d contributes Core `build.rs` include-directory configuration **and** the RC file's relative manifest path. The real owner is [Core build.rs](../../crates/gpui/build.rs), not the Windows HLSL build script. The main-thread source now constructs resource_dir, joins both files, preserves both rerun-if-changed directives and passes `embed_resource::ParamsIncludeDirs([resource_dir])`. [gpui.rc](../../crates/gpui/resources/windows/gpui.rc) now names `"gpui.manifest.xml"` relative to that directory. Both previously missing hunks are present; the Windows selector enables Core windows-manifest so this route is reachable.

Local SHA mapping: **L-PLATFORM**. Disposition: **source-reconciled**. Real-sysroot F-WINDOWS compiles the Core/platform Windows-manifest selection, but cross-host build-script cfg does not run Windows resource compilation/linking. Actual RC/link/executable-manifest output therefore requires a Windows runner. Both source hunks are adopted independently of that native qualification.

### U-WASM

**Both a11y no_main and native-only proptest dev scope implemented.**

9a992ed33c14 changes both `examples/a11y.rs` and the Core manifest. The prior example-only note omitted a real dependency hunk. The source lane added the exact `#![cfg_attr(target_family = "wasm", no_main)]` attribute; main moved proptest from the unconditional dev-dependencies into `[target.'cfg(not(target_family = "wasm"))'.dev-dependencies]`. The separate optional production/test-support dependency retains its own feature contract; this change is specifically dev-target isolation. [The platform manifest](../../crates/gpui_platform/Cargo.toml) now declares a11y, window and testing, with testing's test-support requirement and test=true. Core still has autoexamples=false; the explicit platform targets provide the named launch paths.

The first actual wasm example check additionally exposes unconditional platform Criterion/Rayon native benchmark dependencies. Main gates the platform dev dependencies to non-wasm, while keeping wasm-bindgen in the wasm dev graph. F-WASM then passes the exact a11y dev-target command, independently of production Web compilation; F-WORKSPACE compiles native examples and F-ALL tests the registered testing example. Local SHA mapping: **L-EXAMPLES/L-CORE**; disposition: **implemented-local-verified**. Browser/native accessibility execution remains distinct.

### U-DOCS

**Obsolete Trunk URL change superseded by rewritten supported Web instructions.**

d62802d456ac changes only the examples README URL from trunkrs.dev to trunk-rs.github.io/trunk. The fork's rewritten examples documentation contains neither the old URL nor the old gallery installation paragraph. Its current browser setup belongs to the Web owner, `crates/gpui_web/examples/smoke_web` and xtask's existing Web setup. No stale URL remains to replace and no new product/gallery owner is needed for this narrow documentation patch.

Disposition: superseded, **source-reconciled at `a0e5a818e0a3628114e37362bc57a0bc240e66c0`**. [P19](#p19) and the existing Web smoke guide own the supported instructions/verification. This resolves a URL-only contribution; it grants no gallery source parity or fresh browser execution.

### U-POSITION

**Complete display-relative positioning formulas implemented.**

0a39cdb4f3ec changes five formulas in [window_positioning.rs](../../crates/gpui/examples/window_positioning.rs). The source lane supplied the entire official patch: the first origin adds the selected display origin; top and left positions use the display's top/left edges; right and bottom positions use right()/bottom() rather than just size. The full patch reverse-check matched the corrected file. Core's separate active-display placement is not a substitute for these example hunks.

Main registers the actual platform `window_positioning` target with this file as its path, separately from window.rs. F-WORKSPACE compiles it. Local SHA mapping: **L-EXAMPLES**; disposition: **implemented-local-verified** at source/compilation scope. Actual placement on multiple displays with nonzero/negative origins and differing work areas remains unexecuted.

## Native iOS follow-up receipt

The actual iPhone 17/iOS 26.3 runner first catches `InteractionProbe` being leased
by `WindowHandle::update` while quiescence invokes its activation observer. Three
window-only fixture operations now use `AsyncApp::update_window`, retaining all
composition/focus/submission assertions. A second run proves that UIKit's
`resignKeyWindow` is insufficient to clear actual key ownership. Interaction
revocation now closes the one-way native eligibility gate, synchronously hides and
restores the original visibility, and retains the same view/controller/Metal owner.
Both idle and in-flight composition paths then pass, including new drawable work
after revocation and refusal of responder/key reacquisition.

The final retirement control also disproves a fixture assumption: CAMetalLayer
accepts 1×1 but ignores a zero drawable-size request. Removing the ineffective
setter and zero-metadata oracle retains stronger custody checks: cleared Metal
device, no revival after layout, no next drawable, detached native objects and
exactly one terminal callback. The successful run prints `IOS NATIVE WINDOWING OK`
on 2026-11-13 at 01:39:02 +08:00. It covers public exact-generation activation,
six scene pixel controls, actual submitted-resize extent, idle guards, both IME
cleanup paths and renderer-before-native retirement. `simctl launch` returns zero
even when the app panics, so the explicit marker and absence of failure output are
required. A UIKit unbalanced-appearance-transition warning remains visible; this
smoke receipt does not establish physical-device/input or numerical performance.

## Canonical per-commit ledger

Historical rows follow the reference's reverse non-merge log order; the six rendering
patches and final product patch follow their fixed patch order. Each full SHA occurs
in exactly one row in this table. The source groups and row-specific notes jointly
define its scope, disposition, local SHA mapping and verification owner. Follow each
row's group link to its baseline checkpoint or current owner key in the
[local implementation revision map](#local-implementation-revision-map).

| Full upstream SHA | Official title / source | Owner / mechanism / local SHA mapping groups | Aggregate disposition | Historical and current source evidence |
| --- | --- | --- | --- | --- |
| `137e677a0561eb7284cfad1bedccea70155e2473` | [Fix Wayland IME handling with multiple windows (#58712)](https://github.com/zed-industries/zed/commit/137e677a0561) | [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `9ee3c503a4bbbc6b4a0f8a789acca4871d773223` | [extension: Perform compilation tasks in parallel (#55160)](https://github.com/zed-industries/zed/commit/9ee3c503a4bb) | [D-ROOT-PRODUCT](#d-root-product) | `outside-framework` | Product-only root futures-util 0.3.32 alias serves extension_builder/extension_cli and their compilation benchmark; no selected direct consumer. Existing transitive futures-util is retained; see the source/lock distinction in the group. |
| `ea50042f70911a220edc48e03b7c29090ac49f6b` | [Fix profiler breaking concurrent tests (#58813)](https://github.com/zed-industries/zed/commit/ea50042f7091) | [C-R](#c-r) | `source-reconciled` | profiler/actions.rs:89 retains the empty-running early return; no concurrent-test panic. |
| `bda5ac3626c6160681cc4100560ed0e0323e4ec3` | [linux: Fix Wayland clipboard reads blocking indefinitely (#58826)](https://github.com/zed-industries/zed/commit/bda5ac3626c6) | [P16](#p16) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `1d1a4bee340203e805ae0eee50c1064959b8bc2e` | [Change "Ok" to "OK" in UI (#58744)](https://github.com/zed-industries/zed/commit/1d1a4bee3402) | [C-F](#c-f), [P06](#p06), [G-C1](#g-c1) | `implemented-local-verified` | Historical baseline finding: C1: shared PromptButton conversion still emits Ok rather than OK; example slice outside lane. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `4eebf3fdbcb04625a8bb6491be49896c0d423a9f` | [settings_ui: Move skills management into settings UI (#58701)](https://github.com/zed-industries/zed/commit/4eebf3fdbcb0) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:skill_creator, members:crates/skill_creator. |
| `b14229f1a062817f719233b4d7a527dd0ec16b1b` | [Fix `TestScheduler::spawn_dedicated` leaking due to cycle (#58802)](https://github.com/zed-industries/zed/commit/b14229f1a062) | [H01](#h01) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `78658778a28fe9bfac2de9caddcdad0eb4ea1cd7` | [Use unstable sorts if deduplicating (#58751)](https://github.com/zed-industries/zed/commit/78658778a28f) | [C-T](#c-t), [G-GW1](#g-gw1), [W-WGPU](#w-wgpu) | `implemented-local-verified` | Historical baseline finding: text_system.rs:121 uses sort_unstable followed by dedup. Cosmic `G/src/cosmic_text_system.rs:200` still uses stable sort before deduplication. Core's separate unstable sort does not adopt this backend optimization. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `0d8a4d4292dac3fceaa40e73f1bbae038d7de30f` | [gpui: Free atlas tile space when removing tiles (#58874)](https://github.com/zed-industries/zed/commit/0d8a4d4292da) | [P23](#p23), [W-WINDOWS](#w-windows), [W-WGPU](#w-wgpu) | `source-reconciled` | P23: Intentionally divergent: published atlas regions retained by key/lease contract. `G/src/wgpu_atlas.rs:324/:384` deallocates immediately when unleased and drains deferred tiles after the last visual lease. `W/src/directx_atlas.rs:344` retains published regions while keys/leases share the texture; whole-texture retirement and unpublished-allocation rollback remain its established resource policy. Do not reclaim rectangles referenced by retained Scenes. |
| `48511e0b9cdcc32a25902d715145ec58c617b7af` | [Disable profiler with feature (#58942)](https://github.com/zed-industries/zed/commit/48511e0b9cdc) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c8554b46e78985931b0709a3d024462924e39c8d` | [gpui: Fix title bar clicks being delayed on macOS 27 (#58947)](https://github.com/zed-industries/zed/commit/c8554b46e789) | [P02](#p02) | `source-reconciled` | P02: Reverted by fca2ccd403e8; final successor 23bb2fc135a6 adopted. |
| `170b2dbc96213d2fd910f0cdddf774bbb29322d1` | [gpui: Document standalone app setup (#58766)](https://github.com/zed-industries/zed/commit/170b2dbc9621) | [U-SETUP](#u-setup) | `implemented-local-verified` | Standalone README factory/run/dependency/OS feature guidance complete using extracted aliases and actual native owners; F-PUBLIC/F-WORKSPACE/F-WINDOWS compile the ordinary consumer/selector. L-EXAMPLES. |
| `f1ad1e7fd58aed8a8f3cb7ff995dc9715a74ab0c` | [Update notify to v9.0.0-rc.4 (#58944)](https://github.com/zed-industries/zed/commit/f1ad1e7fd58a) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: patch:notify, patch:notify-types. |
| `bb59dc59fa90ee950ab2a92beb8084a04eb12b8f` | [Fix miscellaneous typos (#58979)](https://github.com/zed-industries/zed/commit/bb59dc59fa90) | [W-WINDOWS](#w-windows) | `source-reconciled` | The comment-only “accommodate” correction survives through the replacement full-edge resize hit test in `W/src/events.rs:2203` / `non_client_resize_hit_test`. No runtime behavior is omitted. |
| `297c4a4d78a7ca1fe6384770959e921ac2cc8b53` | [Bench app context phase 2 (#58202)](https://github.com/zed-industries/zed/commit/297c4a4d78a7) | [C-R](#c-r), [P22](#p22), [H12](#h12), [D-PRODUCT](#d-product) | `source-reconciled` | Root evidence: members:crates/benchmarks. |
| `c1b45aaa5f31401fa5368a8c9636f9d8db979517` | [Bump notify to fix a hang when watching or unwatching paths (#59047)](https://github.com/zed-industries/zed/commit/c1b45aaa5f31) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: patch:notify, patch:notify-types. |
| `03a8544040ff95ec2e7921ca71c83799ad7f17cd` | [tab_switcher: Middle-truncate long picker filenames (#59072)](https://github.com/zed-industries/zed/commit/03a8544040ff) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `7fd5ea4bf3d97826d96fb8e31426d18c459e8eb6` | [gpui: Fix list scroll events being reverted by pending scroll (#59002)](https://github.com/zed-industries/zed/commit/7fd5ea4bf3d9) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `e1bfcf85db56f75a2f6d67143aad2da22c3d2240` | [Fix file descriptor leak when process spawning failed on macOS (#59128)](https://github.com/zed-industries/zed/commit/e1bfcf85db56) | [P29](#p29), [H03](#h03) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `61ad9ebfcd27ff89e3c9b087f6d54999d08c56b6` | [gpui_macos: Initialize fallback fonts with primary font weight and style (#56771)](https://github.com/zed-industries/zed/commit/61ad9ebfcd27) | [P09](#p09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c899d5b590dc7b470c7edd480855d0076482045c` | [gpui: Add `alpha` method to `Rgba` (#52125)](https://github.com/zed-industries/zed/commit/c899d5b590dc) | [C-F](#c-f) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `60ed56b372dba2befe0361ff8b433a29c44562f0` | [Fix two random bugs in macOS process spawning (#59133)](https://github.com/zed-industries/zed/commit/60ed56b372db) | [P29](#p29), [H03](#h03) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `34cd17ff5e29b8eef9c48c69c2c56d24a770addc` | [gpui: Add builder API for BoxShadow (#57743)](https://github.com/zed-industries/zed/commit/34cd17ff5e29) | [C-T](#c-t), [H13](#h13) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `b6c7496aea2595fdbfbc04b0ac8dd393f363b569` | [multi_buffer: Don't eagerly clone `BufferSnapshot` in `range_to_buffer_ranges` (#59190)](https://github.com/zed-industries/zed/commit/b6c7496aea25) | [H07](#h07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `138139f830e6a3a0271c8aac60ed494b752dbc46` | [gpui_macos: Fix traffic light hitbox after repositioning (#58534)](https://github.com/zed-industries/zed/commit/138139f830e6) | [P02](#p02), [D04](#d04) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d4cc8d240965e1b3c86b1132df2278e4d01333f6` | [Patch async-process to allow reusing their reaper (#59156)](https://github.com/zed-industries/zed/commit/d4cc8d240965) | [P29](#p29), [H03](#h03), [D01](#d01) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `fca2ccd403e8d13c8f4b968cda2f2c322f420f5a` | [Revert "gpui: Fix title bar clicks being delayed on macOS 27 (#58947)" (#59214)](https://github.com/zed-industries/zed/commit/fca2ccd403e8) | [P02](#p02) | `source-reconciled` | P02: Superseded by final 23bb2fc135a6 mechanism. |
| `759bacff378bc886c1545a0da015d347c25bf15d` | [gpui_macos: Warn when falling back to NoopTextSystem (#59247)](https://github.com/zed-industries/zed/commit/759bacff378b) | [P09](#p09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c578f4d12b94be3bf41912aa7fb707ddf5e30382` | [agent: Fix shell hang on shell syntax errors with terminal tool usage (#59270)](https://github.com/zed-industries/zed/commit/c578f4d12b94) | [H06](#h06) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c642b422deaf6119aad2943ea22ec3074f39ef3c` | [util: Use job objects to reap spawned process trees on Windows (#58885)](https://github.com/zed-industries/zed/commit/c642b422deaf) | [H04](#h04), [D04](#d04), [W-SHARED](#w-shared) | `source-reconciled` | Adopted support owner: root Win32_System_JobObjects and `util/src/process.rs` provide existing Windows job cleanup. The later activity query is the support report's S-G1, not a second finding here. |
| `f73b9daf3b2d71e17ee1271da0e58ba2fbf2aa28` | [http_proxy: Add hostname allowlist policy types (#59217)](https://github.com/zed-industries/zed/commit/f73b9daf3b2d) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:http_proxy, dep:idna, members:crates/http_proxy. |
| `c52d06a8f31623a536b049d0440d830fb54d82d0` | [http_proxy: Add upstream proxy configuration (#59222)](https://github.com/zed-industries/zed/commit/c52d06a8f316) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:proxyvars. |
| `c8faadd9c81a32455c6ac3bd460230a56655bb73` | [http_proxy: Add the allowlisting proxy server (#59223)](https://github.com/zed-industries/zed/commit/c8faadd9c81a) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:httparse. |
| `a873cf402c8d5ffa13ab54efd29ccd4df59c7e46` | [Remove gpui's dependency on `async-process` (#59358)](https://github.com/zed-industries/zed/commit/a873cf402c8d) | [P24](#p24), [H09](#h09), [W-WINDOWS](#w-windows), [T-TOKIO](#t-tokio) | `implemented-local-verified` | Canonical command/shell helpers are in core-util and forwarded by util; `W/src/restart.rs`, `platform.rs` and `dialog.rs` use the same synchronous native commands/path simplification and ResultExt. The extracted full util dependency also serves fork SanitizedPath consumers; monorepo dependency aliases are not public runtime mechanisms. Inherited Tokio bridge and manifest/util follow-ups implemented; seven public bridge tests and F-ALL/F-PUBLIC pass. L-TOKIO. |
| `84b753cb51441f104fc35b540b9fe77a409f4529` | [editor: Refresh active debug line highlight on theme change (#59274)](https://github.com/zed-industries/zed/commit/84b753cb5144) | [C-F](#c-f) | `source-reconciled` | Only a blank line changes in Core; no surviving framework behavior. |
| `1722fe63bc98f2f3056107b17da4a0bbeba698e4` | [editor: Speed up multi cursor editing (#58510)](https://github.com/zed-industries/zed/commit/1722fe63bc98) | [C-R](#c-r), [P22](#p22), [H12](#h12) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c7ad65e46817b8bdc71c79ba3f15c898c0bc5678` | [markdown: Optimize `paint_search_highlights` (#59473)](https://github.com/zed-industries/zed/commit/c7ad65e46817) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `83d4847462e57a6f72dc314abea05d57d3bdcfb6` | [a11y: Settings UI (#59429)](https://github.com/zed-industries/zed/commit/83d4847462e5) | [C-A](#c-a), [W-SHARED](#w-shared), [D-SOURCE](#d-source) | `source-reconciled` | Root source reconciled through the linked owning audit: dep:accesskit_windows. Core a11y/framework source adopted; Windows adapter package version remains different. Root selects accesskit_windows 0.32.1 versus final official 0.34 after its 0.33.1 introduction here. Record the version difference explicitly; current qualified adapter source is not proof of equality of all third-party bug fixes. |
| `1bc9d8e80ec78a5f2d724c85105bd31a9e4b8898` | [Bump `alacritty_terminal` repo SHA (#59445)](https://github.com/zed-industries/zed/commit/1bc9d8e80ec7) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:alacritty_terminal. |
| `362035d52aaf5e99f47179961ff3df5e67740316` | [Fix opening folders whose name ends in a position-like suffix (#59384)](https://github.com/zed-industries/zed/commit/362035d52aaf) | [H08](#h08) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `6076ce27384ca14bb9f6fb8b1218acf1d78f2878` | [perf: Add initial benchmark for markdown element (#59524)](https://github.com/zed-industries/zed/commit/6076ce27384c) | [C-R](#c-r), [P22](#p22) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `10628c3d2cafc50f78a8861d7d6882b6ecd439ce` | [agent_ui: Add in-thread search bar (#57231)](https://github.com/zed-industries/zed/commit/10628c3d2caf) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d1f500edf15b5e6726710ab89199880e75ad45eb` | [Fix binary name resolution against custom PATH on macOS (#55672)](https://github.com/zed-industries/zed/commit/d1f500edf15b) | [P29](#p29), [H03](#h03) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `ccf4058b7a6b05207d4f1dd25106ec5fc439cc74` | [Add preview to pickers and make them resizable (#59604)](https://github.com/zed-industries/zed/commit/ccf4058b7a6b) | [C-I](#c-i) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `9a992ed33c148179204885f6bff8c744dfa1d410` | [gpui: Fix web examples build (#59470)](https://github.com/zed-industries/zed/commit/9a992ed33c14) | [C-X](#c-x), [U-WASM](#u-wasm) | `implemented-local-verified` | Both official hunks adopted: a11y wasm no_main and native-only Core proptest dev scope. Registered examples compile in F-WORKSPACE; after native-only Criterion/dev gating, actual wasm a11y dev-target compilation passes F-WASM. L-EXAMPLES. |
| `8ba35e5eacf30a847140e82eced017f93f3f6df0` | [acp: Update agent client protocol crate to 0.15.0 (#59593)](https://github.com/zed-industries/zed/commit/8ba35e5eacf3) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `a1a881b0f78f071e2e353f5155f6bc308082e184` | [Add shell completions for CLI (#57440)](https://github.com/zed-industries/zed/commit/a1a881b0f78f) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:clap_complete, dep:clap_complete_nushell. |
| `f791aa57d798f4a4746ae72999f54f7ec2925ebf` | [gpui: Bump resvg/usvg and regression test for panic (#59704)](https://github.com/zed-industries/zed/commit/f791aa57d798) | [C-E](#c-e), [W-SHARED](#w-shared), [D-SOURCE](#d-source) | `source-reconciled` | Root source reconciled through the linked owning audit: dep:resvg, dep:usvg. Adopted by Core SVG owner: resvg/usvg 0.46.0 and split-font glyph regression in `C/src/svg_renderer.rs`; extracted dependency placement differs from upstream root. No second GPU SVG rasterizer is needed. |
| `479bce099509394aa121c2c5c1cf8f2639bbc7e9` | [Implement telemetry for the remote server (#59692)](https://github.com/zed-industries/zed/commit/479bce099509) | [H09](#h09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `ad6ce93202ad82bd436b3576c8bea5b0bc55c370` | [webrtc: Bump webrtc to fix a panic (#59750)](https://github.com/zed-industries/zed/commit/ad6ce93202ad) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: patch:libwebrtc, patch:livekit, patch:webrtc-sys. |
| `f18e91efc6de8ae74fa7e7606203210a7a25286f` | [Reapply traffic light position after appearance changes (#59712)](https://github.com/zed-industries/zed/commit/f18e91efc6de) | [P02](#p02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `8036a3c74b7431b469b00f52ef5f2a546c8e7f60` | [Add non breaking glue characters as non-wrapping characters (#59775)](https://github.com/zed-industries/zed/commit/8036a3c74b74) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `2df74932bc6666e7a00e877d375dafb39a7a9273` | [acp: Support embedded resources in tool call content (#59821)](https://github.com/zed-industries/zed/commit/2df74932bc66) | [D-ROOT-PRODUCT](#d-root-product) | `outside-framework` | Product-only root mime 0.3.17 alias serves acp_thread embedded resources/agent conversation UI. No selected direct consumer; existing HTTP MIME dependencies are retained. |
| `9ac117693b20d4b2dd1f72af655524bed2a0ddcc` | [gpui_windows: Fix immovable windows skipping hit testing entirely (#59816)](https://github.com/zed-industries/zed/commit/9ac117693b20) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/events.rs:2203/:3032` keeps immovable-window caption-button hit testing while suppressing caption drag. Existing focused helper tests survive. The later disabled-control guards are separately missing in row 8. |
| `ee571d3c6991f19caf6f505407d02f3238dfc39e` | [gpui: Add system wake callback (#59831)](https://github.com/zed-industries/zed/commit/ee571d3c6991) | [C-R](#c-r), [P18](#p18), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/platform.rs` owns wake registration, PBT_APMRESUMEAUTOMATIC delivery and notification cleanup; App delivery uses ordered native ingress. Required Win32 Power features are present. |
| `9de0590a8197cf0725509810435b6ed5bae18432` | [acp: Update to Rust SDK 1.0.0 (#59889)](https://github.com/zed-industries/zed/commit/9de0590a8197) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `dae3e574e49baff324d8e708ce4de1b407717568` | [gpui_linux: Improve error message when X11 and Wayland feature flags are missing (#58685)](https://github.com/zed-industries/zed/commit/dae3e574e49b) | [P14](#p14) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `ec7c11c65c577ee6b477ced99d41eb86636b6b2e` | [Fix delayed titlebar clicks on macOS 27 Beta (#59836)](https://github.com/zed-industries/zed/commit/ec7c11c65c57) | [C-F](#c-f), [P02](#p02) | `source-reconciled` | Old is_movable workaround docs superseded by implemented app_owns_titlebar_drag in 23bb2fc135. P02: Superseded by separate app_owns_titlebar_drag contract. |
| `35eaeb94a7e5e9dcd7384df4bd21f01c30a38a09` | [gpui: Fix xdg_toplevel app_id set to None at first commit on Wayland (#55583)](https://github.com/zed-industries/zed/commit/35eaeb94a7e5) | [C-F](#c-f), [P13](#p13) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `eb877503230a2beba1105d1ecb9320d1f555347e` | [gpui_windows: Add missing windows dependency feature (#59886)](https://github.com/zed-industries/zed/commit/eb877503230a) | [W-SHARED](#w-shared) | `source-reconciled` | Adopted prerequisite: root Windows features include Win32_System_Power, supplied to Core/backend rather than relying on upstream's old duplicated 0.61 declaration. |
| `7582bf74349f6084d4f7370ebb8215d70f7105cf` | [Allow empty values in .editorconfig (#60026)](https://github.com/zed-industries/zed/commit/7582bf74349f) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:ec4rs. |
| `4b119fc54758bbd5e1603fb2d48f2af743fe14b2` | [Add llama.cpp as a language model provider (#59964)](https://github.com/zed-industries/zed/commit/4b119fc54758) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:llama_cpp, members:crates/llama_cpp. |
| `441de84a77e7b0c9306801497fbb1bf36630055e` | [gpui_linux: Suppress dead code warning on `wake_sender` (#60090)](https://github.com/zed-industries/zed/commit/441de84a77e7) | [P14](#p14) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `485aeabff3dda9db93b262140f1696f7cc0f2c54` | [reqwest_client: Drop stale connections with keepalives (#59929)](https://github.com/zed-industries/zed/commit/485aeabff3dd) | [H10](#h10) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `7c3160b7bfd7c76be9d00ad6edd6a3d4061eea76` | [gpui_linux: Consume Wayland startup activation token (#59995)](https://github.com/zed-industries/zed/commit/7c3160b7bfd7) | [P13](#p13) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `a923597341c83df525e082bb5ecb3fa66b53c5f4` | [Fix agent terminal in headless eval sandbox (#59969)](https://github.com/zed-industries/zed/commit/a923597341c8) | [H06](#h06) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `b04ae5ed8171d31539847271fd9af4b73447ed1e` | [acp: Update ACP SDK to 1.0.1 (#60088)](https://github.com/zed-industries/zed/commit/b04ae5ed8171) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `04efe79649b4bc0f47431455e9200c1c0c6349bc` | [Improve overall compilation stats after the telescope picker feature (#60114)](https://github.com/zed-industries/zed/commit/04efe79649b4) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:picker_preview, members:crates/picker_preview. |
| `b083358680a69ec982bd3efdcbcd1c90e08d6b2b` | [Suppress `pet` log spam when opening Python projects (#60204)](https://github.com/zed-industries/zed/commit/b083358680a6) | [H15](#h15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `9cb3140ea10dbbf3ca5ece1ab9daf9ec3f44ba0b` | [wgpu: Use wgpu from crates.io (#60264)](https://github.com/zed-industries/zed/commit/9cb3140ea10d) | [W-SHARED](#w-shared), [D-SOURCE](#d-source) | `source-reconciled` | Root source reconciled through the linked owning audit: dep:wgpu. Superseded dependency selection. The earlier registry 29.0.4 migration is replaced by the explicitly selected final official Git WGPU/Naga correction; do not revert to registry merely to match this historical intermediate step. |
| `2882636c06923e58d83865ecc370bd0d8199d738` | [Fix hanging updates after system sleep (#60301)](https://github.com/zed-industries/zed/commit/2882636c0692) | [C-R](#c-r), [H11](#h11) | `source-reconciled` | Explicit upstream introduction/revert pair; final mechanism superseded, do not restore. |
| `b5c2d8a13f395bbdbaf9cb74bda16bbcb00414d1` | [Revert "Fix hanging updates after system sleep (#60301)" (#60321)](https://github.com/zed-industries/zed/commit/b5c2d8a13f39) | [C-R](#c-r), [H11](#h11) | `source-reconciled` | Explicit upstream introduction/revert pair; final mechanism superseded, do not restore. |
| `552fc9f3c3c775276c2ce3f0fb93f1f4c2c18ba6` | [reqwest_client: Fix streamed request bodies being truncated (#60314)](https://github.com/zed-industries/zed/commit/552fc9f3c3c7) | [H10](#h10) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f4364d870e8e805ff272ab25a198ab46db19e51b` | [gpui_linux: Add support for open_window in headless client (#60359)](https://github.com/zed-industries/zed/commit/f4364d870e8e) | [P14](#p14) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `fa8540ff6299c30ebb325601ad821127a314f44e` | [Update Wasmtime dependencies (#60341)](https://github.com/zed-industries/zed/commit/fa8540ff6299) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:wasm-encoder, dep:wasmparser. |
| `5aa6e8a0b37a46828325e9f4f01ce3e0138017b3` | [Reduce OAuth scope to avoid Windows credential size limit (#58541)](https://github.com/zed-industries/zed/commit/5aa6e8a0b37a) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/platform.rs:3183/:4729` rejects oversized blobs before background CredWriteW. Its error includes only size/limit, preserving the fork's qualified redaction policy. OAuth/product scope changes remain outside. |
| `c56646ffdfffc27d0aab8b3940ef93c4153375be` | [terminal: Fix IME candidate window not following cursor in TUI apps (#59911)](https://github.com/zed-industries/zed/commit/c56646ffdfff) | [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `4b7369481dcf36f22ff8f813d411e1d296aebe57` | [Add dylint lint library for Zed-specific patterns (#58496)](https://github.com/zed-industries/zed/commit/4b7369481dcf) | [D-ROOT-PRODUCT](#d-root-product) | `outside-framework` | Product/tooling-only root metadata discovers the separate nightly tooling/lints library and UI fixtures. No framework runtime source or selected dependency change. |
| `28c2e7d1e4dc9c8df965adf7656f9b1d993c9a23` | [Fix tree-sitter-markdown scanner serialize buffer overflow (#60312)](https://github.com/zed-industries/zed/commit/28c2e7d1e4dc) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:tree-sitter-md. |
| `a29c0d41f4787d7cd268d4f8ad1a706af86042c7` | [gpui: Add input region support for Wayland windows (#60161)](https://github.com/zed-industries/zed/commit/a29c0d41f478) | [C-F](#c-f), [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `e7803a88f55e290a0478d46bd0137e711d6da862` | [gpui: Add ParentElement impl for AnimationElement (#54145)](https://github.com/zed-industries/zed/commit/e7803a88f55e) | [C-E](#c-e) | `source-reconciled` | AnimationElement ParentElement forwarding present at elements/animation.rs:209. |
| `20a93f6195ca8e9f0a748038317a5efe1be3e482` | [gpui_macos: Fix glyph rendering when fonts share a PostScript name (#57250)](https://github.com/zed-industries/zed/commit/20a93f6195ca) | [P09](#p09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `7194f987a74317881768b4d3a5ce1cc93a3228bd` | [Fix disabled Windows window controls (#60440)](https://github.com/zed-industries/zed/commit/7194f987a743) | [G-GW3](#g-gw3), [W-WINDOWS](#w-windows) | `implemented-local-verified` | Historical baseline finding: Window creation correctly removes disabled style flags and resize hit testing checks WS_THICKFRAME. `events.rs:3032/:2352` omits both Max/Min capability checks and still performs explicit disabled caption actions. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `71b3b9b8874218170de42cc98f96816b683b2005` | [gpui_macos: Guard start_display_link against nil screens (#60419)](https://github.com/zed-industries/zed/commit/71b3b9b88742) | [P10](#p10) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `693962917b5a015949ad2e768bc10ea169d41546` | [gpui: Fix clear drag overlay when external drag ends outside window (#45759)](https://github.com/zed-industries/zed/commit/693962917b5a) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `961f4f202465629fe00ccf55bd9ebf01d643b931` | [gpui: Refresh mouse position after bounds changes (#60421)](https://github.com/zed-industries/zed/commit/961f4f202465) | [C-I](#c-i), [P15](#p15), [G-M4](#g-m4) | `implemented-local-verified` | Historical baseline finding: M4: bounds_changed/commit_platform_facts never refresh mouse_position. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `fbd911ed3e0fdb98ab5ae7a66679774d4986db0a` | [Limit SVG Pixmap size to avoid GPUI texture allocation errors (#56468)](https://github.com/zed-industries/zed/commit/fbd911ed3e0f) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c05b439174c95a51df92a797c2a17933ae44a59e` | [acp: Show descriptions for elicitation options (#60527)](https://github.com/zed-industries/zed/commit/c05b439174c9) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `56009f39bb916be80baace86919267cf0e09035c` | [gpui_wgpu: Fix wgpu renderer teardown panic (#60160)](https://github.com/zed-industries/zed/commit/56009f39bb91) | [W-WGPU](#w-wgpu) | `source-reconciled` | `G/src/wgpu_renderer.rs:1320/:2468/:2552` and `surface_lifecycle.rs` guard resize/transparency/detach after release/shutdown; optional core ownership replaces the old resources panic. |
| `d12b980ee0721dc7fb72aeea511b1ab7b62af98c` | [bedrock: Add native support for Bedrock Mantle models (#60480)](https://github.com/zed-industries/zed/commit/d12b980ee072) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:aws-sigv4. |
| `74798c68d5c63d31e2ccca5c8f5ec0a02c90679c` | [gpui: Add run_embedded and ApplicationHandle for externally driven run loops (#60574)](https://github.com/zed-industries/zed/commit/74798c68d5c6) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `74b5207744ef95505fd75c56bf2c9d4736e931ee` | [Unify Render and RenderOnce into View (#58087)](https://github.com/zed-industries/zed/commit/74b5207744ef) | [C-V](#c-v), [H13](#h13) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d9e35975c238013d2888c5ef8231c9f3d786c993` | [csv_preview: Add row filtering feature (#60339)](https://github.com/zed-industries/zed/commit/d9e35975c238) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `6b733d105896a20924bd4aba87bd7baa20b62ac6` | [search: Bump fancy-regex dependency and enable CRLF mode (#55471)](https://github.com/zed-industries/zed/commit/6b733d105896) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:fancy-regex. |
| `546a16d64fa589c737a9e33dff02a129a673b21d` | [gpui: Add parent-anchored native popup windows (with wayland xdg_popup implementation only so far) (#60232)](https://github.com/zed-industries/zed/commit/546a16d64fa5) | [C-F](#c-f), [P15](#p15), [W-WINDOWS](#w-windows) | `source-reconciled` | Windows reports typed unsupported anchored popups through its capability/creation validation and `popup::PopupNotSupportedError`; complete Wayland popup behavior belongs to the platform ledger. Ordinary PopUp is a separate kind, covered by row 24. |
| `23c0080d1dd5e8e550ede91c0d573658e5aabe76` | [gpui_windows: Check composition attribute lookup (#60122)](https://github.com/zed-industries/zed/commit/23c0080d1dd5) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/window.rs::set_window_composition_attribute` checks GetProcAddress before transmuting/calling it; fork style and native-authority checks remain. |
| `664b7ecb40b83fa728bc40ef8f0385e35ea01287` | [gpui: Fix hover state not clearing when mouse leaves window (#60275)](https://github.com/zed-industries/zed/commit/664b7ecb40b8) | [C-I](#c-i), [P15](#p15) | `source-reconciled` | elements/div.rs:4412 handles MouseExitEvent; layout reconciliation is accepted-candidate scoped at :4933. |
| `dd68454633ac9fea8b42115ae09568e7395831e2` | [Update wayland-backend to fix Wayland file dialog crash (#60621)](https://github.com/zed-industries/zed/commit/dd68454633ac) | [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f28177003499ec9c5052d1883fee62b148451c59` | [gpui: Store GPU-facing bools as PaddedBool32 to avoid uninitialized padding (#60482)](https://github.com/zed-industries/zed/commit/f28177003499) | [C-F](#c-f), [W-WINDOWS](#w-windows), [W-WGPU](#w-wgpu) | `source-reconciled` | Core's initialized PaddedBool32 plus `G/src/shaders*.wgsl` and `W/src/shaders.hlsl` use complete u32 values for wavy/grayscale/inset. Shared ABI tests and direct upload replace duplicate DirectX layouts. |
| `2c4e44704c37ee87e59ac84e3e17388178b28545` | [Fix "Task polled after completion" panic (#60693)](https://github.com/zed-industries/zed/commit/2c4e44704c37) | [P14](#p14), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/dispatcher.rs:53/:167` uses one-shot owned raw RunnableVariant callbacks, native relative timers and Win32 priority environments. Retained failure semantics keep an infallible scheduler task pending. |
| `8f92822cbfb41db7fc282ad19093caa662815aa1` | [agent: Sandbox security review and docs update (#60291)](https://github.com/zed-industries/zed/commit/8f92822cbfb4) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:seccompiler. |
| `9c3273201ce6fe6b97a5aa38a7a2cab8098b7a6e` | [gpui: Add mobile/touch API surface (#60496)](https://github.com/zed-industries/zed/commit/9c3273201ce6) | [C-I](#c-i), [P21](#p21), [G-M11](#g-m11), [G-IOS-CORE](#g-ios-core) | `implemented-local-verified` | Historical baseline finding: M11: portable touch vocabulary/recognition exists, but TouchClickEvent, ClickEvent::Touch and is_secondary are absent; API slice only. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `b0da438545633412d6792796b3d92337ea44146d` | [gpui: Fill auto-sized window roots to the viewport (#60739)](https://github.com/zed-industries/zed/commit/b0da43854563) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `49ad06c1b4047b018b1622e1cf94cafcacd39247` | [gpui: Add container_query element (#60774)](https://github.com/zed-industries/zed/commit/49ad06c1b404) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `23bb2fc135a69492847c3aa68444a7d14cc282f6` | [macos: Fix window move controls are disabled (#60620)](https://github.com/zed-industries/zed/commit/23bb2fc135a6) | [C-F](#c-f), [P02](#p02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `715557fe51ce8baff5bef1259a1d2fe133d2b4f9` | [editor: Add support for pasting images in Markdown (#58588)](https://github.com/zed-industries/zed/commit/715557fe51ce) | [C-F](#c-f), [P11](#p11), [G-C2](#g-c2) | `implemented-local-verified` | Historical baseline finding: C2: ImageFormat::extension missing from shared Core facade; product Markdown owner excluded. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `89e8a4b9ec7e0fa8a49987a3c1dbf29343dc6999` | [gpui_windows: Harden DirectWrite and DirectX text rendering (#60124)](https://github.com/zed-industries/zed/commit/89e8a4b9ec7e) | [W-WINDOWS](#w-windows), [Q-COLR](#q-colr) | `implemented-local-verified` | `W/src/direct_write.rs` uses mutable RendererContext and nullable-array validation. `directx_atlas.rs:43/:317` checks source and destination before publication. The old staging-map color path was qualified previously and is now removed by CPU COLR; this is not a fresh missing Unmap. |
| `905e955a702707cd81a2e5bae9b381a7a9c7f614` | [gpui: Implement window attention (#58779)](https://github.com/zed-industries/zed/commit/905e955a7027) | [C-F](#c-f), [P18](#p18), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/window.rs:8981` checks activity, exact registration and retirement before FlashWindowEx. Final single-flash policy follows row 23. |
| `8e06c323ae063f3d245ea5c283d4bf373ad51db2` | [Drop the unused avif feature from the image dependency (#60897)](https://github.com/zed-industries/zed/commit/8e06c323ae06) | [W-SHARED](#w-shared), [D-SOURCE](#d-source) | `source-reconciled` | Root source reconciled through the linked owning audit: dep:image. Adopted Core/image owner: root explicit codec features retain supported formats/Rayon and omit default AVIF encoding, with the independently recorded image 0.25.4 floor and existing graph qualification. |
| `084b5d21f67cdbe06edd2822ce34e58b3cd774f7` | [gpui_web: Implement clipboard write and paste (#60890)](https://github.com/zed-industries/zed/commit/084b5d21f67c) | [P12](#p12) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `96ce8f2a05f8912851e5d20d808fe21f4134bd45` | [gpui_macos: Replace per-window CVDisplayLinks with a per-display registry (#60696)](https://github.com/zed-industries/zed/commit/96ce8f2a05f8) | [P10](#p10) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `2268045a119030735f762e5afaf59da0bda869f4` | [a11y: Landmarks and menu improvements (#60397)](https://github.com/zed-industries/zed/commit/2268045a1190) | [C-A](#c-a), [W-SHARED](#w-shared), [D-SOURCE](#d-source) | `source-reconciled` | Root source reconciled through the linked owning audit: dep:accesskit. Adopted prerequisite: root accesskit 0.24.0 enables enumn; current semantic menu/landmark source is reconciled in Core/platform reports. Product menus remain outside, framework properties do not. |
| `5d447d4223f8455db4811ed332c4ad744bb13e40` | [gpui_web: Only prevent default for keyboard events the app handled (#60889)](https://github.com/zed-industries/zed/commit/5d447d4223f8) | [P19](#p19) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f7cf09b3676977f069638b5adce5b664981f276d` | [language_model: Fix compilation on wasm (#60955)](https://github.com/zed-industries/zed/commit/f7cf09b36769) | [H16](#h16), [D03](#d03) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `a496bf0ff6c0e6a1e098907a6aa7f527330d20f3` | [Add a bunch of stray design refinement throughout (#60937)](https://github.com/zed-industries/zed/commit/a496bf0ff6c0) | [C-E](#c-e), [G-M3](#g-m3) | `implemented-local-verified` | Historical M3: style-only drag_over omitted its hitbox. Current should_insert_hitbox includes nonempty drag_over_styles with admitted exact geometry; affected regression passes F-ALL. L-CORE. |
| `5e982c6bdc315a6d2bb68b3edefae00ceedd35f4` | [gpui: Fix crash when opening a popover from inside another popover (#60926)](https://github.com/zed-industries/zed/commit/5e982c6bdc31) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `afc13dc8e054705a42e2931b0840ed03ccfa583a` | [gpui_linux: split xkbcommon wayland/x11 features (#60834)](https://github.com/zed-industries/zed/commit/afc13dc8e054) | [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d06ebb7c033365a427b5cc8a3e2a2651bcb0c267` | [Disable all spinners in Zed (#60614)](https://github.com/zed-industries/zed/commit/d06ebb7c0333) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `b51f967d8bf80230531615cebdd6e26aefa0ed3e` | [Thin out unnecessary dependencies from core crates (#60977)](https://github.com/zed-industries/zed/commit/b51f967d8bf8) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:tree-sitter. |
| `af7de9a03cb8365cd754e686c764c7075ed4d5e3` | [Fix project panel follow focus issues (#60974)](https://github.com/zed-industries/zed/commit/af7de9a03cb8) | [C-I](#c-i) | `source-reconciled` | Window refresh_focus_authority :4236 stages accepted focus-listener changes; followup reconcile avoids missing-element loops. |
| `a1230fc5847e881f584cad2694a3885747572f2a` | [Update Rust crate async-tar to 0.6.0 &#91;SECURITY&#93; (#60623)](https://github.com/zed-industries/zed/commit/a1230fc5847e) | [D02](#d02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `72656afa6d477d57ca875b1b7c8423483e32dc74` | [Use more efficient thread pool API on Windows (#60917)](https://github.com/zed-industries/zed/commit/72656afa6d47) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/dispatcher.rs:64` calls TrySubmitThreadpoolCallback rather than creating/closing a work object; timers retain their separate handle. |
| `166f044fd046e0de73cd64a027505db90f523b97` | [gpui: Linux wayland set exclusive zone and edge (#60163)](https://github.com/zed-industries/zed/commit/166f044fd046) | [C-F](#c-f), [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f181a2f47be57d1dc57412d42ac44df2fc7eca8f` | [Split out `RelPath` into a separate crate (#61029)](https://github.com/zed-industries/zed/commit/f181a2f47be5) | [H07](#h07), [D05](#d05) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `b5fa67cfb31e0f754eb83d1e7918b93f6b15500c` | [path: Fix unconditional use of `serde` in `path` crate (#61037)](https://github.com/zed-industries/zed/commit/b5fa67cfb31e) | [H07](#h07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `63692b8b4724357fa63d6318b45f3c3fee6f672a` | [search: Add LSP location pickers for references, definitions, and diagnostics (#59838)](https://github.com/zed-industries/zed/commit/63692b8b4724) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:lsp_locations, members:crates/lsp_locations. |
| `91fdd55889ab286b3e0712b44e784cbb852b9c0b` | [gpui: Bump taffy from 0.10.1 to 0.12.1 (#60721)](https://github.com/zed-industries/zed/commit/91fdd55889ab) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `53a3874a32ac0ceda9e11ce614de9c9e7171054d` | [Relax core-foundation pin from =0.10.0 to 0.10 (#61089)](https://github.com/zed-industries/zed/commit/53a3874a32ac) | [D-APPLE-PACKAGES](#d-apple-packages) | `source-reconciled` | Adopted relaxed CoreFoundation requirement: current 0.10.0 is a caret range, root resolves 0.10.0 and the independently locked consumer actually resolves 0.10.1. No exact-pin restriction survives. |
| `b05f40c5546b47bcf9561136dc0fcdcd9968cb63` | [Fix post-taffy-bump layouting issues (#61107)](https://github.com/zed-industries/zed/commit/b05f40c5546b) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `ae3bbdeb2df000acd8e0c5c75275cbb357e2b223` | [gpui: Avoid redundant group hover redraws (#61118)](https://github.com/zed-industries/zed/commit/ae3bbdeb2df0) | [C-I](#c-i) | `source-reconciled` | elements/div.rs:4035-4054 notifies current_view only inside a stateful hover-state transition. |
| `058f01fa93503491a735bfded53e77bfaa276148` | [gpui: Fix clicks on window prompts dismissing popovers behind them (#61136)](https://github.com/zed-industries/zed/commit/058f01fa9350) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `9552acc2bc242d45342fa9b5a987d43868aee1ec` | [gpui: Apply EXIF orientation when decoding static images (#61017)](https://github.com/zed-industries/zed/commit/9552acc2bc24) | [C-E](#c-e), [P11](#p11) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `31ceaf7908ef6239dbf5fcf74e2d185a89152ebf` | [Bump taffy (#61183)](https://github.com/zed-industries/zed/commit/31ceaf7908ef) | [U-TAFFY](#u-taffy) | `source-reconciled` | Actual patch only pins Taffy 0.12.1 -> 0.12.2. Superseded by the complete adopted 0.15 measured-leaf/Clip owner at local 11d9a96033ea8425b07f64fdf2bad6503f1720dc before baseline; exact root/independent 0.15.0 lock checksum and full function comparison support this decision. |
| `5079b33d657593ffd8a2d2978534988a8b40867e` | [gpui_linux: Fix unbounded memory growth when composing text with Fcitx5 under KWin/Wayland (#61079)](https://github.com/zed-industries/zed/commit/5079b33d6575) | [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f1280b64a4146519d389c7d2f1c4817893d1d1e3` | [gpui: Unify `raw-window-handle` dependency (#61193)](https://github.com/zed-industries/zed/commit/f1280b64a414) | [P24](#p24), [D05](#d05), [W-WINDOWS](#w-windows), [W-WGPU](#w-wgpu) | `source-reconciled` | Windows and WGPU use compatible raw-window-handle 0.6 declarations; current roots/locks select 0.6.2. Whether the literal is local or workspace-inherited does not change the Rust type identity. |
| `de827bce2ff1cbb40040bea0ef57c8ac56afd726` | [gpui: Add system notification platform APIs (#61189)](https://github.com/zed-industries/zed/commit/de827bce2ff1) | [C-R](#c-r), [P18](#p18), [D04](#d04), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/system_notifications.rs` retains packaged/unpackaged AUMID, registry registration, hashed tags, XML actions, replace/dismiss and asynchronous responses. `platform.rs` forwards public APIs; explicit retirement prevents callbacks after shutdown. |
| `7eb8af27a6f1995edbd2122597e7077b4040fe41` | [gpui: Remove `ExitProcess` shutdown workaround (#61219)](https://github.com/zed-industries/zed/commit/7eb8af27a6f1) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/platform.rs:251/:2677/:4067` retains ExitProcess only for standalone production run-exit, after terminal convergence. ExternalHost/returning-test paths return. Candidate queue's “native quit completion and Restart Manager” and `docs/verification.md:8855` explicitly retain this aws-lc teardown policy; do not silently call it adopted or a newly missing source fix. |
| `eb962794a33cbd414c6a343a8bb1136c94fe6901` | [buffer_diff: Canonicalize ambiguous hunk placement to fix staging corruption (#60584)](https://github.com/zed-industries/zed/commit/eb962794a33c) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:imara-diff. |
| `0c51c7fd2481859e9da5c490ef8e41ddbcf1a341` | [Reduce overdraw for border-only quads (#61274)](https://github.com/zed-industries/zed/commit/0c51c7fd2481) | [C-F](#c-f), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/directx_renderer.rs:118/:414` retains RAII annotations, batch labels and lazy contextual errors. Core border strip partitioning is now the complete later six-commit implementation. |
| `690c7fac64a4ed013ad92d8794333a6548858e2f` | [editor: Add language detection for untitled buffers (#61201)](https://github.com/zed-industries/zed/commit/690c7fac64a4) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:betlang, dep:language_detection, members:crates/language_detection. |
| `ed4d201f507ee28733f52b528244c49c6f5a4daf` | [gpui_web: Only cancel the native context menu when the app claims the right-click (#60892)](https://github.com/zed-industries/zed/commit/ed4d201f507e) | [P19](#p19) | `source-reconciled` | P19: Superseded by eeac1cfa2110 revert. |
| `a0dc9fd13b4cdaaf3ce707f40d634881a9fce60e` | [acp: Validate registry agent checksums (#61334)](https://github.com/zed-industries/zed/commit/a0dc9fd13b4c) | [H10](#h10) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `5c215b36130799f23715e66b31b253a0b294a5b6` | [Revert "editor: Add language detection for untitled buffers (#61201)" (#61351)](https://github.com/zed-industries/zed/commit/5c215b361307) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:betlang, dep:language_detection, members:crates/language_detection. |
| `914e1c9873b6b85bfeedc5b58e8270885bdd532e` | [gpui_macos: Retain pasteboard objects (#61024)](https://github.com/zed-industries/zed/commit/914e1c9873b6) | [P05](#p05) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `2e2fb0a21811ce2dbaca5bb738b0be8f94de051e` | [zed: Unify dependencies (#61381)](https://github.com/zed-industries/zed/commit/2e2fb0a21811) | [P24](#p24), [W-WINDOWS](#w-windows), [W-WGPU](#w-wgpu), [D-APPLE-PACKAGES](#d-apple-packages) | `source-reconciled` | Retained per-package declaration contract for block/cbindgen/core-graphics/core-text/etagere/foreign-types/pathfinder_geometry/swash; actual Metal header/font/atlas owners remain. cbindgen 0.29 and etagere 0.3 differ from official intermediates; swash requirement 0.2.6 resolves 0.2.9 root / 0.2.10 independent, not exact release parity. |
| `875042b698fbfdfb38c4db493419ec8abdd5c8cf` | [cloud_api_client: Implement Wasm support (#61328)](https://github.com/zed-industries/zed/commit/875042b698fb) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:yawc. |
| `54f2b5df1db73e13e70b79dbc7b5e93a595c0f41` | [Open relative file links at line numbers (#61006)](https://github.com/zed-industries/zed/commit/54f2b5df1db7) | [H09](#h09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f15a3ef4529d20ac3779d6c384417d99203c6de7` | [acp: Update agent-client-protocol to 1.3.0 (#61404)](https://github.com/zed-industries/zed/commit/f15a3ef4529d) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `88447e9b9c8a91180ba0f6d7a19aed95a7b24794` | [Fix the macOS tests that hang locally (#61407)](https://github.com/zed-industries/zed/commit/88447e9b9c8a) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: patch:notify, patch:notify-types. |
| `eeac1cfa2110221b7e2e30e5a0cf473733ff2d48` | [Revert "gpui_web: Only cancel the native context menu when the app claims the right-click" (#61403)](https://github.com/zed-industries/zed/commit/eeac1cfa2110) | [P19](#p19) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `dc2a339d5d043da448a3f7ddc7c0a85c63864aad` | [gpui_linux: Fix Wayland serial token tracking (#61454)](https://github.com/zed-industries/zed/commit/dc2a339d5d04) | [P13](#p13) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `75b1c8b71af998ee169cc36141b95e2df1ed5937` | [gpui_web: Support configurable fetch credentials (#61460)](https://github.com/zed-industries/zed/commit/75b1c8b71af9) | [P12](#p12) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `ec3d887507f272119d9fe146c685f0a941d0e798` | [gpui: Defer element arena clears while an enclosing draw is in progress (#60295)](https://github.com/zed-industries/zed/commit/ec3d887507f2) | [C-E](#c-e), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/events.rs:2527/:2545` validates nested WM_PAINT immediately, journals the request and runs DirectManipulation/IME maintenance after the App borrow. Core's stable Rc arena and clear scopes own nested allocation safety; replacing them with upstream's single drawing bool would lose supported nested windows. |
| `547a6cd599a1649cb78d32447185a993f97a6c85` | [editor: Fix right-click context menus failing to appear at pixel-snapped scroll positions (#61348)](https://github.com/zed-industries/zed/commit/547a6cd599a1) | [C-I](#c-i), [G-M8](#g-m8) | `implemented-local-verified` | Historical baseline finding: M8: public test-support Window::set_scale_factor absent; coherent physical geometry controls are different low-level test seams. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `dc682ef12096b799a7e2d247db842ec0219f3bbd` | [Print Asset Error with the whole context (#61482)](https://github.com/zed-industries/zed/commit/dc682ef12096) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d61edb0825ae9a8dcc06c4f30547f4a1348f4a8f` | [Fix incorrect array merging in LSP initialization_options (#54950)](https://github.com/zed-industries/zed/commit/d61edb0825ae) | [H09](#h09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `dd3a94e2e51f2c807ccb3f6a426c017534441fd6` | [client: Extract proxy handshakes into a new proxy_handshake crate (#61427)](https://github.com/zed-industries/zed/commit/dd3a94e2e51f) | [D-ROOT-PRODUCT](#d-root-product) | `outside-framework` | Product transport extraction: proxy_handshake member/key, piper 0.2 and direct tokio-socks removal belong to Zed client. Independent Apache licensing does not imply a selected GPUI consumer; retained HTTP transitive piper/tokio-socks are accounted for. |
| `8c7811ea7291dbfb59bf959400f385565e3dae77` | [Split VSCode and Zed keymap files (#61532)](https://github.com/zed-industries/zed/commit/8c7811ea7291) | [C-I](#c-i), [G-M1](#g-m1) | `implemented-local-verified` | Historical M1: base NoAction skipped source-rank suppression. Current resolver preserves suppression/context/load ordering; F-ALL passes. L-CORE. |
| `984e3bd0ce17a3bcc6503adee4f32f68611651ff` | [acp: Update agent-client-protocol SDK to 2.0.0 (#61570)](https://github.com/zed-industries/zed/commit/984e3bd0ce17) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `c16d19c94dbc1c62752b736186ab8fd6f1cd25c6` | [gpui: Fix deadlock in performance profiler and reenable it (#61584)](https://github.com/zed-industries/zed/commit/c16d19c94dbc) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `50e399332c120cc67a616fd637888bffc70e7906` | [gpui_windows: Only flash the window once on request_attention (#61611)](https://github.com/zed-industries/zed/commit/50e399332c12) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/window.rs:8981` uses FLASHW_ALL and uCount=1, with no FLASHW_TIMERNOFG. |
| `58df5a14ce946fa27b14194d9d2fce8f87b1c331` | [gpui: Fix images not being drawn with rounded corners when using ObjectFit::Cover (#61383)](https://github.com/zed-industries/zed/commit/58df5a14ce94) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `826f28eb8ffa661a0d1cdf639104d4c9131e8aa4` | [gpui_windows: Align inactive GPUI popup behavior on Windows with other platforms (#61411)](https://github.com/zed-industries/zed/commit/826f28eb8ffa) | [G-GW4](#g-gw4), [G-GW5](#g-gw5), [W-WINDOWS](#w-windows) | `implemented-local-verified` | Historical baseline finding: One-shot nonactivating first appearance and lifetime activation policy have source owners in `window.rs`; IME's GetFocus restriction and kind-specific WS_EX_TOPMOST default are missing. WM_MOUSEACTIVATE is adapted through explicit accepts_activation/focus_on_click and current provisional/native admission. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `e191dd00c7e36cfcbbdccad2babca096067777c0` | [Fix crash when dumping debug buffers in a collab session (#61616)](https://github.com/zed-industries/zed/commit/e191dd00c7e3) | [C-R](#c-r) | `source-reconciled` | InputRateTracker monotonic elapsed handling retained in current :2156 owner. |
| `a50a292fd179f5a4391ee1442b00741a9925cc0f` | [gpui: Fix dropped frames when scrolling an unfocused window (#61600)](https://github.com/zed-industries/zed/commit/a50a292fd179) | [C-R](#c-r) | `source-reconciled` | Window frame_admission consumes high_rate_input independently of active state; source moved to window/frame_admission.rs. |
| `c214057e086517920b214800725c5d16294ddf0d` | [gpui_wgpu: Fix bidi paragraph shaping crash (#61651)](https://github.com/zed-industries/zed/commit/c214057e0865) | [G-GW2](#g-gw2), [W-WGPU](#w-wgpu), [D-TEXT](#d-text) | `implemented-local-verified` | unicode-bidi prerequisite adopted through Bidi_Class=B paragraph/source rebasing. Normalized corpus and mixed_direction_paragraphs U+001C/Hebrew workload complete and compiled in F-BENCH. L-BENCH; no timing claim. |
| `30730a305ae235f3be44643d5895e142048ef701` | [fs: Fix crash loop from closedir panic during worktree scan (#59953)](https://github.com/zed-industries/zed/commit/30730a305ae2) | [D-ROOT-PRODUCT](#d-root-product) | `outside-framework` | Product-only rustix 1.1/fs addition implements crates/fs RealFs eager read_dir/closedir policy. No selected direct owner; transitive rustix used by async-io/calloop/Wayland does not adopt this product patch. |
| `63b52a7ee354741a7485e8a20ab087a485fcb8a1` | [gpui: Make HSLA functions const (#61682)](https://github.com/zed-industries/zed/commit/63b52a7ee354) | [C-F](#c-f), [G-M7](#g-m7) | `implemented-local-verified` | Historical baseline finding: M7: hsla and opaque_grey remain non-const. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `865318723ec45d1ef66ecf790a57e7dba2344061` | [reqwest_client: Allow configuring a per-read idle timeout (#61753)](https://github.com/zed-industries/zed/commit/865318723ec4) | [H10](#h10) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c97b7c0ea41acf56e9726964eca10c2078e921b0` | [gpui_web: Fix some bugs (#61707)](https://github.com/zed-industries/zed/commit/c97b7c0ea41a) | [P19](#p19), [D05](#d05), [W-WGPU](#w-wgpu) | `source-reconciled` | `G/Cargo.toml` keeps matching current wasm-bindgen-futures/js-sys/web-sys declarations and HtmlCanvasElement support. Local literals versus upstream workspace aliases are packaging differences. |
| `8e4e5a39eed932d943cb7a3e7078885b75d1ea54` | [Match the macOS app appearance to the selected theme (#58902)](https://github.com/zed-industries/zed/commit/8e4e5a39eed9) | [C-R](#c-r), [P02](#p02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `b2131e9df8aa16d7d287f0b098df3088310d00d6` | [gpui_web: Support Fetch requests from background workers (#61782)](https://github.com/zed-industries/zed/commit/b2131e9df8aa) | [C-X](#c-x), [P12](#p12) | `source-reconciled` | Only image example client selection in this pathset; Web Fetch owner belongs to platform lane. |
| `1102219f812293a09da88eff1df95fbcedde6b09` | [markdown_preview: Open Markdown links in preview and scroll to linked headings and positions (#61784)](https://github.com/zed-industries/zed/commit/1102219f8122) | [H09](#h09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `e4ac280d48afa1a64d4fa59108210ab94ed8ec02` | [Move OpenAI subscription code to separate crate (#61787)](https://github.com/zed-industries/zed/commit/e4ac280d48af) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:openai_subscribed, members:crates/openai_subscribed. |
| `424a68244aa9b8ac9d4766e51b0824d2b2174bd7` | [gpui_web: Point wasm_thread at a fork that fixes the wasm-bindgen init call (#61807)](https://github.com/zed-industries/zed/commit/424a68244aa9) | [P12](#p12) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `06b6160d46ae8a9074cd367ed64f742b47beca64` | [gpui_macos: Drop legacy pre-macOS 12 blur path and its private CGS APIs (#56315)](https://github.com/zed-industries/zed/commit/06b6160d46ae) | [P02](#p02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `82aef44308540b576e4e51fb379efa71614e5c91` | [gpui: Add an abstraction for scheduling foreground work for idle periods, and implement it for the web (#61827)](https://github.com/zed-industries/zed/commit/82aef4430854) | [C-R](#c-r), [P12](#p12), [H01](#h01) | `source-reconciled` | executor.rs:415 spawn_when_idle and :446 idle_time_remaining; platform dispatch forwards with profiler refusal accounting. |
| `5ccbbbd88f74a6283241db699ea00cb14f7e5a7f` | [gpui: Add `grid_rows_min_content` and `grid_rows_max_content` methods (#61837)](https://github.com/zed-industries/zed/commit/5ccbbbd88f74) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `dc1e815e47835095748cbb036541992abd9ac826` | [gpui: Complete pending key bindings before IME input (#61270)](https://github.com/zed-industries/zed/commit/dc1e815e4783) | [C-I](#c-i), [P06](#p06) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `a11083f9a79495e9c7ddee0c5782f22d07695c31` | [gpui: Defer appearance change callback to avoid reentrant borrow (#61789)](https://github.com/zed-industries/zed/commit/a11083f9a794) | [C-R](#c-r), [P02](#p02) | `source-reconciled` | AppearanceChanged uses native ingress then post-borrow deliver_window_event at app/native_event_ingress.rs:1778. |
| `f52fd9ac44b298c089491d9920daa22964c32cc8` | [Add macOS file drag-out from project panel (#58161)](https://github.com/zed-industries/zed/commit/f52fd9ac44b2) | [C-E](#c-e), [P17](#p17) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `36911f8cabc0f76f611dedf7c760b08a22b6cdf4` | [docs: Clarify `window_decorations` currently only works on Linux (#61883)](https://github.com/zed-industries/zed/commit/36911f8cabc0) | [C-F](#c-f), [P24](#p24) | `source-reconciled` | Platform decoration documentation only; no runtime change. |
| `a473ea63a8bc199a73a7e44de3e1d9252e3ca895` | [image_viewer: Fix texture leak when reloading images (#58803)](https://github.com/zed-industries/zed/commit/a473ea63a8bc) | [C-E](#c-e), [P23](#p23), [G-M9](#g-m9) | `implemented-local-verified` | Historical baseline finding: M9: App::has_asset present; ImageSource/Image cache queries and Window::has_image_atlas_entry absent. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `c9d1d0ddfec3c4e5c75e67b13a79a2ab2c76f687` | [fuzzy_nucleo: Remove `util` dependency (#61929)](https://github.com/zed-industries/zed/commit/c9d1d0ddfec3) | [H09](#h09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `a6a23c7b80a5cefa0487b7856335be89ace7e483` | [fuzzy: Remove `util` dependency (#61933)](https://github.com/zed-industries/zed/commit/a6a23c7b80a5) | [H09](#h09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `a8491e63b54bf1881e10fe78bb3e7b1f8a5a2cac` | [gpui: Restore file drags when re-entering the source window on macOS (#61940)](https://github.com/zed-industries/zed/commit/a8491e63b54b) | [C-I](#c-i), [P17](#p17) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `79cc17c216cf62d5deec7b3eed986d0f652d1c9a` | [gpui: Give divs (the option for) a better scrolling experience (#61797)](https://github.com/zed-industries/zed/commit/79cc17c216cf) | [C-I](#c-i) | `source-reconciled` | Div scroll owner and gestures.rs retain smoothing/ongoing scroll; followup platform physics replaces original policy. |
| `ae99a867d7a24682435bd1821c66b4e172a10768` | [x11: Request repaint after exposure instead of within blocked loop (#61162)](https://github.com/zed-industries/zed/commit/ae99a867d7a2) | [C-F](#c-f), [P14](#p14) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `9a631e5461194bbdbbd5b4a8d1b9236a211494ee` | [Fix `path`'s license (#61955)](https://github.com/zed-industries/zed/commit/9a631e546119) | [H07](#h07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `e99616cdd4663ac5d29ae92bbd7ea1629689d4c8` | [gpui: Add resizable and minimizable state to Window (#61473)](https://github.com/zed-industries/zed/commit/e99616cdd466) | [C-F](#c-f), [P02](#p02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `2610332077ee05f4c9e2e1caa7d80a4dcde3deb6` | [path: Normalize Windows paths independently of the host platform (#61374)](https://github.com/zed-industries/zed/commit/2610332077ee) | [H07](#h07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c7aea6cbbd43a5849c4f8be6cbd74789e9ca48c6` | [gpui_linux: Add wl_data_source support for outbound drags on Wayland (#61947)](https://github.com/zed-industries/zed/commit/c7aea6cbbd43) | [P16](#p16) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `21f16f7b5b968092cf4cd9cb38684a2854834fda` | [Speed up `cargo build -p zed` compilation (#62059)](https://github.com/zed-industries/zed/commit/21f16f7b5b96) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:git_ui_core, members:crates/git_ui_core. |
| `8886dcb0d4ea0e145e4512d415d3260602eca99f` | [gpui: Expose threaded dispatcher for tests (#62096)](https://github.com/zed-industries/zed/commit/8886dcb0d4ea) | [C-R](#c-r), [P22](#p22) | `source-reconciled` | platform/threaded_dispatcher and public test-support reexport present; BenchAppContext uses as_threaded. |
| `849ec5898a321eefbeb1d1beda130cc50ef43f10` | [agent: Fix NTFS warning breaking terminal tool when WSL not available (#62049)](https://github.com/zed-industries/zed/commit/849ec5898a32) | [W-SHARED](#w-shared), [D-SOURCE](#d-source) | `source-reconciled` | Root source reconciled through the linked owning audit: dep:windows-registry. Root adds windows-registry 0.6.0. Backend used APIs are get/open/create/set_string and SDK discovery; fork keeps per-crate 0.5. This remains a dependency-version difference, not complete package equivalence. Agent NTFS/WSL product checks are outside this backend. |
| `be8c6f9fb356dcd40a7ff06149568753e64ee171` | [Tweak renderer resource management (#62069)](https://github.com/zed-industries/zed/commit/be8c6f9fb356) | [C-F](#c-f), [P08](#p08), [W-WINDOWS](#w-windows), [W-WGPU](#w-wgpu) | `source-reconciled` | `W/src/directx_renderer.rs:1260/:1331` uses whole-buffer SRVs with batch start constants and bounded capacities. `G/src/wgpu_renderer.rs:2007/:2430` prewrites primitive arrays, separates texture binding and returns fallible capacity errors. Host recording retains its own uploaded buffers. |
| `4601ead4164ae56828bfd7d4566f918d4b337aaa` | [gpui: Webgl backend (#62165)](https://github.com/zed-industries/zed/commit/4601ead4164a) | [P24](#p24), [W-WGPU](#w-wgpu) | `source-reconciled` | `G/src/wgpu_context.rs:200` selects Auto/WebGpu/WebGl with a real web display/surface and compatible adapter. `wgpu_renderer/gpu_data.rs` implements Rgba32Uint transport, aligned first instances, row/tail writes and storage mode. `shaders_webgl.wgsl` decodes every current primitive; WebGL disables dual-source blending and uses required downlevel limits/RGBA atlas. |
| `300972bea37c8cc1c67804c581f62ae6ab4c040e` | [gpui: Add async task benchmark support (#62180)](https://github.com/zed-industries/zed/commit/300972bea37c) | [C-R](#c-r), [P22](#p22) | `source-reconciled` | BenchAppContext::bench_task :693 retains traced setup/output/drop and non-Send foreground tasks. |
| `655ed1385b00c01386bfbd9b7edea439eb1eec64` | [Prevent inactive windows from updating the IME position on linux/Wayland (#62086)](https://github.com/zed-industries/zed/commit/655ed1385b00) | [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `8e18ab0cd7cd8471a84d8d337bedeaefadd79a1e` | [util: Fix stdin redirect for ShellBuilder (#62078)](https://github.com/zed-industries/zed/commit/8e18ab0cd7cd) | [H06](#h06) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `38df25d54c6f3b3ad8b94312405a53978fd496fc` | [gpui: Check run_until readiness between main-thread runnables (#62220)](https://github.com/zed-industries/zed/commit/38df25d54c6f) | [P22](#p22) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `a8cae3bd77f6e1c1dde98bb1dcebba0d254dbd71` | [gpui: Prevent multi-modifier gestures from dispatching standalone modifiers (#62036)](https://github.com/zed-industries/zed/commit/a8cae3bd77f6) | [C-I](#c-i) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `00cba838ad4e0be4b6176438551b72b2d512e9f8` | [markdown: Mermaid zoom (#62115)](https://github.com/zed-industries/zed/commit/00cba838ad4e) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `82878540b5410b288a2c92cb9ee5675533e4d807` | [gpui: Bound benchmark frame servicing to the tasks queued at frame start (#62235)](https://github.com/zed-industries/zed/commit/82878540b541) | [C-R](#c-r), [P22](#p22) | `source-reconciled` | BenchAppContext::run_until :642 and bounded run_ready_main_tasks_with; queue len retained. |
| `d0f797a38c751cff745108e458bbc5ec73c70738` | [wgpu: Only store embedded fonts once in memory (#62192)](https://github.com/zed-industries/zed/commit/d0f797a38c75) | [W-WGPU](#w-wgpu) | `source-reconciled` | `G/src/cosmic_text_system.rs::add_fonts` stores Binary(Arc<Cow<...>>) sources, preserving one embedded allocation. The local explicit fallback search loop is equivalent to position(). |
| `c24358d96cdb4ce14ecbc088462295353b0103f0` | [deps: Bump pathfinder_simd & fix upcoming rustc warnings (#62170)](https://github.com/zed-industries/zed/commit/c24358d96cdb) | [D03](#d03) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `cc053a4a6fa2fd0e8793201ed9099466af1be0b1` | [gpui: Expose accessibility identifiers to platform clients (#61926)](https://github.com/zed-industries/zed/commit/cc053a4a6fa2) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `417d03305ac194470f5d85a4b584c34dcec8cf57` | [Gpui web dont repaint (#62327)](https://github.com/zed-industries/zed/commit/417d03305ac1) | [C-F](#c-f), [P19](#p19) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `4ed3738c4068495e12d4d78044b005e5ed9d3b8c` | [gpui: Add synced repeating animations and phase-lock spinners (#62332)](https://github.com/zed-industries/zed/commit/4ed3738c4068) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `38ca9106c5306ef93e52c35643df015a27f15b72` | [gpui: Reconcile hover listeners after layout changes (#62335)](https://github.com/zed-industries/zed/commit/38ca9106c530) | [C-I](#c-i) | `source-reconciled` | record_interaction_membership accepted-fence reconcile at elements/div.rs:4933, with pending press and refusal preservation. |
| `59b2ebf10351b5c0b5cd4403f01ed0460eeec06d` | [gpui: Fix `img` aspect ratio overriding existing value (#62367)](https://github.com/zed-industries/zed/commit/59b2ebf10351) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `1271f8b0e8f3278eed5dd3fc12ad4bd30dce2c5d` | [Bump rustc to 1.97 (#62395)](https://github.com/zed-industries/zed/commit/1271f8b0e8f3) | [C-X](#c-x), [P08](#p08), [H16](#h16), [D06](#d06) | `source-reconciled` | Example syntax/toolchain change; no runtime mechanism here; fork compiler floor is separately owned. |
| `9e236090b9a31338caf233d440f724922b58d7e1` | [gpui: Add per-window frame-duration and present-interval histograms (#61871)](https://github.com/zed-industries/zed/commit/9e236090b9a3) | [C-R](#c-r) | `source-reconciled` | profiler.rs FrameDurationSnapshot :1135 and Window frame_duration_snapshot :8824 retain histogram mechanism. |
| `d71f1461045c098dc6ca6b1b5adcf1b8949722e8` | [Bump `stacksafe` (#62468)](https://github.com/zed-industries/zed/commit/d71f1461045c) | [D-TEXT](#d-text) | `source-reconciled` | Adopted stacksafe 1.0 prerequisite: root/lock 1.0.2 is actually consumed by Core stacker, Taffy measurement/layout callbacks, Div phases and randomized tree. Baseline source reconciliation only. |
| `992c7d469c392c97b48710eab7776ff1850b1f7b` | [http_client: Bound GitHub release requests (#62175)](https://github.com/zed-industries/zed/commit/992c7d469c39) | [H10](#h10) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `cdf33ac25e3c9b65294d22a027e50eca6e8ab53a` | [markdown: Auto-size table columns to content width (#61773)](https://github.com/zed-industries/zed/commit/cdf33ac25e3c) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `93f6b2e59725de4ac54c73867f69d3bb00091673` | [languages: Avoid probing unresolved macOS Python shim (#62534)](https://github.com/zed-industries/zed/commit/93f6b2e59725) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:pet, dep:pet-conda, dep:pet-core, dep:pet-fs, dep:pet-poetry, dep:pet-reporter, dep:pet-virtualenv. |
| `fc952d52dab0920732b01d031d7324e664e6734f` | [gpui: Add track_caller to gpui_util::log_err (#62538)](https://github.com/zed-industries/zed/commit/fc952d52dab0) | [H09](#h09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `ff9f114cd39692450ef7646db1198f54417531b8` | [gpui: Add binary data support to `Svg` element (#52319)](https://github.com/zed-industries/zed/commit/ff9f114cd396) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `dd04a229dd22700ff43815b8514453e197c333b7` | [gpui: Allow setting max FPS for `Animation` (#62579)](https://github.com/zed-industries/zed/commit/dd04a229dd22) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `0307288d903afd5673c361c548bd448fc8a684df` | [gpui: Settle benchmark app state between task iterations (#62587)](https://github.com/zed-industries/zed/commit/0307288d903a) | [C-R](#c-r), [P22](#p22) | `source-reconciled` | BenchAppContext::settle :622 alternates dispatcher draining and App updates. |
| `47825fe00a8f8aba46a7552fcdf99918bad8cb21` | [Properly measure invisible character replacement (#62478)](https://github.com/zed-industries/zed/commit/47825fe00a8f) | [H02](#h02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `a21007b7a948e46afbe719150f5e9968bfcd1078` | [gpui: Unify performance tracking under the profiler feature (#62496)](https://github.com/zed-industries/zed/commit/a21007b7a948) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `52b2418110ee6c7a67c52398980357b2c15609e9` | [Extract shared Apple renderer from gpui_macos (#62649)](https://github.com/zed-industries/zed/commit/52b2418110ee) | [P08](#p08), [D04](#d04) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `9bde578ef5afa84920c4300af25f9dee31c96fcf` | [acp: Poll the client connection future on a dedicated thread (#62259)](https://github.com/zed-industries/zed/commit/9bde578ef5af) | [C-R](#c-r) | `source-reconciled` | Introduction removed by 3cef31688ae2; complete scheduler dedicated owner remains separately implemented. |
| `b2d9c2e122fbc408d42276b4456243ba4f90f181` | [Fixed the zoomed agent panel closing unexpectedly on rewind (#62711)](https://github.com/zed-industries/zed/commit/b2d9c2e122fb) | [C-F](#c-f) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `b47d8ac455a31b2c8827a47f78076d394430cc18` | [Merge array settings from extension contributions instead of overwriting (#62686)](https://github.com/zed-industries/zed/commit/b47d8ac455a3) | [H09](#h09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `511ac170363776319b38cc0e9c047a06aa2e7541` | [gpui: Make inactive frame throttling configurable (#62628)](https://github.com/zed-industries/zed/commit/511ac1703637) | [C-R](#c-r), [P10](#p10) | `source-reconciled` | WindowOptions inactive_frame_interval :8231 and window/frame_admission.rs consume configured policy. |
| `eb5483528be607b497afdaad0a237bebeb191249` | [Fix overflowing highlights in the markdown blocks (#62714)](https://github.com/zed-industries/zed/commit/eb5483528be6) | [C-E](#c-e), [G-M10](#g-m10) | `implemented-local-verified` | Historical baseline finding: M10: painted_quads test-support API absent; bench-only rendered_scene is not available under plain test-support. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `8b1497dbd22fb06f5838a7c0b84a1e54fafa71bc` | [gpui: Add spring animations; examples (#62778)](https://github.com/zed-industries/zed/commit/8b1497dbd22f) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `7bddd16a09cf0084cefb3d98468b178343b9f1e2` | [gpui: Fix `TestWindow` panicking on `window_handle` (#62775)](https://github.com/zed-industries/zed/commit/7bddd16a09cf) | [C-R](#c-r), [P11](#p11) | `source-reconciled` | TestWindow HasWindowHandle/HasDisplayHandle in platform/test/window.rs:410/:418 return NotSupported; no raw handle fabrication. |
| `07a0bd1220bddb940028d610e40319a459dd49fb` | [gpui: Frame time debug overlay (#62749)](https://github.com/zed-industries/zed/commit/07a0bd1220bd) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `2893b86b048a6e5877eb2f61dac97c172460ba07` | [gpui_macos: Add simple fullscreen mode that covers the notch (#60020)](https://github.com/zed-industries/zed/commit/2893b86b048a) | [C-F](#c-f), [P02](#p02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `05473ed83d2bada157e89f181e15c205a3b22163` | [Rename `csv_preview` crate to `tabular_data_preview` (#62768)](https://github.com/zed-industries/zed/commit/05473ed83d2b) | [W-SHARED](#w-shared), [D-SOURCE](#d-source) | `outside-framework` | Root source reconciled through the linked owning audit: dep:csv_preview, dep:tabular_data_preview, members:crates/csv_preview, members:crates/tabular_data_preview. Outside scoped runtime: reorders the monorepo windows_resources member during a product crate rename. The backend compiles its own HLSL via build.rs; no missing native menu/renderer mechanism follows from that workspace-member order. |
| `a7d74150ac7a663fdaa01ec5177e303baaf6c331` | [Fix the `git_gutter_width` setting (#62704)](https://github.com/zed-industries/zed/commit/a7d74150ac7a) | [D-ROOT-PRODUCT](#d-root-product) | `outside-framework` | Product-only derive_more From feature for settings/editor/gutter consumers. The patch has no selected owner hunk and current Core uses its retained derive family without those From derives. |
| `03c9c4e7070a117bc93aa807b60ce8bd354f6a82` | [gpui: Rasterize parsed SVGs at exact sizes (#62770)](https://github.com/zed-industries/zed/commit/03c9c4e7070a) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `87324045be8738f1322409e99135167b233133f1` | [Switch to `async-tar` fork (#62821)](https://github.com/zed-industries/zed/commit/87324045be87) | [D02](#d02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `2040e0de59db363748e2299c8043beba7c870da5` | [treesitter: Worker-pinned treesitter parsing (#62784)](https://github.com/zed-industries/zed/commit/2040e0de59db) | [C-R](#c-r), [P12](#p12), [H01](#h01), [D-PRODUCT](#d-product) | `source-reconciled` | Root evidence: dep:tree-sitter, patch:tree-sitter-language. |
| `3f660a0a60a5c1ae6200eef4dcb50120f5a4e84f` | [Remove deprecated std::usize import from uniform_list.rs (#62841)](https://github.com/zed-industries/zed/commit/3f660a0a60a5) | [C-E](#c-e) | `source-reconciled` | No behavior; deprecated std::usize import absent. |
| `7a7c3e1d2f03195c5fa19bc890da330ad7f3abef` | [Restart interrupted update downloads after system wake (#60366)](https://github.com/zed-industries/zed/commit/7a7c3e1d2f03) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c43e2d9734800bef0dd216ea9184fdb99bc60625` | [gpui_linux: Handle XKB context initialization failure (#62868)](https://github.com/zed-industries/zed/commit/c43e2d973480) | [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `45ae0572c422e5e8dcf73157730d747152d0be52` | [gpui_web: Stream Fetch response bodies (#62333)](https://github.com/zed-industries/zed/commit/45ae0572c422) | [P12](#p12) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d70c45e5bb1d8b7b3926fba5a6d1c69c23bbc405` | [gpui: Support image paste and async clipboard reads on web (#62871)](https://github.com/zed-industries/zed/commit/d70c45e5bb1d) | [C-I](#c-i), [P12](#p12) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `fa00dccc42311f8dc71c533105488b0dbd518138` | [Fix project path handling when connecting from Unix to Windows remotes (#62038)](https://github.com/zed-industries/zed/commit/fa00dccc4231) | [H07](#h07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `715076597926d566d560b30b806749e1a2166080` | [Preserve `--user-data-dir` across restarts (#62022)](https://github.com/zed-industries/zed/commit/715076597926) | [C-R](#c-r), [P18](#p18), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/restart.rs` owns UTF-16/native-argv quoting and one-shot arguments; `platform.rs:2734` defers process creation past App borrow and preserves terminal quit. Exact Env deletion supersedes the known official PowerShell clearing defect. Product user-data selection is outside. |
| `242fe31a399f695103dd0cfe7dcef33a59fbf596` | [gpui_macos: Fix fullscreen behavior on non-macOS and after restart (#62819)](https://github.com/zed-industries/zed/commit/242fe31a399f) | [P02](#p02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `2936989f1b7a15aaf7131b0a3c17961d706fdbf5` | [gpui: Add split and paint APIs on LineLayout (#60831)](https://github.com/zed-industries/zed/commit/2936989f1b7a) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `1861e58f984c76afc06032e753557994ffc8fe44` | [gpui: Journal foreground work between frames and report hang incidents (#62779)](https://github.com/zed-industries/zed/commit/1861e58f984c) | [C-I](#c-i) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `4d1935b8d00320841da476ec38c7b7747abf6f09` | [gpui_linux: Clear X11 urgency hint when the window becomes active (#61619)](https://github.com/zed-industries/zed/commit/4d1935b8d003) | [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `282f47a544b72030c8a162955415c8def655d76a` | [Switch from `cargo-machete` to `cargo-shear` (#62643)](https://github.com/zed-industries/zed/commit/282f47a544b7) | [P24](#p24), [H16](#h16), [W-WINDOWS](#w-windows), [W-WGPU](#w-wgpu), [D-EXTRACT](#d-extract) | `source-reconciled` | Retained standalone test/doc and cargo-machete contract; all removed direct root zed/brotli/cfg-if/cocoa-foundation/futures-util/pathdiff aliases are absent. Actual fork headless/browser consumers retain needed pollster/wasm dependencies; no whole transitive-package removal is claimed. |
| `f4178619acd0d47ea1f76a2025c42962c6d6638c` | [gpui_linux: Drain buffered X11 events after foreground work (#62081)](https://github.com/zed-industries/zed/commit/f4178619acd0) | [P14](#p14) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `fe9556a11e9cc9dbc78686041aa524d6932879db` | [gpui: Map `ztracing` events to browser `performance` APIs on web (#62898)](https://github.com/zed-industries/zed/commit/fe9556a11e9c) | [H15](#h15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `eb354c8d504071bdb79110a7a5c9d374c2864113` | [gpui: Make the Wayland render loop demand-driven (#60690)](https://github.com/zed-industries/zed/commit/eb354c8d5040) | [C-R](#c-r), [P14](#p14) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `ec18126b1dbd32b089e51d7edee1e20b3bd53637` | [mermaid_render: Update merman to `0.8.0-alpha.5` (#62931)](https://github.com/zed-industries/zed/commit/ec18126b1dbd) | [H15](#h15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `7316cf77459a1dbc62979beffedeeeaa8e91389a` | [gpui: Report foreground executor work in bench reports (#63035)](https://github.com/zed-industries/zed/commit/7316cf77459a) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d9ad6aff67e47de43abb270d22de75dd950f1b48` | [gpui_linux: Release X11 client state before close callback (#63089)](https://github.com/zed-industries/zed/commit/d9ad6aff67e4) | [P14](#p14) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `7f2a2c3c3ee2f23f28772dee7661fb98d3910990` | [gpui: Support Windows Restart Manager shutdown (#62987)](https://github.com/zed-industries/zed/commit/7f2a2c3c3ee2) | [C-R](#c-r), [P18](#p18), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/platform.rs:4033/:4067`, application-window forwarding and current quit callbacks retain confirmed session shutdown, canceled-session no-op, synchronous bool, flush and deferred fallback. Exact native terminal acknowledgement and ExternalHost ownership remain authoritative. |
| `6e2fae619c45ffc90e5bcf5cfbfcef8bb693fbe1` | [Update Wasmtime WASI to fix filesystem sandbox escape (#63140)](https://github.com/zed-industries/zed/commit/6e2fae619c45) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:wasmtime-wasi. |
| `f1d27d545e79f98ffde7ebf3229f8eb8bad791aa` | [Prewarm Linux font match caches (#63158)](https://github.com/zed-industries/zed/commit/f1d27d545e79) | [C-T](#c-t), [P15](#p15), [W-WGPU](#w-wgpu) | `source-reconciled` | `G/src/cosmic_text_system.rs:310/:334` shares FontMatchProperties between shaping and public prewarming, including primary plus configured fallback attributes; local test observation does not replace the actual get_font_matches calls. |
| `4ca9005579ad56c3198d1d6a8583825e97921b13` | [Bump tree-sitter to 0.27 and wasmtime to 48.0 (#63169)](https://github.com/zed-industries/zed/commit/4ca9005579ad) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:tree-sitter, dep:wasmtime, dep:wasmtime-wasi, patch:tree-sitter-language. |
| `41d72f6c1d9038f48a6e97d0f6de02f72625aaf0` | [gpui: Make stacker opt-in (#63187)](https://github.com/zed-industries/zed/commit/41d72f6c1d90) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `224373ab888e873e19ad901dc0a8d4749ef032a3` | [gpui: Web text input improvements (#63201)](https://github.com/zed-industries/zed/commit/224373ab888e) | [P19](#p19) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `1ff7cb669b6351eaf2a5b79ccb40f5cd1676b8f8` | [gpui_wgpu: Reuse glyph image after measuring bounds (#63209)](https://github.com/zed-industries/zed/commit/1ff7cb669b63) | [W-WGPU](#w-wgpu) | `source-reconciled` | `G/src/cosmic_text_system.rs:487/:524` consumes the measured Swash image rather than regenerating it; 256-entry / 8 MiB temporary limits evict and recompute without refusing glyphs. |
| `55007f518bc1d49e6b3291c5eaa1aabf649b36fd` | [gpui: Report average dirty-to-present frame timings (#63221)](https://github.com/zed-industries/zed/commit/55007f518bc1) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d4cd310edb56315d81a3eaee9032f0c8b2f8d5c5` | [gpui: Use platform clock in action profiler (#63183)](https://github.com/zed-industries/zed/commit/d4cd310edb56) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `7040aa56693ba7e85241180d031080ca79b725f8` | [gpui_windows: Clear render target before compositing COLR emoji (#63225)](https://github.com/zed-industries/zed/commit/7040aa56693b) | [W-WINDOWS](#w-windows), [Q-COLR](#q-colr) | `implemented-local-verified` | `W/src/color_glyph_bitmap.rs` initializes all channel planes to zero and composes each layer. This replaces the old cleared D3D target, preserving transparent uncovered pixels. Existing color stability/readback tests are migrated by the current owner. |
| `5dd0666dfb627bb7c04b210b730005628f5a229a` | [Fix missing NUL terminator in X11 WM_CLASS (#62848)](https://github.com/zed-industries/zed/commit/5dd0666dfb62) | [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `74490daced609e3e11104ad9d2a877ce6ea39591` | [gpui_windows: Implement render_to_image for headless window capture (#63012)](https://github.com/zed-industries/zed/commit/74490daced60) | [P24](#p24), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/directx_renderer.rs:569/:589` shares draw_scene_to_target, reads a staging texture without presenting and converts BGRA to RGBA. `window.rs` and `renderer.rs` forward test-support captures; current authority/shutdown checks are retained. |
| `4278ff36ef76916c719dfdd1557efd4401206985` | [gpui: Don't start a wrapped line with closing punctuation or a slash (#62743)](https://github.com/zed-industries/zed/commit/4278ff36ef76) | [C-T](#c-t) | `source-reconciled` | Handwritten closing-punctuation heuristic superseded by final UAX #14 segmenter in line_wrapper.rs:548. |
| `1d7e5f1d014acf0d288c6d11be93c31fe647e725` | [gpui_windows: Fix window placement on secondary monitor with different DPI scaling (#62859)](https://github.com/zed-industries/zed/commit/1d7e5f1d014a) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/display.rs` supplies immutable monitor DPI; `window.rs::initial_window_rect_for_physical_client_bounds` uses the target monitor's scale and exact topology observation rather than a newly created HWND's initial DPI. Mixed-DPI source behavior is retained. |
| `5f1f7e634be61a21fa25411a27d210440ad6e1a8` | [gpui_web: Improve mobile keyboard focus handling (#63220)](https://github.com/zed-industries/zed/commit/5f1f7e634be6) | [P19](#p19) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `1f7eae7fb753be69ac8cd80575daf43103c85883` | [gpui: Separate benchmark support from test support (#63039)](https://github.com/zed-industries/zed/commit/1f7eae7fb753) | [C-R](#c-r), [P22](#p22) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `fecc3273ed32643c2ea1b04a74c8780e2c9ffaf8` | [gpui_platform: Expose headless renderer through bench support (#63244)](https://github.com/zed-industries/zed/commit/fecc3273ed32) | [P22](#p22) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c595091b0a0ab18d68cd97862e4748679dfe54d6` | [Bump dependencies to dedupe the crate graph (#63231)](https://github.com/zed-industries/zed/commit/c595091b0a0a) | [H16](#h16), [D02](#d02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `8fc1a8a0c6f40b9bbf7d71997138866a78563486` | [gpui: Text input configuration and web implementation (#63264)](https://github.com/zed-industries/zed/commit/8fc1a8a0c6f4) | [C-I](#c-i), [P19](#p19) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f66ed399cdde86092af8af3dc7b418abf45f37f8` | [gpui: Fix stale pending input when a window is blurred (#63271)](https://github.com/zed-industries/zed/commit/f66ed399cdde) | [C-I](#c-i) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d84e5d4992bd445b8af8eeaf1717f4cf9c5d5a54` | [gpui: Add text_edit_editable_range API (#63294)](https://github.com/zed-industries/zed/commit/d84e5d4992bd) | [C-I](#c-i), [P19](#p19) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `12ef40c31659ddb879bdb39481b7fc9524ac8ffa` | [lsp: Add client support for LSP 3.18 markdown diagnostic messages (#61030)](https://github.com/zed-industries/zed/commit/12ef40c31659) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:lsp-types. |
| `5218009a2d338898083b4789f5c5fd37ddc73b70` | [gpui: IME race condition bug (#63311)](https://github.com/zed-industries/zed/commit/5218009a2d33) | [P19](#p19) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `ec312b27014d9c829afd1c08ce590d66bfcb4fdc` | [Add LSP call hierarchy modal (#53239)](https://github.com/zed-industries/zed/commit/ec312b27014d) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:call_hierarchy, members:crates/call_hierarchy. |
| `4c6c4750d3f87f0b93c937c9bda7b50639c0d07d` | [Improve Windows shell discovery (#63339)](https://github.com/zed-industries/zed/commit/4c6c4750d3f8) | [H06](#h06), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/platform.rs:2734` uses the canonical get_powershell discovery result and `restart.rs` handles absence; discovery strategy lives in core-util as reconciled by the support lane. |
| `669bede5c582140d2f3917e2c5fae5b764f9023f` | [editor: Add language detection for untitled buffers (#61412)](https://github.com/zed-industries/zed/commit/669bede5c582) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:betlang, dep:language_detection, members:crates/language_detection. |
| `956a49e4ca8aa4b7c2c293e1414c91f009824ae3` | [gpui: Touch events (#63373)](https://github.com/zed-industries/zed/commit/956a49e4ca8a) | [C-I](#c-i), [P21](#p21), [D-ROOT-PRODUCT](#d-root-product) | `source-reconciled` | Root settings_json default-feature trim is product-only with no selected consumer. Full portable gestures.rs and window/touch_gestures.rs dispatch and Web touch slices remain reconciled in C-I/P21; the mixed SHA is not excluded as a whole. |
| `ef075910c99ce2c8fd07da4174e7c3bb71513f35` | [gpui_web: Let applications provide fonts (#63493)](https://github.com/zed-industries/zed/commit/ef075910c99c) | [C-X](#c-x), [P20](#p20) | `source-reconciled` | Example font setup only in this pathset; native/browser font production owner outside lane. |
| `98c6c1403b14463d6274b791f5a5471cdb4345f3` | [gpui: Gate blocking executor APIs on WebAssembly (#63484)](https://github.com/zed-industries/zed/commit/98c6c1403b14) | [C-R](#c-r), [H01](#h01) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `a60addb9a96b171563813f936e43dbecc310ca0b` | [gpui_macos: Migrate `PlatformWindow::prompt` to `objc2` on MacOS (#59572)](https://github.com/zed-industries/zed/commit/a60addb9a96b) | [P17](#p17), [D-APPKIT](#d-appkit) | `source-reconciled` | Adopted root NSAlert/NSApplication/block2 features via the explicit macOS member. Typed alert and sheet-completion prompt route is present at the baseline; no fresh native-dialog qualification. |
| `76b1096cbd83b5b5138793e5f552218abc8fdcbb` | [gpui: Platform-specific scroll physics (#63505)](https://github.com/zed-industries/zed/commit/76b1096cbd83) | [C-I](#c-i), [P21](#p21), [G-M12](#g-m12) | `implemented-local-verified` | Historical baseline finding: M12: physics/recognition/setter present; public touch_prediction_enabled getter absent. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `ee6badf4b096b60da1e4725a6c9ddb10392c0487` | [Support building with corgi (#63396)](https://github.com/zed-industries/zed/commit/ee6badf4b096) | [C-F](#c-f), [P08](#p08), [H14](#h14), [D-PRODUCT](#d-product) | `source-reconciled` | Root evidence: patch:scratch. |
| `ce48461eaadd16c65c31f835511ab96bd3b6e746` | [util: Prevent panic when parsing a malformed shell variable (#63446)](https://github.com/zed-industries/zed/commit/ce48461eaadd) | [H06](#h06) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `a66fb6aebdee68ee5ae7d191d9dff0d61a42b834` | [gpui_web: Tighten up web keyboard feel (#63533)](https://github.com/zed-industries/zed/commit/a66fb6aebdee) | [C-I](#c-i), [P19](#p19) | `source-reconciled` | Touch hover suppression retained by last_input_was_touch and Div style checks. |
| `5e28272c1407ced4bae4a90deaea25352a1fbc96` | [gpui: Lock touch scrolling to the dominant axis (#63553)](https://github.com/zed-industries/zed/commit/5e28272c1407) | [C-I](#c-i) | `source-reconciled` | Final gestures.rs locks pan deltas to dominant axis; target algorithm retained with target-local fork projection. |
| `a49de953a850f7ee1f5d847e7908733c07e516ee` | [gpui_linux: Prevent panic while reporting Wayland errors (#63159)](https://github.com/zed-industries/zed/commit/a49de953a850) | [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `0855410ccd2040efbbf14d71409166b6c472e0bd` | [gpui: Add long-press gesture recognition (#63561)](https://github.com/zed-industries/zed/commit/0855410ccd20) | [C-I](#c-i), [P21](#p21) | `source-reconciled` | Portable long-press offer/claim/cancel owner in window/touch_gestures.rs:163 and gestures.rs; API discrepancy tracked by M11. |
| `d3e54c97f4e22c77de7057f010c600d180e15ee6` | [util: Make fs_embed! compatible with rust-embed's debug-embed feature (#63577)](https://github.com/zed-industries/zed/commit/d3e54c97f4e2) | [H14](#h14) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `ac5af8b9e1ea3f7922fbabefe409c05b8766135c` | [Relicense zlog, ztracing, and ztracing_macro under Apache-2.0 (#63573)](https://github.com/zed-industries/zed/commit/ac5af8b9e1ea) | [H15](#h15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `2551721adb5b5187bc27cfae0fbe47f0ed4c5397` | [util: Embed fs_embed assets with debug-embed (#63583)](https://github.com/zed-industries/zed/commit/2551721adb5b) | [H14](#h14) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `97b1e64a177a2fe3c2803e323087b5c2fa6fff1e` | [gpui: Record hang trigger class and set the release frame budget to 24ms (#63586)](https://github.com/zed-industries/zed/commit/97b1e64a177a) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `6b5e15ed463cdcf28b0d1d49c1a681ad044e308b` | [Bump more dependencies (#63379)](https://github.com/zed-industries/zed/commit/6b5e15ed463c) | [D03](#d03), [W-WINDOWS](#w-windows) | `source-reconciled` | Windows/Core bindings are 0.62 and numerics 0.3. Local registry 0.5 versus official 0.6 and accesskit_windows 0.32.1 versus 0.34 are explicit dependency-version differences below, not claimed package parity. Source uses the existing API family; no new missing backend method is inferred. |
| `9decdcc1f22d324b5d5b186a3da1418ec0ea4c37` | [gpui_web: Drive IME mirror focus state from GPUI (#63629)](https://github.com/zed-industries/zed/commit/9decdcc1f22d) | [C-I](#c-i), [P19](#p19) | `source-reconciled` | Accepted handler/session owner updates focused_text_input_active and publishes TextInputStateChange after admission. |
| `a85cf449ec6e72274ee6f9e8462f702299b04cce` | [gpui: Prevent touch mispredict jitter (#63638)](https://github.com/zed-industries/zed/commit/a85cf449ec6e) | [C-I](#c-i) | `source-reconciled` | gestures.rs prediction compensation retained; raw positions classify gestures and estimate velocity. |
| `b3326e13c142fc8f313aca67a93dd6855a1e7e32` | [gpui: Add `TouchDragEvent` API (#63645)](https://github.com/zed-industries/zed/commit/b3326e13c142) | [C-I](#c-i) | `source-reconciled` | TouchDragEvent and offer/resolve/cancellation path at window/touch_gestures.rs:102 retained. |
| `b1a7ef0cf66dfbf9d7661170c96d97c7df916c68` | [gpui: Snap recomputed padding to the device pixel grid before clamping scroll (#62296)](https://github.com/zed-industries/zed/commit/b1a7ef0cf66d) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f0d8b0b0cde9a5b5000017c1e60db3ebea274b88` | [gpui: Add an option to keep hover listeners active after keyboard input (#63614)](https://github.com/zed-industries/zed/commit/f0d8b0b0cde9) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d85eade7ab475de6d11a193beccf1951be2ff371` | [which_key: Add a pending key binding indicator (#63343)](https://github.com/zed-industries/zed/commit/d85eade7ab47) | [C-I](#c-i) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `3ce72bab201ad82418fc9716aad11332624951bc` | [gpui: Add `Hitbox::is_hovered_at` (#63696)](https://github.com/zed-industries/zed/commit/3ce72bab201a) | [C-I](#c-i) | `source-reconciled` | HitboxId::window_point_target_rank at window.rs:564 uses same committed full hit-test order and hover count; exact geometry/clip retained. API spelling differs. |
| `206a863a95086699f2a8784a58b4825af80df46c` | [gpui_web: Update example to use `run_embedded()` instead of `run()` (#63235)](https://github.com/zed-industries/zed/commit/206a863a9508) | [P28](#p28) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `1057c2cf3d5b4aefd04755e1387c7826a4d7fba6` | [Redact environment secrets from dev container `docker exec` logs (#63606)](https://github.com/zed-industries/zed/commit/1057c2cf3d5b) | [H09](#h09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `ab7afbff3942265ec913598b6d1d35027585685b` | [panel: Remove unused panel crate (#63742)](https://github.com/zed-industries/zed/commit/ab7afbff3942) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:panel, members:crates/panel. |
| `c8c07aea5cf82007e524d4fcb31572eb7eeabcd5` | [Add `disable_ai` check for "Agent Panel" `MenuItem` (#63580)](https://github.com/zed-industries/zed/commit/c8c07aea5cf8) | [P27](#p27) | `implemented-local-verified` | Owned TestPlatform menu snapshots, detached get_menus, replacement and clearing implemented; actual App regression passes F-ALL. L-WINDOW. |
| `9bda6b4e0342f22680bc1e7fcd847f4697c48874` | [gpui_web: Disable native canvas touch callouts (#63775)](https://github.com/zed-industries/zed/commit/9bda6b4e0342) | [P19](#p19) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c7801b0c447c1f2cacffaa86b1f0cbdb0d922b6c` | [macos: Honor "Tiled windows have margins" for the Fill titlebar action (#63759)](https://github.com/zed-industries/zed/commit/c7801b0c447c) | [P02](#p02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `13e5c99ab7c2f7c63102061bade5738c96cdd760` | [gpui_web: Disable native canvas selection (#63877)](https://github.com/zed-industries/zed/commit/13e5c99ab7c2) | [P19](#p19) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `4a217d5316887826a21c554650c1ece6d51fff2a` | [gpui_linux: Disable IME in project panel when no input handler on Wayland (#63776)](https://github.com/zed-industries/zed/commit/4a217d531688) | [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `b95b188b59454bc2b01f08a54ab0182c4a55f141` | [gpui: Add scale-factor simulation to `TestWindow` (#63719)](https://github.com/zed-industries/zed/commit/b95b188b5945) | [C-I](#c-i), [P11](#p11), [G-M8](#g-m8) | `implemented-local-verified` | Historical baseline finding: M8: TestAppWindow/TestAppContext/VisualTestContext scale-factor simulation public methods absent. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `63d15474c721581d942851ae42254309aaabd8f5` | [Use fewer file descriptors on macOS to detect worktree root path changes (#63863)](https://github.com/zed-industries/zed/commit/63d15474c721) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: patch:notify, patch:notify-types. |
| `20fa2fa80949e94b6eecb48c8a46fbf32b82f1fc` | [Reduce LLVM IR bloat one last time (#63882)](https://github.com/zed-industries/zed/commit/20fa2fa80949) | [C-V](#c-v), [H09](#h09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `5896274168e06663568f42b5bae142162936e0a6` | [gpui: Replace shared asset futures with owning cache entries (#63934)](https://github.com/zed-industries/zed/commit/5896274168e0) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `0690433b607b3a1a2298ba1e993aa6e3dc341ce0` | [Support LSP command executions and showDocument requests (#63607)](https://github.com/zed-industries/zed/commit/0690433b607b) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:lsp_command_selector, members:crates/lsp_command_selector. |
| `33c6212b055fa8a97b17c8653311c9ccab7fb3b6` | [Restore macOS windows to their original Space after restart (#58886)](https://github.com/zed-industries/zed/commit/33c6212b055f) | [C-F](#c-f), [P02](#p02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `450775244fade36db70af2447be24e74edb43368` | [gpui: Prevent idle sleep during AI response streaming (#53130)](https://github.com/zed-industries/zed/commit/450775244fad) | [C-R](#c-r), [P18](#p18), [D04](#d04), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/activity.rs` owns PowerCreateRequest, SystemRequired activation and RAII clear/close through ActivityGuard. The product's streaming guard consumer stays outside; the generic framework capability is present. |
| `907b55f7eabef9b9c4197daec1bb913efae08ebe` | [gpui_macos: Migrate `register_url_scheme` to `objc2` (#63945)](https://github.com/zed-industries/zed/commit/907b55f7eabe) | [P17](#p17), [D-APPKIT](#d-appkit) | `source-reconciled` | Adopted NSWorkspace feature via the explicit macOS member; typed register_url_scheme completion feeds the oneshot/background result owner at the baseline. |
| `f9a1fc898baee129f051dd41c9af7b9c60a654ac` | [watch: Add snapshot-based channels (#63964)](https://github.com/zed-industries/zed/commit/f9a1fc898bae) | [D-ROOT-PRODUCT](#d-root-product) | `outside-framework` | New arc-swap 1.9.0 root key serves the independent Apache watch snapshot owner. No selected manifest/source consumes that API or arc-swap; exclusion follows closure use, not a guessed product title or GPL claim. |
| `d3865b099eae3e453bc45ca9a8f7bc9879081342` | [gpui_macos: Migrate `prompt_for_path` to `objc2` (#63994)](https://github.com/zed-industries/zed/commit/d3865b099eae) | [P17](#p17), [D-APPKIT](#d-appkit) | `source-reconciled` | Adopted NSOpenPanel/NSPanel/NSSavePanel features via the macOS member, with actual typed open/save panel completion and foreground delivery. |
| `6ad3c7f278e513875e0bbde5ee935f6c9a342b9c` | [gpui_web: Simplify keyboard focus model (#63989)](https://github.com/zed-industries/zed/commit/6ad3c7f278e5) | [C-R](#c-r), [P19](#p19) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `86b2cf96623a12ae3f524c3780b6bcce4443ed23` | [gpui_macos: Migrate `get_sources` to `objc2` (#64031)](https://github.com/zed-industries/zed/commit/86b2cf96623a) | [P17](#p17), [D-APPKIT](#d-appkit) | `source-reconciled` | Adopted NSScreen/ScreenCaptureKit dependency route through typed SCShareableContent completion, native result/error validation and foreground capture-source delivery; macOS member supplies required features. |
| `72c53bf0aae2e619f146d96415f79caf77310455` | [gpui_macos: Migrate `start_external_drag` to `objc2` (#64033)](https://github.com/zed-industries/zed/commit/72c53bf0aae2) | [P17](#p17), [D-APPKIT](#d-appkit) | `source-reconciled` | Adopted NSDraggingItem/NSImage/NSPasteboard features via the macOS member; typed drag items, path UTType/pasteboard writer and beginDraggingSession are in the real drag owner. |
| `3405c42f6756fcf7f3a66ccb12090a59c9dff30a` | [Fix Python interpreter discovery crashes on non-UTF-8 output (#64040)](https://github.com/zed-industries/zed/commit/3405c42f6756) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:pet, dep:pet-conda, dep:pet-core, dep:pet-fs, dep:pet-poetry, dep:pet-reporter, dep:pet-virtualenv. |
| `5a773a406e499a3314ff0ab9b145c63e8489da0e` | [gpui_web: Disable autoscroll (#64041)](https://github.com/zed-industries/zed/commit/5a773a406e49) | [P19](#p19) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `1a84d5d92bd7d6c1cabb116062650af545783fe9` | [project_panel: Fix dragged folders remaining highlighted even after the drag has ended (#64038)](https://github.com/zed-industries/zed/commit/1a84d5d92bd7) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d9e1c024f393832765a03f4de204d6c8cd9abcb2` | [gpui: Long-press to activate tooltips (#63948)](https://github.com/zed-industries/zed/commit/d9e1c024f393) | [C-I](#c-i) | `source-reconciled` | Div LongPress tooltip listener :5217 and terminal pointer cancellation retained. |
| `6fff327a9046ff971c8a93860da2dc8a4c3d5169` | [Bump Rust to `1.98.1` (#63010)](https://github.com/zed-industries/zed/commit/6fff327a9046) | [D06](#d06) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `3cef31688ae2df816c9ce04fc254c172764485e8` | [Upgrade ACP to 2.1 and futures to 0.3.34 (#64077)](https://github.com/zed-industries/zed/commit/3cef31688ae2) | [C-R](#c-r), [D03](#d03) | `source-reconciled` | Removes interim GPUI dedicated-thread helper; current dedicated executor uses shared scheduler owner without duplicate implementation. |
| `d12e456be7c08a1dfaf21bd49ed0df1f3e2a26c6` | [util: Raise open file soft limit at startup on Unix (#64034)](https://github.com/zed-industries/zed/commit/d12e456be7c0) | [H09](#h09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `5b4a2153a087055a7f9cb3464f188f1cab9fd678` | [gpui: List only available font families in suggestions (#64095)](https://github.com/zed-industries/zed/commit/5b4a2153a087) | [C-T](#c-t), [W-WGPU](#w-wgpu) | `source-reconciled` | Cosmic `all_font_names` inventories actual database families; `cosmic_text_system/font_work_tests.rs` preserves empty/new-family/multiple-face scenarios. Stable-versus-unstable sorting is independently GW1, not a missing inventory API. |
| `fb38178d74c871dbfa60e80038ca5c5d646b1cb7` | [gpui: Fix native object accumulation in headless Metal rendering (#64099)](https://github.com/zed-industries/zed/commit/fb38178d74c8) | [P08](#p08) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d2074f4e15d289345a9583ef973470b8bdfad74a` | [gpui_windows: Fix use-after-free in power request reason string (#64001)](https://github.com/zed-industries/zed/commit/d2074f4e15d2) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/activity.rs:22/:50` keeps the exact UTF-16 reason allocation until the kernel request closes; failure-to-close retains that buffer instead of freeing a pointer still held by Windows. |
| `1a89a92e7e26c1ac810666cc1bd752db8c62d93e` | [gpui: Add unpaced frame-driven renderer sessions (#64109)](https://github.com/zed-industries/zed/commit/1a89a92e7e26) | [C-R](#c-r), [P22](#p22) | `source-reconciled` | BenchAppContext frame sessions use bounded ready tasks and scheduled frames; actual App update/candidate lifecycle retained. |
| `3db02c2c77659ad29810594e6249287ea1e791f7` | [gpui: Add is_visible and on_visibility_changed API on Window (#64107)](https://github.com/zed-industries/zed/commit/3db02c2c7765) | [C-R](#c-r), [P10](#p10), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/window.rs:8713` and events size/show handling report visible-and-not-iconic facts through post-borrow observations; `platform.rs:4187` handles both suspend and resume. Native retirement clears or fences observers. |
| `a7c7219d5bfd0fa7c59edb7198b6c26a58ba6116` | [gpui_linux: Avoid local XKB files for server keymaps (#64113)](https://github.com/zed-industries/zed/commit/a7c7219d5bfd) | [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `87a1ea30e819e193f8e3fcb517c6884f91ee7a9c` | [gpui_linux: Discard stale Wayland file drag reads (#64122)](https://github.com/zed-industries/zed/commit/87a1ea30e819) | [P16](#p16) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `7960b2a7c9568e90fbe0727332149e5b2a5fd57a` | [gpui: Add Canvas font fallback for web text (#64110)](https://github.com/zed-industries/zed/commit/7960b2a7c956) | [C-R](#c-r), [P20](#p20), [W-WGPU](#w-wgpu) | `source-reconciled` | `G/src/cosmic_text_system.rs::font_weight_and_style` reports the selected database face; `gpui_web`'s qualified Canvas wrapper owns actual browser fallback and post-fallback missing reporting. No product font policy enters this backend. |
| `25b5569dd231e740922cfebafa26b1a6c08531e8` | [project_panel: Add Menu key support to project panel context menu (#46744)](https://github.com/zed-industries/zed/commit/25b5569dd231) | [C-F](#c-f) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f792c2d72d14130d6758f21662365b0f36384d28` | [gpui: Add `ShapedLineCursor` API (#64185)](https://github.com/zed-industries/zed/commit/f792c2d72d14) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d62802d456ac93a5abce5fecd5e98d2b8124485d` | [Fix Trunk link in GPUI examples documentation (#64192)](https://github.com/zed-industries/zed/commit/d62802d456ac) | [C-X](#c-x), [U-DOCS](#u-docs) | `source-reconciled` | Superseded URL-only change: rewritten examples docs contain neither old Trunk link nor old gallery setup paragraph; current supported browser setup belongs to Web smoke/xtask. No gallery parity or fresh browser pass is inferred. |
| `9e6e14160c192f682c4d35f4e8d420cc0577ad5f` | [gpui: Support dynamic font installation (#63498)](https://github.com/zed-industries/zed/commit/9e6e14160c19) | [C-T](#c-t), [P20](#p20), [W-WGPU](#w-wgpu) | `source-reconciled` | `G/src/cosmic_text_system.rs` clears family selections on add_fonts, deduplicates LoadedFontKey by database face/configuration, preserves existing FontIds and emits MissingGlyphSink reports. Core generation/candidate cache invalidation is present; paragraph reporting aggregates original source indices. |
| `b68add5b37a125b3f9d6d04f23a2f93f8eaa57bb` | [gpui: Remove mutex from dynamic font loading (#64204)](https://github.com/zed-industries/zed/commit/b68add5b37a1) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `07df4386262dc3465649e26b68a9d12647b1d6d2` | [Remove `ztracing` dependency from GPUI (#64237)](https://github.com/zed-industries/zed/commit/07df4386262d) | [C-E](#c-e) | `source-reconciled` | GPUI SVG renderer has no ztracing dependency/instrumentation; separate tracing/logging owner retained. |
| `db7f9ceed788ebfcbbcb18e65927172f1dd7dde9` | [gpui_macos: Improve mouse tracking for normal windows (#64234)](https://github.com/zed-industries/zed/commit/db7f9ceed788) | [P07](#p07), [D-APPKIT](#d-appkit) | `source-reconciled` | Adopted NSTrackingArea feature and actual normal-window creation route: disable acceptsMouseMovedEvents, add entered/exited/moved/active/in-visible-rect tracking; floating route remains distinct. |
| `a0f8fa7bdbbf0269aaf5d5ebfac3ad9adfb576dd` | [Move Windows dialogs and credential I/O off the foreground thread (#64250)](https://github.com/zed-industries/zed/commit/a0f8fa7bdbbf) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/dialog.rs` has per-owner async serialization, dedicated STA/message pump, owner/request cancellation and TaskDialog cancel policy. Platform credentials run on the background executor; file dialogs distinguish user cancellation from real errors. Retirement uses try_lock_idle rather than destroying an in-use owner. |
| `de2c85f255dac20cacc4ed62e4611361dc7cbb7d` | [Fix Windows build of remote-server (#64270)](https://github.com/zed-industries/zed/commit/de2c85f255da) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/platform.rs` uses the fully qualified futures oneshot return type for optional screen capture. The removed import alias is not a separate product feature. |
| `5f1a6530e93db19c01f9a81eed7a0e94d8ba9f5c` | [language_models: Add SuperGrok OAuth provider (#63248)](https://github.com/zed-industries/zed/commit/5f1a6530e93d) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:x_ai_subscribed, members:crates/x_ai_subscribed. |
| `5c9efb75cf73fc56f6316455983e4f04e9330860` | [gpui: Avoid main-thread waits when unlocking web task queues (#64298)](https://github.com/zed-industries/zed/commit/5c9efb75cf73) | [C-R](#c-r), [P12](#p12) | `source-reconciled` | queue.rs:29 uses std Mutex/Condvar, poison recovery and TryLockError instead of parking_lot unlock path. |
| `f6838a7c4afecd7c212c43c70165588b92a5a11f` | [gpui_macos: Fix hover feedback when another application is focused (#64301)](https://github.com/zed-industries/zed/commit/f6838a7c4afe) | [P07](#p07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f0fb48c71ab664e3a664c29a9e79332c91252a59` | [gpui: Make ForegroundJournal aware of sleep and window visibility (#64269)](https://github.com/zed-industries/zed/commit/f0fb48c71ab6) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `b0f53ad23be82b698e652c962e0d83fec1eb7760` | [Optimize GPUI inspector's runtime cost by an order of magnitude (#64309)](https://github.com/zed-industries/zed/commit/b0f53ad23be8) | [C-V](#c-v), [P22](#p22), [G-BENCH](#g-bench) | `implemented-local-verified` | Historical baseline finding: P22: Partial: lazy Inspector adopted; inspector_render workload missing. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `9930d2c2a3c37562a457c8854fd28a25941d571a` | [Share generics better and strip rodio audio features (#64260)](https://github.com/zed-industries/zed/commit/9930d2c2a3c3) | [H14](#h14), [D05](#d05) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `0dea8c63d9a5d747b3d0903adb110a9def1d1020` | [editor: Render point diagnostics without glyphs (#63933)](https://github.com/zed-industries/zed/commit/0dea8c63d9a5) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `87f65de697bda93ad0a13dc92eabe941115b61de` | [gpui: Fix web fallback for text-default emoji (#64319)](https://github.com/zed-industries/zed/commit/87f65de697bd) | [P20](#p20) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `6c9d10cb83084cfdbb9d4cf3919b57b7dc707758` | [gpui_web: Flatten webgl texel loading (#64337)](https://github.com/zed-industries/zed/commit/6c9d10cb8308) | [W-WGPU](#w-wgpu) | `source-reconciled` | `G/src/shaders_webgl.wgsl::load_quad` has explicit fixed texel loads rather than a per-word cursor. Its 46-word record adds exact clip/transform fields and handles odd records crossing half-texel boundaries; `wgpu_renderer/webgl_tests.rs` verifies the current decoder. |
| `4b47ceb9d328035bde5ef99009e674d9fee07fb7` | [gpui_macos: Migrate screen capture to `objc2` (#64251)](https://github.com/zed-industries/zed/commit/4b47ceb9d328) | [C-F](#c-f), [P17](#p17), [D04](#d04) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `31971937b5c44dc837efb0e172f951264a10c7fd` | [Move `corgi-patches` into `tooling` (#64292)](https://github.com/zed-industries/zed/commit/31971937b5c4) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: patch:scratch. |
| `0ef92145ff87aab5f19f6b44c9c55a486f4f064f` | [gpui: Improve `PlatformAtlas::get_or_insert_with` (#64331)](https://github.com/zed-industries/zed/commit/0ef92145ff87) | [C-F](#c-f), [P23](#p23), [W-WINDOWS](#w-windows), [W-WGPU](#w-wgpu) | `source-reconciled` | Both atlases use Core AtlasState/AtlasBackend with owned keys, build callbacks and clear/remove ownership. Source/destination upload admission precedes key publication, retaining diagnostics and exact texture-instance leases. |
| `613a80b9e9ce55079deb0e5f135f6ed29c88aa58` | [gpui_macos: Fix traffic lights animation during fullscreen exit (#64339)](https://github.com/zed-industries/zed/commit/613a80b9e9ce) | [P02](#p02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `aec7395e30feba9139c9ab8b9444d672882ddf41` | [Fix cargo shear warnings (#64340)](https://github.com/zed-industries/zed/commit/aec7395e30fe) | [P24](#p24), [H16](#h16), [W-WINDOWS](#w-windows), [W-WGPU](#w-wgpu), [T-TOKIO](#t-tokio) | `implemented-local-verified` | Windows/WGPU retain enabled standalone library/doc verification instead of their upstream doctest=false metadata. In Tokio this commit adds test=false to the already existing doctest=false; both settings are adopted for its empty unit/doc harnesses while seven public integration tests remain enabled. Inherited bridge and manifest/util follow-ups implemented; F-ALL/F-PUBLIC pass. L-TOKIO. |
| `b9419ae72814157164b09592ab2466f9f4709f09` | [gpui_windows: Fix reversed linear/sRGB transfer functions in shader (#57543)](https://github.com/zed-industries/zed/commit/b9419ae72814) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/shaders.hlsl::linear_to_srgb/srgb_to_linear` uses corrected reciprocal-2.2 / 2.2 directions. The Windows approximation is retained; WGPU's piecewise transfer and LinearSrgb recording remain their separate existing contracts. |
| `ac6818fb2de7aaec9ee00d4ff697e948c8b8f0a3` | [gpui_windows: Keep DirectWrite's default ligatures when no font features are set (#62338)](https://github.com/zed-industries/zed/commit/ac6818fb2de7) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/direct_write.rs::apply_font_features` no longer returns early for empty tags and supplies DirectWrite's default liga/clig/calt while respecting explicit overrides. Layout regressions live in `direct_write/layout_tests.rs`. |
| `cf4deb270f04b75ecf5b1d4990591e1146560ea0` | [gpui: Fix centered window bounds to display settings header (#62073)](https://github.com/zed-industries/zed/commit/cf4deb270f04) | [C-F](#c-f) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c24e309d935d83f672dff1df64b2331749bcb44c` | [gpui: Fix manifest path for cross-compilation (#62525)](https://github.com/zed-industries/zed/commit/c24e309d935d) | [U-RC](#u-rc) | `source-reconciled` | Both resource hunks adopted: resource_dir/ParamsIncludeDirs and RC-relative gpui.manifest.xml. F-WINDOWS compiles the route; cross-host build-script cfg does not execute Windows resource compiler/link output. Source-reconciled, L-PLATFORM. |
| `94c997e06faabe6538e608c6d04091914f02f86b` | [gpui: Fix debug selectors with cached views (#64433)](https://github.com/zed-industries/zed/commit/94c997e06faa) | [C-E](#c-e) | `source-reconciled` | Window debug_bounds_entries journal at :9751/:9878 carries cached paint selectors; :10474 rollback restores them. |
| `0d08af1e53378fc2aca09eed049ab77df1a8439a` | [Share GPUI and scheduler implementations (#64261)](https://github.com/zed-industries/zed/commit/0d08af1e5337) | [C-R](#c-r), [H01](#h01) | `source-reconciled` | Shared scheduler types and App entity/arena owners retained with AppCell custody rather than copying monorepo Rc<App> assumptions. |
| `01448f565e52f5b370b40718663e9649b16990f0` | [Fix Windows task path handling for worktree variables with spaces (#55727)](https://github.com/zed-industries/zed/commit/01448f565e52) | [H06](#h06) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `0eda7703f6c88aa08a25c1d2105ff1ca46f775d4` | [gpui_macos: Release the accessibility adapter when a window is dropped (#64143)](https://github.com/zed-industries/zed/commit/0eda7703f6c8) | [P17](#p17) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `887149bbdd90a2f376583f156e558f2f016dc2e8` | [Ignore invalid file patterns individually (#64525)](https://github.com/zed-industries/zed/commit/887149bbdd90) | [H08](#h08) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `a51d23c93d15cf04c5d5cae2ecbf5b71404b71b7` | [Windows: Fix files opened by UNC path getting no language servers (#56553)](https://github.com/zed-industries/zed/commit/a51d23c93d15) | [H08](#h08) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `bcf6582ce3500df93a8a39366640173e6786cea6` | [gpui_util: Respect custom Scoop installation paths (#62473)](https://github.com/zed-industries/zed/commit/bcf6582ce350) | [H06](#h06) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `85b8198e863413325bbbf7f34f616e374277c6cf` | [acp: Update the ACP SDK to 2.2.0 (#64457)](https://github.com/zed-industries/zed/commit/85b8198e8634) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `a434bb7ee275cefa348a2bfaf27dc30280bf38f7` | [wgpu: Cache atlas bind groups (#64434)](https://github.com/zed-industries/zed/commit/a434bb7ee275) | [W-WGPU](#w-wgpu) | `source-reconciled` | `G/src/wgpu_renderer.rs:2047` caches bind groups by AtlasTextureInstanceId and prunes against the admitted presentation receipt. `wgpu_atlas.rs` advances instance generations on slot reuse; no aliasing of old/new textures. |
| `f25434f3c5a895d52f402ab05c079aa6703825c1` | [gpui_linux: Force a full render on X11 Expose after GPU recovery (#64570)](https://github.com/zed-industries/zed/commit/f25434f3c5a8) | [P14](#p14) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `96e95edac45b0e0696179b8139aa521cf48db392` | [gpui: Skip sprites whose atlas texture was released instead of panicking (#64623)](https://github.com/zed-industries/zed/commit/96e95edac45b) | [P23](#p23), [W-WINDOWS](#w-windows), [W-WGPU](#w-wgpu) | `source-reconciled` | Atlas lookups no longer panic for absent textures. `W::DirectXAtlas::preflight` and `G::prepare_scene` validate exact instances and refuse the candidate on failure, retaining the previous valid presentation. Skipping a sprite inside an otherwise accepted frame would violate the established fork contract. |
| `ba42ab9d9851f82a7d8f5ae40b941bf5c515fc10` | [gpui: Fix keystroke interception for standalone modifiers (#64537)](https://github.com/zed-industries/zed/commit/ba42ab9d9851) | [C-I](#c-i) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `9183ba4c5579c083b005d7f4c248d90c914fb46f` | [ui: Fix context menu selecting first entry on open (#64365)](https://github.com/zed-industries/zed/commit/9183ba4c5579) | [C-A](#c-a) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `b89471c52d22d2c642a9db6b4ad7e22f232bc974` | [gpui_apple: Remove `block` as a direct dependency (#64641)](https://github.com/zed-industries/zed/commit/b89471c52d22) | [P08](#p08), [D-APPLE-PACKAGES](#d-apple-packages) | `source-reconciled` | Adopted direct block dependency removal. Selected Apple completion routes use block2/RcBlock; legacy transitive block in the owned capture graph is explicitly not claimed removed. |
| `e71963a599c64c213aca1c607e7418503a1e787d` | [gpui: Fix panic when hit testing past a wrapped line's width (#64672)](https://github.com/zed-industries/zed/commit/e71963a599c6) | [C-T](#c-t), [W-WGPU](#w-wgpu) | `source-reconciled` | Core wrapped `index_for_position` clamps the end sliver and returns Err(row_end.index); the Cosmic zero-advance scenario is retained in `G/src/cosmic_text_system/paragraph_tests.rs` and public measured-text consumers. |
| `4c902c9db22a82f5f3a14c02442e7f60ec40d9c8` | [extensions_ui: Sync suggested extensions with the live registry (#63211)](https://github.com/zed-industries/zed/commit/4c902c9db22a) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:extension_suggest, members:crates/extension_suggest. |
| `382ff7a64b334ab59a3dc0e3f3185705f79cd0ab` | [gpui: Stop synthetic mouse drags after button release on macOS (#64175)](https://github.com/zed-industries/zed/commit/382ff7a64b33) | [P07](#p07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `e9ddcdaf98294e335370e4f7566f80494d273bf2` | [gpui: Make create ElementId more easier from u64 (#64702)](https://github.com/zed-industries/zed/commit/e9ddcdaf9829) | [C-F](#c-f), [G-M6](#g-m6) | `implemented-local-verified` | Historical baseline finding: M6: ElementId From<u64> absent; existing usize/i32/tuple conversions do not provide that trait impl. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `8720fd1d093963be0aeb36954c15fd1aca1b94d2` | [gpui_windows: Handle null UserName and CredentialBlob from CredReadW (#64735)](https://github.com/zed-industries/zed/commit/8720fd1d0939) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/platform.rs:3222/:4745` copies optional username/blob as empty when null and calls CredFree after conversion even on error. Native credential tests retain actual absent-username and ordinary controls. |
| `7fdb97cad51694f2cdfd36abcb11265f4c4dafe2` | [cloud_api_client: Update yawc to 0.4.2 (#64728)](https://github.com/zed-industries/zed/commit/7fdb97cad516) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:yawc. |
| `e91b82c106817f2419207ebf81f1da766698ac95` | [gpui_wgpu: Add Linux headless rendering (#64718)](https://github.com/zed-industries/zed/commit/e91b82c10681) | [C-R](#c-r), [P14](#p14), [W-WGPU](#w-wgpu) | `source-reconciled` | `G/src/wgpu_context.rs::create_headless` owns real adapter/device selection; WgpuRendererCore is reused by WgpuHeadlessRenderer for no-readback render and padded-row BGRA/RGBA screenshot paths. DeviceErrorState is installed once per owned device and shared by renderer observers. Optional image/test/bench exposure remains explicit. |
| `bda9c0bd43a8d235d82adb01ea5bc875b861ecfc` | [markdown: Fix search highlights hiding text with opaque theme colors (#64788)](https://github.com/zed-industries/zed/commit/bda9c0bd43a8) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `4c841aaf1c4fa613e89a5d77096523d0ff593b56` | [Remove unused features from dependencies (#64826)](https://github.com/zed-industries/zed/commit/4c841aaf1c4f) | [D-ROOT-PRODUCT](#d-root-product) | `outside-framework` | Product-only aws-config/AWS Bedrock/wasmtime-wasi feature trim: actual services are outside selected consumers. The three direct keys and packages are absent from current selected manifests/root/independent locks. |
| `e683fd7b465ecfb42b1da88ff685d204c2781076` | [gpui: Add bench_metrics for multi-metric Criterion benchmarks (#64753)](https://github.com/zed-industries/zed/commit/e683fd7b465e) | [C-R](#c-r), [H12](#h12), [D05](#d05) | `source-reconciled` | BenchMeasurement/report successor and independent bench-support context adopted; crate implementation outside lane. |
| `4f70d91bda7ec5f5600a000a0dc34391d9f1e96f` | [editor: Add support for customizable soft wrap indentation (#63468)](https://github.com/zed-industries/zed/commit/4f70d91bda7e) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `98f75a23bc401d807fb797ad74a92ededdd9ff8c` | [Remove some unnecessary dependencies (#64862)](https://github.com/zed-industries/zed/commit/98f75a23bc40) | [P24](#p24), [H09](#h09), [D-EXTRACT](#d-extract) | `source-reconciled` | Adopted selected cleanup: no Linux x11-clipboard, no util take_until dependency/reexport, and removed root crossbeam/json_dotpath/portable-pty/take-until aliases are absent. Product queue/terminal replacements require no new direct GPUI owner; postage retains transitive crossbeam-queue. |
| `ee43be113e362502f8f7c030db5e797126577973` | [acp_thread: Adopt shared ACP v2 content values (#64765)](https://github.com/zed-industries/zed/commit/ee43be113e36) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `86c20cc9119d89b21ab8ea1cb063f40faa119736` | [gpui: Add character input preference to keystroke events (#64859)](https://github.com/zed-industries/zed/commit/86c20cc9119d) | [C-I](#c-i) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `244023605536a412ab6b8d5b658466b89fb15401` | [gpui: Add seeded benchmarks and heap allocation metrics (#64843)](https://github.com/zed-industries/zed/commit/244023605536) | [C-R](#c-r), [H12](#h12) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `1b572a2bd0376e5612a61a01c3ad6d087100fa7e` | [Extract hang telemetry into a reusable hang_telemetry crate (#64874)](https://github.com/zed-industries/zed/commit/1b572a2bd037) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:hang_telemetry, members:crates/hang_telemetry. |
| `986828d157135be95d999863dd1d618990173b8c` | [acp_thread: Add shared tool-call state and diff previews (#64875)](https://github.com/zed-industries/zed/commit/986828d15713) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:diffy. |
| `e773a56d6cf08ad83b1b9a7bef3c059c5bf26cb1` | [gpui_web: Preserve iOS keyboard edits and browser window activity (#64882)](https://github.com/zed-industries/zed/commit/e773a56d6cf0) | [P19](#p19) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `3135cdf8dc8e6800d2b7ad89d62ea4994d892fb6` | [gpui: Add randomized element tree benchmarks and fix Linux benchmark leaks (#64842)](https://github.com/zed-industries/zed/commit/3135cdf8dc8e) | [C-R](#c-r), [P22](#p22), [H12](#h12), [W-WGPU](#w-wgpu) | `source-reconciled` | `G/src/wgpu_context.rs::new_headless` caches a process-wide device until loss, avoiding TLS destructor teardown. `wgpu_renderer.rs:2864` starts error observation at the current generation before pipeline creation. Other queue/Linux/benchmark slices belong to support/platform ledgers. |
| `ead2d9eac0d34fa957421ea74aede6440b6ff926` | [http_client: Ensure GitHub digest prefix is always stripped (#64905)](https://github.com/zed-industries/zed/commit/ead2d9eac0d3) | [H10](#h10) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `12f79c0aeb86c49750d2f60f816506675641b1ab` | [cloud_api_client: Use the platform TLS verifier for the cloud websocket (#63686)](https://github.com/zed-industries/zed/commit/12f79c0aeb86) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:tokio-rustls. |
| `7232ba04ef7643b7a5d1f10a60d9c8b9b51974f9` | [gpui: Drop foreground measurements that span an unnotified system suspend (#64891)](https://github.com/zed-industries/zed/commit/7232ba04ef76) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `0f9c923e674e13e2891aaf31b33bbbc43d08b699` | [Add a GPUI-owned hang monitor and make hang_telemetry a consumer (#64944)](https://github.com/zed-industries/zed/commit/0f9c923e674e) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `decbf641b18f1982b3475c037e7c5c554471574f` | [Fix `write_to_clipboard` on Windows when string contains embedded nulls (#64911)](https://github.com/zed-industries/zed/commit/decbf641b18f) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/clipboard.rs::write_string` replaces embedded NUL with spaces for CF_UNICODETEXT and hashes that normalized text, preserving paired metadata. Native clipboard tests and readback retain the scenario. |
| `4c0ead4a3e3726fb229d297c42fde856478b64a8` | [gpui: Fix flaky hang monitor shutdown test (#64993)](https://github.com/zed-industries/zed/commit/4c0ead4a3e37) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d284cbbc87b55d659303106ced627c8612369827` | [gpui: Fix flaky foreground hang detection test (#64996)](https://github.com/zed-industries/zed/commit/d284cbbc87b5) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `8bbcd839686303a299b7b5135117058e9d66e64a` | [Speed up project search over misclassified binary files (#64549)](https://github.com/zed-industries/zed/commit/8bbcd8396863) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:file_content, members:crates/file_content. |
| `40180d9c40e2d20eb63d388bff920818f2910b53` | [gpui_apple: Share dispatcher and Metal renderer with iOS (#64990)](https://github.com/zed-industries/zed/commit/40180d9c40e2) | [C-E](#c-e), [P08](#p08), [D04](#d04), [W-SHARED](#w-shared) | `source-reconciled` | Adopted shared-render slice: Scene's native surface payload is enabled for macOS or iOS; shared Apple renderer/dispatcher topology is in the platform report. |
| `71456c40f3971ee763e620a513c5fb98390d93e3` | [gpui: Hide disabled bindings from the which-key popup (#64935)](https://github.com/zed-industries/zed/commit/71456c40f397) | [C-I](#c-i), [P06](#p06), [G-M2](#g-m2) | `implemented-local-verified` | Historical M2: continuation hints skipped dispatch winners. Current resolver checks full continuations with NoAction/Unbind retained; F-ALL passes. L-CORE. |
| `95cd535a5fad96d649513f96c5784ceefd379e47` | [gpui: Make test-support safe to enable in every build (#65043)](https://github.com/zed-industries/zed/commit/95cd535a5fad) | [C-R](#c-r), [P11](#p11), [H01](#h01) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `cd4fc8de4ca8548cca2567352b87bcaaec13328f` | [gpui: Journal platform frame requests and skipped frames (#64958)](https://github.com/zed-industries/zed/commit/cd4fc8de4ca8) | [C-R](#c-r), [P10](#p10), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/vsync.rs` is final-target equivalent after aliases; `platform.rs` records real signal source/time for visible non-iconic tracked windows and drops stale hidden signals. `events.rs:2527` consumes signals into RequestFrameOptions; Core owns completed/skipped journal admission. |
| `506beb34de3f433707b7ebe8d8ad2d80f856af6c` | [gpui: Record debug selectors only in debug builds (#65056)](https://github.com/zed-industries/zed/commit/506beb34de3f) | [C-E](#c-e), [G-M5](#g-m5) | `implemented-local-verified` | Historical M5: release test-support recorded selectors. Current builder/storage/recording paths require debug_assertions; affected host tests pass F-ALL. Separate release execution not supplied. L-CORE. |
| `36b6d0951fdee409f0957294a69360ba2e8e980e` | [gpui: Stop sealing empty intervals on idle frame skips (#65057)](https://github.com/zed-industries/zed/commit/36b6d0951fde) | [C-R](#c-r), [P10](#p10) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `9dd6993e2e6e26baaf0fdfcb152cb823dba1135d` | [gpui_shared_string: Avoid intermediate string allocation when deserializing a `SharedString` (#65089)](https://github.com/zed-industries/zed/commit/9dd6993e2e6e) | [H02](#h02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f37989fbdf840b308892af66516c2b9c3ef3853a` | [gpui: Cross-platform headless windowing API and linux impl (#64969)](https://github.com/zed-industries/zed/commit/f37989fbdf84) | [C-R](#c-r), [P14](#p14), [D01](#d01), [W-WINDOWS](#w-windows), [Q-COLR](#q-colr) | `implemented-local-verified` | Windows initial/live windowing uses persistent platform state, session/station validation, delayed GPU/vsync attach and closed transition validation. DirectWrite now needs no GPU at all after the CPU successor, replacing its former optional GPUState/release hooks. Current mode/renderer-source owners remain. |
| `a84689073d296dfd39987bc7dd478e43ef76d83a` | [path: Reimplement PathStyle on the standard library's path implementation (#64149)](https://github.com/zed-industries/zed/commit/a84689073d29) | [H07](#h07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `eca87ab896e97449942bea7fa0be53f128247012` | [gpui: Remove redundant flushing_effects reentrancy guard (#65016)](https://github.com/zed-industries/zed/commit/eca87ab896e9) | [C-R](#c-r) | `source-reconciled` | Retained flushing_effects with App update unwind guard and shutdown-fence owner at app.rs:936/:1492; fork recursive flush/terminal contract differs. |
| `533fe08cfb925891bc4f5570894501244e6ee68d` | [language_core: Store path suffixes and modeline aliases as shared strings (#65128)](https://github.com/zed-industries/zed/commit/533fe08cfb92) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:toml. |
| `1dd9bbfd71cdb1eed32e3451d661de48a68493fd` | [gpui: Hash a GlobalElementId once, when it is built (#64209)](https://github.com/zed-industries/zed/commit/1dd9bbfd71cd) | [C-V](#c-v) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d544470d1d3770f0024651dbf121525564a2657c` | [Reduce the dependency footprint (#65176)](https://github.com/zed-industries/zed/commit/d544470d1d37) | [P24](#p24), [H16](#h16), [D05](#d05), [W-WINDOWS](#w-windows), [W-WGPU](#w-wgpu), [T-TOKIO](#t-tokio) | `implemented-local-verified` | Backend OS gates, explicit WGPU native/WebGL features and required Win32 features exist. Root broad Windows features supply the backend rather than repeating them in every manifest. Fork-owned font-kit and standalone build/test defaults remain intentional; unused monorepo tracing/product dependencies are not imported. Inherited Tokio bridge and manifest/util follow-ups implemented; seven public bridge tests and F-ALL/F-PUBLIC pass. L-TOKIO. |
| `285d9750435a098c8c57f6e55ab3cd65b143b7ba` | [Improve Zed's startup time (#65170)](https://github.com/zed-industries/zed/commit/285d9750435a) | [P26](#p26) | `implemented-local-verified` | Historical hidden-view invalidation omission fixed before makeKeyAndOrderFront; F-ALL/F-WORKSPACE compile native owner. Visible reveal/startup timing unexecuted. L-PLATFORM. |
| `72d073d6423b0bf7e04aa87567a308d19617b7f2` | [Fix agent file link navigation (#64748)](https://github.com/zed-industries/zed/commit/72d073d6423b) | [H08](#h08) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d6358b2d3cd04de4d8f8f69610cccd2e390b6a3e` | [gpui_wgpu: Fix fallback fonts disappearing from the font database on Linux (#65269)](https://github.com/zed-industries/zed/commit/d6358b2d3cd0) | [W-WGPU](#w-wgpu) | `source-reconciled` | Cosmic primary eligibility's missing-`m` filter continues without remove_face, retaining Arabic/Devanagari/emoji faces for automatic fallback. The already documented explicit-chain eligibility limitation remains separate. |
| `01f22d2d8646df57da2010a45d491ca679f487aa` | [util: Expose Windows job activity for child processes (#64870)](https://github.com/zed-industries/zed/commit/01f22d2d8646) | [H05](#h05) | `implemented-local-verified` | Existing owned-job ActiveProcesses query, best-effort docs and unmanaged/live-grandchild fixtures implemented; real Windows all-target F-WINDOWS compiles. Native Job execution unqualified. L-PLATFORM. |
| `56303a3372e1007c3d2a587ebcc42939e2794901` | [audio: Recover from notification output stream failures (#65158)](https://github.com/zed-industries/zed/commit/56303a3372e1) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:cpal, dep:rodio, patch:cpal. |
| `7ca96c2a5814fe3950d7a6452bffcf0e8be9acb8` | [Keep remote agent secrets out of launcher arguments (#65174)](https://github.com/zed-industries/zed/commit/7ca96c2a5814) | [H06](#h06) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `67b41bf97607f2dd86319b385040a1310cf24f45` | [Upgrade ACP SDK to 3.1.0 (#65274)](https://github.com/zed-industries/zed/commit/67b41bf97607) | [D01](#d01) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `19c69782eb3c1d99b79878d82efa935d565910f9` | [Fix panic when EGL fails without reporting an error (#65329)](https://github.com/zed-industries/zed/commit/19c69782eb3c) | [D-EGL](#d-egl) | `implemented-local-verified` | Exact khronos-egl 42214b36347b108fd9630ca6254b96dde59e04fe root/independent-root patches and locks adopted, including new iOS root. F-EGL: real binding 8/8, dynamic consumer 3/3; registry 2/3 with actual initialize unwrap(None). No driver/compositor credit. L-DEPS. |
| `b0048cde742ecdef36b36d4a18eec97af99254df` | [gpui_web: Avoid blocking Fetch response-body sends (#65334)](https://github.com/zed-industries/zed/commit/b0048cde742e) | [P12](#p12) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f495dd0f77b69e287ed72c1168dc02965f08be7d` | [gpui: Track connected displays and each window's display (#65059)](https://github.com/zed-industries/zed/commit/f495dd0f77b6) | [C-R](#c-r), [P03](#p03), [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/display.rs` owns immutable topology and cached refresh intervals. Platform uses a hidden top-level broadcast window and ordered topology publication; per-window binding changes follow exact snapshots. Restart Manager reaches the platform even with no application windows. |
| `9ab0715969e9854f2efe61a9d782e7698e5fe6d2` | [agent_servers: Preserve initial ACP session updates (#65339)](https://github.com/zed-industries/zed/commit/9ab0715969e9) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `95d35c6efddfa6dc34f99bb1873620bd74af806a` | [gpui_apple: Share CoreText, Keychain, and thermal services (#65058)](https://github.com/zed-industries/zed/commit/95d35c6efddf) | [C-E](#c-e), [P08](#p08), [D04](#d04), [W-SHARED](#w-shared) | `source-reconciled` | Adopted shared-render slice: Core PaintSurface uses retained objc2 CoreVideo/CFRetained ownership; shared Apple font/keychain/thermal services are separately reconciled. |
| `4f6d97b9de7d9f167da8b7832eeecea22c5a9efa` | [gpui: Measure frame pacing against each window's refresh rate (#65351)](https://github.com/zed-industries/zed/commit/4f6d97b9de7d) | [C-V](#c-v) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `679ad7c30427303f765870c640a04b5ae243da2d` | [gpui: Fix uniform_list item layout far down long lists (#65288)](https://github.com/zed-industries/zed/commit/679ad7c30427) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `a2a92fe7c442dee20631a9c3dfccceeb7538c462` | [gpui_platform: Forward `gpui_web`s multithreaded feature instead of forcing it (#65289)](https://github.com/zed-industries/zed/commit/a2a92fe7c442) | [P24](#p24), [D-SOURCE](#d-source) | `source-reconciled` | Root source reconciled through the linked owning audit: dep:gpui_web. |
| `90d8514059561f0e44af0a32e84e1c329cef3232` | [gpui_macos: Fix blurred window backgrounds on macOS 27 (#65361)](https://github.com/zed-industries/zed/commit/90d851405956) | [P02](#p02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `687d662a0b686aae6ab2c06293d866bbf6be3afd` | [agent: Upgrade ACP SDK to 3.3 (#65414)](https://github.com/zed-industries/zed/commit/687d662a0b68) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `51a3a0e971ee1fd1e96401e6998b9d0a9ac55abc` | [path: Distinguish `is_absolute` and `has_root` in `PathStyle` (#65468)](https://github.com/zed-industries/zed/commit/51a3a0e971ee) | [H07](#h07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f929b3f7ce00017c8e1cb3bcf015eb331b265410` | [path: Reimplement `PathStyle::strip_prefix` based on std lib's implementation (#65503)](https://github.com/zed-industries/zed/commit/f929b3f7ce00) | [H07](#h07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `e18de65b75901f8720ff9c9320bb013395de54e5` | [gpui: Composite translucent layers with source-over alpha (#65571)](https://github.com/zed-industries/zed/commit/e18de65b7590) | [P08](#p08), [W-WINDOWS](#w-windows), [W-WGPU](#w-wgpu) | `source-reconciled` | `W/src/directx_renderer.rs::create_blend_state` uses INV_SRC_ALPHA for destination alpha; WGPU path tiles now share the ordinary source-over state instead of the retired additive-alpha path-sprite state. |
| `a47f721832e5b6ecf977891c1b7c6aac1406723e` | [gpui_ios: Add iOS platform backend (#63068)](https://github.com/zed-industries/zed/commit/a47f721832e5) | [C-X](#c-x), [P21](#p21), [H17](#h17), [D07](#d07), [G-IOS-CORE](#g-ios-core) | `implemented-local-verified` | Historical iOS exclusion superseded by complete selected UIKit/Core/shared Apple adoption; seven findings fixed and re-review closed. F-IOS simulator all-target/all-feature, independent consumer and device lib/all-feature compile pass. Initial native probe fails from fixture root borrow; the corrected native probe passes with explicit success marker after real key-ownership and drawable-oracle corrections. L-IOS/L-CORE. |
| `e463c5ab74e016f6a16e02a8b21925b98643f9c7` | [Fix focus cleanup stalls during conversation restoration (#65177)](https://github.com/zed-industries/zed/commit/e463c5ab74e0) | [C-I](#c-i), [P22](#p22), [G-BENCH](#g-bench) | `implemented-local-verified` | Historical baseline finding: P22: Partial: runtime focus cleanup adopted; focus_cleanup workload missing. Current correction complete; linked groups map its local SHA and passing matrix gate. |
| `e462f873c622f6265736bf8468e7a017f4ba99de` | [Ignore foreign WM_USER messages (#65628)](https://github.com/zed-industries/zed/commit/e462f873c622) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/platform.rs:3320` routes accelerators only to registered hosts; `events.rs` validates per-platform numbers plus exact native generation for custom messages. WM_GPUI_KEYDOWN and untrusted device pointers are removed; recovered devices have a shared owned slot. |
| `21001b273f93d1156d13d4eb67b255478a61276b` | [gpui: Convert the iOS example to a Cargo example target (#65658)](https://github.com/zed-industries/zed/commit/21001b273f93) | [H17](#h17), [D-SOURCE](#d-source) | `implemented-local-verified` | Official separate iOS example member converted to actual open-gpui-ios Cargo example target; F-IOS compiles. Seven historical review findings closed; native iOS 26.3 success-marker receipt now passes for the bounded probe. L-IOS. |
| `402675e8058b6b21771be2004fff0977e3b0a79e` | [gpui_wgpu: Upgrade wgpu to fix Vulkan surface teardown (#65649)](https://github.com/zed-industries/zed/commit/402675e8058b) | [W-WGPU](#w-wgpu), [D-SOURCE](#d-source) | `source-reconciled` | Root source reconciled through the linked owning audit: dep:wgpu. Root and independent locks use exact official WGPU/Naga cddb2f5d80dc0009b2ef6767635140cacf90a007, superseding initial 3b3506. Current source includes the 30-series descriptor fields, queue.present and fallible mapped-range API. |
| `7699cb8bb8b783996c035b3a10ba4b71ea84e8ca` | [gpui: Let a touch that catches a fling pick its own axis (#64239)](https://github.com/zed-industries/zed/commit/7699cb8bb8b7) | [C-I](#c-i) | `source-reconciled` | gestures.rs keeps caught-fling undecided axis and new-pan axis selection. |
| `5504e87bfdaa90b3b4852a814940ece148f5fe1d` | [workspace: Fix trust scope paths when connecting to Windows hosts from Unix (#63102)](https://github.com/zed-industries/zed/commit/5504e87bfdaa) | [H07](#h07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `6b218b8412d2e5aae339ce3826756be561f7efe9` | [Upgrade ACP SDK to 3.4.0 (#65706)](https://github.com/zed-industries/zed/commit/6b218b8412d2) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `2d4c4831943153f63ad0aeddf49c8f5a3d008e75` | [gpui_apple: Display P3 option for MetalRenderer's colorspace (#64394)](https://github.com/zed-industries/zed/commit/2d4c48319431) | [C-F](#c-f), [P08](#p08) | `source-reconciled` | Renderer color-space/platform surface; shared Core parameters already carried but GPU/native claims excluded. |
| `670a779a5ed9e7161c97aa4f998d395c207d5631` | [gpui: Add hot reloading using Subsecond (#65738)](https://github.com/zed-industries/zed/commit/670a779a5ed9) | [C-V](#c-v) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `b4ecc454225fc1819e37e890b62df5473b2ba95b` | [gpui: Group sprites by atlas texture and colocate sort keys (#65814)](https://github.com/zed-industries/zed/commit/b4ecc454225f) | [C-F](#c-f), [W-WINDOWS](#w-windows), [W-WGPU](#w-wgpu) | `source-reconciled` | Shared GPU scene representation/batching; GPU lane owns complete reconciliation. Core Scene sprite headers colocate order/texture keys, batch on exact texture instance and retain overlap safety; WGSL/HLSL/subpixel/packed-WebGL field order matches. Current clip, generation and transform additions remain initialized and covered by ABI source tests. |
| `febcb5bb99dce23384fe298be340cfebf8667788` | [Bump proptest (#65821)](https://github.com/zed-industries/zed/commit/febcb5bb99dc) | [C-R](#c-r), [H13](#h13), [D05](#d05) | `source-reconciled` | Proptest spelling cleanup only in affected Core test; dependency policy outside this lane. |
| `042e0406d33cfb312ea7e5796870671e0afa7383` | [path: Clean up leftover helpers before join_path work (#65792)](https://github.com/zed-industries/zed/commit/042e0406d33c) | [H07](#h07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `e39fb7c2e0560d013df0e77bce1922bf5cce1ad9` | [gpui: Rename `ScrollWheelEvent` to `ScrollEvent` and add trackpad scrolling on iOS (#65805)](https://github.com/zed-industries/zed/commit/e39fb7c2e056) | [C-I](#c-i), [P21](#p21), [H17](#h17), [D07](#d07), [W-WINDOWS](#w-windows) | `implemented-local-verified` | Canonical ScrollEvent/ScrollSource retained in Window/Div/List and Windows manipulation/wheel producers. Selected UIKit trackpad producer implemented and compiled in F-IOS; historical exclusion superseded. L-IOS; native input unqualified. |
| `2c626b3397193b17278bec84641b2c586af74583` | [gpui_ios: Forward Universal Links to URL handlers (#65858)](https://github.com/zed-industries/zed/commit/2c626b339719) | [H17](#h17) | `implemented-local-verified` | Selected UIKit initial/continued Universal Links preserve query/fragment and callback delivery; F-IOS compiles, source re-review closed. Native link execution unqualified. L-IOS. |
| `c4a82ab1ab958aa43b0c0b15c0bad4fb77a7ca7f` | [gpui_ios: Add display corner radii and an `on_back_gesture` (#65806)](https://github.com/zed-industries/zed/commit/c4a82ab1ab95) | [C-X](#c-x), [P21](#p21), [H17](#h17), [D07](#d07) | `implemented-local-verified` | Historical native display-corner exclusion superseded: UIKit corner/back-gesture producers and portable listener compile in F-IOS/F-ALL. L-IOS/L-CORE; physical gestures unexecuted. |
| `a4a3a968d14c00d71dba1ed4ef8ef9ab0a320b66` | [path: Replace `normalize` with component-based normalization in `PathStyle` (#65782)](https://github.com/zed-industries/zed/commit/a4a3a968d14c) | [H07](#h07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `065d7ae59791fbdafbb08c7650da091f6d1f9fd4` | [agent_ui: Improve model and configuration option search (#64147)](https://github.com/zed-industries/zed/commit/065d7ae59791) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: members:crates/agent_ui_benchmarks. |
| `d94887e08eb347a3344fe5a2dfb36fd975f412e2` | [gpui: Report the origin of conflicting app borrows (#65863)](https://github.com/zed-industries/zed/commit/d94887e08eb3) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d7adf3e7e278a39869574c774a55bacef7e2e3f0` | [project_panel: Add extension sort order (#65776)](https://github.com/zed-industries/zed/commit/d7adf3e7e278) | [H08](#h08) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `0a9eb81e6cedf7330491b0a6ca5d593af6abbe13` | [gpui_macos: Migrate keyboard and event bindings to objc2 (#65715)](https://github.com/zed-industries/zed/commit/0a9eb81e6ced) | [P07](#p07), [D-APPKIT](#d-appkit) | `source-reconciled` | Adopted NSEvent feature via macOS member; typed native event/key-code reads and nullable character conversion feed the real framework input path at the baseline. |
| `cb1937a8e7222edbacd6682e00568a5bed3e8949` | [gpui_macos: Migrate display bindings to objc2 (#65714)](https://github.com/zed-industries/zed/commit/cb1937a8e722) | [P03](#p03) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `e83f1c949512bab023f2f82446ffb4b0e4173fef` | [gpui_macos: Migrate clipboard bindings to objc2 (#65713)](https://github.com/zed-industries/zed/commit/e83f1c949512) | [P05](#p05) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f41ac648126becc954455b925ea02176e379435e` | [agent: Parse thread markdown lazily, on first render (#65470)](https://github.com/zed-industries/zed/commit/f41ac648126b) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `cb593458e1d6ed998709ac5257cb48ab147a5b12` | [project: Match document filter patterns in dynamic registrations (#65252)](https://github.com/zed-industries/zed/commit/cb593458e1d6) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:lsp-types. |
| `b488c4e81451f2042d5f47d93f6158732ac28212` | [gpui: Update Taffy 0.13 to 0.15 (#65950)](https://github.com/zed-industries/zed/commit/b488c4e81451) | [C-T](#c-t), [D-TEXT](#d-text) | `source-reconciled` | Adopted final Taffy 0.15 prerequisite with complete NodeContext/measurement/scale callback and one-to-one Clip mapping. Root/independent locks use 0.15.0; compatible library range and smoke 0.15.1 remain explicit, without whole-release parity. |
| `130f1e92a48ecdcc210419c5d9a52d605fa18fc0` | [Update dependencies to address security vulnerabilities (#65956)](https://github.com/zed-industries/zed/commit/130f1e92a48e) | [D02](#d02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `0a39cdb4f3ec57bcd930202924b713c0a9253b13` | [gpui: Fix window_positioning example on non-primary displays (#65917)](https://github.com/zed-industries/zed/commit/0a39cdb4f3ec) | [C-X](#c-x), [U-POSITION](#u-position) | `implemented-local-verified` | All five display-relative formulas adopted and reverse-check matches; actual platform window_positioning target registered and compiled in F-WORKSPACE. L-EXAMPLES; native multiple displays unexecuted. |
| `3389d87577312c126faa3bcbf79951babd6161d7` | [Cache resolved fonts (#65852)](https://github.com/zed-industries/zed/commit/3389d8757731) | [C-T](#c-t), [W-WGPU](#w-wgpu) | `source-reconciled` | Core resolved-font caching handles warm reads independently from cold backend locks; Cosmic installation tests preserve warm-versus-cold concurrency and retry-after-installation. Backend face identity is retained. |
| `d86aa6e6b9f52046fb299ab57ec884b300190813` | [gpui_macos: Migrate application and menu bindings to objc2 (#65932)](https://github.com/zed-industries/zed/commit/d86aa6e6b9f5) | [P07](#p07), [D04](#d04) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `10cfb459449a65c8fee7d390794d6f3d5098e9f7` | [gpui_macos: Migrate window and appearance bindings to objc2 (#65933)](https://github.com/zed-industries/zed/commit/10cfb459449a) | [P07](#p07), [D-APPKIT](#d-appkit) | `source-reconciled` | Adopted direct cocoa/objc removal and NSColor/NSDragging/NSVisualEffectView features via the macOS member. Actual typed appearance/view/drag routes exist at baseline; unrelated transitive legacy bindings are not claimed absent. |
| `b7d50b95e8db53607438ef6f5c29d55b7b38f852` | [Warn only for explicitly requested fonts (#65968)](https://github.com/zed-industries/zed/commit/b7d50b95e8db) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `2d6c664dc25cb3f9ef4e3bdb5885d4268ae5e4f9` | [gpui_macos: Cache the main-thread marker per thread (#65934)](https://github.com/zed-industries/zed/commit/2d6c664dc25c) | [P08](#p08) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `389c22e1c747d1a433c3b51c176936166c9cc1cf` | [gpui: Make `Modifiers::secondary` web platform aware (#65970)](https://github.com/zed-industries/zed/commit/389c22e1c747) | [C-E](#c-e), [P19](#p19), [H17](#h17), [W-WINDOWS](#w-windows) | `implemented-local-verified` | `W/src/platform.rs::platform_info` reports Windows/Native. Shared dynamic modifiers/labels/keybindings and Web runtime identity are already reconciled by Core/platform lanes. Selected iOS source complete, seven prior findings closed, F-IOS compiles; native iOS 26.3 success-marker receipt now passes for the bounded probe. L-IOS. |
| `fd2aedc47199338d00fb51fa39786270f2c4574e` | [agent_servers: Update ACP SDK to 3.5.0 (#65982)](https://github.com/zed-industries/zed/commit/fd2aedc47199) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `f1ccf949d59bc0cc8ec05a4366be4dcb8262aaf8` | [gpui_ios: Reuse the cached main-thread marker (#65975)](https://github.com/zed-industries/zed/commit/f1ccf949d59b) | [P08](#p08), [H17](#h17) | `implemented-local-verified` | Selected UIKit/shared Apple source reuses cached main-thread marker; F-IOS all-target/all-feature compilation passes. L-IOS; no physical/native execution inferred. |
| `91d3b962ae32aa725f351c621c6169e1e1a1e881` | [gpui_linux: Use the patched oo7 0.6 branch (#66035)](https://github.com/zed-industries/zed/commit/91d3b962ae32) | [D-KEYRING](#d-keyring) | `source-reconciled` | Adopted exact oo7 731c1215edb6905876c6137ab300428c7b8d7c13 in root/independent/smoke locks. Linux async-std/native_crypto credential create/search/delete calls select the corrected owner; no fresh Secret Service execution. |
| `9f305aaf88370e8ca39c702242186e3d8f97ecca` | [gpui_macos: Handle null open and save panels (#66042)](https://github.com/zed-industries/zed/commit/9f305aaf8837) | [P17](#p17) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `abd9b3cbd2b11292358d740b9c6ab3eae7d1ab36` | [gpui_apple: Distinguish missing creds from a cancelled prompt (#66060)](https://github.com/zed-industries/zed/commit/abd9b3cbd2b1) | [P09](#p09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `79fe6a645f68f8c9230578f154bf99852d9f716f` | [gpui: Add a way to write credentials only if none exist (#66062)](https://github.com/zed-industries/zed/commit/79fe6a645f68) | [C-F](#c-f), [P09](#p09) | `source-reconciled` | Core facade atomic credentials hook present; native Keychain implementation belongs to platform lane. |
| `347f171a3fd00a3b672382d78348b37bd969d17c` | [util: Preserve path separators when parsing file positions (#66089)](https://github.com/zed-industries/zed/commit/347f171a3fd0) | [H08](#h08) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `18f9d7e7318383a6f8ff8a8838a2746f8c6b376a` | [Fix pwsh-related issues with local and remote env variables detection (#66097)](https://github.com/zed-industries/zed/commit/18f9d7e73183) | [H06](#h06) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `772a8a91006db1c44171545cb7ba2d4c23d3e4cb` | [gpui: Remove synchronization from window text layout (#66147)](https://github.com/zed-industries/zed/commit/772a8a91006d) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `2a74c8dff6902b7d02038050f5672a0989b18f94` | [gpui: Remove unused hash-based text layout cache (#66152)](https://github.com/zed-industries/zed/commit/2a74c8dff690) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `1808df02c0ff7e12f2fa3bcd46b69fa8f5d485c7` | [Remove `block` dependency from the tree (#65942)](https://github.com/zed-industries/zed/commit/1808df02c0ff) | [P08](#p08), [H17](#h17), [G-BENCH](#g-bench), [D-APPLE-PACKAGES](#d-apple-packages) | `implemented-local-verified` | Direct metal removal/typed Metal adopted; owned open-gpui-scap 0.1.0-beta.1 is a source-backed retained contract for non-Apple capture. Legacy transitive block/cocoa remain; official scap revision/block-free lock parity is not claimed. Atlas_upload/iOS compile in F-BENCH/F-IOS; L-BENCH/L-IOS map local hashes. Native timing/input unqualified. |
| `f14592c4a5bd1dcd3545041a620055c5d4be0622` | [Require macOS 12 and SDK 26 (#65994)](https://github.com/zed-industries/zed/commit/f14592c4a5bd) | [P08](#p08), [D-APPKIT](#d-appkit) | `source-reconciled` | Adopted UTI std/UTCoreTypes/UTType feature declaration in the macOS member, resolving 0.3.2. Actual drag owner calls typed filename-extension and folder/data UTTypes; P08 retains native OS/SDK limits. |
| `087340e2d736737e66f6b8da9b3e3997e309bb09` | [Fix panic on cmd-q (#66158)](https://github.com/zed-industries/zed/commit/087340e2d736) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: patch:crash-context. |
| `979cd3fdf02ffa39131b33507cbd7375a68910fd` | [gpui: Keep Unicode separators on one line on Windows (#66159)](https://github.com/zed-industries/zed/commit/979cd3fdf02f) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/direct_write.rs:503` shapes split_inclusive U+2028/U+2029 segments on one baseline, partitions FontRuns, rebases source positions and preserves ascent/descent. Native/source-index regression equivalents are in `direct_write/layout_tests.rs`. |
| `bed9563afc7af5363821f5547f7766c77c57e497` | [gpui_apple: Keep the checkout path out of compiled Metal shaders (#66166)](https://github.com/zed-industries/zed/commit/bed9563afc7a) | [P08](#p08) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `484bcbfea6ec20b8a973404d2899879deec2bf20` | [feat(gpui): Allow building gpui against ohos (#64579)](https://github.com/zed-industries/zed/commit/484bcbfea6ec) | [P25](#p25) | `source-reconciled` | Complete OHOS Cargo/public-linux-factory/current-platform guards implemented; source-reconciled, desktop selector controls compile in F-WORKSPACE/F-PUBLIC. No in-tree backend or OHOS target run claimed. L-PLATFORM. |
| `2c335b81dc94d109bfcf05e8e54aba208837b33a` | [Use tree-sitter-go fork (#64562)](https://github.com/zed-industries/zed/commit/2c335b81dc94) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:tree-sitter-go. |
| `e30095febcfd25ae81d52e59d7d80c2bd21830c6` | [Update Tree-sitter and extension grammar memory imports (#66185)](https://github.com/zed-industries/zed/commit/e30095febcfd) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:tree-sitter, patch:tree-sitter-language. |
| `370591f3d178718e3a822cfbf2a5879cdcccfad1` | [gpui: Defer native window close cleanup to avoid reentrant app borrow (#66181)](https://github.com/zed-industries/zed/commit/370591f3d178) | [C-R](#c-r), [P18](#p18) | `source-reconciled` | Native Closed/CloseRequired delivered through queued generation-scoped App ingress; does not borrow App in synchronous callback. |
| `dde76a1dca5305607a940309ada0568350d1aac6` | [gpui: Some gpui cleanups (#66156)](https://github.com/zed-industries/zed/commit/dde76a1dca53) | [C-T](#c-t), [P11](#p11), [W-SHARED](#w-shared) | `source-reconciled` | Equivalent shared-render cleanup: fork PrimitiveTransform owns checked translation/composition; no obsolete mutability or second upstream TransformationMatrix owner is required. The Core scratch Cell cleanup is already in the Core report. |
| `f2a60bb8b6c162593f38c02601e41083270d0433` | [title_bar: Fix project name shown for `repo.git` bare clones on Windows remotes (#66199)](https://github.com/zed-industries/zed/commit/f2a60bb8b6c1) | [H07](#h07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `fdcb1981103408e63fff5ef47c8ae2f9995262ad` | [gpui: Smooth frame pacing in macOS Low Power Mode (#63477)](https://github.com/zed-industries/zed/commit/fdcb19811034) | [C-R](#c-r), [P10](#p10), [H17](#h17), [W-WINDOWS](#w-windows) | `implemented-local-verified` | Windows caches initial battery-saver status, registers GUID_POWER_SAVING_STATUS, validates unaligned payload lengths/values, emits changes and unregisters. Capture remains test-support-gated. Mac cadence is owned by the existing platform report. Selected iOS source complete, seven prior findings closed, F-IOS compiles; native iOS 26.3 success-marker receipt now passes for the bounded probe. L-IOS. |
| `e4a3732b94eb0881e1d954201ed41ffd2abf18f6` | [gpui: Fix text system reference cycle through pooled line wrappers (#66206)](https://github.com/zed-industries/zed/commit/e4a3732b94eb) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `24a2823456f16e36bf32df510fb5c43fee7a2565` | [gpui_wgpu: Fix WebGL surface reconfiguration panics (#66190)](https://github.com/zed-industries/zed/commit/24a2823456f1) | [W-WGPU](#w-wgpu), [D-SOURCE](#d-source) | `source-reconciled` | Root source reconciled through the linked owning audit: dep:wgpu. WGPU and shader-validation Naga use the same final cddb2f5d80dc0009b2ef6767635140cacf90a007 source, including the actual WebGL resize correction rather than registry 30.0.1. |
| `5e8998a2a704a16e56ea8540bdd77079a480ea09` | [Update agent-client-protocol to 3.6.0 (schema 1.13) (#66245)](https://github.com/zed-industries/zed/commit/5e8998a2a704) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `2580a00264a799d76401f12bde3f9ade219212c4` | [gpui: Apply frame limits consistently and stabilize pacing (#66237)](https://github.com/zed-industries/zed/commit/2580a00264a7) | [C-F](#c-f) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `95218401529ea1ef8687403ed100dec67774caa8` | [path: Reimplement `PathStyle::join` based on the standard library's implementation (#65966)](https://github.com/zed-industries/zed/commit/95218401529e) | [H07](#h07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `44ec5691b55166d49dde21e2b3248558837d3812` | [gpui_apple: Start font-size alternation at the exact size (#65728)](https://github.com/zed-industries/zed/commit/44ec5691b551) | [P09](#p09) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `4d847859f5506471bd80ca78ee6ced161b19a4e8` | [gpui_linux: Fix Wayland activation requests (#64058)](https://github.com/zed-industries/zed/commit/4d847859f550) | [P13](#p13) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `02d47bf64fcece0edc5b849d25e3a60a67d01c83` | [http_client: Unify asset downloading logic (#66048)](https://github.com/zed-industries/zed/commit/02d47bf64fce) | [H10](#h10), [D-SOURCE](#d-source) | `source-reconciled` | Root source reconciled through the linked owning audit: dep:github_releases, members:crates/github_releases. |
| `7f9a39a9c08373dcc62599906f8dc335d358d7c4` | [Bump livekit-rust-sdks to upgrade zip to 9.1 (#66272)](https://github.com/zed-industries/zed/commit/7f9a39a9c083) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: patch:libwebrtc, patch:livekit, patch:webrtc-sys. |
| `f0210f1621c6be9df4510a4356fa5049680e1c01` | [gpui: Use UAX #14 line breaking for text wrapping (#64624)](https://github.com/zed-industries/zed/commit/f0210f1621c6) | [C-T](#c-t), [D-TEXT](#d-text) | `source-reconciled` | Adopted ICU 2.3.0 default-features=false/compiled_data declaration and actual non-complex-script LineSegmenter source route. Root 2.3.0 / independent-smoke compatible 2.4.0 are recorded without claiming every release/script/performance property. |
| `2c2f737ab2f3db793b7a313bc8e5307f1708d2e8` | [gpui: Add sendable foreground execution for web HTTP clients (#66165)](https://github.com/zed-industries/zed/commit/2c2f737ab2f3) | [C-R](#c-r), [P12](#p12), [H01](#h01) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `7c8dbadcb5c39270eba8d45bc040fd626684a748` | [gpui: Make power and thermal frame intervals configurable (#66281)](https://github.com/zed-industries/zed/commit/7c8dbadcb5c3) | [C-F](#c-f), [P10](#p10) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `9cb5f90e3c493fb273230b14b95047fb41c5eab6` | [Suppress synthetic WM_MOUSEMOVE during window repaint (#55739)](https://github.com/zed-industries/zed/commit/9cb5f90e3c49) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/events.rs:1473` compares last logical client position before callback dispatch and suppresses equal no-button repaint events, while physical-frame scopes and held-button input remain authoritative. Pointer leave/capture changes invalidate the relevant cached state. |
| `411266680135a2e89c757d8c677dca20defe8cbb` | [gpui_macos: Update Spanish-ISO shortcut mappings (#66283)](https://github.com/zed-industries/zed/commit/411266680135) | [P07](#p07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `bc7041c600e3da82713849f4d65aa4a3cac89f67` | [Fix standalone modifier sequences not being flushed after timeout (#55800)](https://github.com/zed-industries/zed/commit/bc7041c600e3) | [C-F](#c-f), [P06](#p06) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `2529740084edf8c736e9d316edbcf56d40c1cf0f` | [gpui: Wrap arena in an Rc to prevent UB (#66293)](https://github.com/zed-industries/zed/commit/2529740084ed) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `0373366f43e38375f344c733d0dbc9f856333366` | [gpui_macos: Fix animations running at 30 fps on M5 Macs (#65472)](https://github.com/zed-industries/zed/commit/0373366f43e3) | [P08](#p08), [D04](#d04) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `e20d19d32f25734616243f15b7bfecefc633b8d7` | [gpui_wgpu: Configure the window surface once at creation (#66002)](https://github.com/zed-industries/zed/commit/e20d19d32f25) | [W-WGPU](#w-wgpu) | `source-reconciled` | `G/src/wgpu_context.rs:681` tests the actual renderer SurfaceConfiguration. `wgpu_renderer.rs:414/:600` skips the second configure for a newly selected context, configures later windows, and retains generation-bound resize/recreation tickets. |
| `8fa085e397fd106166eb055f74a6e9d5e674d02f` | [gpui: Repaint cached views when inherited opacity changes (#65951)](https://github.com/zed-industries/zed/commit/8fa085e397fd) | [C-V](#c-v) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `99e70ee4a02d8dcf1b72f487f81aaf0d3be74f76` | [gpui: Time animations with the executor clock (#66317)](https://github.com/zed-industries/zed/commit/99e70ee4a02d) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `14b7b0ef3764d62a85070c8b5ccea105a2ad3235` | [gpui_linux: Replace kde-blur protocol with ext-background-effect protocol (#53746)](https://github.com/zed-industries/zed/commit/14b7b0ef3764) | [C-F](#c-f), [P15](#p15) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c408f8efc41e51099a031d98190be773d7a5e713` | [gpui: Improve borrow and window error diagnostics (#66358)](https://github.com/zed-industries/zed/commit/c408f8efc41e) | [C-R](#c-r) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `f816e1f9263e3a8ef63c4baaa02e1441ce1c6038` | [gpui: Render macOS window screenshots offscreen (#66344)](https://github.com/zed-industries/zed/commit/f816e1f9263e) | [P08](#p08) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `26abd1c9590827ffc5cca3058ff2a94ea9a34715` | [gpui_platform: Extend benchmarks and add a two-revision comparison script (#66316)](https://github.com/zed-industries/zed/commit/26abd1c95908) | [C-R](#c-r), [P22](#p22), [G-M13](#g-m13), [G-BENCH](#g-bench) | `implemented-local-verified` | Historical paths/benchmark omissions fixed: with_paths, four additional targets, five non-tree workloads, full 17-mutation ChildText chain and comparison tooling adopted. F-BENCH compiles; Python 5/5. L-BENCH; native numerical measurement unexecuted. |
| `decd2399b180b64dea2a4182f9ac63545a45e7a3` | [gpui: Keep subscriptions for entities read by reused cached views (#65952)](https://github.com/zed-industries/zed/commit/decd2399b180) | [C-V](#c-v) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `b41a8b60a470e05b3bbb65512d47e7538dbb00e0` | [Fix Windows shell detection for uppercase executable names (#66387)](https://github.com/zed-industries/zed/commit/b41a8b60a470) | [H06](#h06) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `bc6fc117403dd6dc872072626fbc8269c0c06cf1` | [gpui_windows: Keep child windows above GPUI content (#66368)](https://github.com/zed-industries/zed/commit/bc6fc117403d) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/directx_renderer.rs:1052` calls CreateTargetForHwnd(hwnd, false) for startup/recreation, preserving native child HWND layering above GPUI content. |
| `4ec98df5000b21dca9aa1a064d770fb6ebc63880` | [acp: Update agent-client-protocol to 5.0.0 (#66425)](https://github.com/zed-industries/zed/commit/4ec98df5000b) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `f73feed9de36c5e3eb82aa89baa8eaef5ffc9c98` | [http_client: Remove anyhow reexports (#66430)](https://github.com/zed-industries/zed/commit/f73feed9de36) | [H10](#h10) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `0294cfb58e8e24d12be7596f56bdb7a53e3dfd2f` | [gpui_windows: Preserve native tab advances (#66468)](https://github.com/zed-industries/zed/commit/0294cfb58e8e) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/direct_write.rs:1237` handles zero-glyph runs containing tabs, queries native HitTestTextPosition metrics and keeps invisible space glyph entries for source/caret mapping. Mutable context, native layout and nullable strings are retained; layout_tests covers native/tab/cursor variants. |
| `ccb25cbc637616c9067f3a7f61bf7685c37326be` | [crates_io_client: Add crates.io API client (#66437)](https://github.com/zed-industries/zed/commit/ccb25cbc6376) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: members:tooling/crates_io_client. |
| `ce6c5413f1a2eaa8ac58be82cfd701393cff9470` | [ui: Open submenus and popovers on the side that has room (#66496)](https://github.com/zed-industries/zed/commit/ce6c5413f1a2) | [C-E](#c-e) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `e21ff1b368b074586e4dae0d9bbf15d5dd8a1476` | [Fix opening `path:line` in a running Zed on Windows (#66501)](https://github.com/zed-industries/zed/commit/e21ff1b368b0) | [H08](#h08) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `c7bea343838f42b4c14151393e41f0bbe033c95e` | [gpui: Fix cursor reverting when a drag starts (#66540)](https://github.com/zed-industries/zed/commit/c7bea343838f) | [C-F](#c-f), [P06](#p06) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `d01f1258376ec2fb498d8386e734b0dabdbb9fe5` | [gpui: Fix key equivalents for 'backspace' and 'delete' on macOS (#65744)](https://github.com/zed-industries/zed/commit/d01f1258376e) | [P07](#p07) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `635026fabc38440fce5e1767d125026a17b12012` | [gpui: Support animated WebP in `Image::to_image_data` (#66459)](https://github.com/zed-industries/zed/commit/635026fabc38) | [C-E](#c-e), [P11](#p11) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `4a6121bb0f10c6e8037b741b7dc0a4cd639ae289` | [gpui_windows: Composite COLR layers on the CPU (#66588)](https://github.com/zed-industries/zed/commit/4a6121bb0f10) | [Q-COLR](#q-colr), [W-WINDOWS](#w-windows) | `implemented-local-verified` | Current `W/src/direct_write.rs:740/:796/:854/:997`, `color_glyph_bitmap.rs`, build.rs and platform/device paths adopt recommended rendering/grid-fit modes, union nonempty COLR-layer bounds, transparent CPU source-over planes, contrast/gamma correction and straight BGRA output. GPUState, GPU color shader and text recovery callbacks are removed. Helper extraction validates malformed coverage rather than publishing partial success. Portable tests pass F-ALL and real-sysroot all-target compilation passes F-WINDOWS; native DirectWrite/readback unqualified. L-GPU. |
| `416cd8496390721706e6de3514dca8f2ae84063b` | [gpui: Fix `testing` example not notifying after async task (#66599)](https://github.com/zed-industries/zed/commit/416cd8496390) | [C-X](#c-x), [Q-EXAMPLE](#q-example) | `implemented-local-verified` | Both async updates notify in actual testing example; explicit test-support platform target compiles in F-WORKSPACE and participates in F-ALL. L-CORE. |
| `7bb5656e63eff51f232982854c240c4593ab5b39` | [gpui: Implement `Along` for `Edges` (#66614)](https://github.com/zed-industries/zed/commit/7bb5656e63ef) | [C-F](#c-f), [Q-LIST](#q-list) | `implemented-local-verified` | Edges implements Along for complete List padding unit; F-ALL exercises padded layout/geometry. L-CORE. |
| `78fde14d124dbbb7c6175ec2e25da7028b0573d6` | [gpui: Fix ignored horizontal padding on the `list()` element (#64056)](https://github.com/zed-industries/zed/commit/78fde14d124d) | [C-E](#c-e), [Q-LIST](#q-list) | `implemented-local-verified` | Horizontal origin/width/scrollbar/cache correction adopted. Two reviewed vertical-inset defects shared with upstream fixed in bounds_for_item/max_scroll_offset; real 0/1 red becomes passing y=10/range=20 and reveal=-20 in F-ALL; re-review closed. L-CORE. |
| `94f3eb1b7224ec1daf3e8a3029f345e98c6b4816` | [Bookmark picker (#60005)](https://github.com/zed-industries/zed/commit/94f3eb1b7224) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:project_bookmarks, members:crates/project_bookmarks. |
| `4ce4b7b567acbe9a9f86a027458f8122c626d8a7` | [gpui: Move a Div's Interactivity out of line (#66719)](https://github.com/zed-industries/zed/commit/4ce4b7b567ac) | [C-E](#c-e), [P22](#p22) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `62682ebde9e70b8a77a95863a2b16e9d59bf65eb` | [gpui: Compute tab stop order only when moving focus (#66721)](https://github.com/zed-industries/zed/commit/62682ebde9e7) | [C-I](#c-i), [Q-TAB](#q-tab) | `implemented-local-verified` | Accepted dispatch-tree tab order constructed in batch on focus movement, retaining group placement/missing-node handling; F-ALL passes. L-CORE; no measured cost claim. |
| `4023e3c88e0a4fdc193cde84784c1d8f60588a3e` | [gpui: Only take a line wrapper when truncating text (#66720)](https://github.com/zed-industries/zed/commit/4023e3c88e0a) | [C-E](#c-e), [Q-TEXT](#q-text) | `implemented-local-verified` | Wrapper acquired only for real truncation; repaired lifetime fixture requests truncation while retaining cache/weak-release assertions. F-ALL passes. L-CORE. |
| `22580c6e6970de1aebf653a012765025fe8bf0f7` | [gpui: Trim per-line and per-glyph work in the text system (#66722)](https://github.com/zed-industries/zed/commit/22580c6e6970) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `ddb90e5e89d346f21847281727e1644b8bb2cc37` | [gpui: Don't copy the text of every shaped line (#66682)](https://github.com/zed-industries/zed/commit/ddb90e5e89d3) | [C-T](#c-t) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `39467cf4b1b9db03d180312a0ae681f59f85fe08` | [gpui: Remove hand-written IntoElement impls for elements (#64864)](https://github.com/zed-industries/zed/commit/39467cf4b1b9) | [C-V](#c-v), [Q-ELEMENT](#q-element) | `implemented-local-verified` | Blanket Element-to-IntoElement adopted across Core/components/ordinary custom elements; parent imports and undersized fixture arena repaired without weakening assertions. F-ALL/F-PUBLIC pass. L-CORE. |
| `47c46191f4e46fa0c761b7fedeb60218803ca60f` | [gpui_macos: Handle unavailable pasteboards (#66736)](https://github.com/zed-industries/zed/commit/47c46191f4e4) | [P05](#p05) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `1fea6cbaf6891b3bf90d02607215069593674950` | [Bump agent-client-protocol to 5.1.0 (#66748)](https://github.com/zed-industries/zed/commit/1fea6cbaf689) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `bfcd5949ff8200e213d863154722688065ab5526` | [gpui_linux: Time Wayland frame callbacks from receipt (#66774)](https://github.com/zed-industries/zed/commit/bfcd5949ff82) | [P04](#p04) | `source-reconciled` | Wayland frame callback captures receipt Instant::now rather than undefined callback clock. Complete source reconciled under L-PLATFORM; Linux target/compositor execution unqualified. |
| `0929d569fa3e1b1968b7c486f09c599fe1334587` | [client: Report Linux desktop environment and connected display server in telemetry (#66772)](https://github.com/zed-industries/zed/commit/0929d569fa3e) | [C-F](#c-f), [P04](#p04) | `source-reconciled` | Platform helpers report Unix desktop environment/display server and None elsewhere; complete source reconciled under L-PLATFORM. Product telemetry excluded; current Linux execution unqualified. |
| `b1962f7cd8569ecade486c40320e3cff2d4b9d22` | [repl: Update Jupyter dependencies (#66220)](https://github.com/zed-industries/zed/commit/b1962f7cd856) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:jupyter-protocol, dep:jupyter-websocket-client, dep:jupyter-zmq-client, dep:nbformat, dep:runtimelib. |
| `415d310e5260ae512b4e9e4981b44624db2ec9a3` | [gpui: Avoid copying and comparing the bytes of shared text (#66728)](https://github.com/zed-industries/zed/commit/415d310e5260) | [C-E](#c-e), [H02](#h02) | `implemented-local-verified` | Package equality/hash and Core static-text entry adopted. Package 3/3 at 1660391a, run 3617aa39-45b0-4245-9530-d2d1b0160d7d; Core integration passes F-ALL/F-PUBLIC compilation. L-SHARED/L-CORE need separate full hashes. |
| `985bbf42e684ef5b5a9312a6abf075aeafbaf5b9` | [Apply glyph mark offsets in CosmicTextSystem layout (#66799)](https://github.com/zed-industries/zed/commit/985bbf42e684) | [Q-COSMIC](#q-cosmic), [W-WGPU](#w-wgpu) | `implemented-local-verified` | Actual Cosmic glyph construction applies x+x_offset*font_size and y-y_offset*font_size, retaining original glyph/source rebasing. Initial mark regression fails; focused 111/111 and F-ALL pass. L-GPU. |
| `5014e5b3027dded43ab504619565eecd794465d3` | [Update ACP SDK to 5.2.0 and adopt schema 1.17 additions (#66832)](https://github.com/zed-industries/zed/commit/5014e5b3027d) | [D-PRODUCT](#d-product) | `outside-framework` | Root evidence: dep:agent-client-protocol. |
| `64ef8026c7bc3b29b53c57ebcd6e67a0d9a63ef0` | [gpui: Resolve window placement from active window (#61459)](https://github.com/zed-industries/zed/commit/64ef8026c7bc) | [C-R](#c-r), [P03](#p03) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `ba06cc519102da1b1e15d2ae744ce31a6052f292` | [Add setting to show/hide title bar (#65671)](https://github.com/zed-industries/zed/commit/ba06cc519102) | [C-R](#c-r), [P02](#p02) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `3d42ca7c0959010bf1ef2c3307769cd32cd7945f` | [cli: Add debug state command (#66852)](https://github.com/zed-industries/zed/commit/3d42ca7c0959) | [C-R](#c-r), [P01](#p01) | `source-reconciled` | Source reconciliation is defined by the linked owner group(s); no new execution credit. |
| `78f5d7446c55f6c1aa9b66e8111f6b3f4625ebb0` | [gpui_windows: Fix double borrow crash from display change handler (#66855)](https://github.com/zed-industries/zed/commit/78f5d7446c55) | [W-WINDOWS](#w-windows) | `source-reconciled` | `W/src/events.rs:2176/:2183`, topology coordinator and Core native ingress defer App-dependent display delivery beyond active borrows. `native_display_observation_tests.rs` explicitly pumps native display messages while App remains borrowed and retires stale callbacks; no second ad-hoc defer is needed. |
| `ba490197cfb8aed3514544e129ed83d9437ed56d` | [gpui: Manually clip primitive geometry (#66806)](https://github.com/zed-industries/zed/commit/ba490197cfb8) | [RENDER-1](#render-1), [W-RENDER](#w-render) | `source-reconciled` | Adopted/adapted. Vertex-stage geometry clipping in `W/src/shaders.hlsl` and `G/src/shaders.wgsl` uses clipped unit/local coordinates; invalid/projection-sensitive cases fall back to original geometry. Half-open exact clip tests remain in fragments. Tile sampling uses clipped UVs with the existing tiny-image edge treatment. Subpixel shaders use the same fork clip owner. |
| `caead827d77c3ae492eb47db9f151073e2160e1a` | [gpui: Reduce redundant work in pixel shaders (#66807)](https://github.com/zed-industries/zed/commit/caead827d77c) | [RENDER-2](#render-2), [W-RENDER](#w-render) | `implemented-local-verified` | WGPU flat/simple polychrome branch returns before load_poly_sprite, preserving opacity/clip/scene color; pad!=0/grayscale/rounded cases keep slow path. HLSL early return retained. F-ALL shader/image controls pass; no measured reduction. L-GPU/L-RENDER. |
| `ffe0fa29e41ee16c948ccbc312edd304b9bf7a3f` | [gpui: Improve path rendering (#66809)](https://github.com/zed-industries/zed/commit/ffe0fa29e41e) | [RENDER-3](#render-3), [W-RENDER](#w-render) | `source-reconciled` | Adopted. Shared PathBuilder/PathWinding produces monotone quadratic bins and PathTile/PathCurve/PathPaint. `W/src/directx_renderer.rs:709/:1381` and HLSL path_tile entries replace MSAA raster/composite pipelines. `G/src/wgpu_renderer.rs:2149` plus storage and WebGL decoders share direct analytic tiles. Local paint bounds/transform retain correct gradients and patterns; dependency/example migration is present. CPU winding implementation is also reconciled in the Core report. |
| `1d8eda94bd4d9bb8cbbfcfacdd3d25d8b96efc31` | [gpui: Improve primitive batching (#66810)](https://github.com/zed-industries/zed/commit/1d8eda94bd4d) | [RENDER-4](#render-4), [W-RENDER](#w-render) | `source-reconciled` | Adopted/adapted. Core BoundsTree uses kind-aware overlap and actual primitive extents; Scene batches retain exact texture-instance compatibility and validity-aware replay. All renderers consume final shadow raster bounds separately from blurred shape bounds. The platform benchmark workloads are now adopted under G-BENCH/L-BENCH and compile in final F-BENCH; no numerical result is inferred. |
| `be07e43ea0ad8d5dbca37b93e0b2be0fb8e08014` | [gpui: Reduce overdraw using the depth buffer (#66811)](https://github.com/zed-industries/zed/commit/be07e43ea0ad) | [RENDER-5](#render-5), [W-RENDER](#w-render) | `source-reconciled` | Adopted/adapted. Both backends have private depth, reverse opaque quad prepass, Greater depth tests, per-batch quad-cursor depths, regular paint-order blending and resize/recreation. `G::adapter_supports_transient_attachments` guards browser availability. WgpuSceneRecording retains private depth/globals/uploads through host completion; it installs no device callbacks or submission owner. Core safe partitions retain exact clips. |
| `8a5cf7e23a7ce5797bb020a4b7be2ef1b7dc8f79` | [gpui: Use Levien's shadow approximation (#66812)](https://github.com/zed-industries/zed/commit/8a5cf7e23a7c) | [RENDER-6](#render-6), [W-RENDER](#w-render) | `source-reconciled` | Adopted. WGSL/HLSL shadow functions use the Levien rounded-rectangle approximation and retain algorithm/source references, separate shape/raster extents and inset clipping. Shared Apple Metal is already reconciled by the platform owner; no duplicate Metal implementation task is opened. |
| `6ceb6fee8738274589b59139df27efc191ec2231` | [git_ui: Fix "entity released" errors when closing the branch picker early (#66873)](https://github.com/zed-industries/zed/commit/6ceb6fee8738) | [PRODUCT-FINAL](#product-final) | `outside-framework` | Official patch changes only crates/git_ui/src/branch_picker.rs; the picker owns tasks instead of detaching them. |

## Remaining handoff and mechanical validation

Every full SHA remains in the canonical table, with its original title, source link
and evidence-group provenance. All six former source-open classifications and every
former D-OPEN root slice have explicit dispositions. Selected code is implemented;
there is **no unexplained or open source contribution** at the fixed target.

- **Source closure:** exact EGL correction adopted with real-binding positive and
  registry negative controls; root30 is 18 adopted / 3 retained-contract / 9
  product-only. Aggregate root30 rows are 19 source-reconciled / 3
  implemented-local-verified / 8 outside-framework. Retained scap/tooling/version
  contracts remain explicit.
- **Completed local gates:** F-ALL **3615/3615**, three pre-existing ignored AppKit
  cases; F-PUBLIC **8/8** and independent all-target compilation; workspace
  all-target, real clang-cl Windows all-target, iOS simulator all-target/all-feature
  plus independent consumer/device lib, production wasm and actual a11y wasm
  dev-target, final platform/Cosmic bench compilation, and real Rust 1.93 host and
  simulator checks pass. The matrix preserves each gate's actual scope.
- **Review closure:** the List horizontal unit includes two additional shared
  upstream vertical-inset fixes with a real 0/1-to-passing test; all seven prior
  iOS source findings and the complete 17-way ChildText chain are independently
  re-reviewed closed. Historic build/fixture/red-test failures and exact repairs
  remain above.
- **Native/static closure:** the final iPhone 17/iOS 26.3 probe prints its required
  success marker and the imported native text protocol smoke passes. Host/iOS
  strict correctness/suspicious Clippy, root/independent formatting, import/API/
  docs/release/dependency-health gates pass. The benchmark smoke completes across
  all five targets; its unoptimized churn diagnostic follows the original 55-s
  timeout with a successful 93.465-s run. Physical devices/input, Windows/Linux/
  browser native behavior, Windows resource output and numerical comparisons
  retain their explicit owning-runner limits.
- **Local commit map:** every canonical row reaches its exact baseline or current
  full local SHA through the linked evidence groups and L-* map, including both
  package and Core implementations of official 415d310e5260. Source checkpoint
  `138c6ef22c4ce8bdacad800bd1ae2cebd2cf0c44` is committed locally; this cycle has
  not pushed. Historical failures and limited qualification are preserved.

The one-time extraction checked the historical Git set and canonical table in both
directions, and separately checked patch-header SHA/title identity. Title comparison
decodes Markdown/HTML escapes and normalizes whitespace only. The full 31-directory/file
pathspec, all 149 root entries and all seven fixed patch headers were checked again
after the earlier source follow-up. The current documentation closure rechecks row
uniqueness/counts, the inherited SHA-set fingerprint, table structure, local links
and owner mapping. No maintained scanner or reconciliation script is added. The
historical extraction and current document-integrity receipt is:

| Check | Actual result |
| --- | --- |
| Historical relevant SHA set | 557 distinct commits |
| Root manifest/toolchain history included in that set | 149 / 149 |
| Later fixed patch SHA set | 7 distinct commits: six renderer and final git_ui |
| Expected union / ledger rows / distinct table SHAs | 564 / 564 / 564 |
| Expected minus ledger | 0 |
| Ledger minus expected | 0 |
| Duplicate full-SHA rows | 0 |
| SHA/title mismatches | 0 |
| Local file and anchor links checked | Current closure: 1,144, zero unresolved; historical extraction: 1,146, zero unresolved |
| Former root30 / six-source entries with explicit source decisions | 30 / 30 and 6 / 6; EGL implementation gap closed with exact pin/locks and real-binding controls |
| Markdown table structure / temporary evidence paths / trailing whitespace | Valid / 0 / 0 |
| Windows/WGPU primary rows with source dispositions | 81 / 81 |
| Current aggregate dispositions / sum | 429 source-reconciled + 54 implemented-local-verified + 81 outside-framework + 0 open = 564 |
| Sorted full-SHA-set SHA-256 | `cf53075acfb33590d870994c30590bdaa0df9bf3c623e68ff81e0815e18132ef` |

This mechanical receipt establishes inventory integrity; the source groups and
matrix separately establish implementation and scoped executed qualification.
Unexecuted native/input/performance scenarios receive no credit from row counts.
