# Dockspace Remote Adoption and Legacy Retirement

## Fixed source and ownership

This increment starts at Open-GPUI
`c47a6b2a27b4b61db24153f1dabe7069aef4f086`. A fresh fetch observes dockspace
remote `main` at `dcd65e983220e17662b1f02929fcf84348cd8eb3`, which becomes
the production dependency. The sibling checkout is already at that commit;
its unrelated modified and untracked files remain untouched. Only committed
objects enter this comparison.

The [source qualification](dockspace-source-qualification-20261114.md) records
all **44** commits since the previous production pin, including the 42 already
reviewed and the two newly published additions. The GPUI adapter retains its
entities, exact candidate/submission tickets, immutable hit graph and native
retirement owners. Dockspace remains the sole topology and gesture authority.
The egui and egui-native adapters at the same fixed revision provide reference
integration paths; their executed tests do not qualify GPUI's OS input.

| Open-GPUI delivery | Commit | Source mapping |
| --- | --- | --- |
| Fixed published dependency and receiver API | `4290e24f` | All 44 source entries in the qualification inventory; receiver naming from `5558c6a4`, checkpoint from `dcd65e98`. |
| Original output/native ingress ordering | `6ed65837` | GPUI-owned adaptation of the egui-native journal barrier; reproduced old artifact-publication failure. |
| Unused legacy experiments removed | `5417474e` | No shared-core source port; twelve prototype tests removed and existing Windows owners retained. |
| Event-time geometry observations | `674e30fc` | Reference scenarios in `9e737172`; native shown/occlusion distinction reviewed independently and covered by a separate control. |
| Shared browser refusal probe and wasm stack | `91c5871f` | Actual Web backend refusal using the shared session, without legacy runtime or changed platform-test root semantics. |

The new receiver names are a public pre-1.0 dependency API change because this
crate reexports `dockspace`:

| Previous name | Current name | Meaning |
| --- | --- | --- |
| `SurfacePointerReceiverFacts::with_no_delivery` | `with_empty_delivery_lane` | The presented click lane is actually empty. |
| `SurfacePointerReceiverFacts::with_no_drag_delivery` | `with_empty_drag_delivery_lane` | The presented drag lane is actually empty. |
| `NativeReceiverAnswer::NoReceiver` | `EmptyLane` | Exact presented evidence supplies no semantic receiver for the queried lane. |

`Blocked`, `Unknown` and `with_clear_hover_lane` retain their distinct meanings.
The previous splitter correction stays in place. The dependency's default
features, including motion, remain disabled; `serde` stays enabled. Docking's
compiler floor remains Rust 1.95, while Core GPUI remains at 1.93.

## Layout recovery

The new `DockspaceView::to_layout()` is directly usable through
`dock.with_view(|view| view.to_layout())`. It copies committed topology,
selection, split weights, central regions and contained bounds/order without
starting a save generation or creating another mutable owner. Reconstruct a
fresh session with application policy/configuration and retained pane entities
only after the old host has drained its physical resources.

The checkpoint excludes external View-key history, document generations,
native hosting preferences, gestures, queued input, native bindings and output
authority. Continue using `capture_document` and the versioned adapter envelope
for durable workspace persistence. Neither a checkpoint nor an error-clear
operation can authorize reuse of old receivers or native bindings.

## Native integration corrections

Comparison with egui-native's journal and the new independent geometry controls
identifies two GPUI integration seams. The normal GPUI prepaint path already
observes the current window before reducing native input. A captured event from
another window can arrive before the target's next prepaint, however; the target
must publish its changed facts at the observation boundary rather than wait for
that redraw.

