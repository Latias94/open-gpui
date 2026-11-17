---
type: Verification Evidence
title: Dockspace Adapter Parity and Interaction Repairs
status: partial
timestamp: 2026-11-17T10:50:39+08:00
base_commit: e210c52a70dd069baccfe2b29984f77d8cb83bf8
git_commit: 5fc4197370a0b1aab11b9f9936235ea145554482
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

## Ordered chrome follow-up

The follow-up starts at `c6582996632cc1b7e83473608706e850cd176165`. Its final
dependency is the published backport `073b569b43371685964fecde407698412e3c3f0f`,
whose parent is exactly `362aa4c9`. The same core repair is committed on the
sibling's newer main as `4c982919238f9b70490eaa0826f81322d442f363`; that main
has concurrent owner changes and is not pushed by this follow-up. The backport
branch publishes only the reviewed repair. All three consumer lockfiles retain
its exact revision, without an unrelated dependency update.

A tab-list prefix retains the exact
menu session, row roster and prepared focus/activation capabilities. The small
adapter cursor follows the reference egui adapter; core still owns menu focus,
scrolling, permission checks and the final action. The selected core has
`prepare_tab_list_menu_focus`, rather than a dedicated menu keyboard sequence
helper. Terminal confirmation or cancellation yields input immediately. A
replacement must actually present before its witnesses can continue a preceding
committed menu move; an external workspace change or application focus claim
revokes that continuation. `d70eb5b8` owns this adapter change. A pending Escape
also orders a subsequent opener command after its original dismissal; a later
Down cannot toggle the still-open old menu first. The exact mount fallback may
retain this opener input while its control is Inert, but ordinary application
focus and unrelated keys keep their own ownership.

Splitter double clicks use `DockspaceInputBindings::splitter_double_click`:
Preferred by default, Equalize explicitly, or `None` to disable pointer resets.
The semantic action follows the original press/release prefix. Local input and
candidate rollback retain both records together; managed native input retains
the semantic action at its original FIFO position until its own frame commits.
A replacement's Inert controls still expose raw receiver geometry. The press
stores that receiver's identity: a control/splitter visual and a receiver visual
are different identities even when they occupy the same rectangle. Covered
targets and cancelled or dragged presses cannot authorize a reset. Native
release/cancel cleanup remains deliverable through an unpresented replacement,
without granting that replacement pointer authority.

The jitter regression exposes a core limitation beyond adapter event routing:
the original reset was converted to a numeric displacement before a one-pixel
resize changed its weights, making the reset stale. `073b569b` preserves absolute
Preferred/Equalize intent only for `into_deferred()` and validates the original
witness before recomputing current constrained geometry. Direct prepared
actions, relative steps and multi-step sequences retain their strict geometry
contract. The surface separately retains the latest accepted measurement values;
discarding an unpainted scene or falling back to older painted geometry cannot
authorize a reset from superseded measurements. This is a core mechanism repair,
not a new GPUI layout or state owner.

The initial real-event regressions reproduce nine failures in run
`155be5c3-bcdb-403d-a860-96500ff982f7`. Review identifies the receiver/control
identity mismatch before qualification; a later targeted run
`251b4a5b-d48e-4de4-9696-059ec3c21198` passes 14/17 and exposes the remaining
deferred-menu content-focus case. These are diagnostic receipts, not final
qualification.

Independent review also reproduces the pending-dismiss/reopen ordering failure
in `71257399-2c7d-4e89-85d8-eaa465907117`. The corrected complete menu cohort
passes **11/11**, run `bce1f6bc-28f2-449e-8362-27042056a6ce`. Deferred content
focus consumes the next frame scheduled by GPUI itself after the pane becomes
interactive; the test does not force a terminal focus result.

