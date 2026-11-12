# Open GPUI iOS

`open-gpui-ios` connects the ordinary Open GPUI application to UIKit, CoreText and
the shared Apple Metal renderer. It retains native `UITextInput`, keyboard and
safe-area insets, raw touches, pointer scrolling, leading-edge back gestures,
Universal Links, scene lifecycle, clipboard, Keychain and process notifications.

## Source and license

The complete backend, example, app plist and text-input tests were imported from
Zed `78f5d7446c55f6c1aa9b66e8111f6b3f4625ebb0`, with latest iOS source commit
`fdcb1981103408e63fff5ef47c8ae2f9995262ad`. The reference checkout was read only.
The official [comparison to the selected rendering target](https://github.com/zed-industries/zed/compare/78f5d7446c55f6c1aa9b66e8111f6b3f4625ebb0...6ceb6fee8738274589b59139df27efc191ec2231.diff)
changes 35 paths and no `crates/gpui_ios` paths. iOS consumes that rendering
successor through `open-gpui-apple` and Core rather than duplicating a renderer.

`LICENSE-APACHE` is the full regular-file Apache-2.0 text. `NOTICE` records
provenance and adaptations. The upstream shell packager is replaced by
`examples/ios/build_simulator.py`.

## Workspace integration

`crates/gpui_ios` is registered in the root workspace members with this
`[workspace.dependencies]` alias:

```toml
open_gpui_ios = { package = "open-gpui-ios", version = "0.2.0", path = "crates/gpui_ios", default-features = false }
```

`open-gpui-platform` includes the iOS target dependency, selects its
`current_platform` implementation and reexports `IosPlatform`,
`StatusBarContentStyle` and `set_status_bar_style`. Its `test-support`,
`bench-support` and `runtime_shaders` features forward to this backend.
The root also registers `objc2-ui-kit` 0.3.2 with the required UIKit features.
iOS directly enables the shared Apple's `font-kit` feature. Its native test
dependency enables UIKit's `UIConversationContext` locally. `raw-window-handle = "0.6"` is local to this
crate; all other dependencies already have workspace entries.

Core provides the corresponding official `AppLifecyclePhase`,
`Platform::on_app_lifecycle`, `Platform::on_memory_warning` and App subscription
facades. Their delivery uses the original ordered native App ingress, so a
UIKit notification during an App borrow never invokes observers inline.
`PlatformInputHandler::focus_id()` is a public read-only getter returning
the fork's `FocusId`. UIKit retains that identity across temporary frame
checkouts and invalidates text positions/ranges when the document changes.

Core supplies the Apple pixel-buffer alias for `PlatformScreenCaptureFrame` on
iOS when `screen-capture` is enabled. This backend does not advertise native
screen capture. The simulator all-target/all-feature check includes this interface.

## Window and renderer ownership

Native callbacks use Core's `PlatformInputCallbackSlot` and
`PlatformInputHandlerSlot`, including callback leases and generation-safe
terminal touch cancellation. Resize, display, visibility, active and frame
callbacks feed the existing Core ingress. Initial visibility, activation and
keyboard operations execute through `PlatformWindowCommandDispatcher` after
the App borrow is released. Non-activating initial presentation is supported;
desktop placement, transient owners and provisional reveal are unsupported.
Activation reads the window's actual scene activation state and key-window
status, including windows created after the scene became active. Native key
and visibility notifications update the original window owners. Public
`activate_window` requests use Core's original application-wide request generation
and command FIFO. `Activated` requires the exact native positive observation or
the post-dispatch focused-window readback; command acceptance alone is insufficient.

Interaction revocation closes UIKit's key-window and text-responder eligibility,
resigns their current ownership and disables native pointer interaction without
detaching the rendering view. The native window retains the same one-way gate
even when its Rust owner is later released. Active queries and positive callbacks
honor that gate. Revocation retains an idle input handler for Core's existing
`take_input_handler` / `finish_composition` cleanup; an in-flight callback uses
Core's original callback lease and composition cleanup. The handler slot becomes
terminal during presentation shutdown. Direct terminal `FocusLost` invalidates
native text positions and ranges and clears keyboard eligibility.

UIKit's `resignKeyWindow` callback does not itself clear key-window ownership.
Revocation synchronously hides and restores the original visibility of a key
window after its one-way input gate closes. This releases key ownership while
retaining the same view, controller and renderer; subsequent native key/responder
entry cannot reopen the gate. The simulator probe checks both idle and in-flight
composition and requires another drawable submission after revocation.

Geometry comes from the connected scene and actual native view. Logical bounds,
scale and display association are updated before notification. Metal drawable
resize runs in `prepare_frame`, which invokes no callbacks. Insets are cached
for the frame. Zero-sized or hidden windows return `Deferred`; the actual
Metal renderer determines `Submitted`, `RepaintRequired` and rejection.

One demand-driven `CADisplayLink` services the scene. Pending requests retain
their presentation/force requirements and earliest known timestamp/source pair.
Hidden and zero-sized windows retain demand without consuming it or waking the
source; native show and layout changes resume eligible pending work. The source
parks after draining requests, wakes through the original Window invalidator,
and stays suspended while backgrounded. Reconnection retains pending demand.
An executor fallback serves platforms used before their native loop.

Shutdown is bound to the full WindowId and original exact shutdown ticket.
Preparation captures state without native effects. Execution revokes input,
reserves Metal shutdown, detaches UIKit's view, clears the backing layer's Metal
device, and waits for the original renderer's last submission.
Only then may native retirement hide/detach the UIWindow and publish one close
callback. Construction rollback and owner drop schedule this same work on the
foreground executor, retaining native and renderer owners through retry.

Idle-sleep guards reference-count UIKit's actual `idleTimerDisabled` state and
restore the previous setting after the final guard. A guard dropped off the
main thread queues its release on the main dispatch queue.

## Validation

The iOS simulator all-target/all-feature check passes on Apple Silicon with SDK
26.5 and deployment minimum 15.0, on stable Rust and the real Rust 1.93 compiler.
The independently locked iOS consumer and device-target production/all-feature
compilation also pass. Portable frame-demand and probe-oracle controls
pass in the final affected 3,615-test host cohort. These host controls do not run
UIKit. The native window probe also passes on the iPhone 17/iOS 26.3 simulator,
including the explicit `IOS NATIVE WINDOWING OK` marker. The serial integration verification records exact commands, results and
platform limits in the [refresh matrix](../../docs/imports/zed-gpui-commit-reconciliation-2026-11-12.md#platform-and-functional-unit-verification-matrix)
and [verification guide](../../docs/verification.md). The commands below describe
qualification entry points; only the receipts identify executed results.

Run Cargo commands serially in this shared checkout and reuse its target
directory. Install an iOS simulator SDK in Xcode and the Rust simulator target.
The supported deployment minimum is iOS 15; corner radii are queried only when
the newer UIKit API is available. An Intel Mac uses `x86_64-apple-ios` instead.

```text
rustup target add aarch64-apple-ios-sim
cargo check -p open-gpui-ios -p open-gpui-platform --all-targets --features test-support --target aarch64-apple-ios-sim
cargo check -p open-gpui-ios -p open-gpui-platform --all-features --target aarch64-apple-ios-sim
cargo check --manifest-path crates/gpui_ios/tests/consumer/Cargo.toml --target aarch64-apple-ios-sim
python3 crates/gpui_ios/examples/ios/build_simulator.py --locked
```

The consumer has its own workspace and dependency resolution, uses public App
and platform APIs, and subscribes to lifecycle and memory-warning events. Retain
and qualify its generated lock separately after initial resolution.

Boot a simulator and install/launch the ordinary example:

```text
xcrun simctl install booted target/gpui-ios-example/GPUIIosExample.app
xcrun simctl launch --console-pty booted dev.open-gpui.ios-example
```

Build and run the native window probe in the same way:

```text
python3 crates/gpui_ios/examples/ios/build_simulator.py --example native_windowing --features test-support --locked
xcrun simctl install booted target/gpui-ios-example/GPUIIosExample.app
xcrun simctl launch --console-pty booted dev.open-gpui.ios-example
```

The probe requires a real initial `Submitted` and opens another window after
the scene is active to check activation and native key transfer. It supersedes an
older public activation request and requires the latest generation's ticket to
reach `Activated`, checks an already-key request, and compares native key state,
Core active state and focused-window readback. It does not use a native
`makeKeyAndVisible` shortcut. Its six pixel
oracles check expected colors and distinguish clipped interiors/exteriors,
background/border, quadratic curvature, layered overlap, translucent-over-opaque
order, rounded corners and visible shadow. RGB samples allow three byte values
of error; shadow samples use a broad neutral-gray interval. Solid black and
white images fail every case. These are smoke checks rather than numerical
qualification of antialiasing or the shadow approximation.

Immediately before native resize, the probe captures the latest Core submitted
generation and native submission serial. It requires both to advance and the
submitted drawable texture's actual extent to match the target. The ordinary
iOS draw path supplies that same drawable to the shared Metal renderer and
records its extent only when the renderer returns `Submitted`; the probe reads
those facts through private native view selectors, without another public Rust
test interface. It also exercises overlapping/off-thread idle guards and retains
native objects through Core's close path to check terminal uniqueness and
irreversible Metal-layer retirement. Its success marker is
`IOS NATIVE WINDOWING OK`; failure or timeout exits nonzero.

`examples/native_windowing/interaction.rs` opens real UIKit windows with accepted
Core input handlers and sends `setMarkedText:selectedRange:` through the native
text responder. It checks exactly one Core composition cleanup for both an idle
handler and revocation inside the active callback. It then attempts native input,
stale-range access, responder/key reacquisition and public activation after
revocation. A newer actual drawable submission must still occur while the native
window remains visible and its layer retains the renderer; only the subsequent
exact Core close may publish native terminal and release the Metal device. These
programmatic protocol checks do not qualify a particular software/hardware IME.

`tests/native_windowing_oracles.rs` exercises the probe's actual pixel and resize
oracles with failure controls. The production `FrameDemand` owner has unit
regressions for parked demand, merged requirements and shutdown cancellation.
These portable assertions do not execute UIKit or Metal.

The first simulator probe exposes a duplicate-root-update fixture error; window-only
quiescence and close now use `AsyncApp::update_window`. The next run exposes real
UIKit key ownership surviving `resignKeyWindow`, which the visibility-preserving
revocation above corrects. A final property control shows that `CAMetalLayer`
ignores a zero `drawableSize` request even after accepting 1×1. Retirement therefore
checks real custody: no Metal device, no device revival after layout, no next
drawable, detached native objects and exactly one terminal callback. The obsolete
zero-metadata assertion and ineffective setter are removed. The successful probe
retains a UIKit unbalanced-appearance-transition warning; physical-device and
system-input behavior remain separate qualification below.

The imported standalone `text_input` test uses the process main thread rather
than the default Rust test worker. Build it serially and run its reported binary
with the simulator:

```text
cargo test -p open-gpui-ios --test text_input --target aarch64-apple-ios-sim --no-run --message-format=json-render-diagnostics
xcrun simctl spawn booted /absolute/path/to/the/reported/text_input-binary
```

Source parsing/formatting, plist/Python/TOML validation and portable host tests
do not establish iOS target compilation or UIKit/Metal execution.

## Remaining native qualification

The official single-connected-scene limitation remains. Display enumeration is
retained; opening a window on an unrelated screen is refused. File/save pickers,
document sharing, window prompts, cursor/hover/caps-lock tracking, native
accessibility, screen capture and GPU-spec reporting retain the upstream's
unsupported behavior. Desktop menu, movement, resize and fullscreen commands
are not advertised as UIKit capabilities. `current_platform(true)` retains the
official UIKit implementation and is not an iOS headless renderer.

Qualify software/hardware keyboard composition, changing focused documents,
stale UIKit ranges, safe-area and keyboard transitions, multitouch cancellation,
back gestures, rotation/iPad resizing, background/foreground/reconnection,
Universal Links and physical-device GPU retirement. Simulator pixels and
compilation alone do not establish those system behaviors or performance.