`NativeCallbacks` now subscribes to committed bounds/scale, display and visibility
changes and captures the current facts into the existing callback FIFO. Weak
callback ownership and the retained bounds-observer entity retire with callback
sealing. Unchanged facts do not enqueue work. Native shown/minimized state remains
distinct from compositor occlusion: a shown but fully occluded source may still
own captured input. The observer does not enter another window's renderer or
synthesize desktop facts. Five deterministic tests retain a real presented
receiver, forbid target drawing and require bounds, scale, display-origin and
minimization changes to revoke that receiver. Round-trip
changes also revoke the old receiver even when the latest geometry equals its
starting value; the FIFO retains the intermediate observation. A sixth control
requires a Hidden presentation/occlusion signal with `native_visible == true`
and `is_minimized == false` to retain the exact receiver without redraw.

The independent reviewer identifies the initial conflation of occlusion and
native hiding. Removing that extra `window.is_visible()` condition and adding
the occlusion control closes the finding; it does not claim physical captured
dragging under an occluder. With only the new observer registrations temporarily
removed, the five observation regressions fail at their pre-redraw transport
assertions, while the occlusion control passes. The exact original-wiring log is
`/tmp/open-gpui-dockspace-geometry-original-red-20261114.log`. The production
registration block is restored immediately after that experiment.

Previously the exact renderer terminal called `report_surface_presentation`
directly, ahead of queued native events. The new regression fails on that path:
the terminal publishes a new artifact before an older device-removal record is
collected. Managed terminals now move their original affine outputs and frame
correlation into the existing native FIFO as a presentation barrier. Collection
stops at that barrier. The host commits its earlier native prefix, settles that
terminal, and only then collects its later suffix. Withdrawals and logical-close
retirement use the same path; queued outputs continue to prevent premature native
quiescence. A renderer callback never fabricates another binding or output.

The regression uses an actual deferred GPUI renderer ticket, verifies its real
Submitted result, preserves the old semantic artifact while the prefix waits,
and checks that collecting the prefix leaves the later device-removal suffix
queued. Final shutdown must still become quiescent. Two older close-report
fixtures now explicitly drain presentation barriers while retaining their
original policy-failure and terminal-suffix assertions.

The initial geometry fixture accidentally allowed TestAppContext to draw while
holding the host borrow; five tests fail with a RefCell reentry. The corrected
fixture disables drawing and retains the existing semantic candidate lease to
hold collection. It does not weaken production borrowing rules. The original
76-test integration attempt also exposes the two older report fixtures and a
test-only attempt to update an already closed window. After those fixture fixes,
the complete default cohort passes **76/76**, zero skipped, run
`1a9c38db-e0cc-4bb1-8aa7-ee6fe822c8f0`.

The independent review also identifies egui's more complete fatal-host recovery
policy as remaining work. GPUI currently records transport errors without an
explicit recoverable/fatal classification and stopped admission state. Adding
that contract requires preserving original cleanup while distinguishing a
retryable capacity/scheduling refusal from a fatal protocol failure. The new
logical checkpoint does not implement this policy, and this increment does not
claim in-place recovery after fatal failure.

## Legacy removal boundary

Ordinary examples, the editor, DevTools' default docking inspection and the
independent consumer already use the shared session. The browser probe has also
migrated in this increment. The Windows dogfood
and its physical scenario runner still depend on the displaced engine.

The following ten behaviors remain owned by
`examples/docking-native/tests/native_windows_interactive.native-scenarios.toml`:

| Behavior | Replacement qualification still required |
| --- | --- |
| `source-capture` | Real captured cross-window preview and drop using the shared session. |
| `complex-dockspace-workflow` | The complete multiwindow journey with topology, content, selection and lifetime observations. |
| `opaque-occlusion` | An opaque front window blocks the underlying shared docking target. |
| `surface-shutdown` | Anchor shutdown releases real capture and retires dependent HWNDs. |
| `provisional-same-hwnd-promotion` | First-live admission promotes the same provisional HWND. |
| `live-route-and-release-lock` | Real routing and pointer-up retain their original destination authority. |
| `committed-loss-recovery` | Destination loss retires committed native authority without losing the View. |
| `process-convergence` | The worker exits after active-workspace shutdown with no native process residue. |
| `no-input-pass-through` | A pass-through front window and generation drift preserve fail-closed routing. |
| `mixed-dpi-placement` | Final target-display client bounds match actual Windows DPI observations. |