The core backport passes **40/40** consumer cases without motion
(`d1bc3800-9cce-4b79-aa29-6a5dfa6c1d2d`) and **40/40** with motion
(`c0c112b2-e1e1-47e0-b03e-c0b69a82d278`). All core features pass **2434/2434**
(`b8c22b5b-da09-427f-b6a9-d97b26978120`), with three tests skipped by the existing
test configuration. These receipts belong to the backport, not the sibling's concurrently
modified main. The final published core pin passes the GPUI default docking
cohort **220/220**, no skips, run `6356bb3e-f5b5-4b66-b601-76f2334a8f14`,
including both same-stream and separately presented one-pixel jitter.
All docking features and the editor pass **1689/1689**, no skips, run
`5257ed01-e1d4-475a-a9ff-79b1b729a18e`, with leak detection enabled. Logs are
`/tmp/open-gpui-chrome-default-qualified.log` and
`/tmp/open-gpui-chrome-all-qualified.log`.

The final pin also passes Rust **1.95.0** library checking and Windows MSVC
all-feature/all-target checking for docking, its native example and the editor.
Logs are `/tmp/open-gpui-chrome-final-msrv.log` and
`/tmp/open-gpui-chrome-final-msvc.log`. The Windows native fixture repair's
all-feature/all-target cross-compile is separately retained at
`/tmp/open-gpui-chrome-windows-native-check.log`. Cross-compilation does not
establish the native tests' actual submission/pixel result.

| Source or implementation | Disposition |
| --- | --- |
| Dockspace `073b569b43371685964fecde407698412e3c3f0f` | Adopted fixed backport: absolute deferred restoration, original authority checks, latest measurement observation and shared restoration calculation. |
| Dockspace `4c982919238f9b70490eaa0826f81322d442f363` | Equivalent repair on newer local main; independent concurrent edits remain unstaged and main is not published by this task. |
| Open-GPUI `d70eb5b8` | Ordered tab-list navigation, activation, dismissal and opener continuation. |
| Open-GPUI `e322a673` | Pins `073b569b` in the workspace and both independently locked consumers. |
| Open-GPUI `d9add984` | Configurable pointer restoration, original release ordering and retained receiver identity. Adds `DockspaceInputBindings::splitter_double_click`; struct-literal consumers add the field or use `Default`. |
| Open-GPUI `4603c823` | Windows deterministic native fixtures disable inactive pacing without weakening submission/pixel assertions. |
| Open-GPUI `aef2fa73` | Wheel occlusion fixture uses the shared palette and passes the UI contract gate. |
| Dockspace `62ea08cccd5f911a4c97ace034e97a9d3d19bc75` | Reviewed successor; group menu/atomic close and allocation changes remain an explicit integration task, not included by the backport. |
| Dockspace `b70983e5` / `a7223734` / `290046f2` | Observed Web demo loading, upstream CI action tags and native density-fixture follow-ups; excluded from this narrowly pinned core repair. They do not qualify GPUI interaction. |

Final local logs are `/tmp/open-gpui-chrome-default-qualified.log` and
`/tmp/open-gpui-chrome-all-qualified.log`. The final source pin passes Rust
1.95.0 library compilation (`/tmp/open-gpui-chrome-final-msrv.log`), Windows
MSVC docking/native/editor all-feature/all-target cross-compilation
(`/tmp/open-gpui-chrome-final-msvc.log`), and the independently locked wasm
smoke consumer (`/tmp/open-gpui-chrome-final-web.log`). These are compiler
receipts, not physical Windows, macOS or browser interaction receipts.
The independently locked dependency consumer also passes all-target/all-feature
compilation (`/tmp/open-gpui-chrome-final-independent.log`). Formatting,
whitespace, public API tiers and documentation links pass. The UI contract gate
and all of its registered scenario executions pass after the palette repair.

### CI follow-up

The prior head's Verify run `43925362005` has three distinct failures. The
docking wheel occlusion fixture used a new raw RGB literal, violating the shared
palette gate. `aef2fa73` uses the configured palette; the full UI contract scan,
including its registered native scenarios, passes locally. Two Windows native
fixtures combine a frozen virtual clock with real message pumping and default
inactive frame pacing. `4603c823` disables inactive pacing only in those test
windows, following the existing native view-component fixture. Exact accepted
generation, Submitted and pixel checks remain. Windows all-feature/all-target
MSVC cross-compilation passes; actual native execution remains owned by Windows.

