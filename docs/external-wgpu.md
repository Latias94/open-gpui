# Application-Owned WGPU Rendering

Open-GPUI can reuse an application's existing WGPU device and encode UI work in the application's
command encoder. GPU recording, imported images, native renderer selection and external event-loop
hosting are separate capabilities. Application-owned Win32 hosting has synchronous and explicitly
deferred delivery paths; their scoped qualification and remaining platform limits are separate.

| Capability | Interface | Owner |
| --- | --- | --- |
| Scene recording | `WgpuSceneRenderer` | Application supplies adapter, enabled device, queue, target and encoder; submits and presents. |
| Imported GPU image | `WgpuAtlas::import_image`, `Window::paint_image_texture` | Application orders producer work and retains its allocation; framework preserves UI ordering and leases. |
| Win32 renderer selection | `WindowsPlatform::new_with_renderer` | Application supplies a synchronous renderer and text service; GPUI owns HWND, message loop, input/IME and retirement. |
| External Win32 HWND/event loop | `new_external_host`, `attach_window`, `pretranslate_message` | Application retains its native window/loop; adapter supplies native UI services and detaches after synchronous submission quiesces. See scoped qualification. |
| Delayed Scene delivery | `PlatformFrameDeliveryMode::Deferred`, `draw_with_submission` | Original Window retains one prepared candidate and its submitted publications; application completes the exact original token after actual submission. |

Use the workspace's exact WGPU source and compatible package requirements. A matching Rust type is
necessary; supplying the same original device is an additional requirement. Never reinterpret
handles from another WGPU source or device. Open-GPUI creates no device or device-wide callback
for the recording interface. The application selects its device features/limits before construction;
the renderer validates its documented requirements against the enabled device.

## Record A Scene

Construct one renderer for the target format and supply its original atlas to the platform Window
that produces the Scene:

```rust,ignore
let mut renderer = WgpuSceneRenderer::new(
    &adapter,
    device.clone(),
    queue.clone(),
    wgpu::TextureFormat::Rgba8Unorm,
)?;
let atlas = renderer.sprite_atlas().clone();
```

Given an accepted Scene from that Window, encode it after the application's other required work:

```rust,ignore
let recording = renderer.record(
    &scene,
    &mut encoder,
    &target,
    wgpu::LoadOp::Load,
)?;
let submission = queue.submit([encoder.finish()]);
in_flight.push((submission, recording));
```

Retain each recording in the host's in-flight collection through its corresponding GPU completion,
then release it on the owning UI thread. A queue callback can publish a completed-prefix value or
wake the host; the recording itself contains framework atlas custody and is not a Send callback
payload. Recording is not a framework
submission acknowledgement or proof of native presentation. The caller can record several scenes
before one submit; per-scene uploads are ordered in the encoder. Do not submit an encoder after a
recording error which may have emitted partial commands. Known target-contract errors are refused
before commands are added.

The target is a complete, matching, single-sample, single-layer 2D color texture; rendering selects
mip zero. Explicit Clear and Load are supported. Internal path rasterization may require several
private passes, so the interface takes a command encoder instead of an already-open render pass.

Scene production is not supplied by the renderer. A cloned Scene contains drawing data but does not
transfer its original Window, atlas, accepted-frame or native-submission authority. The public GPU
fixture uses a simulated headless Window solely to obtain Scenes; applications must not use that
fixture as a production external-host driver.

## Import An Application Texture

Import a subresource through the renderer's original atlas:

```rust,ignore
let image = atlas.import_image(WgpuImportedImageDescriptor {
    texture: viewport_texture.clone(),
    mip_level: 0,
    array_layer: 0,
    region: Bounds::new(
        point(DevicePixels(0), DevicePixels(0)),
        size(DevicePixels(width), DevicePixels(height)),
    ),
    transfer: ImageTextureTransfer::Srgb,
    alpha: ImageTextureAlpha::Straight,
    sampling: ImageTextureSampling::Linear,
    owner: Some(viewport_allocation.clone()),
})?;
```

In an Element or canvas paint callback, use `window.paint_image_texture(bounds, corner_radii,
&image)`. The selected region fills those logical bounds; normal opacity, rounded corners,
subtree transforms, clipping and painter order apply. The image must belong to the exact current
Window atlas. An atlas reset invalidates new paint with an old image; import a new image and
invalidate the corresponding UI description instead of rebinding the old handle.