The other five manifest behaviors belong directly to the Windows platform
package and remain unchanged. Deterministic adapter tests and Mac Submitted
smoke cover parts of these contracts, but do not replace the ten Windows
journeys. Retain the legacy runtime, dogfood diagnostics, manifest, exact test
selection, receipts and process cleanup until those replacement scenarios run.
The source remains isolated behind `legacy-docking`; it is not a fallback for
a shared workspace.

Unused legacy canonical-layout migration and detached payload experiments are
retired independently: four implementation/test files and their module wiring,
with 1,988 net lines removed. Their only callers are their own experiments/tests.
The default v2/v3 wire-format migrator remains in `adapter::persistence`, and
the old `legacy::layout` still needed by Windows dogfood remains intact. Removing
those **12 tests** (nine migration and three payload/detach) is code retirement,
not additional shared-session test coverage. Twelve other graph kernel tests,
including floating ownership/index checks, remain intact.

## Execution receipts

Cargo commands run serially with one build worker. Tests use the Rust 1.100
compatible nextest runner already installed for this workspace.

| Gate | Executed result |
| --- | --- |
| New dependency, default docking before cleanup | **70/70**, zero skipped; run `028c7edf-bf36-42b4-86e9-02253c2eeca1`. |
| New dependency, all docking features before cleanup | **1546/1546**, zero skipped; run `62442ee8-0c7d-47f9-aefd-61195a59f0d5`. |
| Corrected default adapter, including observation and FIFO controls | **76/76**, zero skipped; run `1a9c38db-e0cc-4bb1-8aa7-ee6fe822c8f0`. |
| Affected packages before final occlusion review | **1795/1795**, one existing ignored fixture-regeneration test; run `5ca1b99a-2bfa-46fd-99ce-6356715be565`. Docking **1540**, DevTools **123**, native dogfood **26**, xtask **106**. Windows-only input cases are not compiled on this Mac. |
| Occlusion distinction and observation controls | **6/6**, run `8f1a04d0-bf70-40b6-9caa-191aa83ac80e`, before the original-wiring red experiment. |
| Final all-feature docking after restoration and review | **1541/1541**, zero skipped; run `51f969ff-bb61-41a3-8e58-abe0c20781ab`. The count is 1546 minus 12 retired prototype tests plus seven new adapter regressions. |
| Final default docking and browser-validator tests | **94/94** selected tests: docking **77**, browser-validator **17**; the other 81 xtask tests are filtered out. Run `ad49689e-c7c3-4e64-9de5-18759ded405f`. |
| Independently locked public consumer | Final `cargo check --locked --manifest-path tests/fixtures/dependency-consumer/Cargo.toml --all-targets` passes with the shared target in 126 seconds. Only the dockspace Git pin changes in its lock. |
| Rust 1.95 | Five affected docking/editor/DevTools/xtask packages pass all-feature/all-target checks; final reviewed docking recheck passes in 19.88 seconds. Core's declared floor stays 1.93. |
| Correctness Clippy | The same affected package set passes `-D clippy::correctness -D clippy::suspicious`; final reviewed docking recheck passes in 24.32 seconds. Existing advisory warnings remain. |
| Real macOS editor | Rebuilt final reviewed adapter completes child creation, two actual Submitted windows, retained Entity/local/shared edits, dependent-before-anchor native terminal and quiescence, and independent sibling retirement. Actual native wait status **0**, normal exit **0**. Temporary app removed. |
| Full real browser at committed implementation `91c5871f` | **Exit 0**, no unavailable skip. WebGL, Auto/WebGPU, explicit WebGPU and unavailable-backend controls pass with their original input/capture/activation and two-viewport pixel assertions. Platform identity, clipboard, text input, font fallback, touch and refusal/cleanup gates also pass. |
| Repository checks | Formatting, whitespace, document links, import boundaries, public API tiers and release-document checks pass. |