The Linux system dependency installation reports no Ubuntu package catalog,
after `apt-get update` lists only the Microsoft feed. A job rerun is requested
without adding fallback repositories or weakening checks. Dependency Health
`43925362157` and Publish Check `43925362047` pass at the prior head. These
statuses do not qualify a subsequent commit.
The palette/Windows fixture head `aef2fa73` subsequently passes Dependency Health
`43992377254` and Publish Check `43992377359`; Verify `43992377289` remains in
progress when the ordered-chrome changes are finalized. Final-head CI is
separate from these local qualification receipts and prior-head statuses.

## Group menus and atomic close adoption

The next implementation starts at Open-GPUI `606b1fe5` and selects published
dockspace `537dff0ffdc3e7f24309be9f2243f786e822980a`. Remote `main` is checked
before changing the pin. This includes both `62ea08cc`'s new group contracts and
`4c982919`, the mainline counterpart of the previous `073b569b` splitter fix.
The workspace and both independent consumer lockfiles share the new source.
Untracked sibling research, regression artifacts and native logs are untouched.

| Dockspace commit | Change | GPUI disposition |
| --- | --- | --- |
| `62ea08cccd5f911a4c97ace034e97a9d3d19bc75` | Scoped tab-group actions, atomic group-close plans, separate group/root chrome allocations and compact geometry. | Render both scopes from their original records. Tabs Float/New Window never become whole-root actions. Group close submits one captured roster with exact decision tokens. Core owns compact allocation and hit geometry. |
| `b70983e5` | Web demo staged Wasm loading and retry. | Reference demo only; no retained-host runtime transplant. |
| `a7223734` | Version tags for upstream GitHub Actions. | Upstream CI only. |
| `290046f2` | Native publication smoke independent of default density. | Upstream test only; GPUI native group tests provide their own framework observations. |
| `4c982919238f9b70490eaa0826f81322d442f363` | Deferred splitter resets survive weight-only changes and preserve the latest measured preferences. | Included in the selected core; the adapter's original release ordering and jitter regressions remain. |
| `8e3a52b4` | Model atomic closes and deferred approval interleavings. | Included upstream evidence; GPUI additionally exercises its facade, retained Views, handlers and exact notifications. |
| `537dff0ffdc3e7f24309be9f2243f786e822980a` | Egui tiles workflow comparison and close-model coverage. | Integration review reference and fixed published source. |

The adapter uses a layout icon for complete-root actions and an ellipsis for
tab-group actions, with explicit accessible scope descriptions. It preserves
core menu records and hides unsupported native commands. The existing update,
close-handler and committed-notification paths carry `TabsClosed`; no new
semantic writer, close loop, View reconstruction or native coordinator is added.
`DockSurface::capture_tabs_group` is a read-only forwarding entry point.

Every opened product menu retains its original surface and session-bound
workspace stamp. Only an installed replacement packet can invalidate that state;
a failed prepaint leaves the installed packet and menu intact. Each opening also
has an independent run identity and popup element IDs, so Escape/reopen cannot
revive an older pointer press or accessibility callback. Keyboard navigation
skips unavailable rows. Opening and confirming in one event batch retains its
ordered semantic prefix until the exact menu is interactively presented; a later
workspace or application focus claim revokes that waiting prefix. A new explicit
key on a refocused opener starts a fresh prefix and discards the old keys. Once
the shown menu supplies its exact command, the existing local/native input FIFO
retains it behind earlier events. No separate UI-gate retry or polling loop is
introduced. Real KeyUp's synthetic button click cannot toggle a menu already
handled by KeyDown. Popup hitboxes block underlying
mouse and wheel handlers, including when the menu itself cannot scroll.

The reference Zed GPUI checkout at
`27bbbc1cc480a7500cec122f120e796d1fb8500c` also synthesizes button clicks on
Enter/Space KeyUp in `crates/gpui/src/elements/div.rs`. The adapter's custom
KeyDown menu handling must account for that inherited mechanism. The correction
stays in docking; the framework's button activation and focus contracts remain
the reference.