Imports retain the original texture and one explicit 2D mip/layer view without copying texels.
Linear filtering clamps to the selected region's edge texel centers; nearest filtering selects
texels in that region. Supported formats are RGBA/BGRA 8-bit unorm/sRGB and linear RGBA16Float.
Depth, integer, multisample, arbitrary view reinterpretation and nonfilterable float imports are
refused. Known descriptor errors do not publish an atlas registration.

`transfer` describes stored RGB. An sRGB texture format automatically decodes RGB during sampling,
and must declare sRGB storage. `alpha` describes the values after any such decoding. In particular,
premultiplied RGB must be premultiplied in that sampled domain: an sRGB view requires stored sRGB
encodings of linear premultiplied RGB. The renderer unpremultiplies before transfer conversion and
preserves GPUI's existing working values. This is not a promise of linear-light UI composition,
HDR tone mapping or wide-gamut color management; those remain application/render-policy decisions.

The optional `owner` can retain a pooled allocation or producer-specific resource. Frame leases
protect the registered instance, and recording receipts retain the exact texture/view/owner through
their original completion, even if the atlas is reset or the renderer is dropped. Built-in WGPU
submission paths retain imported owners through their queue completion callbacks. Owner destructors
run outside the atlas lock. Release application-only ownership when convenient, but do not explicitly
destroy or reuse an allocation while submitted work still samples it. An image handle preserves
resource identity, not immutable pixel contents; producer writes and sampling order remain explicit.
On browser targets, the queue callback publishes completion without capturing non-Send GPU
resources; a local future retains and releases those resources on the originating browser thread.

## Select A Win32 Renderer

`WindowsPlatform::new_with_renderer(text_system, renderer_factory)` preserves the ordinary Win32
window and native-service implementation while selecting application-supplied graphics and text.
Implement `WindowsRendererFactory` and `WindowsRenderer` in the application. The factory receives
the exact borrowed HWND and framework Window identity; creation failure fails that original open
transaction. No Direct3D device or DirectWrite GPU resources are created in this mode.
Applications depending directly on the Windows package must enable core's `windows-manifest`
feature or provide an equivalent Common Controls v6 application manifest. The ordinary platform
selector enables that feature for its Windows consumers; renderer injection does not replace
the executable's native manifest.

`draw` must complete a real submission/presentation handoff before returning `Submitted`. It must
not pump messages, reenter GPUI or claim an encoder recording as a submitted frame. The renderer
receives physical resize requests and must release its surface and settle the original submitted
GPU prefix before quiescence succeeds. An error retains the original native owner for retry;
the renderer must never destroy the HWND or an application's shared device.

The injected `PlatformTextSystem` supplies shaping and rasterization independently of the Win32
IME/input bridge. The existing `CosmicTextSystem` is one option; its output need not be identical
to DirectWrite. Report subpixel support truthfully for the renderer and chosen text service.

Run the ordinary supplied-renderer example on Windows:

```text
cargo run --locked -p open-gpui-wgpu-native
cargo run --locked -p open-gpui-wgpu-native -- --smoke
```

The example updates an imported texture through a GPU render pass when its normal button changes
application state. It composes clipped GPU content beneath ordinary controls, retains recordings
on the UI thread and observes completion through a Send-only prefix notification. Its readback-free
render path demonstrates renderer selection, with GPUI still owning the native window and loop.
The optional smoke mode waits for an initial submission, updates ordinary application state,
waits for the changed-image submission, then requests normal application shutdown. It reports
the original quiesced GPU prefix; it does not replace pixel or physical input/IME qualification.

This mode still uses GPUI-created windows, its native loop and ordinary synchronous candidate
acceptance. Application-owned HWNDs use the separate mode below. Deferred delivery is an explicit
mode in that same Window lifecycle; selecting a synchronous renderer does not enable it.

## Drive An Application-Owned Win32 Window

Construct `WindowsPlatform::new_external_host(text_system, renderer_factory)` and start
`Application::with_platform(platform.clone()).run_embedded(...)`. Keep the returned
`ApplicationHandle` and platform alive while the host message loop runs. This platform starts
ordinary native services and launch but does not enter its own blocking loop or terminate the
host process. It reuses the existing foreground dispatcher and native Window implementation.

Inside launch, use the normal App creation transaction with a supplied platform-window factory:

```rust,ignore
let window = cx.open_window_with_platform_window(
    options,
    move |handle, params, displays| {
        platform.attach_window(handle, params, displays, host_window.clone())
    },
    |window, cx| cx.new(|cx| Root::new(window, cx)),
)?;
```

