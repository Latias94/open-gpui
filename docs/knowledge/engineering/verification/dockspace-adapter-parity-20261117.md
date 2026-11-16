---
type: Verification Evidence
title: Dockspace Adapter Parity and Interaction Repairs
status: partial
timestamp: 2026-11-17T02:04:09+08:00
base_commit: e210c52a70dd069baccfe2b29984f77d8cb83bf8
git_commit: 82e38c4d7ff0e866b43ee2549d5070be9d9e5b50
---

# Dockspace Adapter Parity and Interaction Repairs

## Fixed sources and ownership

Open-GPUI starts at `e210c52a`. The dependency moves from
`e8120864e06de445e1b588b07d6b322bb7125e41` to the published dockspace
`362aa4c921c1d9d5e5fba08a4c0f8b33ab36cb91` (22 commits). The remote main
reference was independently checked before adoption. The workspace and both
independent consumer lockfiles use that exact source; no other dependency is
upgraded. Local uncommitted dockspace close/tab-chrome work is excluded.

The integration review follows the selected revision's
[skill](https://github.com/Latias94/dockspace/blob/362aa4c921c1d9d5e5fba08a4c0f8b33ab36cb91/skills/dockspace-integration/SKILL.md),
custom-host and multiview references, the egui adapter, native coordinator,
ADAPTER_TESTING entry-point inventory and egui chrome design baseline.
That design explicitly distinguishes implemented APIs from future group controls.
No alternative layout owner, application state system or compositor is introduced.

The previous [Windows qualification](dockspace-e812-adoption-20261116.md) and
[editor recovery evidence](dockspace-editor-recovery-20261116.md) retain their
original revisions and physical receipts. They are not rerun receipts for this
new dependency or adapter. This follow-up changes the docking adapter, not the
official GPUI focus or frame scheduler contracts.

## Upstream commit ledger

The implementation commit recorded below pins the complete range. Egui-specific
patches are mechanisms to compare, not Rust files to copy into the retained host.

| Dockspace commit | Change | GPUI disposition |
| --- | --- | --- |
| `a4b1e65c1615861f3e2fa525fa4b52f78bf17be9` | fix(core): defer retained presentation stream reclamation | Included by the fixed core dependency; adapter sizing/receiver/output regressions apply. |
| `763fcc34c41f5d3ee08918efcd4bba42f23882a6` | test(runtime): compose suspended updates with late output terminals | Upstream test evidence retained; not counted as GPUI or OS execution. |
| `13d95b72f69cc1dbef3a8b5cfcedb9d5672e25cc` | fix(core): skip unrepresentable occluded guide placements | Included by the fixed core dependency; adapter sizing/receiver/output regressions apply. |
| `61fdc6708486d0c6b92df4cd9692c1828eaba334` | fix(core): preserve ordered resize and menu scroll adjustments | Included by the fixed core dependency; adapter sizing/receiver/output regressions apply. |
| `f0889e35d9a1bcb3fdb78f6216a29d9232715060` | fix(egui): let accepted accessibility focus supersede pending pane focus | Reference adapter only; reviewed against GPUI focus, semantic input and framework hit ownership. |
| `842f354606a28bae7a1b9807d36df5fd1e495b36` | docs: preserve docking reliability and ImGui design research | Documentation, formatting or upstream CI only; no GPUI runtime transplant required. |
| `ce771856bb2e27e26b73a8835ce48635557c7074` | test(core): shrink semantic command sequences without retargeting | Upstream test evidence retained; not counted as GPUI or OS execution. |
| `e18d4d7f09f63f76defb9f698f9974e18ccf09c6` | fix(core): arbitrate executable pointer lanes by presented stacking | Included by the fixed core dependency; adapter sizing/receiver/output regressions apply. |
| `5cf0bcd9cc10af47799ff68a564549037871886c` | fix(egui): preserve framework widget order when qualifying pointer ownership | Reference adapter only; reviewed against GPUI focus, semantic input and framework hit ownership. |
| `458e6d4d077bbcd2739cec270b6b0fadbee5a6dd` | fix(egui): preserve semantic input order and menu terminal behavior | Reference adapter only; reviewed against GPUI focus, semantic input and framework hit ownership. |
| `1e5499dede78bcd0321b653e2f030f7a74fe6fab` | test(egui): compose semantic packets and retain full timed input traces | Upstream test evidence retained; not counted as GPUI or OS execution. |
| `b7944b31103cf01d5c1956e4c76fe55bd86a2abe` | style: format pointer and focus regressions | Documentation, formatting or upstream CI only; no GPUI runtime transplant required. |
| `c9dd58a84bf7ba8b4d78e16e7ca9ab33ba08f218` | docs: connect the archived research to completed regressions | Documentation, formatting or upstream CI only; no GPUI runtime transplant required. |
| `592f0cee15d5aab98fd2b7819c82173feecc0fef` | docs(research): compare docking architecture and test oracles | Documentation, formatting or upstream CI only; no GPUI runtime transplant required. |
| `93e29168bec1c57b4ca1b5bfd55c8c3a9694959a` | refactor(core)!: separate central allocation and preserve resize intent | Included by the fixed core dependency; adapter sizing/receiver/output regressions apply. |
| `4a8847820f6d6c2396673f76786904077afb8548` | feat(layout)!: support complete pane sizing and preferred restoration | Included by the fixed core dependency; adapter sizing/receiver/output regressions apply. |
| `ea83aa1ce6db24cb0c745603021e6cdb112f1057` | test(layout): cover composable pane sizing lifetimes | Upstream test evidence retained; not counted as GPUI or OS execution. |
| `1c35248d47132c0154225ea5f002d47add9cfe5d` | feat(examples): add interactive pane sizing controls | Sizing interaction reference; GPUI retains its own examples and pane entities. |
| `5d94e8a2ab2961864c2cfe0431bbe6b7c2ef6248` | test(native): control the shutdown service clock | Upstream test evidence retained; not counted as GPUI or OS execution. |
| `32c6c755202d0a23510afbc142bf8a02ecd1eb9e` | fix(ci): keep Wayland runtime sockets out of deep artifact paths | Documentation, formatting or upstream CI only; no GPUI runtime transplant required. |
| `ce82a276a9fd176ee607f24fdae0d33b666a51ac` | test(native): wait for exact idle presentation boundaries | Upstream test evidence retained; not counted as GPUI or OS execution. |
| `362aa4c921c1d9d5e5fba08a4c0f8b33ab36cb91` | docs(egui): define docking chrome design baseline | Design baseline reviewed; unshipped group controls remain explicitly separate. |

## Adapter repairs

| Boundary | Defect or missing connection | Resulting implementation | Owning Open-GPUI commit |
| --- | --- | --- | --- |
| Pane measurements | The host supplied only minimum sizes and could not represent fixed, maximum or preferred content sizing. | `PaneSizing` is retained with each original View; all visible tab metadata is measured; metadata changes revoke measurements. Minimum-only shortcuts remain. | `4d1f0997` |
| Application commands | A successful `update` consumed the only report, hiding stale/policy rejections and asynchronous presentation results. | `update_action(update, map, cx)` maps the exact submitted token before host consumption; a mapper panic still preserves committed report custody. Deferred commit notifications expose document changes and exact transition terminals without affine outputs/effects. | `4d1f0997` |
| Header and splitter keys | Prepared actions were consumed/version-rejected after the first event, and an inert submission packet could remove focus before the next key. | Each event uses the original frozen core witness; tab navigation starts at the focused tab. A bounded weak window subscription continues the original header intent across inert packets. New application focus/blur, terminal content activation and removed hosts revoke it. | `82e38c4d` |
| Menu focus and scope | Clicking a root menu from pane content did not transfer keyboard ownership; root actions were labeled as group/window operations. | Pointer and accessibility opening claim the opener; Escape and dismissal preserve later application claims. Labels identify the complete layout. | `82e38c4d` |
| Overflow scrolling and occlusion | No real wheel entrance existed; a popup's normal hitboxes allowed pane scroll handlers underneath to consume its input. | Real unit/phase/source-aware scroll callbacks target core strip/menu actions. Popup frame/backdrop block underlying input. Precise tails retain their original content/chrome owner, including clamped controls. | `82e38c4d` |
| Custom native bridge | Late collection after natural release could no longer obtain the original release barrier, leaving shutdown blocked. | A public early `NativeCallbacks::observe_capture_release` hook retains the exact barrier; `captured_drag` documents the ReleaseLocker/consumer split. Original FIFO and generation correlation remain. | `2d1010d8` |
| Basic chrome | The group grip had no visible ink and resize controls had no cursor cue. | A vector grip uses the core allocation; operable splitters expose axis cursors. Enter/Space and accessibility Click restore preferred sizes explicitly. | `82e38c4d` |

## Verification

Cargo builds and tests are serialized with `CARGO_BUILD_JOBS=1`, reusing the
workspace target. TestPlatform exercises real framework event dispatch, hit
ordering, retained Views, core state and simulated exact presentation. It does
not qualify physical OS input, capture or scanout.

The first complete implementation passes default docking **183/183** in run
`bcf878ac-e6e4-45f4-8206-6835b9f02768`, and all docking features plus editor,
with leak detection enabled, **1652/1652** in run
`3b3f74b6-fb2a-4cea-8d3c-1e23423de211`. Independent review subsequently finds
additional header/epoch, wheel-start and notification-panic cases. Their
corrections and the separately reproduced split-update clipped activation are
included in the final `82e38c4d` source below.

The final default cohort with leak detection passes **201/201**, run
`eec43686-fb10-4ce2-993a-46c4b2bd66a6`, and all docking features plus the editor
passes **1670/1670**, run `71278e0f-5d86-46f5-bcd1-e7ad88bba6dd`.
Logs: `/tmp/open-gpui-dockspace-parity-qualified-default.log` and
`/tmp/open-gpui-dockspace-parity-qualified-all.log`.

Independent review blocks are fixed before these final receipts: focused versus
selected cycling, activation's terminal key prefix, fully clipped destinations,
old epoch/changed topology, outside/Ctrl scroll starts, multi-region owner
consistency, late tails after reset and panic-safe notification continuation.
The final clipped activation review is independently closed. A split-update
regression first fails in `05167b97-f0f0-4285-85e4-6c3737d209bf`; its corrected
receipt is `d7718a4b-9325-42bf-9b26-be826129dac7`, and the final complete cohorts
also include cancellation by a later application blur. Activation waits for the
exact target presentation and executes after the current focus-stable observers.
Unpresented target geometry cannot grant content input or focus.

| Check | Observed result | Retained local log |
| --- | --- | --- |
| Selected examples, DevTools/docking and all docking targets/features | Passed | `/tmp/open-gpui-dockspace-parity-consumers.log` |
| Windows native example, MSVC all targets with legacy feature | Passed; cross-compile only | `/tmp/open-gpui-dockspace-parity-qualified-msvc.log` |
| Independent dependency consumer, locked all targets | Passed | `/tmp/open-gpui-dockspace-parity-independent.log` |
| Docking library compiler floor | Rust 1.95.0 passed | `/tmp/open-gpui-dockspace-parity-qualified-msrv.log` |
| Independent browser smoke consumer | Locked wasm32 compilation passed; no browser execution claimed | `/tmp/open-gpui-dockspace-parity-qualified-web.log` |
| Formatting, whitespace, public API and documentation links | Passed | `cargo fmt --all --check`, `git diff --check`, `xtask scan-public-api --check`, `xtask scan-doc-links` |

### macOS physical attempt and environment

Three serial `--smoke-native` launches of the pre-review adapter candidate exit
with status 1 at `RootSubmitted`.
The exact diagnostic shows initial generation 1 was Submitted, but the next
original ticket remains Pending with no accepted generation. Both application
windows are native-created and shown, while AppKit reports `Hidden`, no key/main
window and no attached CAMetalDisplayLink. CoreGraphics reports one active
display; IOConsoleUsers reports `CGSSessionScreenIsLocked = true`. Explicit
application activation does not override the locked console. No timer, renderer
success or native acknowledgement is fabricated to advance this smoke.

Retained diagnostic logs are `/tmp/open-gpui-dockspace-parity-native-smoke.log`,
`/tmp/open-gpui-dockspace-parity-native-diagnostic.log` and
`/tmp/open-gpui-dockspace-parity-native-activated.log`. Each launcher observes
the native exit; all three temporary `GPUIProbe.app` bundles are removed while
their stdout/stderr remain. No probe process is left running. The example now
activates on successful startup like the multiviewport example and includes
original host/frame-source diagnostics on timeout (`a68a3811`). The runner also
recognizes the separate recovery smoke's own success receipt.

After unlocking the actual GUI console, rerun serially:

```text
cargo build --locked -p open-gpui-dockspace-editor -j 1
python3 scripts/run_macos_native_probe.py target/debug/open-gpui-dockspace-editor --smoke-native
python3 scripts/run_macos_native_probe.py target/debug/open-gpui-dockspace-editor --smoke-recovery
```

Neither native smoke is newly qualified in this locked session. The earlier
Windows smoke results retain their original source and are not counted here.

## Remaining interaction work

| Area | Current boundary | Next implementation or owning verification |
| --- | --- | --- |
| Group actions | Group drag exists. Published egui design still describes group-menu allocation and atomic captured-group close as future work. | Adopt the eventual committed core group anchor/close-plan contracts; do not loop item closes or silently retarget a root action. |
| Splitter double click | Preferred reset is available through focused Enter/Space and accessibility Click. A double-click reset is not connected. | Order the semantic reset after the original press/release/capture prefix and expose Preferred/Equalize/Disabled configuration. |
| Tab-list key packets | Existing frame-separated navigation is covered. The new header continuation intentionally excludes menu rows, whose focus is core-owned. | Compare coalesced menu navigation/activation/cancellation using the core sequence APIs before promising parity with egui's packet handling. |
| Chrome customization | Current palette/font configuration and core-owned geometry remain; shared vector grip and resize cursors improve the default. | Add density, control feedback, focus decoration and bounded appearance hooks against the selected chrome baseline, without inventing new hit rectangles. |
| Native physical journeys | Prior Windows receipts are bound to their recorded source; macOS native smokes are programmatic lifecycle checks. | Rerun the manifest-owned Windows cohort on the new revision. Mixed-DPI still needs independent real displays; physical macOS capture/stationary sampling retains its explicit backend limit. |

Deferred application notifications are observations, not a persistent event bus.
Applications own debouncing, I/O, retained documents and recovery policy. No old
native bindings, prepared actions or output handles cross a reconstructed host.