New regressions cover `[A,B] / [C,D]` group Float through pointer, keyboard and
AccessKit; whole-root Float retaining the complete split; atomic allow/veto,
deferred tokens, local reorder and sibling changes, membership/epoch/session
invalidation and replay; failed prepaint and stale accessible targets; narrow
strip allocations and real controls; ordered keyboard confirmation, disabled
menus, cancelled presses, real KeyDown/KeyUp, focus reacquisition, no-repaint
native wakes and popup wheel occlusion. The managed native journey
uses the actual group menu and original retained Views. Placement readiness and
Deferred outputs retain AB at the source; exact first-live moves only AB. Native
close plus per-member content approval closes that child while CD and the
original physical host remain usable. TestPlatform physical-display facts are
explicit fixture observations, not evidence of real OS display or pointer input.

### Owning commit and final qualification

| Open-GPUI commit | Upstream source | Adapter ownership |
| --- | --- | --- |
| `5fc4197370a0b1aab11b9f9936235ea145554482` | Published `537dff0f`, including `62ea08cc`, `4c982919` and the complete seven-commit follow-up range above. | Pins all three lockfiles, renders scoped menus, exposes captured groups, reuses atomic-close/native/input owners and adds 24 behavioral regressions. Migration notes name the new exhaustive close variants. |

Independent review identifies three final menu seams: refocusing an opener left
its old keyboard claim permanently invalid; real KeyUp could toggle a menu
already handled on KeyDown; and a Confirm left waiting on a native gate could
lose its only wake. All three are corrected and independently rechecked against
the final paths. Five additional real-event regressions pass in run
`4f5dfbb0-b051-49e2-bbcc-5fb5dba93b7e` and are included in both complete final
cohorts below. No reviewer blocker remains.

All Cargo work uses `CARGO_BUILD_JOBS=1`, one shared workspace target, and serial
nextest execution. Leak detection is enabled for both complete cohorts.

| Check at `5fc41973` | Result | Exact receipt |
| --- | --- | --- |
| Default docking | **244/244 passed** | Run `d551af6f-4fd3-44b6-bfe0-a566c91451a6`; `/tmp/open-gpui-group-default-qualified.log` |
| All docking features and dockspace editor | **1713/1713 passed** | Run `9ecb4d86-1486-4607-bc6f-6fe46e48aa1d`; `/tmp/open-gpui-group-all-qualified.log` |
| Docking compiler floor, all library features | Rust 1.95.0 passed | `/tmp/open-gpui-group-msrv-qualified.log` |
| Windows docking/native example/editor, all targets and features | MSVC cross-compilation passed | `/tmp/open-gpui-group-msvc-qualified.log` |
| Independently locked browser smoke consumer | wasm32 check passed | `/tmp/open-gpui-group-web-qualified.log` |
| Independently locked dependency consumer | All-target/all-feature check passed | `/tmp/open-gpui-group-independent-qualified.log` |
| Formatting and whitespace | Passed | `/tmp/open-gpui-group-fmt.log`; `git diff --check` |
| Public API, documentation links and federated UI contract | Passed, including the UI gate's registered scenario executions | `/tmp/open-gpui-group-public-api.log`, `/tmp/open-gpui-group-doc-links.log`, `/tmp/open-gpui-group-ui-contract.log` |

These are contract and compiler receipts. No GUI probe is launched in this
follow-up. Real Windows/macOS group menus, native physical interaction and
mixed-monitor DPI retain their owning-platform qualification requirements.

## Remaining interaction work

| Area | Current boundary | Next implementation or owning verification |
| --- | --- | --- |
| Group actions | The selected `537dff0f` core and adapter include group menus, atomic close, session-bound menu invalidation and native first-live contract coverage. | Qualify the physical group-menu/native-window journey on the owning platforms; preserve the scope/roster and exact callback guards when customizing appearance. |
| Chrome customization | Current palette/font configuration and core-owned geometry remain; shared vector grip and resize cursors improve the default. | Add density, control feedback, focus decoration and bounded appearance hooks against the selected chrome baseline, without inventing new hit rectangles. |
| Native physical journeys | Prior Windows receipts are bound to their recorded source; macOS native smokes are programmatic lifecycle checks. | Rerun the manifest-owned Windows cohort on the new revision. Mixed-DPI still needs independent real displays; physical macOS capture/stationary sampling retains its explicit backend limit. |

Deferred application notifications are observations, not a persistent event bus.
Applications own debouncing, I/O, retained documents and recovery policy. No old
native bindings, prepared actions or output handles cross a reconstructed host.