The final test/compiler/native logs are
`/tmp/open-gpui-dockspace-final-{reviewed,default-webtests,consumer}-20261114.log`
and `/tmp/open-gpui-dockspace-reviewed-{msrv,clippy,native-build,native-smoke}-20261114.log`.
The two initial new-dependency logs are
`/tmp/open-gpui-dockspace-dcd-{adoption-tests,all-features}-20261114.log`.
Final verification extends into November 15; these paths retain the increment's
November 14 suffix. The previous intermittent native first-scene timeout retains
its unresolved status; two successful native runs here do not diagnose it.

### Browser integration attempts

The first wasm compile succeeds. Trunk's automatic wasm-bindgen 0.2.130 download
then times out twice; a bounded manual release download and an isolated tool
path make that version available without updating the shared installation.
An Edge attempt times out during the CDP handshake. The existing Chromium
151.0.7922.34 binary is used for subsequent browser execution.

The first real shared-session fixture fails validation because a rootless empty
surface is invalid. Its reserved probe surface now has an empty central root.
The next attempt traps during unoptimized host-frame preparation. Wasm inspection
shows a 1 MiB initial `__stack_pointer`; ten frames in the actual failing path
alone reserve 898,800 bytes, before their window/closure callers. The largest is
`DockspaceHostFrame::prepare_commit` at 349,136 bytes. An 8 MiB stack for this
example binary, with unchanged optimization level, permits the exact path to
complete and WebGL/Auto WebGPU pixel, input, capture and docking checks to run.
The final binary's build script owns this linker option; downstream wasm
applications choose their own stack allocation. Reducing core frame moves,
comparison clones and large pending values is a separate measured optimization
for dockspace, not a change to its transaction protocol in this adapter.

The initial fixture also wraps the entire portable platform-test interface in
a docking pane, changing its visibility/focus semantics. That causes an early
click race and then changes a later deactivation count after presentation
authority is intentionally revoked. The final unsupported-native probe retains
the original standalone `SmokeWeb` root, binds its actual native lifetime to the
shared session during construction, and requests a second root through
`open_native_root`. Its inspection accurately reports zero painted docking
mounts and one native root. Existing exact input/capture/activation and renderer
assertions remain intact. This browser probe qualifies native-open refusal and
binding preservation, not cross-window docking or physical OS capture.

Attempt logs use `/tmp/open-gpui-dockspace-shared-web-smoke-*-20261114.log`.

The corrected standalone-root full invocation passes once at the reviewed working
source and again at committed implementation `91c5871f`. The final log is
`/tmp/open-gpui-dockspace-published-browser-20261114.log`; the earlier passing
invocation is `/tmp/open-gpui-dockspace-shared-web-smoke-standalone-20261114.log`.
Both use the required gate without `--allow-unavailable`. Source, binary and
cleanup facts are retained in
`/tmp/open-gpui-dockspace-published-adoption-receipt-20261114.json`.

## Next integration work

Port the ten Windows journeys to the shared adapter in bounded slices, preserving
their original OS observations, exact output correlation and cleanup receipts.
The shared core and egui-native journey fixtures provide the semantic reference;
GPUI's own native input, capture and renderer owners remain authoritative. Once
those replacements execute, remove the remaining legacy graph/controller,
interaction, dogfood diagnostics and legacy-only benchmarks together.

In dockspace itself, measure and reduce debug host-frame stack demand before
claiming a smaller downstream stack requirement. Start with redundant old-state
clones in `reduce_tick_candidate` and full click/drag copies in reconciliation,
then evaluate large pending values or candidate ownership only if measurements
justify them. Do not introduce a second transaction protocol in the GPUI adapter.
Fatal-host classification and drain-then-rebuild recovery remain a separate,
explicit contract; logical checkpoints already supply its topology input.