`host_window` implements `raw_window_handle::HasWindowHandle` and retains the actual native
window. An `Rc` or `Arc` clone is suitable when it supplies that implementation. The adapter
checks the actual Win32 handle, process/thread ownership and exact display snapshot. Its current
entry accepts a hidden, non-child normal/floating top-level window on the platform's UI thread,
without transient/provisional ownership, and requires input styles to match the supplied policy.
The hidden first presentation remains in App's original creation/rollback transaction. Supplying
an already-visible window or arbitrary child surface needs further capability work.

The host owns its WndProc, userdata and non-client geometry. GPUI attaches native input/IME,
accessibility and frame services through a generation-bound subclass. Call
`platform.pretranslate_message(&message)` before `TranslateMessage`/`DispatchMessage`; consume
that message only when it returns `true`. Native dispatch must happen outside an active
`ApplicationHandle::update` borrow. The host continues to handle other messages, including its
private messages and `WM_QUIT`. GPUI does not remove or post that quit message in external mode.

Request ordinary GPUI quit or close, then keep pumping until `platform.is_shutdown_complete()`.
The original renderer must quiesce its submitted GPU prefix before the adapter removes its
services/subclass and publishes binding closure. Failed quiescence retains that original
authority for retry. `WindowPresentationShutdownSnapshot::detached()` describes the successful
binding release; `native_terminal()` remains reserved for actual OS destruction. The host HWND
remains alive and enabled, with its userdata and original IME association restored. Only the host
then chooses to destroy its native window or continue using it. Destroying the HWND prematurely
can violate the renderer's shutdown ordering and is not successful detachment.

Run the ordinary application in this mode:

```text
cargo run --locked -p open-gpui-wgpu-native -- --external-host
cargo run --locked -p open-gpui-wgpu-native -- --external-host --smoke
```

The [consumer guide](../examples/wgpu-native/README.md) describes its actual message, geometry,
resource and shutdown controls. Queue-driven UTF-16/key checks do not qualify physical IME or
monitor/DPI transitions. The default renderer mode submits synchronously. Neither mode by itself
establishes Nara Host/View, a production no-native-window logical root or authored game-UI acceptance.

## Defer An Original Candidate

A Windows renderer opts into `PlatformFrameDeliveryMode::Deferred` and implements
`draw_with_submission(scene, background, submission)`. The normal `draw` remains the synchronous
entry. The deferred callback may encode the Scene into an application-owned command buffer,
retain its `WgpuSceneRecording`, acquired target and original `PlatformFrameSubmission`, then
return `Deferred`. Retain one packet; do not replace it with the newest description or read current
application values later to reconstruct the original producer work.

After the callback and any App borrow return, the host validates that same packet's native target,
physical extent and device/resource basis. Check `submission.is_pending()` immediately before
recording or submitting: resource invalidation settles the original attempt as `RepaintRequired`.
The native adapter revokes it before resize/DPI or shutdown changes can authorize later work.
Submit/present the original commands, then call `submission.complete(Submitted)`. Recording alone
does not permit completion. GPU completion separately controls the original recording's lifetime.
Drop an invalidated packet without submitting it; do not retarget it to the current surface.

Completion is UI-thread work and enters the original ordered App ingress. It does not reenter the
App from the renderer. Clones share one terminal outcome; late or duplicate completion cannot
accept a replacement which reuses a frame generation. A pending candidate preserves the submitted
hit/focus/text/accessibility publications. Mutating input or another invalidation cancels pending
work before changing that basis, while ordinary source updates retain their next-frame demand.
Publication callbacks run only when the original candidate is committed. This does not roll back
arbitrary application writes performed by render/prepaint callbacks.

The initial open may return a still-hidden window whose initial-presentation status is `Pending`.
Keep pumping; the original native reveal command waits for its exact nonempty submission. Do not
manually show it or expose its candidate interaction before completion. Refusal and empty scenes
use the original bounded initial-presentation recovery, and close cancels unfinished delivery.

Run the ordinary host-controlled trajectory:

```text
cargo run --locked -p open-gpui-wgpu-native -- --external-host --deferred --smoke
```

The [qualification](verification.md#deferred-scene-submission-and-submitted-publications) distinguishes
CPU state-ordering tests, actual native GPU submissions, readback scope and remaining physical
input/platform/performance requirements. Deferred profiler samples retain CPU draw completion
before the host wait and the original submission timestamp; no measured GPU duration or scanout
latency is inferred from processing a completion message.

See [the executable recording consumer](../crates/gpui_wgpu/tests/external_scene.rs),
[the native supplied-renderer trajectory](../crates/gpui_windows/src/native_custom_renderer_tests.rs),
[qualification and limits](verification.md), and the
[renderer lifecycle decision](adr/0031-open-gpui-renderer-surface-lifecycle-authority.md).
